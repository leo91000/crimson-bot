//! Bounded local steering learned from observed movement. No global pathfinding.
use crate::{
    control::Action,
    model::{Position, World},
    script::Decision,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

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
struct Obstacle {
    key: String,
    position: Position,
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
    #[serde(default)]
    obstacles: Vec<Obstacle>,
    #[serde(default)]
    recoveries: u8,
    #[serde(default)]
    rates: BTreeMap<String, f64>,
    #[serde(default)]
    last_duration_ms: Option<u64>,
}
fn blocked(reason: &str) -> Decision {
    Decision {
        status: "blocked".into(),
        reason: reason.into(),
        memory: json!({}),
        action: None,
    }
}
fn arrived(state: State) -> Decision {
    Decision {
        status: "succeeded".into(),
        reason: "Waypoint reached in measured coordinates".into(),
        memory: serde_json::to_value(state).unwrap(),
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
fn command(mut state: State, key: &str, duration_ms: u64) -> Decision {
    state.last_key = key.into();
    state.last_duration_ms = Some(duration_ms);
    Decision {
        status: "running".into(),
        reason: "Local motion with measured feedback".into(),
        memory: serde_json::to_value(state).unwrap(),
        action: Some(Action {
            keys: vec![key.into()],
            duration_ms,
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
            obstacles: vec![],
            recoveries: 0,
            rates: BTreeMap::new(),
            last_duration_ms: None,
        }
    } else {
        match serde_json::from_value::<State>(memory.clone()) {
            Ok(v) => v,
            Err(_) => return blocked("Invalid navigation state"),
        }
    };
    let axis_valid = |axis: [f64; 2]| {
        axis.iter().all(|v| v.is_finite()) && (0.95..=1.05).contains(&axis[0].hypot(axis[1]))
    };
    if !state.anchor.valid()
        || !state.previous.valid()
        || state.forward.is_some_and(|v| !axis_valid(v))
        || state.right.is_some_and(|v| !axis_valid(v))
        || (state.phase != "back_probe" && state.forward.is_none())
        || state.stuck >= 3
        || state.recoveries > 6
        || state.obstacles.len() > 6
        || state.rates.iter().any(|(key, rate)| {
            !["forward", "backward", "left", "right"].contains(&key.as_str())
                || !rate.is_finite()
                || !(0.00001..=0.1).contains(rate)
        })
        || state
            .last_duration_ms
            .is_some_and(|duration| !(10..=300).contains(&duration))
        || state.obstacles.iter().any(|o| {
            !o.position.valid()
                || !["forward", "backward", "left", "right"].contains(&o.key.as_str())
        })
        || !["back_probe", "right_probe", "travel"].contains(&state.phase.as_str())
    {
        return blocked("Invalid navigation state");
    }
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
    let reached = error[0].hypot(error[1]) <= goal.radius;
    if reached && state.last_duration_ms.is_none() {
        return arrived(state);
    }
    if state.last_key.is_empty() && state.phase == "back_probe" {
        return command(state, "backward", 150);
    }
    let dx = position.x - state.previous.x;
    let dz = position.z - state.previous.z;
    let delta = unit(dx, dz);
    if let Some(duration) = state.last_duration_ms
        && delta.is_some()
    {
        let observed_rate = dx.hypot(dz) / duration as f64;
        if !(0.00001..=0.1).contains(&observed_rate) {
            return blocked("Observed movement rate outside calibration bounds");
        }
        let rate = state
            .rates
            .entry(state.last_key.clone())
            .or_insert(observed_rate);
        *rate = 0.5 * *rate + 0.5 * observed_rate;
    }
    state.previous = position.clone();
    if state.phase == "back_probe" {
        let Some(delta) = delta else {
            return blocked("Calibration step did not move; refusing blind navigation");
        };
        state.forward = Some([-delta[0], -delta[1]]);
        let forward = state.forward.unwrap();
        // A waypoint already on the measured axis needs no lateral probing.
        if reached || (error[0] * forward[1] - error[1] * forward[0]).abs() <= goal.radius {
            state.phase = "travel".into();
        } else {
            state.phase = "right_probe".into();
            return command(state, "right", 150);
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
    } else if state.phase == "travel" && state.last_duration_ms.is_some() {
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
            if state.stuck >= 2 {
                if state.recoveries >= 6 {
                    return blocked("Local obstacle recovery budget exhausted");
                }
                if state.obstacles.len() >= 6 {
                    state.obstacles.remove(0);
                }
                state.obstacles.push(Obstacle {
                    key: state.last_key.clone(),
                    position: position.clone(),
                });
                state.recoveries += 1;
                state.stuck = 0;
            }
        }
    } else if state.phase != "travel" {
        return blocked("Unknown navigation phase");
    }
    state.last_duration_ms = None;
    if reached {
        return arrived(state);
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
        .filter(|(key, _)| {
            !state.obstacles.iter().any(|o| {
                o.key == *key && (position.x - o.position.x).hypot(position.z - o.position.z) < 0.9
            })
        })
        .max_by(|a, b| a.1.total_cmp(&b.1));
    let Some((key, projection)) = key else {
        return blocked("All calibrated directions blocked near this position");
    };
    let duration = if let Some(rate) = state.rates.get(key) {
        // Aim short of the projected point; measure each direction separately.
        (projection.max(goal.radius) * 0.85 / rate)
            .round()
            .clamp(10.0, 300.0) as u64
    } else {
        // Opposite directions can have different speeds. Probe an unmeasured key gently.
        75
    };
    command(state, key, duration)
}

/// Start a new bounded segment using the axes learned by a completed segment.
pub fn retarget(world: &World, memory: &Value, goal: &Goal) -> Decision {
    let Ok(mut state) = serde_json::from_value::<State>(memory.clone()) else {
        return blocked("Invalid navigation state");
    };
    if state.phase != "travel" || state.forward.is_none() {
        return blocked("Complete calibration required before retargeting");
    }
    let Some(position) = &world.position else {
        return blocked("Position unavailable");
    };
    state.anchor = position.clone();
    state.previous = position.clone();
    state.stuck = 0;
    state.recoveries = 0;
    state.last_duration_ms = None;
    state
        .obstacles
        .retain(|o| (position.x - o.position.x).hypot(position.z - o.position.z) < 4.0);
    step(world, &serde_json::to_value(state).unwrap(), goal)
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
            resource_candidates: None,
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
    #[test]
    fn rejects_incomplete_or_corrupt_calibration_without_inputs() {
        let world = world();
        let goal = Goal {
            x: 0.0,
            z: -1.0,
            radius: 0.2,
            max_distance: 2.0,
            max_height_change: 0.5,
        };
        let baseline = step(&world, &json!({}), &goal).memory;
        for (field, value) in [
            ("phase", json!("right_probe")),
            ("phase", json!("unknown")),
            ("stuck", json!(255)),
            ("forward", json!([0.0, 0.0])),
            ("anchor", json!({"x": 1e300, "y": 0.0, "z": 0.0})),
        ] {
            let mut memory = baseline.clone();
            memory[field] = value;
            let result = step(&world, &memory, &goal);
            assert_eq!(result.status, "blocked");
            assert!(result.action.is_none());
        }
    }
    #[test]
    fn retarget_preserves_axes_and_rejects_incomplete_calibration() {
        let mut world = world();
        let goal = Goal {
            x: 0.0,
            z: -1.0,
            radius: 0.2,
            max_distance: 2.0,
            max_height_change: 0.5,
        };
        let first = step(&world, &json!({}), &goal);
        assert_eq!(retarget(&world, &first.memory, &goal).status, "blocked");
        world.position.as_mut().unwrap().z = -0.3;
        let second = step(&world, &first.memory, &goal);
        world.position.as_mut().unwrap().z = -1.0;
        let arrived = step(&world, &second.memory, &goal);
        let next = Goal { z: -3.0, ..goal };
        assert_eq!(step(&world, &arrived.memory, &next).status, "blocked");
        let decision = retarget(&world, &arrived.memory, &next);
        assert_eq!(decision.status, "running");
        assert_eq!(decision.action.unwrap().keys, ["backward"]);
    }
    #[test]
    fn measured_lateral_blockage_tries_another_calibrated_axis() {
        let mut world = world();
        let goal = Goal {
            x: -2.0,
            z: -1.0,
            radius: 0.2,
            max_distance: 4.0,
            max_height_change: 0.5,
        };
        let first = step(&world, &json!({}), &goal);
        world.position.as_mut().unwrap().z = -0.3;
        let lateral = step(&world, &first.memory, &goal);
        assert_eq!(lateral.action.unwrap().keys, ["right"]);
        world.position.as_mut().unwrap().x = 0.3;
        let left = step(&world, &lateral.memory, &goal);
        assert_eq!(left.action.unwrap().keys, ["left"]);
        let still = step(&world, &left.memory, &goal);
        let recovery = step(&world, &still.memory, &goal);
        assert_eq!(recovery.status, "running");
        assert_eq!(recovery.action.unwrap().keys, ["backward"]);
        assert_eq!(recovery.memory["recoveries"], 1);
    }
    #[test]
    fn unequal_direction_speeds_converge_without_fixed_pulse_oscillation() {
        let mut short_commands = 0;
        // Include diagonal and near targets, several camera headings and asymmetric speeds.
        for heading in [0.0_f64, 0.7, 2.1] {
            for (x, z) in [(0.4, -0.3), (-2.1, 1.7), (4.0, -3.0), (-0.31, -0.44)] {
                let mut world = world();
                let goal = Goal {
                    x,
                    z,
                    radius: 0.15,
                    max_distance: 8.0,
                    max_height_change: 0.5,
                };
                let forward = [heading.sin(), heading.cos()];
                let right = [heading.cos(), -heading.sin()];
                let mut memory = json!({});
                let mut succeeded = false;
                for _ in 0..50 {
                    let decision = step(&world, &memory, &goal);
                    if decision.status == "succeeded" {
                        succeeded = true;
                        break;
                    }
                    assert_eq!(
                        decision.status, "running",
                        "heading {heading}, goal {x},{z}: {}",
                        decision.reason
                    );
                    let action = decision.action.unwrap();
                    let (axis, rate) = match action.keys[0].as_str() {
                        "forward" => (forward, 0.0044),
                        "backward" => ([-forward[0], -forward[1]], 0.0029),
                        "right" => (right, 0.0034),
                        "left" => ([-right[0], -right[1]], 0.0081),
                        _ => panic!("Unexpected motion"),
                    };
                    if action.duration_ms < 75 {
                        short_commands += 1;
                    }
                    let position = world.position.as_mut().unwrap();
                    position.x += axis[0] * rate * action.duration_ms as f64;
                    position.z += axis[1] * rate * action.duration_ms as f64;
                    memory = decision.memory;
                }
                assert!(
                    succeeded,
                    "Did not converge: heading {heading}, goal {x},{z}"
                );
            }
        }
        assert!(short_commands > 0);
    }
    #[test]
    fn arrival_consumes_last_motion_and_retarget_does_not_invent_a_stall() {
        let mut world = world();
        let goal = Goal {
            x: 0.0,
            z: -0.3,
            radius: 0.1,
            max_distance: 2.0,
            max_height_change: 0.5,
        };
        let first = step(&world, &json!({}), &goal);
        world.position.as_mut().unwrap().z = -0.3;
        let reached = step(&world, &first.memory, &goal);
        assert_eq!(reached.status, "succeeded");
        assert_eq!(reached.memory["phase"], "travel");
        assert_eq!(reached.memory["rates"]["backward"], 0.002);
        let next = retarget(&world, &reached.memory, &Goal { z: -1.0, ..goal });
        assert_eq!(next.status, "running");
        assert_eq!(next.memory["stuck"], 0);
        assert_eq!(next.memory["rates"]["backward"], 0.002);
    }
}
