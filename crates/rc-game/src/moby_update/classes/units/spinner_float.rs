//! U155: the bobbing floats with three spinning parts, class 481 (Eudora 04; level04 0x2cdda0), a platform Ratchet
//! rides. Each part is a look-at record on the float's joint lists 0..2 (`FUN_002777d8`, L04 0x2551c8:
//! `crate::moby_update::manip::look`) whose yaw target turns 1079.9°/s; the float bobs on its key time and sits 15
//! below its placed height, or at a controller moby's level (class 280). Read from the level04 disassembly. Native
//! `f32`.
//!
//! Pvar block: +0x3e s16 (13 at init), +0x60 the rider block (`CarryRiders`), +0xa0 + 0x80·i the three records,
//! +0x220 the base height, +0x224 + 4·i the parts' angles, +0x230 the controller moby (index, −1 none).
//!
//! | address | what it does | port |
//! |---|---|---|
//! | 0x2cddf4..0x2cde04 | no pvars: `DeleteMoby` | [`update`] |
//! | 0x2cde2c..0x2cdea8 | state 0: anim speed (+0x58) = `randf(0.5, 0.8)`; +0x220 = z + 15; state 1; update distance 0xff; +0x3e = 13; +0x224.. = three `random_angle_radians` | [`update`] |
//! | 0x2cdeb4..0x2cdf34 | state 1: bob = `fast_cos(FastNormalizeAngle(MobyAnimKeyTime·2π/118))`·0.75 | [`update`] (`creature::ground::key_time`) |
//! | 0x2cdf38..0x2cdf94 | for i = 0..2: +0x224 + 4i += 1079.9 (L04 gp−0x52a4)·0.017453292·dt; that angle → record i's yaw target (+0x108 + 0x80·i); `FUN_002777d8(0.03, 0.3, m, P+0xa0 + 0x80·i, i)` (× 0x15ed64 = 1) | [`update`] ([`manip::look`]) |
//! | 0x2cdf9c..0x2cdff0 | the drop: +0x230 = −1 → 15; a class-280 moby → its pvar +0x00 · 15; another class → 0; z = +0x220 + bob − drop | [`update`] |
//! | 0x2cdff4..0x2ce014 | every state: `CarryRiders(P+0x60, pos − old pos, rot, rot)` | [`update`] (`triggers::carry_riders`) |
//!
//! Side effects: the float's height, anim speed, joint list and rider block (Ratchet rides it: `hero::platform`); it
//! reads the controller's pvar. No sound or particles.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{add_rot, ground, pf, pi32, set_pf, set_pi16};
use crate::moby_update::manip;
use crate::moby_update::services::World;
use crate::moby_update::triggers;

pub const REFERENCE_LEVEL: u32 = 4;
pub const UPDATE_FN: u32 = 0x2c_dda0;
pub const CLASSES: [i16; 1] = [481];
/// The controller class whose pvar +0x00 scales the drop.
pub const CONTROLLER: i16 = 280;
/// L04 gp−0x52a4: the parts' turn rate (degrees per second).
pub const SPIN_DEG: f32 = f32::from_bits(0x4486_fccd);
const RIDERS: usize = 0x60;
const RECS: usize = 0xa0;
const BASE: usize = 0x220;
const ANGLES: usize = 0x224;
const LINK: usize = 0x230;
const SIZE: usize = 0x234;

/// Level04 0x2cdda0.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.is_empty() { return w.delete_moby(id); }
    if w.m(id).pvars.len() < SIZE { w.mm(id).pvars.resize(SIZE, 0); }
    let old = w.m(id).position;
    match w.m(id).state {
        0 => {
            let s = w.rng.randf(0.5, 0.8);
            w.mm(id).anim.speed = s;
            let z = old[2] + 15.0;
            set_pf(w, id, BASE, z);
            w.mm(id).state = 1;
            w.mm(id).update_dist = 0xff;
            set_pi16(w, id, 0x3e, 13);
            for i in 0..3 {
                let a = w.rng.rand_angle();
                set_pf(w, id, ANGLES + 4 * i, a);
            }
        }
        1 => {
            let t = ground::key_time(w, id) * std::f32::consts::TAU / 118.0;
            let bob = normalize(t).cos() * 0.75;
            let dt = crate::moby_update::services::fl(crate::moby_update::services::DT);
            let step = SPIN_DEG * f32::from_bits(0x3c8e_fa35) * dt;
            for i in 0..3 {
                let a = add_rot(pf(w, id, ANGLES + 4 * i), step);
                set_pf(w, id, ANGLES + 4 * i, a);
                let rec = RECS + 0x80 * i;
                set_pf(w, id, rec + manip::rec::TARGET + 8, a);
                manip::look(w, id, id, rec, i as u8, f32::from_bits(0x3cf5_c28f), f32::from_bits(0x3e99_999a));
            }
            let link = pi32(w, id, LINK);
            let drop = if link == -1 {
                15.0
            } else {
                match usize::try_from(link).ok().and_then(|l| w.table.mobys.get(l)) {
                    Some(c) if c.o_class == CONTROLLER => crate::moby_update::services::pvar::ff(&c.pvars, 0) * 15.0,
                    _ => 0.0,
                }
            };
            w.mm(id).position[2] = (pf(w, id, BASE) + bob) - drop;
        }
        _ => {}
    }
    let m = w.mm(id);
    let pos = m.position;
    let delta = [pos[0] - old[0], pos[1] - old[1], pos[2] - old[2], pos[3]];
    let rot = m.rotation;
    triggers::carry_riders(&mut m.pvars, RIDERS, delta, rot, rot);
}

/// `FastNormalizeAngle` 0x222088: into −π..π.
fn normalize(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let a = a % TAU;
    if PI < a { a - TAU } else if a < -PI { a + TAU } else { a }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::services::pvar as p;

    #[test]
    fn spins_its_parts_bobs_and_carries() {
        let mut m = Moby { o_class: 481, pvars: vec![0; SIZE], position: [10.0, 10.0, 40.0, 1.0], ..Moby::default() };
        p::set_i32(&mut m.pvars, LINK, -1);
        for i in 0..3 { p::set_ff(&mut m.pvars, RECS + 0x80 * i + manip::rec::REC_SCALE, 1.0); }
        let mut t = MobyTable::new(vec![m], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        svc.joint_targets.insert(481, vec![1, 2, 3]);
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        let mut r = *w.rng;
        update(&mut w, 0);
        let speed = r.randf(0.5, 0.8);
        let a: Vec<f32> = (0..3).map(|_| r.rand_angle()).collect();
        assert_eq!((w.m(0).state, w.m(0).update_dist, w.m(0).anim.speed, pf(&w, 0, BASE)), (1, 0xff, speed, 55.0));
        assert_eq!(crate::moby_update::creature::pi16(&w, 0, 0x3e), 13);
        update(&mut w, 0);
        // No class data: key time 0, bob 0.75; no controller: 15 down.
        assert_eq!(w.m(0).position[2], (55.0 + 0.75) - 15.0);
        let step = SPIN_DEG * f32::from_bits(0x3c8e_fa35) / 60.0;
        for (i, a) in a.iter().enumerate() { assert_eq!(pf(&w, 0, ANGLES + 4 * i), add_rot(*a, step)); }
        // Three records linked (list 2's first), each turning toward its part's angle.
        assert_eq!(w.m(0).joint_mods.iter().map(|m| m.joint).collect::<Vec<_>>(), vec![3, 2, 1]);
        // The rider block: the move of this tick (from 40 to 40.75).
        assert_eq!(p::ff(&w.m(0).pvars, RIDERS + 0x18), 0.75);
    }
}
