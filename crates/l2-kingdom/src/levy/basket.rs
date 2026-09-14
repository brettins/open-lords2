#![allow(unused_imports)]
use super::*;
use super::raise::*;
use super::tests::*;
use crate::county::{County, MAX_COUNTIES};
use crate::map::{flags, CampaignMap};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{Tables, WEAPON_TYPE_COUNT};
use crate::unit::{ArmyNames, TroopType, Unit, UnitKind, Units, TROOP_TYPES};

impl LevyBasket {
    /// The levy total — slot 7, which the `+`/`−` buttons never move.
    pub fn total(&self) -> i32 {
        self.slots[BASKET_TOTAL].chosen
    }

    /// The men still carrying nothing.
    pub fn unequipped(&self) -> i32 {
        self.slots[0].chosen
    }

    /// The seven troop counts an army raised from this basket would have.
    pub fn troops(&self) -> [i32; TROOP_TYPES] {
        core::array::from_fn(|t| self.slots[t].chosen)
    }

    /// Move `n` men from the unequipped pool into a weapon slot, as the `+`
    /// button does one at a time. Returns how many
    /// bounded by the men available and by the stock left.
    pub fn equip(&mut self, troop: TroopType, n: i32) -> i32 {
        let Some(slot) = troop.weapon_slot().map(|w| w + 1) else { return 0 };
        let moved = n.min(self.slots[0].chosen).min(self.slots[slot].remaining).max(0);
        self.slots[0].chosen -= moved;
        self.slots[slot].chosen += moved;
        self.slots[slot].remaining -= moved;
        moved
    }

    /// Put a weapon slot's men back in the unequipped pool, as the `−` button
    /// does.
    pub fn unequip(&mut self, troop: TroopType, n: i32) -> i32 {
        let Some(slot) = troop.weapon_slot().map(|w| w + 1) else { return 0 };
        let moved = n.min(self.slots[slot].chosen).max(0);
        self.slots[slot].chosen -= moved;
        self.slots[slot].remaining += moved;
        self.slots[0].chosen += moved;
        moved
    }

    /// The AI's auto-equip: ten men at a time, round-robin over the six weapon
    /// types.
    ///
    /// ```c
    /// men = slot[7].chosen;  pass = 0;  changed = true;
    /// while (pass < 50 && changed) {
    ///     changed = false;
    ///     for (t = 1; t < 7; t++) {
    ///         if (men < 10) goto done;
    ///         if (slot[t].remaining >= 10) {
    ///             slot[t].chosen += 10;  slot[t].remaining -= 10;
    ///             slot[0].chosen -= 10;  men -= 10;  changed = true;
    ///         }
    ///     }
    ///     pass++;
    /// }
    /// ```
    ///
    /// > **`docs/armies.md` §6.2 says it runs *"until either the men or a
    /// > weapon type runs out"*. Wrong on the second half:** a weapon type that
    /// > runs out is *skipped* on every later pass
    /// > continues with the others, so an army is equipped from whatever the
    /// > armoury still has
    /// > is also a **50-pass ceiling** — at six types and ten men a pass that is
    /// > 3,000 men, twice [`crate::tables::ARMY_MAX_MEN`], so it never bites in
    /// > play —
    /// > are always left as peasants.** Corrected in the document. `[V]`
    pub fn auto_equip(&mut self) {
        let mut men = self.slots[BASKET_TOTAL].chosen;
        let mut pass = 0;
        let mut changed = true;
        while pass < crate::tables::LEVY_AUTO_EQUIP_ROUNDS && changed {
            changed = false;
            for t in 1..=WEAPON_TYPE_COUNT {
                if men < crate::tables::LEVY_AUTO_EQUIP_BATCH {
                    return;
                }
                if self.slots[t].remaining >= crate::tables::LEVY_AUTO_EQUIP_BATCH {
                    let n = crate::tables::LEVY_AUTO_EQUIP_BATCH;
                    self.slots[t].chosen += n;
                    self.slots[t].remaining -= n;
                    self.slots[0].chosen -= n;
                    men -= n;
                    changed = true;
                }
            }
            pass += 1;
        }
    }

    /// What the realm's stockpiles look like after this basket is spent —
    /// `Levy_ConsumeWeapons` (`0x004A9EB1`).
    pub fn consume_weapons(&self, realm: &mut Realm) {
        for t in 1..=WEAPON_TYPE_COUNT {
            realm.weapons[t - 1] = (realm.weapons[t - 1] - self.slots[t].chosen).max(0);
        }
    }
}

