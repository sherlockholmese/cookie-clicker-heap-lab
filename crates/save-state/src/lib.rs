//! Explicit complete snapshots. No wall-clock timestamps, automatic discovery, or autosave.
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

pub const MAGIC: [u8; 8] = *b"CHLS64\r\n";
pub const FORMAT_VERSION: u32 = 1;
pub const COMPATIBILITY_VERSION: u32 = 4;
pub const MAX_FILE_SIZE: u64 = 32 * 1024 * 1024;
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
    if payload.len() as u64 > MAX_FILE_SIZE - HEADER_SIZE as u64 {
        return Err(SaveError::Invalid("snapshot too large".into()));
    }
    let mut data = Vec::with_capacity(HEADER_SIZE + payload.len());
    data.extend(MAGIC);
    data.extend(FORMAT_VERSION.to_le_bytes());
    data.extend(COMPATIBILITY_VERSION.to_le_bytes());
    data.extend(*b"U64LE\0\0\0"); // architecture and endianness
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
        return Err(bad("expected CHLS64 magic"));
    }
    let u32at = |i| u32::from_le_bytes(data[i..i + 4].try_into().expect("checked header length"));
    let version = u32at(8);
    if version != FORMAT_VERSION {
        return Err(SaveError::UnsupportedVersion(version));
    }
    let compatibility = u32at(12);
    if !(1..=COMPATIBILITY_VERSION).contains(&compatibility) {
        return Err(SaveError::Incompatible(compatibility));
    }
    if data[16..24] != *b"U64LE\0\0\0" {
        return Err(bad("expected 64-bit little-endian architecture"));
    }
    let length = u64::from_le_bytes(data[24..32].try_into().expect("checked header length"));
    if length != data.len() as u64 - HEADER_SIZE as u64 {
        return Err(bad("payload length mismatch"));
    }
    let payload = &data[HEADER_SIZE..];
    if crc32fast::hash(payload) != u32at(32) {
        return Err(bad("checksum mismatch"));
    }
    let mut snapshot: Snapshot = codec().deserialize(payload)?;
    snapshot
        .machine
        .validate()
        .map_err(|e| SaveError::Invalid(e.to_string()))?;
    if compatibility == 1 {
        snapshot
            .machine
            .initialize_activities()
            .map_err(|e| SaveError::Invalid(e.to_string()))?;
        snapshot.machine.memory.clear_events();
        snapshot
            .machine
            .validate()
            .map_err(|e| SaveError::Invalid(e.to_string()))?;
    }
    if compatibility < 3 {
        snapshot
            .machine
            .initialize_encounters()
            .map_err(|e| SaveError::Invalid(e.to_string()))?;
        snapshot.machine.memory.clear_events();
        snapshot
            .machine
            .validate()
            .map_err(|e| SaveError::Invalid(e.to_string()))?;
    }
    if compatibility < 4 {
        snapshot
            .machine
            .initialize_shutdown_callbacks()
            .map_err(|e| SaveError::Invalid(e.to_string()))?;
        snapshot.machine.memory.clear_events();
        snapshot
            .machine
            .validate()
            .map_err(|e| SaveError::Invalid(e.to_string()))?;
    }
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
        return Err(SaveError::Invalid("file exceeds 32 MiB limit".into()));
    }
    let mut bytes = vec![];
    file.take(MAX_FILE_SIZE + 1).read_to_end(&mut bytes)?;
    decode(&bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visitors_resume_and_older_saves_gain_new_event_timing() {
        let mut m = Machine::new(8).unwrap();
        for _ in 0..100 {
            m.click().unwrap();
        }
        let a = m.encounters_address().unwrap();
        let next = m
            .memory
            .read_u64(a + cookie_game::encounter_field::NEXT_SPAWN)
            .unwrap();
        m.tick = next - 1;
        for _ in 0..21 {
            m.advance_tick().unwrap();
        }
        let mut restored = decode(&encode(&m).unwrap()).unwrap();
        assert_eq!(m.snapshot(), restored.snapshot());
        for machine in [&mut m, &mut restored] {
            for _ in 0..machine.encounter_view().unwrap().health {
                machine.shoo_enemy().unwrap();
            }
        }
        assert_eq!(m.snapshot(), restored.snapshot());
        assert_eq!(restored.view().unwrap().cookies, 100);
        // Model a compatibility-2 save with its original activity layout and no visitors.
        m.heap.free(a).unwrap();
        m.write_memory(vm_core::Architecture::GLOBALS + 16, &0u64.to_le_bytes())
            .unwrap();
        let mut bytes = encode(&m).unwrap();
        bytes[12..16].copy_from_slice(&2u32.to_le_bytes());
        let upgraded = decode(&bytes).unwrap();
        assert_eq!(upgraded.player().unwrap(), m.player().unwrap());
        assert_eq!(upgraded.tick, m.tick);
        assert_eq!(upgraded.rng_state, m.rng_state);
        assert_eq!(upgraded.view().unwrap().cookies, 100);
        let gift = upgraded
            .memory
            .read_u64(upgraded.object(cookie_game::field::EFFECTS).unwrap() + 16)
            .unwrap();
        assert!(gift >= upgraded.tick + 180 * cookie_game::TICKS_PER_SECOND);
    }
    #[test]
    fn round_trip_and_exact_continuation() {
        let mut original = Machine::new(1234).unwrap();
        for _ in 0..40 {
            original.click().unwrap();
        }
        original.buy_generator(0).unwrap();
        original.trade_charm().unwrap();
        for _ in 0..9 {
            original.advance_tick().unwrap();
        }
        let bytes = encode(&original).unwrap();
        let mut loaded = decode(&bytes).unwrap();
        assert_eq!(original.snapshot(), loaded.snapshot());
        let gift_tick = original
            .memory
            .read_u64(original.object(cookie_game::field::EFFECTS).unwrap() + 16)
            .unwrap();
        for _ in original.tick..gift_tick {
            original.advance_tick().unwrap();
            loaded.advance_tick().unwrap();
        }
        original.claim_gift().unwrap();
        loaded.claim_gift().unwrap();
        original.click().unwrap();
        loaded.click().unwrap();
        assert_eq!(original.snapshot(), loaded.snapshot());
    }
    #[test]
    fn rejects_malformed_files() {
        let data = encode(&Machine::new(1).unwrap()).unwrap();
        for n in [0, 7, 35, data.len() - 1] {
            assert!(decode(&data[..n]).is_err());
        }
        let mut corrupt = data.clone();
        *corrupt.last_mut().unwrap() ^= 1;
        assert!(decode(&corrupt).is_err());
        let mut bad = data;
        bad[8..12].copy_from_slice(&99u32.to_le_bytes());
        assert!(matches!(
            decode(&bad),
            Err(SaveError::UnsupportedVersion(99))
        ));
    }
    #[test]
    fn explicit_file_does_not_affect_fresh_run() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("game.chls");
        let mut m = Machine::new(1).unwrap();
        m.click().unwrap();
        save(&file, &m).unwrap();
        assert_eq!(load(&file).unwrap().view().unwrap().cookies, 1);
        assert_eq!(Machine::new(1).unwrap().view().unwrap().cookies, 0);
        m.click().unwrap();
        save(&file, &m).unwrap();
        assert_eq!(load(&file).unwrap().view().unwrap().cookies, 2);
    }
    #[test]
    fn corrupted_guest_state_is_preserved() {
        let mut m = Machine::new(1).unwrap();
        let a = m.address(cookie_game::field::SHOP).unwrap();
        m.write_memory(a, &u64::MAX.to_le_bytes()).unwrap();
        let restored = decode(&encode(&m).unwrap()).unwrap();
        assert_eq!(restored.snapshot(), m.snapshot());
        assert!(restored.view().is_err());
    }
    #[test]
    fn compatibility_three_save_gains_shutdown_callbacks() {
        let mut legacy = Machine::new(3).unwrap();
        legacy
            .write_memory(vm_core::Architecture::GLOBALS + 24, &0u64.to_le_bytes())
            .unwrap();
        let mut bytes = encode(&legacy).unwrap();
        bytes[12..16].copy_from_slice(&3u32.to_le_bytes());
        let mut migrated = decode(&bytes).unwrap();
        let handlers = migrated.shutdown_callbacks_address().unwrap();
        assert_eq!(
            migrated.memory.read_ptr64(handlers).unwrap(),
            cookie_game::SHUTDOWN_AUDIT_VTABLE
        );
        assert_eq!(
            migrated.run_shutdown_callbacks().unwrap(),
            cookie_game::ShutdownOutcome::Close
        );
    }
    #[test]
    fn active_rush_and_recipes_resume_exactly() {
        let mut original = Machine::new(1234).unwrap();
        for _ in 0..40 {
            original.click().unwrap();
        }
        original.select_recipe(1).unwrap();
        let a = original.activities_address().unwrap();
        original
            .write_memory(
                a + cookie_game::activity_field::RUSH_STATE,
                &4u64.to_le_bytes(),
            )
            .unwrap();
        original
            .write_memory(
                a + cookie_game::activity_field::RUSH_DEADLINE,
                &600u64.to_le_bytes(),
            )
            .unwrap();
        original.start_rush().unwrap();
        for _ in 0..8 {
            original.click().unwrap();
        }
        for _ in 0..35 {
            original.advance_tick().unwrap();
        }
        let mut loaded = decode(&encode(&original).unwrap()).unwrap();
        assert_eq!(original.snapshot(), loaded.snapshot());
        for machine in [&mut original, &mut loaded] {
            for _ in 0..4 {
                machine.click().unwrap();
            }
            machine.claim_rush().unwrap();
            machine.select_recipe(2).unwrap();
            for _ in 0..250 {
                machine.advance_tick().unwrap();
            }
        }
        assert_eq!(original.snapshot(), loaded.snapshot());
        assert_eq!(loaded.activities_view().unwrap().rush_wins, 1);
    }
    #[test]
    fn legacy_save_adds_activities_without_changing_existing_state() {
        let mut legacy = Machine::new(99).unwrap();
        for _ in 0..45 {
            legacy.click().unwrap();
        }
        legacy.buy_generator(0).unwrap();
        legacy.trade_charm().unwrap();
        for _ in 0..31 {
            legacy.advance_tick().unwrap();
        }
        let root = vm_core::Architecture::GLOBALS + 8;
        let a = legacy.activities_address().unwrap();
        // Recreate a version-one snapshot with no activities root or live activity objects.
        legacy.heap.free(a).unwrap();
        legacy.write_memory(root, &0u64.to_le_bytes()).unwrap();
        let mut bytes = encode(&legacy).unwrap();
        bytes[12..16].copy_from_slice(&1u32.to_le_bytes());
        let migrated = decode(&bytes).unwrap();
        assert_eq!(migrated.tick, legacy.tick);
        assert_eq!(migrated.rng_state, legacy.rng_state);
        assert_eq!(migrated.player().unwrap(), legacy.player().unwrap());
        assert_eq!(migrated.view().unwrap().cookies, 31);
        for allocation in legacy.heap.allocations().iter().filter(|a| a.live) {
            assert_eq!(
                migrated
                    .memory
                    .read(allocation.base, allocation.size)
                    .unwrap(),
                legacy
                    .memory
                    .read(allocation.base, allocation.size)
                    .unwrap()
            );
        }
        assert_eq!(migrated.activities_view().unwrap().recipe, 0);
        assert_eq!(
            decode(&encode(&migrated).unwrap()).unwrap().snapshot(),
            migrated.snapshot()
        );
        // Current snapshots preserve an intentionally corrupted root instead of repairing it.
        let restored = decode(&encode(&legacy).unwrap()).unwrap();
        assert_eq!(restored.snapshot(), legacy.snapshot());
        assert!(restored.view().is_err());
    }
}
