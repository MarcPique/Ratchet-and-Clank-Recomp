//! **Gemlik's state triggers, class 6** (level13 `0x2b0e80`; census U448; 4 placed). Ratchet (his feet, 0x13f3d0)
//! stepping into the cuboid +0x00 sets up to four mobys' states (+0x04..+0x10, to +0x24..+0x27) and four groups'
//! (+0x14..+0x20, to +0x2c..+0x2f, `0x265050`), then its own state (+0x34) and command (+0x36); stepping out, the
//! same with +0x28..+0x2b, +0x30..+0x33, +0x35 and +0x37. A −1 skips a slot; a deleted moby is left alone. +0x38 is
//! "inside". No cuboid: a debug print and deleted.
//!
//! Read from the level13 decomp and disassembly (the group calls' arguments). Native.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::scheduler::group_state;
use crate::moby_update::services::World;
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 13;
pub const UPDATE_FN: u32 = 0x2b_0e80;
pub const CLASSES: [i16; 1] = [6];

const INSIDE: usize = 0x38;

/// One side's writes: mobys' states at `ms`, groups' at `gs`, its own state / command at `own` / `own + 2`.
fn apply(w: &mut World, id: MobyId, ms: usize, gs: usize, own: usize) {
    for k in 0..4 {
        let m = c::pi32(w, id, 0x04 + 4 * k);
        let s = c::pu8(w, id, ms + k);
        if let Some(m) = usize::try_from(m).ok().filter(|&m| m < w.table.mobys.len()) {
            let st = w.m(m).state;
            if st != 0xfe && st != 0xfd && s != 0xff { w.mm(m).state = s; }
        }
        let g = c::pi32(w, id, 0x14 + 4 * k);
        let gs_ = c::pu8(w, id, gs + k);
        if g != -1 && gs_ != 0xff {
            if let Ok(g) = i8::try_from(g) { group_state(w, g, gs_); }
        }
    }
    let s = c::pu8(w, id, own);
    if s != 0xff { w.mm(id).state = s; }
    let cmd = c::pu8(w, id, own + 2);
    if cmd != 0xff { w.mm(id).cmd = cmd; }
}

/// Level13 `0x2b0e80` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x3c);
    let cub = c::pi32(w, id, 0);
    if cub == -1 {
        w.delete_moby(id);
        return;
    }
    let h = super::hero_pos(w);
    let inside = c::pu8(w, id, INSIDE);
    if w.in_cuboid([h[0], h[1], h[2]], cub) {
        if inside != 0 { return; }
        apply(w, id, 0x24, 0x2c, 0x34);
        c::set_pu8(w, id, INSIDE, 1);
    } else {
        if inside != 1 { return; }
        apply(w, id, 0x28, 0x30, 0x35);
        c::set_pu8(w, id, INSIDE, 0);
    }
}
