//! **The Hologuise disguise as the hero (body 3)**: the states 0x53..0x59 of the hero state machine, as level 01
//! compiles them (SetState `0x23cf98`, physics `0x2370b8`, transitions `0x242930`; level 05's `0x24cee8` / `0x244a70` /
//! `0x255960` are the same cases). The disguise moby is class 0x27a; the item and the way in / out are
//! [`crate::hero::hologuise`]. Sequences are the disguise class's: 0 idle, 1 walk, 2 hurt, 3 death, 4 the ○ pose, 5 the
//! idle fidget, 6 fall, 7 the landing.
//!
//! **Coverage** (`address | what | status`), level01 addresses:
//!
//! | address | what | status |
//! |---|---|---|
//! | SetState 0x53 | 0x1415d4 = 0, group 0; `SetAnim(ticks(14), 0, 0)` | ported ([`entry`]) |
//! | SetState 0x54 | group 1, 0x1415d4 = 0; speed = min(\|eff.xy\|, 1.5·dt); 0x13f708 = 0; `SetAnim(ticks(8), 1, 0)` | ported |
//! | SetState 0x55 | 0x13fc90 = 24·dt², group 2, 0x13fc94 = 7·dt², 0x1415d4 = 0, vel = eff; `SetAnim(ticks(7), 6, 0)` | ported |
//! | SetState 0x56 | group 7, 0x1415d4 = 0; motion ×0.5 (`0x2342d8`); vel = eff clamped to 5·dt; `HeroTakeDamage(1)`; 0x13f510 = `ticks(77)`; `SetAnim(−3, 2, 3)` and one curve step | ported |
//! | SetState 0x57 | group 0x14, 0x1415d4 = 0, momentum = eff; `SetAnim(ticks(12), 3, 0)` | ported |
//! | SetState 0x58 | group 2, 0x1415d4 = 0, vel = eff; `SetAnim(ticks(12), 6, 0)` | ported (no caller sets 0x58 on level 01: the state's code only) |
//! | SetState 0x59 | 0x1415d4 = 0, group 0; `SetAnim(ticks(7), 4, 0)`, the loop `0x247cb8(0xe, 0x22)` | ported |
//! | physics 0x53 / 0x57 / 0x59 | edge brake (3.7, 0), stick 1, speed → 0 at 12.6·dt², vel 0, `HeroDecayMomentum(12.6·dt²)`, wall check 0; in the air 25·dt² from eff.z + the steep-wall stop, on the ground 54·dt² | ported ([`physics`]) |
//! | physics 0x54 | residual r = \|yaw − target\|; stick 1.5·dt; target speed ≥ 0.825·dt; turn (0.035·(s + 0.35), 0.1, 130°/s·(s + 0.35)); speed step 7.5·max(1 − 0.3r, 0)·dt² / 8.5·dt²; planar velocity (the facing unless 0x13f708 and no 0x13f658); wall check 1; gravity as 0x53 | ported |
//! | physics 0x55 | stick 1.5·dt; air control (`HeroJumpHorizontal`) unless 0x13f514; gravity 21·dt² from vel.z; vz ≥ −15·dt | ported |
//! | physics 0x56 | \|vel.xy\| scaled (−0.08 on the ground, −0.01 in the air, ×0x15ed60); vz = 0; on the ground −0.004, in the air from eff.z −24·dt² | ported |
//! | physics 0x58 | 5.5·dt down the slope 0x13f63c, air accel 25·dt², gravity 18·dt² from eff.z | ported |
//! | transitions 0x53 | the body's sequence wrapped: from 0, a 50 % chance (`randf(0, 1)` < 0.5) of the fidget 5 (`ticks(8)`); else back to 0 (`ticks(12)`); L1 / L2 → 1; air → 0x55; ○ → 0x59; □ within 7 → the way out (voice 0x19, squash, timer) and `UpdateWrenchSelected(0)`; stick > 0.22 with a target speed → 0x54 | ported ([`transitions`]; `UpdateWrenchSelected(0)` made by the tick after the body's update, `Gadgets::wrench_select` [L: the same tick, the hand slot is empty in the body]) |
//! | transitions 0x54 | playback = \|eff\|·54; on the ground (air < 5) or low (≤ 0.8) on a gentle slope (≤ 50°): □ → the way out; ○ → 0x59; on the ground with stick < 0.17 → 0x53; else → 0x55 | ported |
//! | transitions 0x55 | in the air: nothing; landed: disp.z / vel.z ≥ −9·dt; no health → 0x57; stick ≤ 0.5, 40 ticks or a lockout → `SetState(0x53, 0)`, `SetAnim(−1, 7, 0)`, playback 0.35, lockout `ticks(20)`; else → 0x54 | ported |
//! | transitions 0x56 | after 32 ticks in the air above 1 → 0x55; the sequence wrapped → 0x53; after 45: health → `SetState(0x53, 0)`, else 0x57 | ported |
//! | transitions 0x57 | the sequence wrapped → the death fade `0x2319b0` | ported (`damage::death_fade`) |
//! | transitions 0x58 | after 300 the death fade; off the pit 0x14063a → 0x55 / 0x53 | ported |
//! | transitions 0x59 | after 45 ticks with ○ released → `SetState(0x53, 0)`, `SetAnim(ticks(20), 0, 0)` | ported |
//! | sounds, particles, lights, stats | none in these cases (the way out's voice: [`crate::hero::hologuise`]) | n/a |
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::assign_op_pattern)]

use super::super::common::blend;
use super::super::physics::*;
use super::super::states::Ctx;
use super::super::{AnimCtl, Hero};
use crate::pad::{button, fast_diff_rots};
use crate::ps2v::Pf;
use crate::rng::Rng;

pub const IDLE: i32 = 0x53;
pub const WALK: i32 = 0x54;
pub const FALL: i32 = 0x55;
pub const HURT: i32 = 0x56;
pub const DEATH: i32 = 0x57;
pub const PIT: i32 = 0x58;
pub const POSE: i32 = 0x59;

fn p(x: f32) -> Pf { Pf::f(x) }
fn pressed(c: &Ctx, mask: u32, n: i32) -> bool { c.env.pad.pressed_within(mask, ticks(n)).is_some() }

/// SetState's entry of the disguise's states.
pub(crate) fn entry(h: &mut Hero, c: &mut Ctx, id: i32, play: bool, _old_sub: i32) -> Option<bool> {
    match id {
        IDLE => {
            h.f15d4 = 0;
            h.group = 0;
            if play { h.set_anim(c.anim, c.rng, blend(0xe), 0, 0); }
        }
        WALK => {
            h.group = 1;
            h.f15d4 = 0;
            let l = len2(h.eff);
            h.speed = DT * p(1.5);
            if l <= h.speed { h.speed = l; }
            h.turn_to_target = 0;
            if play { h.set_anim(c.anim, c.rng, blend(8), 1, 0); }
        }
        FALL => {
            h.group_gravity = DT2 * p(24.0);
            h.group = 2;
            h.fc94 = DT2 * p(7.0);
            h.f15d4 = 0;
            h.vel = h.eff;
            if play { h.set_anim(c.anim, c.rng, blend(7), 6, 0); }
        }
        HURT => {
            h.group = 7;
            h.f15d4 = 0;
            h.scale_motion(Pf::b(0x3f00_0000));
            h.vel = h.eff;
            super::super::common::clamp_len_2745f0(&mut h.vel, DT * p(5.0));
            super::super::damage::take_damage(h, 1);
            h.f510 = ticks(0x4d);
            if play {
                h.set_anim(c.anim, c.rng, Pf::b(0xc040_0000), 2, 3);
                c.anim.curve_step();
            }
        }
        DEATH => {
            h.group = 0x14;
            h.f15d4 = 0;
            h.momentum = h.eff;
            if play { h.set_anim(c.anim, c.rng, blend(0xc), 3, 0); }
        }
        PIT => {
            h.group = 2;
            h.f15d4 = 0;
            h.vel = h.eff;
            if play { h.set_anim(c.anim, c.rng, blend(0xc), 6, 0); }
        }
        POSE => {
            h.f15d4 = 0;
            h.group = 0;
            if play {
                h.set_anim(c.anim, c.rng, blend(7), 4, 0);
                c.anim.set_loop(0xe, 0x22);
            }
        }
        _ => {}
    }
    None
}

/// The ground states' gravity tail: in the air 25·dt² from eff.z and the steep-wall stop `0x232820`, else 54·dt².
fn gravity(h: &mut Hero, env: &Env) {
    if h.air_ticks != 0 {
        h.gravity_from(h.eff_v[2], DT2 * p(25.0));
        h.climb_check(env);
    } else {
        let z = h.vel[2];
        h.gravity_from(z, DT2 * p(54.0));
    }
}

/// The per-state physics of the disguise's states. False: not one of them.
pub(crate) fn physics(h: &mut Hero, env: &Env, _anim: &mut dyn AnimCtl, _rng: &mut Rng) -> bool {
    match h.state {
        IDLE | DEATH | POSE => {
            h.edge_brake(env, Pf::b(0x406c_cccd), Pf::ZERO);
            h.stick_target(env, Pf::ONE);
            h.target_speed = Pf::ZERO;
            h.speed_step(Pf::ZERO, DT2 * p(12.6));
            h.vel = V0;
            h.momentum_decay(DT2 * p(12.6));
            h.wall_check(env, 0);
            gravity(h, env);
        }
        WALK => {
            let r = fast_diff_rots(h.rot[2], h.target_yaw);
            h.stick_target(env, DT * p(1.5));
            let floor = (DT * p(1.5)) * p(0.55);
            if h.target_speed < floor { h.target_speed = floor; }
            let mut k = Pf::ONE - r * p(0.3);
            if k < Pf::ZERO { k = Pf::ZERO; }
            let s = h.stick_mag + p(0.35);
            h.turn_to((SCALE64 * p(0.035)) * s, SCALE64 * p(0.1), (DT * p(2.268_928)) * s);
            h.speed_step((DT2 * p(7.5)) * k, DT2 * p(8.5));
            if h.turn_to_target == 0 || h.f658 != 0 { h.set_planar_vel(Pf::b(0x47c3_4f80)); } else { h.set_planar_vel(h.target_yaw); }
            h.wall_check(env, 1);
            gravity(h, env);
        }
        FALL => {
            h.stick_target(env, DT * p(1.5));
            if h.lockout == 0 { h.air_control(env); }
            let z = h.vel[2];
            h.gravity_from(z, DT2 * p(21.0));
            let floor = DT * p(-15.0);
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
        PIT => {
            h.target_speed = DT * p(5.5);
            h.target_yaw = h.slope_yaw;
            h.air_accel(DT2 * p(25.0));
            h.gravity_from(h.eff_v[2], DT2 * p(18.0));
        }
        _ => return false,
    }
    true
}

/// The way out from the transitions (□): the timer with voice 0x19 (when it is not running) and
/// `UpdateWrenchSelected(0)` (made by the tick after the body's update: `Gadgets::wrench_select`).
fn way_out(h: &mut Hero) {
    super::super::hologuise::way_out(h);
    h.gadgets.wrench_select = true;
}

/// The per-state transitions of the disguise's states.
pub(crate) fn transitions(h: &mut Hero, c: &mut Ctx) {
    match h.state {
        IDLE => tr_idle(h, c),
        WALK => tr_walk(h, c),
        FALL => tr_fall(h, c),
        HURT => {
            if !(h.timer < ticks(0x20) || h.air_ticks == 0 || h.height <= Pf::ONE) {
                h.set_state(c, FALL, true);
            } else if c.anim.view().flags & 2 != 0 {
                h.set_state(c, IDLE, true);
            } else if ticks(0x2d) < h.timer {
                if 0 < h.health { h.set_state(c, IDLE, false); } else { h.set_state(c, DEATH, true); }
            }
        }
        DEATH => {
            if c.anim.view().flags & 2 != 0 { super::super::damage::death_fade(h); }
        }
        PIT => {
            if ticks(300) < h.timer { super::super::damage::death_fade(h); }
            if h.f063a != 0 { return; }
            let s = if h.air_ticks == 0 { IDLE } else { FALL };
            h.set_state(c, s, true);
        }
        POSE => {
            if !(ticks(0x2d) < h.timer) || c.env.pad.held & button::CIRCLE != 0 { return; }
            h.set_state(c, IDLE, false);
            h.set_anim(c.anim, c.rng, blend(0x14), 0, 0);
        }
        _ => {}
    }
}

fn tr_idle(h: &mut Hero, c: &mut Ctx) {
    let v = c.anim.view();
    if v.flags & 2 != 0 {
        if v.seq_b == 0 {
            if c.rng.randf(0.0, 1.0) < 0.5 { h.set_anim(c.anim, c.rng, blend(8), 5, 0); }
        } else {
            h.set_anim(c.anim, c.rng, blend(0xc), 0, 0);
        }
    }
    let next = if c.env.pad.held & (button::L1 | button::L2) != 0 {
        1
    } else if h.air_ticks != 0 {
        FALL
    } else if c.env.pad.pressed & button::CIRCLE != 0 {
        POSE
    } else if pressed(c, button::SQUARE, 7) {
        way_out(h);
        return;
    } else {
        h.stick_target(c.env, Pf::ONE);
        if h.stick_mag <= p(0.22) || h.target_speed <= Pf::ZERO { return; }
        WALK
    };
    h.set_state(c, next, true);
}

fn tr_walk(h: &mut Hero, c: &mut Ctx) {
    h.anim_speed = h.eff_len * p(54.0);
    let low = h.height <= p(0.8) && h.slope <= p(0.872_664_6);
    let next = if (h.air_ticks as i32) < ticks(5) || low {
        if pressed(c, button::SQUARE, 7) {
            way_out(h);
            return;
        }
        if c.env.pad.pressed & button::CIRCLE != 0 {
            POSE
        } else if h.air_ticks != 0 || p(0.17) <= h.stick_mag {
            return;
        } else {
            IDLE
        }
    } else {
        FALL
    };
    h.set_state(c, next, true);
}

fn tr_fall(h: &mut Hero, c: &mut Ctx) {
    if h.air_ticks != 0 { return; }
    let lim = DT * p(-9.0);
    if h.disp[2] < lim { h.disp[2] = lim; }
    if h.vel[2] < lim { h.vel[2] = lim; }
    if h.health < 1 {
        h.set_state(c, DEATH, true);
    } else if h.stick_mag <= p(0.5) || ticks(0x28) <= h.timer || h.lockout != 0 {
        h.set_state(c, IDLE, false);
        h.set_anim(c.anim, c.rng, Pf::b(0xbf80_0000), 7, 0);
        h.anim_speed = p(0.35);
        h.lockout = ticks(0x14);
    } else {
        h.set_state(c, WALK, true);
    }
}
