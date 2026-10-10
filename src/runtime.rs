use crate::{
    backup, capture,
    control::{Controls, has_game_focus},
    game::Game,
    script::Script,
};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde_json::json;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub struct Options {
    pub script: PathBuf,
    pub state: PathBuf,
    pub save_root: PathBuf,
    pub seconds: u64,
    pub dry_run: bool,
    pub frames: bool,
    pub models: Option<PathBuf>,
    pub params: Option<PathBuf>,
}
struct Events {
    file: File,
}
impl Events {
    fn write(&mut self, event: serde_json::Value) -> Result<()> {
        serde_json::to_writer(&mut self.file, &event)?;
        self.file.write_all(b"\n")?;
        self.file.flush()?;
        Ok(())
    }
}
pub fn run(options: Options) -> Result<()> {
    ensure!(
        (1..=3600).contains(&options.seconds),
        "Run duration outside 1..3600 seconds"
    );
    let mut script = Script::load(&options.script)?;
    if let Some(path) = &options.params {
        script.configure(serde_json::from_slice(&fs::read(path)?)?)?;
    }
    let vision = options
        .models
        .as_ref()
        .map(|path| crate::vision::Vision::load(path))
        .transpose()?;
    fs::create_dir_all(&options.state)?;
    fs::set_permissions(&options.state, fs::Permissions::from_mode(0o700))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(options.state.join("control.lock"))?;
    lock.try_lock_exclusive()
        .context("Another runtime owns game control")?;
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_nanos()
        .to_string();
    let session = options.state.join("runs").join(id);
    fs::create_dir_all(&session)?;
    let mut events = Events {
        file: OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(session.join("events.jsonl"))?,
    };
    let backup = if options.dry_run {
        None
    } else {
        Some(backup::snapshot(&options.save_root, &options.state)?)
    };
    let mut game = Game::attach()?;
    let initial = game.observe()?;
    events.write(json!({"event":"start","dry_run":options.dry_run,"script":options.script,"backup":backup,"world":initial}))?;
    let stop = Arc::new(AtomicBool::new(false));
    let handler = stop.clone();
    ctrlc::set_handler(move || {
        handler.store(true, Ordering::SeqCst);
    })?;
    let mut controls = if options.dry_run {
        None
    } else {
        Some(Controls::create()?)
    };
    // Allow gamescope to enumerate the newly created device.
    if controls.is_some() {
        thread::sleep(Duration::from_millis(1200));
    }
    let start = Instant::now();
    let deadline = start + Duration::from_secs(options.seconds);
    let result = (|| -> Result<String> {
        let mut tick = 0u64;
        while Instant::now() < deadline
            && !stop.load(Ordering::SeqCst)
            && !options.state.join("STOP").exists()
        {
            let ui = if let Some(vision) = &vision {
                let frame = capture::capture(&session)?;
                events.write(json!({"event":"frame","path":frame}))?;
                Some(vision.read(&frame)?)
            } else {
                None
            };
            let mut world = game.observe()?;
            world.ui = ui;
            let observed = Instant::now();
            let decision = script.tick(&world, start.elapsed().as_millis() as u64)?;
            events.write(json!({"event":"tick","tick":tick,"world":world,"decision":decision}))?;
            if decision.status != "running" {
                if options.frames {
                    let frame = capture::capture(&session)?;
                    events.write(json!({"event":"frame","path":frame}))?;
                }
                return Ok(decision.status);
            }
            if let Some(action) = decision.action {
                ensure!(
                    observed.elapsed() < Duration::from_secs(1),
                    "Observation stale before action"
                );
                if let Some(control) = controls.as_mut() {
                    if let Some(expected) = &action.ui_mode {
                        let ui = world
                            .ui
                            .as_ref()
                            .context("UI action requires current perception")?;
                        ensure!(&ui.mode == expected, "UI mode changed before action");
                        if let Some([x, y]) = action.cursor {
                            ensure!(
                                x < ui.width && y < ui.height,
                                "Cursor outside current frame"
                            );
                        }
                    }
                    ensure!(
                        world.player_resolved,
                        "Cannot act without a validated controlled body"
                    );
                    ensure!(
                        world
                            .stats
                            .iter()
                            .any(|stat| stat.kind == 0 && stat.current > 0.0),
                        "Cannot act without living player health"
                    );
                    ensure!(
                        has_game_focus(world.pid)?,
                        "Game lost focus; controls stopped"
                    );
                    control.apply(&action)?;
                }
                let action_end = Instant::now() + Duration::from_millis(action.duration_ms);
                while Instant::now() < action_end {
                    if stop.load(Ordering::SeqCst)
                        || options.state.join("STOP").exists()
                        || Instant::now() >= deadline
                    {
                        break;
                    }
                    if controls.is_some() {
                        ensure!(has_game_focus(world.pid)?, "Focus lost during action");
                    }
                    thread::sleep(Duration::from_millis(20));
                }
            } else {
                thread::sleep(Duration::from_millis(100));
            }
            if let Some(control) = controls.as_mut() {
                control.release()?;
            }
            // Capture after releasing inputs; next tick reacquires the world.
            if options.frames && tick.is_multiple_of(10) {
                let frame = capture::capture(&session)?;
                events.write(json!({"event":"frame","path":frame}))?;
            }
            tick += 1;
        }
        Ok(
            if stop.load(Ordering::SeqCst) || options.state.join("STOP").exists() {
                "stopped".into()
            } else {
                "timeout".into()
            },
        )
    })();
    if let Some(control) = controls.as_mut() {
        control.release()?;
    }
    events.write(json!({"event":"end","outcome":match &result {Ok(v)=>v.as_str(),Err(_)=>"error"},"error":result.as_ref().err().map(|v|format!("{v:#}")),"elapsed_ms":start.elapsed().as_millis()}))?;
    println!(
        "{}",
        json!({"session":session,"outcome":result.as_ref().ok()})
    );
    let status = result?;
    ensure!(
        status == "succeeded" || status == "stopped",
        "Task ended without success: {status}"
    );
    Ok(())
}
pub fn default_state() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME missing")?;
    Ok(Path::new(&home).join(".local/state/crimson-bot"))
}
