use serde::{Deserialize, Serialize};
/// Native interface state. Only a validated memory adapter may populate this.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ui {
    pub mode: String,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionKind {
    Pickup,
    Talk,
    TurnIn,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Interaction {
    pub target_id: String,
    pub kind: InteractionKind,
    pub item_key: Option<u32>,
    pub available: bool,
}
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
    pub ui: Option<Ui>,
    /// None is unavailable, rather than a confirmed absence of interactions.
    #[serde(default)]
    pub interactions: Option<Vec<Interaction>>,
    pub characters: usize,
    pub capabilities: Vec<String>,
    pub diagnostics: serde_json::Value,
}
