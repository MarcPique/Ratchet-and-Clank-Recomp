//! Blarg's flame jets, class 911: level06 0x2f3ad8 (census U211; 14 created instances), with the private flame
//! puffs `0x2f3d70`, the sweeping jet's hit lines `0x2f4130` and the timed jet's `0x2f4390`. A jet with a sweep
//! period (+0x60 ≠ 0) swings its yaw ±+0x64° about its placed yaw for ever, burning along the arc it just swept; one
//! without burns straight ahead in cycles (off +0x74 s, first after +0x74 + +0x78·60 ticks; on +0x70 s, the flame
//! growing 6 a second to 4 long). While burning it keeps its loop (class sound 0, flags 4) and, near Ratchet, throws
//! two type-2 flame puffs a tick. The hit lines carry damage 1, flags 0x10001 (0x10000, sparing Ratchet, while he
//! wears head item 6). Read from the level06 decomp and disassembly (0x2f3d70's particle arguments and vector lanes).
//! Native `f32`.
//!
//! **Pvar block**: +0x50 / +0x54 4 / 6 while Ratchet is in cuboid +0x90 (else 0; read by no ported code), +0x60 the
//! sweep flag, +0x64 the sweep half-angle (°), +0x68 the sweep period (s), +0x6c the start phase (°), +0x70 / +0x74 /
//! +0x78 on / off / delay, +0x7c the phase, +0x80 its step, +0x84 the placed yaw, +0x88 s32 the timer, +0x90 the
//! cuboid (−1 none), +0x94 the loop's voice.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2f3ad8 | cuboid +0x90 ≥ 0: Ratchet (0x13f3d0) in it → +0x50 / +0x54 = 4 / 6 (gp−0x4edc / −0x4ed8), else 0 | [`update`] |
//! | state 0 | +0x60 = 0 → 3, timer ticks(trunc(+0x74 + +0x78·60)); else → 1, phase +0x6c°, step (360 / +0x68)°·dt, +0x84 yaw; voice −1 | [`update`] |
//! | state 1 | the loop not alive (0x2a12f0) → `PlayClassSound(0, 4, m)`; the puffs; the sweep lines; yaw = +0x84 + +0x64°·sin(phase), phase += step | [`update`], [`puffs`], [`sweep_lines`] |
//! | state 3 | timer done → 2, timer ticks(trunc(+0x70·60)) | [`update`] |
//! | state 2 | the loop, the puffs, the jet lines; timer done → 3, timer ticks(trunc(+0x74·60)), the loop released when it owns it, voice −1 | [`update`], [`jet_lines`] |
//! | 0x2f3d70 | d = \|Ratchet − pos\|; (d ≤ 10 or `FastBSphereCheck(24, (pos, 4))` ≠ −1) and d ≤ 24: v1 = row 0 of `euler_to_matrix(rot)` · 6·dt (gp−0x4f14), w 0.33; v2 = (v1.xy·0.125, dt), w 0.33 | [`puffs`] |
//! | | puff 1: t_A = max of two `trunc(randf(1, 20))`, t_B of `trunc(randf(20, 40))` and `trunc(randf(20, 20))` (drawn A, B, A, B), t_C ticks(30); pos + v1·`randf(0, 1)`; velocity 2 = `rand_vec(0, 0.75·dt)` + v2, w 1.01; `PartType02Spawn(p, v1, v2, 0x6070ff90, 0x10b0ffc0, t_A, t_B, t_C, def 0x30)` (0x277ea0 = 0x27dc98) | [`puffs`] (`fx::part02`) |
//! | | puff 2: the same with one draw each for t_A, t_B | [`puffs`] |
//! | 0x2f4130 | n = trunc(4 / (6·dt) · 0.25) = 10; a = phase − step·n·4; four lines from radius r to r − 1 (r = 4, 3, 2, 1): yaw at a and at a + step·n, `CollLine_Fix(p1, p2, 0, m, template)`; template dir (0, 0, 1, 5627.92), damage 1, type 3 / 1, the class, flags 0x10001 (head item 0x1404a8 = 6: 0x10000) | [`sweep_lines`] (`services::line_hit_in`) |
//! | 0x2f4390 | len = min((+0x70·60 − timer)·6·dt, 4); two lines from pos ± row 1 (+0xd0) set to 0.125, z + 0.125, along the yaw, the same template | [`jet_lines`] |
//! | | no light, save flag, other moby written | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, fx, pf, pi32, set_pf, set_pi32, DT};
use crate::moby_update::services::{HitTemplate, World};
use crate::particles::type02;
use crate::ps2v::Pf;

/// The update in the level06 class table.
pub const UPDATE_FN: u32 = 0x2f_3ad8;
pub const REFERENCE_LEVEL: u32 = 6;
pub const CLASSES: [i16; 1] = [911];
const DEG: f32 = 0.017_453_292;
/// gp−0x4f14 / −0x4f10: the flame speed (a second) and length.
pub const SPEED: f32 = 6.0;
pub const LENGTH: f32 = 4.0;
/// The head item that spares Ratchet.
pub const SPARING_HEAD_ITEM: i32 = 6;

fn loop_sound(w: &mut World, id: MobyId) {
    let s = pi32(w, id, 0x94);
    if !w.sound_alive(s, id) {
        let s = w.play_sound(0, 4, id);
        set_pi32(w, id, 0x94, s);
    }
}

fn template(w: &World, id: MobyId) -> HitTemplate {
    let flags = if w.hero.head_slot.id != SPARING_HEAD_ITEM { 0x1_0001 } else { 0x1_0000 };
    HitTemplate { dir: [Pf::ZERO, Pf::ZERO, Pf::ONE, Pf::b(0x45af_df66)], attacker: Some(id), flags, b18: 3, b19: 1, h1a: w.m(id).o_class as u16, damage: Pf::ONE, w20: 1 }
}

fn line(w: &mut World, id: MobyId, a: [f32; 3], b: [f32; 3], t: &HitTemplate) {
    let (a, b) = ([Pf::f(a[0]), Pf::f(a[1]), Pf::f(a[2]), Pf::ONE], [Pf::f(b[0]), Pf::f(b[1]), Pf::f(b[2]), Pf::ONE]);
    crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, a, b, 0, Some(id), t);
}

fn scaled_trunc(w: &mut World, lo: f32, hi: f32) -> i32 {
    let r = w.rng.randf(lo, hi);
    crate::moby_update::services::fl(w.svc.timing.scale(Pf::f(r))) as i32
}

/// `0x2f3d70` (module doc).
pub fn puffs(w: &mut World, id: MobyId) {
    let pos = w.m(id).position;
    let h = super::hero_pos(w);
    let d = c::dist3(h, pos);
    let seen = d <= 10.0 || w.view.is_some_and(|v| !v.culled(24.0, [pos[0], pos[1], pos[2], 4.0]));
    if !seen || 24.0 < d { return; }
    let r = w.m(id).rotation;
    let rows = rc_formats::moby_light::rotation_rows([r[0], r[1], r[2]]);
    let r0 = rows[0].map(f32::from_bits);
    let s = SPEED * DT;
    let v1 = [r0[0] * s, r0[1] * s, r0[2] * s, 0.33];
    let v2 = [v1[0] * 0.125, v1[1] * 0.125, DT, 0.33];
    for first in [true, false] {
        let (ta, tb) = if first {
            let a1 = scaled_trunc(w, 1.0, 20.0);
            let b1 = scaled_trunc(w, 20.0, 40.0);
            let a2 = scaled_trunc(w, 1.0, 20.0);
            let b2 = scaled_trunc(w, 20.0, 20.0);
            (a1.max(a2), b1.max(b2))
        } else {
            (scaled_trunc(w, 1.0, 20.0), scaled_trunc(w, 20.0, 40.0))
        };
        let tc = w.ticks(30);
        let k = w.rng.randf(0.0, 1.0);
        let p = [pos[0] + v1[0] * k, pos[1] + v1[1] * k, pos[2] + v1[2] * k, pos[3]];
        let rv = w.rng.rand_vec(0.0, 0.75 * DT);
        let vel2 = [rv[0] + v2[0], rv[1] + v2[1], rv[2] + v2[2], 1.01];
        fx::part02(w, &type02::Spawn { pos: p, v1, v2: vel2, c1: 0x6070_ff90, c2: 0x10b0_ffc0, t: [ta, tb, tc], def: 0x30 });
    }
}

/// `0x2f4130` (module doc).
pub fn sweep_lines(w: &mut World, id: MobyId) {
    let n = ((LENGTH / (SPEED * DT)) * 0.25) as i32;
    let step = pf(w, id, 0x80) * n as f32;
    let (amp, home) = (pf(w, id, 0x64) * DEG, pf(w, id, 0x84));
    let mut a = c::sub_rot(pf(w, id, 0x7c), step * 4.0);
    let t = template(w, id);
    let p = w.m(id).position;
    let mut r = LENGTH;
    for _ in 0..4 {
        let r2 = r - LENGTH * 0.25;
        let y1 = c::add_rot(home, amp * a.sin());
        a = c::add_rot(a, step);
        let y2 = c::add_rot(home, amp * a.sin());
        let p1 = [y1.cos() * r + p[0], y1.sin() * r + p[1], p[2]];
        let p2 = [y2.cos() * r2 + p[0], y2.sin() * r2 + p[1], p[2]];
        line(w, id, p1, p2, &t);
        r = r2;
    }
}

/// `0x2f4390` (module doc).
pub fn jet_lines(w: &mut World, id: MobyId) {
    let len = ((pf(w, id, 0x70) * 60.0 - pi32(w, id, 0x88) as f32) * SPEED * DT).min(LENGTH);
    let t = template(w, id);
    let (p, r1, yaw) = (w.m(id).position, w.m(id).rows[1], w.m(id).rotation[2]);
    for side in [0.125f32, -0.125] {
        let o = c::set_len3([r1[0], r1[1], r1[2], 0.0], side);
        let a = [p[0] + o[0], p[1] + o[1], p[2] + o[2] + 0.125];
        let b = [a[0] + yaw.cos() * len, a[1] + yaw.sin() * len, a[2]];
        line(w, id, a, b, &t);
    }
}

fn set_timer(w: &mut World, id: MobyId, secs_ticks: f32) {
    let t = w.ticks(secs_ticks as i32);
    set_pi32(w, id, 0x88, t);
}

/// Level06 0x2f3ad8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x98 { return; }
    let cub = pi32(w, id, 0x90);
    if cub >= 0 {
        let h = super::hero_pos(w);
        let inside = crate::moby_update::triggers::point_in_cuboid(&w.svc.volumes, [h[0], h[1], h[2]], cub);
        set_pf(w, id, 0x50, if inside { 4.0 } else { 0.0 });
        set_pf(w, id, 0x54, if inside { 6.0 } else { 0.0 });
    }
    match w.m(id).state {
        0 => {
            if pi32(w, id, 0x60) == 0 {
                w.mm(id).state = 3;
                let t = pf(w, id, 0x74) + pf(w, id, 0x78) * 60.0;
                set_timer(w, id, t);
            } else {
                w.mm(id).state = 1;
                let ph = pf(w, id, 0x6c) * DEG;
                set_pf(w, id, 0x7c, ph);
                let st = (360.0 / pf(w, id, 0x68)) * DEG * DT;
                set_pf(w, id, 0x80, st);
                let y = w.m(id).rotation[2];
                set_pf(w, id, 0x84, y);
            }
            set_pi32(w, id, 0x94, -1);
        }
        1 => {
            loop_sound(w, id);
            puffs(w, id);
            sweep_lines(w, id);
            let ph = pf(w, id, 0x7c);
            let s = ph.sin();
            set_pf(w, id, 0x7c, c::add_rot(ph, pf(w, id, 0x80)));
            let y = c::add_rot(pf(w, id, 0x84), pf(w, id, 0x64) * DEG * s);
            w.mm(id).rotation[2] = y;
        }
        2 => {
            loop_sound(w, id);
            puffs(w, id);
            jet_lines(w, id);
            if c::dec_timer_pvar_i32(w, id, 0x88) == 0 { return; }
            w.mm(id).state = 3;
            let t = pf(w, id, 0x74) * 60.0;
            set_timer(w, id, t);
            let slot = pi32(w, id, 0x94);
            if slot != -1 && w.sound_owner(slot) == Some(id) { w.release_sound(slot, id); }
            set_pi32(w, id, 0x94, -1);
        }
        3 => {
            if c::dec_timer_pvar_i32(w, id, 0x88) == 0 { return; }
            w.mm(id).state = 2;
            let t = pf(w, id, 0x70) * 60.0;
            set_timer(w, id, t);
        }
        _ => {}
    }
}
