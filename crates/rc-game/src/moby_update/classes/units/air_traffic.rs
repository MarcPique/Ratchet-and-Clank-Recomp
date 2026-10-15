//! Kerwan's air traffic, classes 75, 115–120, 132, 795: level03 0x29dba8 (census U128; 246 created instances), and the
//! exhaust trail 235 the 75 / 119 make (level03 0x2bae48, created by `0x2bad40`). Cars, buses and ships flying their
//! paths: level 03's own copy of the flyer path driver (`classes::flyer`, the Kochanek–Bartels / Hermite spline of
//! level01 0x2f5168, with its pvars from +0xf0 on moved up by 0x10 and without the arc-length correction), the class
//! sounds, the exhaust, the group freeze out of view and the wreck on a hit: the beam explosion, bolts, `BreakFxB`
//! pieces, the level sound def 2, then hidden for 600 ticks and back once out of view. Read from the level03 decomp
//! and disassembly of 0x29dba8 and its private helpers 0x29c8a8, 0x29c9d0, 0x29caa8, 0x29cff0, 0x29d1b8, 0x29d2e0,
//! 0x2bad40, 0x2bae48; the shared callees mapped to level01 by the clusters (`tools/ghidra/names/clusters.tsv`).
//! Native `f32`.
//!
//! **Pvar block** (P; as the flyer's up to +0xe0): +0x60 the path (s32 point, +0x64 s8 loop, +0x65 init byte, +0x6c,
//! +0x74 the spline, +0x78), +0x90 / +0xa0 tangents m0 / m1, +0xb0 / +0xc0 points p0 / p1 (w = the segment length),
//! +0xd0 / +0xe0 this / last tick's curve point, +0xf0 the exhaust point (joint list 0), +0x100 t, +0x104 / +0x108
//! tension / bias, +0x10c speed, +0x110 / +0x114 roll gain / velocity, +0x118 / +0x11c pitch gain / velocity, +0x120 the
//! yaw time constant, +0x124 / +0x128 the roll from the yaw step, +0x12c s16 start delay, +0x12e / +0x12f u8 exhaust
//! timers, +0x131 u8 (7), +0x132 s16 yaw offset (degrees), +0x134 / +0x138 / +0x13c bob amplitude / rate / phase,
//! +0x140 / +0x144 lateral / vertical offset, +0x148 s16 the horn timer, +0x14a s16 the wreck timer, +0x14c / +0x14e s16
//! the horn interval, +0x150 s32 the loop-sound slot.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x29dba8 | not drawn last frame (+0x31 = 0) → culled = `FastBSphereCheck(draw distance, (pos, 7))` = −1 | [`update`] (no view: culled, as `flyer::culled`) |
//! | | pvar block 0 → return | [`update`] |
//! | | group ≥ 0, state ∉ {0, 3}, this moby the group's first (`0x247e08`): no member in view (`0x24eec0` = L01 `0x275690`) → every member state 3, P+0x12c = `ticks(5)` | [`group_freeze`] |
//! | 0x29c9d0 | state 0 → P+0x150 = −1; else on the moby's tick of 8, wreck timer 0, the loop not alive (`0x27a108` = L01 `0x2a12f0`): `PlayClassSound(1, 4, m)` (75, 115, 116, 118, 119) / `(0, 4, m)` (117, 120) → P+0x150; 132, 795 none | [`loop_sound`] |
//! | 0x29d1b8 | 75, 115, 116, 118, 119 (and 51, 64, not placed): P+0x148 = −1 → `ticks(rand_range(P+0x14c, P+0x14c + P+0x14e))`; else `FastDecTimer` fires → camera (0x166ec0) 15 < d < 90: `PlayClassSound(0, 0, m)`; next `ticks(rand_range(ticks(2200), ticks(2200) + ticks(1000)))` | [`horn`] |
//! | state 0 | mode \|= 0x1000; spline −1 → printf; else the spline's closing point dropped (< 0.5 from the first); update distance 0xff, P+0x2b = 1, P+0x2c = 0x25; offsets > 500 → `randf(−1, 1)·v/1000`; spline −1 → state 2 | [`init`] |
//! | | P+0x131 = (slot / 2) % 7 (overwritten below) | n/a (dead store) |
//! | | `0x264458` (= L01 `0x28b410`): the chords; P+0x60 < 1 → the nearest point `0x264558` (a looping path's last → 0); +0x6c, +0x78, +0x65 = 2, +0x68 = +0x7c = 0 | [`init`] (`flyer::chord_lengths`, `flyer::nearest`) |
//! | | +0xe0 = the point, t = 1, m1 = the tangent (`0x264778` = L01 `0x28b8c8`), p0 = p1 = the point, rotation (0, −atan(\|m1.xy\|, m1.z), atan(m1)), +0x114 = +0x11c = 0, position = the point, lateral `0x29c8a8` | [`init`] (`flyer::lateral_at`) |
//! | | +0x131 = 7; the bob phase (> 2π → ·π/180; −1 with an amplitude → `rand_angle`); +0xd0 = p1; the arc lengths `0x264a40` (= L01 `0x28bb90`) | [`init`] (`flyer::arc_lengths`) |
//! | | delay P+0x12c < 1 → P+0x14a = 0, state 1; else state 3 | [`init`] |
//! | state 1 | t ≥ 1: t −= 1, the next segment (`0x264728`, `0x264778`, `0x2646d8` = L01 `0x28b878`, `0x28b8c8`, `0x28b828`) | [`fly`] |
//! | | v = P+0x10c + P+0x134·sin(phase); dt = v / p0.w; phase += P+0x138·π/180; +0xe0 = +0xd0; +0xd0 = `0x264930(t + dt)` (= L01 `0x28ba80`, no arc-length correction) | [`fly`] (`flyer::hermite`) |
//! | | seen: `0x29cff0` yaw eased to the heading + P+0x132° by 1/P+0x120, pitch and roll through `SpringTurn` 0x246c80 (= L01 `0x26cef0`) and `0x24cd58` (= L01 `0x2731d0`); culled: rotation (0, 0, heading) | [`rotate`] |
//! | | t += dt; position = +0xe0; seen → lateral; d = position − the old | [`fly`] |
//! | 0x29caa8 | hidden (mode & 1) → none. 75, 119: timers +0x12f / +0x12e (`0x1f8930`); culled with +0x12f out → none; +0x12e out or +0x12f running → +0xf0 = joint 0 (`0x23e338` = L01 `0x2645a8`); +0x12e out: \|d\| < 40·dt and camera < 90 → the trail moby 235 (`0x2bad40`: 2.5·dt, 0.15, colours 0xa040 / 0xa080, 75: 0x30a0 / 0x90a0, `ticks(50)`) and +0x12f = `ticks(50)`; +0x12e = `ticks(13)` | [`exhaust`], [`trail_spawn`] |
//! | | 115, 117, 120, 132 (not culled, +0x12e out): \|d\| < 40·dt and camera < 75 → 10 type-22 puffs at joint 0, `polar(0.6·dt, rand_angle, rand_angle)` + 0.7·d, size 105000, 0x80808080 / 0x808080, `ticks(35)`; +0x12e = `rand_range(ticks(7), ticks(20))` | [`exhaust`] (`projectile::part22`) |
//! | | 118: the same with 15 puffs, 1.2·dt, 0.5·d, size 164928, `ticks(45)`, camera < 90 | [`exhaust`] |
//! | | 116 (not culled): 2 puffs a tick at joints 0 and 1, 0.15·dt, 0.35·d, size 84000, 0x8040c0f0 / 0x808080, `ticks(20)` | [`exhaust`] |
//! | 0x29d2e0 | `MobyGetHitMessage(m, 0xa30000, 0)`; none, or the record's type 0x102 (a beam explosion's) → the tail | [`wreck`] |
//! | | the blast point: pos (75: + (cos, sin)(yaw)·−2.747); vel = 0.5·d (the beam's velocity argument, which `fx::beam_explosion` does not take: none of its effects reads it in the port) | [`wreck`] |
//! | | `BoltBurst(m, 10, 20, 2, −1)` for hit flags 0x800000, else `(m, 3, 5, 2, −1)` (0x24f1b8 = L01 0x275988) | [`wreck`] (`crate_::bolt_burst`) |
//! | | P+0x14a = `ticks(600)`; `SpawnBeamExplosion(0, 0, 4, 2, 100000, 3, 15, m, vel, point, 20, 3, 4, −1, 1, 1, −1, 0)` (0x24ce98 = L01 0x273310): the flashes, streaks, sparks, puffs, fireball, camera shake, light | [`wreck`] (`fx::beam_explosion`, [`WRECK_BEAM`]) |
//! | | the pieces: 115 0x6a9–0x6ab, 116 0x6ac–0x6ae, 117 0x6af–0x6b1, 118 0x6b2–0x6b4, 119 0x6b5–0x6b7, 120 0x6b8–0x6ba, 132 0x6bc, 0x6be, 0x6bf: `BreakFxB(12·dt², m, class, pos, rot, ticks(90), 0, row 0·0.075 + (0, 0, 0.08), 0x15f580 (zero), 0x15f580)` (0x2520b0 = L01 0x278ad8); 75, 795 none | [`wreck`] (`fx::break_piece_with`) |
//! | | 795: skill point 0x13d40c, `PlayLevelSoundAtMoby(1, 0, 0)`, `ShowBanner(0x53d6, −1)` | ported (`story::award_skill_point`) |
//! | | `0x27a618(0, 0, m)`: level def 2 at the moby | [`wreck`] (`World::play_level_def`) |
//! | tail | 0x160770 = 0x29dba8 (debug) | n/a |
//! | | +0xa4 = 0xff; P+0x14a ≠ 0: `FastDecTimer`; still running or seen → collision off, mode \|= 0x41, the loop released, P+0x150 = −1, untargetable; out and culled → mode &= ~0x41 \| 0x1000, collision back (class +0x10) | [`wreck`] |
//! | state 2 | spline ≠ −1 → state 0; the loop released, P+0x150 = −1 | [`update`] |
//! | state 3 | `FastDecTimer(P+0x12c)` out → state 1 | [`update`] |
//! | 0x2bae48 (235) | `FastDecTimer(P+0xe)` out → `DeleteMoby`; else rotation y, z = the owner's; position = the owner's P+0xf0 moved −(P+0x10 += P+0x14) along (yaw, pitch) (`0x2511d0` = L01 0x277bf0); spin += 5.236·dt; ambient = `FastTweenColor(life / life0, P+8, P+4)`; alpha = life / life0 · 128; scale ·= 1 + 0.02 | [`trail_update`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::bolt_burst;
use crate::moby_update::classes::flyer::{self as fl, Path, DEG, DT, DT2, K};
use crate::moby_update::creature::fx::{beam_explosion, break_piece_with, polar, Beam};
use crate::moby_update::creature::{add, dist3, scale};
use crate::moby_update::scheduler::group_ids;
use crate::moby_update::services::{fast_dec_timer_s16, fast_dec_timer_u8, pvar as p, World};
use crate::particles::type22;

pub const UPDATE_FN: u32 = 0x29_dba8;
pub const REFERENCE_LEVEL: u32 = 3;
pub const CLASSES: [i16; 9] = [75, 115, 116, 117, 118, 119, 120, 132, 795];
/// The classes whose joint points the exhaust reads (`0x23e338`).
pub const JOINTS: [i16; 8] = [75, 115, 116, 117, 118, 119, 120, 132];
/// The exhaust trail 235 (`0x2bad40` / its update 0x2bae48).
pub const TRAIL_FN: u32 = 0x2b_ae48;
pub const TRAIL_CLASS: i16 = 235;
pub const TRAIL_CLASSES: [i16; 1] = [TRAIL_CLASS];

/// The pvar block's size the update needs.
const PV_LEN: usize = 0x154;

pub mod pv {
    pub const IDX: usize = 0x60;
    pub const LOOP: usize = 0x64;
    pub const INIT: usize = 0x65;
    pub const SPLINE: usize = 0x74;
    pub const M0: usize = 0x90;
    pub const M1: usize = 0xa0;
    pub const P0: usize = 0xb0;
    pub const P1: usize = 0xc0;
    pub const CUR: usize = 0xd0;
    pub const PREV: usize = 0xe0;
    pub const EXHAUST: usize = 0xf0;
    pub const T: usize = 0x100;
    pub const TENSION: usize = 0x104;
    pub const BIAS: usize = 0x108;
    pub const SPEED: usize = 0x10c;
    pub const ROLL_GAIN: usize = 0x110;
    pub const ROLL_VEL: usize = 0x114;
    pub const PITCH_GAIN: usize = 0x118;
    pub const PITCH_VEL: usize = 0x11c;
    pub const YAW_TC: usize = 0x120;
    pub const ROLL_A: usize = 0x124;
    pub const ROLL_B: usize = 0x128;
    pub const DELAY: usize = 0x12c;
    pub const PUFF_T: usize = 0x12e;
    pub const TRAIL_T: usize = 0x12f;
    pub const B131: usize = 0x131;
    pub const YAW_OFF: usize = 0x132;
    pub const BOB_AMP: usize = 0x134;
    pub const BOB_RATE: usize = 0x138;
    pub const BOB_PHASE: usize = 0x13c;
    pub const LATERAL: usize = 0x140;
    pub const HORN_T: usize = 0x148;
    pub const WRECK_T: usize = 0x14a;
    pub const HORN_MIN: usize = 0x14c;
    pub const HORN_SPAN: usize = 0x14e;
    pub const LOOP_SLOT: usize = 0x150;
}

/// `SpawnBeamExplosion(0, 0, 4, 2, 100000, 3, 15, m, vel, point, 20, 3, 4, −1, 1, 1, −1, 0)` (0x29d474): the flyers'
/// kill blast ([`fl::KILL_BEAM`]) without the class sound.
pub const WRECK_BEAM: Beam = Beam { sound: -1, ..fl::KILL_BEAM };

/// The wreck pieces `BreakFxB` makes, by class (0x29d4e0's switch on class − 0x73).
pub fn pieces(o_class: i16) -> &'static [i16] {
    match o_class {
        115 => &[0x6a9, 0x6aa, 0x6ab],
        116 => &[0x6ac, 0x6ad, 0x6ae],
        117 => &[0x6af, 0x6b0, 0x6b1],
        118 => &[0x6b2, 0x6b3, 0x6b4],
        119 => &[0x6b5, 0x6b6, 0x6b7],
        120 => &[0x6b8, 0x6b9, 0x6ba],
        132 => &[0x6bc, 0x6be, 0x6bf],
        _ => &[],
    }
}

/// Level03's camera record 0x166ec0 (the game camera as `f32`).
fn camera(w: &World) -> [f32; 4] {
    let c = w.camera_point();
    [c[0], c[1], c[2], 0.0]
}

fn view_culled(w: &World, far: f32, s: [f32; 4]) -> bool { w.view.map(|v| v.culled(far, s)).unwrap_or(true) }

/// Level03 0x29dba8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    let culled = if w.m(id).visible == 0 {
        let m = w.m(id);
        let q = m.position;
        view_culled(w, m.draw_dist as f32, [q[0], q[1], q[2], 7.0])
    } else {
        false
    };
    if w.m(id).pvars.len() < PV_LEN { return; }
    group_freeze(w, id);
    loop_sound(w, id);
    horn(w, id);
    match w.m(id).state {
        0 => init(w, id),
        1 => fly(w, id, culled),
        2 => {
            if p::i32(&w.m(id).pvars, pv::SPLINE) != -1 { w.mm(id).state = 0; }
            release_loop(w, id);
        }
        3 => {
            let mut t = p::i16(&w.m(id).pvars, pv::DELAY);
            let r = fast_dec_timer_s16(&mut t);
            p::set_i16(&mut w.mm(id).pvars, pv::DELAY, t);
            if r != 0 { w.mm(id).state = 1; }
        }
        _ => {}
    }
}

/// The loop's voice released when it still plays this moby's sound; P+0x150 = −1.
fn release_loop(w: &mut World, id: MobyId) {
    let slot = p::i32(&w.m(id).pvars, pv::LOOP_SLOT);
    if slot != -1 { w.release_sound(slot, id); }
    p::set_i32(&mut w.mm(id).pvars, pv::LOOP_SLOT, -1);
}

/// The group freeze (0x29dc6c..): the group's first live member, when no member is in view, parks them all in state 3
/// for `ticks(5)`.
pub fn group_freeze(w: &mut World, id: MobyId) {
    let (g, st) = (w.m(id).group, w.m(id).state);
    if g == -1 || st == 0 || st == 3 { return; }
    let members: Vec<MobyId> = group_ids(w, g).into_iter().filter(|&m| w.table.mobys.get(m).is_some_and(|q| (q.state as i8) >= 0)).collect();
    if members.first() != Some(&id) { return; }
    if members.iter().any(|&m| !fl::culled(w, m)) { return; }
    let t = w.ticks(5);
    for m in members {
        let mm = w.mm(m);
        mm.state = 3;
        if mm.pvars.len() >= PV_LEN { p::set_i16(&mut mm.pvars, pv::DELAY, t as i16); }
    }
}

/// 0x29c9d0 (module doc): the loop sound.
fn loop_sound(w: &mut World, id: MobyId) {
    if w.m(id).state == 0 {
        p::set_i32(&mut w.mm(id).pvars, pv::LOOP_SLOT, -1);
        return;
    }
    if (w.counter & 7) != (super::kalebo_traffic::slot_phase(id) & 7) { return; }
    if p::i16(&w.m(id).pvars, pv::WRECK_T) != 0 { return; }
    let slot = p::i32(&w.m(id).pvars, pv::LOOP_SLOT);
    if slot >= 0 && w.sound_alive(slot, id) { return; }
    let idx = match w.m(id).o_class {
        75 | 115 | 116 | 118 | 119 => 1,
        117 | 120 => 0,
        _ => return,
    };
    let h = w.play_sound(idx, 4, id);
    p::set_i32(&mut w.mm(id).pvars, pv::LOOP_SLOT, h);
}

/// 0x29d1b8 (module doc): the horn at random intervals.
fn horn(w: &mut World, id: MobyId) {
    if !matches!(w.m(id).o_class, 51 | 64 | 75 | 115 | 116 | 118 | 119) { return; }
    let mut t = p::i16(&w.m(id).pvars, pv::HORN_T);
    let (lo, hi) = if t == -1 {
        let lo = p::i16(&w.m(id).pvars, pv::HORN_MIN) as i32;
        (lo, lo + p::i16(&w.m(id).pvars, pv::HORN_SPAN) as i32)
    } else {
        let r = fast_dec_timer_s16(&mut t);
        p::set_i16(&mut w.mm(id).pvars, pv::HORN_T, t);
        if r == 0 { return; }
        let d = dist3(w.m(id).position, camera(w));
        if 15.0 < d && d < 90.0 { w.play_sound(0, 0, id); }
        let lo = w.ticks(0x898) as i16 as i32;
        let hi = w.ticks(0x898) + w.ticks(1000);
        (lo, hi)
    };
    let r = w.rng.rand_range(lo, hi);
    let n = w.ticks(r);
    p::set_i16(&mut w.mm(id).pvars, pv::HORN_T, n as i16);
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    w.mm(id).mode |= mode::TARGETABLE;
    let spline = p::i32(&w.m(id).pvars, pv::SPLINE);
    if let Some(s) = usize::try_from(spline).ok().filter(|&s| s < w.svc.splines.len()) {
        let pts = &w.svc.splines[s];
        if let (Some(a), Some(b)) = (pts.first(), pts.last()) {
            if fl::dist(a.map(f32::from_bits), b.map(f32::from_bits)) < 0.5 { w.svc.splines[s].pop(); }
        }
    }
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.pvars[0x2b] = 1;
        m.pvars[0x2c] = 0x25;
    }
    for o in [pv::LATERAL, pv::LATERAL + 4] {
        let v = p::ff(&w.m(id).pvars, o);
        if 500.0 < v {
            let r = w.rng.randf(-1.0, 1.0);
            p::set_ff(&mut w.mm(id).pvars, o, (r * v) / 1000.0);
        }
    }
    let Some(path) = (spline != -1).then(|| Path::of(w, id)).flatten() else {
        if spline != -1 { w.svc.unported("air traffic: spline index out of range"); }
        w.mm(id).state = 2;
        return;
    };
    {
        let m = w.mm(id);
        m.pvars[pv::INIT] = 0;
        p::set_i32(&mut m.pvars, 0x68, 0);
    }
    fl::chord_lengths(w, &path);
    let pos = w.m(id).position;
    if p::i32(&w.m(id).pvars, pv::IDX) < 1 {
        let mut k = fl::nearest(w, &path, pos);
        if path.looped && k == path.count - 1 { k = 0; }
        p::set_i32(&mut w.mm(id).pvars, pv::IDX, k);
    }
    let idx = p::i32(&w.m(id).pvars, pv::IDX);
    let q = path.pt(path.pts(w), idx.max(0) as usize);
    {
        let m = w.mm(id);
        p::set_ff(&mut m.pvars, 0x6c, fl::dist(pos, q));
        p::set_ff(&mut m.pvars, 0x78, fl::atan(q[0] - pos[0], q[1] - pos[1]));
        m.pvars[pv::INIT] = 2;
        p::set_i32(&mut m.pvars, 0x68, 0);
        p::set_i32(&mut m.pvars, 0x7c, 0);
        p::set_v4f(&mut m.pvars, pv::PREV, q);
        p::set_ff(&mut m.pvars, pv::T, 1.0);
    }
    let (tension, bias) = (p::ff(&w.m(id).pvars, pv::TENSION), p::ff(&w.m(id).pvars, pv::BIAS));
    let m1 = path.tangent(path.pts(w), idx, 0, tension, bias);
    {
        let m = w.mm(id);
        p::set_v4f(&mut m.pvars, pv::M1, m1);
        p::set_v4f(&mut m.pvars, pv::P0, q);
        p::set_v4f(&mut m.pvars, pv::P1, q);
        m.rotation[0] = 0.0;
        m.rotation[1] = -fl::atan((m1[0] * m1[0] + m1[1] * m1[1]).sqrt(), m1[2]);
        m.rotation[2] = fl::atan(m1[0], m1[1]);
        p::set_ff(&mut m.pvars, pv::ROLL_VEL, 0.0);
        p::set_ff(&mut m.pvars, pv::PITCH_VEL, 0.0);
        m.position = q;
    }
    fl::lateral_at(w, id, pv::LATERAL);
    w.mm(id).pvars[pv::B131] = 7;
    let (amp, ph) = (p::ff(&w.m(id).pvars, pv::BOB_AMP), p::ff(&w.m(id).pvars, pv::BOB_PHASE));
    if fl::TWO_PI < ph {
        p::set_ff(&mut w.mm(id).pvars, pv::BOB_PHASE, ph * DEG);
    } else if amp != 0.0 && ph == -1.0 {
        let a = w.rng.rand_angle();
        p::set_ff(&mut w.mm(id).pvars, pv::BOB_PHASE, a);
    }
    {
        let m = w.mm(id);
        let p1 = p::v4f(&m.pvars, pv::P1);
        p::set_v4f(&mut m.pvars, pv::CUR, p1);
    }
    fl::arc_lengths(w, &path, tension, bias);
    if p::i16(&w.m(id).pvars, pv::DELAY) < 1 {
        p::set_i16(&mut w.mm(id).pvars, pv::WRECK_T, 0);
        w.mm(id).state = 1;
    } else {
        w.mm(id).state = 3;
    }
}

/// State 1 (module doc).
fn fly(w: &mut World, id: MobyId, culled: bool) {
    let Some(path) = Path::of(w, id) else { return };
    let old = w.m(id).position;
    let (tension, bias) = (p::ff(&w.m(id).pvars, pv::TENSION), p::ff(&w.m(id).pvars, pv::BIAS));
    let t = p::ff(&w.m(id).pvars, pv::T);
    if 1.0 <= t {
        p::set_ff(&mut w.mm(id).pvars, pv::T, t - 1.0);
        let idx = path.next_index(p::i32(&w.m(id).pvars, pv::IDX));
        p::set_i32(&mut w.mm(id).pvars, pv::IDX, idx);
        let m1 = path.tangent(path.pts(w), idx, 1, tension, bias);
        let next = path.pt(path.pts(w), path.wrap_index(idx + 1).max(0) as usize);
        let m = w.mm(id);
        let (a0, c0) = (p::v4f(&m.pvars, pv::M1), p::v4f(&m.pvars, pv::P1));
        p::set_v4f(&mut m.pvars, pv::M0, a0);
        p::set_v4f(&mut m.pvars, pv::M1, m1);
        p::set_v4f(&mut m.pvars, pv::P0, c0);
        p::set_v4f(&mut m.pvars, pv::P1, next);
    }
    let pv_ = w.m(id).pvars.clone();
    let phase = p::ff(&pv_, pv::BOB_PHASE);
    let (p0, p1, m0, m1) = (p::v4f(&pv_, pv::P0), p::v4f(&pv_, pv::P1), p::v4f(&pv_, pv::M0), p::v4f(&pv_, pv::M1));
    let dt = (p::ff(&pv_, pv::SPEED) + p::ff(&pv_, pv::BOB_AMP) * phase.sin()) / p0[3];
    let phase = fl::add_rot(phase, p::ff(&pv_, pv::BOB_RATE) * DEG);
    let prev = p::v4f(&pv_, pv::CUR);
    let t = p::ff(&pv_, pv::T);
    let cur = fl::hermite(t + dt, p0, p1, m0, m1);
    {
        let m = w.mm(id);
        p::set_ff(&mut m.pvars, pv::BOB_PHASE, phase);
        p::set_v4f(&mut m.pvars, pv::PREV, prev);
        p::set_v4f(&mut m.pvars, pv::CUR, cur);
    }
    if culled {
        w.mm(id).rotation = [0.0, 0.0, fl::atan(cur[0] - prev[0], cur[1] - prev[1]), 0.0];
    } else {
        rotate(w, id);
    }
    {
        let m = w.mm(id);
        let t = p::ff(&m.pvars, pv::T);
        p::set_ff(&mut m.pvars, pv::T, t + dt);
        m.position = prev;
    }
    if !culled { fl::lateral_at(w, id, pv::LATERAL); }
    let pos = w.m(id).position;
    let d = [pos[0] - old[0], pos[1] - old[1], pos[2] - old[2], pos[3] - old[3]];
    exhaust(w, id, d, culled);
    wreck(w, id, culled);
}

/// 0x29cff0 (module doc): yaw, pitch and roll of a flyer in view.
fn rotate(w: &mut World, id: MobyId) {
    let pv_ = w.m(id).pvars.clone();
    let (cur, prev) = (p::v4f(&pv_, pv::CUR), p::v4f(&pv_, pv::PREV));
    let rot = w.m(id).rotation;
    let off = p::i16(&pv_, pv::YAW_OFF) as f32 * DEG;
    let heading = fl::add_rot(fl::atan(cur[0] - prev[0], cur[1] - prev[1]), off);
    let step = fl::sub_rot(heading, rot[2]) * (1.0 / p::ff(&pv_, pv::YAW_TC)) * K;
    let yaw = fl::add_rot(step, rot[2]);
    let v = fl::sub3(cur, prev);
    let pitch = fl::atan((v[0] * v[0] + v[1] * v[1]).sqrt(), v[2]);
    let g = p::ff(&pv_, pv::PITCH_GAIN);
    let mut vel = p::ff(&pv_, pv::PITCH_VEL);
    let ry = fl::spring_turn(rot[1], -pitch, g * 10.0 * DEG * DT2, g * 5.0 * DEG * DT2, g * DEG * DT, &mut vel);
    p::set_ff(&mut w.mm(id).pvars, pv::PITCH_VEL, vel);
    let x = fl::wrap_frac(step * p::ff(&pv_, pv::ROLL_A));
    let tgt = fl::wrap_frac(fl::sub_rot(rot[0], x) * p::ff(&pv_, pv::ROLL_B));
    let g = p::ff(&pv_, pv::ROLL_GAIN);
    let mut vel = p::ff(&pv_, pv::ROLL_VEL);
    let rx = fl::spring_turn(rot[0], tgt, g * 10.0 * DEG * DT2, g * 5.0 * DEG * DT2, g * DEG * DT, &mut vel);
    let m = w.mm(id);
    p::set_ff(&mut m.pvars, pv::ROLL_VEL, vel);
    m.rotation[0] = rx;
    m.rotation[1] = ry;
    m.rotation[2] = yaw;
}

fn dec_u8(w: &mut World, id: MobyId, o: usize) -> i32 {
    let mut t = w.m(id).pvars[o];
    let r = fast_dec_timer_u8(&mut t);
    w.mm(id).pvars[o] = t;
    r
}

/// One burst of type-22 puffs at joint list `list` (or 0) of the moby: `n` puffs, speed `sp`·dt, `share`·d.
#[allow(clippy::too_many_arguments)]
fn puffs(w: &mut World, id: MobyId, n: usize, joints: &[usize], sp: f32, d: [f32; 4], size: f32, c1: u32, life: i32) {
    for k in 0..n {
        let at = w.joint_point(id, joints[k % joints.len()]);
        let (a, b) = (w.rng.rand_angle(), w.rng.rand_angle());
        let vel = add(polar(DT * sp, a, b), d);
        let life = w.ticks(life);
        crate::moby_update::creature::projectile::part22(w, &type22::Spawn { size, pos: at, vel, c1, c2: 0x0080_8080, life });
    }
}

/// 0x29caa8 (module doc): the exhaust per class; `d` = this tick's move.
pub fn exhaust(w: &mut World, id: MobyId, d: [f32; 4], culled: bool) {
    if w.m(id).mode & mode::HIDDEN != 0 { return; }
    let oc = w.m(id).o_class;
    let slow = |d: [f32; 4]| fl::len3([d[0], d[1], d[2]]) < DT * 40.0;
    match oc {
        75 | 119 => {
            dec_u8(w, id, pv::TRAIL_T);
            if culled && w.m(id).pvars[pv::TRAIL_T] == 0 { return; }
            dec_u8(w, id, pv::PUFF_T);
            let (pt, tt) = (w.m(id).pvars[pv::PUFF_T], w.m(id).pvars[pv::TRAIL_T]);
            if pt == 0 || tt != 0 {
                let j = w.joint_point(id, 0);
                p::set_v4f(&mut w.mm(id).pvars, pv::EXHAUST, j);
            }
            if w.m(id).pvars[pv::PUFF_T] != 0 { return; }
            if slow(d) && dist3(w.m(id).position, camera(w)) < 90.0 {
                let (c1, c2) = if oc == 75 { (0x30a0, 0x90a0) } else { (0xa040, 0xa080) };
                let life = w.ticks(50);
                trail_spawn(w, DT * 2.5, 0.15, id, c1, c2, life);
                let t = w.ticks(50);
                w.mm(id).pvars[pv::TRAIL_T] = t as u8;
            }
            let t = w.ticks(13);
            w.mm(id).pvars[pv::PUFF_T] = t as u8;
        }
        115 | 117 | 120 | 132 | 118 => {
            if culled || dec_u8(w, id, pv::PUFF_T) == 0 { return; }
            let big = oc == 118;
            let near = if big { 90.0 } else { 75.0 };
            if slow(d) && dist3(w.m(id).position, camera(w)) < near {
                let (share, sp, n, size, life) = if big { (0.5, 1.2, 15, f32::from_bits(0x4824_1000), 0x2d) } else { (0.7, 0.6, 10, f32::from_bits(0x47cd_1400), 0x23) };
                puffs(w, id, n, &[0], sp, scale(d, share), size, 0x8080_8080, life);
            }
            let (a, b) = (w.ticks(7), w.ticks(20));
            let t = w.rng.rand_range(a, b);
            w.mm(id).pvars[pv::PUFF_T] = t as u8;
        }
        116 => {
            if culled { return; }
            puffs(w, id, 2, &[0, 1], 0.15, scale(d, 0.35), f32::from_bits(0x47a4_1000), 0x8040_c0f0, 0x14);
        }
        _ => {}
    }
}

/// `0x2bad40(rate, size, owner, c1, c2, life)`: the exhaust trail 235 at the owner's P+0xf0 (module doc).
pub fn trail_spawn(w: &mut World, rate: f32, size: f32, owner: MobyId, c1: u32, c2: u32, life: i32) -> Option<MobyId> {
    let m = w.create_moby(TRAIL_CLASS)?;
    let (at, rot) = (p::v4f(&w.m(owner).pvars, pv::EXHAUST), w.m(owner).rotation);
    let spin = w.rng.rand_angle();
    let mo = w.mm(m);
    mo.update_dist = 0xff;
    mo.draw_dist = 0x7f;
    mo.visible = 1;
    mo.scale *= size;
    if mo.pvars.len() < 0x18 { mo.pvars.resize(0x80, 0); }
    p::set_i32(&mut mo.pvars, 0x00, owner as i32);
    mo.position = at;
    mo.rotation = rot;
    mo.rotation[0] = spin;
    p::set_u32(&mut mo.pvars, 0x08, c2);
    p::set_i16(&mut mo.pvars, 0x0e, life as i16);
    p::set_ff(&mut mo.pvars, 0x14, rate);
    p::set_i16(&mut mo.pvars, 0x0c, life as i16);
    p::set_u32(&mut mo.pvars, 0x04, c1);
    mo.ambient[0] = c1 as u8;
    mo.ambient[1] = (c1 >> 8) as u8;
    mo.ambient[2] = (c1 >> 16) as u8;
    w.build_matrix(m);
    Some(m)
}

/// Level03 0x2bae48, the trail 235 (module doc).
pub fn trail_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x18 { return; }
    let mut t = p::i16(&w.m(id).pvars, 0x0e);
    let r = fast_dec_timer_s16(&mut t);
    p::set_i16(&mut w.mm(id).pvars, 0x0e, t);
    if r != 0 {
        w.delete_moby(id);
        return;
    }
    let owner = p::i32(&w.m(id).pvars, 0) as usize;
    let Some((orot, at)) = w.table.mobys.get(owner).filter(|o| o.pvars.len() >= PV_LEN).map(|o| (o.rotation, p::v4f(&o.pvars, pv::EXHAUST))) else { return };
    let m = w.mm(id);
    m.rotation[1] = orot[1];
    m.rotation[2] = orot[2];
    let dist = p::ff(&m.pvars, 0x10) + p::ff(&m.pvars, 0x14);
    p::set_ff(&mut m.pvars, 0x10, dist);
    m.position = add(at, polar(-dist, m.rotation[2], m.rotation[1]));
    m.rotation[0] = fl::add_rot(m.rotation[0], DT * 5.235_987_7);
    let f = p::i16(&m.pvars, 0x0e) as f32 / p::i16(&m.pvars, 0x0c) as f32;
    let c = crate::particles::tween_color(f.to_bits(), p::u32(&m.pvars, 0x08), p::u32(&m.pvars, 0x04));
    m.ambient[0] = c as u8;
    m.ambient[1] = (c >> 8) as u8;
    m.ambient[2] = (c >> 16) as u8;
    m.alpha = (f * 128.0) as i32 as u8;
    m.scale *= K * 0.019_999_98 + 1.0;
}

/// 0x29d2e0 (module doc): the hit, the wreck and the hide timer.
pub fn wreck(w: &mut World, id: MobyId, culled: bool) {
    let hit = w.get_hit(id, 0xa3_0000, false);
    if let Some(h) = hit.filter(|h| (h.b28, h.b29) != (2, 1)) {
        let m = w.m(id);
        let (oc, pos, rot, row0) = (m.o_class, m.position, m.rotation, m.rows[0]);
        let mut at = [0.0f32; 4];
        if oc == 75 {
            at[0] = rot[2].cos() * -2.747;
            at[1] = rot[2].sin() * -2.747;
        }
        let at = add(at, pos);
        if h.flags & 0x80_0000 != 0 { bolt_burst(w, id, 10, 20, 2, -1) } else { bolt_burst(w, id, 3, 5, 2, -1) }
        let t = w.ticks(600);
        p::set_i16(&mut w.mm(id).pvars, pv::WRECK_T, t as i16);
        beam_explosion(w, &WRECK_BEAM, Some(id), at);
        let mut v = scale(row0, K * 0.075);
        v[2] += K * 0.08;
        for &c in pieces(oc) {
            let t = w.ticks(0x5a);
            break_piece_with(w, id, c, pos, rot, t, 0, v, [0.0; 4], [0.0; 4]);
        }
        if oc == 795 { crate::moby_update::story::award_skill_point(w, crate::moby_update::story::skill_index(0x13_d40c)); }
        w.play_level_def(0, 0, id);
    }
    w.mm(id).hit_slot = 0xff;
    let mut t = p::i16(&w.m(id).pvars, pv::WRECK_T);
    if t == 0 { return; }
    let r = fast_dec_timer_s16(&mut t);
    p::set_i16(&mut w.mm(id).pvars, pv::WRECK_T, t);
    if r == 0 || !culled {
        {
            let m = w.mm(id);
            m.has_collision = false;
            m.mode |= mode::HIDDEN | mode::NO_ANIM;
        }
        release_loop(w, id);
        w.mm(id).mode &= !mode::TARGETABLE;
    } else {
        let coll = super::class_collision(w, w.m(id).o_class);
        let m = w.mm(id);
        m.mode = (m.mode & !(mode::HIDDEN | mode::NO_ANIM)) | mode::TARGETABLE;
        m.has_collision = coll;
    }
}
