use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
impl Position {
    pub fn valid(&self) -> bool {
        [self.x, self.y, self.z]
            .iter()
            .all(|v| v.is_finite() && v.abs() < 1_000_000.0)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stat {
    pub kind: u32,
    pub current: f64,
    pub maximum: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub storage: u16,
    pub type_id: u16,
    pub quantity: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct World {
    pub pid: u32,
    pub observed_at_ms: u64,
    pub player_resolved: bool,
    pub position: Option<Position>,
    pub stats: Vec<Stat>,
    // None means unavailable; empty means a validated, empty inventory.
    pub inventory: Option<Vec<Item>>,
    #[serde(default)]
    pub resource_candidates: Option<Vec<crate::resources::Candidate>>,
    pub ui: Option<crate::vision::Ui>,
    pub characters: usize,
    pub capabilities: Vec<String>,
    pub diagnostics: serde_json::Value,
}
