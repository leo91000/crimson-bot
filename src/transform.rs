//! Committed transform layout researched by Crimson World Builder (MIT; NOTICE).
use crate::model::Position;
use anyhow::{Result, ensure};
pub fn decode(snapshot: &[u8]) -> Result<Position> {
    ensure!(snapshot.len() == 0x48, "Invalid transform snapshot length");
    let float =
        |offset: usize| f32::from_le_bytes(snapshot[offset..offset + 4].try_into().unwrap()) as f64;
    let sector =
        |offset: usize| i16::from_le_bytes(snapshot[offset..offset + 2].try_into().unwrap()) as f64;
    let mut v = [
        float(0) + sector(0x0c) * 1000.0,
        float(4),
        float(8) + sector(0x0e) * 1000.0,
    ];
    ensure!(
        v.iter().all(|f| f.is_finite()),
        "Non-finite local transform"
    );
    let parent = [
        float(0x38) + sector(0x44) * 1000.0,
        float(0x3c),
        float(0x40) + sector(0x46) * 1000.0,
    ];
    ensure!(
        parent.iter().all(|f| f.is_finite()),
        "Non-finite parent transform"
    );
    if parent.iter().any(|f| f.abs() > 0.001) {
        let scale = [float(0x1c), float(0x20), float(0x24)];
        ensure!(
            scale
                .iter()
                .all(|s| s.is_finite() && (0.001..=1000.0).contains(s)),
            "Unsupported parent scale"
        );
        let q = [float(0x28), float(0x2c), float(0x30), float(0x34)];
        let norm = q.iter().map(|f| f * f).sum::<f64>();
        ensure!(
            norm.is_finite() && (norm - 1.0).abs() < 0.05,
            "Invalid parent quaternion"
        );
        for i in 0..3 {
            v[i] *= scale[i];
        }
        let t = [
            2.0 * (q[1] * v[2] - q[2] * v[1]),
            2.0 * (q[2] * v[0] - q[0] * v[2]),
            2.0 * (q[0] * v[1] - q[1] * v[0]),
        ];
        v = [
            v[0] + q[3] * t[0] + q[1] * t[2] - q[2] * t[1],
            v[1] + q[3] * t[1] + q[2] * t[0] - q[0] * t[2],
            v[2] + q[3] * t[2] + q[0] * t[1] - q[1] * t[0],
        ];
        for i in 0..3 {
            v[i] += parent[i];
        }
    }
    let result = Position {
        x: v[0],
        y: v[1],
        z: v[2],
    };
    ensure!(result.valid(), "World transform outside supported range");
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sector_transition_preserves_world_coordinates() {
        let mut data = [0u8; 0x48];
        data[0..4].copy_from_slice(&(-502.0f32).to_le_bytes());
        data[4..8].copy_from_slice(&(610.0f32).to_le_bytes());
        data[8..12].copy_from_slice(&(-374.0f32).to_le_bytes());
        data[12..14].copy_from_slice(&(-10i16).to_le_bytes());
        data[14..16].copy_from_slice(&(-4i16).to_le_bytes());
        let world = decode(&data).unwrap();
        assert_eq!(world.x, -10502.0);
        assert_eq!(world.z, -4374.0);
        data[0..4].copy_from_slice(&(498.0f32).to_le_bytes());
        data[12..14].copy_from_slice(&(-11i16).to_le_bytes());
        assert_eq!(decode(&data).unwrap().x, world.x);
    }
    #[test]
    fn parented_transforms_require_valid_scale_and_rotation() {
        let mut data = [0u8; 0x48];
        data[0..4].copy_from_slice(&(1.0f32).to_le_bytes());
        data[0x38..0x3c].copy_from_slice(&(10.0f32).to_le_bytes());
        assert!(decode(&data).is_err());
        for offset in [0x1c, 0x20, 0x24, 0x34] {
            data[offset..offset + 4].copy_from_slice(&(1.0f32).to_le_bytes());
        }
        assert_eq!(decode(&data).unwrap().x, 11.0);
    }
}
