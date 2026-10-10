use crate::{
    memory::Memory,
    model::{Item, Position, Stat, World},
};
use anyhow::{Result, ensure};
use serde_json::json;
use std::{
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};
// Structure facts and signatures derived from Trinity, MIT. See NOTICE.
const MANAGER_PATTERNS: [&str; 4] = [
    "48 8B 05 ?? ?? ?? ?? 48 8B 08 4D 8B 00 49 C1 E8 20",
    "48 8B 05 ?? ?? ?? ?? 44 8B 82 90 00 00 00 48 8D 54 24 ?? 48 8B 08 E8",
    "48 8B 05 ?? ?? ?? ?? 44 8B 81 60 01 00 00 48 8D 55 ?? 48 8B 08 E8",
    "48 8B 05 ?? ?? ?? ?? 44 8B 07 48 8D 54 24 ?? 48 8B 08 E8",
];
const INVENTORY_PATTERN: &str = "48 89 54 24 ? 53 48 83 EC 30 48 8B DA C7 44 24 20 00 00 00 00 48 8B 05 ? ? ? ? 48 8B 50 30 48 8B 52 50 48 8B CB E8";
pub struct Game {
    pub memory: Memory,
    manager_global: Option<u64>,
    inventory_global: Option<u64>,
    xyz: Option<(u64, u64)>,
    pub diagnostics: serde_json::Value,
}
impl Game {
    pub fn attach() -> Result<Self> {
        let memory = Memory::attach()?;
        let mut votes = BTreeSet::new();
        let mut signatures = vec![];
        for (index, pattern) in MANAGER_PATTERNS.iter().enumerate() {
            let hits = memory.scan(pattern)?;
            for hit in &hits {
                votes.insert(memory.rip(*hit, 3, 7)?);
            }
            signatures.push(json!({"anchor": index, "matches": hits.len()}));
        }
        let manager_global = if votes.len() == 1 {
            votes.first().copied()
        } else {
            None
        };
        let inv_hits = memory.scan(INVENTORY_PATTERN)?;
        let inventory_global = if inv_hits.len() == 1 {
            Some(memory.rip(inv_hits[0] + 0x15, 3, 7)?)
        } else {
            None
        };
        let xyz_hits = memory.scan("C5 FB 11 05 ? ? ? ? 8B 44 24 28 89 05 ? ? ? ?")?;
        let xyz = if xyz_hits.len() == 1 {
            Some((
                memory.rip(xyz_hits[0], 4, 8)?,
                memory.rip(xyz_hits[0] + 12, 2, 6)?,
            ))
        } else {
            None
        };
        let diagnostics = json!({"executable_sha256": memory.fingerprint()?, "image_size": memory.image_size, "module_base": format!("{:#x}",memory.base),
            "manager_anchors": signatures, "manager_roots": votes.iter().map(|v|format!("{v:#x}")).collect::<Vec<_>>(),
            "inventory_signature_matches": inv_hits.len(), "xyz_signature_matches": xyz_hits.len()});
        Ok(Self {
            memory,
            manager_global,
            inventory_global,
            xyz,
            diagnostics,
        })
    }
    fn characters(&self) -> Result<Vec<u64>> {
        let global = self
            .manager_global
            .ok_or_else(|| anyhow::anyhow!("Manager unavailable"))?;
        let manager = self.memory.chain(global, &[0, 0])?;
        let data = self.memory.ptr(manager + 0xb8)?;
        let count = self.memory.u32(manager + 0xc0)?;
        ensure!((1..=8192).contains(&count), "Character count invalid");
        let bytes = self.memory.read(data, count as usize * 8)?;
        Ok(bytes
            .chunks_exact(8)
            .map(|v| u64::from_le_bytes(v.try_into().unwrap()))
            .filter(|v| *v > 0x10000)
            .collect())
    }
    fn controlled(&self, characters: &[u64]) -> Result<u64> {
        let mut found = vec![];
        for &owner in characters {
            let Ok(descriptor) = self.memory.ptr(owner + 0x88) else {
                continue;
            };
            let Ok(tag) = self.memory.u8(descriptor + 1) else {
                continue;
            };
            if tag != 1 && tag != 9 {
                continue;
            }
            let Ok(controller) = self.memory.ptr(owner + 0xa0) else {
                continue;
            };
            if self.memory.ptr(controller + 0xd0).ok() == Some(owner) {
                found.push(owner);
            }
        }
        ensure!(
            found.len() == 1,
            "Controlled body ambiguous or absent: {}",
            found.len()
        );
        Ok(found[0])
    }
    fn stats(&self, owner: u64) -> Result<Vec<Stat>> {
        let array = self.memory.chain(owner, &[0x68, 0x20, 0x18, 0x58])?;
        ensure!(self.memory.u32(array)? == 0, "Health entry type mismatch");
        let mut values = vec![];
        for index in 0..16 {
            let entry = array + index * 0x90;
            let kind = self.memory.u32(entry)?;
            if ![0, 17, 18, 20, 21].contains(&kind) {
                continue;
            }
            let current = self.memory.i64(entry + 8)? as f64 / 1000.0;
            let maximum = self.memory.i64(entry + 0x30)? as f64 / 1000.0;
            if (0.0..=1_000_000.0).contains(&current) && (0.0..=1_000_000.0).contains(&maximum) {
                values.push(Stat {
                    kind,
                    current,
                    maximum,
                });
            }
        }
        Ok(values)
    }
    fn position(&self, owner: u64) -> Result<Position> {
        let vector = self.memory.chain(owner, &[0x68, 0x40, 0x08, 0x248])? + 0x90;
        self.position_at(vector, vector + 8)
    }
    fn position_at(&self, xy: u64, z: u64) -> Result<Position> {
        let position = Position {
            x: self.memory.f32(xy)?,
            y: self.memory.f32(xy + 4)?,
            z: self.memory.f32(z)?,
        };
        ensure!(position.valid(), "Invalid position");
        Ok(position)
    }
    fn inventory(&self) -> Result<Vec<Item>> {
        let global = self
            .inventory_global
            .ok_or_else(|| anyhow::anyhow!("Inventory unavailable"))?;
        let holder = self.memory.chain(global, &[0, 0x30, 0x50, 0x68, 0xb8])?;
        let buckets = self.memory.ptr(holder + 0x18)?;
        let count = self.memory.u32(holder + 0x20)?;
        ensure!((1..=128).contains(&count), "Invalid bucket count");
        let mut items = vec![];
        for index in 0..count {
            let bucket = self.memory.ptr(buckets + index as u64 * 8)?;
            let size = self.memory.u16(bucket + 8)?;
            ensure!(size <= 8192, "Invalid slot count");
            if size == 0 {
                continue;
            }
            let slots = self.memory.ptr(bucket)?;
            let storage = self.memory.u16(bucket + 0x10)?;
            let bytes = self.memory.read(slots, size as usize * 0xc0)?;
            for slot in bytes.chunks_exact(0xc0) {
                let type_id = u16::from_le_bytes(slot[8..10].try_into().unwrap());
                if type_id == 0xffff {
                    continue;
                }
                let quantity = i64::from_le_bytes(slot[0x10..0x18].try_into().unwrap());
                ensure!(
                    (0..=1_000_000_000).contains(&quantity),
                    "Invalid item quantity"
                );
                if quantity > 0 {
                    items.push(Item {
                        storage,
                        type_id,
                        quantity,
                    });
                }
            }
        }
        Ok(items)
    }
    pub fn observe(&mut self) -> Result<World> {
        let characters = self.characters().unwrap_or_default();
        let owner = self.controlled(&characters).ok();
        let stats = owner.and_then(|v| self.stats(v).ok()).unwrap_or_default();
        let position = owner
            .and_then(|v| self.position(v).ok())
            .or_else(|| self.xyz.and_then(|(xy, z)| self.position_at(xy, z).ok()));
        let inventory = self.inventory().ok();
        let mut capabilities = vec![];
        if owner.is_some() {
            capabilities.push("controlled_body".into());
        }
        if !stats.is_empty() {
            capabilities.push("vitals".into());
        }
        if position.is_some() {
            capabilities.push("position_candidate".into());
        }
        if inventory.is_some() {
            capabilities.push("inventory".into());
        }
        Ok(World {
            pid: self.memory.pid,
            observed_at_ms: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64,
            player_resolved: owner.is_some(),
            position,
            stats,
            inventory,
            characters: characters.len(),
            capabilities,
            diagnostics: self.diagnostics.clone(),
        })
    }
}
