use crate::{control::Action, model::World};
use anyhow::{Context, Result, ensure};
use rhai::{AST, Dynamic, Engine, Scope};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, path::Path};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub status: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub memory: Value,
    #[serde(default)]
    pub action: Option<Action>,
}
pub struct Script {
    engine: Engine,
    ast: AST,
    memory: Value,
    goal: Value,
}
impl Script {
    pub fn load(path: &Path) -> Result<Self> {
        Self::compile(&fs::read_to_string(path)?)
    }
    pub fn compile(source: &str) -> Result<Self> {
        let mut engine = Engine::new();
        engine
            .set_max_operations(50_000)
            .set_max_call_levels(24)
            .set_max_expr_depths(64, 64)
            .set_max_string_size(16_384)
            .set_max_array_size(16_384)
            .set_max_map_size(16_384);
        // Scripts receive data only. No filesystem, shell, network or input APIs.
        engine.set_module_resolver(rhai::module_resolvers::DummyModuleResolver::new());
        engine.on_print(|_| {});
        engine.register_fn("distance_xz", |ax: f64, az: f64, bx: f64, bz: f64| {
            (ax - bx).hypot(az - bz)
        });
        engine.register_fn("bearing_xz", |ax: f64, az: f64, bx: f64, bz: f64| {
            (bx - ax).atan2(bz - az)
        });
        engine.register_fn("inventory_count", inventory_count);
        engine.register_fn(
            "navigation_step",
            |world: Dynamic, memory: Dynamic, goal: Dynamic| {
                navigation_call(world, memory, goal, crate::navigation::step)
            },
        );
        engine.register_fn(
            "navigation_retarget",
            |world: Dynamic, memory: Dynamic, goal: Dynamic| {
                navigation_call(world, memory, goal, crate::navigation::retarget)
            },
        );
        let ast = engine.compile(source).context("Invalid Rhai task")?;
        Ok(Self {
            engine,
            ast,
            memory: json!({}),
            goal: json!({}),
        })
    }
    pub fn tick(&mut self, world: &World, elapsed_ms: u64) -> Result<Decision> {
        let context = rhai::serde::to_dynamic(
            json!({"world":world,"elapsed_ms":elapsed_ms,"memory":self.memory,"goal":self.goal}),
        )?;
        let value: Dynamic = self
            .engine
            .call_fn(&mut Scope::new(), &self.ast, "tick", (context,))
            .context("Task tick failed")?;
        let decision: Decision =
            rhai::serde::from_dynamic(&value).context("Invalid task decision")?;
        ensure!(
            ["running", "succeeded", "blocked", "failed"].contains(&decision.status.as_str()),
            "Invalid task status"
        );
        if let Some(action) = &decision.action {
            action.validate()?;
        }
        ensure!(
            decision.status == "running" || decision.action.is_none(),
            "Terminal decision contains controls"
        );
        ensure!(
            serde_json::to_vec(&decision.memory)?.len() <= 64 * 1024,
            "Task state exceeds bound"
        );
        self.memory = decision.memory.clone();
        Ok(decision)
    }
    pub fn configure(&mut self, goal: Value) -> Result<()> {
        ensure!(
            goal.is_object() && serde_json::to_vec(&goal)?.len() <= 64 * 1024,
            "Task parameters must be an object below 64 KiB"
        );
        self.goal = goal;
        Ok(())
    }
    pub fn restore(&mut self, memory: Value) -> Result<()> {
        ensure!(
            memory.is_object() && serde_json::to_vec(&memory)?.len() <= 64 * 1024,
            "Checkpoint task state must be an object below 64 KiB"
        );
        let value: Dynamic = self
            .engine
            .call_fn(
                &mut Scope::new(),
                &self.ast,
                "resume",
                (rhai::serde::to_dynamic(memory)?,),
            )
            .context("Task must define a safe resume(memory) hook")?;
        let memory: Value = rhai::serde::from_dynamic(&value)?;
        ensure!(
            memory.is_object() && serde_json::to_vec(&memory)?.len() <= 64 * 1024,
            "Invalid resumed task state"
        );
        self.memory = memory;
        Ok(())
    }
    pub fn resumable(&self) -> bool {
        self.ast
            .iter_functions()
            .any(|f| f.name == "resume" && f.params.len() == 1)
    }
    pub fn set_catalog(
        &mut self,
        catalog: &std::collections::BTreeMap<u16, crate::catalog::CatalogItem>,
    ) {
        let mut by_key: std::collections::BTreeMap<u32, Vec<u16>> = Default::default();
        for item in catalog.values() {
            by_key.entry(item.key).or_default().push(item.type_id);
        }
        self.engine
            .register_fn("item_type_id", move |key: i64| -> Dynamic {
                let Ok(key) = u32::try_from(key) else {
                    return Dynamic::UNIT;
                };
                let Some(ids) = by_key.get(&key) else {
                    return Dynamic::UNIT;
                };
                if ids.len() != 1 {
                    return Dynamic::UNIT;
                }
                Dynamic::from_int(ids[0] as i64)
            });
    }
}
fn navigation_call(
    world: Dynamic,
    memory: Dynamic,
    goal: Dynamic,
    step: fn(&World, &Value, &crate::navigation::Goal) -> Decision,
) -> Result<Dynamic, Box<rhai::EvalAltResult>> {
    let world = rhai::serde::from_dynamic::<World>(&world)?;
    let memory = rhai::serde::from_dynamic::<Value>(&memory)?;
    let goal = rhai::serde::from_dynamic::<Value>(&goal)?;
    let goal = serde_json::from_value::<crate::navigation::Goal>(goal)
        .map_err(|e| -> Box<rhai::EvalAltResult> { e.to_string().into() })?;
    rhai::serde::to_dynamic(step(&world, &memory, &goal))
}
fn inventory_count(
    items: Dynamic,
    storage: i64,
    type_id: i64,
) -> Result<i64, Box<rhai::EvalAltResult>> {
    let storage = u16::try_from(storage).map_err(|_| "Invalid storage id")?;
    let type_id = u16::try_from(type_id).map_err(|_| "Invalid item id")?;
    let items = rhai::serde::from_dynamic::<Option<Vec<crate::model::Item>>>(&items)?;
    crate::inventory::count(items.as_deref(), storage, type_id).map_err(|e| e.to_string().into())
}
#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> World {
        World {
            pid: 1,
            observed_at_ms: 0,
            player_resolved: true,
            position: None,
            stats: vec![],
            inventory: None,
            resource_candidates: None,
            ui: None,
            interactions: None,
            characters: 1,
            capabilities: vec![],
            diagnostics: json!({}),
        }
    }
    #[test]
    fn tick_carries_state_and_stops_on_success() {
        let mut script=Script::compile("fn tick(ctx) { let n=if ctx.memory.contains(\"n\") { ctx.memory.n+1 } else {1}; #{status:if n>2 {\"succeeded\"} else {\"running\"},memory:#{n:n}} }").unwrap();
        assert_eq!(script.tick(&world(), 0).unwrap().memory["n"], 1);
        assert_eq!(script.tick(&world(), 1).unwrap().memory["n"], 2);
        assert_eq!(script.tick(&world(), 2).unwrap().status, "succeeded");
    }

    #[test]
    fn navigation_accepts_integer_coordinates_from_json_parameters() {
        let mut world = world();
        world.position = Some(crate::model::Position {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        world.ui = Some(crate::model::Ui {
            mode: "world".into(),
            width: 1000,
            height: 1000,
        });
        let mut script =
            Script::compile("fn tick(ctx) {navigation_step(ctx.world,ctx.memory,ctx.goal)}")
                .unwrap();
        script
            .configure(json!({"x": 0, "z": -1, "max_distance": 2}))
            .unwrap();
        let first = script.tick(&world, 0).unwrap();
        assert_eq!(first.status, "running");
        assert_eq!(first.action.unwrap().keys, ["backward"]);
        world.position.as_mut().unwrap().z = -1.0;
        assert_eq!(script.tick(&world, 1).unwrap().status, "succeeded");
    }
    #[test]
    fn infinite_scripts_and_terminal_inputs_fail() {
        let mut looping = Script::compile("fn tick(ctx) {loop {}} ").unwrap();
        assert!(looping.tick(&world(), 0).is_err());
        let mut terminal = Script::compile(
            "fn tick(ctx) {#{status:\"succeeded\",action:#{keys:[\"forward\"],duration_ms:50}}}",
        )
        .unwrap();
        assert!(terminal.tick(&world(), 0).is_err());
    }

    #[test]
    fn gather_requires_native_interaction_and_measured_inventory_result() {
        let mut w = world();
        w.inventory = Some(vec![crate::model::Item {
            storage: 1,
            type_id: 1189,
            quantity: 4,
        }]);
        w.ui = Some(crate::model::Ui {
            mode: "world".into(),
            width: 1000,
            height: 1000,
        });
        let mut script = Script::compile(include_str!("../tasks/gather_nearby.rhai")).unwrap();
        script.set_catalog(&std::collections::BTreeMap::from([(
            1189,
            crate::catalog::CatalogItem {
                type_id: 1189,
                key: 751107,
                internal_name: "Lavender".into(),
            },
        )]));
        script
            .configure(json!({"item_key":751107,"storage":1,"amount":1}))
            .unwrap();
        assert!(script.tick(&w, 0).unwrap().action.is_none());
        let unknown = script.tick(&w, 1).unwrap();
        assert_eq!(unknown.status, "blocked");
        assert!(unknown.action.is_none());
        w.interactions = Some(vec![crate::model::Interaction {
            target_id: "lavender".into(),
            kind: crate::model::InteractionKind::Pickup,
            item_key: Some(751107),
            available: true,
        }]);
        assert_eq!(
            script.tick(&w, 2).unwrap().action.unwrap().keys,
            ["interact"]
        );
        assert_eq!(script.tick(&w, 100).unwrap().status, "running");
        w.inventory.as_mut().unwrap()[0].quantity = 5;
        assert_eq!(script.tick(&w, 6000).unwrap().status, "succeeded");
    }

    #[test]
    fn all_bundled_tasks_compile_with_the_sandboxed_engine() {
        for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tasks")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|v| v.to_str()) != Some("rhai") {
                continue;
            }
            assert!(
                Script::load(&path).is_ok(),
                "Failed to compile {}",
                path.display()
            );
        }
    }
    #[test]
    fn resource_route_checks_navigation_health_prompt_and_inventory() {
        let mut w = world();
        w.interactions = Some(vec![]);
        w.position = Some(crate::model::Position {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        w.stats = vec![crate::model::Stat {
            kind: 0,
            current: 100.0,
            maximum: None,
        }];
        w.inventory = Some(vec![crate::model::Item {
            storage: 1,
            type_id: 1198,
            quantity: 4,
        }]);
        w.resource_candidates = Some(vec![crate::resources::Candidate {
            id: "plant".into(),
            prefab:
                "/object/cd_gimmick/00_common/collect/plant/gimmick_collect_bush_peony_01.prefab"
                    .into(),
            position: crate::model::Position {
                x: 0.0,
                y: 0.0,
                z: -2.0,
            },
        }]);
        w.ui = Some(crate::model::Ui {
            mode: "world".into(),
            width: 1000,
            height: 1000,
        });
        let start = || {
            let mut script = Script::compile(include_str!("../tasks/route_gather.rhai")).unwrap();
            script.set_catalog(&std::collections::BTreeMap::from([(
                1198,
                crate::catalog::CatalogItem {
                    type_id: 1198,
                    key: 757006,
                    internal_name: "Peony".into(),
                },
            )]));
            script.configure(json!({"prefab_suffix":"gimmick_collect_bush_peony_01.prefab","item_name":"Pivoine","item_key":757006,"storage":1,"amount":1})).unwrap();
            script
        };
        let mut script = start();
        assert!(script.tick(&w, 0).unwrap().action.is_none());
        w.ui.as_mut().unwrap().mode = "unknown".into();
        let missed = script.tick(&w, 1).unwrap();
        assert_eq!(missed.status, "running");
        assert!(missed.action.is_none());
        w.ui.as_mut().unwrap().mode = "world".into();
        assert_eq!(
            script.tick(&w, 1).unwrap().action.unwrap().keys,
            ["backward"]
        );
        w.position.as_mut().unwrap().z = -0.3;
        assert_eq!(script.tick(&w, 2).unwrap().status, "running");
        w.position.as_mut().unwrap().z = -1.2;
        let near = script.tick(&w, 3).unwrap();
        assert_eq!(near.status, "running");
        assert_eq!(near.action.unwrap().mouse_x, 300);
        for tick in 4..15 {
            assert_eq!(script.tick(&w, tick).unwrap().status, "running");
        }
        let exhausted = script.tick(&w, 15).unwrap();
        assert_eq!(exhausted.status, "running");
        assert!(exhausted.action.is_none());
        assert_eq!(exhausted.memory["skipped"], 1);
        assert_eq!(script.tick(&w, 16).unwrap().status, "blocked");
        let mut script = start();
        script.tick(&w, 0).unwrap();
        w.stats[0].current = 99.0;
        let hurt = script.tick(&w, 1).unwrap();
        assert_eq!(hurt.status, "blocked");
        assert!(hurt.action.is_none());
        w.stats[0].current = 100.0;
        w.interactions = Some(vec![crate::model::Interaction {
            target_id: "plant".into(),
            kind: crate::model::InteractionKind::Pickup,
            item_key: Some(757006),
            available: true,
        }]);
        let mut script = start();
        script.tick(&w, 0).unwrap();
        assert_eq!(
            script.tick(&w, 1).unwrap().action.unwrap().keys,
            ["interact"]
        );
        assert_eq!(script.tick(&w, 100).unwrap().status, "running");
        w.inventory.as_mut().unwrap()[0].quantity = 5;
        assert_eq!(script.tick(&w, 6000).unwrap().status, "succeeded");
        // A completed scene can remain in memory; skip it and preserve the whole-task baseline.
        w.inventory.as_mut().unwrap()[0].quantity = 4;
        let mut next = w.resource_candidates.as_ref().unwrap()[0].clone();
        next.id = "second-plant".into();
        next.position.z = -3.0;
        w.resource_candidates.as_mut().unwrap().push(next);
        let mut batch = start();
        batch.configure(json!({"prefab_suffix":"gimmick_collect_bush_peony_01.prefab","item_name":"Pivoine","item_key":757006,"storage":1,"amount":2})).unwrap();
        batch.tick(&w, 0).unwrap();
        batch.tick(&w, 1).unwrap();
        w.inventory.as_mut().unwrap()[0].quantity = 5;
        assert_eq!(batch.tick(&w, 6000).unwrap().status, "running");
        let selected = batch.tick(&w, 6001).unwrap();
        assert_eq!(selected.memory["target"]["id"], "second-plant");
        assert_eq!(selected.memory["baseline"], 4);
        w.interactions.as_mut().unwrap()[0].target_id = "second-plant".into();
        // A fresh engine resumes exclusions and total progress, but discards old calibration.
        let mut saved = selected.memory.clone();
        saved["nav"] = json!({"phase":"travel","forward":{"x":0,"z":1}});
        saved["waypoint"] = json!({"x":0,"z":-3});
        let mut resumed = start();
        resumed.configure(json!({"prefab_suffix":"gimmick_collect_bush_peony_01.prefab","item_name":"Pivoine","item_key":757006,"storage":1,"amount":2})).unwrap();
        resumed.restore(saved).unwrap();
        let resumed_tick = resumed.tick(&w, 6002).unwrap();
        assert_eq!(resumed_tick.memory["baseline"], 4);
        assert_eq!(resumed_tick.memory["completed_ids"], json!(["plant"]));
        assert_eq!(resumed_tick.memory["target"]["id"], "second-plant");
        assert_eq!(resumed_tick.memory["nav"], json!({}));
        assert!(resumed_tick.memory.get("waypoint").is_none());
        assert_eq!(resumed_tick.action.unwrap().keys, ["interact"]);
        let mut no_hook =
            Script::compile("fn tick(ctx) {#{status:\"running\",memory:ctx.memory}}").unwrap();
        assert!(!no_hook.resumable());
        assert!(no_hook.restore(json!({})).is_err());
        assert_eq!(
            batch.tick(&w, 6002).unwrap().action.unwrap().keys,
            ["interact"]
        );
        w.inventory.as_mut().unwrap()[0].quantity = 6;
        let done = batch.tick(&w, 12000).unwrap();
        assert_eq!(done.status, "succeeded");
        assert_eq!(done.memory["collected"], 2);
        assert_eq!(done.memory["targets"], 2);
    }
}
