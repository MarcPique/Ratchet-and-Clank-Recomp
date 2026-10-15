//! Three small carriers whose whole update is the platform system (G-CLS-024, `triggers`' moby side): each publishes
//! its move to its platform block (pvar +0x20, `CarryRiders` L01 0x2755f8) so that Ratchet and the mobys on it ride
//! along, and the turntables and the pinned platforms **turn** (the general `CarryRiders`, `triggers::carry_riders`).
//! Read from the disassembly of each level's copy. The names are descriptive [L]. Native `f32`.
//!
//! **U102, the turntables 707 / 734** (level02 0x2ddc00 and 0x2df8c0, the same code: 4 created): every tick the yaw
//! +0x48 turns by `5°/s · dt` (`fast_add_rotations(yaw, dt · 0x3db2b8c2)`), then `CarryRiders(+0x20, 0, rot_old,
//! rot)` (the displacement is a zeroed stack vector).
//!
//! **U126, the joint-carried platform 1210** (level03 0x2953f8: 5 created): skipped while mode bit 2 is set. In state
//! 0 (the class never leaves it; another moby's code would): +0x60 = 0 (u32), +0x64 = 0 (u16), +0x68 = 4, +0x7e = 13
//! (u16), +0xc4 = the distance from the position to joint list 0's point (`FUN_002645a8`), +0xa0 = position,
//! +0xb0 = rotation, the block flags +0x5c = 3, update distance 0xff, draw distance 0xff. Every tick: `CarryRiders(
//! +0x20, pos − +0xa0, +0xb0, rot)`, then +0xa0 / +0xb0 = position / rotation. (In state 0 the init re-records the
//! pose first, so the carry is zero until something moves it between the two.)
//!
//! **U179, the pinned platforms 812** (level05 0x30bf98: 25 created): state 0: block flags +0x5c = 1; with a carrier
//! index +0x80 ≥ 0: `FUN_00275528(m, carrier, pos, rot, +0x60, +0x70)` (its pose in the carrier's frame), state 1.
//! State 1: `FUN_002753b0(m, carrier, +0x60, +0x70, pos, rot)` (the pose back through the carrier's rows now: it
//! follows the carrier, turns with it and takes the block's update-order correction), then `CarryRiders(+0x20, pos −
//! old pos, old rot, rot)`: a platform pinned to a platform, carrying riders of its own. A carrier without a block
//! leaves the pose (the functions return 0 without writing).
//!
//! | address | what | port |
//! |---|---|---|
//! | L02 0x2ddc00 / 0x2df8c0 | yaw += dt·5° (`fast_add_rotations`), `CarryRiders(+0x20, 0, old, new)` | [`turntable`] |
//! | L03 0x2953f8 | mode & 2 → nothing; state 0 init (the words above, joint 0 distance `0x23e338` = L01 0x2645a8, `VecDistance`) | [`joint_platform`] |
//! | | `CarryRiders(+0x20, pos − +0xa0, +0xb0, rot)`, +0xa0 / +0xb0 = pose | [`joint_platform`] |
//! | L05 0x30bf98 state 0 | +0x5c = 1; +0x80 ≥ 0 → `0x28a6f0` (= L01 0x275528), state 1 | [`pinned_platform`] (`triggers::to_local`) |
//! | state 1 | `0x28a578` (= L01 0x2753b0), `VecSub`, `0x28a7c0` (= L01 0x2755f8) | [`pinned_platform`] (`triggers::from_local`, `carry_riders`) |
//! | all three | no sound, particle, light, hit, other moby written, save flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, add_rot, DT};
use crate::moby_update::services::World;
use crate::moby_update::triggers;

pub const REFERENCE_LEVEL_TURNTABLE: u32 = 2;
pub const TURNTABLE_FN: u32 = 0x2d_dc00;
pub const TURNTABLE_CLASSES: [i16; 1] = [707];
/// Class 734 runs the same code at another address of the level02 overlay (0x2df8c0; the relocator maps a function to
/// one copy per overlay, so it has its own row).
pub const TURNTABLE_FN_734: u32 = 0x2d_f8c0;
pub const TURNTABLE_CLASSES_734: [i16; 1] = [734];
pub const REFERENCE_LEVEL_JOINT: u32 = 3;
pub const JOINT_FN: u32 = 0x29_53f8;
pub const JOINT_CLASSES: [i16; 1] = [1210];
pub const REFERENCE_LEVEL_PINNED: u32 = 5;
pub const PINNED_FN: u32 = 0x30_bf98;
pub const PINNED_CLASSES: [i16; 1] = [812];

/// The platform block of all three (pvar +0x20; its flags word is +0x5c).
pub const BLOCK: usize = 0x20;
/// The turntables' speed (`0x3db2b8c2` = 5° in radians, per second).
pub const TURN_SPEED: f32 = f32::from_bits(0x3db2_b8c2);

fn rot3(r: [f32; 4]) -> [f32; 3] { [r[0], r[1], r[2]] }

/// Level02 0x2ddc00 (module doc).
pub fn turntable(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < BLOCK + 0x40 { return; }
    let old = w.m(id).rotation;
    let yaw = add_rot(old[2], DT * TURN_SPEED);
    w.mm(id).rotation[2] = yaw;
    let new = w.m(id).rotation;
    triggers::carry_riders(&mut w.mm(id).pvars, BLOCK, [0.0; 4], old, new);
}

/// Level03 0x2953f8 (module doc).
pub fn joint_platform(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xc8 || w.m(id).mode & crate::moby_runtime::mode::NO_UPDATE != 0 { return; }
    if w.m(id).state == 0 {
        let j = w.joint_point(id, 0);
        let d = c::dist3(c::pos(w, id), j);
        let (p, r) = (c::pos(w, id), w.m(id).rotation);
        let m = w.mm(id);
        let pv = &mut m.pvars;
        pv[0x60..0x64].fill(0);
        pv[0x64..0x66].fill(0);
        pv[0x68] = 4;
        pv[0x7e..0x80].copy_from_slice(&13u16.to_le_bytes());
        c::set_pf(w, id, 0xc4, d);
        c::set_pv4(w, id, 0xa0, p);
        c::set_pv4(w, id, 0xb0, r);
        c::set_pi32(w, id, BLOCK + 0x3c, 3);
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
    }
    let d = c::sub(c::pos(w, id), c::pv4(w, id, 0xa0));
    let (old, new) = (c::pv4(w, id, 0xb0), w.m(id).rotation);
    triggers::carry_riders(&mut w.mm(id).pvars, BLOCK, d, old, new);
    let (p, r) = (c::pos(w, id), w.m(id).rotation);
    c::set_pv4(w, id, 0xa0, p);
    c::set_pv4(w, id, 0xb0, r);
}

/// The carrier a pinned platform follows (+0x80, a moby index; −1 none).
fn pinned_carrier(w: &World, id: MobyId) -> Option<triggers::Carrier> {
    let i = usize::try_from(c::pi32(w, id, 0x80)).ok()?;
    w.table.mobys.get(i).and_then(triggers::carrier)
}

/// Level05 0x30bf98 (module doc).
pub fn pinned_platform(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x84 { return; }
    match w.m(id).state {
        0 => {
            c::set_pi32(w, id, BLOCK + 0x3c, 1);
            if c::pi32(w, id, 0x80) >= 0 {
                if let Some(k) = pinned_carrier(w, id) {
                    let (l, lr) = triggers::to_local(&k, rot3(c::pos(w, id)), rot3(w.m(id).rotation));
                    c::set_pv4(w, id, 0x60, [l[0], l[1], l[2], c::pf(w, id, 0x6c)]);
                    c::set_pv4(w, id, 0x70, [lr[0], lr[1], lr[2], c::pf(w, id, 0x7c)]);
                }
                w.mm(id).state = 1;
            }
        }
        1 => {
            let (old_p, old_r) = (c::pos(w, id), w.m(id).rotation);
            if let Some(k) = pinned_carrier(w, id) {
                let slot = w.m(id).class_slot;
                let (mut l, mut lr) = (rot3(c::pv4(w, id, 0x60)), rot3(c::pv4(w, id, 0x70)));
                let (p, r) = triggers::from_local(&k, slot, &mut l, &mut lr);
                c::set_pv4(w, id, 0x60, [l[0], l[1], l[2], c::pf(w, id, 0x6c)]);
                c::set_pv4(w, id, 0x70, [lr[0], lr[1], lr[2], c::pf(w, id, 0x7c)]);
                let m = w.mm(id);
                m.position[..3].copy_from_slice(&p);
                m.rotation[..3].copy_from_slice(&r);
            }
            let d = c::sub(c::pos(w, id), old_p);
            let new = w.m(id).rotation;
            triggers::carry_riders(&mut w.mm(id).pvars, BLOCK, d, old_r, new);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::services::{pvar as p, Services};

    fn carrier_moby(o_class: i16, pos: [f32; 3], yaw: f32) -> Moby {
        let mut m = Moby { o_class, pvars: vec![0; 0xd0], position: [pos[0], pos[1], pos[2], 1.0], ..Moby::default() };
        m.mode |= 0x20;
        p::set_i32(&mut m.pvars, 8, BLOCK as i32);
        m.rotation[2] = yaw;
        let r = triggers::euler_matrix([0.0, 0.0, yaw]);
        for (row, src) in m.rows.iter_mut().zip(&r) { for (x, y) in row.iter_mut().zip(src) { *x = *y as f32; } }
        m
    }

    fn run(t: &mut MobyTable, f: fn(&mut World, MobyId), ids: &[MobyId], ticks: u32) {
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = Services::new();
        for k in 0..ticks {
            let mut w = World::new(t, &hero, &mut rng, &classes, &mut svc, k as u64);
            for &id in ids { f(&mut w, id); }
        }
    }

    #[test]
    fn turntable_turns_its_riders() {
        let mut t = MobyTable::new(vec![carrier_moby(707, [10.0, 0.0, 0.0], 0.3)], 4);
        run(&mut t, turntable, &[0], 1);
        let m = &t.mobys[0];
        let step = DT * TURN_SPEED;
        assert_eq!(m.rotation[2], add_rot(0.3, step));
        let d = triggers::platform_delta(m).unwrap();
        assert_eq!(&d.displacement, &[0.0; 4]);
        assert!((d.rotation[2] - step).abs() < 1e-6 && d.rotation[0].abs() < 1e-6 && d.rotation[1].abs() < 1e-6, "{d:?}");
        // A point 2 units out on +x rides round the centre by the same angle.
        let mut m2 = m.clone();
        let rows = triggers::euler_matrix([0.0, 0.0, m.rotation[2]]);
        for (row, src) in m2.rows.iter_mut().zip(&rows) { for (x, y) in row.iter_mut().zip(src) { *x = *y as f32; } }
        let q = triggers::carry_point(&m2, [12.0, 0.0, 0.0]).unwrap();
        assert!((q[0] - (10.0 + 2.0 * step.cos())).abs() < 1e-5 && (q[1] - 2.0 * step.sin()).abs() < 1e-5, "{q:?}");
    }

    #[test]
    fn joint_platform_publishes_its_move_and_skips_with_mode_2() {
        let mut m = carrier_moby(1210, [1.0, 2.0, 3.0], 0.0);
        m.state = 1;
        p::set_v4f(&mut m.pvars, 0xa0, [0.5, 2.0, 3.0, 1.0]);
        let mut t = MobyTable::new(vec![m], 4);
        run(&mut t, joint_platform, &[0], 1);
        let d = triggers::platform_delta(&t.mobys[0]).unwrap();
        assert_eq!(d.displacement[..3], [0.5, 0.0, 0.0]);
        assert_eq!(p::v4f(&t.mobys[0].pvars, 0xa0)[..3], [1.0, 2.0, 3.0]);
        // State 0: the init re-records the pose first (zero carry) and sets the words.
        t.mobys[0].state = 0;
        t.mobys[0].position[0] = 4.0;
        run(&mut t, joint_platform, &[0], 1);
        let m = &t.mobys[0];
        assert_eq!(triggers::platform_delta(m).unwrap().displacement[..3], [0.0; 3]);
        assert_eq!((p::i32(&m.pvars, 0x5c), m.pvars[0x68], p::u16(&m.pvars, 0x7e), m.update_dist, m.draw_dist), (3, 4, 13, 0xff, 0xff));
        t.mobys[0].mode |= crate::moby_runtime::mode::NO_UPDATE;
        t.mobys[0].position[0] = 9.0;
        run(&mut t, joint_platform, &[0], 1);
        assert_eq!(p::v4f(&t.mobys[0].pvars, 0xa0)[0], 4.0, "mode bit 2: nothing");
    }

    #[test]
    fn pinned_platform_follows_and_turns_with_its_carrier() {
        // Carrier 0 at the origin, the pinned platform 1 at (3, 0, 1) pinned to it.
        let mut k = carrier_moby(707, [0.0, 0.0, 0.0], 0.0);
        k.class_slot = 5;
        let mut m = carrier_moby(812, [3.0, 0.0, 1.0], 0.0);
        p::set_i32(&mut m.pvars, 0x80, 0);
        m.class_slot = 9;
        let mut t = MobyTable::new(vec![k, m], 4);
        run(&mut t, pinned_platform, &[1], 1);
        assert_eq!(t.mobys[1].state, 1);
        assert_eq!(p::i32(&t.mobys[1].pvars, 0x5c), 1);
        assert_eq!(p::v4f(&t.mobys[1].pvars, 0x60)[..3], [3.0, 0.0, 1.0]);
        // The carrier turns a quarter and moves by (1, 0, 0): the platform goes round it and takes its yaw.
        let q = std::f32::consts::FRAC_PI_2;
        let rows = triggers::euler_matrix([0.0, 0.0, q]);
        {
            let k = &mut t.mobys[0];
            k.position[0] = 1.0;
            k.rotation[2] = q;
            for (row, src) in k.rows.iter_mut().zip(&rows) { for (x, y) in row.iter_mut().zip(src) { *x = *y as f32; } }
            let r = k.rotation;
            triggers::carry_riders(&mut k.pvars, BLOCK, [1.0, 0.0, 0.0, 0.0], [0.0; 4], r);
        }
        run(&mut t, pinned_platform, &[1], 1);
        let m = &t.mobys[1];
        assert!((m.position[0] - 1.0).abs() < 1e-5 && (m.position[1] - 3.0).abs() < 1e-5 && m.position[2] == 1.0, "{:?}", m.position);
        assert!((m.rotation[2] - q).abs() < 1e-5);
        // Its own block: the move and the turn (the slot 9 is above the carrier's 5: no update-order correction).
        let d = triggers::platform_delta(m).unwrap();
        assert!((d.displacement[0] + 2.0).abs() < 1e-5 && (d.displacement[1] - 3.0).abs() < 1e-5, "{d:?}");
        assert!((d.rotation[2] - q).abs() < 1e-5);
    }

    #[test]
    fn pinned_platform_without_a_carrier_waits() {
        let mut m = carrier_moby(812, [3.0, 0.0, 1.0], 0.0);
        p::set_i32(&mut m.pvars, 0x80, -1);
        let mut t = MobyTable::new(vec![m], 4);
        run(&mut t, pinned_platform, &[0], 2);
        assert_eq!(t.mobys[0].state, 0);
    }
}
