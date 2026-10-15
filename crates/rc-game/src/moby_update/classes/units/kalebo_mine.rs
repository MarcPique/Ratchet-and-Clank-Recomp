//! **Kalebo III's floating mines, class 933** (level16 `0x2ddde0`, census U509; 25 placed, more made by the mine drones
//! 1401 ([`super::kalebo_mine_drone`]), which carry one each and drop it). A mine bobs 0.7 above the ground, spins on
//! its three axes and pulses its glow; a hit (anything but class 643) or Ratchet's touch blows it up, the touch with a
//! damage sphere. A placed mine comes back once out of view; a dropped one is gone, and is cleared at once whenever
//! Ratchet is not grinding (movement group 0xf). Read from the level16 decomp. Native `f32`.
//!
//! **Pvars** (0x1c): +0x00 the drone carrying it (0 none; the port keeps the moby index + 1), +0x04 the hover height
//! (−1 unset), +0x08 the bob's spring velocity, +0x0c the glow's phase, +0x10 / +0x14 / +0x18 the spin rates (x, y, z;
//! the drone's `0x2de298` draws them for its mines, state 0 for the placed ones).
//!
//! ## Coverage (`0x2ddde0`)
//! | address | what | port |
//! |---|---|---|
//! | state 0 | hover height −1; no carrier: the position 0.7 above `GroundHeight(0.5)`, the spin rates `randf(π/300, π/150)`, `randf(π/450, π/225)`, `randf(π/900, π/450)`, → 1; a carrier that is gone (state < 0 as a byte) → 1; else carried | [`update`] |
//! | state 1 | on level 16 (0x15ed84): the hover height 0.7 above `GroundHeight(0.5)` once, `Spring(height, 0.03, 0.3, 0, &z, &v)`; a dropped mine (past 0x15ffdc) with Ratchet not in group 0xf is deleted | [`update`] |
//! | state 1, hit | `MobyGetHitMessage(m, 0x230000, 0)` into the resolver `0x255db0` with a fresh record (health 1, +0x04 = 1, +0x10 = 0.5, +0x3a = 7, +0x3b = 0xff: no cooldown is ever running, so any hit gives out5 ≥ 2); no hit, or one from class 643, and not Ratchet's contact moby (0x13f58c) → alive; else `PlayClassSound(0, 0, m)`, `SpawnBeamExplosion(3 or 0, 1 or 0, 3, 5, 9, 1, 15, m, 0, pos, 10, 20, 60, −1, 1, 1, −1, 0)` (the damage sphere only for the touch); placed → update distance 0xff, hidden, no collision, hit slot 0xff, state 2 (no tail); dropped → deleted | [`update`] |
//! | state 2 | `FastBSphereCheck(64, pos, 3) == −1` (out of view) → shown, the class collision, state 1 | [`update`] |
//! | tail (states 0..2) | the glow's phase += π/60 (gp−0x4fb8); s = sin: glow = 0xff000000 \| (⌊32s⌋ + 0x30) · 0x10100 \| (⌊48s⌋ + 0xcf); rotation += the spin rates | [`tail`] |
//!
//! The resolver's null-attacker read (`hit +0x20` = 0: the game reads the class at 0xa6) is taken as "not 643" [L].
//! Ratchet's contact moby 0x13f58c (`Hero::cap_moby`) is not filled by the port yet (gaps.md G-HERO-033), so the touch
//! blast never fires; hits do.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, fx};
use crate::moby_update::services::World;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2d_dde0;
pub const CLASSES: [i16; 1] = [super::kalebo_mine_drone::MINE];

mod pv {
    pub const CARRIER: usize = 0x00;
    pub const HEIGHT: usize = 0x04;
    pub const VEL: usize = 0x08;
    pub const PHASE: usize = 0x0c;
    pub const SPIN: usize = 0x10;
    pub const LEN: usize = 0x1c;
}

/// The attacker class whose hits leave a mine alone.
const SPARED: i16 = 0x283;
/// The grind's movement group: dropped mines live only while Ratchet grinds.
const GRIND_GROUP: i32 = 0xf;
/// gp−0x4fb8 (0x161c48): the glow's phase step, π/60.
const GLOW_STEP: u32 = 0x3d56_7750;

fn blast(touched: bool) -> fx::Beam {
    let (damage_r, damage) = if touched { (3.0, 1.0) } else { (0.0, 0.0) };
    fx::Beam { damage_r, damage, flash: 3.0, flash2: 5.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 10, sparks: 20, puffs: 60, debris: 1, sound: -1, shake: true }
}

fn hover_ground(w: &World, id: MobyId) -> f32 {
    let pv4 = crate::moby_update::services::pv;
    w.ground_height(Pf::b(0x3f00_0000), pv4(c::pos(w, id)), 0).to_f32() + 0.7
}

/// Level16 `0x2ddde0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, pv::LEN);
    match w.m(id).state {
        0 => {
            c::set_pf(w, id, pv::HEIGHT, -1.0);
            let carrier = usize::try_from(c::pi32(w, id, pv::CARRIER) - 1).ok().filter(|&d| d < w.table.mobys.len());
            match carrier {
                None => {
                    let z = hover_ground(w, id);
                    w.mm(id).position[2] = z;
                    let a = w.rng.randf(f32::from_bits(0x3c2b_92a6), f32::from_bits(0x3cab_92a6));
                    c::set_pf(w, id, pv::SPIN, a);
                    let b = w.rng.randf(f32::from_bits(0x3be4_c388), f32::from_bits(0x3c64_c388));
                    c::set_pf(w, id, pv::SPIN + 4, b);
                    let d = w.rng.randf(f32::from_bits(0x3b64_c388), f32::from_bits(0x3be4_c388));
                    c::set_pf(w, id, pv::SPIN + 8, d);
                    w.mm(id).state = 1;
                }
                Some(d) => {
                    if (w.m(d).state as i8) < 0 { w.mm(id).state = 1; }
                }
            }
            tail(w, id);
        }
        1 => {
            let dropped = w.table.first_dynamic <= id;
            if w.svc.level == 16 {
                if c::pf(w, id, pv::HEIGHT) < 0.0 {
                    let h = hover_ground(w, id);
                    c::set_pf(w, id, pv::HEIGHT, h);
                }
                let (mut z, mut v) = (Pf::f(w.m(id).position[2]), Pf::f(c::pf(w, id, pv::VEL)));
                crate::hero::physics::spring(Pf::f(c::pf(w, id, pv::HEIGHT)), Pf::f(0.03), Pf::f(0.3), Pf::ZERO, &mut z, &mut v);
                w.mm(id).position[2] = z.to_f32();
                c::set_pf(w, id, pv::VEL, v.to_f32());
                if dropped && w.hero.group != GRIND_GROUP {
                    w.delete_moby(id);
                    return;
                }
            }
            let hit = w.get_hit(id, 0x23_0000, false);
            let by_hit = hit.is_some_and(|h| h.attacker.is_none_or(|a| w.m(a).o_class != SPARED));
            let touched = w.hero.cap_moby == Some(id);
            if !(by_hit || touched) {
                tail(w, id);
                return;
            }
            w.play_sound(0, 0, id);
            let p = c::pos(w, id);
            fx::beam_explosion(w, &blast(touched), Some(id), p);
            if dropped {
                w.delete_moby(id);
                return;
            }
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.state = 2;
            m.mode |= 0x41;
            m.hit_slot = 0xff;
            m.has_collision = false;
        }
        2 => {
            let p = c::pos(w, id);
            let out = w.view.is_some_and(|v| v.culled(64.0, [p[0], p[1], p[2], 3.0]));
            if out {
                let coll = super::class_collision(w, w.m(id).o_class);
                let m = w.mm(id);
                m.state = 1;
                m.mode &= 0xffbe;
                m.has_collision = coll;
            }
            tail(w, id);
        }
        _ => {}
    }
}

/// The tail (module doc): the glow's pulse and the spin.
fn tail(w: &mut World, id: MobyId) {
    let phase = c::add_rot(c::pf(w, id, pv::PHASE), f32::from_bits(GLOW_STEP));
    c::set_pf(w, id, pv::PHASE, phase);
    let s = phase.sin();
    let blue = (s * 48.0) as i32 + 0xcf;
    let rg = (s * 32.0) as i32 + 0x30;
    w.mm(id).glow = (rg.wrapping_mul(0x10000) as u32) | 0xff00_0000 | (rg.wrapping_mul(0x100) as u32) | blue as u32;
    for k in 0..3 {
        let r = c::add_rot(w.m(id).rotation[k], c::pf(w, id, pv::SPIN + 4 * k));
        w.mm(id).rotation[k] = r;
    }
}
