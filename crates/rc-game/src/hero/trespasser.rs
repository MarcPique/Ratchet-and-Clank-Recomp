//! **The Trespasser** (item 26 = 0x1a, gadget class 188; its update is `0x2d7380` on level 01, the same code in every
//! overlay's class table: L02 `0x2c1128`, L05 `0x2ed3a0`, …). The gadget itself only tells the locks that ○ is held:
//! the lock class 615 (`crate::moby_update::classes::units::trespasser_lock`, level02 `0x2d8ad0`) reads the hand moby's
//! byte +0xbc while Ratchet stands on it with the Trespasser in hand, and runs the whole minigame (camera, hold, rings,
//! input, solve, door) itself.
//!
//! **Coverage** (`address | what | status`):
//!
//! | address | what | status |
//! |---|---|---|
//! | 0x2d7380 | pvar +0x04 = 0 (the hand item): +0xbc = (○ held: 0x13cae0 & the slot's fire mask 0x1403f0) ? 1 : 0 | ported ([`update`], `Gadgets::item_bc`) |
//! | 0x2d7380 → 0x2d73e0 | pvar +0x04 ≠ 0: the same class placed as a pickup (spins at 0x15f5cc % 10, Ratchet within 1 (xy) and 2 (z) with health → `GiveItem(0x1a, 1)`, state 2; state 0 snaps it to the ground `0x26e618(0.5)` + 0x1742e8 + gp−0x53a8 and sets its Euler (π/2, π, ·); state 2 shrinks its scale by `Approach(0, class scale·0.02)` to 0 → `DeleteMoby`) | n/a (no level places class 188 and no class creates one: census `classes.tsv`; the Trespasser is given by the story) |
//! | sounds, particles, lights, stats, bolts, save flags | none in the hand form | n/a |

use super::items::{HitSink, ItemEnv};
use super::Hero;
use crate::moby_runtime::MobyTable;
use crate::rng::Rng;

pub const TRESPASSER: i32 = 0x1a;

/// `0x2d7380`, the Trespasser's update: its +0xbc = ○ held.
pub fn update(hero: &mut Hero, _table: &mut MobyTable, _anim: &dyn super::anim::AnimCtl, env: &ItemEnv, _hits: &mut dyn HitSink, _rng: &mut Rng) {
    hero.gadgets.item_bc = (env.pad.held & hero.items.slot.fire_mask != 0) as u8;
}
