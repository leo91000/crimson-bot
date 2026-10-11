//! Private task state. A checkpoint never contains an input to replay or a game save.
use crate::{control::MovementLayout, model::World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pid: u32,
    process_start: u64,
    boot: String,
    executable: String,
    body: String,
    script: String,
    goal: Value,
    layout: MovementLayout,
}
impl Binding {
    pub fn new(world: &World, source: &[u8], goal: Value, layout: MovementLayout) -> Result<Self> {
        let stat = fs::read_to_string(format!("/proc/{}/stat", world.pid))?;
        Ok(Self {
            pid: world.pid,
            process_start: process_start(&stat)?,
            boot: fs::read_to_string("/proc/sys/kernel/random/boot_id")?
                .trim()
                .to_owned(),
            executable: diagnostic(world, "executable_sha256")?,
            body: diagnostic(world, "controlled_token")?,
            script: format!("{:x}", Sha256::digest(source)),
            goal,
            layout,
        })
    }
}
fn diagnostic(world: &World, key: &str) -> Result<String> {
    world.diagnostics[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .with_context(|| format!("Checkpoint requires {key}"))
}
fn process_start(stat: &str) -> Result<u64> {
    // comm can contain spaces and parentheses; field 22 is relative to its final ')'.
    stat.rsplit_once(')')
        .context("Invalid proc stat")?
        .1
        .split_whitespace()
        .nth(19)
        .context("Missing process start time")?
        .parse()
        .context("Invalid process start time")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    version: u32,
    binding: Binding,
    pub memory: Value,
    pub elapsed_ms: u64,
    pub outcome: String,
    position: crate::model::Position,
    health: f64,
    inventory: Vec<crate::model::Item>,
}
impl Checkpoint {
    pub fn new(binding: Binding, world: &World, memory: Value, elapsed_ms: u64) -> Result<Self> {
        ensure!(
            memory.is_object() && serde_json::to_vec(&memory)?.len() <= 64 * 1024,
            "Checkpoint state must be a bounded object"
        );
        let position = world
            .position
            .clone()
            .context("Checkpoint requires player position")?;
        ensure!(position.valid(), "Invalid checkpoint position");
        let health = health(world)?;
        Ok(Self {
            version: 1,
            binding,
            memory,
            elapsed_ms,
            outcome: "running".into(),
            position,
            health,
            inventory: world
                .inventory
                .clone()
                .context("Checkpoint requires inventory")?,
        })
    }
    pub fn validate(&self, binding: &Binding, world: &World) -> Result<()> {
        ensure!(
            self.version == 1
                && ["running", "timeout", "stopped", "blocked"].contains(&self.outcome.as_str()),
            "Checkpoint is not resumable"
        );
        ensure!(
            self.binding.pid == binding.pid
                && self.binding.process_start == binding.process_start
                && self.binding.boot == binding.boot
                && self.binding.executable == binding.executable
                && self.binding.body == binding.body,
            "Checkpoint belongs to another game process, build or controlled body"
        );
        ensure!(
            self.binding.script == binding.script
                && self.binding.goal == binding.goal
                && self.binding.layout == binding.layout,
            "Checkpoint script, goal or movement profile changed"
        );
        ensure!(
            world.player_resolved
                && health(world)? == self.health
                && self.health.is_finite()
                && world.inventory.as_ref() == Some(&self.inventory),
            "Player health or inventory changed since checkpoint"
        );
        let position = world.position.as_ref().context("Position unavailable")?;
        ensure!(
            self.position.valid()
                && position.valid()
                && (position.x - self.position.x).hypot(position.z - self.position.z) <= 2.0
                && (position.y - self.position.y).abs() <= 1.0,
            "Player moved beyond checkpoint tolerance"
        );
        ensure!(
            self.memory.is_object() && serde_json::to_vec(&self.memory)?.len() <= 64 * 1024,
            "Invalid checkpoint memory"
        );
        ensure!(
            self.elapsed_ms <= 24 * 3600 * 1000,
            "Checkpoint active time exceeds 24 hours"
        );
        Ok(())
    }
    pub fn read(path: &Path) -> Result<Self> {
        let mut bytes = Vec::new();
        File::open(path)?
            .take(256 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 256 * 1024, "Checkpoint file too large");
        Ok(serde_json::from_slice(&bytes)?)
    }
    pub fn write(&self, path: &Path) -> Result<()> {
        let bytes = serde_json::to_vec(self)?;
        ensure!(bytes.len() < 256 * 1024, "Checkpoint file too large");
        let temp = path.with_extension("new");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        File::open(path.parent().context("Checkpoint parent missing")?)?.sync_all()?;
        Ok(())
    }
}
fn health(world: &World) -> Result<f64> {
    world
        .stats
        .iter()
        .find(|s| s.kind == 0 && s.current.is_finite() && s.current > 0.0)
        .map(|s| s.current)
        .context("Living player required for checkpoint")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> (Binding, World) {
        (
            Binding {
                pid: 1,
                process_start: 123,
                boot: "boot".into(),
                executable: "exe".into(),
                body: "body".into(),
                script: "script".into(),
                goal: json!({"amount": 2}),
                layout: MovementLayout::Zqsd,
            },
            World {
                pid: 1,
                observed_at_ms: 0,
                player_resolved: true,
                position: Some(crate::model::Position {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                }),
                stats: vec![crate::model::Stat {
                    kind: 0,
                    current: 100.0,
                    maximum: None,
                }],
                inventory: Some(vec![]),
                resource_candidates: None,
                ui: None,
                characters: 1,
                capabilities: vec![],
                diagnostics: json!({}),
            },
        )
    }
    #[test]
    fn rejects_changed_process_goal_state_and_completed_tasks() {
        let (binding, mut world) = fixture();
        let mut cp =
            Checkpoint::new(binding.clone(), &world, json!({"excluded":["plant"]}), 100).unwrap();
        cp.validate(&binding, &world).unwrap();
        let mut changed = binding.clone();
        changed.process_start += 1;
        assert!(cp.validate(&changed, &world).is_err());
        changed = binding.clone();
        changed.boot = "another-boot".into();
        assert!(cp.validate(&changed, &world).is_err());
        changed = binding.clone();
        changed.goal = json!({"amount":3});
        assert!(cp.validate(&changed, &world).is_err());
        changed = binding.clone();
        changed.script = "other".into();
        assert!(cp.validate(&changed, &world).is_err());
        changed = binding.clone();
        changed.body = "other".into();
        assert!(cp.validate(&changed, &world).is_err());
        changed = binding.clone();
        changed.layout = MovementLayout::Wasd;
        assert!(cp.validate(&changed, &world).is_err());
        world.stats[0].current -= 1.0;
        assert!(cp.validate(&binding, &world).is_err());
        world.stats[0].current += 1.0;
        world.inventory.as_mut().unwrap().push(crate::model::Item {
            storage: 1,
            type_id: 1198,
            quantity: 1,
        });
        assert!(cp.validate(&binding, &world).is_err());
        world.inventory.as_mut().unwrap().clear();
        world.position.as_mut().unwrap().x = 3.0;
        assert!(cp.validate(&binding, &world).is_err());
        world.position.as_mut().unwrap().x = 0.5;
        cp.outcome = "timeout".into();
        cp.validate(&binding, &world).unwrap();
        cp.outcome = "succeeded".into();
        assert!(cp.validate(&binding, &world).is_err());
    }
    #[test]
    fn proc_stat_comm_may_contain_spaces_and_parentheses() {
        let tail = (3..=22)
            .map(|n| {
                if n == 3 {
                    "S".to_owned()
                } else {
                    n.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            process_start(&format!("42 (game (name)) {tail}")).unwrap(),
            22
        );
        assert!(process_start("42 (short) S").is_err());
    }
    #[test]
    fn replacement_round_trips_state_and_keeps_private_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "crimson-checkpoint-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let path = root.join("checkpoint.json");
        let (binding, world) = fixture();
        let mut cp = Checkpoint::new(
            binding.clone(),
            &world,
            serde_json::json!({"retired":["plant"]}),
            50,
        )
        .unwrap();
        cp.write(&path).unwrap();
        cp.elapsed_ms = 100;
        cp.outcome = "timeout".into();
        cp.write(&path).unwrap();
        let loaded = Checkpoint::read(&path).unwrap();
        loaded.validate(&binding, &world).unwrap();
        assert_eq!(loaded.elapsed_ms, 100);
        assert_eq!(loaded.memory, cp.memory);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(!path.with_extension("new").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
