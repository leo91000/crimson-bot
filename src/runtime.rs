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
    pub params: Option<PathBuf>,
    pub allow_title: bool,
    pub movement_layout: crate::control::MovementLayout,
    pub resume: Option<PathBuf>,
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
    ensure!(
        !options.dry_run || options.resume.is_none(),
        "Dry runs cannot resume live checkpoints"
    );
    let source = fs::read(&options.script)?;
    let mut script = Script::compile(std::str::from_utf8(&source)?)?;
    let goal = if let Some(path) = &options.params {
        serde_json::from_slice(&fs::read(path)?)?
    } else {
        json!({})
    };
    script.configure(goal.clone())?;
    fs::create_dir_all(&options.state)?;
    fs::set_permissions(&options.state, fs::Permissions::from_mode(0o700))?;
    let control_root = default_state()?;
    fs::create_dir_all(&control_root)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(control_root.join("control.lock"))?;
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
    script.set_catalog(&game.catalog);
    let initial = game.observe()?;
    let binding = if script.resumable() && !options.dry_run {
        Some(crate::checkpoint::Binding::new(
            &initial,
            &source,
            goal,
            options.movement_layout,
        )?)
    } else {
        None
    };
    let mut checkpoint = None;
    let mut elapsed_offset = 0;
    if let Some(path) = &options.resume {
        let binding = binding
            .as_ref()
            .context("Task does not support checkpoints")?;
        let loaded = crate::checkpoint::Checkpoint::read(path)?;
        loaded.validate(binding, &initial)?;
        script.restore(loaded.memory.clone())?;
        elapsed_offset = loaded.elapsed_ms;
        checkpoint = Some(loaded);
    }
    let checkpoint_path = session.join("checkpoint.json");
    events.write(json!({"event":"start","dry_run":options.dry_run,"script":options.script,"resume":options.resume,"movement_layout":options.movement_layout,"backup":backup,"world":initial}))?;
    let stop = Arc::new(AtomicBool::new(false));
    let handler = stop.clone();
    ctrlc::set_handler(move || {
        handler.store(true, Ordering::SeqCst);
    })?;
    let mut controls = if options.dry_run {
        None
    } else {
        Some(Controls::create_with_layout(options.movement_layout)?)
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
            let world = game.observe()?;
            // Perception may take seconds; recheck cancellation before any decision or input.
            if Instant::now() >= deadline
                || stop.load(Ordering::SeqCst)
                || options.state.join("STOP").exists()
            {
                break;
            }
            let observed = Instant::now();
            let elapsed_ms = elapsed_offset + start.elapsed().as_millis() as u64;
            let decision = script.tick(&world, elapsed_ms)?;
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
                    authorize_native_action(&world, &action, options.allow_title)?;
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
                    thread::sleep(
                        action_end
                            .saturating_duration_since(Instant::now())
                            .min(Duration::from_millis(20)),
                    );
                }
            } else {
                thread::sleep(Duration::from_millis(100));
            }
            if let Some(control) = controls.as_mut() {
                control.release()?;
            }
            if let Some(binding) = &binding {
                let checkpoint_world = game.observe()?;
                let current = crate::checkpoint::Checkpoint::new(
                    binding.clone(),
                    &checkpoint_world,
                    decision.memory,
                    elapsed_ms,
                )?;
                current.write(&checkpoint_path)?;
                checkpoint = Some(current);
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
    if let Some(mut checkpoint) = checkpoint {
        checkpoint.outcome = result
            .as_ref()
            .map(|v| v.as_str())
            .unwrap_or("error")
            .into();
        checkpoint.write(&checkpoint_path)?;
    }
    events.write(json!({"event":"end","outcome":match &result {Ok(v)=>v.as_str(),Err(_)=>"error"},"error":result.as_ref().err().map(|v|format!("{v:#}")),"elapsed_ms":start.elapsed().as_millis()}))?;
    println!(
        "{}",
        json!({"session":session,"outcome":result.as_ref().ok(),"checkpoint":checkpoint_path.exists().then_some(checkpoint_path)})
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

fn authorize_native_action(
    world: &crate::model::World,
    action: &crate::control::Action,
    allow_title: bool,
) -> Result<()> {
    action.validate()?;
    let expected = action.ui_mode.as_deref().unwrap_or("world");
    let ui = world
        .ui
        .as_ref()
        .context("Controls require validated native interface state")?;
    ensure!(
        ui.mode == expected,
        "Native interface state does not authorize this action"
    );
    if let Some([x, y]) = action.cursor {
        ensure!(
            x < ui.width && y < ui.height,
            "Cursor outside native interface bounds"
        );
    }
    let title = allow_title
        && action.cursor.is_some()
        && expected == "title"
        && action.keys.iter().all(|k| k == "click");
    ensure!(
        title || world.player_resolved,
        "Cannot act without a validated controlled body"
    );
    ensure!(
        title
            || world
                .stats
                .iter()
                .any(|s| s.kind == 0 && s.current.is_finite() && s.current > 0.0),
        "Cannot act without living player health"
    );
    if let Some(target) = &action.interaction_target {
        let interactions = world
            .interactions
            .as_ref()
            .context("Native interaction state unavailable")?;
        ensure!(
            interactions
                .iter()
                .filter(|i| i.available && &i.target_id == target)
                .count()
                == 1,
            "Native interaction target unavailable or ambiguous"
        );
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        control::Action,
        model::{Interaction, InteractionKind, Stat, Ui, World},
    };
    #[test]
    fn all_inputs_require_native_interface_and_interactions_require_current_target() {
        let mut w = World {
            pid: 1,
            observed_at_ms: 0,
            player_resolved: true,
            position: None,
            stats: vec![Stat {
                kind: 0,
                current: 100.0,
                maximum: None,
            }],
            inventory: Some(vec![]),
            resource_candidates: None,
            ui: None,
            interactions: None,
            characters: 1,
            capabilities: vec![],
            diagnostics: json!({}),
        };
        let motion = Action {
            keys: vec!["forward".into()],
            duration_ms: 100,
            ..Default::default()
        };
        assert!(authorize_native_action(&w, &motion, false).is_err());
        w.ui = Some(Ui {
            mode: "journal".into(),
            width: 1920,
            height: 1080,
        });
        assert!(authorize_native_action(&w, &motion, false).is_err());
        let close = Action {
            keys: vec!["escape".into()],
            ui_mode: Some("journal".into()),
            duration_ms: 60,
            ..Default::default()
        };
        authorize_native_action(&w, &close, false).unwrap();
        w.ui.as_mut().unwrap().mode = "world".into();
        authorize_native_action(&w, &motion, false).unwrap();
        let pickup = Action {
            keys: vec!["interact".into()],
            interaction_target: Some("plant".into()),
            duration_ms: 100,
            ..Default::default()
        };
        assert!(authorize_native_action(&w, &pickup, false).is_err());
        w.interactions = Some(vec![Interaction {
            target_id: "other".into(),
            kind: InteractionKind::Pickup,
            item_key: Some(757006),
            available: true,
        }]);
        assert!(authorize_native_action(&w, &pickup, false).is_err());
        w.interactions.as_mut().unwrap()[0].target_id = "plant".into();
        authorize_native_action(&w, &pickup, false).unwrap();
        w.interactions.as_mut().unwrap()[0].available = false;
        assert!(authorize_native_action(&w, &pickup, false).is_err());
    }
}
