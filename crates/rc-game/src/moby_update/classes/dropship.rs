//! The Blarg dropship 666 (`DropshipUpdate` 0x2f4960, level01 only): carries up to eight mobys (pvar+0x150..0x16c;
//! on Novalis three robot troopers 459) and drops them where its path says. Spec `docs/plan/creatures.md`
//! "Dropship 666".
//!
//! Without a trigger cuboid (P+0x170 = −1) it only holds sequence 5 (the parked ship). Otherwise its own state P+0x174:
//! 0 → 1 (hidden, update distance 0xff); in 1 it waits, hidden, for Ratchet to enter the cuboid, carrying its load at
//! its placed position; then (2) it shows up and flies its path with the flyer driver ([`flyer::driver`]), and on
//! reaching the drop point P+0x17c (the path's first point with w = 1) opens up (sequence 2, P+0x178 = 1). Each tick
//! the load rides with it: child k at `rows · (−1 + k/2, 0, 1.75)` from the ship, the ship's Euler turned by π, its
//! animation advanced and matrix built here, mode 6 (no update of its own, keep the matrix); shown once the ship
//! flies. While open (and once sequence 2 is fully in, +0x52), every `rand_range(20, 30)` ticks the next child is
//! released: mode 6 cleared, upright and facing Ratchet; a trooper 459 gets +0x1e8 = 1 (woken), +0x1ec = 30 ticks
//! and +0x1d0 = the ship's velocity (the flyer's next curve point − position), so it glides from the ship to its drop
//! cuboid ([`super::path_enemy`] state 3); classes 427 / 340 (other levels) get state 4 / 2. With nothing left it
//! closes (sequence 0). Two points before the path's end it is deleted (unless P+0x184 is set).
//!
//! Not ported (counted): the camera moby P+0x180 (−1 on Novalis: it would get a timer and a yaw that follows the ship).

use super::flyer;
use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2f4960;
pub const CLASSES: [i16; 1] = [666];

/// `0x20b360`: the load's seats in the ship's frame.
pub const SEATS: [[f32; 4]; 8] = [
    [-1.0, 0.0, 1.75, 0.0],
    [-0.5, 0.0, 1.75, 0.0],
    [0.0, 0.0, 1.75, 0.0],
    [0.5, 0.0, 1.75, 0.0],
    [1.0, 0.0, 1.75, 0.0],
    [1.5, 0.0, 1.75, 0.0],
    [2.0, 0.0, 1.75, 0.0],
    [2.5, 0.0, 1.75, 0.0],
];

const P_NODE: usize = 0x60;
const P_SPLINE: usize = 0x74;
const P_CUR: usize = 0xd0;
const P_LOAD: usize = 0x150;
const P_TRIGGER: usize = 0x170;
const P_STATE: usize = 0x174;
const P_DROP_T: usize = 0x176;
const P_OPEN: usize = 0x178;
const P_DROP_NODE: usize = 0x17c;
const P_CAMERA: usize = 0x180;
const P_KEEP: usize = 0x184;
const P_194: usize = 0x194;

fn blend_if(w: &mut World, id: MobyId, seq: u8, t: i32) {
    if w.m(id).anim.seq_b != seq {
        let t = if t == 0 { 0 } else { w.ticks(t) };
        w.anim_blend(id, seq, 0, t);
    }
}

fn spline(w: &World, id: MobyId) -> Option<usize> {
    usize::try_from(c::pi32(w, id, P_SPLINE)).ok().filter(|&s| s < w.svc.splines.len())
}

/// `DropshipUpdate` 0x2f4960 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x198 { return; }
    if c::pi32(w, id, P_TRIGGER) == -1 {
        blend_if(w, id, 5, 0);
        return;
    }
    match c::pi16(w, id, P_STATE) {
        0 => {
            c::set_pi32(w, id, P_194, -1);
            c::set_pi16(w, id, P_STATE, 1);
            c::set_pi32(w, id, P_KEEP, 0);
            c::set_pi32(w, id, P_OPEN, 0);
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.mode |= 0x41;
            return;
        }
        1 => {
            let h = w.hero.pos.map(|x| f32::from_bits(x.0));
            if w.in_cuboid([h[0], h[1], h[2]], c::pi32(w, id, P_TRIGGER)) {
                c::set_pi16(w, id, P_STATE, 2);
                w.mm(id).mode &= !0x41;
                if c::pi32(w, id, P_CAMERA) != -1 { w.svc.unported("dropship 666: camera moby +0x180"); }
                if let Some(s) = spline(w, id) {
                    let pts = &w.svc.splines[s];
                    if let Some(k) = pts.iter().position(|p| f32::from_bits(p[3]) == 1.0) { c::set_pi32(w, id, P_DROP_NODE, k as i32); }
                }
            }
        }
        2 => {
            flyer::driver(w, id);
            if c::pi32(w, id, P_CAMERA) != -1 { w.svc.unported("dropship 666: camera moby +0x180"); }
            if let Some(s) = spline(w, id) {
                let node = c::pi32(w, id, P_NODE);
                let skip = node == c::pi32(w, id, P_DROP_NODE) && c::pi32(w, id, P_OPEN) == 0;
                if skip {
                    let t = w.ticks(0x14);
                    if w.m(id).anim.seq_b != 2 { w.anim_blend(id, 2, 0, t); }
                    c::set_pi16(w, id, P_DROP_T, 0);
                    c::set_pi32(w, id, P_OPEN, 1);
                } else if node == w.svc.splines[s].len() as i32 - 2 && c::pi32(w, id, P_KEEP) == 0 {
                    w.delete_moby(id);
                    return;
                }
            }
        }
        _ => {}
    }
    c::dec_timer_pvar_s16(w, id, P_DROP_T);
    if c::pi32(w, id, P_OPEN) == 0 || c::pi16(w, id, P_DROP_T) != 0 || w.m(id).anim.seq_a != 2 {
        carry(w, id);
    } else {
        release(w, id);
    }
}

/// The load rides with the ship (module doc).
fn carry(w: &mut World, id: MobyId) {
    let flying = c::pi16(w, id, P_STATE) != 1;
    let (pos, rot, rows) = { let m = w.m(id); (m.position, m.rotation, m.rows) };
    for (k, s) in SEATS.iter().enumerate() {
        let Some(ch) = usize::try_from(c::pi32(w, id, P_LOAD + 4 * k)).ok().filter(|&m| m < w.table.mobys.len()) else { continue };
        if flying { w.mm(ch).mode &= !0x41; }
        let off: [f32; 4] = std::array::from_fn(|l| if l == 3 { 0.0 } else { rows[0][l] * s[0] + rows[1][l] * s[1] + rows[2][l] * s[2] });
        {
            let m = w.mm(ch);
            m.position = c::add(pos, off);
            m.rotation = rot;
            m.rotation[2] = c::add_rot(rot[2], std::f32::consts::PI);
        }
        // MobyAnimAdvance(child), its sound triggers and loop included (crate::moby_update::anim_sound).
        crate::moby_update::anim_sound::advance(w, ch);
        w.build_matrix(ch);
        w.mm(ch).mode |= 6;
    }
}

/// The next child is released (module doc); `rand_range(20, 30)` ticks to the next.
fn release(w: &mut World, id: MobyId) {
    for k in 0..8 {
        let o = P_LOAD + 4 * k;
        let Some(ch) = usize::try_from(c::pi32(w, id, o)).ok().filter(|&m| m < w.table.mobys.len()) else { continue };
        let ship = c::pos(w, id);
        let h = w.hero.pos.map(|x| f32::from_bits(x.0));
        {
            let m = w.mm(ch);
            m.mode &= !6;
            m.rotation = [0.0; 4];
            m.rotation[2] = c::atan(h[0] - ship[0], h[1] - ship[1]);
        }
        match w.m(ch).o_class {
            0x1ab => w.mm(ch).state = 4,
            0x154 => w.mm(ch).state = 2,
            0x1cb if w.m(ch).pvars.len() >= 0x1f0 => {
                c::set_pi32(w, ch, 0x1e8, 1);
                let t = w.ticks(0x1e);
                c::set_pi16(w, ch, 0x1ec, t as i16);
                let v = c::sub(c::pv4(w, id, P_CUR), ship);
                c::set_pv4(w, ch, 0x1d0, v);
            }
            _ => {}
        }
        c::set_pi32(w, id, o, -1);
        let t = w.rng.rand_range(0x14, 0x1e);
        c::set_pi16(w, id, P_DROP_T, t as i16);
        return;
    }
    if w.m(id).anim.seq_b != 0 {
        let t = w.ticks(0x14);
        w.anim_blend(id, 0, 0, t);
    }
}
