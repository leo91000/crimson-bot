use anyhow::{Result, bail, ensure};
use evdev::{
    AttributeSet, EventType, InputEvent, Key, RelativeAxisType,
    uinput::{VirtualDevice, VirtualDeviceBuilder},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, process::Command};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum MovementLayout {
    #[default]
    Wasd,
    Zqsd,
}
impl MovementLayout {
    pub fn key(self, name: &str) -> Result<Key> {
        match (self, name) {
            (Self::Zqsd, "forward") => Ok(Key::KEY_Z),
            (Self::Zqsd, "left") => Ok(Key::KEY_Q),
            _ => key(name),
        }
    }
}

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
    #[serde(default)]
    pub wheel_y: i32,
    #[serde(default)]
    pub interaction_target: Option<String>,
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
        if self.keys.iter().any(|key| key == "interact") {
            ensure!(
                self.interaction_target
                    .as_ref()
                    .is_some_and(|id| !id.is_empty() && id.len() <= 128)
                    && self.keys == ["interact"]
                    && self.cursor.is_none()
                    && self.mouse_x == 0
                    && self.mouse_y == 0
                    && self.wheel_y == 0,
                "Interaction requires one explicit native target and no other inputs"
            );
        } else {
            ensure!(
                self.interaction_target.is_none(),
                "Target supplied without interaction input"
            );
        }
        if let Some(mode) = self.ui_mode.as_deref() {
            ensure!(
                ["world", "journal", "pause", "settings", "title"].contains(&mode),
                "Unsupported action interface state"
            );
        }
        if self.wheel_y != 0 {
            ensure!(
                self.wheel_y.unsigned_abs() <= 3
                    && self.cursor.is_some()
                    && matches!(self.ui_mode.as_deref(), Some("journal" | "settings"))
                    && self.keys.is_empty()
                    && self.mouse_x == 0
                    && self.mouse_y == 0,
                "Scroll requires a supported menu cursor, no other inputs and at most three notches"
            );
        }
        if let Some([x, y]) = self.cursor {
            ensure!(x < 7680 && y < 4320, "Cursor outside supported dimensions");
            ensure!(
                matches!(
                    self.ui_mode.as_deref(),
                    Some("journal" | "pause" | "settings" | "title")
                ),
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
    layout: MovementLayout,
}
impl Controls {
    pub fn create() -> Result<Self> {
        Self::create_with_layout(MovementLayout::Wasd)
    }
    pub fn create_with_layout(layout: MovementLayout) -> Result<Self> {
        let mut keys = AttributeSet::<Key>::new();
        for binding in [
            "forward", "backward", "left", "right", "interact", "jump", "sprint", "escape",
            "journal", "map", "attack", "guard", "lock_on", "confirm", "up", "down",
        ] {
            keys.insert(layout.key(binding)?);
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
            layout,
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
            if action.wheel_y != 0 {
                let status = Command::new("xdotool")
                    .args([
                        "click",
                        "--repeat",
                        &action.wheel_y.unsigned_abs().to_string(),
                        "--delay",
                        "40",
                        if action.wheel_y > 0 { "4" } else { "5" },
                    ])
                    .env("DISPLAY", ":1")
                    .status()?;
                ensure!(status.success(), "Menu scroll failed");
            }
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
            let code = self.layout.key(binding)?.code();
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
    fn movement_profiles_map_logical_actions_without_changing_other_bindings() {
        assert_eq!(MovementLayout::Wasd.key("forward").unwrap(), Key::KEY_W);
        assert_eq!(MovementLayout::Zqsd.key("forward").unwrap(), Key::KEY_Z);
        assert_eq!(MovementLayout::Zqsd.key("left").unwrap(), Key::KEY_Q);
        for name in ["backward", "right", "interact", "escape", "attack"] {
            assert_eq!(
                MovementLayout::Wasd.key(name).unwrap(),
                MovementLayout::Zqsd.key(name).unwrap()
            );
        }
    }
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
    #[test]
    fn menu_scroll_rejects_gameplay_and_mixed_inputs_without_overflow() {
        let valid = Action {
            cursor: Some([100, 200]),
            ui_mode: Some("journal".into()),
            wheel_y: -3,
            duration_ms: 100,
            ..Default::default()
        };
        valid.validate().unwrap();
        for value in [4, -4, i32::MIN] {
            assert!(
                Action {
                    wheel_y: value,
                    ..valid.clone()
                }
                .validate()
                .is_err()
            );
        }
        assert!(
            Action {
                ui_mode: Some("world".into()),
                ..valid.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Action {
                keys: vec!["click".into()],
                ..valid.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Action {
                cursor: None,
                ..valid.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Action {
                mouse_x: 1,
                ..valid
            }
            .validate()
            .is_err()
        );
    }
}
