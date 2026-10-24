//! The `sentinel` command: start the daemon, run a backlog from the terminal, and look at the
//! environment.

mod commands;
mod doctor_host;
mod serve;
mod service;
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
    #[command(subcommand)]
    command: Command,
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
        /// Use the scripted pretend agents from `fixtures/fake-cli` instead of real agent CLIs.
        #[arg(long)]
        fake_agents: bool,
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
        /// Use the scripted pretend agents from `fixtures/fake-cli`.
        #[arg(long)]
        fake_agents: bool,
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

#[tokio::main]
async fn main() -> ExitCode {
    match dispatch(Cli::parse().command).await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(2)
        }
    }
}

async fn dispatch(command: Command) -> anyhow::Result<ExitCode> {
    match command {
        Command::Run {
            backlog,
            repo,
            cap,
            fake_agents,
        } => commands::run(&backlog, repo, cap, fake_agents).await,
        Command::Scan { repo } => commands::scan(repo).await,
        Command::Doctor { repo } => commands::doctor(repo).await,
        Command::Report { repo, writeup } => commands::report(repo, writeup),
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
            fake_agents,
            headless,
        } => {
            serve::daemon(serve::DaemonArgs {
                repo,
                port,
                dev_bind,
                fake_agents,
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
        | Command::Report { .. } => {
            unreachable!("handled by dispatch")
        }
    }
}
