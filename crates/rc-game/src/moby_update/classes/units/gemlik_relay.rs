//! **Gemlik's state relays** (level 13; read from the level13 decomp; native):
//!
//! * **667** (`0x2f8880`, census U470, one placed): it watches the moby +0x00. Starting with +0x1d set it waits for
//!   the watched state to enter set B (+0x0a..+0x0f), else for set A (+0x04..+0x09); entering B sets the moby +0x10's
//!   state / command to +0x15 / +0x17 and its own to +0x19 / +0x1b, then waits for A; entering A sets +0x14 / +0x16
//!   and +0x18 / +0x1a, then waits for B. −1 entries skip. No watched moby: a debug print and deleted.
//! * **674, 677, 680** (`0x2f8a78`, U471, 1 + 1 + 2 placed): a field (alpha 20 × its fade +0x10, mode | 0xa08, its
//!   class texture scrolling 0xc0 a tick) that is up (with its collision) while the watched moby +0x00's state is in
//!   set A (+0x04..+0x09) and goes once it is in set B (+0x0a..+0x0f), fading 0.05 a tick either way; it starts down
//!   with +0x18 set. No watched moby: deleted.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2f8880` | 667 | [`relay_update`] |
//! | `0x2f8a78` | 674 / 677 / 680 | [`field_update`] |
//!
//! [L] The texture scroll (`0x25ac38`) is kept in +0x14 only (no moby texture scroll in the renderer).

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature as c;
use crate::moby_update::services::World;
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 13;
pub const RELAY_FN: u32 = 0x2f_8880;
pub const RELAY_CLASSES: [i16; 1] = [667];
pub const FIELD_FN: u32 = 0x2f_8a78;
pub const FIELD_CLASSES: [i16; 3] = [674, 677, 680];

fn watched(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, 0)).ok().filter(|&m| m < w.table.mobys.len()) }
/// The watched state in the six bytes at `o`.
fn in_set(w: &World, id: MobyId, m: MobyId, o: usize) -> bool {
    let s = w.m(m).state as i32;
    (0..6).any(|k| c::pu8(w, id, o + k) as i8 as i32 == s)
}
fn put(w: &mut World, m: MobyId, state: u8, cmd: u8) {
    if state != 0xff { w.mm(m).state = state; }
    if cmd != 0xff { w.mm(m).cmd = cmd; }
}

/// Level13 `0x2f8880`: 667 (module doc).
pub fn relay_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x20);
    let Some(m) = watched(w, id) else {
        if c::pu8(w, id, 0x1c) == 0 { w.delete_moby(id); }
        return;
    };
    let target = usize::try_from(c::pi32(w, id, 0x10)).ok().filter(|&t| t < w.table.mobys.len());
    match c::pu8(w, id, 0x1c) {
        0 => {
            let next = if c::pu8(w, id, 0x1d) != 0 { 1 } else { 2 };
            c::set_pu8(w, id, 0x1c, next);
        }
        1 => {
            if !in_set(w, id, m, 0x0a) { return; }
            let (ts, tc, os, oc) = (c::pu8(w, id, 0x15), c::pu8(w, id, 0x17), c::pu8(w, id, 0x19), c::pu8(w, id, 0x1b));
            if let Some(t) = target { put(w, t, ts, tc); }
            put(w, id, os, oc);
            c::set_pu8(w, id, 0x1c, 2);
        }
        2 => {
            if !in_set(w, id, m, 0x04) { return; }
            let (ts, tc, os, oc) = (c::pu8(w, id, 0x14), c::pu8(w, id, 0x16), c::pu8(w, id, 0x18), c::pu8(w, id, 0x1a));
            if let Some(t) = target { put(w, t, ts, tc); }
            put(w, id, os, oc);
            c::set_pu8(w, id, 0x1c, 1);
        }
        _ => {}
    }
}

/// Level13 `0x2f8a78`: 674 / 677 / 680 (module doc).
pub fn field_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x20);
    let m = watched(w, id);
    let st = w.m(id).state;
    let (a, b) = match m {
        Some(m) if st != 0 => (in_set(w, id, m, 0x04), in_set(w, id, m, 0x0a)),
        _ => (false, false),
    };
    let coll = super::class_collision(w, w.m(id).o_class);
    match st {
        0 => {
            if m.is_none() {
                w.delete_moby(id);
                return;
            }
            let mm = w.mm(id);
            mm.ambient = [0x80, 0x80, 0x80, 0];
            mm.mode |= 0xa08;
            if c::pi32(w, id, 0x18) == 0 {
                c::set_pf(w, id, 0x10, 1.0);
                let mm = w.mm(id);
                mm.state = 1;
                mm.alpha = 0x14;
                mm.has_collision = coll;
            } else {
                c::set_pf(w, id, 0x10, 0.0);
                let mm = w.mm(id);
                mm.state = 3;
                mm.has_collision = false;
                mm.alpha = 0;
            }
        }
        1 => {
            if b {
                let mm = w.mm(id);
                mm.has_collision = false;
                mm.state = 2;
            }
        }
        2 => {
            if a {
                let mm = w.mm(id);
                mm.state = 4;
                mm.has_collision = coll;
            }
            let f = c::pf(w, id, 0x10) - c::SPEED * 0.05;
            c::set_pf(w, id, 0x10, f);
            if f < 0.0 {
                c::set_pf(w, id, 0x10, 0.0);
                let mm = w.mm(id);
                mm.state = 3;
                mm.has_collision = false;
                mm.visible = 0;
                mm.mode |= mode::HIDDEN;
            }
        }
        3 => {
            if a {
                let mm = w.mm(id);
                mm.state = 4;
                mm.visible = 1;
                mm.has_collision = coll;
                mm.mode &= !mode::HIDDEN;
            }
        }
        4 => {
            if b {
                let mm = w.mm(id);
                mm.has_collision = false;
                mm.state = 2;
            }
            let f = c::pf(w, id, 0x10) + c::SPEED * 0.05;
            c::set_pf(w, id, 0x10, f);
            if 1.0 < f {
                c::set_pf(w, id, 0x10, 1.0);
                let mm = w.mm(id);
                mm.state = 1;
                mm.has_collision = coll;
            }
        }
        _ => {}
    }
    w.mm(id).alpha = (c::pf(w, id, 0x10) * 20.0) as i32 as u8;
    let mut s = c::pi32(w, id, 0x14) + 0xc0;
    if 0x1000 < s { s -= 0x1000; } else if s < 0 { s += 0x1000; }
    c::set_pi32(w, id, 0x14, s);
}
