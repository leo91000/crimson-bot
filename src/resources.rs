//! Loaded collection scene objects, exposed as candidates until interaction proves usability.
use crate::{memory::Memory, model::Position, scene::Reader};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    hash::{DefaultHasher, Hash, Hasher},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    /// Opaque identity valid only within this process; never a memory address.
    pub id: String,
    pub prefab: String,
    pub position: Position,
}
pub struct Index {
    objects: Vec<u64>,
    pub truncated: bool,
}
const SCENE_TYPE: &str = ".?AVSceneObjectServer@pa@@";

fn collection_path(bytes: &[u8]) -> Result<String> {
    let path = std::str::from_utf8(bytes)?;
    ensure!(
        path.starts_with("/object/cd_gimmick/")
            && path.contains("/collect/")
            && path.ends_with(".prefab")
            && path
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b)),
        "Not a collection scene path"
    );
    Ok(path.to_owned())
}
fn read_candidate(reader: &impl Reader, object: u64) -> Result<Candidate> {
    ensure!(
        reader.type_name(object)? == SCENE_TYPE,
        "Scene object type changed"
    );
    let header = reader.read(object, 0x1d0)?;
    ensure!(header.len() == 0x1d0, "Incomplete scene header");
    let name = u64::from_le_bytes(header[0x38..0x40].try_into().unwrap());
    let name = reader.read(name, 12)?;
    ensure!(name.len() == 12, "Incomplete prefab string object");
    let text = u64::from_le_bytes(name[..8].try_into().unwrap());
    let length = u32::from_le_bytes(name[8..12].try_into().unwrap()) as usize;
    ensure!((1..=300).contains(&length), "Invalid prefab name length");
    let path = collection_path(&reader.read(text, length)?)?;
    // SceneObject's committed WORLD transform: scale, xyzw quaternion, local xyz, sectors.
    let xf = &header[0x1a4..0x1d0];
    let float = |off: usize| f32::from_le_bytes(xf[off..off + 4].try_into().unwrap()) as f64;
    let sector = |off: usize| i16::from_le_bytes(xf[off..off + 2].try_into().unwrap()) as f64;
    ensure!(
        [float(0), float(4), float(8)]
            .iter()
            .all(|v| v.is_finite() && (0.001..=1000.0).contains(v)),
        "Invalid scene scale"
    );
    let norm = (3..7).map(|i| float(i * 4).powi(2)).sum::<f64>();
    ensure!(
        norm.is_finite() && (norm - 1.0).abs() < 0.05,
        "Invalid scene rotation"
    );
    let position = Position {
        x: float(28) + sector(40) * 1000.0,
        y: float(32),
        z: float(36) + sector(42) * 1000.0,
    };
    ensure!(position.valid(), "Invalid scene position");
    let mut hash = DefaultHasher::new();
    object.hash(&mut hash);
    Ok(Candidate {
        id: format!("{:016x}", hash.finish()),
        prefab: path,
        position,
    })
}

impl Index {
    /// Discover only arenas containing a structurally verified scene object.
    /// At most 256 MiB read in 4 MiB chunks, 8 arenas and 8192 collection records.
    pub fn discover(memory: &Memory, actors: &[u64], origin: &Position) -> Result<Self> {
        let mut arenas = BTreeSet::new();
        let mut tables = BTreeSet::new();
        for &actor in actors.iter().take(8192) {
            let Ok(seed) = memory.chain(actor, &[0x68, 0x30, 0x18]) else {
                continue;
            };
            if memory.type_name(seed).ok().as_deref() != Some(SCENE_TYPE) {
                continue;
            }
            if let Some(mapping) = memory.mappings.iter().find(|m| {
                m.readable && m.writable && !m.executable && seed >= m.start && seed < m.end
            }) {
                arenas.insert((mapping.end - mapping.start, mapping.start, mapping.end));
                tables.insert(memory.ptr(seed)?);
                if arenas.len() > 8 {
                    arenas.pop_last();
                }
            }
        }
        ensure!(!arenas.is_empty(), "No verified scene allocation arena");
        let mut objects = BTreeSet::new();
        let mut remaining = 256 * 1024 * 1024usize;
        let mut truncated = false;
        for (_, start, end) in arenas {
            let mut at = start;
            while at < end && remaining > 0 && objects.len() < 8192 {
                let size = ((end - at) as usize).min(4 * 1024 * 1024).min(remaining);
                let data = memory.read(at, size)?;
                for &table in &tables {
                    let needle = table.to_le_bytes();
                    for offset in memchr::memmem::find_iter(&data, &needle) {
                        let object = at + offset as u64;
                        if !object.is_multiple_of(8) {
                            continue;
                        }
                        let Ok(candidate) = read_candidate(memory, object) else {
                            continue;
                        };
                        if (candidate.position.x - origin.x).hypot(candidate.position.z - origin.z)
                            <= 250.0
                        {
                            objects.insert(object);
                        }
                        if objects.len() >= 8192 {
                            truncated = true;
                            break;
                        }
                    }
                }
                remaining -= size;
                if size < 8 {
                    break;
                }
                at += size as u64 - 7;
            }
            if at < end {
                truncated = true;
            }
        }
        Ok(Self {
            objects: objects.into_iter().collect(),
            truncated,
        })
    }
    /// Reread identity and position on every observation; stale/reused objects are discarded.
    pub fn nearby(&self, memory: &Memory, origin: &Position) -> Vec<Candidate> {
        let mut candidates: Vec<_> = self
            .objects
            .iter()
            .filter_map(|&object| read_candidate(memory, object).ok())
            .filter(|c| (c.position.x - origin.x).hypot(c.position.z - origin.z) <= 150.0)
            .collect();
        let distance = |c: &Candidate| (c.position.x - origin.x).hypot(c.position.z - origin.z);
        candidates.sort_by(|a, b| distance(a).total_cmp(&distance(b)));
        candidates.truncate(64);
        candidates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        header: Vec<u8>,
        path: Vec<u8>,
        kind: String,
    }
    impl Reader for Fake {
        fn read(&self, address: u64, size: usize) -> Result<Vec<u8>> {
            let data = match address {
                0x1000 => self.header.clone(),
                0x2000 => {
                    let mut bytes = 0x3000u64.to_le_bytes().to_vec();
                    bytes.extend((self.path.len() as u32).to_le_bytes());
                    bytes
                }
                0x3000 => self.path.clone(),
                _ => anyhow::bail!("Unmapped"),
            };
            ensure!(data.len() >= size, "Short read");
            Ok(data[..size].to_vec())
        }
        fn type_name(&self, _: u64) -> Result<String> {
            Ok(self.kind.clone())
        }
        fn eligible(&self, _: u64) -> bool {
            true
        }
    }
    fn scene() -> Fake {
        let mut header = vec![0; 0x1d0];
        header[0x38..0x40].copy_from_slice(&0x2000u64.to_le_bytes());
        for offset in [0x1a4, 0x1a8, 0x1ac, 0x1bc] {
            header[offset..offset + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        header[0x1c0..0x1c4].copy_from_slice(&(-525.0f32).to_le_bytes());
        header[0x1c4..0x1c8].copy_from_slice(&(609.0f32).to_le_bytes());
        header[0x1c8..0x1cc].copy_from_slice(&(-425.0f32).to_le_bytes());
        header[0x1cc..0x1ce].copy_from_slice(&(-10i16).to_le_bytes());
        header[0x1ce..0x1d0].copy_from_slice(&(-4i16).to_le_bytes());
        Fake {header,path:b"/object/cd_gimmick/00_common/collect/plant/gimmick_collect_bush_rosemary_01.prefab".to_vec(),kind:SCENE_TYPE.into()}
    }
    #[test]
    fn collection_candidates_decode_world_sectors_and_opaque_identity() {
        let candidate = read_candidate(&scene(), 0x1000).unwrap();
        assert_eq!(candidate.position.x, -10525.0);
        assert_eq!(candidate.position.z, -4425.0);
        assert_ne!(candidate.id, "0x1000");
    }
    #[test]
    fn stale_types_noncollection_paths_and_invalid_transforms_are_rejected() {
        let mut input = scene();
        input.kind = ".?AVOther@pa@@".into();
        assert!(read_candidate(&input, 0x1000).is_err());
        input = scene();
        input.path = b"/object/equip/weapon.prefab".to_vec();
        assert!(read_candidate(&input, 0x1000).is_err());
        input = scene();
        input.header[0x1c0..0x1c4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(read_candidate(&input, 0x1000).is_err());
    }
}
