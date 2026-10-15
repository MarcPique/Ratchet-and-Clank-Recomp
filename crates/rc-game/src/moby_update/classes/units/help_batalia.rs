//! The Batalia help director, class 1349 (census U292; level08 `0x307540`, one instance). Read from the level08
//! decomp.
//!
//! **Pvars** (s32): +0x00 / +0x04 / +0x08 / +0x0c cuboids, +0x10 the grind-rail state (0 / 1).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x307558 | +0x10 = 0: in cuboid +0x08 while grinding (group 0xf) → 1; = 1: state 0x42 → 0 | [`update`] |
//! | | = 1, in cuboid +0x0c and skill point 0x13d413 not earned → skill point, level sound 1, banner 0x53d6 | [`update`] (`story::award_skill_point`) |
//! | 0x307618 | Grindboots (item 29) not owned, in cuboid +0x00: the reminder on `H[0x30]` → `Help_Request(8000, 0x30)` | [`update`] ([`super::hints::remind`]) |
//! | 0x307728 | owned, in cuboid +0x00 and global flag 0x2f (0x13d3b7) clear: the arm / reminder on `H[0x31]` → `(8001, 0x31)` | [`update`] |
//! | 0x3077f8 | Magneboots (item 28) not owned, in cuboid +0x04 with planet 10 unlocked (0x13dd4a): the reminder on `H[0x32]` → `(8002, 0x32)` | [`update`] |
//! | 0x3079a0 | owned, in cuboid +0x04 with planet 10: the arm / reminder on `H[0x70]` → `(8005, 0x70)` | [`update`] |
//! | 0x307af0 | gravity 0x13f5e0 · (0, 0, −1) < 0.1 (walking a magnetic wall) → `H[0x70]` count := 0xffff | [`update`] |
//! | | flag 0x2f clear while grinding (group 0xf) → flag := 1 (the grind done once) | [`update`] (G-SAV-010 write) |
//! | no sound (besides the skill point's), particle, save, other moby written | | n/a |

use super::hints::{arm_or_remind, flag, in_cuboid, owned, pvi, remind, set_flag, set_pvi};
use crate::moby_runtime::MobyId;
use crate::moby_update::services::{fl, World};

pub const UPDATE_FN: u32 = 0x30_7540;
pub const REFERENCE_LEVEL: u32 = 8;
pub const CLASSES: [i16; 1] = [1349];

fn planet(w: &World, i: usize) -> bool { w.svc.interact.game.planet_unlocked.get(i).is_some_and(|&b| b != 0) }

/// Level08 0x307540 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    match pvi(w, id, 0x10) {
        0 if in_cuboid(w, id, 8) && w.hero.group == 0xf => set_pvi(w, id, 0x10, 1),
        1 if w.hero.state == 0x42 => set_pvi(w, id, 0x10, 0),
        1 if in_cuboid(w, id, 0xc) => { crate::moby_update::story::award_skill_point(w, crate::moby_update::story::skill_index(0x13_d413)); }
        _ => {}
    }
    if !owned(w, 29) {
        if in_cuboid(w, id, 0) { remind(w, 0x30, 8000, 0x30); }
    } else if in_cuboid(w, id, 0) && !flag(w, 0x2f) {
        arm_or_remind(w, 0x31, 0x1f41, 0x31);
    }
    let rails = in_cuboid(w, id, 4) && planet(w, 10);
    if !owned(w, 28) {
        if rails { remind(w, 0x32, 0x1f42, 0x32); }
    } else if rails {
        arm_or_remind(w, 0x70, 0x1f45, 0x70);
    }
    if -fl(w.hero.gravity_dir[2]) < 0.1 { w.svc.help.records.help[0x70].count = 0xffff; }
    if !flag(w, 0x2f) && w.hero.group == 0xf { set_flag(w, 0x2f); }
}
