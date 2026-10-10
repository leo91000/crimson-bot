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
}
fn find_ui(ui: Dynamic, wanted: &str) -> Result<Dynamic, Box<rhai::EvalAltResult>> {
    let ui = rhai::serde::from_dynamic::<crate::vision::Ui>(&ui)?;
    let wanted = crate::vision::normalize(wanted);
    let found: Vec<_> = ui
        .words
        .iter()
        .filter(|word| crate::vision::normalize(&word.text) == wanted)
        .collect();
    if found.len() != 1 {
        return Ok(Dynamic::UNIT);
    }
    let word = found[0];
    rhai::serde::to_dynamic(
        json!({"x":((word.left+word.right)/2.0) as u32,"y":((word.top+word.bottom)/2.0) as u32}),
    )
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
        };
        let mut w = world();
        w.ui = Some(crate::vision::Ui {
            mode: "journal".into(),
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
}
