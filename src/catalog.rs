use crate::memory::Memory;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogItem {
    pub type_id: u16,
    pub key: u32,
    pub internal_name: String,
}
pub fn read(memory: &Memory, global_rva: u64) -> Result<BTreeMap<u16, CatalogItem>> {
    let manager = memory.chain(memory.base + global_rva, &[0, 0])?;
    ensure!(
        memory.type_name(manager)? == ".?AVItemInfoManager@pa@@",
        "Item table type mismatch"
    );
    let count = memory.u32(manager + 8)?;
    ensure!((1..=65535).contains(&count), "Item table count invalid");
    let rows = memory.ptr(manager + 0x58)?;
    let data = memory.read(rows, count as usize * 8)?;
    let mut items = BTreeMap::new();
    for (index, bytes) in data.as_chunks::<8>().0.iter().enumerate() {
        let row = u64::from_le_bytes(*bytes);
        let Ok(key) = memory.u32(row) else { continue };
        let Ok(name_object) = memory.ptr(row + 8) else {
            continue;
        };
        let Ok(length) = memory.u32(name_object + 8) else {
            continue;
        };
        if length == 0 || length > 256 {
            continue;
        }
        let Ok(buffer) = memory.ptr(name_object) else {
            continue;
        };
        let Ok(bytes) = memory.read(buffer, length as usize) else {
            continue;
        };
        let Ok(name) = String::from_utf8(bytes) else {
            continue;
        };
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            continue;
        }
        let type_id = index as u16;
        items.insert(
            type_id,
            CatalogItem {
                type_id,
                key,
                internal_name: name,
            },
        );
    }
    ensure!(!items.is_empty(), "Item table empty or unreadable");
    Ok(items)
}
