mod game;
mod memory;
mod model;
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
}
fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Observe => {
            let mut game = game::Game::attach()?;
            println!("{}", serde_json::to_string_pretty(&game.observe()?)?);
        }
    }
    Ok(())
}
