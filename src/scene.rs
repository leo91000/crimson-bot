//! Bounded, read-only discovery hints. These are never actionable resource targets.
use crate::memory::Memory;
use anyhow::Result;
use serde::Serialize;
use std::collections::{BTreeSet, VecDeque};

pub trait Reader {
    fn read(&self, address: u64, size: usize) -> Result<Vec<u8>>;
    fn type_name(&self, address: u64) -> Result<String>;
    fn eligible(&self, address: u64) -> bool;
}
impl Reader for Memory {
    fn read(&self, address: u64, size: usize) -> Result<Vec<u8>> {
        self.read(address, size)
    }
    fn type_name(&self, address: u64) -> Result<String> {
        self.type_name(address)
    }
    fn eligible(&self, address: u64) -> bool {
        address >= 0x1000_0000
            && self.mappings.iter().any(|m| {
                m.readable
                    && m.writable
                    && !m.executable
                    && address >= m.start
                    && address < m.end
                    && !(address >= self.base && address < self.base + self.image_size)
            })
    }
}
#[derive(Debug, Serialize)]
pub struct Hint {
    pub offsets: Vec<usize>,
    pub kind: String,
    pub prefab: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub nodes_read: usize,
    pub truncated: bool,
    pub hints: Vec<Hint>,
}

/// Accept only engine resource paths, never arbitrary strings from process memory.
fn prefab(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|b| *b == 0)?;
    let path = std::str::from_utf8(&bytes[..end]).ok()?;
    if path.len() > 300
        || !path.ends_with(".prefab")
        || !(path.starts_with("/object/") || path.starts_with("/character/"))
        || !path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
    {
        return None;
    }
    Some(path.to_owned())
}

/// A pointer association is a research hint, not proof that an actor is a plant.
/// Per actor: at most 256 nodes, 5 edges deep, and 256 B for the root / 512 B per node.
pub fn probe(reader: &impl Reader, actor: u64) -> Report {
    let mut pending = VecDeque::from([(actor, vec![])]);
    let mut seen = BTreeSet::from([actor]);
    let mut report = Report {
        nodes_read: 0,
        truncated: false,
        hints: vec![],
    };
    while let Some((address, offsets)) = pending.pop_front() {
        if report.nodes_read >= 256 {
            report.truncated = true;
            break;
        }
        report.nodes_read += 1;
        let Ok(bytes) = reader.read(address, if offsets.is_empty() { 0x100 } else { 0x200 }) else {
            continue;
        };
        let path = prefab(&bytes);
        let kind = reader.type_name(address).unwrap_or_default();
        if path.is_some() || kind.contains("SceneObject") || kind.contains("ResourcePath") {
            report.hints.push(Hint {
                offsets: offsets.clone(),
                kind,
                prefab: path,
            });
        }
        if offsets.len() >= 5 {
            continue;
        }
        for (index, word) in bytes.as_chunks::<8>().0.iter().enumerate() {
            let pointer = u64::from_le_bytes(*word);
            if !reader.eligible(pointer) || seen.contains(&pointer) {
                continue;
            }
            // Bound queued nodes too, including unreadable pointers and cyclic graphs.
            if seen.len() >= 256 {
                report.truncated = true;
                continue;
            }
            seen.insert(pointer);
            let mut path = offsets.clone();
            path.push(index * 8);
            pending.push_back((pointer, path));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, collections::BTreeMap};
    struct Fake {
        nodes: BTreeMap<u64, Vec<u8>>,
        reads: Cell<usize>,
    }
    impl Reader for Fake {
        fn read(&self, address: u64, size: usize) -> Result<Vec<u8>> {
            self.reads.set(self.reads.get() + 1);
            let mut bytes = self
                .nodes
                .get(&address)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing"))?;
            bytes.resize(size, 0);
            Ok(bytes)
        }
        fn type_name(&self, _: u64) -> Result<String> {
            Ok(String::new())
        }
        fn eligible(&self, address: u64) -> bool {
            self.nodes.contains_key(&address)
        }
    }
    #[test]
    fn finds_only_prefabs_and_deduplicates_cycles() {
        let root = 0x1000_0000u64;
        let child = root + 0x1000;
        let mut actor = vec![0; 0x800];
        actor[0x28..0x30].copy_from_slice(&child.to_le_bytes());
        let path =
            b"/object/cd_gimmick/00_common/collect/plant/gimmick_collect_bush_rosemary_01.prefab\0";
        let mut leaf = vec![0; 0x200];
        leaf[..path.len()].copy_from_slice(path);
        leaf[0x100..0x108].copy_from_slice(&root.to_le_bytes());
        let reader = Fake {
            nodes: BTreeMap::from([(root, actor), (child, leaf)]),
            reads: Cell::new(0),
        };
        let result = probe(&reader, root);
        assert_eq!(result.nodes_read, 2);
        assert!(!result.truncated);
        assert_eq!(result.hints.len(), 1);
        assert_eq!(result.hints[0].offsets, [0x28]);
        assert_eq!(
            result.hints[0].prefab.as_deref(),
            Some(std::str::from_utf8(&path[..path.len() - 1]).unwrap())
        );
        assert!(prefab(b"/home/deck/private.prefab\0").is_none());
        assert!(prefab(b"account-token\0").is_none());
    }
    #[test]
    fn bounds_dense_pointer_graphs() {
        let root = 0x1000_0000u64;
        let mut nodes = BTreeMap::new();
        for i in 0..512u64 {
            let mut bytes = vec![0; 0x800];
            for j in 0..256u64 {
                let target = root + ((i + j + 1) % 512) * 0x1000;
                bytes[j as usize * 8..j as usize * 8 + 8].copy_from_slice(&target.to_le_bytes());
            }
            nodes.insert(root + i * 0x1000, bytes);
        }
        let reader = Fake {
            nodes,
            reads: Cell::new(0),
        };
        let result = probe(&reader, root);
        assert!(result.truncated);
        assert!(reader.reads.get() <= 256);
    }
}
