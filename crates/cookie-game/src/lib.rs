//! Workshop game state stored entirely in checked, simulated memory.
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use thiserror::Error;
use vm_core::{
    Address, Architecture, ExecutionBackend, Memory, Permissions, RegionKind, WriteSource,
};
use vm_heap::Heap;

pub const TICKS_PER_SECOND: u64 = 20;
pub const FLAG_PRICE: u64 = 1_000_000_000_000;
pub const CURSOR_PRICE: u64 = 10;
pub const MAX_CURSORS: usize = 32;
pub mod globals {
    pub const COOKIES: u64 = 0x00;
    pub const USER: u64 = 0x08;
    pub const CURSORS: u64 = 0x10;
    pub const CURSOR_COUNT: u64 = 0x18;
    pub const MAX_FIELD_SIZE: u64 = 0x20;
    pub const FUNCTIONS: u64 = 0x28;
    pub const WON: u64 = 0x30;
    pub const CURSOR_TABLE: u64 = 0x100;
}
pub mod user_field {
    pub const ID: u64 = 0;
    pub const USERNAME: u64 = 8;
}
pub mod cursor_field {
    pub const ID: u64 = 0;
    pub const NAME: u64 = 8;
    pub const DESCRIPTION: u64 = 16;
    pub const ON_CLICKED: u64 = 24;
}
pub const FN_ADD_COOKIE: Address = Architecture::CODE + 0x100;
pub const FN_WIN: Address = Architecture::CODE + 0x180;
pub const FUNCTION_TABLE: Address = Architecture::GLOBALS + 0x300;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GameError {
    #[error("segmentation fault at 0x{0:016X}; the operation was reverted")]
    SegmentationFault(Address),
    #[error("not enough cookies")]
    NotEnoughCookies,
    #[error("the cursor table is full")]
    CursorTableFull,
    #[error("field input exceeds the current maximum of {0} bytes")]
    FieldTooLong(u64),
    #[error("{0}")]
    Vm(String),
}
type Result<T> = std::result::Result<T, GameError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorView {
    pub address: Address,
    pub id: u64,
    pub name: String,
    pub description: String,
    pub description_ptr: Address,
    pub on_clicked: Address,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameView {
    pub cookies: u64,
    pub username: String,
    pub username_ptr: Address,
    pub cookies_ptr: Address,
    pub user_ptr: Address,
    pub cursor_table_ptr: Address,
    pub max_field_size: u64,
    pub cursors: Vec<CursorView>,
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionView {
    pub name: &'static str,
    pub address: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Machine {
    pub memory: Memory,
    pub heap: Heap,
    pub tick: u64,
    pub passive_remainder: u64,
    paused: bool,
}
impl Machine {
    pub fn new(_seed: u64) -> Result<Self> {
        let mut memory = Memory::default();
        memory
            .map_region(
                Architecture::CODE,
                0x1000,
                Permissions::RW,
                RegionKind::Code,
            )
            .map_err(vm)?;
        memory
            .map_region(
                Architecture::GLOBALS,
                0x1000,
                Permissions::RW,
                RegionKind::Globals,
            )
            .map_err(vm)?;
        memory
            .map_region(
                Architecture::HEAP,
                Architecture::HEAP_SIZE,
                Permissions::RW,
                RegionKind::Heap,
            )
            .map_err(vm)?;
        memory
            .write(
                FN_ADD_COOKIE,
                b"ADD_COOKIE\x01\x00\x00\x00\x00\x00",
                WriteSource::Initialization,
                0,
                "add_cookie bytecode",
            )
            .map_err(vm)?;
        memory
            .write(
                FN_WIN,
                b"WIN\x01\x00\x00\x00\x00",
                WriteSource::Initialization,
                0,
                "win bytecode",
            )
            .map_err(vm)?;
        memory
            .set_region_permissions(Architecture::CODE, Permissions::RX)
            .map_err(vm)?;
        let heap = Heap::new(Architecture::HEAP, Architecture::HEAP_SIZE)
            .map_err(|e| GameError::Vm(e.to_string()))?;
        let mut m = Self {
            memory,
            heap,
            tick: 0,
            passive_remainder: 0,
            paused: false,
        };
        m.initialize()?;
        Ok(m)
    }
    fn initialize(&mut self) -> Result<()> {
        // Deliberate adjacency: overflowing username reaches Cookies.cookies.
        let username = self.alloc(32, "char username[32]")?;
        let cookies = self.alloc(8, "struct Cookies")?;
        let user = self.alloc(16, "struct User")?;
        self.store(username, b"student\0", "initial username")?;
        self.store_u64(cookies, 0, "Cookies.cookies")?;
        self.store_u64(user + user_field::ID, 1, "User.id")?;
        self.store_u64(user + user_field::USERNAME, username, "User.username")?;
        for (off, value, label) in [
            (globals::COOKIES, cookies, "global Cookies*"),
            (globals::USER, user, "global User*"),
            (
                globals::CURSORS,
                Architecture::GLOBALS + globals::CURSOR_TABLE,
                "global Cursor**",
            ),
            (globals::CURSOR_COUNT, 0, "cursor count"),
            (globals::MAX_FIELD_SIZE, 32, "max_field_size"),
            (globals::FUNCTIONS, FUNCTION_TABLE, "function table"),
            (globals::WON, 0, "win state"),
        ] {
            self.store_u64(Architecture::GLOBALS + off, value, label)?;
        }
        self.store_u64(FUNCTION_TABLE, FN_ADD_COOKIE, "function add_cookie")?;
        self.store_u64(FUNCTION_TABLE + 8, FN_WIN, "function win")?;
        self.memory.clear_events();
        Ok(())
    }
    fn alloc(&mut self, size: u64, name: &str) -> Result<Address> {
        self.heap
            .allocate(size, 8, name)
            .map_err(|e| GameError::Vm(e.to_string()))
    }
    fn store(&mut self, a: Address, b: &[u8], op: &str) -> Result<()> {
        self.memory
            .write(a, b, WriteSource::Guest, self.tick, op)
            .map_err(vm)
    }
    fn store_u64(&mut self, a: Address, v: u64, op: &str) -> Result<()> {
        self.store(a, &v.to_le_bytes(), op)
    }
    fn load_u64(&self, a: Address) -> Result<u64> {
        self.memory
            .read_u64(a)
            .map_err(|_| GameError::SegmentationFault(a))
    }
    fn ptr(&self, off: u64) -> Result<Address> {
        self.load_u64(Architecture::GLOBALS + off)
    }
    fn at(base: Address, offset: u64) -> Result<Address> {
        base.checked_add(offset)
            .ok_or(GameError::SegmentationFault(base))
    }
    pub fn globals_address(&self) -> Address {
        Architecture::GLOBALS
    }
    pub fn cookies_address(&self) -> Result<Address> {
        self.ptr(globals::COOKIES)
    }
    pub fn user_address(&self) -> Result<Address> {
        self.ptr(globals::USER)
    }
    pub fn cursor_table_address(&self) -> Address {
        Architecture::GLOBALS + globals::CURSOR_TABLE
    }
    pub fn function_table_address(&self) -> Address {
        FUNCTION_TABLE
    }
    pub fn function_table(&self) -> Result<Vec<FunctionView>> {
        Ok(vec![
            FunctionView {
                name: "add_cookie",
                address: self.load_u64(FUNCTION_TABLE)?,
            },
            FunctionView {
                name: "win",
                address: self.load_u64(FUNCTION_TABLE + 8)?,
            },
        ])
    }
    pub fn view(&self) -> Result<GameView> {
        let cookies_ptr = self.cookies_address()?;
        let user_ptr = self.user_address()?;
        let username_ptr = self.load_u64(Self::at(user_ptr, user_field::USERNAME)?)?;
        let count = self
            .load_u64(Architecture::GLOBALS + globals::CURSOR_COUNT)?
            .min(MAX_CURSORS as u64);
        let mut cursors = Vec::new();
        for i in 0..count {
            let address = self.load_u64(self.cursor_table_address() + i * 8)?;
            let description_ptr = self.load_u64(Self::at(address, cursor_field::DESCRIPTION)?)?;
            cursors.push(CursorView {
                address,
                id: self.load_u64(Self::at(address, cursor_field::ID)?)?,
                name: self.read_string(Self::at(address, cursor_field::NAME)?, 8)?,
                description: self.read_string(description_ptr, 128)?,
                description_ptr,
                on_clicked: self.load_u64(Self::at(address, cursor_field::ON_CLICKED)?)?,
            });
        }
        Ok(GameView {
            cookies: self.load_u64(cookies_ptr)?,
            username: self.read_string(username_ptr, 128)?,
            username_ptr,
            cookies_ptr,
            user_ptr,
            cursor_table_ptr: self.cursor_table_address(),
            max_field_size: self.load_u64(Architecture::GLOBALS + globals::MAX_FIELD_SIZE)?,
            cursors,
            won: self.load_u64(Architecture::GLOBALS + globals::WON)? != 0,
        })
    }
    fn read_string(&self, a: Address, max: u64) -> Result<String> {
        let bytes = self
            .memory
            .read(a, max)
            .map_err(|_| GameError::SegmentationFault(a))?;
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        Ok(String::from_utf8_lossy(&bytes[..end]).into_owned())
    }
    pub fn click(&mut self) -> Result<()> {
        let a = self.cookies_address()?;
        self.store_u64(a, self.load_u64(a)?.saturating_add(1), "click cookie")
    }
    pub fn buy_flag(&mut self) -> Result<()> {
        let a = self.cookies_address()?;
        let n = self.load_u64(a)?;
        if n < FLAG_PRICE {
            return Err(GameError::NotEnoughCookies);
        }
        self.store_u64(a, n - FLAG_PRICE, "buy flag")?;
        self.store_u64(Architecture::GLOBALS + globals::WON, 1, "flag purchased")
    }
    pub fn buy_cursor(&mut self) -> Result<()> {
        let ca = self.cookies_address()?;
        let cookies = self.load_u64(ca)?;
        if cookies < CURSOR_PRICE {
            return Err(GameError::NotEnoughCookies);
        }
        let count = self.load_u64(Architecture::GLOBALS + globals::CURSOR_COUNT)? as usize;
        if count >= MAX_CURSORS {
            return Err(GameError::CursorTableFull);
        }
        let description = self.alloc(64, "char Cursor.description[64]")?;
        let cursor = self.alloc(32, "struct Cursor")?;
        self.store(
            description,
            b"Produces one cookie per second.\0",
            "Cursor.description",
        )?;
        self.store_u64(cursor + cursor_field::ID, count as u64 + 1, "Cursor.id")?;
        let mut name = [0; 8];
        name[..6].copy_from_slice(b"Cursor");
        self.store(cursor + cursor_field::NAME, &name, "Cursor.name")?;
        self.store_u64(
            cursor + cursor_field::DESCRIPTION,
            description,
            "Cursor.description*",
        )?;
        self.store_u64(
            cursor + cursor_field::ON_CLICKED,
            FN_ADD_COOKIE,
            "Cursor.on_clicked",
        )?;
        self.store_u64(
            self.cursor_table_address() + count as u64 * 8,
            cursor,
            "Cursor* table entry",
        )?;
        self.store_u64(
            Architecture::GLOBALS + globals::CURSOR_COUNT,
            count as u64 + 1,
            "cursor count",
        )?;
        self.store_u64(ca, cookies - CURSOR_PRICE, "buy cursor")
    }
    pub fn rename_user(&mut self, bytes: &[u8]) -> Result<()> {
        let max = self.load_u64(Architecture::GLOBALS + globals::MAX_FIELD_SIZE)?;
        if bytes.len() as u64 + 1 > max {
            return Err(GameError::FieldTooLong(max));
        }
        let target = self.load_u64(Self::at(self.user_address()?, user_field::USERNAME)?)?;
        let mut input = bytes.to_vec();
        input.push(0);
        self.store(target, &input, "rename username (vulnerable copy)")
    }
    pub fn rename_cursor(&mut self, i: usize, bytes: &[u8]) -> Result<()> {
        let a = self.cursor_address(i)?;
        let mut v = bytes[..bytes.len().min(4095)].to_vec();
        v.push(0);
        self.store(
            Self::at(a, cursor_field::NAME)?,
            &v,
            "rename Cursor.name (vulnerable copy)",
        )
    }
    pub fn describe_cursor(&mut self, i: usize, bytes: &[u8]) -> Result<()> {
        let a = self.cursor_address(i)?;
        let target = self.load_u64(Self::at(a, cursor_field::DESCRIPTION)?)?;
        let mut v = bytes[..bytes.len().min(4095)].to_vec();
        v.push(0);
        self.store(target, &v, "edit Cursor.description through pointer")
    }
    pub fn invoke_cursor(&mut self, i: usize) -> Result<()> {
        let before = self.clone();
        let a = self.cursor_address(i)?;
        let f = self.load_u64(Self::at(a, cursor_field::ON_CLICKED)?)?;
        let result = match f {
            FN_ADD_COOKIE => self.click(),
            FN_WIN => self.store_u64(Architecture::GLOBALS + globals::WON, 1, "win() called"),
            bad => Err(GameError::SegmentationFault(bad)),
        };
        if result.is_err() {
            *self = before;
        }
        result
    }
    fn cursor_address(&self, i: usize) -> Result<Address> {
        let n = self.load_u64(Architecture::GLOBALS + globals::CURSOR_COUNT)? as usize;
        if i >= n || i >= MAX_CURSORS {
            return Err(GameError::Vm("cursor does not exist".into()));
        }
        self.load_u64(self.cursor_table_address() + i as u64 * 8)
    }
    pub fn advance_tick(&mut self) -> Result<()> {
        self.tick = self.tick.saturating_add(1);
        let n = self.load_u64(Architecture::GLOBALS + globals::CURSOR_COUNT)?;
        self.passive_remainder = self.passive_remainder.saturating_add(n);
        let gain = self.passive_remainder / TICKS_PER_SECOND;
        self.passive_remainder %= TICKS_PER_SECOND;
        if gain > 0 {
            let a = self.cookies_address()?;
            self.store_u64(
                a,
                self.load_u64(a)?.saturating_add(gain),
                "cursor production",
            )?;
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        self.memory.validate().map_err(vm)?;
        self.heap
            .validate()
            .map_err(|e| GameError::Vm(e.to_string()))?;
        Ok(())
    }
}
fn vm(e: vm_core::Fault) -> GameError {
    GameError::Vm(e.to_string())
}
impl ExecutionBackend for Machine {
    type Snapshot = Self;
    fn reset(&mut self, seed: u64) -> std::result::Result<(), String> {
        *self = Self::new(seed).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn step(&mut self) -> std::result::Result<(), String> {
        self.advance_tick().map_err(|e| e.to_string())
    }
    fn set_paused(&mut self, p: bool) {
        self.paused = p
    }
    fn is_paused(&self) -> bool {
        self.paused
    }
    fn read_memory(&self, a: Address, n: u64) -> vm_core::Result<Vec<u8>> {
        self.memory.read(a, n)
    }
    fn write_memory(&mut self, a: Address, b: &[u8]) -> std::result::Result<(), String> {
        self.memory
            .write(a, b, WriteSource::Debugger, self.tick, "debugger write")
            .map_err(|e| e.to_string())
    }
    fn memory_regions(&self) -> &[vm_core::Region] {
        self.memory.regions()
    }
    fn allocations(&self) -> &[vm_core::Allocation] {
        self.heap.allocations()
    }
    fn snapshot(&self) -> Self {
        let mut s = self.clone();
        s.memory.clear_events();
        s
    }
    fn restore(&mut self, s: Self) -> std::result::Result<(), String> {
        s.validate().map_err(|e| e.to_string())?;
        *self = s;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic_game_and_live_memory() {
        let mut m = Machine::new(1).unwrap();
        for _ in 0..10 {
            m.click().unwrap();
        }
        m.buy_cursor().unwrap();
        for _ in 0..20 {
            m.advance_tick().unwrap();
        }
        assert_eq!(m.view().unwrap().cookies, 1);
        let a = m.cookies_address().unwrap();
        m.write_memory(a, &FLAG_PRICE.to_le_bytes()).unwrap();
        assert_eq!(m.view().unwrap().cookies, FLAG_PRICE);
    }
    #[test]
    fn username_overflow_reaches_cookies() {
        let mut m = Machine::new(1).unwrap();
        m.write_memory(
            Architecture::GLOBALS + globals::MAX_FIELD_SIZE,
            &64u64.to_le_bytes(),
        )
        .unwrap();
        let mut p = vec![b'A'; 32];
        p.extend(FLAG_PRICE.to_le_bytes());
        m.rename_user(&p).unwrap();
        assert_eq!(m.view().unwrap().cookies, FLAG_PRICE);
    }
    #[test]
    fn function_pointer_calls_win() {
        let mut m = Machine::new(1).unwrap();
        for _ in 0..10 {
            m.click().unwrap();
        }
        m.buy_cursor().unwrap();
        let c = m.view().unwrap().cursors[0].address;
        m.write_memory(c + cursor_field::ON_CLICKED, &FN_WIN.to_le_bytes())
            .unwrap();
        m.invoke_cursor(0).unwrap();
        assert!(m.view().unwrap().won);
    }
    #[test]
    fn function_table_lists_identifiers_and_addresses() {
        let m = Machine::new(1).unwrap();
        assert_eq!(
            m.function_table(),
            Ok(vec![
                FunctionView {
                    name: "add_cookie",
                    address: FN_ADD_COOKIE,
                },
                FunctionView {
                    name: "win",
                    address: FN_WIN,
                },
            ])
        );
    }
    #[test]
    fn intended_cursor_overflow_calls_win() {
        let mut m = Machine::new(1).unwrap();
        for _ in 0..10 {
            m.click().unwrap();
        }
        m.buy_cursor().unwrap();
        let cursor = m.view().unwrap().cursors[0].address;
        let mut overflow = b"12345678".to_vec();
        overflow.extend((cursor + cursor_field::ON_CLICKED).to_le_bytes());
        m.rename_cursor(0, &overflow).unwrap();
        m.describe_cursor(0, &FN_WIN.to_le_bytes()).unwrap();
        m.invoke_cursor(0).unwrap();
        assert!(m.view().unwrap().won);
    }
}
