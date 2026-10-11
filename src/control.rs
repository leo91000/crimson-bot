use anyhow::{Result, bail, ensure};
use evdev::{
    AttributeSet, EventType, InputEvent, Key, RelativeAxisType,
    uinput::{VirtualDevice, VirtualDeviceBuilder},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, process::Command};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    #[serde(default)]
    pub cursor: Option<[u32; 2]>,
    #[serde(default)]
    pub ui_mode: Option<String>,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub mouse_x: i32,
    #[serde(default)]
    pub mouse_y: i32,
    #[serde(default = "default_duration")]
    pub duration_ms: u64,
}
fn default_duration() -> u64 {
    100
}

pub fn key(name: &str) -> Result<Key> {
    Ok(match name {
        "forward" => Key::KEY_W,
        "backward" => Key::KEY_S,
        "left" => Key::KEY_A,
        "right" => Key::KEY_D,
        "interact" => Key::KEY_E,
        "jump" => Key::KEY_SPACE,
        "sprint" => Key::KEY_LEFTSHIFT,
        "escape" => Key::KEY_ESC,
        "journal" => Key::KEY_J,
        "map" => Key::KEY_M,
        "attack" | "click" => Key::BTN_LEFT,
        "guard" => Key::BTN_RIGHT,
        "lock_on" => Key::BTN_MIDDLE,
        "confirm" => Key::KEY_ENTER,
        "expand" => Key::KEY_SPACE,
        "up" => Key::KEY_UP,
        "down" => Key::KEY_DOWN,
        _ => bail!("Unknown action binding: {name}"),
    })
}

impl Action {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=500).contains(&self.duration_ms),
            "Action duration outside 1..500 ms"
        );
        ensure!(self.keys.len() <= 4, "Too many simultaneous keys");
        ensure!(
            self.mouse_x.unsigned_abs() <= 500 && self.mouse_y.unsigned_abs() <= 200,
            "Camera step too large"
        );
        for binding in &self.keys {
            key(binding)?;
        }
        if let Some([x, y]) = self.cursor {
            ensure!(x < 7680 && y < 4320, "Cursor outside supported dimensions");
            ensure!(
                matches!(self.ui_mode.as_deref(), Some("journal" | "title")),
                "Cursor actions require a recognized supported menu"
            );
            ensure!(
                self.keys.iter().all(|v| v == "click"),
                "Cursor actions cannot contain gameplay keys"
            );
        }
        Ok(())
    }
}

pub fn has_game_focus(pid: u32) -> Result<bool> {
    let output = Command::new("xdotool")
        .args(["getwindowfocus", "getwindowpid"])
        .env("DISPLAY", ":1")
        .output()?;
    Ok(output.status.success()
        && String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u32>()
            .ok()
            == Some(pid))
}

pub struct Controls {
    device: VirtualDevice,
    held: BTreeSet<u16>,
    held_x_mouse: bool,
}
impl Controls {
    pub fn create() -> Result<Self> {
        let mut keys = AttributeSet::<Key>::new();
        for binding in [
            "forward", "backward", "left", "right", "interact", "jump", "sprint", "escape",
            "journal", "map", "attack", "guard", "lock_on", "confirm", "up", "down",
        ] {
            keys.insert(key(binding)?);
        }
        let mut axes = AttributeSet::<RelativeAxisType>::new();
        axes.insert(RelativeAxisType::REL_X);
        axes.insert(RelativeAxisType::REL_Y);
        let device = VirtualDeviceBuilder::new()?
            .name("Crimson Bot keyboard and mouse")
            .with_keys(&keys)?
            .with_relative_axes(&axes)?
            .build()?;
        Ok(Self {
            device,
            held: BTreeSet::new(),
            held_x_mouse: false,
        })
    }
    pub fn apply(&mut self, action: &Action) -> Result<()> {
        action.validate()?;
        self.release()?;
        if let Some([x, y]) = action.cursor {
            let status = Command::new("xdotool")
                .args(["mousemove", &x.to_string(), &y.to_string()])
                .env("DISPLAY", ":1")
                .status()?;
            ensure!(status.success(), "Cursor move failed");
            if action.keys.iter().any(|v| v == "click") {
                self.held_x_mouse = true;
                let status = Command::new("xdotool")
                    .args(["mousedown", "1"])
                    .env("DISPLAY", ":1")
                    .status()?;
                ensure!(status.success(), "UI mouse press failed");
            }
        }
        let mut events = vec![];
        for binding in &action.keys {
            if action.cursor.is_some() && binding == "click" {
                continue;
            }
            let code = key(binding)?.code();
            if self.held.insert(code) {
                events.push(InputEvent::new(EventType::KEY, code, 1));
            }
        }
        if action.mouse_x != 0 {
            events.push(InputEvent::new(
                EventType::RELATIVE,
                RelativeAxisType::REL_X.0,
                action.mouse_x,
            ));
        }
        if action.mouse_y != 0 {
            events.push(InputEvent::new(
                EventType::RELATIVE,
                RelativeAxisType::REL_Y.0,
                action.mouse_y,
            ));
        }
        self.device.emit(&events)?;
        Ok(())
    }
    pub fn release(&mut self) -> Result<()> {
        if self.held_x_mouse {
            let status = Command::new("xdotool")
                .args(["mouseup", "1"])
                .env("DISPLAY", ":1")
                .status()?;
            ensure!(status.success(), "UI mouse release failed");
            self.held_x_mouse = false;
        }
        let events: Vec<_> = self
            .held
            .iter()
            .map(|&code| InputEvent::new(EventType::KEY, code, 0))
            .collect();
        if !events.is_empty() {
            self.device.emit(&events)?;
        }
        self.held.clear();
        Ok(())
    }
}
impl Drop for Controls {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn action_validation_rejects_unbounded_or_unknown_commands() {
        assert!(
            Action {
                duration_ms: 501,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Action {
                duration_ms: 50,
                keys: vec!["cheat".into()],
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Action {
                duration_ms: 50,
                keys: vec!["forward".into()],
                ..Default::default()
            }
            .validate()
            .is_ok()
        );
    }
}
