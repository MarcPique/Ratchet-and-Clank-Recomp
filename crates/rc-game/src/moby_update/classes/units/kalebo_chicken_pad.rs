//! **Kalebo's chicken pad, class 1923** (level16 `0x2e8970`, 1 placed; census U570; the name is descriptive [L]):
//! while Ratchet is in its cuboid and more than 3 from its teleporter pad (moby #1192, class 1135), every
//! `rand_range(180, 360)` ticks it takes a hidden chicken (class 270, burst and waiting in state 6) of its moby group,
//! opens the pad (state 7: the arms, then the beam, `classes::teleporter`) and, once the beam has grown past 0.2, sets
//! the chicken down on the ground there (`chicken::respawn_at`). Read from the level16 decomp.
//!
//! **Pvars**: +0x00 the chickens' group (86), +0x04 the pad (a moby index), +0x08 the cuboid, +0x0c the timer.
//!
//! | address | what | port |
//! |---|---|---|
//! | every tick | Ratchet (0x13f3d0) not in the cuboid (`PointInCuboid` 0x25b258) → nothing | [`update`] |
//! | state 0 | → 1; the timer | [`update`] |
//! | state 1 | `vec_distance2(Ratchet, pad)` > 3, a chicken (`0x2e8b08`: the first of the group's list `0x1abcc0[group]` with class 0x10e and state 6, `0x2c4710`) and `FastDecTimer(+0x0c)` ≠ 0 → 2, `0x2e0de0(pad)` (pad in state 1 → `PlayClassSound(0, 0, pad)`, state 7); the timer | [`update`] |
//! | state 2 | `0x2e0e28(pad)`: state 7 / 8 → 1, or 2 in state 8 with the beam (pad +0x34) past 0.2; else 0. 0 → 1; 2 → 1 and the chicken (again `0x2e8b08`) set down at the pad: z + 1, `GroundHeight(0.5, ·, 0)` (0x254fd0), `0x2c44f0` | [`update`] |
//! | the timer | +0x0c = `ticks(rand_range(180, 360))` | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::chicken;
use crate::moby_update::creature as c;
use crate::moby_update::services::{self as sv, World};

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2e_8970;
pub const CLASSES: [i16; 1] = [1923];
/// The pad's class.
pub const PAD: i16 = 1135;
/// The wait between chickens.
pub const WAIT: (i32, i32) = (180, 360);

fn pad(w: &World, id: MobyId) -> Option<MobyId> {
    usize::try_from(c::pi32(w, id, 4)).ok().filter(|&p| p < w.table.mobys.len())
}

/// `0x2e8b08(group)`: the first chicken of the group's list waiting in state 6.
fn chicken_of(w: &World, group: i32) -> Option<MobyId> {
    let list = usize::try_from(group).ok().and_then(|g| w.svc.groups.lists.get(g).cloned().flatten())?;
    list.into_iter().map(usize::from).find(|&k| k < w.table.mobys.len() && w.m(k).o_class == chicken::CLASS && chicken::available(w, k))
}

fn arm(w: &mut World, id: MobyId) {
    let n = w.rng.rand_range(WAIT.0, WAIT.1);
    let t = w.ticks(n);
    c::set_pi32(w, id, 0xc, t);
}

/// Level16 `0x2e8970` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x10 { return; }
    if !w.in_cuboid(w.hero_point(), c::pi32(w, id, 8)) { return; }
    let Some(p) = pad(w, id) else { return };
    match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            arm(w, id);
        }
        1 => {
            if c::dist2(super::hero_pos(w), w.m(p).position) <= 3.0 { return; }
            if chicken_of(w, c::pi32(w, id, 0)).is_none() { return; }
            if c::dec_timer_pvar_i32(w, id, 0xc) == 0 { return; }
            w.mm(id).state = 2;
            if w.m(p).state == 1 {
                w.play_sound(0, 0, p);
                w.mm(p).state = 7;
            }
            arm(w, id);
        }
        2 => {
            let ps = w.m(p).state;
            let r = if ps == 7 || ps == 8 {
                if ps == 8 && 0.2 < c::pf(w, p, 0x34) { 2 } else { 1 }
            } else {
                0
            };
            if r == 1 { return; }
            w.mm(id).state = 1;
            if r == 0 { return; }
            let Some(ch) = chicken_of(w, c::pi32(w, id, 0)) else { return };
            let pp = w.m(p).position;
            let mut at = [pp[0], pp[1], pp[2] + 1.0, pp[3]];
            at[2] = sv::fl(w.ground_height(sv::pf(0.5), sv::pv(at), 0));
            chicken::respawn_at(w, ch, at);
        }
        _ => {}
    }
}
