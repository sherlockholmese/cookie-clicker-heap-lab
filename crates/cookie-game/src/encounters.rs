//! Timed visitors and rare rewards use only the saved guest clock and RNG.
use super::*;

pub const ENCOUNTERS_ROOT: Address = Architecture::GLOBALS + 16;
const SIZE: u64 = 80;
pub mod encounter_field {
    pub const KIND: u64 = 0;
    pub const HEALTH: u64 = 8;
    pub const NEXT_SPAWN: u64 = 16;
    pub const NEXT_BITE: u64 = 24;
    pub const BITE_SIZE: u64 = 32;
    pub const EATEN: u64 = 40;
    pub const TOTAL_STOLEN: u64 = 48;
    pub const DEFEATED: u64 = 56;
    pub const EXPIRES: u64 = 64;
    pub const RECOVERED: u64 = 72;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visitors_eat_and_return_cookies_without_inflating_lifetime() {
        let mut m = Machine::new(1234).unwrap();
        m.set(field::COOKIES, 100, "test").unwrap();
        m.set(field::LIFETIME, 100, "test").unwrap();
        let a = m.encounters_address().unwrap();
        let next = m.memory.read_u64(a + encounter_field::NEXT_SPAWN).unwrap();
        assert!((90 * TICKS_PER_SECOND..=150 * TICKS_PER_SECOND).contains(&next));
        m.tick = next - 1;
        m.advance_tick().unwrap();
        assert!(m.encounter_view().unwrap().name.is_some());
        for _ in 0..TICKS_PER_SECOND {
            m.advance_tick().unwrap();
        }
        assert_eq!(m.value(field::COOKIES).unwrap(), 99);
        assert_eq!(m.value(field::LIFETIME).unwrap(), 100);
        for _ in 0..m.encounter_view().unwrap().health {
            m.shoo_enemy().unwrap();
        }
        assert_eq!(m.value(field::COOKIES).unwrap(), 100);
        assert_eq!(m.value(field::LIFETIME).unwrap(), 100);
        assert_eq!(m.encounter_view().unwrap().defeated, 1);
        assert!(m.encounter_view().unwrap().name.is_none());
        assert!(m.shoo_enemy().is_err());
        let next = m.memory.read_u64(a + encounter_field::NEXT_SPAWN).unwrap();
        assert!((90 * TICKS_PER_SECOND..=180 * TICKS_PER_SECOND).contains(&(next - m.tick)));
    }
    #[test]
    fn edited_enemy_damage_is_bounded_by_balance_and_expiry_keeps_the_loss() {
        let mut m = Machine::new(1).unwrap();
        m.set(field::COOKIES, 10, "test").unwrap();
        let a = m.encounters_address().unwrap();
        for (offset, value) in [
            (encounter_field::KIND, 2),
            (encounter_field::HEALTH, 1),
            (encounter_field::BITE_SIZE, u64::MAX),
            (encounter_field::NEXT_BITE, 1),
            (encounter_field::EXPIRES, 2),
        ] {
            m.write_memory(a + offset, &value.to_le_bytes()).unwrap();
        }
        m.advance_tick().unwrap();
        assert_eq!(m.value(field::COOKIES).unwrap(), 0);
        assert_eq!(m.encounter_view().unwrap().stolen, 10);
        m.advance_tick().unwrap();
        assert!(m.encounter_view().unwrap().name.is_none());
        assert_eq!(m.value(field::COOKIES).unwrap(), 0);
        m.write_memory(ENCOUNTERS_ROOT, &u64::MAX.to_le_bytes())
            .unwrap();
        assert!(m.encounter_view().is_err());
        assert!(m.advance_tick().is_err());
    }
    #[test]
    fn rush_offers_arrive_randomly_and_expire_deterministically() {
        let mut deadlines = std::collections::BTreeSet::new();
        for seed in 1..=6 {
            let mut m = Machine::new(seed).unwrap();
            assert!(m.start_rush().is_err());
            let a = m.activities_address().unwrap();
            let next = m.memory.read_u64(a + activity_field::RUSH_NEXT).unwrap();
            deadlines.insert(next);
            assert!((45 * TICKS_PER_SECOND..=90 * TICKS_PER_SECOND).contains(&next));
            m.tick = next - 1;
            let mut replay = m.clone();
            for machine in [&mut m, &mut replay] {
                machine.advance_tick().unwrap();
                assert_eq!(
                    machine.activities_view().unwrap().rush_state,
                    RushState::Offered
                );
                assert!((12..=20).contains(&machine.activities_view().unwrap().rush_target));
                for _ in 0..30 * TICKS_PER_SECOND {
                    machine.advance_tick().unwrap();
                }
                assert_eq!(
                    machine.activities_view().unwrap().rush_state,
                    RushState::Idle
                );
                assert!(machine.start_rush().is_err());
            }
            assert_eq!(m.snapshot(), replay.snapshot());
        }
        assert!(
            deadlines.len() > 1,
            "the arrival time must vary with the seed"
        );
    }
}
#[derive(Debug, Clone)]
pub struct EncounterView {
    pub name: Option<&'static str>,
    pub health: u64,
    pub bite: u64,
    pub stolen: u64,
    pub defeated: u64,
}
fn initial_delay(seed: u64, salt: u64, min: u64, max: u64) -> u64 {
    let mut z = seed.wrapping_add(salt);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    (min + (z ^ (z >> 31)) % (max - min + 1)) * TICKS_PER_SECOND
}
impl Machine {
    pub fn initialize_encounters(&mut self) -> Result<()> {
        if self.memory.read_ptr64(ENCOUNTERS_ROOT)? != 0 {
            return Ok(());
        }
        let a = self.heap.allocate(SIZE, 8, "BakeryVisitors")?;
        self.write(
            a,
            &[0; SIZE as usize],
            WriteSource::Initialization,
            "initialize_visitors",
            Some(a),
        )?;
        self.store_init(
            a + encounter_field::NEXT_SPAWN,
            self.tick
                .saturating_add(initial_delay(self.seed, 43, 90, 150)),
        )?;
        self.store_init(ENCOUNTERS_ROOT, a)?;
        // Upgrade old saves without discarding a running rush or an unclaimed win.
        if let Ok(activity) = self.activities_address() {
            let state = self
                .memory
                .read_u64(activity + activity_field::RUSH_STATE)?;
            if state == 0 || state == 3 {
                self.store_init(activity + activity_field::RUSH_STATE, 0)?;
                self.store_init(
                    activity + activity_field::RUSH_NEXT,
                    self.tick
                        .saturating_add(initial_delay(self.seed, 17, 45, 90)),
                )?;
            }
        }
        if let Ok(effects) = self.object(field::EFFECTS) {
            self.store_init(
                effects + 16,
                self.tick
                    .saturating_add(initial_delay(self.seed, 71, 180, 300)),
            )?;
        }
        Ok(())
    }
    pub fn encounters_address(&self) -> Result<Address> {
        let a = self.memory.read_ptr64(ENCOUNTERS_ROOT)?;
        self.memory.read(a, SIZE)?;
        Ok(a)
    }
    pub(crate) fn next_event_tick(&mut self, min_seconds: u64, max_seconds: u64) -> u64 {
        let delay = min_seconds + self.next_random() % (max_seconds - min_seconds + 1);
        self.tick.saturating_add(delay * TICKS_PER_SECOND)
    }
    pub(crate) fn schedule_gift(&mut self) -> Result<()> {
        let next = self.next_event_tick(240, 420);
        self.store(self.object(field::EFFECTS)? + 16, next, "schedule_gift")
    }
    pub(crate) fn advance_encounters(&mut self) -> Result<()> {
        let a = self.encounters_address()?;
        if self.memory.read_u64(a + encounter_field::KIND)? == 0 {
            if self.tick >= self.memory.read_u64(a + encounter_field::NEXT_SPAWN)? {
                let kind = 1 + self.next_random() % 3;
                let bite = (self
                    .value(field::CPS)?
                    .saturating_mul(self.recipe_production_multiplier()?)
                    / 4)
                .max(self.value(field::COOKIES)? / 500)
                .max(1);
                for (offset, value) in [
                    (encounter_field::KIND, kind),
                    (encounter_field::HEALTH, kind + 2),
                    (encounter_field::BITE_SIZE, bite),
                    (encounter_field::EATEN, 0),
                    (
                        encounter_field::NEXT_BITE,
                        self.tick.saturating_add(TICKS_PER_SECOND),
                    ),
                    (
                        encounter_field::EXPIRES,
                        self.tick.saturating_add(20 * TICKS_PER_SECOND),
                    ),
                ] {
                    self.store(a + offset, value, "visitor_arrives")?;
                }
            }
            return Ok(());
        }
        if self.tick >= self.memory.read_u64(a + encounter_field::EXPIRES)? {
            return self.dismiss_enemy();
        }
        if self.tick >= self.memory.read_u64(a + encounter_field::NEXT_BITE)? {
            let cookies = self.value(field::COOKIES)?;
            let bite = self
                .memory
                .read_u64(a + encounter_field::BITE_SIZE)?
                .min(cookies);
            self.set(field::COOKIES, cookies - bite, "enemy_eats")?;
            self.add_at(a + encounter_field::EATEN, bite, "enemy_eats")?;
            self.add_at(a + encounter_field::TOTAL_STOLEN, bite, "enemy_eats")?;
            self.store(
                a + encounter_field::NEXT_BITE,
                self.tick.saturating_add(TICKS_PER_SECOND),
                "enemy_next_bite",
            )?;
        }
        Ok(())
    }
    pub fn shoo_enemy(&mut self) -> Result<u64> {
        let a = self.encounters_address()?;
        if self.memory.read_u64(a + encounter_field::KIND)? == 0 {
            return Err(GameError::Locked);
        }
        let health = self.memory.read_u64(a + encounter_field::HEALTH)?;
        if health > 1 {
            self.store(a + encounter_field::HEALTH, health - 1, "shoo_visitor")?;
            return Ok(0);
        }
        let recovered = self.memory.read_u64(a + encounter_field::EATEN)?;
        // These cookies were already earned: restoring them must not increase lifetime cookies.
        self.set(
            field::COOKIES,
            self.value(field::COOKIES)?.saturating_add(recovered),
            "recover_cookies",
        )?;
        self.add_at(a + encounter_field::RECOVERED, recovered, "recover_cookies")?;
        self.add_at(a + encounter_field::DEFEATED, 1, "visitor_defeated")?;
        self.dismiss_enemy()?;
        Ok(recovered)
    }
    pub(crate) fn dismiss_enemy(&mut self) -> Result<()> {
        let a = self.encounters_address()?;
        let next = self.next_event_tick(90, 180);
        self.store(a + encounter_field::KIND, 0, "visitor_leaves")?;
        self.store(a + encounter_field::HEALTH, 0, "visitor_leaves")?;
        self.store(a + encounter_field::NEXT_SPAWN, next, "schedule_visitor")
    }
    pub fn encounter_view(&self) -> Result<EncounterView> {
        let a = self.encounters_address()?;
        let name = match self.memory.read_u64(a)? {
            0 => None,
            1 => Some("Sugar ants"),
            2 => Some("Crumb mouse"),
            3 => Some("Cookie gobbler"),
            _ => return Err(GameError::InvalidState("unknown bakery visitor".into())),
        };
        Ok(EncounterView {
            name,
            health: self.memory.read_u64(a + encounter_field::HEALTH)?,
            bite: self.memory.read_u64(a + encounter_field::BITE_SIZE)?,
            stolen: self.memory.read_u64(a + encounter_field::EATEN)?,
            defeated: self.memory.read_u64(a + encounter_field::DEFEATED)?,
        })
    }
}
