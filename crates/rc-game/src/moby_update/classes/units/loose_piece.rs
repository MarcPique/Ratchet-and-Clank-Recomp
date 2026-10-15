//! The loose pieces of level 08 (Batalia), classes 547–551, 588–598 (placed) and 344, 782–785 (in the table): level08
//! 0x2dba40, one function for all 21 classes (census U268; 85 created instances, in groups 9 and 11 and loose). A
//! piece rests (state 2) until another moby sets it flying (state 1 with a velocity, a spin and the moby it may pass
//! through: none of the unit's own code does that); then it tumbles about an anchor point, bounces off the world and
//! other mobys at half speed, falls, fades once it has settled and is deleted below z 5. Read from the level08
//! decomp and disassembly (0x2dba40). Native `f32`.
//!
//! **Pvar block** (s32 / f32): +0x00 timer (120 in the level data), +0x04 the anchor: a joint list of the class (≥ 1)
//! or none (< 1: the offset +0x10 in the piece's frame), +0x08 the moby it does not bounce off (a moby pointer in the
//! game, set by the mover; here `index + 1`, 0 none), +0x0c the collision radius in 1/1024 units (256), +0x10 the
//! anchor offset, +0x20 / +0x24 / +0x28 spin per tick (x, y, z), +0x30 the velocity (units per tick).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2dba40 state 0 | draw distance bit 0 → mode \|= 0x8000 (the mirrored second row), → state 2 | [`update`] |
//! | state 1, anchor < 1 | `MatrixMulVec3` (L01 0x2215e0) before; Euler += spin (`fast_add_rotations` ×3); `EulerToMatrix` (0x221980); mode 0x8000: row 1 ·= −1 (`VecScale`); `MatrixCopyRows` into +0xc0; after; Δ = before − after (`VecSub`); point = after + position | [`update`] (`script::euler_rows`) |
//! | state 1, anchor ≥ 1 | joint point `0x2645a8(m, list)` before / after the same turn (no mirror) | [`update`] (`World::joint_point`) |
//! | | `coll_sphere(r/1024, point, 0, m)` (0x212960): a hit on the world or on a moby other than +0x08 → timer = 0, and when v·n < 0: `v = reflect(v, n)` (0x221570), `v ·= 0.5` | [`update`] (`World::coll_sphere`, `hero::guns::reflect`) |
//! | | `FastDecTimer(+0x00)` (0x220e78): 0 or run out → timer 0, `k = 128 / ticks(20)`: alpha −= trunc(k) while k ≤ alpha, else `DeleteMoby` | [`update`] |
//! | | v.z −= 10.8·dt²; position += v + Δ; z < 5 → `DeleteMoby` (0x2636c0) | [`update`] |
//! | | no sound, particle, light, save flag; no effect on other mobys | n/a |
//! | states 2.. | nothing | [`update`] |
//! | `0x2db990(piece, mover, v, offset, life)` | the mover's start (Batalia's tanks breaking a wall, `batalia_tank`): state 1, +0x30 = v, +0x10 = the offset, +0x00 = `ticks(life)`, spins `randf(±4π/3·dt)` ×3 (+0x08 left as it is) | [`fling`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::{add, add_rot, dot3, pi32, pv4, scale, set_pi32, set_pv4, sub};
use crate::moby_update::services::{pf, pv, World};

/// The update in the level08 class table.
pub const UPDATE_FN: u32 = 0x2d_ba40;
pub const REFERENCE_LEVEL: u32 = 8;
pub const CLASSES: [i16; 21] = [344, 547, 548, 549, 550, 551, 588, 589, 590, 591, 592, 593, 594, 595, 596, 597, 598, 782, 783, 784, 785];

/// Gravity on v.z, units per second² (the level08 constant 10.8).
pub const GRAVITY: f32 = 10.8;
/// Below this z the piece is deleted.
pub const FLOOR_Z: f32 = 5.0;

type V = [f32; 4];

/// `rows · v` (x, y, z; `MatrixMulVec3` 0x2215e0 with row 3 zero).
fn rows_mul(r: &[[f32; 4]; 4], v: V) -> V { std::array::from_fn(|l| if l == 3 { 0.0 } else { r[0][l] * v[0] + r[1][l] * v[1] + r[2][l] * v[2] }) }

/// The anchor's world point (module doc).
fn anchor(w: &World, id: MobyId, list: i32) -> V {
    if list < 1 { add(rows_mul(&w.m(id).rows, pv4(w, id, 0x10)), w.m(id).position) } else { w.joint_point(id, list as usize) }
}

/// `0x2db990(piece, mover, v, offset, life)` (module doc).
pub fn fling(w: &mut World, id: MobyId, v: [f32; 4], offset: [f32; 4], life: i32) {
    if w.m(id).pvars.len() < 0x40 { w.mm(id).pvars.resize(0x40, 0); }
    w.mm(id).state = 1;
    set_pv4(w, id, 0x30, v);
    set_pv4(w, id, 0x10, offset);
    let t = w.ticks(life);
    set_pi32(w, id, 0, t);
    let s = crate::moby_update::creature::DT * 4.188_790_3;
    for k in 0..3 {
        let r = w.rng.randf(-s, s);
        crate::moby_update::creature::set_pf(w, id, 0x20 + 4 * k, r);
    }
}

/// Level08 0x2dba40 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x40 { return; }
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            if m.draw_dist & 1 != 0 { m.mode |= mode::MIRROR; }
            m.state = 2;
        }
        1 => flying(w, id),
        _ => {}
    }
}

fn flying(w: &mut World, id: MobyId) {
    let list = pi32(w, id, 4);
    let before = anchor(w, id, list);
    let spin = pv4(w, id, 0x20);
    let m = w.mm(id);
    for (r, s) in m.rotation.iter_mut().zip(spin).take(3) { *r = add_rot(*r, s); }
    let r = crate::follow_camera::script::euler_rows([m.rotation[0], m.rotation[1], m.rotation[2]]);
    let mut rows = [[r[0][0], r[0][1], r[0][2], 0.0], [r[1][0], r[1][1], r[1][2], 0.0], [r[2][0], r[2][1], r[2][2], 0.0], m.rows[3]];
    if list < 1 && m.mode & mode::MIRROR != 0 { rows[1] = scale(rows[1], -1.0); }
    m.rows = rows;
    let point = anchor(w, id, list);
    let delta = sub(before, point);
    let radius = pi32(w, id, 0xc) as f32 * (1.0 / 1024.0);
    if let Some(h) = w.coll_sphere(pv(point), pf(radius), 0, Some(id)) {
        let passes = pi32(w, id, 8);
        if h.moby.is_none_or(|o| o as i32 + 1 != passes) {
            set_pi32(w, id, 0, 0);
            let v = pv4(w, id, 0x30);
            let n = [h.normal[0], h.normal[1], h.normal[2], 0.0];
            if dot3(v, n) < 0.0 {
                let r = crate::hero::guns::reflect([v[0], v[1], v[2]], h.normal);
                set_pv4(w, id, 0x30, scale([r[0], r[1], r[2], v[3]], 0.5));
            }
        }
    }
    let mut t = pi32(w, id, 0);
    let fired = crate::moby_update::creature::dec_timer_i32(&mut t);
    set_pi32(w, id, 0, t);
    if fired != 0 {
        let n = w.ticks(20);
        set_pi32(w, id, 0, 0);
        let k = 128.0 / n as f32;
        let a = w.m(id).alpha;
        if k <= a as f32 { w.mm(id).alpha = a.wrapping_sub(k as i32 as u8); } else { w.delete_moby(id); }
    }
    let mut v = pv4(w, id, 0x30);
    v[2] -= crate::moby_update::creature::DT2 * GRAVITY;
    set_pv4(w, id, 0x30, v);
    let m = w.mm(id);
    m.position = add(add(m.position, v), delta);
    if m.position[2] < FLOOR_Z { w.delete_moby(id); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::services::pvar as p;

    #[test]
    fn rests_then_flies_fades_and_drops_out() {
        let mut m = Moby { o_class: 547, draw_dist: 255, alpha: 0x80, position: [0.0, 0.0, 50.0, 1.0], pvars: vec![0; 0x40], ..Moby::default() };
        p::set_i32(&mut m.pvars, 0, 3);
        p::set_i32(&mut m.pvars, 4, -1);
        p::set_i32(&mut m.pvars, 0xc, 256);
        let mut t = MobyTable::new(vec![m], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        update(&mut w, 0);
        assert_eq!((w.m(0).state, w.m(0).mode & mode::MIRROR), (2, mode::MIRROR));
        update(&mut w, 0);
        assert_eq!(w.m(0).position[2], 50.0, "resting");
        // Set loose (the mover's part): a velocity and a spin.
        w.mm(0).state = 1;
        p::set_v4f(&mut w.mm(0).pvars, 0x30, [0.1, 0.0, 0.0, 0.0]);
        p::set_v4f(&mut w.mm(0).pvars, 0x20, [0.0, 0.0, 0.05, 0.0]);
        update(&mut w, 0);
        let g = crate::moby_update::creature::DT2 * GRAVITY;
        assert_eq!(w.m(0).position, [0.1, 0.0, 50.0 - g, 1.0]);
        assert_eq!((w.m(0).rotation[2], p::i32(&w.m(0).pvars, 0), w.m(0).alpha), (0.05, 2, 0x80));
        assert_eq!(w.m(0).rows[1][1], -(0.05f32.cos()), "the mirrored second row");
        update(&mut w, 0);
        update(&mut w, 0);
        // The timer ran out: fade by trunc(128 / 20) = 6 per tick.
        assert_eq!(w.m(0).alpha, 0x80 - 6);
        w.mm(0).position[2] = 5.0;
        update(&mut w, 0);
        assert!(w.m(0).state >= 0x80, "deleted below z 5");
    }
}
