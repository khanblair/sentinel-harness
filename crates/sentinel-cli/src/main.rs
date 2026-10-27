//! The `sentinel` command: start the daemon, run a backlog from the terminal, and look at the
//! environment.

mod app;
mod commands;
mod doctor_host;
mod login_path;
mod projects;
mod selftest;
mod serve;
mod service;
#[cfg(all(feature = "window", target_os = "macos"))]
mod window;
#[cfg(all(feature = "window", target_os = "macos"))]
mod window_menu;
mod writeup;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "sentinel",
    version,
    about = "Run many coding agents in one shared working tree."
)]
struct Cli {
    /// With no command, the macOS app starts itself (`sentinel app`), and a terminal shows help.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run a backlog from the terminal and print what happens.
    Run {
        /// The backlog file. It may be relative to the repository.
        backlog: String,
        /// The repository to work in. Defaults to the current folder.
        #[arg(long)]
        repo: Option<PathBuf>,
        /// The budget cap in dollars, for example 1.50.
        #[arg(long)]
        cap: Option<String>,
        /// The command the barrier runs, for example `npm test` or `node scripts/run-suite.js`.
        /// Without it, Sentinel uses the saved setting, then the project's own test command.
        #[arg(long)]
        test_cmd: Option<String>,
        /// The agents of the run, cheapest first, for example `claude=haiku,openrouter=qwen/qwen3-coder`.
        /// A tool may appear twice with different models. Without it, the routing picks the agents.
        #[arg(long)]
        agents: Option<String>,
    },
    /// Find the agent CLIs on this computer.
    Scan {
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Check Rust, tools, git, and Tailscale, then the always-on host.
    Doctor {
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Print the report of the last run.
    Report {
        #[arg(long)]
        repo: Option<PathBuf>,
        /// Print a one-page Markdown draft of what the run did, built from the report and the log.
        #[arg(long)]
        writeup: bool,
    },
    /// Serve the UI and the API for a repository until you press Ctrl+C.
    Daemon {
        #[arg(long)]
        repo: Option<PathBuf>,
        /// The port to listen on. 0 picks a free one.
        #[arg(long, default_value_t = 7878)]
        port: u16,
        /// Serve on 127.0.0.1 without Tailscale. For development only; never binds 0.0.0.0.
        #[arg(long)]
        dev_bind: bool,
        /// Run as an always-on host: no native folder dialog, the folder browser instead.
        #[arg(long)]
        headless: bool,
    },
    /// Open the UI of the running daemon in the browser, signed in as this computer.
    Open {
        #[arg(long)]
        repo: Option<PathBuf>,
        /// Print the link instead of opening it.
        #[arg(long)]
        print: bool,
    },
    /// Print the link and one-time code for pairing a phone.
    Pair {
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Run the daemon at boot and restart it when it stops (launchd on macOS, systemd on Linux).
    Service {
        #[command(subcommand)]
        action: ServiceCommand,
    },
    /// Keep an API key in the system keychain. `set` reads the key from standard input.
    Keys {
        #[command(subcommand)]
        action: KeysCommand,
    },
    /// What the macOS app runs: pick a project, serve it on this computer, and open the UI.
    App {
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Stop the running daemon.
    Stop {
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Make a small Python project with a test, to try Sentinel on. Prints its folder.
    Starter {
        /// The folder to make it in. Defaults to your home folder.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Serve the file and Brain tools over stdio for an agent. Agent CLIs start it; no need to run it by hand.
    Mcp,
    /// Run scripted bad agents on a throwaway repository and print a PASS or FAIL line for each check.
    Selftest {
        /// Run only the scenario with this name: collider, tamperer, liar, overspender, or crasher.
        #[arg(long)]
        only: Option<String>,
    },
}

#[derive(Subcommand)]
enum ServiceCommand {
    /// Write the service file and start it.
    Install {
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long, default_value_t = 7878)]
        port: u16,
        /// Only write the file. Do not start the service.
        #[arg(long)]
        no_start: bool,
    },
    /// Stop the service and remove its file.
    Uninstall,
    /// Say whether the service is installed.
    Status,
}

#[derive(Subcommand)]
enum KeysCommand {
    /// Save a key. Example: echo "$KEY" | sentinel keys set openrouter
    Set { name: String },
    /// Remove a key.
    Delete { name: String },
    /// Say whether a key is set. The key itself is never printed.
    Status { name: String },
}

fn main() -> ExitCode {
    let Some(command) = Cli::parse().command.or_else(default_command) else {
        return show_help();
    };
    // The app and the daemon need the agent CLIs on PATH. Other commands stay fast.
    if matches!(command, Command::App { .. } | Command::Daemon { .. }) {
        login_path::ensure_login_path();
    }
    run(command)
}

/// The window owns the main thread on macOS, so it cannot sit inside a tokio runtime.
fn run(command: Command) -> ExitCode {
    match command {
        #[cfg(all(feature = "window", target_os = "macos"))]
        Command::App { repo } => match window::run(repo) {
            Ok(code) => code,
            Err(error) => report(error),
        },
        other => run_commands(other),
    }
}

fn run_commands(command: Command) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return report(error),
    };
    match runtime.block_on(dispatch(command)) {
        Ok(code) => code,
        Err(error) => report(error),
    }
}

fn report(error: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {error:#}");
    ExitCode::from(2)
}

/// Finder starts an app with no arguments. Inside a `.app` bundle that means: open Sentinel.
fn default_command() -> Option<Command> {
    let exe = std::env::current_exe().ok()?;
    exe.to_string_lossy()
        .contains(".app/Contents/MacOS/")
        .then_some(Command::App { repo: None })
}

fn show_help() -> ExitCode {
    // Help goes to the terminal. A failure to print it has nowhere to be reported.
    let _ = <Cli as clap::CommandFactory>::command().print_help();
    println!();
    ExitCode::from(2)
}

async fn dispatch(command: Command) -> anyhow::Result<ExitCode> {
    match command {
        Command::Run {
            backlog,
            repo,
            cap,
            test_cmd,
            agents,
        } => {
            commands::run(commands::RunArgs {
                backlog,
                repo,
                cap,
                test_cmd,
                agents,
            })
            .await
        }
        Command::Scan { repo } => commands::scan(repo).await,
        Command::Doctor { repo } => commands::doctor(repo).await,
        Command::Report { repo, writeup } => commands::report(repo, writeup),
        Command::Mcp => commands::mcp().await,
        Command::App { repo } => app::app(repo).await,
        Command::Stop { repo } => serve::stop(repo).await,
        Command::Starter { dir } => commands::starter(dir),
        Command::Selftest { only } => selftest::run(only.as_deref()).await,
        other @ (Command::Daemon { .. }
        | Command::Open { .. }
        | Command::Pair { .. }
        | Command::Service { .. }
        | Command::Keys { .. }) => dispatch_serve(other).await,
    }
}

fn dispatch_service(action: ServiceCommand) -> anyhow::Result<ExitCode> {
    match action {
        ServiceCommand::Install {
            repo,
            port,
            no_start,
        } => service::install(repo, port, !no_start),
        ServiceCommand::Uninstall => service::uninstall(),
        ServiceCommand::Status => service::status(),
    }
}

/// The commands that run or reach the daemon.
async fn dispatch_serve(command: Command) -> anyhow::Result<ExitCode> {
    match command {
        Command::Daemon {
            repo,
            port,
            dev_bind,
            headless,
        } => {
            serve::daemon(serve::DaemonArgs {
                repo,
                port,
                dev_bind,
                headless,
            })
            .await
        }
        Command::Service { action } => dispatch_service(action),
        Command::Open { repo, print } => serve::open(repo, print),
        Command::Pair { repo } => serve::pair(repo).await,
        Command::Keys { action } => match action {
            KeysCommand::Set { name } => serve::keys(&serve::KeysAction::Set, &name),
            KeysCommand::Delete { name } => serve::keys(&serve::KeysAction::Delete, &name),
            KeysCommand::Status { name } => serve::keys(&serve::KeysAction::Status, &name),
        },
        Command::Run { .. }
        | Command::Scan { .. }
        | Command::Doctor { .. }
        | Command::Report { .. }
        | Command::Mcp
        | Command::App { .. }
        | Command::Stop { .. }
        | Command::Starter { .. }
        | Command::Selftest { .. } => {
            unreachable!("handled by dispatch")
        }
    }
}
