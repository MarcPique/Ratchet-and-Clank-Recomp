//! The leg walker (level04 `0x2922d0`..`0x295fe8`, the same library code in every overlay; level 01 links it at
//! `0x2b5160`..`0x2b7a30` and calls none of it): a creature that walks by planting its feet. Each tick it reads six
//! joint points (four feet and two body points), moves the body so that the feet on the ground stay where they are,
//! turns its hips toward the heading, fits its height and pitch to the ground under the feet and picks its gait from
//! the step ahead (level ground, a step up or down, the run). Read from the level04 decomp and disassembly. Native
//! `f32`. Consumer: Eudora's walker 563 (`units::eudora_walker`).
//!
//! **The walker record W** (0x250 bytes in the creature's pvars, offsets below are W-relative):
//!
//! | W+ | use |
//! |---|---|
//! | 0x00..0x60 | six points (vec4): the joint points of lists +0xf0, +0xf1, +0x1a0, +0x1a1, +0xb4, +0xb5 |
//! | 0x70..0xa0 | thirteen gait records (the game: pointers to the records the sequence headers' +0x14 point to; the port: their sequence index, the record from `Services::gaits`) |
//! | 0xb0 / 0xb3 | bytes: the stand sequence; the landing sequence (jumps) |
//! | 0xb4 / 0xb5 | bytes: the body points' joint lists |
//! | 0xb6 | byte: the feet on the ground (bits 0–3), 0x10 / 0x20 outside the record's stance ranges |
//! | 0xb7 | byte: the gait (below) |
//! | 0xb8 / 0xbc | the ground pitch and its turn velocity |
//! | 0xc0 / 0xc4 | the speed and its largest change a tick |
//! | 0xc8 | the yaw's turn velocity; 0xcc / 0xd0 / 0xd4 its spring (acceleration, damping, limit) |
//! | 0xd8 / 0xdc | the fall speed and gravity |
//! | 0xe0 | the step height; 0xe4 the ground ahead; 0xe8 the capsule's lift |
//! | 0xec | s16 a wait count; 0xee u16 flags (8 stepping down) |
//! | 0xf0 + 0xb0·i | hip pair i: +0 / +1 the feet's joint lists, +2 / +3 the manipulators' lists, +8 the hip yaw, +0xc / +0x14 the two hip angles (+0x10 / +0x18 their velocities), +0x1c / +0x20 their limits, +0x30 / +0x70 the manipulators |
//!
//! **A gait record** (0x50 bytes, words): +0x00 the sequence, +0x04 the stride, +0x08 the speed at anim speed 1
//! (`stride / scale(2·duration)`), +0x0c / +0x10 the sequence's first key time and duration, +0x14 the start key,
//! +0x18 / +0x1c the stance ranges' starts (1 key long), +0x28..+0x34 / +0x40..+0x4c the feet's contact ranges
//! (start, end; two feet each), +0x20 / +0x24 / +0x38 / +0x3c their lengths ([`setup`] computes +0x08..+0x10 and the
//! lengths).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x292370` | the hip limits: pair 0 −π/16 .. π/2, pair 1 −π/2 .. π/16 | [`hip_limits`] |
//! | `0x2923b8` | the records' derived fields; the six points; the hip yaws from joint list +0xf0's matrix (row 1), the hip angles 0; the manipulators attached (lists +0xf3 → +0x160, +0xf2 → +0x120); flags, gait, pitch, speed 0, contact 5 | [`setup`] |
//! | `0x295de8` | the same points and hips; the gait of the sequence playing (`0x1ca938`, the last record of it) and the speed `|anim speed · record speed|`; the contact; the counters 0 | [`reset`] |
//! | `0x2927d0` / `0x292688` | the contact mask of the record of key A's sequence (else the last one) | [`contact`] |
//! | `0x295778` | [`step`]: the yaw spring toward the heading (no feet down: no acceleration, double damping); the hips; the pitch spring (0.005, 0.01, 1°); the feet's drift (the mean of the planted points' moves) moves the body; the speed eased by min(+0xc4, `accel`); the height; the gait; the body capsule (collision primitive 0, lifted by +0xe8) pushed out up to 8 times (flag 1), moved less than half the speed (flag 2); the points re-read; the anim speed `speed / record speed` | [`step`] |
//! | `0x292d48` | the hips (per pair, its feet down: the hip turned toward the step, half on each joint; else both springing back) | [`hips`] |
//! | `0x292828` / `0x2922d0` | the ground under feet 0 and 2 (a line 0.75 above to 2.5 below), the mean rise of the flat ones (or the ground under the body), the pitch along the yaw from their normals; the fall; all points and the body moved | [`height`] |
//! | `0x293128` | the gait (module table below) | [`gait`] |
//! | `0x292cb8` | the hip angles and manipulators back to 0 | [`reset_hips`] |
//!
//! **Gaits** (`0x293128`; `T(r, side, g)`: blend to record r's sequence at its foot `side`'s contact start plus an
//! eighth of the contact, over a quarter of the contact at this speed, hips reset, gait g; `h` the rise of the ground
//! half a stride ahead of foot 1 or 3 plus the pitch's, `H` = 0.75 × the step height):
//!
//! | gait | what |
//! |---|---|
//! | 0 stand | slow (accel ≤ 0.0008 or under 1.35 × the walk speed): turning or moving → after 4 ticks the walk (record 0 from its start key, gait 1); else the run (record 7, gait 13) |
//! | 1–11 | nearly stopped and facing the heading → the stand sequence, 0; past 0.375 × (walk + run speed) with no foot down → the run in phase; on a new step (feet 0 / 2): the step up / down transitions |
//! | 13 run | slower than 0.35 × (walk + run) → the walk in phase (gait 1); the speed at least that |
//! | 20 | after a reset into the walk sequence: on foot 0's step the run (record 7), gait 13 |
//!
//! **Not ported (unreachable):** the jump gaits 14–19 (`W+0xee` bit 1 has no setter: no class asks for a jump, so
//! bit 4, the jump target +0x60 and the jump heading +0x6c are never used) and gait 12 (no transition into it but a
//! reset into the run sequence, which 563 never plays at a reset).

use super::{turn, V};
use crate::moby_runtime::MobyId;
use crate::moby_update::manip;
use crate::moby_update::services::{self as sv, World};
use crate::ps2v::Pf;
use std::f32::consts::FRAC_PI_2;

/// W-relative offsets (module doc).
pub mod w {
    pub const FEET: usize = 0x00;
    pub const TABLES: usize = 0x70;
    pub const STAND: usize = 0xb0;
    pub const BODY_A: usize = 0xb4;
    pub const BODY_B: usize = 0xb5;
    pub const CONTACT: usize = 0xb6;
    pub const GAIT: usize = 0xb7;
    pub const PITCH: usize = 0xb8;
    pub const PITCH_V: usize = 0xbc;
    pub const SPEED: usize = 0xc0;
    pub const ACCEL: usize = 0xc4;
    pub const YAW_V: usize = 0xc8;
    pub const YAW_ACC: usize = 0xcc;
    pub const YAW_DAMP: usize = 0xd0;
    pub const YAW_MAX: usize = 0xd4;
    pub const FALL: usize = 0xd8;
    pub const GRAVITY: usize = 0xdc;
    pub const STEP: usize = 0xe0;
    pub const AHEAD: usize = 0xe4;
    pub const LIFT: usize = 0xe8;
    pub const WAIT: usize = 0xec;
    pub const FLAGS: usize = 0xee;
    /// Hip pair i at `PAIR + 0xb0·i`.
    pub const PAIR: usize = 0xf0;
    pub const SIZE: usize = 0x250;
}

/// Hip pair offsets (pair-relative, add `w::PAIR + 0xb0·i`).
mod hp {
    pub const FOOT_A: usize = 0x00;
    pub const MANIP_A: usize = 0x02;
    pub const MANIP_B: usize = 0x03;
    pub const YAW: usize = 0x08;
    pub const ANGLE_A: usize = 0x0c;
    pub const VEL_A: usize = 0x10;
    pub const ANGLE_B: usize = 0x14;
    pub const VEL_B: usize = 0x18;
    pub const MIN: usize = 0x1c;
    pub const MAX: usize = 0x20;
    pub const REC_A: usize = 0x30;
    pub const REC_B: usize = 0x70;
}

/// The gait of each record index at a reset (`0x1ca938`, bytes of words).
const GAIT_OF: [u8; 13] = [1, 2, 3, 4, 7, 8, 9, 12, 13, 14, 19, 15, 20];
/// The record index of each gait (`0x1ca8e0`).
const RECORD_OF: [i32; 21] = [-1, 0, 1, 2, 3, 1, 2, 4, 5, 6, 4, 5, 8, 7, 9, 11, -1, 10, 12, 10, 12];
/// gp−0x7ea0 / 0x15ed60: the game speed (1); 0x15ed68 the anim speed factor (1).
const GAME_SPEED: f32 = 1.0;

/// A gait record with the set-up's derived fields.
#[derive(Clone, Copy, Debug, Default)]
pub struct Rec {
    pub seq: u8,
    pub stride: f32,
    pub speed: f32,
    pub first: f32,
    pub dur: f32,
    pub start_key: f32,
    pub stance: [f32; 2],
    /// Foot ranges: `b` (+0x28 / +0x30 starts and ends) and `a` (+0x40 / +0x48), their lengths (+0x20, +0x38).
    pub b_start: [f32; 2],
    pub b_end: [f32; 2],
    pub a_start: [f32; 2],
    pub a_end: [f32; 2],
    pub b_len: [f32; 2],
    pub a_len: [f32; 2],
}

fn wv(w: &World, id: MobyId, o: usize) -> u32 { super::pi32(w, id, o) as u32 }
fn wf(w: &World, id: MobyId, o: usize) -> f32 { super::pf(w, id, o) }
fn set_wf(w: &mut World, id: MobyId, o: usize, x: f32) { super::set_pf(w, id, o, x) }

/// The key times of a sequence's first and last frames (`0x2418b0` / `0x241910`: frame time / 16).
fn seq_times(w: &World, id: MobyId, seq: u8) -> (f32, f32) {
    let Some(s) = w.classes.anim(w.m(id).o_class).and_then(|c| c.sequence(seq)) else { return (0.0, 0.0) };
    let t = |i: usize| s.frames.get(i).map(|f| f.header.time as f32 * 0.0625).unwrap_or(0.0);
    (t(0), t(s.frames.len().saturating_sub(1)))
}

/// `MobyAnimFindFrame(key, seq)` 0x263870: the frame nearest `key` (the earlier on a tie), −1 past the end.
fn find_frame(w: &World, id: MobyId, seq: u8, key: f32) -> i32 {
    let Some(s) = w.classes.anim(w.m(id).o_class).and_then(|c| c.sequence(seq)) else { return -1 };
    let k = (key * 16.0) as i32 as i16 as i32;
    for (i, f) in s.frames.iter().enumerate() {
        let t = f.header.time as i32;
        if k < t {
            if i < 1 { return 0; }
            let prev = s.frames[i - 1].header.time as i32;
            return if t - k <= k - prev { i as i32 } else { i as i32 - 1 };
        }
    }
    -1
}

/// Record index `k` of the walker at `wr` (its derived fields as `0x2923b8` writes them).
pub fn rec(w: &World, id: MobyId, wr: usize, k: usize) -> Rec {
    let seq = wv(w, id, wr + w::TABLES + 4 * k) as u8;
    let Some(r) = w.svc.gaits.get(&(w.m(id).o_class, seq)) else { return Rec { seq, ..Rec::default() } };
    let f = |i: usize| f32::from_bits(r[i]);
    let (first, last) = seq_times(w, id, r[0] as u8);
    let dur = last - first;
    let span = |s: f32, e: f32| if e < s { e + dur } else { e } - s;
    let mut out = Rec {
        seq: r[0] as u8,
        stride: f(1),
        speed: f(2),
        first,
        dur,
        start_key: r[5] as i32 as f32,
        stance: [f(6), f(7)],
        b_start: [f(10), f(11)],
        b_end: [f(12), f(13)],
        a_start: [f(16), f(17)],
        a_end: [f(18), f(19)],
        b_len: [f(8), f(9)],
        a_len: [f(14), f(15)],
    };
    if dur != 0.0 {
        for j in 0..2 {
            out.a_len[j] = span(out.a_start[j], out.a_end[j]);
            out.b_len[j] = span(out.b_start[j], out.b_end[j]);
        }
        let k2 = sv::fl(w.svc.timing.scale(Pf::f(dur + dur)));
        out.speed = out.stride / k2;
    }
    out
}

/// `0x292688(key, record)`: the feet whose contact ranges hold `key` (a range may wrap); none: 0x10 / 0x20 outside the
/// stance ranges (one key long).
pub fn contact_of(r: &Rec, key: f32) -> u8 {
    let inside = |s: f32, e: f32| if s <= e { s <= key && key <= e } else { s <= key || key <= e };
    let mut m = 0u8;
    for j in 0..2 {
        if inside(r.a_start[j], r.a_end[j]) { m |= 1 << (2 * j); }
        if inside(r.b_start[j], r.b_end[j]) { m |= 2 << (2 * j); }
    }
    if m == 0 {
        if key < r.stance[0] || r.stance[0] + 1.0 < key { m = 0x10; }
        if key < r.stance[1] || r.stance[1] + 1.0 < key { m |= 0x20; }
    }
    m
}

/// The key time the walker reads (`seq A` 0xff: key B's frame plus its sequence's first key time).
fn key(w: &World, id: MobyId) -> f32 {
    let a = &w.m(id).anim;
    if a.seq_a == 0xff { a.frame_b as f32 + seq_times(w, id, a.seq_b).0 } else { super::ground::key_time(w, id) }
}

/// `0x2927d0(key, m, W)`: the contact of the last record of key A's sequence, else the current one.
pub fn contact(w: &World, id: MobyId, wr: usize, kt: f32) -> u8 {
    let seq = w.m(id).anim.seq_a;
    let mut hit = None;
    for k in 0..13 {
        let r = rec(w, id, wr, k);
        if r.seq == seq { hit = Some(r); }
    }
    match hit {
        Some(r) => contact_of(&r, kt),
        None => super::pu8(w, id, wr + w::CONTACT),
    }
}

/// The six points: lists +0xf0, +0xf1, +0x1a0, +0x1a1, +0xb4, +0xb5 (`MobyGetBoneMatrix(m, 6, …)`).
fn points(w: &World, id: MobyId, wr: usize) -> [V; 6] {
    let lists = [w::PAIR, w::PAIR + 1, w::PAIR + 0xb0, w::PAIR + 0xb1, w::BODY_A, w::BODY_B].map(|o| super::pu8(w, id, wr + o) as usize);
    lists.map(|l| w.joint_point(id, l))
}

fn set_points(w: &mut World, id: MobyId, wr: usize, p: &[V; 6]) {
    for (k, q) in p.iter().enumerate() { super::set_pv4(w, id, wr + w::FEET + 0x10 * k, *q); }
}

/// `0x292370`: the hip limits.
pub fn hip_limits(w: &mut World, id: MobyId, wr: usize) {
    let p0 = wr + w::PAIR;
    let p1 = p0 + 0xb0;
    set_wf(w, id, p0 + hp::MIN, f32::from_bits(0xbe49_0fdb));
    set_wf(w, id, p1 + hp::MAX, f32::from_bits(0x3e49_0fdb));
    set_wf(w, id, p0 + hp::MAX, FRAC_PI_2);
    set_wf(w, id, p1 + hp::MIN, -FRAC_PI_2);
}

/// The points and hips of [`setup`] / [`reset`]: each pair's yaw from its foot list's matrix (row 1), its angles 0, its
/// manipulators attached.
fn hips_start(w: &mut World, id: MobyId, wr: usize) {
    let p = points(w, id, wr);
    set_points(w, id, wr, &p);
    for i in 0..2 {
        let b = wr + w::PAIR + 0xb0 * i;
        let list = super::pu8(w, id, b + hp::FOOT_A) as usize;
        let m = w.joint_matrix(id, list);
        set_wf(w, id, b + hp::ANGLE_B, 0.0);
        set_wf(w, id, b + hp::ANGLE_A, 0.0);
        set_wf(w, id, b + hp::YAW, super::atan(m[1][0], m[1][1]));
        if !manip::attached(w, id, b + hp::REC_B) {
            let l = super::pu8(w, id, b + hp::MANIP_B);
            manip::attach(w, id, l, id, b + hp::REC_B);
        }
        if !manip::attached(w, id, b + hp::REC_A) {
            let l = super::pu8(w, id, b + hp::MANIP_A);
            manip::attach(w, id, l, id, b + hp::REC_A);
        }
    }
}

/// `0x2923b8(m, W)` (module table; the records' derived fields are computed where read, [`rec`]).
pub fn setup(w: &mut World, id: MobyId, wr: usize) {
    hips_start(w, id, wr);
    super::set_pi16(w, id, wr + w::FLAGS, 0);
    super::set_pu8(w, id, wr + w::GAIT, 0);
    for o in [w::PITCH, w::PITCH_V, w::SPEED, w::YAW_V, w::LIFT, w::FALL, w::AHEAD] { set_wf(w, id, wr + o, 0.0); }
    super::set_pi16(w, id, wr + w::WAIT, 0);
    super::set_pu8(w, id, wr + w::CONTACT, 5);
}

/// `0x295de8(m, W)` (module table).
pub fn reset(w: &mut World, id: MobyId, wr: usize) {
    hips_start(w, id, wr);
    let kt = key(w, id);
    super::set_pu8(w, id, wr + w::CONTACT, 5);
    set_wf(w, id, wr + w::SPEED, 0.0);
    super::set_pu8(w, id, wr + w::GAIT, 0);
    let seq_b = w.m(id).anim.seq_b;
    for (k, g) in GAIT_OF.iter().enumerate() {
        let r = rec(w, id, wr, k);
        if r.seq == seq_b {
            let s = (w.m(id).anim.speed * r.speed).abs();
            set_wf(w, id, wr + w::SPEED, s);
            super::set_pu8(w, id, wr + w::GAIT, *g);
        }
    }
    let c = contact(w, id, wr, kt);
    super::set_pu8(w, id, wr + w::CONTACT, c);
    super::set_pi16(w, id, wr + w::WAIT, 0);
    for o in [w::PITCH, w::PITCH_V, w::YAW_V, w::LIFT, w::FALL, w::AHEAD] { set_wf(w, id, wr + o, 0.0); }
}

/// `0x292cb8(W)`: both pairs' hip angles 0 and their manipulators at rest.
pub fn reset_hips(w: &mut World, id: MobyId, wr: usize) {
    for i in 0..2 {
        let b = wr + w::PAIR + 0xb0 * i;
        if manip::attached(w, id, b + hp::REC_B) { manip::set_axis(w, id, id, b + hp::REC_B, 0.0, 2); }
        if manip::attached(w, id, b + hp::REC_A) { manip::set_axis(w, id, id, b + hp::REC_A, 0.0, 2); }
        set_wf(w, id, b + hp::ANGLE_B, 0.0);
        set_wf(w, id, b + hp::ANGLE_A, 0.0);
    }
}

/// `0x292d48(heading, m, W, mask)` (module table).
fn hips(w: &mut World, id: MobyId, wr: usize, mask: u8) {
    let d2 = GAME_SPEED * 0.034_906_585;
    let gait = super::pu8(w, id, wr + w::GAIT);
    for i in 0..2 {
        let b = wr + w::PAIR + 0xb0 * i;
        let bits = 3u8 << (2 * i);
        let list = super::pu8(w, id, b + hp::FOOT_A) as usize;
        let m = w.joint_matrix(id, list);
        let a = super::atan(m[1][0], m[1][1]);
        let t = super::add_rot(super::sub_rot(wf(w, id, b + hp::YAW), a), wf(w, id, b + hp::ANGLE_B));
        let total = super::add_rot(t, wf(w, id, b + hp::ANGLE_A));
        let rest = super::sub_rot(a, total);
        if gait == 0 || mask & bits == 0 {
            let mut v = wf(w, id, b + hp::VEL_B);
            let x = turn::spring_turn(wf(w, id, b + hp::ANGLE_B), 0.0, 0.1, 0.1, d2, &mut v);
            set_wf(w, id, b + hp::VEL_B, v);
            set_wf(w, id, b + hp::ANGLE_B, x);
            let mut v = wf(w, id, b + hp::VEL_A);
            let x = turn::spring_turn(wf(w, id, b + hp::ANGLE_A), 0.0, 0.1, 0.1, d2, &mut v);
            set_wf(w, id, b + hp::VEL_A, v);
            set_wf(w, id, b + hp::ANGLE_A, x);
        } else {
            let target = if gait == 13 { 0.0 } else { total * 0.5 };
            let mut v = wf(w, id, b + hp::VEL_B);
            let x = turn::spring_turn(wf(w, id, b + hp::ANGLE_B), target, 0.1, 0.1, d2, &mut v);
            set_wf(w, id, b + hp::VEL_B, v);
            set_wf(w, id, b + hp::ANGLE_B, x);
            set_wf(w, id, b + hp::ANGLE_A, super::sub_rot(total, x));
            set_wf(w, id, b + hp::VEL_A, 0.0);
        }
        let (lo, hi) = (wf(w, id, b + hp::MIN), wf(w, id, b + hp::MAX));
        let mut hb = wf(w, id, b + hp::ANGLE_B);
        if hb < lo { hb = lo; }
        if hi < hb { hb = hi; }
        set_wf(w, id, b + hp::ANGLE_B, hb);
        let mut ha = wf(w, id, b + hp::ANGLE_A);
        if ha < lo { ha = lo; }
        if hi < ha { ha = hi; }
        set_wf(w, id, b + hp::ANGLE_A, ha);
        let y = super::add_rot(ha, super::add_rot(hb, rest));
        set_wf(w, id, b + hp::YAW, y);
        if manip::attached(w, id, b + hp::REC_B) { manip::set_axis(w, id, id, b + hp::REC_B, hb, 2); }
        if manip::attached(w, id, b + hp::REC_A) {
            manip::set_axis(w, id, id, b + hp::REC_A, ha, 2);
            if mask & bits != 0 {
                let q = crate::hero::idle::axis_quat(-w.m(id).rotation[0], 0);
                let node = manip::node(w, id, b + hp::REC_A).quat;
                manip::set_quat(w, id, id, b + hp::REC_A, rc_formats::moby_anim::quat_product(node, q));
            }
        }
    }
}

/// `0x2922d0(foot)`: the ground under a foot (a line from 0.75 above to 2.5 below, not below 0.1; flags 2), 0 on a miss,
/// with the hit's normal.
fn foot_ground(w: &World, p: V) -> (f32, [f32; 3]) {
    let a = [p[0], p[1], p[2] + 0.75, p[3]];
    let b = [p[0], p[1], (p[2] - 2.5).max(0.1), p[3]];
    match w.coll_line(sv::pv(a), sv::pv(b), 2, None) {
        Some(o) => (o.point[2], o.normal),
        None => (0.0, [0.0; 3]),
    }
}

/// `0x292828(m, W)`: the height and pitch (module table). Returns flag 2 for a slope over 45°.
fn height(w: &mut World, id: MobyId, wr: usize) -> u32 {
    let mut flags = 0;
    let mut fitted = false;
    let mut rise = 0.0f32;
    let mut n = 0.0f32;
    let mut normals = [0.0f32; 4];
    let mut total = 0.0;
    let contact = super::pu8(w, id, wr + w::CONTACT);
    for (i, bit) in [(0usize, 1u8), (2, 4)] {
        if contact & bit == 0 { continue; }
        let foot = super::pv4(w, id, wr + w::FEET + 0x10 * i);
        let (g, nrm) = foot_ground(w, foot);
        total += g;
        if 0.0 < g && nrm[0].abs() + nrm[1].abs() < nrm[2].abs() {
            n += 4.0;
            for k in 0..3 { normals[k] += nrm[k]; }
            rise += g - foot[2];
        }
    }
    if total == 0.0 {
        let g = super::ground::ground(w, super::pos(w, id), 0.5, 0).z;
        if 0.0 < g {
            n += 4.0;
            rise += g - w.m(id).position[2];
        }
    }
    if normals[..3] == [0.0; 3] {
        set_wf(w, id, wr + w::PITCH, 0.0);
    } else {
        let y = w.m(id).rotation[2];
        let nv = sv::pv([normals[0], normals[1], normals[2], normals[3]]);
        let v = sv::pv([y.cos(), y.sin(), 0.0, 0.0]);
        let v = sv::cross_ba(v, nv);
        let v = sv::cross_ba(v, nv).map(|x| x.to_f32());
        set_wf(w, id, wr + w::PITCH, super::atan(super::len2(v), v[2]));
    }
    if FRAC_PI_4 < wf(w, id, wr + w::PITCH).abs() { flags = 2; }
    if super::pu8(w, id, wr + w::GAIT) == 13 && 0.0 < wf(w, id, wr + w::AHEAD) && contact & 0xf == 0 {
        let run = rec(w, id, wr, 7);
        let z = w.m(id).position[2];
        let a = super::atan(run.stride, wf(w, id, wr + w::AHEAD) - z);
        if (-a).abs() < FRAC_PI_6 {
            fitted = true;
            set_wf(w, id, wr + w::PITCH, -a);
            n += 4.0;
            rise += wf(w, id, wr + w::AHEAD) - z;
        }
    }
    if n != 0.0 { rise /= n; }
    if super::pi16(w, id, wr + w::FLAGS) & 8 == 0 {
        if -0.025 < rise {
            if 0.025 < rise && !fitted { rise = 0.0; }
            set_wf(w, id, wr + w::FALL, 0.0);
        } else {
            rise = wf(w, id, wr + w::FALL) - wf(w, id, wr + w::GRAVITY);
            set_wf(w, id, wr + w::FALL, rise);
        }
    } else {
        if rise < -0.025 { rise = 0.0; }
        set_wf(w, id, wr + w::FALL, 0.0);
    }
    for k in 0..6 {
        let o = wr + w::FEET + 0x10 * k + 8;
        set_wf(w, id, o, wf(w, id, o) + rise);
    }
    w.mm(id).position[2] += rise;
    flags
}

use std::f32::consts::{FRAC_PI_4, FRAC_PI_6};

/// `T(r, side, gait)` (module doc): record `k`'s sequence from foot `side`'s contact start plus `phase` of its length,
/// over `blend` of its length at speed `speed` (the record's speed per tick), hips reset, `gait`.
#[allow(clippy::too_many_arguments)]
fn to(w: &mut World, id: MobyId, wr: usize, k: usize, side: usize, phase: f32, blend: f32, speed: f32, gait: u8) {
    let r = rec(w, id, wr, k);
    let len = r.a_len[side];
    let ticks = ((len * blend) / (speed / r.speed)) as i32;
    let key = ((r.a_start[side] + len * phase) - r.first) as i32;
    let dur = r.dur as i32;
    let key = if dur == 0 { 0 } else { key % dur };
    let frame = find_frame(w, id, r.seq, key as f32);
    w.anim_blend(id, r.seq, frame, ticks);
    reset_hips(w, id, wr);
    super::set_pu8(w, id, wr + w::GAIT, gait);
}

/// The ground probe of the step transitions: half the record's stride ahead along `heading` of foot 1 or 3; returns
/// (the ground, the foot's height).
fn ahead(w: &World, id: MobyId, wr: usize, r: &Rec, heading: f32, side: u32, lift: f32) -> (f32, f32) {
    let foot = super::pv4(w, id, wr + w::FEET + if side != 0 { 0x10 } else { 0x30 });
    let s = r.stride * 0.5;
    let p = [heading.cos() * s + foot[0], heading.sin() * s + foot[1], foot[2] + lift, foot[3]];
    (super::ground::ground(w, p, 0.5, 0).z, foot[2])
}

/// `0x293128(speed, heading, accel, m, W, new_feet)` (module table).
fn gait(w: &mut World, id: MobyId, wr: usize, speed: f32, heading: f32, accel: f32, new_feet: u8) {
    use super::DT;
    let g = super::pu8(w, id, wr + w::GAIT);
    let yaw = w.m(id).rotation[2];
    match g {
        0 => {
            set_wf(w, id, wr + w::AHEAD, 0.0);
            let walk = rec(w, id, wr, 0);
            let k = if accel <= 0.0008 || speed <= walk.speed * 1.35 {
                if speed <= DT * 0.21 && super::diff_rots(yaw, heading) <= 0.104_719_76 { return; }
                let n = super::pi16(w, id, wr + w::WAIT) + 1;
                super::set_pi16(w, id, wr + w::WAIT, n);
                if n < 4 { return; }
                0
            } else {
                7
            };
            let r = rec(w, id, wr, k);
            let frame = find_frame(w, id, r.seq, r.start_key);
            w.anim_blend(id, r.seq, frame, 10);
            reset_hips(w, id, wr);
            super::set_pu8(w, id, wr + w::GAIT, if k == 0 { 1 } else { 13 });
            super::set_pu8(w, id, wr + w::CONTACT, contact_of(&r, r.start_key));
            super::set_pi16(w, id, wr + w::WAIT, 0);
        }
        1..=11 => {
            if g <= 6 && new_feet & 5 != 0 {
                let f = super::pi16(w, id, wr + w::FLAGS) & !8;
                super::set_pi16(w, id, wr + w::FLAGS, f);
            }
            let k = RECORD_OF[g as usize] as usize;
            if w.m(id).anim.speed < 0.25 && super::diff_rots(yaw, heading) < 0.092_399_78 && speed < DT * 0.19 {
                let s = super::pu8(w, id, wr + w::STAND);
                w.anim_blend(id, s, 0, 10);
                reset_hips(w, id, wr);
                super::set_pu8(w, id, wr + w::GAIT, 0);
                return;
            }
            let cur = rec(w, id, wr, k);
            if w.m(id).anim.seq_a != cur.seq { return; }
            let (walk, run) = (rec(w, id, wr, 0), rec(w, id, wr, 7));
            let lim = (run.speed + walk.speed) * 0.375;
            if lim < speed && lim < wf(w, id, wr + w::SPEED) && super::pu8(w, id, wr + w::CONTACT) & 0x3a == 0 {
                let kt = super::ground::key_time(w, id);
                let d = cur.dur as i32;
                let m = |x: f32| if d == 0 { 0 } else { (x as i32) % d };
                let (a, b) = (m((cur.a_end[0] - kt) + cur.dur), m((cur.a_end[1] - kt) + cur.dur));
                let f = if a < b { run.a_end[0] } else { run.a_end[1] };
                let rd = run.dur as i32;
                let key = if rd == 0 { 0 } else { ((f - run.first) as i32 + rd) % rd };
                let ticks = (10.0 / (speed / run.speed)) as i32;
                let frame = find_frame(w, id, run.seq, key as f32);
                w.anim_blend(id, run.seq, frame, ticks);
                reset_hips(w, id, wr);
                super::set_pu8(w, id, wr + w::GAIT, 13);
                return;
            }
            if new_feet & 5 == 0 { return; }
            let side = (new_feet & 1) as u32;
            let (gz, fz) = ahead(w, id, wr, &cur, heading, side, 0.0);
            set_wf(w, id, wr + w::AHEAD, gz);
            let h = (gz - fz) + cur.stride * 0.5 * wf(w, id, wr + w::PITCH).sin();
            let hh = wf(w, id, wr + w::STEP) * 0.75;
            let down = |w: &mut World| {
                let f = super::pi16(w, id, wr + w::FLAGS) | 8;
                super::set_pi16(w, id, wr + w::FLAGS, f);
            };
            let set = |w: &mut World, g: u8| super::set_pu8(w, id, wr + w::GAIT, g);
            // Record indices: 0x70 + 4k (k = 0 walk, 1 0x74, 2 0x78, 3 0x7c, 4 0x80, 5 0x84, 6 0x88, 7 run).
            let t = |w: &mut World, k: usize, side: usize, g: u8| to(w, id, wr, k, side, 0.125, 0.25, speed, g);
            let s0 = side == 0;
            match g {
                1 => {
                    if h <= hh {
                        if -hh <= h { return; }
                        down(w);
                        if s0 { t(w, 4, 1, 7) } else { t(w, 5, 0, 8) }
                    } else if s0 { t(w, 1, 1, 2) } else { t(w, 2, 0, 3) }
                }
                2 => {
                    if -hh <= h {
                        if h < hh { set(w, 5) } else { t(w, 3, 0, 4) }
                    } else { down(w); t(w, 5, 0, 8) }
                }
                3 => {
                    if -hh <= h {
                        if h < hh { set(w, 6) } else { t(w, 3, 1, 4) }
                    } else { down(w); t(w, 4, 1, 7) }
                }
                4 => {
                    if -hh <= h {
                        if hh <= h { return; }
                        if s0 { t(w, 2, 1, 6) } else { t(w, 1, 0, 5) }
                    } else {
                        down(w);
                        if s0 { t(w, 4, 1, 7) } else { t(w, 5, 0, 8) }
                    }
                }
                5 => {
                    if h < -hh { down(w); t(w, 4, 1, 7) } else if h <= hh { t(w, 0, 1, 1) } else { set(w, 2) }
                }
                6 => {
                    if h < -hh { down(w); t(w, 5, 0, 8) } else if h <= hh { t(w, 0, 0, 1) } else { set(w, 3) }
                }
                7 => {
                    if h <= hh {
                        if -hh < h { set(w, 10) } else { t(w, 6, 0, 9) }
                    } else { t(w, 2, 0, 3) }
                }
                8 => {
                    if h <= hh {
                        if -hh < h { set(w, 11) } else { t(w, 6, 1, 9) }
                    } else { t(w, 1, 1, 2) }
                }
                9 => {
                    if h <= hh {
                        if h <= -hh { return; }
                        if s0 { t(w, 5, 1, 11) } else { t(w, 4, 0, 10) }
                    } else if s0 { t(w, 1, 1, 2) } else { t(w, 2, 0, 3) }
                }
                10 => {
                    if h <= hh {
                        if h < -hh { set(w, 7) } else { to(w, id, wr, 0, 1, 0.5, 1.0, speed, 1) }
                    } else { t(w, 1, 1, 2) }
                }
                11 => {
                    if h <= hh {
                        if h < -hh { set(w, 8) } else { to(w, id, wr, 0, 0, 0.5, 1.0, speed, 1) }
                    } else { t(w, 2, 0, 3) }
                }
                _ => {}
            }
        }
        13 => {
            let (walk, run) = (rec(w, id, wr, 0), rec(w, id, wr, 7));
            let lim = (run.speed + walk.speed) * 0.35;
            let contact = super::pu8(w, id, wr + w::CONTACT);
            if w.m(id).anim.seq_a == run.seq && contact & 0x30 == 0 {
                if lim <= speed || lim < wf(w, id, wr + w::SPEED) {
                    if new_feet & 5 != 0 && super::pi16(w, id, wr + w::FLAGS) & 1 == 0 {
                        let (gz, _) = ahead(w, id, wr, &walk, heading, (new_feet & 1) as u32, 1.0);
                        set_wf(w, id, wr + w::AHEAD, gz);
                    }
                } else {
                    let side = if contact & 3 == 0 { 1 } else { 0 };
                    let key = ((walk.a_len[side] + walk.a_len[side]) / 3.0 + walk.a_start[side] - walk.first) as i32;
                    let d = walk.dur as i32;
                    let key = if d == 0 { 0 } else { key % d };
                    let frame = find_frame(w, id, walk.seq, key as f32);
                    w.anim_blend(id, walk.seq, frame, 10);
                    reset_hips(w, id, wr);
                    super::set_pu8(w, id, wr + w::GAIT, 1);
                }
            }
            if wf(w, id, wr + w::SPEED) < lim { set_wf(w, id, wr + w::SPEED, lim); }
        }
        19 | 20 => {
            if new_feet & 1 == 0 { return; }
            let run = rec(w, id, wr, 7);
            let (len, start) = if g == 19 { (run.a_len[1], run.a_start[1]) } else { (run.a_len[0], run.a_start[0]) };
            let ticks = ((len * 0.25 + 2.0) / (speed / run.speed)) as i32;
            let key = ((start + 1.0 + run.a_len[0] * 0.125) - run.first) as i32;
            let d = run.dur as i32;
            let key = if d == 0 { 0 } else { key % d };
            let frame = find_frame(w, id, run.seq, key as f32);
            w.anim_blend(id, run.seq, frame, ticks);
            reset_hips(w, id, wr);
            set_wf(w, id, wr + w::FALL, 0.0);
            super::set_pu8(w, id, wr + w::CONTACT, 1);
            super::set_pu8(w, id, wr + w::GAIT, 13);
            let f = super::pi16(w, id, wr + w::FLAGS) & !2;
            super::set_pi16(w, id, wr + w::FLAGS, f);
            let foot = super::pv4(w, id, wr + w::FEET + 0x10);
            let s = run.stride * 0.5;
            let p = [heading.cos() * s + foot[0], heading.sin() * s + foot[1], foot[2] + 1.0, foot[3]];
            let gz = super::ground::ground(w, p, 0.5, 0).z;
            set_wf(w, id, wr + w::AHEAD, gz);
        }
        _ => {}
    }
}

/// Collision primitive 0 of the moby (`0x263a28(m, 0, …)`): its centre (the lower end for a joint capsule) in the world
/// (turned by the Euler angles, scale / 1024), radius and height. Kinds 0 / 1 sphere, 3 vertical cylinder (height
/// +4), 2 a sphere on a joint, 4 a capsule between two joints (height their distance); the joints posed as the
/// collision kernels pose them (`fun_0020fa90` with the blob's +2 count: `collision_query::pose_joints`).
fn primitive(w: &World, id: MobyId) -> Option<([f32; 3], f32, f32)> {
    let m = w.m(id);
    let blob = w.svc.coll_classes.get(&m.o_class)?;
    let p = blob.prims.first()?;
    let k = m.scale * 0.000_976_562_5;
    let q1 = p.q1().map(f32::from_bits);
    let joints = |n: usize| {
        let snap = w.svc.snapshots.get(id).and_then(|s| s.as_ref());
        crate::collision_query::pose_joints(w.classes.anim(m.o_class), &m.anim, snap, n).into_iter().map(|j| j.map(f32::from_bits)).collect::<Vec<_>>()
    };
    let count = blob.joint_counts[1] as usize;
    let (v, r, h) = match p.kind() {
        0 | 1 => ([q1[0] * k, q1[1] * k, q1[2] * k], q1[3] * k, 0.0),
        3 => ([q1[0] * k, q1[1] * k, q1[2] * k], q1[3] * k, f32::from_bits(p.q0()[1]) * k),
        2 => {
            let j = joints(count);
            let a = j.get(usize::try_from(p.word4()).ok()?)?;
            ([a[0] * k, a[1] * k, a[2] * k], f32::from_bits(p.q0()[3]) * k, 0.0)
        }
        4 => {
            let j = joints(count);
            let [ia, ib] = p.joints();
            let a = j.get(usize::try_from(ia).ok()?)?.map(|x| x * k);
            let b = j.get(usize::try_from(ib).ok()?)?.map(|x| x * k);
            let low = if a[2] < b[2] { a } else { b };
            let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            ([low[0], low[1], low[2]], f32::from_bits(p.q0()[3]) * k, len)
        }
        _ => return None,
    };
    let e = crate::moby_update::triggers::euler_matrix([m.rotation[0], m.rotation[1], m.rotation[2]]);
    let c: [f32; 3] = std::array::from_fn(|l| (e[0][l] * v[0] as f64 + e[1][l] * v[1] as f64 + e[2][l] * v[2] as f64) as f32 + m.position[l]);
    Some((c, r, h))
}

/// `0x295778(speed, heading, accel, m, W)` (module table). Returns flags 1 (pushed by the world), 2 (moved less than
/// half the speed or on a slope over 45°).
pub fn step(w: &mut World, id: MobyId, wr: usize, speed: f32, heading: f32, accel: f32) -> u32 {
    let kt = key(w, id);
    let mask = contact(w, id, wr, kt);
    let (acc, damp) = if mask & 0xf == 0 {
        let d = wf(w, id, wr + w::YAW_DAMP);
        (0.0, d + d)
    } else {
        (wf(w, id, wr + w::YAW_ACC), wf(w, id, wr + w::YAW_DAMP))
    };
    let mut v = wf(w, id, wr + w::YAW_V);
    let y = turn::spring_turn(w.m(id).rotation[2], heading, acc, damp, wf(w, id, wr + w::YAW_MAX), &mut v);
    set_wf(w, id, wr + w::YAW_V, v);
    w.mm(id).rotation[2] = y;
    hips(w, id, wr, mask);
    let mut pv_ = wf(w, id, wr + w::PITCH_V);
    let py = turn::spring_turn(w.m(id).rotation[1], wf(w, id, wr + w::PITCH), 0.005, 0.01, GAME_SPEED * 0.017_453_292, &mut pv_);
    set_wf(w, id, wr + w::PITCH_V, pv_);
    w.mm(id).rotation[1] = py;
    let now = points(w, id, wr);
    let bits = mask | (super::pu8(w, id, wr + w::CONTACT) & 0x30);
    let mut drift = [0.0f32; 4];
    let mut n = 0.0f32;
    for (k, p) in now.iter().enumerate() {
        if bits & (1 << k) == 0 { continue; }
        n += 1.0;
        let was = super::pv4(w, id, wr + w::FEET + 0x10 * k);
        drift = super::sub(super::add(drift, was), *p);
    }
    // `0x221288` (VU `vdiv`): no foot down divides 0 by 0, which the VU gives as 0 (the port: no drift).
    let drift = if n == 0.0 { [0.0; 4] } else { drift.map(|x| x / n) };
    for (k, p) in now.iter().enumerate() { super::set_pv4(w, id, wr + w::FEET + 0x10 * k, super::add(*p, drift)); }
    let pos = super::add(super::pos(w, id), drift);
    super::set_pos(w, id, pos);
    let old = super::pu8(w, id, wr + w::CONTACT);
    super::set_pu8(w, id, wr + w::CONTACT, mask);
    let new_feet = (mask ^ old) & mask;
    let a = wf(w, id, wr + w::ACCEL).min(accel);
    let mut s = wf(w, id, wr + w::SPEED);
    if s < speed {
        s += a;
        if speed < s { s = speed; }
    } else if speed < s {
        s -= a;
        if s < speed { s = speed; }
    }
    set_wf(w, id, wr + w::SPEED, s);
    let mut flags = height(w, id, wr);
    gait(w, id, wr, speed, heading, a, new_feet);
    if let Some((mut c, r, mut h)) = primitive(w, id) {
        let seq_b = w.m(id).anim.seq_b;
        let step2 = wf(w, id, wr + w::STEP) * 2.0;
        let stepping = [1, 2, 3].iter().any(|&k| rec(w, id, wr, k).seq == seq_b);
        let running = rec(w, id, wr, 7).seq == seq_b && super::pu8(w, id, wr + w::CONTACT) & 0xf != 0;
        let lift = if stepping || running {
            step2
        } else if seq_b == w.m(id).anim.seq_a {
            wf(w, id, wr + w::LIFT) * 0.9
        } else {
            wf(w, id, wr + w::LIFT)
        };
        set_wf(w, id, wr + w::LIFT, lift);
        c[2] += lift;
        h -= lift;
        let pos = super::pos(w, id);
        let off = [pos[0] - c[0], pos[1] - c[1], pos[2] - c[2]];
        for _ in 0..8 {
            let Some(o) = w.coll_capsule(c, h, r, 4, Some(id)) else { break };
            let Some(pc) = o.pushed_centre else { break };
            c = pc;
            flags |= 1;
        }
        let d = [c[0] - pos[0], c[1] - pos[1]];
        let m = w.mm(id);
        for l in 0..3 { m.position[l] = c[l] + off[l]; }
        if (d[0] * d[0] + d[1] * d[1]).sqrt() < wf(w, id, wr + w::SPEED) * 0.5 { flags |= 2; }
    } else {
        w.svc.unported("leg walker: no collision primitive 0");
    }
    let now = points(w, id, wr);
    set_points(w, id, wr, &now);
    w.mm(id).anim.speed = 1.0;
    if super::pu8(w, id, wr + w::GAIT) != 0 {
        let seq_a = w.m(id).anim.seq_a;
        for k in 0..13 {
            let r = rec(w, id, wr, k);
            if r.seq != seq_a { continue; }
            let cs = wf(w, id, wr + w::SPEED);
            let (r, v) = if cs < super::DT * 0.19 && !(super::diff_rots(w.m(id).rotation[2], heading) <= 0.092_399_78 || super::DT * 0.19 <= speed) {
                (rec(w, id, wr, 0), super::DT * 0.19)
            } else {
                (r, cs)
            };
            w.mm(id).anim.speed = GAME_SPEED * (v / r.speed).abs();
            break;
        }
    }
    flags
}
