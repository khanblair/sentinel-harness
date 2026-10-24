//! The `sentinel` command: start the daemon, run a backlog from the terminal, and look at the
//! environment.

mod commands;

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
    /// Check Rust, tools, git, and Tailscale.
    Doctor,
    /// Print the report of the last run.
    Report {
        #[arg(long)]
        repo: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run {
            backlog,
            repo,
            cap,
            fake_agents,
        } => commands::run(&backlog, repo, cap, fake_agents).await,
        Command::Scan { repo } => commands::scan(repo).await,
        Command::Doctor => commands::doctor().await,
        Command::Report { repo } => commands::report(repo),
    };
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(2)
        }
    }
}
