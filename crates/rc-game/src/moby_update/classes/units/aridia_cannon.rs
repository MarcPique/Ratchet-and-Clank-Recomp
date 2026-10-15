//! U105 (census 2026-10-02): class 733, Aridia's background cannon (level02 0x2df3d8, the only copy: 1 placed), its
//! boom `0x2df730` and the shell 1208 it fires (`0x2ec228` spawn, update `0x2ebf20`). Read from the level02 decomp
//! and disassembly. Native `f32`; the `rand` draws in the game's order.
//!
//! The cannon turns to a random heading, fires a shell from its barrel (a muzzle blast at joint 1, the barrel's
//! direction from joint 0) and recoils (sequence 1, its glow fading in), rests 1–3 s and turns again. The shot's boom
//! travels out from it at 100 a second and is heard (two class sounds, louder nearer) when it reaches the camera. The
//! shell flies out on its own, trailing smoke, and is deleted a second after it is last drawn.
//!
//! **Cannon pvars**: +0x00 the turn velocity, +0x04 the heading it turns to, +0x08 s16 the rest timer, +0x0c s16 the
//! turn's hum voice, +0x10 the boom's radius (−1: none). **Shell pvars**: +0x00 the velocity, +0x10 s32 the life.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2df730 | the boom: radius ≥ 0: += 100·dt (gp−0x4f80); the camera (0x1673c0) within it and Ratchet not in state 0x72 → class sounds 0 and 1 (flags 0x10), volumes `clamp(trunc((1 − (d − 0) / (200 − 0))·1024), 0, 0x400)` and `clamp(trunc(((d + 100) / 100)·1024), 0, 0x400)` (`SoundSetVolume` 0x2a1968; gp−0x4f78..−0x4f6c); radius −1 | [`boom`] |
//! | 0 | the rest timer out (`FastDecTimer` s16; 0 at load: at once) → heading = `randf(0, π)`, 1, class sound 4 | [`update`] |
//! | 1 | `0x270cc0(heading, 5°·dt², 5°·dt², 10°·dt, yaw, +0x00)` (gp−0x4f90 / −0x4f8c); the hum (class sound 2, flags 4) kept alive; within 1° and stopped: the hum released, class sound 3, radius 100 (gp−0x4f7c), 2, sequence 1 (1 tick); v = (joint 1 − joint 0) at 100·dt (gp−0x4f94); `SpawnBeamExplosion(0, 0, 7, 4, 1, 3, 30, m, v, joint 1, 0, 6, 32, no sound, no shake, 1 debris)`; the shell `0x2ec228(m, joint 1, v)` | [`update`] |
//! | 2 | mode \|= 0x10; the glow (+0x90) = `FastTweenColor(min(key time / 10, 1), 0x800000ff, 0x80329600)`; wrapped → sequence 0 (1 tick), 0, rest = trunc(scale·`randf(60, 180)`) | [`update`] |
//! | 0x2ec228 | `CreateMoby(1208)`: update / draw distance 0xff, drawn; ambient (208, 96, 16) (`0x2650d0`, gp−0x4cb4..); the cannon's mode; at p; yaw atan(v), pitch atan(\|v.xy\|, v.z); 1; velocity v; life `ticks(60)` | [`spawn_shell`] |
//! | 0x2ebf20 | 0 → `DeleteMoby`; 1: position += velocity; vz `Approach`es 0 by 15·dt² (gp−0x4ca8); pitch = π/2 − atan(\|v.xy\|, vz) | [`shell_update`] |
//! | | two smoke puffs (type 44: size `randf(300000, 500000)`, growth 10000, 1, fall −0.0002, life `ticks(trunc(randf(20, 60)))`, colour gp−0x4ca4 (alpha its top byte), spin 3, the def of type 23 (0x1b26dc), byte 9 = 4 − 0x80) at the position, drifting at −0.1·velocity, the point += 0.5·velocity + `rand_vec(0.5, 1.5)` after each; one more (250000, 5000, 1, −0.0004, `ticks(6)`, alpha gp−0x4c9d) | [`shell_update`] |
//! | | drawn → life `ticks(60)`; else the life (int) counts down; 0 → `DeleteMoby` | [`shell_update`] |

#![allow(clippy::needless_range_loop)] // the game's per-lane VU writes, spelled out.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::{self as c, fx, turn, DT, DT2};
use crate::moby_update::services::{fast_dec_timer_s16, World};
use crate::particles::type44;
use crate::ps2v::Pf;
use std::f32::consts::FRAC_PI_2;

pub const REFERENCE_LEVEL: u32 = 2;
pub const UPDATE_FN: u32 = 0x2d_f3d8;
pub const CLASSES: [i16; 1] = [733];
pub const SHELL_FN: u32 = 0x2e_bf20;
pub const SHELL: i16 = 1208;
pub const SHELL_CLASSES: [i16; 1] = [SHELL];

const DEG: f32 = 0.017_453_292;
const TURN_ACC: f32 = 5.0;
const TURN_MAX: f32 = 10.0;
const SHOT_SPEED: f32 = 100.0;
const BOOM_RADIUS: f32 = 100.0;
const BOOM_SPEED: f32 = 100.0;
const GLOW_A: u32 = 0x8000_00ff;
const GLOW_B: u32 = 0x8032_9600;
const BLAST: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 7.0, flash2: 4.0, flash_dist: 1.0, scale: 3.0, light: 30.0, streaks: 0, sparks: 6, puffs: 32, debris: 1, sound: -1, shake: false };
/// gp−0x4ca8 / −0x4ca4 / −0x4c9d (level02): the shell's vertical drag (·dt²), its smoke colour, the last puff's alpha.
const SHELL_DRAG: f32 = 15.0;
const SMOKE: u32 = 0x3070_4020;
/// The top byte of gp−0x4ca0 (0x161f63).
const SMOKE_ALPHA: u8 = 0x50;

/// The boom `0x2df730` (module table).
fn boom(w: &mut World, id: MobyId) {
    let r = c::pf(w, id, 0x10);
    if r < 0.0 { return; }
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    let d = c::dist3(cam, w.m(id).position);
    let r = r + BOOM_SPEED * DT;
    c::set_pf(w, id, 0x10, r);
    if d < r && w.hero.state != 0x72 {
        let s0 = w.play_sound(0, 0x10, id);
        let s1 = w.play_sound(1, 0x10, id);
        let v0 = (((1.0 - (d - 0.0) / (200.0 - 0.0)) * 1024.0) as i32).clamp(0, 0x400);
        let v1 = ((((d - -100.0) / (0.0 - -100.0)) * 1024.0) as i32).clamp(0, 0x400);
        w.set_volume(s0, v0);
        w.set_volume(s1, v1);
        c::set_pf(w, id, 0x10, -1.0);
    }
}

/// Level02 0x2df3d8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x14 { return; }
    boom(w, id);
    match w.m(id).state {
        0 => {
            let mut t = c::pi16(w, id, 8);
            let out = fast_dec_timer_s16(&mut t) != 0;
            c::set_pi16(w, id, 8, t);
            if out {
                let h = w.rng.randf(0.0, f32::from_bits(0x4049_0fd0));
                c::set_pf(w, id, 4, h);
                w.mm(id).state = 1;
                w.play_sound(4, 0, id);
            }
        }
        1 => {
            let k = TURN_ACC * DEG * DT2;
            let target = c::pf(w, id, 4);
            let mut yaw = w.m(id).rotation[2];
            let mut v = c::pf(w, id, 0);
            turn::turn_toward(target, k, k, TURN_MAX * DEG * DT, &mut yaw, &mut v);
            w.mm(id).rotation[2] = yaw;
            c::set_pf(w, id, 0, v);
            let hum = c::pi16(w, id, 0xc) as i32;
            if !w.sound_alive(hum, id) {
                let s = w.play_sound(2, 4, id);
                c::set_pi16(w, id, 0xc, s as i16);
            }
            if c::diff_rots(w.m(id).rotation[2], target) < DEG && c::pf(w, id, 0) == 0.0 {
                let hum = c::pi16(w, id, 0xc) as i32;
                if hum != -1 { w.release_sound(hum, id); }
                c::set_pi16(w, id, 0xc, -1);
                w.play_sound(3, 0, id);
                c::set_pf(w, id, 0x10, BOOM_RADIUS);
                w.mm(id).state = 2;
                if w.m(id).anim.seq_b != 1 { w.anim_blend(id, 1, 0, 1); }
                let j1 = w.joint_point(id, 1);
                let j0 = w.joint_point(id, 0);
                let v = c::set_len3(c::sub(j1, j0), SHOT_SPEED * DT);
                fx::beam_explosion(w, &BLAST, Some(id), j1);
                spawn_shell(w, id, j1, v);
            }
        }
        2 => {
            w.mm(id).mode |= mode::GLOW;
            let k = crate::moby_update::creature::ground::key_time(w, id) / 10.0;
            let k = if 1.0 < k { 1.0 } else { k };
            w.mm(id).glow = crate::hud::tween_color(k, GLOW_A, GLOW_B);
            if w.m(id).anim.flags & 2 != 0 {
                if w.m(id).anim.seq_b != 0 { w.anim_blend(id, 0, 0, 1); }
                w.mm(id).state = 0;
                let r = w.rng.randf(60.0, 180.0);
                let t = crate::moby_update::services::fl(w.svc.timing.scale(Pf::f(r))) as i32;
                c::set_pi16(w, id, 8, t as i16);
            }
        }
        _ => {}
    }
}

/// Level02 0x2ec228(cannon, p, v): a shell (module table). None when the table is full.
pub fn spawn_shell(w: &mut World, cannon: MobyId, p: c::V, v: c::V) -> Option<MobyId> {
    let e = w.create_moby(SHELL)?;
    let md = w.m(cannon).mode;
    {
        let m = w.mm(e);
        m.update_dist = 0xff;
        m.visible = 1;
        m.draw_dist = 0xff;
        m.ambient = [208, 96, 16, m.ambient[3]];
        m.mode = md;
        m.position = p;
        m.rotation[2] = c::atan(v[0], v[1]);
        m.rotation[1] = c::atan(c::len2(v), v[2]);
        m.state = 1;
    }
    if w.m(e).pvars.len() < 0x14 { return Some(e); }
    c::set_pv4(w, e, 0, v);
    let t = w.ticks(60);
    c::set_pi32(w, e, 0x10, t);
    Some(e)
}

/// One type-44 puff with the shell's def patch (type 23's def, byte 9 = 4 − 0x80).
fn puff(w: &mut World, a: &type44::Spawn) {
    let Some(sys) = w.particles.as_deref_mut() else {
        w.rng.randi(0xff);
        return;
    };
    if let Some(i) = type44::spawn_rng(sys, w.rng, a) {
        let def = sys.def_first(23);
        let r = &mut sys.pool.recs[i];
        r[2] = def;
        r[9] = 4u8.wrapping_sub(0x80);
    }
}

/// Level02 0x2ebf20, the shell (module table).
pub fn shell_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x14 { return; }
    match w.m(id).state {
        0 => {
            w.delete_moby(id);
            return;
        }
        1 => {}
        _ => return,
    }
    let mut vel = c::pv4(w, id, 0);
    {
        let m = w.mm(id);
        for k in 0..3 { m.position[k] += vel[k]; }
    }
    turn::approach(0.0, SHELL_DRAG * DT2, &mut vel[2]);
    c::set_pv4(w, id, 0, vel);
    w.mm(id).rotation[1] = FRAC_PI_2 - c::atan(c::len2(vel), vel[2]);
    let pos = w.m(id).position;
    let half = c::scale(vel, 0.5);
    let drift = c::scale(vel, -0.1);
    let mut p = pos;
    for _ in 0..2 {
        let n = w.rng.randf(20.0, 60.0) as i32;
        let life = w.ticks(n);
        let size = w.rng.randf(300_000.0, 500_000.0);
        puff(w, &type44::Spawn { size, growth: 10_000.0, damp: 1.0, fall: f32::from_bits(0xb951_b717), w: 0.0, pos: [p[0], p[1], p[2]], vel: [drift[0], drift[1], drift[2]], life, alpha: (SMOKE >> 24) as u8, rgb: SMOKE & 0xff_ffff, spin: 3 });
        p = c::add(p, half);
        let r = w.rng.rand_vec(0.5, 1.5);
        p = c::add(p, [r[0], r[1], r[2], 0.0]);
    }
    let life = w.ticks(6);
    puff(w, &type44::Spawn { size: 250_000.0, growth: 5_000.0, damp: 1.0, fall: f32::from_bits(0xb9d1_b717), w: 0.0, pos: [pos[0], pos[1], pos[2]], vel: [drift[0], drift[1], drift[2]], life, alpha: SMOKE_ALPHA, rgb: SMOKE & 0xff_ffff, spin: 3 });
    let life = if w.m(id).visible != 0 {
        w.ticks(60)
    } else {
        let l = c::pi32(w, id, 0x10);
        if l != 0 { l.max(1) - 1 } else { 0 }
    };
    c::set_pi32(w, id, 0x10, life);
    if life == 0 { w.delete_moby(id); }
}
