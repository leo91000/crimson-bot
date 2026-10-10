//! Bounded local steering learned from observed movement. No global pathfinding.
use crate::{
    control::Action,
    model::{Position, World},
    script::Decision,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Deserialize)]
pub struct Goal {
    pub x: f64,
    pub z: f64,
    #[serde(default = "radius")]
    pub radius: f64,
    #[serde(default = "max_distance")]
    pub max_distance: f64,
    #[serde(default = "height")]
    pub max_height_change: f64,
}
fn radius() -> f64 {
    0.25
}
fn max_distance() -> f64 {
    5.0
}
fn height() -> f64 {
    0.5
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct State {
    anchor: Position,
    previous: Position,
    phase: String,
    forward: Option<[f64; 2]>,
    right: Option<[f64; 2]>,
    last_key: String,
    stuck: u8,
}
fn blocked(reason: &str) -> Decision {
    Decision {
        status: "blocked".into(),
        reason: reason.into(),
        memory: json!({}),
        action: None,
    }
}
fn unit(x: f64, z: f64) -> Option<[f64; 2]> {
    let length = x.hypot(z);
    if length < 0.02 || !length.is_finite() {
        return None;
    }
    Some([x / length, z / length])
}
fn command(state: State, key: &str) -> Decision {
    Decision {
        status: "running".into(),
        reason: "Local motion with measured feedback".into(),
        memory: serde_json::to_value(state).unwrap(),
        action: Some(Action {
            keys: vec![key.into()],
            duration_ms: 150,
            ui_mode: Some("world".into()),
            ..Default::default()
        }),
    }
}
pub fn step(world: &World, memory: &Value, goal: &Goal) -> Decision {
    if !world.player_resolved || !world.ui.as_ref().is_some_and(|ui| ui.mode == "world") {
        return blocked("Known gameplay and controlled body required for navigation");
    }
    let Some(position) = &world.position else {
        return blocked("Position unavailable");
    };
    if !position.valid()
        || !goal.x.is_finite()
        || !goal.z.is_finite()
        || !(0.05..=1.0).contains(&goal.radius)
        || !(0.1..=30.0).contains(&goal.max_distance)
        || !(0.05..=2.0).contains(&goal.max_height_change)
    {
        return blocked("Invalid local navigation bounds");
    }
    let empty = memory.as_object().is_some_and(|v| v.is_empty());
    let mut state = if empty {
        State {
            anchor: position.clone(),
            previous: position.clone(),
            phase: "back_probe".into(),
            forward: None,
            right: None,
            last_key: String::new(),
            stuck: 0,
        }
    } else {
        match serde_json::from_value::<State>(memory.clone()) {
            Ok(v) => v,
            Err(_) => return blocked("Invalid navigation state"),
        }
    };
    if (goal.x - state.anchor.x).hypot(goal.z - state.anchor.z) > goal.max_distance
        || (position.x - state.anchor.x).hypot(position.z - state.anchor.z)
            > goal.max_distance * 1.5
    {
        return blocked("Goal or displacement exceeds the local travel bound");
    }
    if (position.y - state.anchor.y).abs() > goal.max_height_change {
        return blocked("Terrain height changed beyond the trial bound");
    }
    let error = [goal.x - position.x, goal.z - position.z];
    if error[0].hypot(error[1]) <= goal.radius {
        return Decision {
            status: "succeeded".into(),
            reason: "Waypoint reached in measured coordinates".into(),
            memory: serde_json::to_value(state).unwrap(),
            action: None,
        };
    }
    if state.last_key.is_empty() {
        state.last_key = "backward".into();
        return command(state, "backward");
    }
    let delta = unit(position.x - state.previous.x, position.z - state.previous.z);
    state.previous = position.clone();
    if state.phase == "back_probe" {
        let Some(delta) = delta else {
            return blocked("Calibration step did not move; refusing blind navigation");
        };
        state.forward = Some([-delta[0], -delta[1]]);
        let forward = state.forward.unwrap();
        // A waypoint already on the measured axis needs no lateral probing.
        if (error[0] * forward[1] - error[1] * forward[0]).abs() <= goal.radius {
            state.phase = "travel".into();
        } else {
            state.phase = "right_probe".into();
            state.last_key = "right".into();
            return command(state, "right");
        }
    } else if state.phase == "right_probe" {
        let Some(delta) = delta else {
            return blocked("Lateral calibration did not move");
        };
        let forward = state.forward.unwrap();
        if (forward[0] * delta[0] + forward[1] * delta[1]).abs() > 0.75 {
            return blocked("Movement axes are not sufficiently independent");
        }
        state.right = Some(delta);
        state.phase = "travel".into();
    } else if state.phase == "travel" {
        if let Some(delta) = delta {
            state.stuck = 0;
            match state.last_key.as_str() {
                "forward" => state.forward = Some(delta),
                "backward" => state.forward = Some([-delta[0], -delta[1]]),
                "right" => state.right = Some(delta),
                "left" => state.right = Some([-delta[0], -delta[1]]),
                _ => return blocked("Unknown previous motion"),
            }
        } else {
            state.stuck += 1;
            if state.stuck >= 3 {
                return blocked("Repeated motion without coordinate progress");
            }
        }
    } else {
        return blocked("Unknown navigation phase");
    }
    let Some(forward) = state.forward else {
        return blocked("Forward axis unavailable");
    };
    let mut choices = vec![
        ("forward", error[0] * forward[0] + error[1] * forward[1]),
        ("backward", -(error[0] * forward[0] + error[1] * forward[1])),
    ];
    if let Some(right) = state.right {
        let dot = error[0] * right[0] + error[1] * right[1];
        choices.push(("right", dot));
        choices.push(("left", -dot));
    }
    let key = choices
        .into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap()
        .0;
    state.last_key = key.into();
    command(state, key)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn world() -> World {
        World {
            pid: 1,
            observed_at_ms: 0,
            player_resolved: true,
            position: Some(Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            }),
            stats: vec![],
            inventory: None,
            ui: Some(crate::vision::Ui {
                mode: "world".into(),
                page: "unknown".into(),
                lines: vec![],
                words: vec![],
                width: 100,
                height: 100,
                processing_ms: 0,
            }),
            characters: 1,
            capabilities: vec![],
            diagnostics: json!({}),
        }
    }
    #[test]
    fn reaches_waypoint_only_after_observed_progress() {
        let mut world = world();
        let goal = Goal {
            x: 0.0,
            z: -1.0,
            radius: 0.2,
            max_distance: 2.0,
            max_height_change: 0.5,
        };
        let first = step(&world, &json!({}), &goal);
        assert_eq!(first.action.as_ref().unwrap().keys, ["backward"]);
        world.position.as_mut().unwrap().z = -0.3;
        let second = step(&world, &first.memory, &goal);
        assert_eq!(second.status, "running");
        world.position.as_mut().unwrap().z = -0.9;
        let third = step(&world, &second.memory, &goal);
        assert_eq!(third.status, "succeeded");
        assert!(third.action.is_none());
    }
    #[test]
    fn blocks_unbounded_goals_failed_calibration_and_height_changes() {
        let mut world = world();
        let mut goal = Goal {
            x: 0.0,
            z: -1.0,
            radius: 0.2,
            max_distance: 2.0,
            max_height_change: 0.5,
        };
        let first = step(&world, &json!({}), &goal);
        assert_eq!(step(&world, &first.memory, &goal).status, "blocked");
        goal.z = -30.0;
        assert_eq!(step(&world, &json!({}), &goal).status, "blocked");
        goal.z = -1.0;
        world.position.as_mut().unwrap().y = -3.0;
        assert_eq!(step(&world, &first.memory, &goal).status, "blocked");
    }
}
