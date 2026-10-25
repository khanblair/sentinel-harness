//! The macOS app window: the same local UI, served by the same daemon, inside a native window.
//! Closing the window only hides it. The daemon keeps serving a phone and Telegram until the app
//! is asked to quit.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
};
use std::time::Duration;

use anyhow::Result;
use muda::MenuId;
use sentinel_daemon::engine::Engine;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use wry::{NewWindowResponse, PageLoadEvent, WebView, WebViewBuilder};

use crate::app;
use crate::commands;
use crate::serve;
use crate::window_menu::{self, WindowMenu};

/// Matches the dark page, so the first paint and the resize edges are not white.
const BACKGROUND: (u8, u8, u8, u8) = (26, 26, 26, 255);
const QUIT_WAIT: Duration = Duration::from_secs(3);
const SMOKE_LOAD_WAIT: Duration = Duration::from_secs(40);
const SMOKE_SETTLE: Duration = Duration::from_secs(1);

#[derive(Debug, Clone)]
pub(crate) enum UserEvent {
    Quit,
    Menu(MenuId),
}

struct StartedDaemon {
    engine: Engine,
    server: tokio::task::JoinHandle<Result<ExitCode>>,
    url: String,
    origin: String,
}

struct WindowState {
    window: Window,
    webview: WebView,
    menu: WindowMenu,
    engine: Engine,
    runtime: tokio::runtime::Runtime,
    server: tokio::task::JoinHandle<Result<ExitCode>>,
}

/// A page-load signal for the CI smoke run. The window itself cannot be checked in CI.
struct SmokeSignal {
    loaded: Arc<AtomicBool>,
    loaded_tx: Sender<()>,
}

/// Open the app window for `repo`, or focus the app that already serves this project.
pub(crate) fn run(repo: Option<PathBuf>) -> Result<ExitCode> {
    let Some(repo) = app::choose_repo(repo)? else {
        return Ok(ExitCode::SUCCESS);
    };
    app::remember(&repo);
    if serve::is_running(&repo) {
        focus_running_app();
        return Ok(ExitCode::SUCCESS);
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let started = runtime.block_on(start_daemon(&repo))?;
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    spawn_quit_watch(&runtime, &started.engine, &proxy);
    let window = build_window(&event_loop)?;
    let smoke = start_smoke(&proxy);
    let webview = build_webview(&window, &started.url, &started.origin, smoke)?;
    let menu = window_menu::install(proxy)?;
    run_events(
        event_loop,
        WindowState {
            window,
            webview,
            menu,
            engine: started.engine,
            runtime,
            server: started.server,
        },
    )
}

/// The app is already running for this project. Bring its window forward.
fn focus_running_app() {
    // A failure only leaves the user to click the Dock icon themselves.
    let _ = Command::new("open").args(["-a", "Sentinel"]).status();
}

async fn start_daemon(repo: &Path) -> Result<StartedDaemon> {
    let engine = serve::prepare(repo, false, false).await?;
    let bound = app::bind_here(&engine).await?;
    serve::publish(repo, &bound)?;
    let (addr, token) = serve::daemon_access(repo)?;
    let server_engine = engine.clone();
    let server_repo = repo.to_path_buf();
    let server =
        tokio::spawn(
            async move { serve::run_until_stopped(&server_engine, &server_repo, bound).await },
        );
    Ok(StartedDaemon {
        engine,
        server,
        url: url_from(&addr, &token),
        origin: format!("http://{addr}"),
    })
}

fn spawn_quit_watch(
    runtime: &tokio::runtime::Runtime,
    engine: &Engine,
    proxy: &EventLoopProxy<UserEvent>,
) {
    let engine = engine.clone();
    let proxy = proxy.clone();
    runtime.spawn(async move {
        engine.quit_requested().await;
        let _ = proxy.send_event(UserEvent::Quit);
    });
}

fn build_window(event_loop: &EventLoop<UserEvent>) -> Result<Window> {
    Ok(WindowBuilder::new()
        .with_title("Sentinel")
        .with_inner_size(LogicalSize::new(1280.0, 820.0))
        .with_min_inner_size(LogicalSize::new(900.0, 600.0))
        .with_background_color(BACKGROUND)
        .build(event_loop)?)
}

fn build_webview(
    window: &Window,
    url: &str,
    origin: &str,
    smoke: Option<SmokeSignal>,
) -> Result<WebView> {
    let origin = origin.to_owned();
    let downloads = commands::home_dir().join("Downloads");
    Ok(WebViewBuilder::new()
        .with_url(url)
        .with_devtools(false)
        .with_background_color(BACKGROUND)
        .with_navigation_handler(move |next| allow_navigation(&origin, &next))
        .with_new_window_req_handler(|next, _| {
            open_external(&next);
            NewWindowResponse::Deny
        })
        .with_download_started_handler(move |source, path| {
            let name = path.file_name().map_or_else(
                || filename_from_url(&source),
                |name| name.to_string_lossy().into_owned(),
            );
            *path = download_target(&downloads, &name);
            true
        })
        // Returning false leaves the drop to the page, so the Add files dialog keeps working.
        .with_drag_drop_handler(|_| false)
        .with_on_page_load_handler(move |event, _| report_page_load(smoke.as_ref(), &event))
        .build(window)?)
}

fn report_page_load(smoke: Option<&SmokeSignal>, event: &PageLoadEvent) {
    let Some(smoke) = smoke else {
        return;
    };
    if !matches!(event, PageLoadEvent::Finished) || smoke.loaded.swap(true, Ordering::SeqCst) {
        return;
    }
    println!("WINDOW_LOADED");
    let _ = smoke.loaded_tx.send(());
}

/// Run the CI smoke run: quit one second after the page loads, or fail after 40 seconds.
fn start_smoke(proxy: &EventLoopProxy<UserEvent>) -> Option<SmokeSignal> {
    std::env::var_os("SENTINEL_WINDOW_SMOKE")?;
    let loaded = Arc::new(AtomicBool::new(false));
    let (loaded_tx, loaded_rx) = std::sync::mpsc::channel();
    let timeout_loaded = Arc::clone(&loaded);
    std::thread::spawn(move || {
        std::thread::sleep(SMOKE_LOAD_WAIT);
        if !timeout_loaded.load(Ordering::SeqCst) {
            println!("WINDOW_TIMEOUT");
            std::process::exit(3);
        }
    });
    let quit_proxy = proxy.clone();
    std::thread::spawn(move || {
        if loaded_rx.recv().is_ok() {
            std::thread::sleep(SMOKE_SETTLE);
            let _ = quit_proxy.send_event(UserEvent::Quit);
        }
    });
    Some(SmokeSignal { loaded, loaded_tx })
}

fn run_events(event_loop: EventLoop<UserEvent>, mut state: WindowState) -> ! {
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        handle_event(&mut state, &event, control_flow);
    });
}

fn handle_event(state: &mut WindowState, event: &Event<UserEvent>, control_flow: &mut ControlFlow) {
    if matches!(
        event,
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        }
    ) {
        state.hide();
    } else if matches!(
        event,
        Event::WindowEvent {
            event: WindowEvent::Destroyed,
            ..
        }
    ) || matches!(event, Event::UserEvent(UserEvent::Quit))
    {
        state.quit(control_flow);
    } else if matches!(event, Event::Reopen { .. }) {
        state.show();
    } else if let Event::UserEvent(UserEvent::Menu(id)) = event {
        handle_menu(state, id, control_flow);
    }
}

fn handle_menu(state: &mut WindowState, id: &MenuId, control_flow: &mut ControlFlow) {
    if state.menu.is_quit(id) {
        state.quit(control_flow);
    } else if state.menu.is_close(id) {
        state.hide();
    } else if state.menu.is_reload(id) {
        state.reload();
    }
}

impl WindowState {
    fn hide(&self) {
        self.window.set_visible(false);
    }

    fn show(&self) {
        self.window.set_visible(true);
        self.window.set_focus();
        let _ = self.webview.focus();
    }

    fn reload(&self) {
        let _ = self.webview.evaluate_script("location.reload()");
    }

    fn quit(&mut self, control_flow: &mut ControlFlow) {
        self.engine.request_quit();
        // Wait for the server task so `.sentinel/daemon.addr` is removed before the process ends.
        let _ = self
            .runtime
            .block_on(tokio::time::timeout(QUIT_WAIT, &mut self.server));
        *control_flow = ControlFlow::ExitWithCode(0);
    }
}

/// The page stays inside the daemon. Any other web address goes to the browser.
fn allow_navigation(origin: &str, next: &str) -> bool {
    if is_inside(origin, next) {
        return true;
    }
    if is_web_address(next) {
        open_external(next);
    }
    false
}

fn is_web_address(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

fn open_external(url: &str) {
    // A failed open leaves the page where it was, and the window stays usable.
    let _ = Command::new("open").arg(url).spawn();
}

fn is_inside(origin: &str, url: &str) -> bool {
    let (Some((want_scheme, want_host)), Some((scheme, host))) =
        (split_origin(origin), split_origin(url))
    else {
        return false;
    };
    scheme.eq_ignore_ascii_case(want_scheme) && host.eq_ignore_ascii_case(want_host)
}

fn split_origin(url: &str) -> Option<(&str, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    (!scheme.is_empty() && !authority.is_empty()).then_some((scheme, authority))
}

fn url_from(addr: &str, token: &str) -> String {
    format!("http://{addr}/#t={token}")
}

fn filename_from_url(url: &str) -> String {
    let path = url.find(['?', '#']).map_or(url, |end| &url[..end]);
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.is_empty() {
        "download".to_owned()
    } else {
        name.to_owned()
    }
}

fn download_target(dir: &Path, filename: &str) -> PathBuf {
    let first = dir.join(filename);
    if !first.exists() {
        return first;
    }
    let name = Path::new(filename);
    let stem = name.file_stem().map_or_else(
        || filename.to_owned(),
        |part| part.to_string_lossy().into_owned(),
    );
    let extension = name
        .extension()
        .map_or_else(String::new, |part| format!(".{}", part.to_string_lossy()));
    let mut copy = 1;
    loop {
        let candidate = dir.join(format!("{stem} ({copy}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
        copy += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: &str = "http://127.0.0.1:7878";

    #[test]
    fn an_address_with_the_same_scheme_host_and_port_is_inside() {
        assert!(is_inside(ORIGIN, "http://127.0.0.1:7878/x"));
        assert!(is_inside(ORIGIN, "http://127.0.0.1:7878/#t=tok"));
    }

    #[test]
    fn an_address_outside_the_origin_is_rejected() {
        assert!(!is_inside(ORIGIN, "http://127.0.0.1:7879/"));
        assert!(!is_inside(ORIGIN, "https://evil.example"));
        assert!(!is_inside(ORIGIN, "http://evil.example:7878/"));
    }

    #[test]
    fn the_url_carries_the_token_in_the_fragment() {
        assert_eq!(
            url_from("127.0.0.1:7878", "tok"),
            "http://127.0.0.1:7878/#t=tok"
        );
    }

    #[test]
    fn a_free_name_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            download_target(dir.path(), "report.csv"),
            dir.path().join("report.csv")
        );
    }

    #[test]
    fn a_taken_name_gets_the_next_number_before_the_extension() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("report.csv"), "old").unwrap();
        assert_eq!(
            download_target(dir.path(), "report.csv"),
            dir.path().join("report (1).csv")
        );
        std::fs::write(dir.path().join("report (1).csv"), "old").unwrap();
        assert_eq!(
            download_target(dir.path(), "report.csv"),
            dir.path().join("report (2).csv")
        );
    }
}
