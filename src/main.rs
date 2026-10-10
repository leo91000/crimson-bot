mod backup;
mod capture;
mod control;
mod game;
mod memory;
mod model;
mod runtime;
mod script;
use anyhow::Result;
use clap::{Parser, Subcommand};
#[derive(Parser)]
#[command(about = "Local, read-only game observations and autonomous Rhai tasks")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Resolve signatures and report available observations. Sends no controls.
    Observe,
    /// Search structurally validated character-manager roots. Read-only.
    Discover,
    /// Make an independent, verified backup. Never restores or overwrites a save.
    Backup {
        #[arg(long)]
        save_root: Option<std::path::PathBuf>,
        #[arg(long)]
        state_dir: Option<std::path::PathBuf>,
    },
    /// Capture a diagnostic frame from gamescope. Sends no controls.
    Capture {
        #[arg(long)]
        state_dir: Option<std::path::PathBuf>,
    },
    /// Execute an autonomous task; dry runs produce decisions without controls.
    Run {
        script: std::path::PathBuf,
        #[arg(long, default_value_t = 30)]
        seconds: u64,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        frames: bool,
        #[arg(long)]
        save_root: Option<std::path::PathBuf>,
        #[arg(long)]
        state_dir: Option<std::path::PathBuf>,
    },
}
fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Observe => {
            let mut game = game::Game::attach()?;
            println!("{}", serde_json::to_string_pretty(&game.observe()?)?);
        }
        Command::Discover => {
            let game = game::Game::attach()?;
            println!("{}", serde_json::to_string_pretty(&game.discover()?)?);
        }
        Command::Backup {
            save_root,
            state_dir,
        } => {
            let source = save_root.unwrap_or(backup::default_save_root()?);
            let state = state_dir.unwrap_or(runtime::default_state()?);
            println!("{}", backup::snapshot(&source, &state)?.display());
        }
        Command::Capture { state_dir } => {
            let state = state_dir.unwrap_or(runtime::default_state()?);
            println!("{}", capture::capture(&state.join("frames"))?.display());
        }
        Command::Run {
            script,
            seconds,
            dry_run,
            frames,
            save_root,
            state_dir,
        } => {
            runtime::run(runtime::Options {
                script,
                seconds,
                dry_run,
                frames,
                save_root: save_root.unwrap_or(backup::default_save_root()?),
                state: state_dir.unwrap_or(runtime::default_state()?),
            })?;
        }
    }
    Ok(())
}
