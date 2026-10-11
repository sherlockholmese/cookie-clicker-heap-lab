//! Explicit, versioned workshop snapshots.
#![forbid(unsafe_code)]
use bincode::Options;
use cookie_game::Machine;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};
use thiserror::Error;
use vm_core::ExecutionBackend;

pub const MAGIC: [u8; 8] = *b"CCPW64\r\n";
pub const FORMAT_VERSION: u32 = 1;
pub const COMPATIBILITY_VERSION: u32 = 1;
pub const MAX_FILE_SIZE: u64 = 2 * 1024 * 1024;
const HEADER_SIZE: usize = 36;
#[derive(Debug, Error)]
pub enum SaveError {
    #[error("save file I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid save file: {0}")]
    Invalid(String),
    #[error("unsupported save-format version {0}")]
    UnsupportedVersion(u32),
    #[error("incompatible application snapshot version {0}")]
    Incompatible(u32),
    #[error("save codec: {0}")]
    Codec(#[from] bincode::Error),
}
#[derive(Serialize, Deserialize)]
struct Snapshot {
    machine: Machine,
}
fn codec() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
        .with_limit(MAX_FILE_SIZE)
        .reject_trailing_bytes()
}
pub fn encode(machine: &Machine) -> Result<Vec<u8>, SaveError> {
    machine
        .validate()
        .map_err(|e| SaveError::Invalid(e.to_string()))?;
    let payload = codec().serialize(&Snapshot {
        machine: machine.snapshot(),
    })?;
    let mut data = Vec::with_capacity(HEADER_SIZE + payload.len());
    data.extend(MAGIC);
    data.extend(FORMAT_VERSION.to_le_bytes());
    data.extend(COMPATIBILITY_VERSION.to_le_bytes());
    data.extend(*b"U64LE\0\0\0");
    data.extend((payload.len() as u64).to_le_bytes());
    data.extend(crc32fast::hash(&payload).to_le_bytes());
    data.extend(payload);
    Ok(data)
}
pub fn decode(data: &[u8]) -> Result<Machine, SaveError> {
    let bad = |s: &str| SaveError::Invalid(s.into());
    if data.len() < HEADER_SIZE || data.len() as u64 > MAX_FILE_SIZE {
        return Err(bad("truncated or oversized file"));
    }
    if data[..8] != MAGIC {
        return Err(bad("expected CCPW64 magic"));
    }
    let u32at = |i| u32::from_le_bytes(data[i..i + 4].try_into().expect("checked header"));
    let version = u32at(8);
    if version != FORMAT_VERSION {
        return Err(SaveError::UnsupportedVersion(version));
    }
    let compatibility = u32at(12);
    if compatibility != COMPATIBILITY_VERSION {
        return Err(SaveError::Incompatible(compatibility));
    }
    if data[16..24] != *b"U64LE\0\0\0" {
        return Err(bad("expected 64-bit little-endian architecture"));
    }
    let length = u64::from_le_bytes(data[24..32].try_into().expect("checked header"));
    if length != data.len() as u64 - HEADER_SIZE as u64 {
        return Err(bad("payload length mismatch"));
    }
    let payload = &data[HEADER_SIZE..];
    if crc32fast::hash(payload) != u32at(32) {
        return Err(bad("checksum mismatch"));
    }
    let snapshot: Snapshot = codec().deserialize(payload)?;
    snapshot
        .machine
        .validate()
        .map_err(|e| SaveError::Invalid(e.to_string()))?;
    Ok(snapshot.machine)
}
pub fn save(path: &Path, machine: &Machine) -> Result<(), SaveError> {
    let bytes = encode(machine)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(&bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| SaveError::Io(e.error))?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}
pub fn load(path: &Path) -> Result<Machine, SaveError> {
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_FILE_SIZE {
        return Err(SaveError::Invalid("file exceeds size limit".into()));
    }
    let mut bytes = vec![];
    file.take(MAX_FILE_SIZE + 1).read_to_end(&mut bytes)?;
    decode(&bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip() {
        let mut m = Machine::new(1).unwrap();
        m.click().unwrap();
        assert_eq!(decode(&encode(&m).unwrap()).unwrap(), m.snapshot());
    }
    #[test]
    fn rejects_damage() {
        let mut b = encode(&Machine::new(1).unwrap()).unwrap();
        *b.last_mut().unwrap() ^= 1;
        assert!(decode(&b).is_err());
    }
}
