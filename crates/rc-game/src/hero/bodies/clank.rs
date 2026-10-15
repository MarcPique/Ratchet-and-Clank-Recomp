//! **Clank as the hero (body 1)**: the states 0x43..0x52 and 0x7d of the hero state machine, as level 00 compiles them
//! (SetState L00 `0x2223f8`, physics `0x217970`, transitions `0x229b70`; levels 04, 06, 07, 09, 10, 13, 17 compile the
//! same cases), the lean `0x215b68` and Clank's part of `HeroUpdateAlt` (`0x2062b0`: the antenna glow 0x4b4, the rotor
//! 0x47a, the command flash). Clank has his own health 0x1415fc (`super::switch_character` swaps it in), his own
//! capsule, probes and ledge heights (`physics.rs`, `ledge.rs`); he has no items, no weapons, no packs (the weapon check
//! refuses in a body). Sequences are Clank's class's (0x57).
//!
//! **Coverage** (`address | what | ported (fn) / NOT ported (gap) / n/a (why)`), L00 addresses:
//!
//! | address | what | status |
//! |---|---|---|
//! | SetState 0x43 (0x2268b0) | group 0, 0x1415d4 = 0; pit (0x14063a, 0x13f65c = 0) → 0x52 (SetState returns 0); anim 0 over 9 | ported ([`entry`]) |
//! | SetState 0x44 (0x226a7c) | pit → 0x52; group 1; speed = \|eff.xy\| (0 after the stop 3, \|momentum.xy\| from group 0); 0x13f708 = 0; anim 2 over 8 | ported |
//! | SetState 0x45 (0x226964) | group 2, 0x13fc94 = 7·dt², vel = eff; anim 4 over 8 | ported |
//! | SetState 0x46 (0x226a04) | group 7; motion ×0.5; vel = eff clamped to 5·dt; `HeroTakeDamage(1)`; flash 0x13f53e = 45, invulnerable 0x13f510 = 50; anim 6 on curve −3 from 3, one curve step | ported |
//! | SetState 0x47 (0x226b10) | group 0x14, health 0, momentum = eff; anim 5 over 12; (epilogue: weapon put away, 0x1413fc = 1) | ported |
//! | SetState 0x49 / 0x4c (0x224590, the jump case) | the jump lockout 0x13f542 undoes the change; the jump defaults; 0x49: h 1.3..1.35 (ramp 14, takeoff 5), frames 18 / 27 / 27, bottom 0.6, gravity 25·dt², air speed 2·dt, windup brake 50·dt², anim 7 over 5 from frame 5; 0x4c: the climb (h 1.55..1.59, ramp 1, takeoff 13, frames 10 / 18 / 18, bottom 0.85, gravity 28·dt², 0x13f7ac = 0), anim 9 over 4 | ported ([`jump_entry`]) |
//! | SetState 0x4a (0x226b58) | group 3, 0x1415d4 = 0xd, 0x13f83c = 12·dt², 0x13f840 = z (the jump's g / reference height from a jump); anim 8 over 7 + 2 from frame 18 | ported |
//! | SetState 0x4b / 0x4d / 0x4e | group 3, 0x1415d4 = 0xd; anims 10 / 11 / 12 over 6 | ported |
//! | SetState 0x4f (0x2268f4) | group 5; \|eff\| > 4.5·dt → motion scaled to 4.5·dt; vel = disp; anim 0xe over 12 | ported |
//! | SetState 0x51 (0x225bdc → 0x2261f0) | the group-6 prologue (0x13fdbc = 1, group 6, aim cleared, hit flag cleared); the kick row 7 / 8 (chains from the other within its chain window), aim (5, 45°, 45°); anim 0xf / 0x10 over 5 from frame 1 | ported |
//! | SetState 0x52 (0x2269c0) | group 2, vel = eff; anim 4 over 12 | ported |
//! | SetState 0x7d (0x226848) | group 0x14; 0x13f51c = 10000; 0x13f644 = ground z; health 0; Ratchet's moby at the hero; `PlayClassSound(9, 0, Ratchet)`; anim 0x11 over 8 | ported |
//! | physics 0x43 / 0x47 | edge brake (3.7, 0), stick, speed → 0 at 12.6·dt², vel = 0, wall check 0, gravity (25·dt² from eff in the air + the climb check, 54·dt² on the ground) | ported ([`physics`]) |
//! | physics 0x44 / 0x50 | stick × 3·dt; turn (0.035 / 0.008 × (stick + 0.35), 0.15, 550° / 570° × (stick + 0.35) per s by speed); speed step 7.5·(1 − 0.3·residual)·dt² / 8.5·dt²; planar velocity; wall check 1; gravity | ported |
//! | physics 0x45 | stick × 3·dt; air control unless 0x13f514; gravity 12·dt², ≥ −10·dt | ported |
//! | physics 0x46 | \|vel.xy\| ×0.92 (×0.99 in the air); vel.z = 0; on the ground −0.004, in the air from eff −24·dt² | ported |
//! | physics 0x48 | none | ported (no-op) |
//! | physics 0x49 / 0x4c | the jump case (`jump.rs`), 0x4c's horizontal: forward 1.25·dt over key 5..15, 0 by 18 | ported (`phys_jump`, [`climb_horizontal`]) |
//! | physics 0x4a / 0x4b | face the wall (0.02, 0.2, 270°/s), pull to the hang point ≤ 1.7·dt, 0x13f840 → z at 3·dt | ported |
//! | physics 0x4d / 0x4e | vel 0, playback 1.3; probe B here and 0.3 aside; both within 30°: turn to their mean, move to their midpoint at Clank's table 0x1c3cf0[key]·0.5 | ported |
//! | physics 0x4f | stick × 2·dt; turn (0.04, 0.2, 500°/s); speed step 15 / 7·dt²; planar velocity; vz = −1.44·dt | ported |
//! | physics 0x51 | speed 3.5·dt before key 5.5; in the hit window a sphere hit (0.25 at the mean of joint lists 2 / 3, 0 / 1 for the second kick; damage 1, push 1, flags 0x10000, types 0 / 1); playback → 1 (0.2 a tick); the aim; speed step 37 / 28·dt²; gravity; edge brake | ported |
//! | physics 0x52 | 5.5·dt down the slope (0x13f63c), air accel 25·dt², gravity 18·dt² from eff.z | ported |
//! | physics 0x7d | vel 0, vz = −0.35·dt | ported |
//! | transitions 0x43 | the command flash (> 42 of 57 left) → anim 0x12; on a wrap a 50 % chance of the fidget 1 (from 0) or back to 0; pit → 0x52; L1 / L2 → 1; air → 0x45; ✕ (8) → 0x49; □ (9) → 0x51; stick > 0.2 → 0x44; anim 0x12 over → 0 | ported ([`transitions`]) |
//! | transitions 0x44 / 0x50 | walk ↔ run (2 / 3, frame carried over, below 1.5·dt / above 1.8·dt), playback = \|eff\|·90 / ·20 (0x50: ·196); ✕ → 0x49; □ → 0x51; air ≥ 5 ticks above 0.8 or a slope → 0x45; stick < 0.17 → 0x43 | ported |
//! | transitions 0x45 | level 6 below z 115: the death fade; landing: vz clamps (−9·dt), → 0x47 / 0x44 / 0x43; in the air: ledge 0x4a, ✕ (5) above 0.6 → glide 0x4f | ported |
//! | transitions 0x46 | after 32 in the air above 1 → 0x45; after 35 → 0x43 (no anim) / 0x47 | ported |
//! | transitions 0x47 | the anim wrapped → the death fade `0x2319b0` | ported |
//! | transitions 0x49 / 0x4c | the apex playback (0.5), the landing frame, the descent playback (60-tick ETA, 0.2..2.7), the bunny hop, after landing → 0x43 (12 ticks; the run branch compares the target speed with the table's 0.82: never), → 0x45 above 2 after 40, □ near the ground → 0x51, the ledge 0x4a, ✕ (5) above 0.55 → 0x4f (not 0x4c) | ported |
//! | transitions 0x4a | wrap → 0x4b | ported |
//! | transitions 0x4b | probe C lost → 0x45 (lockout 10); back + ✕ / R1 / R2 → 0x45 (10); ✕ → 0x4c; stick to a side + probe C there → 0x4d / 0x4e | ported |
//! | transitions 0x4d / 0x4e | stick released / turned / the ledge ends + wrap → 0x4b; back + ✕ → 0x45 (lockout 40); ✕ → 0x4c | ported |
//! | transitions 0x4f | after 30 without ✕ held → 0x45; the ledge → 0x4a; landed: 0x13f524 for an early landing, → 0x44 / 0x43 | ported |
//! | transitions 0x51 | pit → 0x52; past the idle frame → 0x43 (no anim); □ in the chain window → 0x51; ✕ after the jump frame → 0x49 | ported |
//! | transitions 0x52 | after 300 the death fade; off the pit → 0x45 / 0x43 | ported |
//! | transitions 0x7d | 0.8 under the burn floor or after 150 → the death fade | ported |
//! | 0x215b68 | the lean (records 24..26) in 0x44, 0x4f, groups 2 / 4 | ported ([`lean`]) |
//! | HeroUpdateAlt mode 1 | the glow moby, the rotor, the command flash (module `super`) | ported ([`after_update`]) |
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

pub const IDLE: i32 = 0x43;
pub const WALK: i32 = 0x44;
pub const FALL: i32 = 0x45;
pub const HURT: i32 = 0x46;
pub const DEATH: i32 = 0x47;
pub const JUMP: i32 = 0x49;
pub const LEDGE_GRAB: i32 = 0x4a;
pub const LEDGE_HANG: i32 = 0x4b;
pub const CLIMB: i32 = 0x4c;
pub const SHIMMY_L: i32 = 0x4d;
pub const SHIMMY_R: i32 = 0x4e;
pub const GLIDE: i32 = 0x4f;
pub const KICK: i32 = 0x51;
pub const PIT: i32 = 0x52;
pub const BURN: i32 = 0x7d;

fn p(x: f32) -> Pf { Pf::f(x) }
fn f(x: Pf) -> f32 { x.to_f32() }

/// The kick rows 7 / 8 of the melee table (L00 0x17bc28 + 0x2c·row): `[kind, step, chain-before, input-ref,
/// chain-from, jump-after, idle-after, hit-from, hit-to, +0x24, +0x28]`, key times of Clank's sequences 0xf / 0x10.
pub const KICK_ROWS: [[i32; 11]; 2] = [[0, 0, 16, 3, 12, 8, 10, 3, 6, 99, 99], [0, 1, 16, 3, 12, 8, 10, 3, 6, 99, 99]];
const K_KIND: usize = 0;
const K_STEP: usize = 1;
const K_CHAIN_BEFORE: usize = 2;
const K_REF: usize = 3;
const K_CHAIN_FROM: usize = 4;
const K_JUMP_AFTER: usize = 5;
const K_IDLE_AFTER: usize = 6;
const K_HIT_FROM: usize = 7;
const K_HIT_TO: usize = 8;

/// Clank's shimmy speed by key time (L00 0x1c3cf0; ×0.5 in the physics; beyond the table: 0).
pub const SHIMMY_SPEED: [f32; 28] = [
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.02, 0.02, 0.042, 0.042, 0.042, 0.042, 0.033, 0.033, 0.013, 0.013, 0.013, 0.013,
    0.013, 0.013, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
];

/// Clank's ledge / wall dimensions for the ledge probes B and C (`ledge.rs`; L00 0x20bed0 / 0x20c758's body-1 values).
pub const LEDGE: super::super::ledge::LedgeDims =
    super::super::ledge::LedgeDims { top: 0.725, low: 0.7, step: 0.25, min_above: 0.7, wall_from: -0.4, hang: -0.71, out: 0.32 };

fn kick_row(h: &Hero) -> &'static [i32; 11] { &KICK_ROWS[(h.melee.combo - 7).clamp(0, 1) as usize] }
fn pressed(c: &Ctx, mask: u32, n: i32) -> bool { c.env.pad.pressed_within(mask, ticks(n)).is_some() }

// ------------------------------------------------------------------------------------------------
// SetState.

/// SetState's entry of Clank's states. `None`: the epilogue follows; `Some(r)`: SetState returns `r`.
pub(crate) fn entry(h: &mut Hero, c: &mut Ctx, id: i32, play: bool, old_sub: i32) -> Option<bool> {
    let pit = |h: &Hero| h.f063a != 0 && h.f65c == 0;
    match id {
        IDLE => {
            h.group = 0;
            h.f15d4 = 0;
            if pit(h) { return Some(h.set_state(c, PIT, true) && false); }
            if play { h.set_anim(c.anim, c.rng, blend(9), 0, 0); }
        }
        WALK => {
            if pit(h) { return Some(h.set_state(c, PIT, true) && false); }
            h.group = 1;
            h.f15d4 = 0;
            h.speed = len2(h.eff);
            if h.prev_state == 3 { h.speed = Pf::ZERO; }
            if h.prev_group == 0 { h.speed = len2(h.momentum); }
            h.turn_to_target = 0;
            if play { h.set_anim(c.anim, c.rng, blend(8), 2, 0); }
        }
        FALL => {
            h.group = 2;
            h.fc94 = DT2 * p(7.0);
            h.f15d4 = 0;
            h.vel = h.eff;
            if play { h.set_anim(c.anim, c.rng, blend(8), 4, 0); }
        }
        HURT => {
            h.group = 7;
            h.f15d4 = 0;
            h.scale_motion(Pf::b(0x3f00_0000));
            h.vel = h.eff;
            super::super::common::clamp_len_2745f0(&mut h.vel, DT * p(5.0));
            super::super::damage::take_damage(h, 1);
            h.f53e = ticks(45) as i16;
            h.f510 = ticks(50);
            if play {
                h.set_anim(c.anim, c.rng, Pf::b(0xc040_0000), 6, 3);
                c.anim.curve_step();
            }
        }
        DEATH => {
            h.group = 0x14;
            h.f15d4 = 0;
            h.health = 0;
            h.momentum = h.eff;
            if play { h.set_anim(c.anim, c.rng, blend(12), 5, 0); }
        }
        JUMP | CLIMB => {
            if h.jump_lockout != 0 {
                h.state = h.prev_state;
                h.timer = h.prev_timer;
                h.substate = old_sub;
                return Some(false);
            }
            jump_entry(h, c, id, play);
        }
        LEDGE_GRAB => {
            h.ledge_blk.f83c = f(DT2) * 12.0;
            h.group = 3;
            h.f15d4 = 0xd;
            h.ledge_blk.f840 = f(h.pos[2]);
            if h.prev_group == 4 {
                h.ledge_blk.f83c = f(h.jump.g);
                h.ledge_blk.f840 = f(h.jump.ref_z);
            }
            if play { h.set_anim(c.anim, c.rng, Pf::from_i32(ticks(7) + 2), 8, 18); }
        }
        LEDGE_HANG | SHIMMY_L | SHIMMY_R => {
            h.group = 3;
            h.f15d4 = 0xd;
            let seq = match id { LEDGE_HANG => 10, SHIMMY_L => 11, _ => 12 };
            if play { h.set_anim(c.anim, c.rng, blend(6), seq, 0); }
        }
        GLIDE => {
            h.group = 5;
            h.f15d4 = 0;
            let cap = DT * p(4.5);
            if cap < h.eff_len { h.scale_motion(cap / h.eff_len); }
            h.vel = h.disp;
            if play { h.set_anim(c.anim, c.rng, blend(12), 0xe, 0); }
        }
        KICK => kick_entry(h, c, play),
        PIT => {
            h.group = 2;
            h.f15d4 = 0;
            h.vel = h.eff;
            if play { h.set_anim(c.anim, c.rng, blend(12), 4, 0); }
        }
        BURN => {
            h.group = 0x14;
            h.no_vel_clamp = 10000;
            h.damage.burn_floor = f(h.ground_z);
            h.f15d4 = 0;
            h.health = 0;
            // Ratchet's moby at the hero, then `PlayClassSound(9, 0, Ratchet)` (his class's sound 9, from there).
            h.bodies.cmds.push(super::BodyCmd::RatchetPlace([f(h.pos[0]), f(h.pos[1]), f(h.pos[2]), f(h.pos[3])]));
            h.bodies.ratchet_sounds.push((9, 0));
            if play { h.set_anim(c.anim, c.rng, blend(8), 0x11, 0); }
        }
        _ => {}
    }
    None
}

/// The jump case's part for 0x49 / 0x4c (L00 SetState case 7 … 0x69, branches 0x49 / 0x4c): the jump block's
/// defaults (`jump_block_defaults`), then Clank's parameters. 0x49: h 1.3..1.35 over 14 ticks after a 5-tick windup,
/// frames 18 / 27 / 27, bottom 0.6, gravity 25·dt², air speed 2·dt, windup brake 50·dt²; anim 7 over 5 from frame 5.
/// 0x4c (the climb): h 1.55..1.59 (ramp 1) after 13, frames 10 / 18 / 18, bottom 0.85, gravity 28·dt², 0x13f7ac = 0;
/// anim 9 over 4.
fn jump_entry(h: &mut Hero, c: &mut Ctx, id: i32, play: bool) {
    h.jump_block_defaults(c.rng);
    let j = &mut h.jump;
    if id == JUMP {
        j.h = p(1.3);
        j.takeoff = ticks(5);
        j.bottom784 = Pf::b(0x3f19_999a);
        (j.f_apex, j.f_hold, j.f_land) = (p(18.0), p(27.0), p(27.0));
        j.hmax = p(1.35);
        j.hmin = p(1.3);
        j.ramp = ticks(14) as i16;
        j.g = DT2 * p(25.0);
        j.air_speed = DT + DT;
        j.dec = DT2 * p(50.0);
        if play { h.set_anim(c.anim, c.rng, blend(5), 7, 5); }
    } else {
        j.h = p(1.55);
        j.takeoff = ticks(13);
        j.bottom784 = Pf::b(0x3f59_999a);
        (j.f_apex, j.f_hold, j.f_land) = (p(10.0), p(18.0), p(18.0));
        j.hmax = p(1.59);
        j.hmin = p(1.55);
        j.ramp = ticks(1) as i16;
        j.g = DT2 * p(28.0);
        j.speed7ac = Pf::ZERO;
        if play { h.set_anim(c.anim, c.rng, blend(4), 9, 0); }
    }
}

/// SetState 0x51 (the group-6 entry with Clank's kick rows).
fn kick_entry(h: &mut Hero, c: &mut Ctx, play: bool) {
    h.melee.speed_k = Pf::ONE;
    h.group = 6;
    h.f15d4 = 0;
    h.melee.aimed = 0;
    h.melee.target = None;
    h.melee.hit = 0;
    let old = *kick_row(h);
    let chaining = h.prev_group == 6 || (h.prev_prev_group == 6 && matches!(c.anim.view().seq_b, 0xf | 0x10));
    let mut step = 0;
    if chaining && h.melee.combo >= 7 && old[K_KIND] == 0 && p(c.anim.view().frame) < Pf::from_i32(old[K_CHAIN_BEFORE]) {
        step = (old[K_STEP] + 1) % 2;
    }
    h.melee.combo = step + 7;
    h.aim_assist(c.env, 5.0, std::f32::consts::FRAC_PI_4, std::f32::consts::FRAC_PI_4);
    if play { h.set_anim(c.anim, c.rng, blend(5), (0xf + step) as u8, 1); }
}

// ------------------------------------------------------------------------------------------------
// Physics.

/// The per-state physics of Clank's states (L00 0x217970). False: not Clank's.
pub(crate) fn physics(h: &mut Hero, env: &Env, anim: &mut dyn AnimCtl, rng: &mut Rng) -> bool {
    let _ = rng;
    match h.state {
        IDLE | DEATH => {
            h.edge_brake(env, Pf::b(0x406c_cccd), Pf::ZERO);
            h.stick_target(env, Pf::ONE);
            h.target_speed = Pf::ZERO;
            h.speed_step(Pf::ZERO, DT2 * p(12.6));
            h.vel = V0;
            h.wall_check(env, 0);
            gravity(h, env);
        }
        WALK | 0x50 => {
            let r = fast_diff_rots(h.rot[2], h.target_yaw);
            let t = DT * p(3.0);
            h.stick_target(env, t);
            let mut k = Pf::ONE - r * Pf::b(0x3e99_999a);
            let s = h.stick_mag + Pf::b(0x3eb3_3333);
            if k < Pf::ZERO { k = Pf::ZERO; }
            if h.eff_len < t * Pf::b(0x3f00_0000) {
                h.turn_to((SCALE64 * Pf::b(0x3d0f_5c29)) * s, SCALE64 * Pf::b(0x3e19_999a), (DT * Pf::b(0x4119_96c7)) * s);
            } else {
                h.turn_to((SCALE64 * Pf::b(0x3c03_126f)) * s, SCALE64 * Pf::b(0x3e19_999a), (DT * Pf::b(0x411f_2c8d)) * s);
            }
            h.speed_step((DT2 * p(7.5)) * k, DT2 * p(8.5));
            if h.turn_to_target == 0 || h.f658 != 0 { h.set_planar_vel(Pf::b(0x47c3_4f80)); } else { h.set_planar_vel(h.target_yaw); }
            h.wall_check(env, 1);
            gravity(h, env);
        }
        FALL => {
            h.stick_target(env, DT * p(3.0));
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
                h.vel[2] = Pf::ZERO;
                h.gravity_from(h.eff[2], DT2 * p(24.0));
            }
        }
        0x48 => {}
        JUMP | CLIMB => {
            let key = anim.view().frame;
            h.phys_jump_anim(env, key);
        }
        LEDGE_GRAB | LEDGE_HANG => {
            h.target_yaw = fast_add_rotations(p(h.ledge_blk.yaw), PI);
            h.turn_to(SCALE64 * p(0.02), SCALE64 * p(0.2), DT * p(4.712_389));
            let pt = h.ledge_blk.point;
            h.vel = vsub([p(pt[0]), p(pt[1]), p(pt[2]), h.vel[3]], h.pos);
            super::super::common::clamp_len_2745f0(&mut h.vel, DT * p(1.7));
            let mut z = p(h.ledge_blk.f840);
            approach(h.pos[2], DT * p(3.0), &mut z);
            h.ledge_blk.f840 = f(z);
        }
        SHIMMY_L | SHIMMY_R => shimmy_physics(h, env, &anim.view()),
        GLIDE => {
            let t = DT + DT;
            h.stick_target(env, t);
            h.turn_to(SCALE64 * p(0.04), SCALE64 * p(0.2), DT * p(8.726_646));
            h.speed_step(DT2 * p(15.0), DT2 * p(7.0));
            h.set_planar_vel(Pf::b(0x47c3_4f80));
            h.vel[2] = Pf::ZERO;
            h.gravity_from(Pf::ZERO, t * p(0.72));
        }
        KICK => kick_physics(h, env, anim),
        PIT => {
            h.target_speed = DT * p(5.5);
            h.target_yaw = h.slope_yaw;
            h.air_accel(DT2 * p(25.0));
            h.gravity_from(h.eff_v[2], DT2 * p(18.0));
        }
        BURN => {
            h.vel = V0;
            h.vel[2] = -(DT * p(0.35));
        }
        _ => return false,
    }
    true
}

/// The ground states' gravity tail (`0x2334d0` 25·dt² from eff.z in the air + `0x2121c0`, else 54·dt²).
fn gravity(h: &mut Hero, env: &Env) {
    if h.air_ticks != 0 {
        h.gravity_from(h.eff_v[2], DT2 * p(25.0));
        h.climb_check(env);
    } else {
        let z = h.vel[2];
        h.gravity_from(z, DT2 * p(54.0));
    }
}

/// 0x4d / 0x4e: probe B at the feet and 0.3 to the side; both found and within 30°: move to their midpoint.
fn shimmy_physics(h: &mut Hero, env: &Env, v: &super::super::AnimView) {
    h.vel[0] = Pf::ZERO;
    h.vel[1] = Pf::ZERO;
    h.vel[2] = Pf::ZERO;
    h.anim_speed = p(1.3);
    let saved = h.pos;
    let side = if h.state == SHIMMY_L { Pf::b(0x3fc9_0fdb) } else { Pf::b(0xbfc9_0fdb) };
    let a = fast_add_rotations(h.rot[2], side);
    let there = [h.pos[0] + fast_cos(a) * p(0.3), h.pos[1] + fast_sin(a) * p(0.3), h.pos[2], h.pos[3]];
    super::super::ledge::wall_ledge_probe_b(h, env, v);
    if h.f838 == 0 { return; }
    let p1 = h.ledge_blk.point;
    let y1 = fast_add_rotations(p(h.ledge_blk.yaw), PI);
    h.pos = there;
    super::super::ledge::wall_ledge_probe_b(h, env, v);
    h.pos = saved;
    if h.f838 == 0 { return; }
    let p2 = h.ledge_blk.point;
    let y2 = fast_add_rotations(p(h.ledge_blk.yaw), PI);
    if Pf::b(0x3f06_0a92) < fast_diff_rots(y1, y2) { return; }
    let t = fast_add_rotations(fast_subtract_rotations(y1, y2) * Pf::b(0x3f00_0000), y1);
    h.target_yaw = t;
    h.turn_to(SCALE64 * p(0.04), SCALE64 * p(0.2), DT * p(6.283_185_5));
    // Approach(target·dt, 5·dt², &|eff.xy|): the result is left unused by the game.
    let mid = [(p1[0] + p2[0]) * 0.5, (p1[1] + p2[1]) * 0.5, (p1[2] + p2[2]) * 0.5];
    h.vel = vsub([p(mid[0]), p(mid[1]), p(mid[2]), h.vel[3]], h.pos);
    let key = Pf::f(v.frame).to_i32();
    let s = usize::try_from(key).ok().and_then(|i| SHIMMY_SPEED.get(i)).copied().unwrap_or(0.0);
    h.vel = set_len3(h.vel, p(s * 0.5));
}

/// 0x51: the kick.
fn kick_physics(h: &mut Hero, env: &Env, anim: &mut dyn AnimCtl) {
    h.target_speed = Pf::ZERO;
    let row = *kick_row(h);
    let v = anim.view();
    let key = p(v.frame);
    if (row[K_STEP] == 0 || row[K_STEP] == 1) && key < p(5.5) { h.target_speed = DT * p(3.5); }
    if !v.blending() && Pf::from_i32(row[K_HIT_FROM]) <= key && key <= Pf::from_i32(row[K_HIT_TO]) {
        let j = if row[K_STEP] == 1 { 0 } else { 2 };
        let a = super::super::fx::joint_point(h, &*anim, j);
        let b = super::super::fx::joint_point(h, &*anim, j | 1);
        let c = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5, (a[2] + b[2]) * 0.5];
        h.bodies.hits.push(BodyHit::Sphere { r: 0.25, damage: 1.0, push: 1.0, centre: c, flags: 0x1_0000, b18: 0, b19: 1, sphere_flags: 0 });
    }
    let mut s = h.anim_speed;
    approach(Pf::ONE, p(0.2), &mut s);
    h.anim_speed = s;
    let mut k = DT2;
    let aimed_branch = h.melee.aimed != 0 || h.melee.target.is_some();
    if !aimed_branch {
        let mut aim = true;
        if row[K_STEP] == 1 && !v.blending() && p(7.0) < key { aim = false; }
        if aim { h.aim_assist(env, 5.0, std::f32::consts::FRAC_PI_4, std::f32::consts::FRAC_PI_4); }
    } else {
        let mut sc = SCALE64;
        if row[K_STEP] == 1 && Pf::ZERO < fast_subtract_rotations(h.melee.aim_yaw, h.rot[2]) {
            let d = fast_diff_rots(h.melee.aim_yaw, h.rot[2]);
            h.anim_speed = Pf::ONE / (d + Pf::ONE);
            sc = SCALE64;
        }
        h.target_yaw = h.melee.aim_yaw;
        h.turn_to(sc * p(0.05), sc * p(0.2), DT * p(15.009_831));
        k = DT2;
    }
    h.speed_step(k * p(37.0), k * p(28.0));
    if h.melee.aimed == 0 { h.set_planar_vel(Pf::b(0x47c3_4f80)); } else { h.set_planar_vel(h.melee.aim_yaw); }
    if h.air_ticks == 0 {
        h.vel[2] = h.vel[2] - DT2 * p(54.0);
    } else {
        h.vel[2] = h.disp[2] - DT2 * p(25.0);
        h.climb_check(env);
    }
    h.edge_brake(env, Pf::b(0x406c_cccd), Pf::ZERO);
}

/// `HeroJumpHorizontal` 0x214658's 0x4c branch (the climb): the forward speed 0x13f7ac → 1.25·dt over key 5..15, then 0
/// by 18, along the facing.
pub(crate) fn climb_horizontal(h: &mut Hero, key: f32) {
    if (5.0..=18.0).contains(&key) {
        let t = if key <= 15.0 { DT * p(1.25) } else { Pf::ZERO };
        let mut s = h.jump.speed7ac;
        approach(t, DT2 * p(8.0), &mut s);
        h.jump.speed7ac = s;
    }
    h.vel[0] = fast_cos(h.rot[2]) * h.jump.speed7ac;
    h.vel[1] = fast_sin(h.rot[2]) * h.jump.speed7ac;
}

// ------------------------------------------------------------------------------------------------
// Transitions.

/// The per-state transitions of Clank's states (L00 0x229b70).
pub(crate) fn transitions(h: &mut Hero, c: &mut Ctx) {
    match h.state {
        IDLE => tr_idle(h, c),
        WALK | 0x50 => tr_walk(h, c),
        FALL => tr_fall(h, c),
        HURT => {
            if !(h.timer < ticks(32) || h.air_ticks == 0 || h.height <= Pf::ONE) {
                h.set_state(c, FALL, true);
            } else if ticks(35) < h.timer {
                if 0 < h.health { h.set_state(c, IDLE, false); } else { h.set_state(c, DEATH, true); }
            }
        }
        DEATH => {
            if c.anim.view().flags & 2 != 0 { super::super::damage::death_fade(h); }
        }
        JUMP | CLIMB => tr_jump(h, c),
        LEDGE_GRAB => {
            if c.anim.view().flags & 2 != 0 { h.set_state(c, LEDGE_HANG, true); }
        }
        LEDGE_HANG => tr_hang(h, c),
        SHIMMY_L | SHIMMY_R => tr_shimmy(h, c),
        GLIDE => tr_glide(h, c),
        KICK => tr_kick(h, c),
        PIT => {
            if ticks(300) < h.timer { super::super::damage::death_fade(h); }
            if h.f063a != 0 { return; }
            let s = if h.air_ticks == 0 { IDLE } else { FALL };
            h.set_state(c, s, true);
        }
        BURN if h.pos[2] < p(h.damage.burn_floor) - p(0.8) || ticks(150) < h.timer => super::super::damage::death_fade(h),
        _ => {}
    }
}

fn tr_idle(h: &mut Hero, c: &mut Ctx) {
    if (ticks(57) - ticks(15)) < h.bodies.flash as i32 {
        h.set_anim(c.anim, c.rng, blend(10), 0x12, 0);
        return;
    }
    let v = c.anim.view();
    if h.bodies.flash == 0 && v.flags & 2 != 0 {
        if v.seq_b == 0 {
            if c.rng.randf(0.0, 1.0) < 0.5 { h.set_anim(c.anim, c.rng, blend(8), 1, 0); }
        } else {
            h.set_anim(c.anim, c.rng, blend(12), 0, 0);
        }
    }
    let next = if h.f063a != 0 {
        PIT
    } else if c.env.pad.held & (button::L1 | button::L2) != 0 {
        1
    } else if h.air_ticks != 0 {
        FALL
    } else if pressed(c, button::CROSS, 8) {
        JUMP
    } else if pressed(c, button::SQUARE, 9) {
        KICK
    } else {
        h.stick_target(c.env, Pf::ONE);
        if !(h.stick_mag <= p(0.2) || h.target_speed <= Pf::ZERO) {
            WALK
        } else {
            if h.bodies.flash == 0 && c.anim.view().seq_b == 0x12 { h.set_anim(c.anim, c.rng, blend(12), 0, 0); }
            return;
        }
    };
    h.set_state(c, next, true);
}

fn tr_walk(h: &mut Hero, c: &mut Ctx) {
    if h.f063a != 0 {
        h.set_state(c, PIT, true);
        return;
    }
    if h.state == WALK {
        let v = c.anim.view();
        if !v.blending() {
            let t = DT * p(3.0);
            if !(h.eff_len < t * Pf::b(0x3f00_0000)) || v.seq_b == 2 {
                if t * Pf::b(0x3f19_999a) < h.eff_len && v.seq_b != 3 {
                    let (fc2, fc3) = (c.anim.frame_count(2) as u32, c.anim.frame_count(3) as u32);
                    if fc2 != 0 && fc3 != 0 { h.set_anim(c.anim, c.rng, blend(8), 3, (((v.frame_b as u32 * fc3) / fc2 + 7) % fc3) as i32); }
                }
            } else {
                let (fc2, fc3) = (c.anim.frame_count(2) as u32, c.anim.frame_count(3) as u32);
                if fc2 != 0 && fc3 != 0 { h.set_anim(c.anim, c.rng, blend(8), 2, (((v.frame_b as u32 * fc2) / fc3 + 1) % fc2) as i32); }
            }
        }
        match c.anim.view().seq_b {
            2 => h.anim_speed = h.eff_len * p(90.0),
            3 => h.anim_speed = h.eff_len * p(20.0),
            _ => {}
        }
    } else {
        h.anim_speed = h.eff_len * p(196.0);
    }
    let next = if pressed(c, button::CROSS, 8) {
        JUMP
    } else if pressed(c, button::SQUARE, 9) {
        KICK
    } else if !((h.air_ticks as i32) < ticks(5) || (h.height <= p(0.8) && h.slope <= Pf::b(0x3f5f_66f3))) {
        FALL
    } else {
        if h.air_ticks != 0 { return; }
        if p(0.17) <= h.stick_mag { return; }
        IDLE
    };
    h.set_state(c, next, true);
}

fn tr_fall(h: &mut Hero, c: &mut Ctx) {
    if h.idle.level == 6 && h.pos[2] < p(115.0) {
        super::super::damage::death_fade(h);
        return;
    }
    let next = if h.air_ticks == 0 {
        let floor = DT * p(-9.0);
        if h.disp[2] < floor { h.disp[2] = floor; }
        if h.vel[2] < floor { h.vel[2] = floor; }
        if h.health < 1 {
            DEATH
        } else if p(0.5) < h.stick_mag && h.lockout == 0 {
            WALK
        } else {
            IDLE
        }
    } else {
        if h.health < 1 { return; }
        if h.f838 != 0 {
            LEDGE_GRAB
        } else {
            if !pressed(c, button::CROSS, 5) { return; }
            if h.height <= p(0.6) { return; }
            GLIDE
        }
    };
    h.set_state(c, next, true);
}

fn tr_jump(h: &mut Hero, c: &mut Ctx) {
    let j = h.jump;
    if h.timer <= j.takeoff { return; }
    let pad = c.env.pad;
    if h.timer == j.takeoff + ticks(1) {
        // 0x2492c8(g, pos, vel, &n): the ticks to the apex; the playback reaches the apex frame by then.
        let n = (h.vel[2] / j.g).to_i32();
        let v = c.anim.view();
        h.anim_speed_for_ticks(j.f_apex, Pf::from_i32(n), Pf::b(0x3f00_0000), Pf::b(0xbf80_0000), &v);
        return;
    }
    if j.curve_on != 0 && j.curve_from <= h.timer && h.timer < j.curve_to {
        let k = j.curve_len - j.curve_from;
        if 0 < k {
            let v = c.anim.view();
            h.anim_speed_for_ticks(j.f_apex, Pf::from_i32(k), Pf::b(0x3f00_0000), Pf::b(0xbf80_0000), &v);
        }
        return;
    }
    let mut landed = h.jump.landed + 1;
    if h.jump.landed == 0 {
        landed = 0;
        if h.air_ticks == 0 && h.jump.descending != 0 {
            h.jump.landed = ticks(1);
            let fh = h.jump.f_hold.to_i32();
            let v = c.anim.view();
            if p(v.frame) < h.jump.f_hold { h.set_anim(c.anim, c.rng, blend(6), v.seq_b, fh + 3); }
            landed = h.jump.landed;
        }
    }
    h.jump.landed = landed;
    if h.jump.landed == 0 {
        if h.jump.descending != 0 {
            let v = c.anim.view();
            let fh = h.jump.f_hold.to_i32();
            if (v.frame_b as i32) < fh {
                let e = h.land_eta(c, p(60.0), h.jump.g, Pf::b(0xbf80_0000));
                h.jump.land_eta = e.to_i32();
                h.anim_speed_for_ticks(h.jump.f_hold, Pf::from_i32(h.jump.land_eta), Pf::b(0x3f00_0000), Pf::b(0xbf80_0000), &v);
                if p(1.8) < h.anim_speed { h.set_anim(c.anim, c.rng, Pf::from_i32(h.jump.land_eta + 2), v.seq_b, fh + 1); }
                if h.anim_speed < p(0.2) { h.anim_speed = p(0.2); }
                if p(2.7) < h.anim_speed { h.anim_speed = p(2.7); }
            }
            if p(c.anim.view().frame).to_i32() == fh { h.anim_speed = Pf::ZERO; }
        }
    } else {
        h.anim_speed = Pf::ONE;
    }
    // The bunny hop.
    if h.jump.landed != 0 && h.lockout == 0 && pressed(c, button::CROSS, 4) && (h.air_ticks as i32) < ticks(4) {
        h.stick_target(c.env, Pf::ONE);
        h.set_state(c, JUMP, true);
        return;
    }
    if ticks(1) < h.jump.landed {
        h.f15d4 = 0;
        h.stick_target(c.env, Pf::ONE);
        // The run branch compares the target speed (u/tick) with the speed table's 0.82 (0x17bdc4): never taken.
        if h.target_speed <= SPEED_TABLE[0][3]
            || h.stick_mag <= p(0.5)
            || h.f063a != 0
            || h.lockout != 0
            || Pf::b(0x3fc9_0fdb) <= fast_diff_rots(h.target_yaw, h.rot[2])
        {
            if ticks(12) < h.jump.landed { h.set_state(c, IDLE, true); }
        } else {
            h.set_state(c, WALK, true);
        }
    }
    let near_ground = if Pf::f(2.0) < h.height {
        if h.jump.descending != 0 {
            if ticks(40) < h.timer {
                h.set_state(c, FALL, true);
                return;
            }
            true
        } else {
            false
        }
    } else {
        true
    };
    if near_ground && h.jump.descending != 0 && h.height < p(0.2) && pressed(c, button::SQUARE, 5) {
        h.set_state(c, KICK, true);
        return;
    }
    let _ = pad;
    let next = if h.f838 != 0 {
        LEDGE_GRAB
    } else {
        if h.state == CLIMB { return; }
        if !pressed(c, button::CROSS, 5) { return; }
        if h.height <= p(0.55) { return; }
        GLIDE
    };
    h.set_state(c, next, true);
}

fn side_point(h: &Hero, side: Pf) -> V4 {
    let a = fast_add_rotations(h.rot[2], side);
    [h.pos[0] + fast_cos(a) * p(0.3), h.pos[1] + fast_sin(a) * p(0.3), h.pos[2], h.pos[3]]
}

fn probe_c(h: &Hero, c: &Ctx, at: V4) -> bool { super::super::ledge::probe_c(h, c.env, to_f32x3(at), f(h.rot[2])) }

fn drop_off(h: &mut Hero, c: &mut Ctx, lockout: i32) {
    h.set_state(c, FALL, true);
    h.f500 = ticks(lockout) as i16;
}

fn tr_hang(h: &mut Hero, c: &mut Ctx) {
    h.stick_target(c.env, Pf::ONE);
    if !probe_c(h, c, h.pos) {
        drop_off(h, c, 10);
        return;
    }
    let back = Pf::b(0x4016_cbe4);
    if p(0.5) < h.stick_mag && pressed(c, button::CROSS, 9) && back < fast_diff_rots(h.target_yaw, h.rot[2]) {
        drop_off(h, c, 10);
        return;
    }
    if c.env.pad.held & button::CROUCH != 0 {
        drop_off(h, c, 10);
        return;
    }
    if pressed(c, button::CROSS, 9) {
        h.set_state(c, CLIMB, true);
        return;
    }
    if h.stick_mag <= p(0.5) { return; }
    let (l, r) = (Pf::b(0x3fc9_0fdb), Pf::b(0xbfc9_0fdb));
    if fast_diff_rots(h.target_yaw, fast_add_rotations(h.rot[2], l)) < Pf::b(0x3f49_0fdb) {
        if probe_c(h, c, side_point(h, l)) { h.set_state(c, SHIMMY_L, true); }
        return;
    }
    if p(0.5) < h.stick_mag && fast_diff_rots(h.target_yaw, fast_add_rotations(h.rot[2], r)) < Pf::b(0x3f49_0fdb) && probe_c(h, c, side_point(h, r)) {
        h.set_state(c, SHIMMY_R, true);
    }
}

fn tr_shimmy(h: &mut Hero, c: &mut Ctx) {
    let side = if h.state == SHIMMY_R { Pf::b(0xbfc9_0fdb) } else { Pf::b(0x3fc9_0fdb) };
    let there = side_point(h, side);
    h.stick_target(c.env, Pf::ONE);
    let going = p(0.5) <= h.stick_mag
        && !(Pf::b(0x3f49_0fdb) < fast_diff_rots(h.target_yaw, fast_add_rotations(h.rot[2], side)))
        && probe_c(h, c, there);
    if !going && c.anim.view().flags & 2 != 0 {
        h.set_state(c, LEDGE_HANG, true);
        return;
    }
    if p(0.5) < h.stick_mag && pressed(c, button::CROSS, 9) && Pf::b(0x4016_cbe4) < fast_diff_rots(h.target_yaw, h.rot[2]) {
        drop_off(h, c, 40);
        return;
    }
    if pressed(c, button::CROSS, 9) { h.set_state(c, CLIMB, true); }
}

fn tr_glide(h: &mut Hero, c: &mut Ctx) {
    let next = if ticks(30) < h.timer && c.env.pad.held & button::CROSS == 0 {
        FALL
    } else if h.f838 != 0 {
        LEDGE_GRAB
    } else {
        if h.air_ticks != 0 && Pf::ZERO <= h.pos[2] - h.ground_z { return; }
        if h.timer < ticks(10) { h.f524 = ticks(12) - h.timer; }
        if h.stick_mag <= p(0.3) {
            if h.air_ticks != 0 { return; }
            IDLE
        } else {
            WALK
        }
    };
    h.set_state(c, next, true);
}

fn tr_kick(h: &mut Hero, c: &mut Ctx) {
    if h.f063a != 0 {
        h.set_state(c, PIT, true);
        return;
    }
    let row = *kick_row(h);
    let v = c.anim.view();
    if !v.blending() {
        if Pf::from_i32(row[K_IDLE_AFTER]) < p(v.frame) { h.set_state(c, IDLE, false); }
        let v = c.anim.view();
        if !v.blending() && Pf::from_i32(row[K_CHAIN_FROM]) <= p(v.frame) {
            let mut n = p(v.frame).to_i32() - row[K_REF];
            if n < 1 { n = 1; }
            if ticks(15) < n { n = ticks(15); }
            if c.env.pad.pressed_within(button::SQUARE, n).is_some() { h.set_state(c, KICK, true); }
        }
    }
    let v = c.anim.view();
    let k = p(v.frame).to_i32();
    if Pf::from_i32(row[K_JUMP_AFTER]) < p(v.frame) {
        if v.blending() { return; }
        if c.env.pad.pressed_within(button::CROSS, (k - row[K_REF]) * 2).is_some() { h.set_state(c, JUMP, true); }
    }
}

// ------------------------------------------------------------------------------------------------
// The lean 0x215b68.

/// `0x215b68` (Clank's lean: HeroUpdateAlt in body 1 and the jump physics): the turn residual 0x13f4d8 as targets of the
/// body records 24..26 — 0x44: `a = 1.25·v·|v|` of half the residual (±0.7) on 24.z, 26.x = −0.3·a·min(|eff.xy| / 3·dt,
/// 1), 24.x = −0.35·a, 25.z = 1.35·a, 25.y = 0.25·|a|; 0x4f: v = 1.6·r (±1.25): 26.x = −0.52·v (±0.28), 25.z = 1.7·v
/// (±0.7), 24.z = 0.58·v, 25.y = 0.2·|v| (±0.2), 25.x = 0.16·|v|; groups 2 / 4: v = r (±1.4): 24.z = 0.29·v, 25.z = 0.87·v.
pub fn lean(h: &mut Hero) {
    let r = h.yaw_residual.to_f32();
    let (state, group) = (h.state, h.group);
    let speed = h.eff_len_xy.to_f32();
    let dt = DT.to_f32();
    let j = &mut h.bodies.joints;
    if state == WALK {
        let v = (r * 0.5).clamp(-0.7, 0.7);
        let a = v * v.abs() * 1.25;
        j[rec::R24].target[2] = a;
        let s = (speed / (dt * 3.0)).clamp(0.0, 1.0);
        // The game clamps the ratio as `min(x, 1)` then `max(…, 0)`.
        j[rec::R26].target[0] = -a * 0.3 * s;
        j[rec::R24].target[0] = -a * 0.35;
        j[rec::R25].target[2] = a * 1.35;
        j[rec::R25].target[1] = a.abs() * 0.25;
    } else if state == GLIDE {
        let v = (r * 1.6).clamp(-1.25, 1.25);
        j[rec::R26].target[0] = (-v * 0.52).clamp(-0.28, 0.28);
        j[rec::R25].target[2] = (v * 1.7).clamp(-0.7, 0.7);
        j[rec::R24].target[2] = v * 0.58;
        j[rec::R25].target[1] = (v.abs() * 0.2).clamp(-0.2, 0.2);
        j[rec::R25].target[0] = v.abs() * 0.16;
    } else if group == 2 || group == 4 {
        let v = r.clamp(-1.4, 1.4);
        j[rec::R24].target[2] = v * 0.29;
        j[rec::R25].target[2] = v * 0.87;
    }
}

// ------------------------------------------------------------------------------------------------
// HeroUpdateAlt mode 1: the mobys.

/// The glow colours by flash kind (0x14164e): (pulse low, pulse high) for `FastTweenColor(sin·0.5 + 0.5, a, b)`.
fn glow_colours(flash: bool, kind: i16) -> (u32, u32) {
    if !flash { return (0x2814_14c8, 0x1e00_004b); }
    match kind {
        1 => (0x2814_c814, 0x1e00_4b00),
        2 => (0x2814_c8c8, 0x1e00_4b4b),
        3 => (0x3232_32fa, 0x1e00_0078),
        4 => (0x28f0_1414, 0x1e4b_0000),
        _ => (0x2814_14c8, 0x1e00_004b),
    }
}

/// HeroUpdateAlt's body-1 mobys after the write-back: the antenna glow 0x4b4 (created at the hero once, placed at the
/// body's joint list 7 every tick the body is shown, its scale toward 1.7 × its class scale, ×1.4 while the command
/// flash runs), the glow phase and colour, the rotor 0x47a (created once, placed at joint list 5, its sequence by the
/// glide), and this tick's command 0x141610 (`PlayClassSound(cmd + 16, 0, hero)`, the flash).
#[allow(clippy::too_many_arguments)]
pub(crate) fn after_update(h: &mut Hero, table: &mut MobyTable, body: MobyId, hits: &mut dyn super::super::items::HitSink, sounds: &mut dyn super::super::HeroSounds, rng: &mut Rng, counter: u64) {
    let pos = h.pos.map(Pf::to_f32);
    let rot = h.rot.map(Pf::to_f32);
    let make = |table: &mut MobyTable, hits: &mut dyn super::super::items::HitSink, class: i16| -> Option<MobyId> {
        let id = hits.create_moby(table, class, counter)?;
        let m = &mut table.mobys[id];
        m.position = pos;
        m.rotation = rot;
        m.draw_dist = 0x40;
        m.visible = 1;
        m.update_dist = 0;
        m.mode = 0;
        Some(id)
    };
    if h.bodies.glow.is_none() {
        if let Some(id) = make(table, hits, super::GLOW_CLASS) {
            let m = &mut table.mobys[id];
            m.glow = 0x8014_32d7;
            m.scale *= 1.7;
            m.mode |= crate::moby_runtime::mode::GLOW;
            h.bodies.glow = Some(id);
            h.bodies.glow_colour = 0x3014_32d7;
        }
    }
    let shown = table.mobys.get(body).is_some_and(|m| m.mode & crate::moby_runtime::mode::HIDDEN == 0);
    let hero = h.clone();
    let (glow, flash) = (h.bodies.glow, h.bodies.flash);
    // 1.7, or 3.1 with the cheat 0x15edb3 (`crate::cheats`).
    let big = h.cheats.on(crate::cheats::slot::CLANK);
    if shown {
        if let Some(g) = glow {
            hits.world(table, &hero, rng, counter, &mut |w| {
                // `moby_attach_to_joint(hero, 7, M)`: the glow at the joint, its rows the joint's; 0.0257 along those rows
                // (`fun_001f9cf8`), only its z taken off the position; then `MobyBuildMatrix`.
                let mtx = w.joint_matrix(body, 7);
                let dz = mtx[2][2] * 0.0257;
                {
                    let m = w.mm(g);
                    m.position = [mtx[3][0], mtx[3][1], mtx[3][2] - dz, m.position[3]];
                    m.rows[..3].copy_from_slice(&mtx[..3]);
                }
                w.build_matrix(g);
                let mut k = if big { 3.1f32 } else { 1.7f32 };
                if flash != 0 { k *= 1.4; }
                let cls = w.classes.info(super::GLOW_CLASS).map_or(1.0, |i| i.scale);
                let s = &mut w.mm(g).scale;
                *s += ((cls * k) - *s).clamp(-0.02, 0.02);
            });
        }
    }
    // The glow phase and colour (0x140978, 0x14164c, 0x14164e).
    let dt = DT.to_f32();
    h.bodies.glow_phase = wrap(h.bodies.glow_phase + dt * 2.967_059_6);
    let running = super::super::idle::dec_timer_s16(&mut h.bodies.flash) == 0;
    if running { h.bodies.glow_phase = wrap(h.bodies.glow_phase + dt * 12.217_304); }
    let (lo, hi) = glow_colours(running, h.bodies.flash_kind);
    let c = crate::hud::tween_color(fast_sin(p(h.bodies.glow_phase)).to_f32() * 0.5 + 0.5, lo, hi);
    if let Some(m) = h.bodies.glow.and_then(|g| table.mobys.get_mut(g)) { m.glow = crate::hud::tween_color(0.1, m.glow, c); }
    h.bodies.glow_colour = crate::hud::tween_color(0.1, h.bodies.glow_colour, c);
    // The rotor 0x47a.
    if h.bodies.rotor.is_none() {
        if let Some(id) = make(table, hits, super::ROTOR_CLASS) {
            h.bodies.rotor = Some(id);
            hits.world(table, &hero, rng, counter, &mut |w| w.build_matrix(id));
        }
    }
    if let Some(r) = h.bodies.rotor {
        let gliding = h.state == GLIDE;
        let yaw = h.rot[2].to_f32();
        hits.world(table, &hero, rng, counter, &mut |w| {
            let mtx = w.joint_matrix(body, 5);
            {
                let m = w.mm(r);
                m.position = [mtx[3][0], mtx[3][1], mtx[3][2], m.position[3]];
            }
            w.build_matrix(r);
            // `sceVu0MulMatrix(M, M, R)` with R = the Euler (0, 0, −yaw) (`0x1ffa70`, `0x1ffdd8`): R first, in the joint's
            // frame, then the joint's rows (rows = R · M, as `blarg_barrier::frame`); the other order turned the rotor in
            // world space and lost it from the joint when the glide tilts him.
            let rz = euler_rows([Pf::ZERO, Pf::ZERO, p(-yaw), Pf::ZERO]).map(|r| r.map(Pf::to_f32));
            let rows: [[f32; 4]; 3] = std::array::from_fn(|i| {
                let mut o = [0.0; 4];
                for (j, oo) in o.iter_mut().enumerate().take(3) { *oo = rz[i][0] * mtx[0][j] + rz[i][1] * mtx[1][j] + rz[i][2] * mtx[2][j]; }
                o
            });
            let light = w.m(body).light;
            let m = w.mm(r);
            for (k, row) in rows.iter().enumerate() { m.rows[k] = *row; }
            m.light = light;
            let seq_b = m.anim.seq_b;
            if gliding {
                if seq_b != 1 { w.anim_blend(r, 1, 0, ticks(10)); }
            } else if seq_b != 0 {
                w.anim_blend(r, 0, 0, ticks(17));
            }
        });
    }
    // This tick's command (0x141610): its sound on the hero moby, the flash.
    let cmd = std::mem::take(&mut h.bodies.command);
    if cmd != 0 {
        if let Some(m) = table.mobys.get(body) { sounds.voice(m, cmd + 0x10, 0, rng); }
        h.bodies.flash_kind = cmd as i16;
        h.bodies.flash = ticks(0x39) as i16;
    }
}

fn wrap(a: f32) -> f32 { fast_add_rotations(p(a), Pf::ZERO).to_f32() }
