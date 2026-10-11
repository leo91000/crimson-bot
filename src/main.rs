use anyhow::Result;
use clap::{Parser, Subcommand};
use crimson_bot::{backup, capture, game, runtime, vision};
#[derive(Parser)]
#[command(about = "Local, read-only game observations and autonomous Rhai tasks")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Launch the local game through Steam. Does not load a save or send inputs.
    Launch,
    /// Resolve signatures and report available observations. Sends no controls.
    Observe,
    /// Search structurally validated character-manager roots. Read-only.
    Discover,
    /// Read-only scene associations near the player. Hints cannot trigger actions.
    ProbeScene {
        #[arg(long, default_value_t = 20.0)]
        radius: f64,
        #[arg(long, default_value_t = 4)]
        actors: usize,
    },
    /// Search the live game's item definitions. Does not expose the whole table by default.
    Catalog { query: String },
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
    /// Read text and UI state from an existing frame using local OCR models.
    ReadScreen {
        image: std::path::PathBuf,
        #[arg(long)]
        models: std::path::PathBuf,
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
        #[arg(long)]
        models: Option<std::path::PathBuf>,
        #[arg(long)]
        params: Option<std::path::PathBuf>,
        #[arg(long)]
        allow_title: bool,
    },
}
fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Launch => {
            let status = std::process::Command::new("steam")
                .arg("steam://rungameid/3321460")
                .env("DISPLAY", ":0")
                .status()?;
            anyhow::ensure!(status.success(), "Steam launch request failed");
        }
        Command::Observe => {
            let mut game = game::Game::attach()?;
            println!("{}", serde_json::to_string_pretty(&game.observe()?)?);
        }
        Command::Discover => {
            let game = game::Game::attach()?;
            println!("{}", serde_json::to_string_pretty(&game.discover()?)?);
        }
        Command::ProbeScene { radius, actors } => {
            let game = game::Game::attach()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&game.probe_scene(radius, actors)?)?
            );
        }
        Command::Catalog { query } => {
            let game = game::Game::attach()?;
            let query = query.to_lowercase();
            let items: Vec<_> = game
                .catalog
                .values()
                .filter(|item| {
                    item.internal_name.to_lowercase().contains(&query)
                        || item.key.to_string() == query
                })
                .take(30)
                .collect();
            println!("{}", serde_json::to_string_pretty(&items)?);
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
        Command::ReadScreen { image, models } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&vision::Vision::load(&models)?.read(&image)?)?
            );
        }
        Command::Run {
            script,
            seconds,
            dry_run,
            frames,
            save_root,
            state_dir,
            models,
            params,
            allow_title,
        } => {
            runtime::run(runtime::Options {
                script,
                seconds,
                dry_run,
                frames,
                models,
                params,
                allow_title,
                save_root: save_root.unwrap_or(backup::default_save_root()?),
                state: state_dir.unwrap_or(runtime::default_state()?),
            })?;
        }
    }
    Ok(())
}
