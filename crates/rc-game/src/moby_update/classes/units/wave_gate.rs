//! U426 (census 2026-09-29): class 1271, a wave gate of Gemlik (level13 `0x30af50`, one instance). Not an enemy: a
//! watcher over up to eight moby groups. While any of them has a live member it does nothing; once all are gone (and
//! Ratchet is inside its cuboid, when it has one) it sets its own state and its command byte +0xbc to the values in
//! its pvars, which the level's other code reads (the doors / the next wave the gate opens: their classes' own
//! business). No sound, no effect, no hit.
//!
//! **Pvars** (s32): +0x00..+0x1c the groups (−1 unused), +0x20 the state to take (−1 none), +0x24 the command byte
//! (−1 none), +0x28 the cuboid (−1 none).
//!
//! ## Coverage (level13 `0x30af50` and `0x30ae68`)
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | the groups up to the first −1 (the loop's pointer does not advance past a −1, so it re-reads it for the remaining of the eight): `0x30ae68(group, −1)` ≠ 0 → nothing this tick | a group still alive | [`update`] ([`alive`]) |
//! | `0x30ae68(group, s)`: the members of the group list (`0x1abb40[group]`) in a live state (≥ 0), not held by the Suck Cannon (no suck record, `0x2fe8d0` = `0x304100`, or its state < 4) and, unless `s` = −1, not in state `s` | the count | [`alive`] (`react::rec_state`) |
//! | the ship-combat word 0x140940 ≠ 0 and Ratchet in state 0x32 → nothing | Ratchet's ship mode (the ridden-vehicle record, [`crate::vehicle::Record`]) | [`update`] |
//! | cuboid +0x28 ≠ −1 and Ratchet (0x13f3d0) outside it (`0x26b9a8` = `PointInCuboid`) → nothing | | [`update`] (`World::in_cuboid`) |
//! | +0x20 ≠ −1 → state; +0x24 ≠ −1 → +0xbc | the gate opens | [`update`] |
//!
//! Native; no rand, no float work beyond the cuboid test.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, react};
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x30_af50;
pub const REFERENCE_LEVEL: u32 = 13;
pub const CLASSES: [i16; 1] = [1271];

/// `0x30ae68(group, s)` (module doc): the live members of `group` (not in state `s` when `s` ≠ −1).
pub fn alive(w: &World, group: i32, s: i32) -> usize {
    let Some(Some(list)) = usize::try_from(group).ok().and_then(|g| w.svc.groups.lists.get(g)) else { return 0 };
    list.iter()
        .map(|&e| (e & 0x7fff) as usize)
        .filter(|&m| m < w.table.mobys.len())
        .filter(|&m| (w.m(m).state as i8) >= 0)
        .filter(|&m| react::rec_state(w, m).is_none_or(|r| r < 4))
        .filter(|&m| s == -1 || w.m(m).state as i32 != s)
        .count()
}

/// Level13 `0x30af50` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x2c { return; }
    // The game's pointer advances only past a used slot, so the scan stops at the first −1 (the rest re-read it).
    for k in 0..8 {
        let g = c::pi32(w, id, 4 * k);
        if g == -1 { break; }
        if alive(w, g, -1) != 0 { return; }
    }
    // `0x140940 ≠ 0 && 0x1413d4 == 0x32`: Ratchet's ship mode.
    if w.svc.vehicle.moby.is_some() && w.hero.state == 0x32 { return; }
    let cub = c::pi32(w, id, 0x28);
    if cub != -1 {
        let h = crate::moby_update::classes::units::hero_pos(w);
        if !w.in_cuboid([h[0], h[1], h[2]], cub) { return; }
    }
    let s = c::pi32(w, id, 0x20);
    if s != -1 { w.mm(id).state = s as u8; }
    let b = c::pi32(w, id, 0x24);
    if b != -1 { w.mm(id).cmd = b as u8; }
}
