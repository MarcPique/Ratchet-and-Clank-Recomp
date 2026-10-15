//! The Aridia help director, class 1324 (census U119; level02 `0x2ee890`, one instance). Read from the level02
//! decomp. `H[n]` / `M[n]` / `G[n]`: help, move and gadget-help records; "ok" = group ok.
//!
//! **Pvars** (s32): +0x00 a mission (its byte `0x14c050[L·16 + m]` and its deaths `0x14ee90[m]`); cuboids +0x10,
//! +0x14, +0x18, +0x1c, +0x40, +0x44, +0x48, +0x4c, +0x5c; +0x20..+0x2c Ratchet's position (stored every tick);
//! counters +0x30 (sand-hop landings), +0x50 (Trespasser ticks); +0x54 the dune-jump state; +0x58 the camera-hint latch.
//!
//! | address | what | port |
//! |---|---|---|
//! | entry | update distance 0xff | [`update`] |
//! | 0x2ee8c0 | in cuboid +0x5c, group < 2, `H[0x4f]` and `M[20]` 0, two or more owned hand items (slot type 0, not 8 / 0x18; `0x17a0c8` = the item definitions' +8 here) → `(20005, 0x4f)` | [`update`] |
//! | 0x2ee990 | +0x54 = 0: in cuboid +0x48 in state 0x2c → 1; = 1: airborne (0x13f650 = 0) in cuboid +0x4c with skill point 0x13d409 not earned → skill point, level sound 1, banner 0x53d6; grounded → 0 | [`update`]; the award: `story::award_skill_point` |
//! | 0x2eea40 | `H[0xc]` 0, in cuboid +0x10, the mission open, item 16 owned, its deaths ≥ 6 and more than `ScaleTicks(18000)` since `G[16]`'s time → `(2002, 0xc)` | [`update`] |
//! | 0x2eeae8 | in cuboid +0x40 → the latch +0x58 := 1; latched: `M[8]` < 3 and `H[0x4c]` without this level's bit → `(20002, 0x4c)`; a camera option reversed (0x15eddc / 0x15ede0) → `H[0x76]` count := 0xffff, else `H[0x76]` without this level's bit → `(20012, 0x76)` | [`update`] (`Help::cam_reversed`) |
//! | 0x2eeba0 | `H[0xd]` 0, in cuboid +0x10, the mission open, item 16 not owned, its deaths ≥ 3 → `(2003, 0xd)` | [`update`] |
//! | 0x2eec08 | `H[0x10]` 0: in cuboid +0x18 → `H[0x10]` count := 0xffff; state 0x2c left on its first tick (previous state 0x2c, timer 1) → +0x30 += 1; +0x30 > 1 in cuboid +0x1c → `(2006, 0x10)` | [`update`] |
//! | 0x2eecb8 | item 12 not owned, in cuboid +0x14: the reminder on `H[0x11]` → `(2007, 0x11)`; owned, in cuboid +0x14: the arm / reminder on `H[0x12]` → `(2008, 0x12)`; otherwise in cuboid +0x1c → `H[0x12]` count := 0xffff | [`update`] |
//! | 0x2eef1c | `H[0x46]` or `H[0x47]` unused, state 0x72 on a Trespasser (ground moby of class 615): `H[0x46]` 0 → `(2011, 0x46)`; else +0x50 += 1, past `ScaleTicks(600)` → `(2012, 0x47)` | [`update`] |
//! | 0x2eefd8 | global flag 0x14 (0x13d39c) clear, in cuboid +0x44 → flag := 1 | [`update`] (G-SAV-010) |
//! | exit | +0x20..+0x2c := Ratchet's position 0x13f3d0 | [`update`] |
//! | no sound (besides the skill point's), particle, save, other moby written | | n/a |

use super::hints::{arm_or_remind, flag, in_cuboid, mission_done, older_than, owned, pvi, remind, request, set_flag, set_pvi, ticks};
use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2e_e890;
pub const REFERENCE_LEVEL: u32 = 2;
pub const CLASSES: [i16; 1] = [1324];

fn h(w: &World, r: usize) -> u16 { w.svc.help.records.help[r].count }
fn level_bit(w: &World) -> u32 { 1u32 << (w.svc.help.level as u32 & 31) }

/// Level02 0x2ee890 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    w.mm(id).update_dist = 0xff;
    if in_cuboid(w, id, 0x5c) && (w.hero.group as u32) < 2 && h(w, 0x4f) == 0 && w.svc.help.records.moves[20].count == 0 {
        let n = (0..0x25usize).filter(|&i| i != 8 && i != 0x18 && w.inventory.slot_type(i) == 0 && owned(w, i)).count();
        if n > 1 { request(w, 0x4e25, 0x4f); }
    }
    match pvi(w, id, 0x54) {
        0 if in_cuboid(w, id, 0x48) && w.hero.state == 0x2c => set_pvi(w, id, 0x54, 1),
        1 if w.hero.grounded_ticks != 0 => set_pvi(w, id, 0x54, 0),
        // Airborne (0x13f650 = 0) in cuboid +0x4c: skill point 0x13d409 (`crate::moby_update::story`).
        1 if in_cuboid(w, id, 0x4c) => { crate::moby_update::story::award_skill_point(w, crate::moby_update::story::skill_index(0x13_d409)); }
        _ => {}
    }
    let mission = pvi(w, id, 0);
    let deaths = u8::try_from(mission).map_or(0, |m| w.missions.ammo_crate_gate(m));
    let open = !mission_done(w, mission);
    if h(w, 0xc) == 0 && in_cuboid(w, id, 0x10) && open && owned(w, 16) && deaths >= 6 && older_than(w, w.svc.help.records.gadget[16].time, 18000) {
        request(w, 0x7d2, 0xc);
    }
    if in_cuboid(w, id, 0x40) { set_pvi(w, id, 0x58, 1); }
    if pvi(w, id, 0x58) != 0 {
        if w.svc.help.records.moves[8].count < 3 && w.svc.help.records.help[0x4c].mask & level_bit(w) == 0 { request(w, 0x4e22, 0x4c); }
        if w.svc.help.cam_reversed {
            w.svc.help.records.help[0x76].count = 0xffff;
        } else if w.svc.help.records.help[0x76].mask & level_bit(w) == 0 {
            request(w, 0x4e2c, 0x76);
        }
    }
    if h(w, 0xd) == 0 && in_cuboid(w, id, 0x10) && open && !owned(w, 16) && deaths as f32 >= 3.0 { request(w, 0x7d3, 0xd); }
    if h(w, 0x10) == 0 {
        if in_cuboid(w, id, 0x18) { w.svc.help.records.help[0x10].count = 0xffff; }
        if w.hero.state != 0x2c && w.hero.prev_state == 0x2c && w.hero.timer == 1 { set_pvi(w, id, 0x30, pvi(w, id, 0x30) + 1); }
        if pvi(w, id, 0x30) > 1 && in_cuboid(w, id, 0x1c) { request(w, 0x7d6, 0x10); }
    }
    let hop = in_cuboid(w, id, 0x14);
    if !owned(w, 12) {
        if hop { remind(w, 0x11, 0x7d7, 0x11); }
    } else if hop {
        arm_or_remind(w, 0x12, 0x7d8, 0x12);
    }
    if !(owned(w, 12) && hop) && in_cuboid(w, id, 0x1c) { w.svc.help.records.help[0x12].count = 0xffff; }
    let on_trespasser = w.hero.state == 0x72 && w.hero.ground_moby.and_then(|g| w.table.mobys.get(g)).is_some_and(|g| g.o_class == 0x267);
    if (h(w, 0x46) == 0 || h(w, 0x47) == 0) && on_trespasser {
        if h(w, 0x46) == 0 {
            request(w, 0x7db, 0x46);
        } else {
            let n = pvi(w, id, 0x50) + 1;
            set_pvi(w, id, 0x50, n);
            if ticks(600) < n { request(w, 0x7dc, 0x47); }
        }
    }
    if !flag(w, 0x14) && in_cuboid(w, id, 0x44) { set_flag(w, 0x14); }
    let p = w.hero_point();
    for (k, v) in p.iter().enumerate() { set_pvi(w, id, 0x20 + 4 * k, v.to_bits() as i32); }
    set_pvi(w, id, 0x2c, w.hero.pos[3].0 as i32);
}
