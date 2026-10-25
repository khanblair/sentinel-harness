//! The app menu bar. The web view needs it for the standard clipboard shortcuts to work.

use anyhow::Result;
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tao::event_loop::EventLoopProxy;

use crate::window::UserEvent;

const QUIT: &str = "quit-sentinel";
const RELOAD: &str = "reload";
const CLOSE: &str = "close-window";

/// The menu must stay alive for as long as the window, or macOS drops the items.
pub(crate) struct WindowMenu {
    _menu: Menu,
    quit: MenuId,
    reload: MenuId,
    close: MenuId,
}

impl WindowMenu {
    pub(crate) fn is_quit(&self, id: &MenuId) -> bool {
        id == &self.quit
    }

    pub(crate) fn is_reload(&self, id: &MenuId) -> bool {
        id == &self.reload
    }

    pub(crate) fn is_close(&self, id: &MenuId) -> bool {
        id == &self.close
    }
}

/// Build the menu bar and forward its clicks to the event loop.
pub(crate) fn install(proxy: EventLoopProxy<UserEvent>) -> Result<WindowMenu> {
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = proxy.send_event(UserEvent::Menu(event.id));
    }));
    let quit = MenuItem::with_id(QUIT, "Quit Sentinel", true, Some(shortcut(Code::KeyQ)));
    let reload = MenuItem::with_id(RELOAD, "Reload", true, Some(shortcut(Code::KeyR)));
    let close = MenuItem::with_id(CLOSE, "Close Window", true, Some(shortcut(Code::KeyW)));
    let app = app_menu(&quit)?;
    let edit = edit_menu()?;
    let view = view_menu(&reload)?;
    let window = window_menu(&close)?;
    let menu = Menu::with_items(&[&app, &edit, &view, &window])?;
    menu.init_for_nsapp();
    // Must come after init_for_nsapp, as muda documents.
    window.set_as_windows_menu_for_nsapp();
    Ok(WindowMenu {
        _menu: menu,
        quit: quit.id().clone(),
        reload: reload.id().clone(),
        close: close.id().clone(),
    })
}

fn shortcut(code: Code) -> Accelerator {
    Accelerator::new(Modifiers::META, code)
}

fn app_menu(quit: &MenuItem) -> Result<Submenu> {
    Ok(Submenu::with_items(
        "Sentinel",
        true,
        &[
            &PredefinedMenuItem::about(None, None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            quit,
        ],
    )?)
}

fn edit_menu() -> Result<Submenu> {
    Ok(Submenu::with_items(
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(None),
            &PredefinedMenuItem::redo(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::cut(None),
            &PredefinedMenuItem::copy(None),
            &PredefinedMenuItem::paste(None),
            &PredefinedMenuItem::select_all(None),
        ],
    )?)
}

fn view_menu(reload: &MenuItem) -> Result<Submenu> {
    Ok(Submenu::with_items(
        "View",
        true,
        &[reload, &PredefinedMenuItem::fullscreen(None)],
    )?)
}

fn window_menu(close: &MenuItem) -> Result<Submenu> {
    Ok(Submenu::with_items(
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(None),
            close,
            &PredefinedMenuItem::bring_all_to_front(None),
        ],
    )?)
}
