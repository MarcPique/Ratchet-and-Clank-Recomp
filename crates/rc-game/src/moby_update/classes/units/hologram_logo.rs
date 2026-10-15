//! U514: the Gadgetron logo, class 1143 (level16 0x2e1088; the same code on every level, e.g. level01 0x309990, all
//! with the rate 45 in their data): the ten logos placed on Kalebo III, and on every level the hologram the vendor 11
//! creates (`classes::vendor`, whose own two nodes turn its lists 0 / 1 as well). The logo turns its yaw one way and
//! its list-1 joint twice as fast the other way (a manipulator on its own joint list: `crate::moby_update::manip`).
//! Native `f32`.
//!
//! Pvar block: +0x00 the manipulator record (its angle at +0x40).
//!
//! | address | what it does | port |
//! |---|---|---|
//! | 0x2e10c0..0x2e10e8 | state 0: state 1, +0x73 = 32 (the shine distance), P+0x40 = `random_angle_radians`, `AttachManipulator(m, 1, P)` | [`update`] ([`manip::attach`]) |
//! | 0x2e10f0..0x2e1110 | state 1: rate = 45 (L16 gp−0x4ea8 = 0x161d58) · 0.017453292 · dt (0x15ed6c) | [`RATE_DEG`] |
//! | 0x2e110c..0x2e1124 | yaw (+0x48) = `fast_add_rotations(yaw, rate)` | [`update`] |
//! | 0x2e111c..0x2e1144 | P+0x40 = `fast_add_rotations(P+0x40, −2·rate)`; `FUN_00221e38(P+0x40, P+0x10, 2)` | [`update`] ([`manip::set_axis`]) |
//! | other states | nothing | — |
//!
//! Side effects: none besides the moby's yaw and its joint list (no sound, particles or other mobys).

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{add_rot, pf, set_pf};
use crate::moby_update::manip;
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2e_1088;
pub const CLASSES: [i16; 1] = [1143];
/// Level16's gp−0x4ea8 (0x161d58): 45 degrees per second.
pub const RATE_DEG: f32 = 45.0;
/// P+0x00: the record; P+0x40: its angle.
const REC: usize = 0x00;
const ANGLE: usize = 0x40;

/// Level16 0x2e1088.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < manip::rec::SIZE { w.mm(id).pvars.resize(manip::rec::SIZE, 0); }
    match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            w.mm(id).b73 = 0x20;
            let a = w.rng.rand_angle();
            set_pf(w, id, ANGLE, a);
            manip::attach(w, id, 1, id, REC);
        }
        1 => {
            let dt = crate::moby_update::services::fl(crate::moby_update::services::DT);
            let rate = RATE_DEG * f32::from_bits(0x3c8e_fa35) * dt;
            let yaw = add_rot(w.m(id).rotation[2], rate);
            w.mm(id).rotation[2] = yaw;
            let a = add_rot(pf(w, id, ANGLE), rate * -2.0);
            set_pf(w, id, ANGLE, a);
            manip::set_axis(w, id, id, REC, a, 2);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};

    #[test]
    fn turns_and_spins_its_joint_the_other_way() {
        let m = Moby { o_class: 1143, pvars: vec![0; 0x80], ..Moby::default() };
        let mut t = MobyTable::new(vec![m], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        svc.joint_targets.insert(1143, vec![2, 5]);
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        let mut r = *w.rng;
        update(&mut w, 0);
        let a0 = r.rand_angle();
        assert_eq!((w.m(0).state, w.m(0).b73, pf(&w, 0, ANGLE)), (1, 0x20, a0));
        // Linked on list 1's target joint, identity until the first turn.
        assert_eq!(w.m(0).joint_mods, vec![rc_formats::moby_anim::JointModifier::compose(5)]);
        update(&mut w, 0);
        let rate = 45.0 * f32::from_bits(0x3c8e_fa35) / 60.0;
        assert!((w.m(0).rotation[2] - rate).abs() < 1e-7);
        let a1 = add_rot(a0, -2.0 * rate);
        assert_eq!(pf(&w, 0, ANGLE), a1);
        assert_eq!(w.m(0).joint_mods[0].quat, crate::hero::idle::axis_quat(a1, 2));
    }
}
