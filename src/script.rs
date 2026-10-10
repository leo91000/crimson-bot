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
            .set_max_map_size(512);
        // Scripts receive data only. No filesystem, shell, network or input APIs.
        engine.set_module_resolver(rhai::module_resolvers::DummyModuleResolver::new());
        engine.on_print(|_| {});
        engine.register_fn("distance_xz", |ax: f64, az: f64, bx: f64, bz: f64| {
            (ax - bx).hypot(az - bz)
        });
        engine.register_fn("bearing_xz", |ax: f64, az: f64, bx: f64, bz: f64| {
            (bx - ax).atan2(bz - az)
        });
        let ast = engine.compile(source).context("Invalid Rhai task")?;
        Ok(Self {
            engine,
            ast,
            memory: json!({}),
        })
    }
    pub fn tick(&mut self, world: &World, elapsed_ms: u64) -> Result<Decision> {
        let context = rhai::serde::to_dynamic(
            json!({"world":world,"elapsed_ms":elapsed_ms,"memory":self.memory}),
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
}
