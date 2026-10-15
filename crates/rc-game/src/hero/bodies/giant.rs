//! **Giant Clank as the hero (body 2)**: the states 0x5a..0x62 of the hero state machine as level 00 compiles them
//! (SetState L00 `0x2223f8`, physics `0x217970`, transitions `0x229b70`; level 15's `0x216c38` / `0x20ed50` and
//! level 18's are the same cases, levels 04, 07, 09, 10, 13 compile them too), his energy 0x140980 (200 at the switch;
//! the hit intake takes the damage from it: `damage::hit_intake`) and his part of `HeroUpdateAlt` (`0x2062b0`: the
//! glow pulse, the pilot Ratchet in the cockpit `0x2061f0`). Sequences are Giant Clank's class's (0x1a3).
//!
//! Everything Giant Clank touches smashes: his walk (three spheres 2.7 ahead at 1.8, 40 damage), his falls and jumps
//! (3.5 under him, 40), his punches (two fists, 4 damage with an exact push, plus the 2.2 sweep). He fires the missiles
//! (class 0x100) from his arms (○), the shockwave (class 0x593) when a jump lands, and the beam (class 0x5f3) from his
//! head (△, then a ticks(301) lockout 0x140986).
//!
//! **Coverage** (L00 addresses; `ported (fn) / NOT ported (gap) / n/a`):
//!
//! | address | what | status |
//! |---|---|---|
//! | SetState 0x5a (0x226c6c) | group 0, 0x1415d4 = 0; anim 0 over 15 | ported ([`entry`]) |
//! | SetState 0x5b (0x226e60) | group 1, speed = \|eff.xy\|, 0x13f708 = 0; anim 3 over 8 from frame 18 | ported |
//! | SetState 0x5c (0x226db8) | group 2, 0x13fc90 = 24·dt², 0x13fc94 = 7·dt², vel = eff; anim 5 over 8 above 2.5, else 0 over 11 | ported |
//! | SetState 0x5d (0x226ef0) | group 7; motion ×0.5; vel = eff clamped to 7·dt; invulnerable 77; anim 6 on curve −3 from 3 + one step | ported |
//! | SetState 0x5e (the jump case) | the jump lockout; the defaults; h 4.2..4.3 (ramp 15) after 19, frames 17 / 22 / 40, bottom 0.6, gravity 30·dt² (50 descending), turn 250°/s, air speed 4·dt, anim factor 2; anim 4 over 8 | ported ([`jump_entry`]) |
//! | SetState 0x5f / 0x61 | group 6; anim 8 / 0xc over 8 | ported |
//! | SetState 0x60 (0x226c98) | group 6; the punch row 0x13fdb0: the next of three when chained (the previous state 0x60, or 0x60 before that ended within 10 ticks), else 0; anim 9 + row over 7 (13 when chained) from frame 1 | ported |
//! | SetState 0x62 (0x226eac) | group 0x14, momentum = eff; anim 7 over 12 | ported |
//! | physics 0x5a / 0x62 | edge brake (3.7, 0), stick, speed → 0 (12.6·dt²), vel 0, momentum decay 12.6·dt², wall check 0, gravity (25 from eff + the climb check / 54) | ported ([`physics`]) |
//! | physics 0x5b | the three walk spheres (r 2.7, damage 40, push 1, flags 0x30000, at 1.8 ahead / 1.4 up at −45°, 0°, +45°); a footstep (key 30 or 0 passed): camera shake 0.1 for 20 ticks; stick × 6·dt; turn (0.021 / 0.3 / 130°/s × (stick + 0.35)); speed step 7.5·(1 − 0.3·r) / 8.5·dt²; the records 27..29 (k / d, the lean targets from the residual ±1.7); planar velocity; wall check 1; gravity | ported |
//! | physics 0x5c | stick × 6·dt; the sphere under him (3.5 at the feet −1, 40, push 1, 0x30000); air control unless 0x13f514; gravity 12·dt², ≥ −10·dt | ported |
//! | physics 0x5d | \|vel.xy\| ×0.92 (×0.99 in the air), vel.z 0, −0.004 / eff.z − 24·dt² | ported |
//! | physics 0x5e | the jump case (`jump.rs`) | ported |
//! | physics 0x5f | speed → 0; stick; turn (0.01, 0.2, 70°/s); vel 0; momentum decay; key 4 passed: a missile from each arm (joint lists 3 / 4, 1.4 ahead, `0x2a96f8(yaw, 0, 0.5, yaw, 0, hero, point, 0)`); gravity | ported ([`Spawn::Missile`] → `classes::units::giant_missile`) |
//! | physics 0x60 | stick; turn (0.02, 0.2, 110°/s); vel 0; momentum decay; not blending: the fists (template `0x26e808(4, …, 0xb0000, (cos, sin))`, dir.z 1, exact push, types 7 / 1, `coll_sphere_mobys(2, joint 3 / 4, 0x10)` by row and key) and the sweep (2.2, 40); the lunge (rows 0 / 1: 21·dt over key 7..15, step 180 / 140; row 2: 30·dt over 11..20, 150 / 120); wall check 1; gravity; edge brake | ported |
//! | physics 0x61 | speed → 0, vel 0, momentum decay; not blending: up to key 22 the beam's point (3 ahead along row 0, 5 up) through `0x2a9b80` and playback 0.5; key 22..55 playback 1 | ported ([`Spawn::Beam`] → `classes::units::giant_beam`) |
//! | transitions 0x5a | wrap: after 50 ticks on 0 a 10 % fidget 1 / 40 % fidget 2, else back to 0; L1 / L2 → 1; ✕ → 0x5e; □ (15) → 0x60; △ with no lockout → 0x61; ○ → 0x5f; in the air above 2 → 0x5c; stick > 0.22 → 0x5b | ported ([`transitions`]) |
//! | transitions 0x5b | playback \|eff\|·8 (≥ 0.75); the attacks as 0x5a; air 5 ticks above 1.5 → 0x5c; stopped 20 ticks → 0x5a | ported |
//! | transitions 0x5c | descending: the sphere (3.5, damage 1); landing: vz clamps, → 0x62 (no energy) / 0x5b / 0x5a | ported |
//! | transitions 0x5d | after 32 in the air above 1.5 → 0x5c; after 40 or the wrap → 0x5a (no anim) / 0x62 | ported |
//! | transitions 0x5e | descending: the sphere (3.5, 40); the apex playback (factor 0x13f7e4); the landing (camera shake 0.15 for 30, the frame), the descent playback; the first landed tick: the shockwave `0x2a99b0(2, 15·dt, 40, hero, (x, y, ground + 0.2), ticks(25))`; the hop; → 0x5a after 12 | ported ([`Spawn::Shockwave`]; class 0x593: `classes::units::giant_shockwave`) |
//! | transitions 0x5f | ○ held keeps firing; released and wrapped → 0x5a | ported |
//! | transitions 0x60 | □ (17) after frame 21 (25.5 on sequence 0xb) and 20 ticks → 0x60; past 28.5 or wrapped → 0x5a (no anim) | ported |
//! | transitions 0x61 | wrapped: lockout 0x140986 = ticks(301), → 0x5a | ported |
//! | transitions 0x62 | wrapped → the death fade | ported |
//! | hit intake (body 2) | flag bit 2, energy −= damage (≥ 0); flag 4 or no energy → 0x5d, `0x2a9be0` (the beam deleted), on the ground the knockback (7, 3.5)·dt | ported (`damage::hit_intake`, [`beam_end`]) |
//! | HeroUpdateAlt mode 2 | `0x2278c0` (the glow), `0x2061f0` (the pilot) | ported (`super::glow`, [`after_update`]); the cheat's head scale: NOT ported (G-SAV-006); joint list 6's point and the draw callback: ported (`super::super::glow::body_dot`) |
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::assign_op_pattern)]

use super::super::common::blend;
use super::super::physics::*;
use super::super::states::Ctx;
use super::super::{AnimCtl, Hero};
use super::{rec, BodyHit};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::pad::{button, fast_diff_rots};
use crate::ps2v::Pf;
use crate::rng::Rng;

pub const IDLE: i32 = 0x5a;
pub const WALK: i32 = 0x5b;
pub const FALL: i32 = 0x5c;
pub const HURT: i32 = 0x5d;
pub const JUMP: i32 = 0x5e;
pub const MISSILES: i32 = 0x5f;
pub const PUNCH: i32 = 0x60;
pub const BEAM: i32 = 0x61;
pub const DEATH: i32 = 0x62;

/// The missile (`0x2a96f8`, L15 `0x29d408`, L18 `0x2aa0f8`).
pub const MISSILE_CLASS: i16 = 0x100;
/// The landing shockwave (`0x2a99b0`).
pub const SHOCKWAVE_CLASS: i16 = 0x593;
/// The head beam (`0x2a9aa0`).
pub const BEAM_CLASS: i16 = 0x5f3;

fn p(x: f32) -> Pf { Pf::f(x) }
fn pressed(c: &Ctx, mask: u32, n: i32) -> bool { c.env.pad.pressed_within(mask, ticks(n)).is_some() }

/// An effect moby Giant Clank's states fire this tick (made by the tick, [`after_update`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spawn {
    /// `0x2a96f8(yaw, 0, 0.5, yaw, 0, hero, point, 0)`: a missile (class 0x100) at `pos` heading `yaw`.
    Missile { pos: [f32; 3], yaw: f32 },
    /// `0x2a99b0(2.0, 15·dt, 40, hero, pos, ticks(25))`: the shockwave ring (class 0x593).
    Shockwave { pos: [f32; 3] },
    /// `0x2a9b80(hero, pos)`: the beam (class 0x5f3, made by `0x2a9aa0` if there is none) held at `pos` this tick.
    Beam { pos: [f32; 3] },
    /// `0x2a9be0()`: the charging beam deleted (the hit intake's body-2 branch).
    BeamEnd,
}

// ------------------------------------------------------------------------------------------------
// SetState.

pub(crate) fn entry(h: &mut Hero, c: &mut Ctx, id: i32, play: bool, old_sub: i32) -> Option<bool> {
    match id {
        IDLE => {
            h.f15d4 = 0;
            h.group = 0;
            if play { h.set_anim(c.anim, c.rng, blend(15), 0, 0); }
        }
        WALK => {
            h.group = 1;
            h.f15d4 = 0;
            h.speed = len2(h.eff);
            h.turn_to_target = 0;
            if play { h.set_anim(c.anim, c.rng, blend(8), 3, 18); }
        }
        FALL => {
            h.group_gravity = DT2 * p(24.0);
            h.group = 2;
            h.fc94 = DT2 * p(7.0);
            h.f15d4 = 0;
            h.vel = h.eff;
            if play {
                if p(2.5) < h.height { h.set_anim(c.anim, c.rng, blend(8), 5, 0); } else { h.set_anim(c.anim, c.rng, blend(11), 0, 0); }
            }
        }
        HURT => {
            h.group = 7;
            h.f15d4 = 0;
            h.scale_motion(Pf::b(0x3f00_0000));
            h.vel = h.eff;
            super::super::common::clamp_len_2745f0(&mut h.vel, DT * p(7.0));
            h.f510 = ticks(77);
            if play {
                h.set_anim(c.anim, c.rng, Pf::b(0xc040_0000), 6, 3);
                c.anim.curve_step();
            }
        }
        JUMP => {
            if h.jump_lockout != 0 {
                h.state = h.prev_state;
                h.timer = h.prev_timer;
                h.substate = old_sub;
                return Some(false);
            }
            jump_entry(h, c, play);
        }
        MISSILES | BEAM => {
            h.group = 6;
            h.f15d4 = 0;
            if play { h.set_anim(c.anim, c.rng, blend(8), if id == MISSILES { 8 } else { 0xc }, 0); }
        }
        PUNCH => {
            h.group = 6;
            h.f15d4 = 0;
            let chained = h.prev_state == PUNCH || (h.prev_prev_state == PUNCH && h.prev_timer < ticks(10));
            h.melee.combo = if chained { (h.melee.combo + 1) % 3 } else { 0 };
            let b = if h.prev_state == PUNCH { ticks(13) } else { ticks(7) };
            if play { h.set_anim(c.anim, c.rng, Pf::from_i32(b), (h.melee.combo + 9) as u8, 1); }
        }
        DEATH => {
            h.group = 0x14;
            h.f15d4 = 0;
            h.momentum = h.eff;
            if play { h.set_anim(c.anim, c.rng, blend(12), 7, 0); }
        }
        _ => {}
    }
    None
}

/// The jump case's 0x5e branch (L00 SetState): h 4.2..4.3 over 15 ticks after a 19-tick windup, frames 17 / 22 / 40,
/// bottom 0.6, gravity 30·dt² (50·dt² once descending), turn cap 250°/s, air speed 4·dt, anim factor 2; anim 4 over 8.
fn jump_entry(h: &mut Hero, c: &mut Ctx, play: bool) {
    h.jump_block_defaults(c.rng);
    let j = &mut h.jump;
    j.h = p(4.2);
    j.takeoff = ticks(19);
    j.bottom784 = Pf::b(0x3f19_999a);
    (j.f_apex, j.f_hold, j.f_land) = (p(17.0), p(22.0), p(40.0));
    j.hmax = p(4.3);
    j.hmin = p(4.2);
    j.ramp = ticks(15) as i16;
    j.g = DT2 * p(30.0);
    j.turn_max = DT * p(4.363_323);
    j.g_down = DT2 * p(50.0);
    j.air_speed = DT * p(4.0);
    j.ak = p(2.0);
    if play { h.set_anim(c.anim, c.rng, blend(8), 4, 0); }
}

// ------------------------------------------------------------------------------------------------
// Physics.

pub(crate) fn physics(h: &mut Hero, env: &Env, anim: &mut dyn AnimCtl, rng: &mut Rng) -> bool {
    let _ = rng;
    match h.state {
        IDLE | DEATH => {
            h.edge_brake(env, Pf::b(0x406c_cccd), Pf::ZERO);
            h.stick_target(env, Pf::ONE);
            h.target_speed = Pf::ZERO;
            h.speed_step(Pf::ZERO, DT2 * p(12.6));
            h.vel = V0;
            h.momentum_decay(DT2 * p(12.6));
            h.wall_check(env, 0);
            gravity(h, env);
        }
        WALK => walk_physics(h, env, &anim.view()),
        FALL => {
            h.stick_target(env, DT * p(6.0));
            let c = [f(h.pos[0]), f(h.pos[1]), f(h.pos[2]) - 1.0];
            h.bodies.hits.push(BodyHit::Sphere { r: 3.5, damage: 40.0, push: 1.0, centre: c, flags: 0x3_0000, b18: 0, b19: 1, sphere_flags: 0 });
            if h.lockout == 0 { h.air_control(env); }
            let z = h.vel[2];
            h.gravity_from(z, DT2 * p(12.0));
            let floor = DT * p(-10.0);
            if h.vel[2] < floor { h.vel[2] = floor; }
        }
        HURT => {
            let l = len2(h.vel);
            let k = if h.air_ticks == 0 { Pf::b(0xbda3_d708) } else { Pf::b(0xbc23_d700) };
            let mut v = h.vel;
            clamp_len2(&mut v, ((SCALE60 * k) * l) + l);
            h.vel = v;
            h.vel[2] = Pf::ZERO;
            if h.air_ticks == 0 {
                let z = h.vel[2];
                h.gravity_from(z, Pf::b(0x3b83_126f));
            } else {
                h.gravity_from(h.eff[2], DT2 * p(24.0));
            }
        }
        JUMP => {
            let key = anim.view().frame;
            h.phys_jump_anim(env, key);
        }
        MISSILES => {
            h.target_speed = Pf::ZERO;
            h.speed_step(Pf::ZERO, DT2 * p(12.6));
            h.stick_target(env, Pf::ONE);
            h.turn_to(SCALE64 * p(0.01), SCALE64 * p(0.2), DT * p(1.221_730_5));
            h.vel = V0;
            h.momentum_decay(DT2 * p(12.6));
            if super::super::boots::passed(&anim.view(), 4.0) {
                let yaw = h.rot[2];
                for list in [3usize, 4] {
                    let j = super::super::fx::joint_point(h, &*anim, list);
                    let pos = [j[0] + f(fast_cos(yaw)) * 1.4, j[1] + f(fast_sin(yaw)) * 1.4, j[2]];
                    h.bodies.spawns.push(Spawn::Missile { pos, yaw: f(yaw) });
                }
            }
            gravity(h, env);
        }
        PUNCH => punch_physics(h, env, anim),
        BEAM => {
            h.target_speed = Pf::ZERO;
            h.speed_step(Pf::ZERO, DT2 * p(12.6));
            h.vel = V0;
            h.momentum_decay(DT2 * p(12.6));
            let v = anim.view();
            if !v.blending() {
                let key = p(v.frame);
                if key <= p(22.0) {
                    // FastVecNormalize(3, &q, row 0); q.z += 5; q += the hero moby's position; `0x2a9b80(hero, q)`.
                    let q = set_len3(h.rows[0], p(3.0));
                    let pos = [f(q[0]) + f(h.pos[0]), f(q[1]) + f(h.pos[1]), (f(q[2]) + 5.0) + f(h.pos[2])];
                    h.bodies.spawns.push(Spawn::Beam { pos });
                    h.anim_speed = p(0.5);
                } else if key < p(55.0) {
                    h.anim_speed = Pf::ONE;
                }
            }
            gravity(h, env);
        }
        _ => return false,
    }
    true
}

fn f(x: Pf) -> f32 { x.to_f32() }

/// The gravity tail of the standing / attacking states (25·dt² from eff.z in the air + `0x2121c0`, else 54·dt²).
fn gravity(h: &mut Hero, env: &Env) {
    if h.air_ticks != 0 {
        h.gravity_from(h.eff_v[2], DT2 * p(25.0));
        h.climb_check(env);
    } else {
        let z = h.vel[2];
        h.gravity_from(z, DT2 * p(54.0));
    }
}

/// `0x248c60(dist, yaw, up, out)`: a point `dist` along the hero-frame direction `yaw`, `up` above the feet.
fn local_dir(h: &Hero, dist: f32, yaw: Pf, up: f32) -> [f32; 3] {
    let v = [fast_cos(yaw) * p(dist), fast_sin(yaw) * p(dist), p(up), Pf::ZERO];
    let w = vadd(mul_rows3(&[h.rows[0], h.rows[1], h.rows[2]], v), h.pos);
    to_f32x3(w)
}

/// The three walk / sweep spheres (`0x248c60(1.8, −45° + 45°·i, 1.4)`, `0x26e830(r, 40, 1, hero, point, 0x30000, 0, 1, 0)`).
fn sweep(h: &mut Hero, r: f32) {
    for i in 0..3 {
        let a = fast_add_rotations(Pf::b(0xbf49_0fdb), Pf::from_i32(i) * Pf::b(0x3f49_0fdb));
        let c = local_dir(h, 1.8, a, 1.4);
        h.bodies.hits.push(BodyHit::Sphere { r, damage: 40.0, push: 1.0, centre: c, flags: 0x3_0000, b18: 0, b19: 1, sphere_flags: 0 });
    }
}

fn walk_physics(h: &mut Hero, env: &Env, v: &super::super::AnimView) {
    let r = fast_diff_rots(h.rot[2], h.target_yaw);
    sweep(h, 2.7);
    // A footstep: the camera shake records 0x167260 / 0x167268 (L00 0x166de0 / 0x166de8) = 0.1, ticks(20).
    if super::super::boots::passed(v, 30.0) || super::super::boots::passed(v, 0.0) { super::super::fx::shake(h, crate::follow_camera::ShakeAxis::Up, 0.1, ticks(20)); }
    h.stick_target(env, DT * p(6.0));
    let mut k = Pf::ONE - r * Pf::b(0x3e99_999a);
    if k < Pf::ZERO { k = Pf::ZERO; }
    let s = h.stick_mag + Pf::b(0x3eb3_3333);
    h.turn_to((SCALE64 * p(0.021)) * s, SCALE64 * p(0.3), (DT * p(2.268_928)) * s);
    h.speed_step((DT2 * p(7.5)) * k, DT2 * p(8.5));
    let res = h.yaw_residual.to_f32().clamp(-1.7, 1.7);
    let sc = 1.0f32;
    let j = &mut h.bodies.joints;
    j[rec::R29].k = sc * 0.01;
    j[rec::R27].d = sc * 0.3;
    j[rec::R27].k = sc * 0.015;
    j[rec::R28].k = sc * 0.02;
    j[rec::R27].target[0] = -res * 0.14;
    j[rec::R27].target[2] = res * 0.25;
    j[rec::R28].target[2] = res * 0.57;
    j[rec::R28].d = j[rec::R27].d;
    j[rec::R29].d = j[rec::R27].d;
    if h.turn_to_target == 0 || h.f658 != 0 { h.set_planar_vel(Pf::b(0x47c3_4f80)); } else { h.set_planar_vel(h.target_yaw); }
    h.wall_check(env, 1);
    gravity(h, env);
}

/// 0x60: the punches.
fn punch_physics(h: &mut Hero, env: &Env, anim: &mut dyn AnimCtl) {
    h.stick_target(env, Pf::ONE);
    h.turn_to(SCALE64 * p(0.02), SCALE64 * p(0.2), DT * p(1.919_862_2));
    h.target_speed = Pf::ZERO;
    h.vel = V0;
    h.momentum_decay(DT2 * p(12.6));
    let v = anim.view();
    let key = v.frame;
    let row = h.melee.combo;
    if !v.blending() {
        let yaw = h.rot[2];
        let dir = [f(fast_cos(yaw)), f(fast_sin(yaw)), 1.0];
        let fist = |h: &mut Hero, anim: &dyn AnimCtl, list: usize| {
            let j = super::super::fx::joint_point(h, anim, list);
            h.bodies.hits.push(BodyHit::Punch { r: 2.0, damage: 4.0, centre: [j[0], j[1], j[2]], dir, flags: 0xb_0000 });
        };
        match row {
            1 if 7.0 < key && key < 14.0 => fist(h, &*anim, 4),
            0 if 7.0 < key && key < 14.0 => fist(h, &*anim, 3),
            2 if 14.0 < key && key < 21.0 => {
                fist(h, &*anim, 3);
                fist(h, &*anim, 4);
            }
            _ => {}
        }
        sweep(h, 2.2);
    }
    if 0 <= row {
        if row < 2 {
            if !v.blending() && (7.0..15.0).contains(&key) { h.target_speed = DT * p(21.0); }
            h.speed_step(DT2 * p(180.0), DT2 * p(140.0));
            h.set_planar_vel(Pf::b(0x47c3_4f80));
        } else if row == 2 {
            if !v.blending() && (11.0..20.0).contains(&key) { h.target_speed = DT * p(30.0); }
            h.speed_step(DT2 * p(150.0), DT2 * p(120.0));
            h.set_planar_vel(Pf::b(0x47c3_4f80));
        }
    }
    h.wall_check(env, 1);
    if h.air_ticks != 0 {
        h.gravity_from(h.eff_v[2], DT2 * p(25.0));
    } else {
        let z = h.vel[2];
        h.gravity_from(z, DT2 * p(54.0));
    }
    h.edge_brake(env, Pf::b(0x406c_cccd), Pf::ZERO);
}

/// Giant Clank's punch on the moby world: the template `0x26e808(damage, tmpl, hero, flags, dir)` with dir.z = 1 and the
/// exact-push marker (w 5627.925), +0x18 / +0x19 = 7 / 1, +0x1a = his class; then `coll_sphere_mobys(r, centre, 0x10,
/// hero, tmpl)`.
pub(crate) fn punch(w: &mut crate::moby_update::services::World, body: MobyId, r: f32, damage: f32, centre: [f32; 3], dir: [f32; 3], flags: u32) {
    use crate::moby_update::services::HitTemplate;
    let tmpl = HitTemplate {
        dir: [Pf::f(dir[0]), Pf::f(dir[1]), Pf::f(dir[2]), Pf::b(0x45af_df66)],
        attacker: Some(body),
        flags,
        b18: 7,
        b19: 1,
        h1a: w.m(body).o_class as u16,
        damage: Pf::f(damage),
        w20: 1,
    };
    w.sphere_mobys(Pf::f(r), [Pf::f(centre[0]), Pf::f(centre[1]), Pf::f(centre[2]), Pf::ZERO], 0x10, Some(body), Some(&tmpl));
}

// ------------------------------------------------------------------------------------------------
// Transitions.

pub(crate) fn transitions(h: &mut Hero, c: &mut Ctx) {
    match h.state {
        IDLE => tr_idle(h, c),
        WALK => tr_walk(h, c),
        FALL => {
            if h.jump.descending != 0 { feet_hit(h, 1.0); }
            if h.air_ticks != 0 { return; }
            let floor = DT * p(-9.0);
            if h.disp[2] < floor { h.disp[2] = floor; }
            if h.vel[2] < floor { h.vel[2] = floor; }
            let next = if h.bodies.energy < 1 {
                DEATH
            } else if p(0.5) < h.stick_mag && h.lockout == 0 {
                WALK
            } else {
                IDLE
            };
            h.set_state(c, next, true);
        }
        HURT => {
            if !(h.timer < ticks(32) || h.air_ticks == 0 || h.height <= p(1.5)) {
                h.set_state(c, FALL, true);
                return;
            }
            if h.timer <= ticks(40) && c.anim.view().flags & 2 == 0 { return; }
            if 0 < h.bodies.energy { h.set_state(c, IDLE, false); } else { h.set_state(c, DEATH, true); }
        }
        JUMP => tr_jump(h, c),
        MISSILES => {
            if c.env.pad.held & button::CIRCLE != 0 || c.anim.view().flags & 2 == 0 { return; }
            h.set_state(c, IDLE, true);
        }
        PUNCH => {
            let v = c.anim.view();
            let from = if v.seq_b == 0xb { 25.5 } else { 21.0 };
            if !v.blending() && from < v.frame && ticks(20) < h.timer && pressed(c, button::SQUARE, 17) { h.set_state(c, PUNCH, true); }
            let v = c.anim.view();
            if (28.5 < v.frame && !v.blending()) || v.flags & 2 != 0 { h.set_state(c, IDLE, false); }
        }
        BEAM => {
            if c.anim.view().flags & 2 == 0 { return; }
            h.bodies.beam_lock = ticks(0x12d) as i16;
            h.set_state(c, IDLE, true);
        }
        DEATH if c.anim.view().flags & 2 != 0 => super::super::damage::death_fade(h),
        _ => {}
    }
}

/// The sphere under his feet (`0x26e830(3.5, damage, 1, hero, (x, y, z − 1), 0x30000, 0, 1, 0)`).
fn feet_hit(h: &mut Hero, damage: f32) {
    let c = [f(h.pos[0]), f(h.pos[1]), f(h.pos[2]) - 1.0];
    h.bodies.hits.push(BodyHit::Sphere { r: 3.5, damage, push: 1.0, centre: c, flags: 0x3_0000, b18: 0, b19: 1, sphere_flags: 0 });
}

/// The attacks both 0x5a and 0x5b start: ✕ (8) → 0x5e, □ (`square`) → 0x60, △ (8) with no lockout → 0x61, ○ (8) → 0x5f.
fn attack(h: &Hero, c: &Ctx, square: i32) -> Option<i32> {
    if pressed(c, button::CROSS, 8) { return Some(JUMP); }
    if pressed(c, button::SQUARE, square) { return Some(PUNCH); }
    if pressed(c, button::TRIANGLE, 8) && h.bodies.beam_lock == 0 { return Some(BEAM); }
    if pressed(c, button::CIRCLE, 8) { return Some(MISSILES); }
    None
}

fn tr_idle(h: &mut Hero, c: &mut Ctx) {
    let v = c.anim.view();
    if v.flags & 2 != 0 {
        if v.seq_b == 0 {
            if ticks(50) < h.timer {
                let r = c.rng.randf(0.0, 1.0);
                if r < 0.1 {
                    h.set_anim(c.anim, c.rng, blend(8), 1, 0);
                } else if r < 0.5 {
                    h.set_anim(c.anim, c.rng, blend(8), 2, 0);
                }
            }
        } else {
            h.set_anim(c.anim, c.rng, blend(12), 0, 0);
        }
    }
    let next = if c.env.pad.held & (button::L1 | button::L2) != 0 {
        1
    } else if let Some(s) = attack(h, c, 15) {
        s
    } else if h.air_ticks != 0 && p(2.0) < h.height {
        FALL
    } else {
        h.stick_target(c.env, Pf::ONE);
        if h.stick_mag <= p(0.22) || h.target_speed <= Pf::ZERO { return; }
        WALK
    };
    h.set_state(c, next, true);
}

fn tr_walk(h: &mut Hero, c: &mut Ctx) {
    h.anim_speed = h.eff_len * p(8.0);
    if h.anim_speed < p(0.75) { h.anim_speed = p(0.75); }
    let next = if let Some(s) = attack(h, c, 8) {
        s
    } else if (h.air_ticks as i32) < ticks(5) || h.height <= p(1.5) {
        if h.air_ticks != 0 || p(0.17) <= h.stick_mag { return; }
        if h.timer <= ticks(20) { return; }
        IDLE
    } else {
        FALL
    };
    h.set_state(c, next, true);
}

fn tr_jump(h: &mut Hero, c: &mut Ctx) {
    if h.jump.descending != 0 { feet_hit(h, 40.0); }
    let j = h.jump;
    if h.timer <= j.takeoff { return; }
    if h.timer == j.takeoff + ticks(1) {
        let n = (h.vel[2] / j.g).to_i32();
        let v = c.anim.view();
        h.anim_speed_for_ticks(j.f_apex, Pf::from_i32(n), j.ak, Pf::b(0xbf80_0000), &v);
        return;
    }
    let mut landed = h.jump.landed + 1;
    if h.jump.landed == 0 {
        landed = 0;
        if h.air_ticks == 0 && h.jump.descending != 0 {
            h.jump.landed = ticks(1);
            // The landing's camera shake 0x167260 / 0x167268 = 0.15, ticks(30).
            super::super::fx::shake(h, crate::follow_camera::ShakeAxis::Up, 0.15, ticks(0x1e));
            let fl = h.jump.f_land.to_i32();
            let v = c.anim.view();
            if p(v.frame) < h.jump.f_hold { h.set_anim(c.anim, c.rng, blend(6), v.seq_b, fl + 3); }
            landed = h.jump.landed;
        }
    }
    h.jump.landed = landed;
    if h.jump.landed == 0 {
        if h.jump.descending != 0 {
            let v = c.anim.view();
            if (v.frame_b as i32) < h.jump.f_hold.to_i32() {
                let e = h.land_eta(c, p(60.0), h.jump.g, Pf::b(0xbf80_0000));
                h.jump.land_eta = e.to_i32();
                h.anim_speed_for_ticks(h.jump.f_hold, Pf::from_i32(h.jump.land_eta), h.jump.ak, Pf::b(0xbf80_0000), &v);
                let fl = h.jump.f_land.to_i32();
                if p(1.8) < h.anim_speed { h.set_anim(c.anim, c.rng, Pf::from_i32(h.jump.land_eta + 2), v.seq_b, fl + 1); }
                if h.anim_speed < p(0.2) { h.anim_speed = p(0.2); }
                if p(2.7) < h.anim_speed { h.anim_speed = p(2.7); }
            }
            if p(c.anim.view().frame).to_i32() == h.jump.f_hold.to_i32() { h.anim_speed = Pf::ZERO; }
        }
    } else {
        h.anim_speed = Pf::ONE;
    }
    if h.jump.landed == 1 {
        let pos = [f(h.pos[0]), f(h.pos[1]), f(h.ground_z) + 0.2];
        h.bodies.spawns.push(Spawn::Shockwave { pos });
    }
    if h.jump.landed != 0 && h.lockout == 0 && pressed(c, button::CROSS, 4) && (h.air_ticks as i32) < ticks(4) {
        h.stick_target(c.env, Pf::ONE);
        h.set_state(c, JUMP, true);
        return;
    }
    if ticks(1) < h.jump.landed {
        h.f15d4 = 0;
        h.stick_target(c.env, Pf::ONE);
        // The run branch compares the target speed (u/tick) with the table's 0.82 (0x17bdc4): never taken.
        if SPEED_TABLE[0][3] < h.target_speed && p(0.5) < h.stick_mag && h.lockout == 0 && fast_diff_rots(h.target_yaw, h.rot[2]) < Pf::b(0x3fc9_0fdb) {
            h.set_state(c, WALK, true);
            return;
        }
        if ticks(12) < h.jump.landed { h.set_state(c, IDLE, true); }
    }
}

/// `0x2a9be0` (the hit intake's body-2 branch): the beam moby (class 0x5f3, the level's beam word) deleted when it is in
/// its first state, its point light freed, the screen record 0x15f320 / 0x15f330 cleared; the word = 0. Queued in
/// order with the other effect mobys of this tick ([`after_update`]: `classes::units::giant_beam::end`).
pub fn beam_end(h: &mut Hero) { h.bodies.spawns.push(Spawn::BeamEnd); }

// ------------------------------------------------------------------------------------------------
// HeroUpdateAlt mode 2: the pilot and the effect mobys.

/// HeroUpdateAlt's body-2 part after the write-back: `0x2061f0` (Ratchet in the cockpit: his +0x7f = 0; shown when the
/// body is, then his sequence 0x81 over 10 ticks and his generic advance; hidden otherwise) and the effect mobys Giant
/// Clank's states fired this tick ([`Spawn`], in the order the hero code called them: the missiles
/// `classes::units::giant_missile`, the shockwave `giant_shockwave`, the beam's point and end `giant_beam`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn after_update(h: &mut Hero, table: &mut MobyTable, ratchet: MobyId, body: MobyId, anim: &mut dyn AnimCtl, hits: &mut dyn super::super::items::HitSink, rng: &mut Rng, counter: u64) {
    let shown = table.mobys.get(body).is_some_and(|m| m.mode & crate::moby_runtime::mode::HIDDEN == 0);
    if let Some(r) = table.mobys.get_mut(ratchet) {
        r.b7f = 0;
        if shown {
            r.mode &= !(crate::moby_runtime::mode::HIDDEN | crate::moby_runtime::mode::NO_ANIM);
        } else {
            r.mode |= crate::moby_runtime::mode::HIDDEN | crate::moby_runtime::mode::NO_ANIM;
        }
    }
    if shown { anim.ratchet_generic(0x81, ticks(10)); }
    // The effect mobys, in the hero code's call order.
    let spawns = std::mem::take(&mut h.bodies.spawns);
    if spawns.is_empty() { return; }
    let hero = h.clone();
    let dt = DT.to_f32();
    hits.world(table, &hero, rng, counter, &mut |w| {
        use crate::moby_update::classes::units::{giant_beam, giant_missile, giant_shockwave};
        for s in &spawns {
            match *s {
                // `0x2a96f8(yaw, 0, 0.5, yaw, 0, hero, point, 0)`.
                Spawn::Missile { pos, yaw } => { giant_missile::spawn(w, yaw, 0.0, 0.5, yaw, 0.0, body, pos, None); }
                Spawn::Shockwave { pos } => {
                    let life = w.ticks(25);
                    giant_shockwave::spawn(w, 2.0, dt * 15.0, 40.0, pos, life);
                }
                Spawn::Beam { pos } => giant_beam::point(w, pos),
                Spawn::BeamEnd => giant_beam::end(w),
            }
        }
    });
}
