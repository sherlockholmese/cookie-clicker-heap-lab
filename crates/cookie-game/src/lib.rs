//! The UI is a projection of this guest machine, never a second game state.
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use thiserror::Error;
use vm_core::{
    Address, Architecture, ExecutionBackend, Fault, Memory, Permissions, RegionKind, WriteSource,
};
use vm_heap::{Heap, HeapError};
mod activities;
mod encounters;
pub use activities::{ActivitiesView, RECIPES, RecipeSpec, RushState, activity_field};
pub use encounters::{EncounterView, encounter_field};

pub const TICKS_PER_SECOND: u64 = 20;
pub const PLAYER_SIZE: u64 = 136;
pub const GENERATOR_SIZE: u64 = 32;
pub const NAME_SIZE: u64 = 32;
pub const SHUTDOWN_CALLBACKS_SIZE: u64 = 32;
pub const SHUTDOWN_AUDIT_VTABLE: Address = Architecture::RODATA + 0x100;
pub const SHUTDOWN_OVEN_VTABLE: Address = Architecture::RODATA + 0x108;
pub const SHUTDOWN_AUDIT_FUNCTION: Address = Architecture::CODE + 0x100;
pub const SHUTDOWN_OVEN_FUNCTION: Address = Architecture::CODE + 0x108;
pub const GUEST_SHELL_FUNCTION: Address = Architecture::CODE + 0x180;
pub mod field {
    pub const COOKIES: u64 = 0;
    pub const LIFETIME: u64 = 8;
    pub const CLICK: u64 = 16;
    pub const CPS: u64 = 24;
    pub const PRESTIGE: u64 = 32;
    pub const FLAGS: u64 = 40;
    pub const NAME: u64 = 48;
    pub const GENERATORS: u64 = 56;
    pub const UPGRADES: u64 = 64;
    pub const ACHIEVEMENTS: u64 = 72;
    pub const GENERATOR_COUNT: u64 = 80;
    pub const UPGRADE_COUNT: u64 = 88;
    pub const ACHIEVEMENT_COUNT: u64 = 96;
    pub const STATS: u64 = 104;
    pub const EFFECTS: u64 = 112;
    pub const SHOP: u64 = 120;
    pub const PRESTIGE_STATE: u64 = 128;
}
#[derive(Debug, Error)]
pub enum GameError {
    #[error(transparent)]
    Memory(#[from] Fault),
    #[error(transparent)]
    Heap(#[from] HeapError),
    #[error("not enough cookies")]
    Unaffordable,
    #[error("this upgrade is already owned or locked")]
    Locked,
    #[error("invalid guest state: {0}")]
    InvalidState(String),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownOutcome {
    Close,
    GuestShell,
}
pub type Result<T> = std::result::Result<T, GameError>;
#[derive(Debug, Clone, Copy)]
pub struct GeneratorSpec {
    pub name: &'static str,
    pub price: u64,
    pub rate: u64,
}
pub const GENERATORS: [GeneratorSpec; 9] = [
    GeneratorSpec {
        name: "Cursor",
        price: 15,
        rate: 1,
    },
    GeneratorSpec {
        name: "Grandma",
        price: 100,
        rate: 5,
    },
    GeneratorSpec {
        name: "Bakery",
        price: 1_100,
        rate: 40,
    },
    GeneratorSpec {
        name: "Factory",
        price: 12_000,
        rate: 250,
    },
    GeneratorSpec {
        name: "Mine",
        price: 130_000,
        rate: 1_500,
    },
    GeneratorSpec {
        name: "Laboratory",
        price: 1_400_000,
        rate: 9_000,
    },
    GeneratorSpec {
        name: "Portal Oven",
        price: 20_000_000,
        rate: 60_000,
    },
    GeneratorSpec {
        name: "Quantum Bakery",
        price: 330_000_000,
        rate: 400_000,
    },
    GeneratorSpec {
        name: "Cookie Singularity",
        price: 5_100_000_000,
        rate: 3_000_000,
    },
];
#[derive(Debug, Clone, Copy)]
pub struct UpgradeSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub price: u64,
    pub unlock: u64,
}
pub const UPGRADES: [UpgradeSpec; 8] = [
    UpgradeSpec {
        name: "Better dough",
        description: "Double cookies per click",
        price: 100,
        unlock: 50,
    },
    UpgradeSpec {
        name: "Warm ovens",
        description: "Double all generator production",
        price: 500,
        unlock: 250,
    },
    UpgradeSpec {
        name: "Chocolate chips",
        description: "Double cookies per click",
        price: 2_000,
        unlock: 1_000,
    },
    UpgradeSpec {
        name: "Night shift",
        description: "Double all generator production",
        price: 10_000,
        unlock: 5_000,
    },
    UpgradeSpec {
        name: "Golden whisk",
        description: "Double cookies per click",
        price: 100_000,
        unlock: 50_000,
    },
    UpgradeSpec {
        name: "Assembly line",
        description: "Double all generator production",
        price: 1_000_000,
        unlock: 500_000,
    },
    UpgradeSpec {
        name: "Perfect recipe",
        description: "Double cookies per click",
        price: 10_000_000,
        unlock: 5_000_000,
    },
    UpgradeSpec {
        name: "Infinite ovens",
        description: "Double all generator production",
        price: 100_000_000,
        unlock: 50_000_000,
    },
];
pub const ACHIEVEMENTS: [(&str, u64); 8] = [
    ("First batch", 1),
    ("A hundred crumbs", 100),
    ("Local favorite", 1_000),
    ("Cookie entrepreneur", 10_000),
    ("Baking empire", 100_000),
    ("Millionaire baker", 1_000_000),
    ("Beyond the oven", 1_000_000_000),
    ("Universal bakery", 1_000_000_000_000),
];
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Machine {
    pub memory: Memory,
    pub heap: Heap,
    pub seed: u64,
    pub rng_state: u64,
    pub tick: u64,
    #[serde(skip)]
    paused: bool,
}
#[derive(Debug, Clone)]
pub struct GeneratorView {
    pub owned: u64,
    pub price: u64,
    pub production: u64,
    pub per_unit: u64,
}
#[derive(Debug, Clone)]
pub struct GameView {
    pub name: String,
    pub cookies: u64,
    pub lifetime: u64,
    pub click_power: u64,
    pub cps: u64,
    pub prestige: u64,
    pub clicks: u64,
    pub spent: u64,
    pub play_ticks: u64,
    pub generators: Vec<GeneratorView>,
    pub upgrades: Vec<bool>,
    pub achievements: Vec<bool>,
    pub boost_remaining: u64,
    pub gift_available: bool,
    pub prestige_gain: u64,
    pub recipe_multiplier: u64,
    pub charm_traded: bool,
    pub activities: ActivitiesView,
    pub encounter: EncounterView,
}
impl Machine {
    pub fn new(seed: u64) -> Result<Self> {
        let mut memory = Memory::default();
        memory.map_region(Architecture::CODE, 4096, Permissions::RX, RegionKind::Code)?;
        memory.map_region(
            Architecture::RODATA,
            4096,
            Permissions::R,
            RegionKind::ReadOnly,
        )?;
        memory.map_region(
            Architecture::GLOBALS,
            4096,
            Permissions::RW,
            RegionKind::Globals,
        )?;
        memory.map_region(
            Architecture::HEAP,
            Architecture::HEAP_SIZE,
            Permissions::RW,
            RegionKind::Heap,
        )?;
        memory.map_region(
            Architecture::STACK_TOP - 4096,
            4096,
            Permissions::RW,
            RegionKind::Stack,
        )?;
        let mut m = Self {
            memory,
            heap: Heap::new(Architecture::HEAP, Architecture::HEAP_SIZE)?,
            seed,
            rng_state: seed ^ 0x9E3779B97F4A7C15,
            tick: 0,
            paused: false,
        };
        m.initialize_guest_code()?;
        // Adjacency is intentional: a rename reaches the shutdown callbacks' vtable pointers.
        let name = m.heap.allocate(NAME_SIZE, 8, "PlayerName")?;
        m.initialize_shutdown_callbacks()?;
        let player = m.heap.allocate(PLAYER_SIZE, 8, "PlayerState")?;
        let generators =
            m.heap
                .allocate(GENERATOR_SIZE * GENERATORS.len() as u64, 8, "Generator[]")?;
        let upgrades = m.heap.allocate(UPGRADES.len() as u64 * 8, 8, "Upgrade[]")?;
        let achievements = m
            .heap
            .allocate(ACHIEVEMENTS.len() as u64 * 8, 8, "Achievement[]")?;
        let stats = m.heap.allocate(48, 8, "Statistics")?;
        let effects = m.heap.allocate(32, 8, "TemporaryEffects")?;
        let shop = m.heap.allocate(40, 8, "ShopState / Recipe")?;
        let prestige = m.heap.allocate(16, 8, "PrestigeState")?;
        let bonus = m.heap.allocate(8, 8, "RecipeBonus")?;
        let charm = m.heap.allocate(8, 8, "LuckyCharm")?;
        m.store_init(Architecture::GLOBALS, player)?;
        for (off, val) in [
            (field::CLICK, 1),
            (field::NAME, name),
            (field::GENERATORS, generators),
            (field::UPGRADES, upgrades),
            (field::ACHIEVEMENTS, achievements),
            (field::GENERATOR_COUNT, 9),
            (field::UPGRADE_COUNT, 8),
            (field::ACHIEVEMENT_COUNT, 8),
            (field::STATS, stats),
            (field::EFFECTS, effects),
            (field::SHOP, shop),
            (field::PRESTIGE_STATE, prestige),
        ] {
            m.store_init(player + off, val)?;
        }
        m.write(
            name,
            b"Crumb & Co.\0",
            WriteSource::Initialization,
            "initialize_name",
            Some(name),
        )?;
        m.write(
            shop,
            b"House recipe\0",
            WriteSource::Initialization,
            "initialize_recipe",
            Some(shop),
        )?;
        m.store_init(shop + 24, 1)?;
        m.store_init(shop + 32, bonus)?;
        m.store_init(bonus, 1)?;
        m.store_init(stats + 32, charm)?;
        m.store_init(charm, 1)?;
        for (i, spec) in GENERATORS.iter().enumerate() {
            let a = generators + i as u64 * GENERATOR_SIZE;
            for (off, v) in [(8, spec.price), (16, spec.rate), (24, 1)] {
                m.store_init(a + off, v)?;
            }
        }
        m.initialize_activities()?;
        m.initialize_encounters()?;
        m.memory.clear_events();
        Ok(m)
    }
    pub fn player(&self) -> Result<Address> {
        let a = self.memory.read_ptr64(Architecture::GLOBALS)?;
        self.memory.read(a, PLAYER_SIZE)?;
        Ok(a)
    }
    fn initialize_guest_code(&mut self) -> Result<()> {
        self.memory
            .set_region_permissions(Architecture::CODE, Permissions::RW)?;
        for (address, bytes) in [
            (
                SHUTDOWN_AUDIT_FUNCTION,
                &[0x55, 0x48, 0x89, 0xE5, 0x48, 0xFF, 0x47, 0x08, 0x5D, 0xC3][..],
            ),
            (
                SHUTDOWN_OVEN_FUNCTION,
                &[0x55, 0x48, 0x89, 0xE5, 0x48, 0xFF, 0x47, 0x08, 0x5D, 0xC3][..],
            ),
            (
                GUEST_SHELL_FUNCTION,
                &[
                    0x55, 0x48, 0x89, 0xE5, 0x48, 0x83, 0xEC, 0x20, 0x90, 0x90, 0xC9, 0xC3,
                ][..],
            ),
        ] {
            self.memory.write(
                address,
                bytes,
                WriteSource::Initialization,
                self.tick,
                "initialize_guest_code",
            )?;
        }
        self.memory
            .set_region_permissions(Architecture::CODE, Permissions::RX)?;
        Ok(())
    }
    pub fn initialize_shutdown_callbacks(&mut self) -> Result<()> {
        let root = Architecture::GLOBALS + 24;
        let existing = self.memory.read_ptr64(root)?;
        if existing != 0 {
            self.memory.read(existing, SHUTDOWN_CALLBACKS_SIZE)?;
            return Ok(());
        }

        // Populate immutable guest vtables during machine construction or save migration.
        self.memory
            .set_region_permissions(Architecture::RODATA, Permissions::RW)?;
        let vtables = [SHUTDOWN_AUDIT_FUNCTION, SHUTDOWN_OVEN_FUNCTION]
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect::<Vec<_>>();
        let write_result = self.memory.write(
            SHUTDOWN_AUDIT_VTABLE,
            &vtables,
            WriteSource::Initialization,
            self.tick,
            "initialize_shutdown_vtables",
        );
        self.memory
            .set_region_permissions(Architecture::RODATA, Permissions::R)?;
        write_result?;

        let handlers = self
            .heap
            .allocate(SHUTDOWN_CALLBACKS_SIZE, 8, "ShutdownCallback[]")?;
        self.store_init(root, handlers)?;
        self.store_init(handlers, SHUTDOWN_AUDIT_VTABLE)?;
        self.store_init(handlers + 16, SHUTDOWN_OVEN_VTABLE)?;
        Ok(())
    }
    pub fn shutdown_callbacks_address(&self) -> Result<Address> {
        let address = self.memory.read_ptr64(Architecture::GLOBALS + 24)?;
        self.memory.read(address, SHUTDOWN_CALLBACKS_SIZE)?;
        Ok(address)
    }
    pub fn run_shutdown_callbacks(&mut self) -> Result<ShutdownOutcome> {
        let handlers = self.shutdown_callbacks_address()?;
        for (offset, expected) in [(0, SHUTDOWN_AUDIT_FUNCTION), (16, SHUTDOWN_OVEN_FUNCTION)] {
            let object = handlers + offset;
            let vtable = self.memory.read_ptr64(object)?;
            let function = self.memory.read_ptr64(vtable)?;
            if function == GUEST_SHELL_FUNCTION {
                self.add_at(object + 8, 1, "invoke_shutdown_callback")?;
                return Ok(ShutdownOutcome::GuestShell);
            }
            if function != expected {
                return Err(GameError::InvalidState(format!(
                    "shutdown callback attempted unknown guest function 0x{function:016X}"
                )));
            }
            self.add_at(object + 8, 1, "invoke_shutdown_callback")?;
        }
        Ok(ShutdownOutcome::Close)
    }
    pub fn address(&self, offset: u64) -> Result<Address> {
        self.player()?
            .checked_add(offset)
            .ok_or(Fault::AddressOverflow.into())
    }
    pub fn value(&self, offset: u64) -> Result<u64> {
        Ok(self.memory.read_u64(self.address(offset)?)?)
    }
    pub fn object(&self, offset: u64) -> Result<Address> {
        let a = self.value(offset)?;
        let length = match offset {
            field::NAME => NAME_SIZE,
            field::GENERATORS => 9 * GENERATOR_SIZE,
            field::UPGRADES | field::ACHIEVEMENTS => 64,
            field::STATS => 48,
            field::EFFECTS => 32,
            field::SHOP => 40,
            field::PRESTIGE_STATE => 16,
            _ => 8,
        };
        self.memory.read(a, length)?;
        Ok(a)
    }
    fn store_init(&mut self, a: Address, v: u64) -> Result<()> {
        self.write(
            a,
            &v.to_le_bytes(),
            WriteSource::Initialization,
            "initialize",
            Some(a),
        )
    }
    pub fn write(
        &mut self,
        a: Address,
        bytes: &[u8],
        source: WriteSource,
        operation: &str,
        origin: Option<Address>,
    ) -> Result<()> {
        let (origin, affected, overflow) = self.heap.describe_write(a, bytes.len() as u64, origin);
        let changed = self.memory.read(a, bytes.len() as u64)? != bytes;
        self.memory.write(a, bytes, source, self.tick, operation)?;
        if changed {
            self.memory
                .annotate_last_write(a, origin, affected, overflow);
            let limit = match operation {
                "rename_player" => Some(NAME_SIZE),
                "label_recipe" => Some(24),
                _ => None,
            };
            if let Some(limit) = limit {
                self.memory.annotate_semantic_boundary(
                    a.checked_add(limit).ok_or(Fault::AddressOverflow)?,
                );
            }
        }
        Ok(())
    }
    fn store(&mut self, a: Address, v: u64, operation: &str) -> Result<()> {
        self.write(a, &v.to_le_bytes(), WriteSource::Guest, operation, Some(a))
    }
    fn set(&mut self, offset: u64, v: u64, op: &str) -> Result<()> {
        self.store(self.address(offset)?, v, op)
    }
    fn add_at(&mut self, a: Address, v: u64, op: &str) -> Result<()> {
        self.store(a, self.memory.read_u64(a)?.saturating_add(v), op)
    }
    fn check_table(&self, field: u64, count: u64, stride: u64) -> Result<Address> {
        let a = self.object(field)?;
        self.memory
            .read(a, count.checked_mul(stride).ok_or(Fault::AddressOverflow)?)?;
        Ok(a)
    }
    pub fn click_power(&self) -> Result<u64> {
        let shop = self.object(field::SHOP)?;
        let stats = self.object(field::STATS)?;
        let effects = self.object(field::EFFECTS)?;
        let recipe = self.memory.read_u64(shop + 24)?;
        let bonus = self.memory.read_u64(self.memory.read_ptr64(shop + 32)?)?;
        let charm = self.memory.read_u64(self.memory.read_ptr64(stats + 32)?)?;
        let boost = if self.memory.read_u64(effects)? > self.tick {
            self.memory.read_u64(effects + 8)?
        } else {
            1
        };
        Ok(self
            .value(field::CLICK)?
            .saturating_mul(recipe)
            .saturating_mul(bonus)
            .saturating_mul(charm)
            .saturating_mul(self.value(field::PRESTIGE)?.saturating_add(1))
            .saturating_mul(boost)
            .saturating_mul(self.recipe_click_multiplier()?))
    }
    fn award(&mut self, amount: u64, op: &str) -> Result<()> {
        self.set(
            field::COOKIES,
            self.value(field::COOKIES)?.saturating_add(amount),
            op,
        )?;
        self.set(
            field::LIFETIME,
            self.value(field::LIFETIME)?.saturating_add(amount),
            op,
        )?;
        self.update_achievements()
    }
    pub fn click(&mut self) -> Result<()> {
        let amount = self.click_power()?;
        let stats = self.object(field::STATS)?;
        self.award(amount, "click_cookie")?;
        self.add_at(stats, 1, "count_click")?;
        self.record_rush_click()
    }
    pub fn rename(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > 4096 {
            return Err(GameError::InvalidState(
                "name payload exceeds 4096 bytes".into(),
            ));
        }
        let a = self.object(field::NAME)?;
        let mut payload = bytes.to_vec();
        payload.push(0);
        // Deliberately no allocation-size check. Region checks remain enforced.
        self.write(a, &payload, WriteSource::Guest, "rename_player", Some(a))
    }
    pub fn label_recipe(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > 4096 {
            return Err(GameError::InvalidState(
                "recipe payload exceeds 4096 bytes".into(),
            ));
        }
        let a = self.object(field::SHOP)?;
        let mut payload = bytes.to_vec();
        payload.push(0);
        self.write(a, &payload, WriteSource::Guest, "label_recipe", Some(a))
    }
    fn spend(&mut self, price: u64) -> Result<()> {
        let cookies = self.value(field::COOKIES)?;
        if cookies < price {
            return Err(GameError::Unaffordable);
        }
        self.set(field::COOKIES, cookies - price, "purchase")?;
        self.add_at(self.object(field::STATS)? + 8, price, "record_spending")
    }
    pub fn buy_generator(&mut self, index: usize) -> Result<()> {
        if index >= GENERATORS.len() {
            return Err(GameError::Locked);
        }
        let a =
            self.check_table(field::GENERATORS, 9, GENERATOR_SIZE)? + index as u64 * GENERATOR_SIZE;
        let price = self.memory.read_u64(a + 8)?;
        let count = self.memory.read_u64(a)?;
        self.spend(price)?;
        self.store(a, count.saturating_add(1), "buy_generator")?;
        self.store(a + 8, next_generator_price(price), "scale_price")?;
        self.recalculate_production()
    }
    fn recalculate_production(&mut self) -> Result<()> {
        let base = self.check_table(field::GENERATORS, 9, GENERATOR_SIZE)?;
        let mut cps = 0u64;
        for i in 0..9 {
            let a = base + i * GENERATOR_SIZE;
            cps = cps.saturating_add(
                self.memory
                    .read_u64(a)?
                    .saturating_mul(self.memory.read_u64(a + 16)?)
                    .saturating_mul(self.memory.read_u64(a + 24)?),
            );
        }
        self.set(
            field::CPS,
            cps.saturating_mul(self.value(field::PRESTIGE)?.saturating_add(1)),
            "recalculate_production",
        )
    }
    pub fn buy_upgrade(&mut self, index: usize) -> Result<()> {
        let spec = UPGRADES.get(index).ok_or(GameError::Locked)?;
        let a = self.check_table(field::UPGRADES, 8, 8)? + index as u64 * 8;
        if self.memory.read_u64(a)? != 0 || self.value(field::LIFETIME)? < spec.unlock {
            return Err(GameError::Locked);
        }
        self.spend(spec.price)?;
        self.store(a, 1, "buy_upgrade")?;
        if index.is_multiple_of(2) {
            self.set(
                field::CLICK,
                self.value(field::CLICK)?.saturating_mul(2),
                "upgrade_click",
            )?;
        } else {
            let base = self.check_table(field::GENERATORS, 9, GENERATOR_SIZE)?;
            for i in 0..9 {
                let a = base + i * GENERATOR_SIZE + 24;
                self.store(
                    a,
                    self.memory.read_u64(a)?.saturating_mul(2),
                    "upgrade_generator",
                )?;
            }
            self.recalculate_production()?;
        }
        Ok(())
    }
    fn update_achievements(&mut self) -> Result<()> {
        let base = self.check_table(field::ACHIEVEMENTS, 8, 8)?;
        let lifetime = self.value(field::LIFETIME)?;
        for (i, (_, threshold)) in ACHIEVEMENTS.iter().enumerate() {
            if lifetime >= *threshold && self.memory.read_u64(base + i as u64 * 8)? == 0 {
                self.store(base + i as u64 * 8, 1, "unlock_achievement")?;
            }
        }
        Ok(())
    }
    pub fn advance_tick(&mut self) -> Result<()> {
        // Guest clock, fractional production and effect deadlines survive exact snapshots.
        let stats = self.object(field::STATS)?;
        let effects = self.object(field::EFFECTS)?;
        let boost = if self.memory.read_u64(effects)? > self.tick {
            self.memory.read_u64(effects + 8)?
        } else {
            1
        };
        let cps = self
            .value(field::CPS)?
            .saturating_mul(boost)
            .saturating_mul(self.recipe_production_multiplier()?);
        let remainder = self.memory.read_u64(stats + 24)?;
        let total = cps as u128 + remainder as u128;
        let award = (total / TICKS_PER_SECOND as u128).min(u64::MAX as u128) as u64;
        self.tick = self
            .tick
            .checked_add(1)
            .ok_or_else(|| GameError::InvalidState("clock exhausted".into()))?;
        self.store(
            stats + 24,
            (total % TICKS_PER_SECOND as u128) as u64,
            "production_remainder",
        )?;
        self.add_at(stats + 16, 1, "playtime")?;
        if award > 0 {
            self.award(award, "passive_production")?;
        }
        self.advance_activity_tick()?;
        self.advance_encounters()
    }
    pub fn next_random(&mut self) -> u64 {
        self.rng_state = self.rng_state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.rng_state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    pub fn claim_gift(&mut self) -> Result<()> {
        let e = self.object(field::EFFECTS)?;
        if self.tick < self.memory.read_u64(e + 16)? {
            return Err(GameError::Locked);
        }
        let duration = 200 + self.next_random() % 101;
        self.store(e, self.tick.saturating_add(duration), "activate_boost")?;
        self.store(e + 8, 7, "boost_multiplier")?;
        self.schedule_gift()
    }
    pub fn trade_charm(&mut self) -> Result<()> {
        let stats = self.object(field::STATS)?;
        if self.memory.read_u64(stats + 40)? != 0 {
            return Err(GameError::Locked);
        }
        let stale = self.memory.read_ptr64(stats + 32)?;
        self.heap.free(stale)?;
        let coupon = self.heap.allocate(8, 8, "Coupon")?;
        let value = 2 + self.next_random() % 4;
        self.store(coupon, value, "create_coupon")?;
        // The stale LuckyCharm pointer is intentionally retained and dereferenced by clicks.
        self.store(stats + 40, 1, "trade_charm")
    }
    pub fn prestige_gain(&self) -> Result<u64> {
        let prestige = self.object(field::PRESTIGE_STATE)?;
        let previous = self.memory.read_u64(prestige)?;
        let eligible = self.value(field::LIFETIME)? / 1_000_000;
        Ok(integer_sqrt(eligible).saturating_sub(previous))
    }
    pub fn prestige(&mut self) -> Result<()> {
        let gain = self.prestige_gain()?;
        if gain == 0 {
            return Err(GameError::Locked);
        }
        let p = self.object(field::PRESTIGE_STATE)?;
        let total = self.memory.read_u64(p)?.saturating_add(gain);
        self.store(p, total, "prestige_total")?;
        self.add_at(p + 8, 1, "prestige_reset")?;
        self.set(
            field::PRESTIGE,
            self.value(field::PRESTIGE)?.saturating_add(gain),
            "prestige",
        )?;
        self.set(field::COOKIES, 0, "prestige")?;
        self.set(field::CLICK, 1, "prestige")?;
        let gen_base = self.check_table(field::GENERATORS, 9, GENERATOR_SIZE)?;
        for (i, s) in GENERATORS.iter().enumerate() {
            let a = gen_base + i as u64 * GENERATOR_SIZE;
            for (off, v) in [(0, 0), (8, s.price), (16, s.rate), (24, 1)] {
                self.store(a + off, v, "prestige_reset")?;
            }
        }
        let up = self.check_table(field::UPGRADES, 8, 8)?;
        self.write(up, &[0; 64], WriteSource::Guest, "prestige_reset", Some(up))?;
        self.store(self.object(field::STATS)? + 24, 0, "prestige_reset")?;
        let e = self.object(field::EFFECTS)?;
        self.store(e, 0, "prestige_reset")?;
        self.recalculate_production()?;
        self.reset_activities_after_prestige()
    }
    pub fn view(&self) -> Result<GameView> {
        let name = self.memory.read(self.object(field::NAME)?, NAME_SIZE)?;
        let name =
            String::from_utf8_lossy(name.split(|b| *b == 0).next().unwrap_or(&[])).into_owned();
        let stats = self.object(field::STATS)?;
        let effects = self.object(field::EFFECTS)?;
        let base = self.check_table(field::GENERATORS, 9, GENERATOR_SIZE)?;
        let boost = if self.memory.read_u64(effects)? > self.tick {
            self.memory.read_u64(effects + 8)?
        } else {
            1
        };
        let recipe_production = self.recipe_production_multiplier()?;
        let mut generators = vec![];
        for i in 0..9 {
            let a = base + i * GENERATOR_SIZE;
            let owned = self.memory.read_u64(a)?;
            let per_unit = self
                .memory
                .read_u64(a + 16)?
                .saturating_mul(self.memory.read_u64(a + 24)?)
                .saturating_mul(self.value(field::PRESTIGE)?.saturating_add(1))
                .saturating_mul(boost)
                .saturating_mul(recipe_production);
            generators.push(GeneratorView {
                owned,
                price: self.memory.read_u64(a + 8)?,
                per_unit,
                production: owned.saturating_mul(per_unit),
            });
        }
        let table = |field, count| -> Result<Vec<bool>> {
            let a = self.check_table(field, count, 8)?;
            (0..count)
                .map(|i| Ok(self.memory.read_u64(a + i * 8)? != 0))
                .collect()
        };
        Ok(GameView {
            name,
            cookies: self.value(field::COOKIES)?,
            lifetime: self.value(field::LIFETIME)?,
            click_power: self.click_power()?,
            cps: self
                .value(field::CPS)?
                .saturating_mul(boost)
                .saturating_mul(recipe_production),
            prestige: self.value(field::PRESTIGE)?,
            clicks: self.memory.read_u64(stats)?,
            spent: self.memory.read_u64(stats + 8)?,
            play_ticks: self.memory.read_u64(stats + 16)?,
            generators,
            upgrades: table(field::UPGRADES, 8)?,
            achievements: table(field::ACHIEVEMENTS, 8)?,
            boost_remaining: self.memory.read_u64(effects)?.saturating_sub(self.tick),
            gift_available: self.tick >= self.memory.read_u64(effects + 16)?,
            prestige_gain: self.prestige_gain()?,
            recipe_multiplier: self.memory.read_u64(self.object(field::SHOP)? + 24)?,
            charm_traded: self.memory.read_u64(stats + 40)? != 0,
            activities: self.activities_view()?,
            encounter: self.encounter_view()?,
        })
    }
    pub fn validate(&self) -> Result<()> {
        self.memory.validate()?;
        self.heap.validate()?;
        let r = self
            .memory
            .region_for(self.heap.base())
            .ok_or_else(|| GameError::InvalidState("heap is unmapped".into()))?;
        if r.kind() != RegionKind::Heap
            || r.len() != self.heap.size()
            || r.base() != self.heap.base()
            || !r.permissions().write
        {
            return Err(GameError::InvalidState("heap backing mismatch".into()));
        }
        if self.heap.base() != Architecture::HEAP || self.heap.size() != Architecture::HEAP_SIZE {
            return Err(GameError::InvalidState(
                "unsupported heap architecture".into(),
            ));
        }
        let expected = [
            (Architecture::CODE, 4096, Permissions::RX, RegionKind::Code),
            (
                Architecture::RODATA,
                4096,
                Permissions::R,
                RegionKind::ReadOnly,
            ),
            (
                Architecture::GLOBALS,
                4096,
                Permissions::RW,
                RegionKind::Globals,
            ),
            (
                Architecture::HEAP,
                Architecture::HEAP_SIZE,
                Permissions::RW,
                RegionKind::Heap,
            ),
            (
                Architecture::STACK_TOP - 4096,
                4096,
                Permissions::RW,
                RegionKind::Stack,
            ),
        ];
        if self.memory.regions().len() != expected.len() {
            return Err(GameError::InvalidState("unexpected memory map".into()));
        }
        for (base, len, permissions, kind) in expected {
            if self.memory.region_for(base).is_none_or(|r| {
                r.base() != base
                    || r.len() != len
                    || r.permissions() != permissions
                    || r.kind() != kind
            }) {
                return Err(GameError::InvalidState(
                    "unsupported memory architecture".into(),
                ));
            }
        }
        // Corrupted guest pointers remain valid snapshot data and fault only on access.
        Ok(())
    }
}
fn next_generator_price(price: u64) -> u64 {
    price
        .saturating_add(
            (price / 100)
                .saturating_mul(15)
                .saturating_add((price % 100) * 15 / 100),
        )
        .max(price.saturating_add(1))
}
fn integer_sqrt(n: u64) -> u64 {
    let mut lo = 0u64;
    let mut hi = n.min(1u64 << 32).saturating_add(1);
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if mid as u128 * mid as u128 <= n as u128 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
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
        self.paused = p;
    }
    fn is_paused(&self) -> bool {
        self.paused
    }
    fn read_memory(&self, a: Address, n: u64) -> vm_core::Result<Vec<u8>> {
        self.memory.read(a, n)
    }
    fn write_memory(&mut self, a: Address, b: &[u8]) -> std::result::Result<(), String> {
        self.write(a, b, WriteSource::Debugger, "memory_edit", Some(a))
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
        s.paused = false;
        s
    }
    fn restore(&mut self, mut snapshot: Self) -> std::result::Result<(), String> {
        snapshot.validate().map_err(|e| e.to_string())?;
        snapshot.memory.clear_events();
        snapshot.paused = false;
        *self = snapshot;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guest_edit_changes_real_game() {
        let mut m = Machine::new(1).unwrap();
        m.write_memory(m.address(field::CLICK).unwrap(), &1000u64.to_le_bytes())
            .unwrap();
        m.click().unwrap();
        assert_eq!(m.view().unwrap().cookies, 1000);
    }
    #[test]
    fn economy_effects_and_prestige() {
        let mut m = Machine::new(1).unwrap();
        m.set(field::COOKIES, 10_000_000, "test").unwrap();
        m.set(field::LIFETIME, 10_000_000, "test").unwrap();
        m.buy_generator(0).unwrap();
        m.buy_upgrade(1).unwrap();
        assert_eq!(m.view().unwrap().cps, 2);
        for _ in 0..20 {
            m.advance_tick().unwrap();
        }
        assert_eq!(m.view().unwrap().cookies, 10_000_000 - 15 - 500 + 2);
        let gain = m.prestige_gain().unwrap();
        assert_eq!(gain, 3);
        m.prestige().unwrap();
        assert_eq!(m.view().unwrap().prestige, 3);
        assert_eq!(m.view().unwrap().cookies, 0);
        assert_eq!(m.view().unwrap().click_power, 4);
    }
    #[test]
    fn name_overflow_corrupts_neighbor_without_metadata_damage() {
        let mut m = Machine::new(1).unwrap();
        let heap = m.heap.clone();
        let mut payload = vec![b'A'; 32];
        payload.extend(m.object(field::NAME).unwrap().to_le_bytes());
        m.rename(&payload).unwrap();
        assert_eq!(
            m.memory
                .read_ptr64(m.shutdown_callbacks_address().unwrap())
                .unwrap(),
            m.object(field::NAME).unwrap()
        );
        assert_eq!(m.heap, heap);
        let e = m.memory.events().back().unwrap();
        assert_eq!(e.overflow, 9);
        assert_eq!(e.affected.len(), 2);
    }
    #[test]
    fn name_overflow_redirects_virtual_callback_to_guest_shell() {
        let mut m = Machine::new(1).unwrap();
        assert_eq!(m.run_shutdown_callbacks().unwrap(), ShutdownOutcome::Close);

        let name = m.object(field::NAME).unwrap();
        let mut payload = GUEST_SHELL_FUNCTION.to_le_bytes().to_vec();
        payload.resize(NAME_SIZE as usize, b'A');
        payload.extend(name.to_le_bytes());
        m.rename(&payload).unwrap();

        assert_eq!(
            m.run_shutdown_callbacks().unwrap(),
            ShutdownOutcome::GuestShell
        );
        assert_eq!(
            m.heap.allocation_for(name).unwrap().semantic_type,
            "PlayerName"
        );
    }
    #[test]
    fn recipe_pointer_and_intra_object_overwrite() {
        let mut m = Machine::new(1).unwrap();
        let mut payload = vec![b'X'; 24];
        payload.extend(7u64.to_le_bytes());
        payload.extend(m.address(field::CLICK).unwrap().to_le_bytes());
        m.label_recipe(&payload).unwrap();
        m.click().unwrap();
        assert_eq!(m.value(field::COOKIES).unwrap(), 7);
    }
    #[test]
    fn stale_pointer_reuses_allocation() {
        let mut m = Machine::new(1).unwrap();
        let stats = m.object(field::STATS).unwrap();
        let stale = m.memory.read_ptr64(stats + 32).unwrap();
        m.trade_charm().unwrap();
        assert_eq!(m.memory.read_ptr64(stats + 32).unwrap(), stale);
        assert_eq!(
            m.heap.allocation_for(stale).unwrap().semantic_type,
            "Coupon"
        );
        assert!(m.click_power().unwrap() >= 2);
        m.heap.validate().unwrap();
    }
    #[test]
    fn deterministic_machine_and_fractional_production() {
        let mut a = Machine::new(1234).unwrap();
        let mut b = Machine::new(1234).unwrap();
        for m in [&mut a, &mut b] {
            for _ in 0..30 {
                m.click().unwrap();
            }
            m.buy_generator(0).unwrap();
            for _ in 0..10 {
                m.advance_tick().unwrap();
            }
            assert_eq!(m.view().unwrap().cookies, 15);
            for _ in 0..10 {
                m.advance_tick().unwrap();
            }
            assert_eq!(m.view().unwrap().cookies, 16);
        }
        assert_eq!(a, b);
    }
    #[test]
    fn corrupted_pointer_faults_without_changing_host() {
        let mut m = Machine::new(1).unwrap();
        m.set(field::SHOP, u64::MAX, "test").unwrap();
        assert!(m.click().is_err());
        assert!(m.memory.read_u64(u64::MAX).is_err());
    }
}
