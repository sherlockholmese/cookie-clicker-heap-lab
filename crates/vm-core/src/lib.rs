//! Safe, sparse 64-bit guest memory. Guest addresses are never host pointers.
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use thiserror::Error;

pub type Address = u64;
pub const POINTER_BYTES: usize = 8;
pub const MAX_MEMORY: u64 = 16 * 1024 * 1024;
pub const MAX_EVENTS: usize = 512;

pub struct Architecture;
impl Architecture {
    pub const CODE: Address = 0x0040_0000;
    pub const RODATA: Address = 0x0100_0000;
    pub const GLOBALS: Address = 0x0200_0000;
    pub const HEAP: Address = 0x0000_5555_0000_0000;
    pub const STACK_TOP: Address = 0x0000_7FFF_FFFF_F000;
    pub const HEAP_SIZE: u64 = 64 * 1024;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Fault {
    #[error("guest address arithmetic overflow")]
    AddressOverflow,
    #[error("unmapped guest memory at 0x{0:016X}")]
    Unmapped(Address),
    #[error("guest {access} permission denied at 0x{address:016X}")]
    Permission {
        address: Address,
        access: &'static str,
    },
    #[error("invalid memory map: {0}")]
    InvalidMap(String),
    #[error("guest transfer exceeds the memory limit")]
    TooLarge,
    #[error("invalid snapshot: {0}")]
    InvalidSnapshot(String),
}
pub type Result<T> = std::result::Result<T, Fault>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Permissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}
impl Permissions {
    pub const RW: Self = Self {
        read: true,
        write: true,
        execute: false,
    };
    pub const R: Self = Self {
        read: true,
        write: false,
        execute: false,
    };
    pub const RX: Self = Self {
        read: true,
        write: false,
        execute: true,
    };
    pub fn label(self) -> String {
        format!(
            "{}{}{}",
            if self.read { 'R' } else { '-' },
            if self.write { 'W' } else { '-' },
            if self.execute { 'X' } else { '-' }
        )
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RegionKind {
    Code,
    ReadOnly,
    Globals,
    Heap,
    Stack,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Region {
    base: Address,
    permissions: Permissions,
    kind: RegionKind,
    bytes: Vec<u8>,
}
impl Region {
    pub fn base(&self) -> Address {
        self.base
    }
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
    pub fn end(&self) -> Address {
        self.base + self.len()
    }
    pub fn permissions(&self) -> Permissions {
        self.permissions
    }
    pub fn kind(&self) -> RegionKind {
        self.kind
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn contains(&self, address: Address) -> bool {
        address >= self.base && address < self.end()
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WriteSource {
    Guest,
    Debugger,
    Restore,
    Initialization,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteEvent {
    pub tick: u64,
    pub operation: String,
    pub source: WriteSource,
    pub address: Address,
    pub old: Vec<u8>,
    pub new: Vec<u8>,
    pub origin: Option<u64>,
    pub affected: Vec<u64>,
    pub overflow: u64,
    pub semantic_boundary: Option<Address>,
    pub semantic_overflow: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Memory {
    regions: Vec<Region>,
    #[serde(skip)]
    events: VecDeque<WriteEvent>,
}
impl Memory {
    pub fn regions(&self) -> &[Region] {
        &self.regions
    }
    pub fn events(&self) -> &VecDeque<WriteEvent> {
        &self.events
    }
    pub fn clear_events(&mut self) {
        self.events.clear();
    }
    pub fn region_for(&self, address: Address) -> Option<&Region> {
        self.regions.iter().find(|r| r.contains(address))
    }
    pub fn map_region(
        &mut self,
        base: Address,
        length: u64,
        permissions: Permissions,
        kind: RegionKind,
    ) -> Result<()> {
        let end = base.checked_add(length).ok_or(Fault::AddressOverflow)?;
        if base == 0 || length == 0 {
            return Err(Fault::InvalidMap("empty region or null address".into()));
        }
        let total = self
            .regions
            .iter()
            .map(Region::len)
            .sum::<u64>()
            .checked_add(length)
            .ok_or(Fault::TooLarge)?;
        if total > MAX_MEMORY {
            return Err(Fault::TooLarge);
        }
        if self.regions.iter().any(|r| base < r.end() && end > r.base) {
            return Err(Fault::InvalidMap("overlapping regions".into()));
        }
        let size = usize::try_from(length).map_err(|_| Fault::TooLarge)?;
        self.regions.push(Region {
            base,
            permissions,
            kind,
            bytes: vec![0; size],
        });
        self.regions.sort_by_key(|r| r.base);
        Ok(())
    }
    pub fn unmap_region(&mut self, base: Address) -> Result<()> {
        let i = self
            .regions
            .iter()
            .position(|r| r.base == base)
            .ok_or(Fault::Unmapped(base))?;
        self.regions.remove(i);
        Ok(())
    }
    pub fn set_region_permissions(
        &mut self,
        base: Address,
        permissions: Permissions,
    ) -> Result<()> {
        let region = self
            .regions
            .iter_mut()
            .find(|region| region.base == base)
            .ok_or(Fault::Unmapped(base))?;
        region.permissions = permissions;
        Ok(())
    }
    // Validate the entire transfer before mutating: faults never leave a partial write.
    fn spans(
        &self,
        address: Address,
        length: u64,
        write: bool,
    ) -> Result<Vec<(usize, usize, usize)>> {
        address.checked_add(length).ok_or(Fault::AddressOverflow)?;
        if length > MAX_MEMORY {
            return Err(Fault::TooLarge);
        }
        let mut cursor = address;
        let mut left = length;
        let mut spans = Vec::new();
        while left > 0 {
            let (i, r) = self
                .regions
                .iter()
                .enumerate()
                .find(|(_, r)| r.contains(cursor))
                .ok_or(Fault::Unmapped(cursor))?;
            if (write && !r.permissions.write) || (!write && !r.permissions.read) {
                return Err(Fault::Permission {
                    address: cursor,
                    access: if write { "write" } else { "read" },
                });
            }
            let n = left.min(r.end() - cursor);
            spans.push((
                i,
                usize::try_from(cursor - r.base).map_err(|_| Fault::TooLarge)?,
                usize::try_from(n).map_err(|_| Fault::TooLarge)?,
            ));
            cursor += n;
            left -= n;
        }
        Ok(spans)
    }
    pub fn read(&self, address: Address, length: u64) -> Result<Vec<u8>> {
        let spans = self.spans(address, length, false)?;
        let mut bytes = Vec::with_capacity(length as usize);
        for (i, off, n) in spans {
            bytes.extend_from_slice(&self.regions[i].bytes[off..off + n]);
        }
        Ok(bytes)
    }
    pub fn write(
        &mut self,
        address: Address,
        bytes: &[u8],
        source: WriteSource,
        tick: u64,
        operation: &str,
    ) -> Result<()> {
        let spans = self.spans(address, bytes.len() as u64, true)?;
        let mut old = Vec::with_capacity(bytes.len());
        let mut cursor = 0;
        for (i, off, n) in spans {
            let dest = &mut self.regions[i].bytes[off..off + n];
            old.extend_from_slice(dest);
            dest.copy_from_slice(&bytes[cursor..cursor + n]);
            cursor += n;
        }
        if !bytes.is_empty() && old != bytes {
            if self.events.len() == MAX_EVENTS {
                self.events.pop_front();
            }
            self.events.push_back(WriteEvent {
                tick,
                operation: operation.into(),
                source,
                address,
                old,
                new: bytes.to_vec(),
                origin: None,
                affected: vec![],
                overflow: 0,
                semantic_boundary: None,
                semantic_overflow: 0,
            });
        }
        Ok(())
    }
    pub fn annotate_last_write(
        &mut self,
        address: Address,
        origin: Option<u64>,
        affected: Vec<u64>,
        overflow: u64,
    ) {
        if let Some(e) = self.events.back_mut().filter(|e| e.address == address) {
            e.origin = origin;
            e.affected = affected;
            e.overflow = overflow;
        }
    }
    pub fn annotate_semantic_boundary(&mut self, boundary: Address) {
        if let Some(e) = self.events.back_mut() {
            e.semantic_boundary = Some(boundary);
            e.semantic_overflow = (e.address + e.new.len() as u64).saturating_sub(boundary);
        }
    }
    pub fn read_array<const N: usize>(&self, address: Address) -> Result<[u8; N]> {
        self.read(address, N as u64)?
            .try_into()
            .map_err(|_| Fault::TooLarge)
    }
    pub fn read_u8(&self, a: Address) -> Result<u8> {
        Ok(self.read_array::<1>(a)?[0])
    }
    pub fn read_u16(&self, a: Address) -> Result<u16> {
        Ok(u16::from_le_bytes(self.read_array(a)?))
    }
    pub fn read_u32(&self, a: Address) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read_array(a)?))
    }
    pub fn read_u64(&self, a: Address) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read_array(a)?))
    }
    pub fn read_ptr64(&self, a: Address) -> Result<Address> {
        self.read_u64(a)
    }
    pub fn write_u8(&mut self, a: Address, v: u8, s: WriteSource) -> Result<()> {
        self.write(a, &v.to_le_bytes(), s, 0, "store_u8")
    }
    pub fn write_u16(&mut self, a: Address, v: u16, s: WriteSource) -> Result<()> {
        self.write(a, &v.to_le_bytes(), s, 0, "store_u16")
    }
    pub fn write_u32(&mut self, a: Address, v: u32, s: WriteSource) -> Result<()> {
        self.write(a, &v.to_le_bytes(), s, 0, "store_u32")
    }
    pub fn write_u64(&mut self, a: Address, v: u64, s: WriteSource) -> Result<()> {
        self.write(a, &v.to_le_bytes(), s, 0, "store_u64")
    }
    pub fn write_ptr64(&mut self, a: Address, v: Address, s: WriteSource) -> Result<()> {
        self.write_u64(a, v, s)
    }
    pub fn snapshot(&self) -> Self {
        let mut s = self.clone();
        s.clear_events();
        s
    }
    pub fn restore(&mut self, snapshot: Self) -> Result<()> {
        snapshot.validate()?;
        *self = snapshot;
        self.clear_events();
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        let mut end = 0;
        let mut total = 0u64;
        for r in &self.regions {
            if r.base == 0 || r.is_empty() || r.base < end {
                return Err(Fault::InvalidSnapshot("invalid/overlapping regions".into()));
            }
            end = r.base.checked_add(r.len()).ok_or(Fault::AddressOverflow)?;
            total = total.checked_add(r.len()).ok_or(Fault::TooLarge)?;
        }
        if total > MAX_MEMORY {
            return Err(Fault::TooLarge);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Allocation {
    pub id: u64,
    pub base: Address,
    pub size: u64,
    pub alignment: u64,
    pub live: bool,
    pub generation: u64,
    pub semantic_type: String,
}
impl Allocation {
    pub fn end(&self) -> Address {
        self.base + self.size
    }
    pub fn contains(&self, a: Address) -> bool {
        a >= self.base && a < self.end()
    }
}
/// An execution contract independent of the concrete educational machine.
pub trait ExecutionBackend {
    type Snapshot;
    fn reset(&mut self, seed: u64) -> std::result::Result<(), String>;
    fn step(&mut self) -> std::result::Result<(), String>;
    fn set_paused(&mut self, paused: bool);
    fn is_paused(&self) -> bool;
    fn read_memory(&self, address: Address, length: u64) -> Result<Vec<u8>>;
    fn write_memory(&mut self, address: Address, bytes: &[u8]) -> std::result::Result<(), String>;
    fn memory_regions(&self) -> &[Region];
    fn allocations(&self) -> &[Allocation];
    fn snapshot(&self) -> Self::Snapshot;
    fn restore(&mut self, snapshot: Self::Snapshot) -> std::result::Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mem() -> Memory {
        let mut m = Memory::default();
        m.map_region(Architecture::HEAP, 8192, Permissions::RW, RegionKind::Heap)
            .unwrap();
        m
    }
    #[test]
    fn pointer_and_little_endian() {
        let mut m = mem();
        let p = Architecture::HEAP + 0x3120;
        m.write_ptr64(Architecture::HEAP, p, WriteSource::Debugger)
            .unwrap();
        assert_eq!(m.read(Architecture::HEAP, 8).unwrap(), p.to_le_bytes());
        assert_eq!(m.read_ptr64(Architecture::HEAP).unwrap(), p);
        assert!(matches!(m.read(p, 8), Err(Fault::Unmapped(_))));
        let p = Architecture::HEAP + 0x120;
        m.write_ptr64(Architecture::HEAP, p, WriteSource::Guest)
            .unwrap();
        m.write_u64(p, 999, WriteSource::Guest).unwrap();
        assert_eq!(
            m.read_u64(m.read_ptr64(Architecture::HEAP).unwrap())
                .unwrap(),
            999
        );
    }
    #[test]
    fn boundaries_and_atomic_faults() {
        let mut m = mem();
        let a = Architecture::HEAP + 4093;
        m.write_u64(a, u64::MAX, WriteSource::Guest).unwrap();
        assert_eq!(m.read_u64(a).unwrap(), u64::MAX);
        let before = m.clone();
        assert!(
            m.write(
                Architecture::HEAP + 8190,
                &[1; 4],
                WriteSource::Guest,
                0,
                "test"
            )
            .is_err()
        );
        assert_eq!(m, before);
        assert_eq!(m.read(u64::MAX, 2), Err(Fault::AddressOverflow));
        m.map_region(100, 8, Permissions::R, RegionKind::ReadOnly)
            .unwrap();
        assert!(matches!(
            m.write_u64(100, 1, WriteSource::Guest),
            Err(Fault::Permission { .. })
        ));
    }
    #[test]
    fn adjacent_regions() {
        let mut m = Memory::default();
        for a in [100, 104] {
            m.map_region(a, 4, Permissions::RW, RegionKind::Heap)
                .unwrap();
        }
        m.write_u64(100, 0x1122334455667788, WriteSource::Guest)
            .unwrap();
        assert_eq!(m.read_u64(100).unwrap(), 0x1122334455667788);
        assert!(
            m.map_region(103, 8, Permissions::RW, RegionKind::Heap)
                .is_err()
        );
    }
}
