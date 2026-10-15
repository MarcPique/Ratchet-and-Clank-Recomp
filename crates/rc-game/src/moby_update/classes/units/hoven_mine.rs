//! **Hoven's seeker mines, class 1269** (level12 `0x304e00`; census U438; 20 placed). They pop up (half a second of
//! a sine rise), then roll at their target (within 8; up to 5.75·dt, accelerating at 30·dt² after `ticks(15)`),
//! swerving 40° to one side within 7 (the other side when their spawner's command is set), bobbing (a sine of 0.1 at
//! 300°/s) and springing 0.58 over the ground (or Ratchet's ground height, when higher; within 0.5 above to 3 below)
//! once rolling. They blow up when within 0.25 (xy) of the target, when hit by anything but another of their kind
//! (0x4f5) hard enough, or a tick after their sphere (0.4 at 0.4 up, a hit of 1 pushing along their heading) touches
//! a moby other than their spawner and their kind, or their sphere (0.35 at 0.35 up) touches the world. A blob shadow
//! under them.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x304e00` | 0: targetable, home, the swerve ±40° (the spawner's +0xbc), → 1; 1: the roll (module doc); 2: the big burst (`SpawnBeamExplosion(0, 0, 2, 1, 4, 1, 7, m, 0, —, 5, 15, 25, 2, 0, 9, −1, 0)`), deleted; the near / hit burst (`(0, 0, 1, 0.5, 4, 0.7, 7, m, 0, —, 3, 3, 5, 2, 0, 0, −1, 0)`), `SetDeathBits`, deleted; the shadow `0x272eb8(1, m)` | [`update`] (`fx::beam_explosion`, `shadows::blob`, `Spring` 0x2747b0 = L01 0x270780: `hero::physics::spring`) |
//!
//! Read from the level12 decomp and disassembly (the explosions' arguments, the spheres' radii). [L] The explosions
//! pass no position (null): the moby's. Native `f32`.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, damage, fx, target};
use crate::moby_update::services::{pv, HitTemplate, World};
use crate::ps2v::Pf;
use std::f32::consts::PI;

pub const REFERENCE_LEVEL: u32 = 12;
pub const UPDATE_FN: u32 = 0x30_4e00;
pub const CLASSES: [i16; 1] = [1269];

const DT: f32 = c::DT;
const DT2: f32 = c::DT2;
const KIND: i16 = 0x4f5;

mod pv_ {
    pub const D: usize = 0x20;
    pub const HOME: usize = 0x60;
    pub const SPEED: usize = 0x70;
    pub const BOB: usize = 0x74;
    pub const PHASE: usize = 0x78;
    pub const ZV: usize = 0x7c;
    pub const SWERVE: usize = 0x80;
    pub const AGE: usize = 0x84;
    pub const SIZE: usize = 0x88;
}
use pv_ as o;

const SMALL: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 1.0, flash2: 0.5, flash_dist: 4.0, scale: f32::from_bits(0x3f33_3333), light: 7.0, streaks: 3, sparks: 3, puffs: 5, debris: 0, sound: 2, shake: false };
const BIG: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 4.0, scale: 1.0, light: 7.0, streaks: 5, sparks: 15, puffs: 25, debris: 9, sound: 2, shake: false };

fn blow(w: &mut World, id: MobyId, b: &fx::Beam) {
    let p = c::pos(w, id);
    fx::beam_explosion(w, b, Some(id), p);
}

/// Level12 `0x304e00` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < o::SIZE { return; }
    let t = target::acquire(w, id, 8.0);
    let hit = w.get_hit(id, 0x23_0000, false);
    let res = damage::resolve(w, id, hit, o::D, 0, 4);
    let by_kind = hit.and_then(|h| h.attacker).is_some_and(|a| w.m(a).o_class == KIND);
    if c::dist2(c::pos(w, id), t.pos) < 0.25 || (2 <= res.out5 && hit.is_some() && !by_kind) {
        blow(w, id, &SMALL);
        set_death_bits(w, id, 0, -1);
        w.delete_moby(id);
        return;
    }
    match w.m(id).state {
        0 => {
            w.mm(id).mode |= mode::TARGETABLE;
            let p = c::pos(w, id);
            c::set_pv4(w, id, o::HOME, p);
            w.mm(id).state = 1;
            c::set_pf(w, id, o::SWERVE, f32::from_bits(0x3f32_b8c2));
            if w.m(id).parent.is_some_and(|s| w.m(s).cmd != 0) { c::set_pf(w, id, o::SWERVE, f32::from_bits(0xbf32_b8c2)); }
            c::set_pi32(w, id, o::AGE, 0);
        }
        1 => {
            let age = c::pi32(w, id, o::AGE) + 1;
            c::set_pi32(w, id, o::AGE, age);
            let t30 = w.ticks(0x1e);
            if age < t30 { w.mm(id).position[2] += ((age as f32 * PI) / t30 as f32).sin() * 0.07; }
            if w.ticks(0xf) < age {
                let s = c::pf(w, id, o::SPEED) + DT2 * 30.0;
                c::set_pf(w, id, o::SPEED, s);
            }
            if w.ticks(0x1e) < age {
                if w.m(id).anim.seq_b == 0 { w.anim_blend(id, 1, 0, 2); }
                if w.m(id).anim.seq_b == 1 && w.m(id).anim.flags & 2 != 0 { w.anim_blend(id, 2, 0, 2); }
            }
            if DT * 5.75 < c::pf(w, id, o::SPEED) { c::set_pf(w, id, o::SPEED, DT * 5.75); }
            let p = c::pos(w, id);
            let a = c::atan(t.pos[0] - p[0], t.pos[1] - p[1]);
            let h = if c::dist2(p, t.pos) < 7.0 { c::add_rot(a, c::pf(w, id, o::SWERVE)) } else { a };
            let sp = c::pf(w, id, o::SPEED);
            {
                let m = w.mm(id);
                m.position[0] += h.cos() * sp;
                m.position[1] += h.sin() * sp;
            }
            if w.ticks(0x1e) < age {
                let ph = c::add_rot(c::pf(w, id, o::PHASE), DT * 5.235_987_7);
                c::set_pf(w, id, o::PHASE, ph);
                let b = ph.sin() * 0.1;
                let old = c::pf(w, id, o::BOB);
                w.mm(id).position[2] += b - old;
                c::set_pf(w, id, o::BOB, b);
            }
            if w.ticks(0x1e) < age {
                let g = c::ground::ground(w, c::pos(w, id), 0.5, 0).z.max(w.hero.ground_z.to_f32());
                let d = g - c::pos(w, id)[2];
                if d < 0.5 && -3.0 < d {
                    let (mut z, mut v) = (Pf::f(c::pos(w, id)[2]), Pf::f(c::pf(w, id, o::ZV)));
                    crate::hero::physics::spring(Pf::f(g + 0.58), Pf::f(0.02), Pf::f(f32::from_bits(0x3e99_999a)), Pf::f(DT * 4.0), &mut z, &mut v);
                    w.mm(id).position[2] = z.to_f32();
                    c::set_pf(w, id, o::ZV, v.to_f32());
                }
            }
            if w.ticks(0x1e) < age {
                let dir = [Pf::f(a.cos() * 0.3), Pf::f(a.sin() * 0.3), Pf::f(1.2), Pf::f(f32::from_bits(0x45af_df66))];
                let tmpl = HitTemplate { dir, attacker: Some(id), flags: 0x1_0001, damage: Pf::ONE, w20: 1, ..Default::default() };
                let p = c::pos(w, id);
                let first = w.sphere_mobys_list(Pf::f(0.4), pv([p[0], p[1], p[2] + 0.4, p[3]]), 0x10, Some(id), Some(&tmpl)).first().copied();
                let moby_hit = first.is_some_and(|m| w.m(m).o_class != KIND && Some(m) != w.m(id).parent);
                let touched = moby_hit || w.coll_sphere(pv([p[0], p[1], p[2] + 0.35, p[3]]), Pf::f(0.35), 0, Some(id)).is_some();
                if touched { w.mm(id).state = 2; }
            }
        }
        2 => {
            blow(w, id, &BIG);
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    crate::shadows::blob(w, 1.0, id);
}
