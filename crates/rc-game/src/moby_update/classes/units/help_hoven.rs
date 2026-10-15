//! The Hoven help director, class 422 (census U391; level12 `0x2ed280`, one instance). Read from the level12 decomp
//! (the four "unreachable" blocks are `div` traps).
//!
//! **Pvars** (s32): +0x00 the hint's cuboid, +0x04 / +0x08 two tick counters, +0x0c a moby group.
//!
//! | address | what | port |
//! |---|---|---|
//! | entry | `MobyGroupCount(P+0x0c, −1)` = 0 and skill point 0x13d41c not earned → skill point, level sound 1, banner 0x53d6 | [`update`] (`story::award_skill_point`) |
//! | | `H[0x6d]` count 0: hero state 0x81 → count := 1; time / mask refreshed; in cuboid +0x00 → `Help_Request(12003, 0x6d)` | [`update`] |
//! | | `H[0x6e]` count 0, or count 1 with more than `ScaleTicks(72000)` of play since its time: in state 0x81, +0x04 += 1; the pad's L2 / R2 (0x13cae0 & 3) held with the stick past 0.3 (0x1415ec) → +0x08 += 1, else +0x08 := 0; +0x08 > `ScaleTicks(40)` → +0x04 := 0 and `H[0x6e]` bumped; +0x04 > count·`ScaleTicks(7200)` + `ScaleTicks(1800)` → `Help_Request(12004, 0x6e)` | [`update`] |
//! | | item 4 owned (0x13d4c4) and `H[0x3e]` count 0 → `Help_Request(12000, 0x3e)` | [`update`] |
//! | no sound (besides the skill point's), particle, flag, save, other moby written | | n/a |

use super::hints::{bump, in_cuboid, owned, pvi, request, set_pvi, ticks, touch};
use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2e_d280;
pub const REFERENCE_LEVEL: u32 = 12;
pub const CLASSES: [i16; 1] = [422];

/// Level12 0x2ed280 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    // Its moby group (P+0x0c) emptied: skill point 0x13d41c.
    if crate::moby_update::scheduler::group_count(w, pvi(w, id, 0xc), -1) == 0 {
        crate::moby_update::story::award_skill_point(w, crate::moby_update::story::skill_index(0x13_d41c));
    }
    if w.svc.help.records.help[0x6d].count == 0 {
        if w.hero.state == 0x81 { w.svc.help.records.help[0x6d].count = 1; }
        touch(w, 0x6d);
        if in_cuboid(w, id, 0) { request(w, 0x2ee3, 0x6d); }
    }
    let r = w.svc.help.records.help[0x6e];
    let open = r.count == 0 || (r.count <= 1 && ticks(72000) < ticks(w.svc.help.play_time) - 600 * r.time as i32);
    if open && w.hero.state == 0x81 {
        set_pvi(w, id, 4, pvi(w, id, 4) + 1);
        let held = w.hero.loop_in.pad.held & 3 != 0;
        let n = if held && 0.3 < f32::from_bits(w.hero.stick_mag.0) { pvi(w, id, 8) + 1 } else { 0 };
        set_pvi(w, id, 8, n);
        if ticks(0x28) < n {
            set_pvi(w, id, 4, 0);
            bump(w, 0x6e);
        }
        let c = w.svc.help.records.help[0x6e].count as i32;
        if c * ticks(0x1c20) + ticks(0x708) < pvi(w, id, 4) { request(w, 0x2ee4, 0x6e); }
    }
    if owned(w, 4) && w.svc.help.records.help[0x3e].count == 0 { request(w, 12000, 0x3e); }
}
