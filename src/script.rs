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
        engine.register_fn("find_ui", find_ui);
        engine.register_fn("find_ui_in", find_ui_in);
        engine.register_fn("inventory_count", inventory_count);
        engine.register_fn(
            "navigation_step",
            |world: Dynamic,
             memory: Dynamic,
             goal: Dynamic|
             -> Result<Dynamic, Box<rhai::EvalAltResult>> {
                let world = rhai::serde::from_dynamic::<World>(&world)?;
                let memory = rhai::serde::from_dynamic::<Value>(&memory)?;
                let goal = rhai::serde::from_dynamic::<Value>(&goal)?;
                let goal = serde_json::from_value::<crate::navigation::Goal>(goal)
                    .map_err(|e| -> Box<rhai::EvalAltResult> { e.to_string().into() })?;
                rhai::serde::to_dynamic(crate::navigation::step(&world, &memory, &goal))
            },
        );
        engine.register_fn(
            "navigation_retarget",
            |world: Dynamic,
             memory: Dynamic,
             goal: Dynamic|
             -> Result<Dynamic, Box<rhai::EvalAltResult>> {
                let world = rhai::serde::from_dynamic::<World>(&world)?;
                let memory = rhai::serde::from_dynamic::<Value>(&memory)?;
                let goal = rhai::serde::from_dynamic::<Value>(&goal)?;
                let goal = serde_json::from_value::<crate::navigation::Goal>(goal)
                    .map_err(|e| -> Box<rhai::EvalAltResult> { e.to_string().into() })?;
                rhai::serde::to_dynamic(crate::navigation::retarget(&world, &memory, &goal))
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
fn find_ui(ui: Dynamic, wanted: &str) -> Result<Dynamic, Box<rhai::EvalAltResult>> {
    find_ui_in(ui, wanted, 0.0, 0.0, 1.0, 1.0)
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
fn find_ui_in(
    ui: Dynamic,
    wanted: &str,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
) -> Result<Dynamic, Box<rhai::EvalAltResult>> {
    let ui = rhai::serde::from_dynamic::<crate::vision::Ui>(&ui)?;
    if ![left, top, right, bottom]
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        || right <= left
        || bottom <= top
    {
        return Err("Invalid normalized UI region".into());
    }
    let wanted = crate::vision::normalize(wanted);
    let wanted_chars: Vec<_> = wanted.chars().collect();
    if wanted_chars.is_empty() {
        return Ok(Dynamic::UNIT);
    }
    let mut found = vec![];
    for word in &ui.words {
        if crate::vision::normalize(&word.text) == wanted {
            let point = glyph_center(&word.glyphs).unwrap_or((
                (word.left + word.right) / 2.0,
                (word.top + word.bottom) / 2.0,
            ));
            found.push(point);
            continue;
        }
        let letters: Vec<_> = word
            .glyphs
            .iter()
            .map(|c| {
                crate::vision::normalize(&c.text)
                    .chars()
                    .next()
                    .unwrap_or(' ')
            })
            .collect();
        for (offset, window) in letters.windows(wanted_chars.len()).enumerate() {
            if window != wanted_chars {
                continue;
            }
            let span = &word.glyphs[offset..offset + wanted_chars.len()];
            if let Some(point) = glyph_center(span) {
                found.push(point);
            }
        }
    }
    found.retain(|&(x, y)| {
        let x = x / ui.width as f64;
        let y = y / ui.height as f64;
        x >= left && x <= right && y >= top && y <= bottom
    });
    if found.len() != 1 {
        return Ok(Dynamic::UNIT);
    }
    let (x, y) = found[0];
    rhai::serde::to_dynamic(json!({"x":x as u32,"y":y as u32}))
}
fn glyph_center(glyphs: &[crate::vision::Glyph]) -> Option<(f64, f64)> {
    let mut xs = vec![];
    let mut ys = vec![];
    for glyph in glyphs {
        let x = (glyph.left + glyph.right) / 2.0;
        let y = (glyph.top + glyph.bottom) / 2.0;
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        xs.push(x);
        ys.push(y);
    }
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(f64::total_cmp);
    ys.sort_by(f64::total_cmp);
    let median = |values: &[f64]| (values[(values.len() - 1) / 2] + values[values.len() / 2]) / 2.0;
    Some((median(&xs), median(&ys)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trailing_ocr_whitespace_does_not_shift_selection_into_neighbor() {
        let glyphs = vec![
            crate::vision::Glyph {
                text: "a".into(),
                left: 10.0,
                top: 0.0,
                right: 20.0,
                bottom: 10.0,
            },
            crate::vision::Glyph {
                text: "b".into(),
                left: 20.0,
                top: 0.0,
                right: 30.0,
                bottom: 10.0,
            },
            crate::vision::Glyph {
                text: "c".into(),
                left: 30.0,
                top: 0.0,
                right: 500.0,
                bottom: 10.0,
            },
        ];
        assert_eq!(glyph_center(&glyphs), Some((25.0, 5.0)));
    }
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
        world.ui = Some(crate::vision::Ui {
            mode: "world".into(),
            page: "unknown".into(),
            lines: vec![],
            words: vec![],
            width: 100,
            height: 100,
            processing_ms: 0,
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
    fn ui_selectors_preserve_geometry_through_rhai_and_reject_ambiguity() {
        let word = crate::vision::Line {
            text: "Factions".into(),
            left: 10.0,
            top: 20.0,
            right: 30.0,
            bottom: 40.0,
            glyphs: vec![],
        };
        let mut w = world();
        w.ui = Some(crate::vision::Ui {
            mode: "journal".into(),
            page: "unknown".into(),
            lines: vec![word.clone()],
            words: vec![word.clone()],
            width: 1920,
            height: 1080,
            processing_ms: 1,
        });
        // A real menu contains enough nested fields to exceed the old 512 limit.
        for _ in 0..160 {
            w.ui.as_mut().unwrap().words.push(crate::vision::Line {
                text: "Other".into(),
                ..word.clone()
            });
        }
        let mut script = Script::compile(
            "fn tick(ctx) {#{status:\"succeeded\",memory:find_ui(ctx.world.ui,\"Factions\")}} ",
        )
        .unwrap();
        assert_eq!(script.tick(&w, 0).unwrap().memory, json!({"x":20,"y":30}));
        w.ui.as_mut().unwrap().words.push(word);
        assert_eq!(script.tick(&w, 1).unwrap().memory, Value::Null);
    }
    #[test]
    fn gather_requires_named_prompt_and_measured_inventory_result() {
        let mut w = world();
        w.inventory = Some(vec![crate::model::Item {
            storage: 1,
            type_id: 1189,
            quantity: 4,
        }]);
        let plant = crate::vision::Line {
            text: "Lavande".into(),
            left: 550.0,
            top: 400.0,
            right: 650.0,
            bottom: 430.0,
            glyphs: vec![],
        };
        let pickup = crate::vision::Line {
            text: "Prendre".into(),
            left: 550.0,
            top: 435.0,
            right: 650.0,
            bottom: 460.0,
            glyphs: vec![],
        };
        w.ui = Some(crate::vision::Ui {
            mode: "world".into(),
            page: "unknown".into(),
            lines: vec![plant.clone(), pickup.clone()],
            words: vec![plant, pickup],
            width: 1000,
            height: 1000,
            processing_ms: 1,
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
            .configure(json!({"item_name":"Lavande","item_key":751107,"storage":1,"amount":1}))
            .unwrap();
        assert!(script.tick(&w, 0).unwrap().action.is_none());
        assert_eq!(
            script.tick(&w, 1).unwrap().action.unwrap().keys,
            ["interact"]
        );
        assert_eq!(script.tick(&w, 100).unwrap().status, "running");
        w.inventory.as_mut().unwrap()[0].quantity = 5;
        assert_eq!(script.tick(&w, 6000).unwrap().status, "succeeded");
    }
    #[test]
    fn resource_route_checks_navigation_health_prompt_and_inventory() {
        let mut w = world();
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
        w.ui = Some(crate::vision::Ui {
            mode: "world".into(),
            page: "unknown".into(),
            lines: vec![],
            words: vec![],
            width: 1000,
            height: 1000,
            processing_ms: 0,
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
        assert_eq!(
            script.tick(&w, 1).unwrap().action.unwrap().keys,
            ["backward"]
        );
        w.position.as_mut().unwrap().z = -0.3;
        assert_eq!(script.tick(&w, 2).unwrap().status, "running");
        w.position.as_mut().unwrap().z = -1.2;
        let near = script.tick(&w, 3).unwrap();
        assert_eq!(near.status, "blocked");
        assert!(near.action.is_none());
        let mut script = start();
        script.tick(&w, 0).unwrap();
        w.stats[0].current = 99.0;
        let hurt = script.tick(&w, 1).unwrap();
        assert_eq!(hurt.status, "blocked");
        assert!(hurt.action.is_none());
        w.stats[0].current = 100.0;
        let word = crate::vision::Line {
            text: "Pivoine".into(),
            left: 550.0,
            top: 400.0,
            right: 650.0,
            bottom: 430.0,
            glyphs: vec![],
        };
        w.ui.as_mut().unwrap().words = vec![
            word.clone(),
            crate::vision::Line {
                text: "Prendre".into(),
                top: 435.0,
                bottom: 460.0,
                ..word
            },
        ];
        let mut script = start();
        script.tick(&w, 0).unwrap();
        assert_eq!(
            script.tick(&w, 1).unwrap().action.unwrap().keys,
            ["interact"]
        );
        assert_eq!(script.tick(&w, 100).unwrap().status, "running");
        w.inventory.as_mut().unwrap()[0].quantity = 5;
        assert_eq!(script.tick(&w, 6000).unwrap().status, "succeeded");
    }
}
