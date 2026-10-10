use anyhow::{Context, Result, bail, ensure};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    os::unix::fs::FileExt,
};
#[derive(Clone, Debug)]
pub struct Mapping {
    pub start: u64,
    pub end: u64,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
    pub path: String,
}
/// Provides no write operation or writable handle.
pub struct Memory {
    pub pid: u32,
    file: File,
    pub base: u64,
    pub image_size: u64,
    pub mappings: Vec<Mapping>,
}
impl Memory {
    pub fn attach() -> Result<Self> {
        let mut processes = vec![];
        for entry in fs::read_dir("/proc")? {
            let entry = entry?;
            let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            let Ok(comm) = fs::read_to_string(entry.path().join("comm")) else {
                continue;
            };
            if !comm.trim().starts_with("CrimsonDesert") {
                continue;
            }
            let Ok(cmd) = fs::read(entry.path().join("cmdline")) else {
                continue;
            };
            let executable = cmd.split(|b| *b == 0).next().unwrap_or_default();
            if executable.ends_with(b"CrimsonDesert.exe") {
                processes.push(pid);
            }
        }
        ensure!(
            processes.len() == 1,
            "Expected one local game process, found {}",
            processes.len()
        );
        Self::open(processes[0])
    }
    pub fn open(pid: u32) -> Result<Self> {
        let maps = fs::read_to_string(format!("/proc/{pid}/maps"))?;
        let mut mappings = vec![];
        for line in maps.lines() {
            let columns: Vec<_> = line.split_whitespace().collect();
            if columns.len() < 5 {
                continue;
            }
            let Some((start, end)) = columns[0].split_once('-') else {
                continue;
            };
            mappings.push(Mapping {
                start: u64::from_str_radix(start, 16)?,
                end: u64::from_str_radix(end, 16)?,
                readable: columns[1].starts_with('r'),
                writable: columns[1].contains('w'),
                executable: columns[1].contains('x'),
                path: columns[5..].join(" "),
            });
        }
        let base = mappings
            .iter()
            .filter(|m| m.path.ends_with("/CrimsonDesert.exe"))
            .map(|m| m.start)
            .min()
            .context("Game PE module not mapped")?;
        let file = File::open(format!("/proc/{pid}/mem")).context(
            "Read-only process access denied; do not disable system protections globally",
        )?;
        let mut memory = Self {
            pid,
            file,
            base,
            image_size: 0,
            mappings,
        };
        ensure!(memory.read(base, 2)? == b"MZ", "Invalid PE module header");
        let pe = base + memory.u32(base + 0x3c)? as u64;
        ensure!(memory.read(pe, 4)? == b"PE\0\0", "Invalid PE signature");
        memory.image_size = memory.u32(pe + 24 + 56)? as u64;
        ensure!(
            (4096..=1_073_741_824).contains(&memory.image_size),
            "Invalid PE image size"
        );
        Ok(memory)
    }
    pub fn fingerprint(&self) -> Result<String> {
        let path = &self
            .mappings
            .iter()
            .find(|m| m.start == self.base)
            .context("Module mapping missing")?
            .path;
        let mut file = File::open(path)?;
        let mut hash = Sha256::new();
        let mut buffer = vec![0; 1024 * 1024];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
        Ok(format!("{:x}", hash.finalize()))
    }
    pub fn read(&self, address: u64, size: usize) -> Result<Vec<u8>> {
        ensure!(size <= 16 * 1024 * 1024, "Read exceeds bounded chunk size");
        let mut bytes = vec![0; size];
        self.file
            .read_exact_at(&mut bytes, address)
            .with_context(|| format!("Unreadable address {address:#x}"))?;
        Ok(bytes)
    }
    pub fn u8(&self, a: u64) -> Result<u8> {
        Ok(self.read(a, 1)?[0])
    }
    pub fn u16(&self, a: u64) -> Result<u16> {
        Ok(u16::from_le_bytes(self.read(a, 2)?.try_into().unwrap()))
    }
    pub fn u32(&self, a: u64) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read(a, 4)?.try_into().unwrap()))
    }
    pub fn u64(&self, a: u64) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read(a, 8)?.try_into().unwrap()))
    }
    pub fn i64(&self, a: u64) -> Result<i64> {
        Ok(i64::from_le_bytes(self.read(a, 8)?.try_into().unwrap()))
    }
    pub fn ptr(&self, address: u64) -> Result<u64> {
        let pointer = self.u64(address)?;
        ensure!(
            (0x1000_0000..0x8000_0000_0000).contains(&pointer),
            "Invalid pointer"
        );
        Ok(pointer)
    }
    pub fn chain(&self, start: u64, offsets: &[u64]) -> Result<u64> {
        let mut address = start;
        for offset in offsets {
            address = self.ptr(address + offset)?;
        }
        Ok(address)
    }
    pub fn f32(&self, a: u64) -> Result<f32> {
        Ok(f32::from_le_bytes(self.read(a, 4)?.try_into().unwrap()))
    }
    /// MSVC x64 RTTI, validated against this module before following its RVA.
    pub fn type_name(&self, object: u64) -> Result<String> {
        let table = self.ptr(object)?;
        ensure!(
            table >= self.base && table < self.base + self.image_size,
            "Vtable outside game module"
        );
        let locator = self.ptr(table - 8)?;
        ensure!(
            locator >= self.base && locator + 24 <= self.base + self.image_size,
            "RTTI locator outside module"
        );
        ensure!(self.u32(locator)? == 1, "Unsupported RTTI locator");
        let descriptor = self.base + self.u32(locator + 12)? as u64;
        ensure!(
            descriptor >= self.base && descriptor + 176 < self.base + self.image_size,
            "RTTI descriptor outside module"
        );
        let bytes = self.read(descriptor + 16, 160)?;
        let end = bytes
            .iter()
            .position(|v| *v == 0)
            .context("Unterminated RTTI name")?;
        let name = std::str::from_utf8(&bytes[..end])?;
        ensure!(
            name.starts_with(".?AV") && name.ends_with("@pa@@"),
            "Unexpected RTTI namespace"
        );
        Ok(name.to_owned())
    }
    pub fn rip(&self, instruction: u64, displacement: u64, length: u64) -> Result<u64> {
        let rel = self.u32(instruction + displacement)? as i32 as i64;
        instruction
            .checked_add(length)
            .and_then(|v| v.checked_add_signed(rel))
            .context("RIP target overflow")
    }
    pub fn scan(&self, signature: &str) -> Result<Vec<u64>> {
        let pattern = Pattern::parse(signature)?;
        let mut matches = vec![];
        for mapping in &self.mappings {
            let start = mapping.start.max(self.base);
            let end = mapping.end.min(self.base + self.image_size);
            if !mapping.readable || end <= start {
                continue;
            }
            let mut at = start;
            while at < end {
                let size = (end - at).min(4 * 1024 * 1024) as usize;
                let bytes = self.read(at, size)?;
                for offset in pattern.find_all(&bytes) {
                    matches.push(at + offset as u64);
                }
                if size <= pattern.bytes.len() {
                    break;
                }
                at += (size - pattern.bytes.len() + 1) as u64;
            }
        }
        matches.sort_unstable();
        matches.dedup();
        Ok(matches)
    }
}
struct Pattern {
    bytes: Vec<Option<u8>>,
    anchor: u8,
    index: usize,
}
impl Pattern {
    fn parse(text: &str) -> Result<Self> {
        let mut bytes = vec![];
        for byte in text.split_whitespace() {
            bytes.push(if byte == "?" || byte == "??" {
                None
            } else {
                Some(u8::from_str_radix(byte, 16)?)
            });
        }
        let Some((index, &Some(anchor))) = bytes.iter().enumerate().find(|(_, b)| b.is_some())
        else {
            bail!("Empty signature")
        };
        Ok(Self {
            bytes,
            anchor,
            index,
        })
    }
    fn find_all(&self, data: &[u8]) -> Vec<usize> {
        let mut hits = vec![];
        for at in memchr::memchr_iter(self.anchor, data) {
            let Some(start) = at.checked_sub(self.index) else {
                continue;
            };
            let Some(window) = data.get(start..start + self.bytes.len()) else {
                continue;
            };
            if self
                .bytes
                .iter()
                .zip(window)
                .all(|(wanted, actual)| wanted.is_none_or(|v| v == *actual))
            {
                hits.push(start);
            }
        }
        hits
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signatures_allow_wildcards_and_overlaps() {
        let pattern = Pattern::parse("AA ? AA").unwrap();
        assert_eq!(pattern.find_all(&[0xaa, 1, 0xaa, 2, 0xaa]), [0, 2]);
        assert!(Pattern::parse("? ?").is_err());
        assert!(Pattern::parse("ZZ").is_err());
    }
}
