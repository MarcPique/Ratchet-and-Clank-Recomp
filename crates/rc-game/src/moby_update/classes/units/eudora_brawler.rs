//! U163 (census 2026-10-02): class 340, Eudora's brawler bots (level04 0x2c2270, the only copy: 63 placed, 28 created).
//! The name is descriptive [L]. Read from the level04 decomp and disassembly. Native `f32`; the `rand` draws in the
//! game's order.
//!
//! A ground bot with a humming engine (its pitch and volume follow the sequence) that wanders around its home, or
//! patrols a path and stops on pads, inside an optional area (a path polygon, +0x138). It notices Ratchet within its
//! range (doubled while alerted), circles in to him keeping its distance from the others of its kind, and when it is
//! the nearest of them within 2 it swings: four arm spheres that hurt him over keys 15–44 of sequence 7. Walking into it
//! also starts the swing. A hit pushes it back (a slide along its back), any killing hit or a fall of 5 below its home
//! blows it up (an explosion at joint 5, death bits). Near the camera it trails exhaust puffs.
//!
//! **Pvars** (0x2b0): header (+0x00 D, +0x0c F, +0x10 K, +0x14 the suck record, +0x18 J); +0x20 D (health 1, meter 1,
//! column 0, +0x2e 1 while alive, +0x30 0.75, +0x38 the lure); +0x58 / +0x5a bytes 8 / 6; +0x60 J (`SeedJumpPattern`;
//! +0x78 the fall speed); +0xb0 F; +0xc0 K; +0x120 home; +0x130 the turn velocity; +0x138 the area path (−1 none);
//! +0x13c the push-back speed; +0x140 the range; +0x144 s16 the strafe sequence timer; +0x146 s16 the hold timer; +0x14c
//! byte the strafe quadrant; +0x14d byte the hit guard; +0x14e s16 flags (1 noticed, 2 swung); +0x150 the patrol path
//! (−1 none), +0x154 its node; +0x158 s16 the pad cuboid, +0x15a s16 the pad step; +0x160 four pad cuboids; +0x180 the
//! suck record (+0x1e8 its state, +0x1f0 its sequence table); +0x230 / +0x234 the engine's volume / pitch, +0x238 its
//! voice; +0x240 the wander record (`walker::wander`); +0x270 the big-head manipulator.
//!
//! | address | what | port |
//! |---|---|---|
//! | top | alerted = lure (+0x38) or the alert timer (+0x136); `0x2563b8(2.9, m, 7, +0x270)` the big-head cheat; drawn and within 27 (3-D) of the camera → the shadow probe (`0x26f020`), +0x7f = 0x15; the target (`0x274b78`, range ×2 alerted); its xy distance; key time; update distance 0x20; [`engine`] | [`update`] |
//! | top | state 1 / 3 and Ratchet's capsule on it (0x13f590) or within 1.5 (3-D) → seq 7 (`ticks(10)`), hold `ticks(60)`, 7 | [`update`] (the capsule word: G-HERO-033) |
//! | hits | not in 0 / 8: `MobyGetHitMessage(m, 0x210000)`, the resolver (column 4); a hit with the guard +0x14d out: K (+0x10 0.008, +0x14 0.0005, +0x18 4·dt, +0x24 9, +0x3d 0); attacker class 0xb0 / 0xb1 → damage 1; health −= damage, ≤ 0 → reaction 1; alert `ticks(240)`; reaction 1 / 2 → [`die`]; 3–6 → seq 8 (`ticks(5)`), push-back 10/`scale(15)` (half for an attacker of class 0x131 / 0xb0), 6, yaw = the heading from the attacker + π, the flash twice (colour 0x78, the guard `ticks(20)`) | [`hits`] |
//! | | +0xa4 = 0xff; the lure → alert `ticks(240)`, lure 0; the alert timer | [`update`] |
//! | 7, seq 7, key 15–44 | joint lists 1–4 (`0x161890`): `coll_sphere_mobys(0.5, joint, 0x10, m, {(Ratchet − pos) at 10·dt, m, 0x10000, damage 1, +0x20 1})`; the first listed the target → hold = min(hold, `ticks(60)`) | [`swing`] |
//! | | not in 0 and D+0x0e ≠ 1 → the voice released, `DeleteMoby` | [`update`] |
//! | 0 | D fields, J (`SeedJumpPattern`, +0x6c 0.15, +0x68 0.25, +0x60 0x200, +0x84 3·dt, +0x64 1, +0x98 4), the wander record (home, leash 2, step 2·dt, turn 2.967·dt), home, hold `ticks(60)`, the suck table (gp−0x5388), alert / strafe timers 0; → 4 with a path, else 1 | [`init`] |
//! | 1 | `walker::wander(0.35, 0.25)`, undone outside the area; wrapped and not on seq 0 → seq 0; in range (the target in the area): noticed (seq 6, hold `ticks(60)`), then within 2, hold out and the nearest ([`nearest`]) → 7; else hold out → seq 1, quadrant 0, 3 | [`update`] |
//! | 2 | on seq 5: wrapped → seq 0, 1; else seq 5 (`ticks(5)`), fall speed −7.668·dt | [`update`] (no setter on level 04) |
//! | 3 | in range: the goal and heading of [`flock`]; within 2, hold out, the nearest → 7; out of range (not alerted): the goal home, within 1 → seq 0, 1; `SpringTurn2(heading, dt²·π/3, dt²·π, dt·π)` | [`update`] |
//! | 4 | in range as 1 (→ 7 or 3); the path's node reached (1, xy) or more than 135° off → the next (wraps); the hold out and in a pad cuboid → 5, pad step 0; turn to the node | [`patrol`] |
//! | 5 | in range as 4 (7 also zeroes the turn velocity); step 0: turn to the pad's yaw, seq 1, within 1 and 10° → 1; 1: seq 7 to its end → seq 0, step 2; 2: turn to the node, within 10° → 4, hold `ticks(240)` | [`pad`] |
//! | 6 | seq 8 past key 30 or wrapped → seq 0, 1, hold 0 | [`update`] |
//! | 7 | [`flock`]; hold out and past key 44 or wrapped → seq 0 (`ticks(5)`), hold `ticks(120)`, 1, flags \|= 2; the turn | [`update`] |
//! | 8 / 9 | 8: wrapped → [`die`]; 9: `0x305260` the Suck Cannon's carried update, done → 0, suck state 0 | [`update`] (`react::carried`) |
//! | push-back | > 0: `0x26d270(0.25, 0.57, 0.5, 0.5236, m, pos, pos + (cos, sin)(yaw + π)·it, 1)`, it −= 10/`scale(15)`² | [`update`] |
//! | moves | 9 → done; 4: seq 1, 2·dt ahead (0.57); 3: [`strafe`]; 5, or 7 on seq 7 past key 15: 2·dt toward the goal when farther (1.25); 6 / 8 none; else in place; a move leaving the area is undone | [`moves`] |
//! | tail | fall speed += 9.8·dt²; z −= it; `GroundHeight(0.5)` below → on it, speed 0; 5 below home → [`die`]; within 20 (xy) of the camera → [`exhaust`]; the flash | [`update`] |
//!
//! **Not the game's, noted [L]:** a hit with no attacker reads the class of address 0 in the game (the port: not
//! 0xb0 / 0xb1); the capsule word 0x13f590 is not filled by the port's hero (G-HERO-033).

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, damage, flash, fx, react, region, target, turn, walker, DT, DT2};
use crate::moby_update::services::{self as sv, pf, pv, HitTemplate, World};
use crate::ps2v::Pf;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_3, FRAC_PI_4, FRAC_PI_6, PI};

pub const REFERENCE_LEVEL: u32 = 4;
pub const UPDATE_FN: u32 = 0x2c_2270;
pub const CLASSES: [i16; 1] = [340];

/// Pvar offsets (module doc).
pub mod pv_ {
    pub const D: usize = 0x20;
    pub const LURE: usize = 0x38;
    pub const J: usize = 0x60;
    pub const FALL: usize = 0x78;
    pub const F: usize = 0xb0;
    pub const K: usize = 0xc0;
    pub const HOME: usize = 0x120;
    pub const TURN_V: usize = 0x130;
    pub const ALERT: usize = 0x136;
    pub const AREA: usize = 0x138;
    pub const PUSH: usize = 0x13c;
    pub const RANGE: usize = 0x140;
    pub const STRAFE_T: usize = 0x144;
    pub const HOLD: usize = 0x146;
    pub const QUADRANT: usize = 0x14c;
    pub const GUARD: usize = 0x14d;
    pub const FLAGS: usize = 0x14e;
    pub const PATH: usize = 0x150;
    pub const NODE: usize = 0x154;
    pub const PAD: usize = 0x158;
    pub const PAD_STEP: usize = 0x15a;
    pub const PADS: usize = 0x160;
    pub const SUCK: usize = 0x180;
    pub const VOL: usize = 0x230;
    pub const PITCH: usize = 0x234;
    pub const VOICE: usize = 0x238;
    pub const WANDER: usize = 0x240;
    pub const BIG_HEAD: usize = 0x270;
    pub const SIZE: usize = 0x2b0;
}
use pv_ as p;

/// `SpawnBeamExplosion(0, 0, 2, 1, 100000, 1, 15, m, 0, joint 5, 10, 3, 4, −1, 1, 1, −1, 0)` (0x2c1e60).
pub const BEAM: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 100000.0, scale: 1.0, light: 15.0, streaks: 10, sparks: 3, puffs: 4, debris: 1, sound: -1, shake: true };
/// The swing's joint lists (0x161890).
const ARMS: [usize; 4] = [1, 2, 3, 4];
/// The strafe sequences by quadrant (0x1618a0).
const STRAFE: [u8; 4] = [1, 4, 2, 3];
/// gp−0x537c: the weight of a neighbour's push in [`flock`].
const SPREAD: f32 = 6.0;
/// The moves' `0x26d270` slope limit (30°).
const SLOPE: f32 = FRAC_PI_6;

fn st(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_st(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn seq_a(w: &World, id: MobyId) -> u8 { w.m(id).anim.seq_a }
fn seq_b(w: &World, id: MobyId) -> u8 { w.m(id).anim.seq_b }
fn blend(w: &mut World, id: MobyId, seq: u8, t: i32) {
    let t = w.ticks(t);
    w.anim_blend(id, seq, 0, t);
}
fn set_hold(w: &mut World, id: MobyId, t: i32) {
    let t = w.ticks(t);
    c::set_pi16(w, id, p::HOLD, t as i16);
}
fn hold(w: &World, id: MobyId) -> i16 { c::pi16(w, id, p::HOLD) }
fn gscale(w: &World, x: f32) -> f32 { sv::fl(w.svc.timing.scale(Pf::f(x))) }
fn heading_to(a: c::V, b: c::V) -> f32 { c::atan(b[0] - a[0], b[1] - a[1]) }
fn spline(w: &World, o: i32) -> Option<usize> { usize::try_from(o).ok().filter(|&s| s < w.svc.splines.len()) }
fn node_point(w: &World, path: usize, i: i32) -> c::V {
    usize::try_from(i).ok().and_then(|i| w.svc.splines[path].get(i)).map(|q| q.map(f32::from_bits)).unwrap_or([0.0; 4])
}
/// `PointInPathPolygon(p, area)`; true without an area.
fn in_area(w: &World, id: MobyId, q: c::V) -> bool {
    match spline(w, c::pi32(w, id, p::AREA)) {
        Some(a) => region::point_in_polygon(w, a, q),
        None => true,
    }
}
fn alerted(w: &World, id: MobyId) -> bool { c::pi32(w, id, p::LURE) != 0 || c::pi16(w, id, p::ALERT) != 0 }
fn range(w: &World, id: MobyId) -> f32 {
    let r = c::pf(w, id, p::RANGE);
    if alerted(w, id) { r + r } else { r }
}
fn turn_to(w: &mut World, id: MobyId, a: f32) { turn::spring_turn2_pvar(w, id, a, DT2 * FRAC_PI_3, DT2 * PI, DT * PI, p::TURN_V); }

/// The engine loop `0x2c1b80`: started (class sound 0, flags 4) when not playing on sequences below 8, its volume and
/// pitch eased (`Approach`) toward the sequence's (0: 0.6, 0; 1: 1, 3; 2: 1, −2; 3 / 4: 1, 4; 5–7: 0.8, 0), released
/// in 9 and on other sequences.
fn engine(w: &mut World, id: MobyId) {
    let voice = c::pi32(w, id, p::VOICE);
    let release = |w: &mut World| {
        if voice != -1 { w.release_sound(voice, id); }
        c::set_pi32(w, id, p::VOICE, -1);
    };
    if st(w, id) == 9 { return release(w); }
    if !w.sound_alive(voice, id) && seq_b(w, id) < 8 {
        let s = w.play_sound(0, 4, id);
        c::set_pi32(w, id, p::VOICE, s);
        c::set_pf(w, id, p::VOL, 1024.0);
        c::set_pf(w, id, p::PITCH, 0.0);
    }
    let (pitch, vol) = match seq_b(w, id) {
        0 => (0.0, 1024.0 * 0.6),
        1 => (3.0, 1024.0),
        2 => (-2.0, 1024.0),
        3 | 4 => (4.0, 1024.0),
        5..=7 => (0.0, 1024.0 * 0.8),
        _ => return release(w),
    };
    let mut v = c::pf(w, id, p::VOL);
    turn::approach(vol, 1024.0 * DT + 1024.0 * DT, &mut v);
    c::set_pf(w, id, p::VOL, v);
    let mut q = c::pf(w, id, p::PITCH);
    turn::approach(pitch, DT * 8.0, &mut q);
    c::set_pf(w, id, p::PITCH, q);
    let voice = c::pi32(w, id, p::VOICE);
    w.set_volume(voice, v as i32);
    w.set_pitch_bend(voice, q as i32);
}

/// `0x2c1d70`: the voice released, `SetDeathBits(m, 0, −1)`, the explosion [`BEAM`] at joint 5, class sound 1; then
/// `DeleteMoby` (the callers).
fn die(w: &mut World, id: MobyId) {
    let voice = c::pi32(w, id, p::VOICE);
    if voice != -1 { w.release_sound(voice, id); }
    c::set_pi32(w, id, p::VOICE, -1);
    set_death_bits(w, id, 0, -1);
    let j = w.joint_point(id, 5);
    fx::beam_explosion(w, &BEAM, Some(id), j);
    w.play_sound(1, 0, id);
    w.delete_moby(id);
}

/// `0x2c1e98`: no other 340 of the run list within 3 (xy) of it nor nearer (xy) to the target.
fn nearest(w: &World, id: MobyId) -> bool {
    let t = target::acquire(w, id, range(w, id));
    let me = c::pos(w, id);
    let d = c::dist2(me, t.pos);
    for o in super::hints::run_list(w) {
        if o == id || w.m(o).o_class != CLASSES[0] { continue; }
        let q = w.m(o).position;
        if c::dist2(me, q) < 3.0 || c::dist2(q, t.pos) < d { return false; }
    }
    true
}

/// `0x2c1fb8(range, m, &goal)`: the goal among its kind (pushed apart within 3 at [`SPREAD`] each; the target itself
/// when no other is nearer to it, else a point at 4.5–5.5 from it averaged in at weight 10) and the heading to it (to
/// the target when the goal is within 1).
fn flock(w: &World, id: MobyId, r: f32) -> (f32, c::V) {
    let t = target::acquire(w, id, r);
    let me = c::pos(w, id);
    let dt = c::dist2(me, t.pos);
    let mut goal = [0.0; 4];
    let mut sum = 0.0;
    let mut first = true;
    for o in super::hints::run_list(w) {
        if o == id || w.m(o).o_class != w.m(id).o_class { continue; }
        let q = w.m(o).position;
        if c::dist2(q, me) < 3.0 {
            goal = c::add(goal, c::scale(c::sub(me, q), SPREAD));
            sum += SPREAD;
        }
        if c::dist2(q, t.pos) < dt { first = false; }
    }
    if first {
        goal = t.pos;
    } else {
        let s = if dt < 4.5 { 5.0 } else if 5.5 < dt { -5.0 } else { 0.0 };
        sum += 10.0;
        goal = c::add(goal, c::set_len3(c::sub(me, t.pos), s * 10.0));
        goal = c::add(c::scale(goal, 1.0 / sum), me);
    }
    if 1.0 <= c::dist2(me, goal) { (heading_to(me, goal), goal) } else { (heading_to(me, t.pos), me) }
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    let d = p::D;
    c::set_pf(w, id, d, 1.0);
    c::set_pi16(w, id, d + 4, 1);
    c::set_pu8(w, id, d + 8, 0);
    c::set_pu8(w, id, d + 0xe, 1);
    c::set_pu8(w, id, d + 9, 0);
    c::set_pf(w, id, d + 0x10, 0.75);
    c::set_pu8(w, id, 0x58, 8);
    c::set_pu8(w, id, 0x5a, 6);
    walker::seed(&mut w.mm(id).pvars, p::J);
    c::set_pf(w, id, p::J + 0xc, 0.15);
    c::set_pf(w, id, p::J + 8, 0.25);
    c::set_pi32(w, id, p::J, 0x200);
    c::set_pf(w, id, p::J + 0x24, DT * 3.0);
    c::set_pf(w, id, p::J + 4, 1.0);
    c::set_pi32(w, id, p::J + 0x38, 4);
    let pos = c::pos(w, id);
    c::set_pv4(w, id, p::WANDER, pos);
    c::set_pf(w, id, p::WANDER + walker::wr::LEASH, 2.0);
    c::set_pf(w, id, p::WANDER + 0x14, DT + DT);
    c::set_pf(w, id, p::WANDER + walker::wr::TURN, DT * 2.967_059_6);
    c::set_pv4(w, id, p::HOME, pos);
    set_hold(w, id, 60);
    c::set_pi32(w, id, p::SUCK + react::rec::SEQS, react::seq_table_id(react::SEQS_340));
    c::set_pi16(w, id, p::ALERT, 0);
    c::set_pi16(w, id, p::STRAFE_T, 0);
    let s = if c::pi32(w, id, p::PATH) == -1 { 1 } else { 4 };
    set_st(w, id, s);
}

/// The hits (module doc).
fn hits(w: &mut World, id: MobyId) {
    let s = st(w, id);
    if s == 8 || s == 0 { return; }
    let hit = w.get_hit(id, 0x21_0000, false);
    let res = damage::resolve(w, id, hit, p::D, 0, 4);
    let Some(h) = hit else { return };
    if c::pu8(w, id, p::GUARD) != 0 { return; }
    let me = c::pos(w, id);
    let from = match h.attacker.filter(|&a| Some(a) != w.hero_moby && a < w.table.mobys.len()) {
        Some(a) => w.m(a).position,
        None => super::hero_pos(w),
    };
    let heading = c::atan(me[0] - from[0], me[1] - from[1]);
    let k = p::K;
    c::set_pf(w, id, k + 0x10, f32::from_bits(0x3c03_126f));
    c::set_pf(w, id, k + 0x14, f32::from_bits(0x3a03_126f));
    c::set_pf(w, id, k + 0x18, DT * 4.0);
    c::set_pi32(w, id, k + 0x24, 9);
    c::set_pu8(w, id, k + 0x3d, 0);
    let class = h.attacker.filter(|&a| a < w.table.mobys.len()).map(|a| w.m(a).o_class);
    let mut dmg = res.damage;
    if class.is_some_and(|c| (0xb0..=0xb1).contains(&c)) { dmg = 1.0; }
    let hp = c::pf(w, id, p::D) - dmg;
    c::set_pf(w, id, p::D, hp);
    let reaction = if hp <= 0.0 { 1 } else { res.reaction };
    set_alert(w, id);
    match reaction {
        1 | 2 => die(w, id),
        3..=6 => {
            blend(w, id, 8, 5);
            let k = 10.0 / gscale(w, 15.0);
            let push = if matches!(class, Some(0x131) | Some(0xb0)) { k * 0.5 } else { k };
            c::set_pf(w, id, p::PUSH, push);
            set_st(w, id, 6);
            w.mm(id).rotation[2] = c::add_rot(PI, heading);
            for _ in 0..2 {
                c::set_pu8(w, id, p::F + 7, 0x78);
                flash::start(w, id, p::F);
                let g = w.ticks(20);
                c::set_pu8(w, id, p::GUARD, g as u8);
            }
        }
        _ => {}
    }
}

fn set_alert(w: &mut World, id: MobyId) {
    let t = w.ticks(240);
    c::set_pi16(w, id, p::ALERT, t as i16);
}

/// State 7's arm spheres (module doc).
fn swing(w: &mut World, id: MobyId, t: Option<MobyId>) {
    for list in ARMS {
        let j = w.joint_point(id, list);
        let me = c::pos(w, id);
        let d = c::set_len3(c::sub(super::hero_pos(w), me), DT * 10.0);
        let tmpl = HitTemplate { dir: pv(d), attacker: Some(id), flags: 0x1_0000, b18: 0, b19: 0, h1a: 0, damage: Pf::ONE, w20: 1 };
        let listed = sv::sphere_mobys_in(w.table, w.svc, w.classes, pf(0.5), pv(j), 0x10, Some(id), Some(&tmpl));
        if listed.first().copied() == t {
            let cap = w.ticks(60) as i16;
            if cap <= hold(w, id) { c::set_pi16(w, id, p::HOLD, cap); }
        }
    }
}

/// Noticed: the first time in range, seq 6 and the hold `ticks(60)`.
fn notice(w: &mut World, id: MobyId) {
    let f = c::pi16(w, id, p::FLAGS);
    if f & 1 != 0 { return; }
    c::set_pi16(w, id, p::FLAGS, f | 1);
    blend(w, id, 6, 10);
    set_hold(w, id, 60);
}

/// To the swing: seq 7, hold `ticks(60)`, 7.
fn to_swing(w: &mut World, id: MobyId) {
    blend(w, id, 7, 10);
    set_hold(w, id, 60);
    set_st(w, id, 7);
}

/// States 1, 4, 5: in range (the target in the area when not alerted) → noticed; the swing or the approach (module doc).
/// True when it engaged (in range).
fn engage(w: &mut World, id: MobyId, t: &target::Target, d: f32, zero_turn: bool) -> bool {
    if range(w, id) <= d { return false; }
    if !alerted(w, id) && !in_area(w, id, t.pos) { return false; }
    notice(w, id);
    if d < 2.0 && hold(w, id) == 0 && nearest(w, id) {
        to_swing(w, id);
        if zero_turn { c::set_pf(w, id, p::TURN_V, 0.0); }
    } else if hold(w, id) == 0 {
        blend(w, id, 1, 10);
        c::set_pu8(w, id, p::QUADRANT, 0);
        set_st(w, id, 3);
    }
    true
}

/// State 4 (module doc): returns the goal and the heading.
fn patrol(w: &mut World, id: MobyId, t: &target::Target, d: f32) -> (c::V, f32) {
    engage(w, id, t, d, false);
    let Some(path) = spline(w, c::pi32(w, id, p::PATH)) else { return (c::pos(w, id), w.m(id).rotation[2]) };
    let n = w.svc.splines[path].len() as i32;
    let me = c::pos(w, id);
    let node = c::pi32(w, id, p::NODE);
    let q = node_point(w, path, node);
    let next = if c::dist2(q, me) < 1.0 { true } else { 2.356_194_5 < c::diff_rots(w.m(id).rotation[2], heading_to(me, q)) };
    if next {
        let k = node + 1;
        c::set_pi32(w, id, p::NODE, if k == n { 0 } else { k });
    }
    for k in 0..4 {
        let cub = c::pi32(w, id, p::PADS + 4 * k);
        if hold(w, id) != 0 || cub == -1 || !w.in_cuboid([me[0], me[1], me[2]], cub) { continue; }
        c::set_pi16(w, id, p::PAD, cub as i16);
        set_st(w, id, 5);
        c::set_pi16(w, id, p::PAD_STEP, 0);
        break;
    }
    let q = node_point(w, path, c::pi32(w, id, p::NODE));
    (q, heading_to(me, q))
}

/// The pad cuboid's centre and yaw.
fn pad_cuboid(w: &World, id: MobyId) -> (c::V, f32) {
    let k = c::pi16(w, id, p::PAD) as i32;
    w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, k).map(|s| (s.matrix[3], s.euler[2])).unwrap_or(([0.0; 4], 0.0))
}

/// State 5 (module doc): returns the goal.
fn pad(w: &mut World, id: MobyId, t: &target::Target, d: f32) -> c::V {
    engage(w, id, t, d, true);
    let (centre, yaw) = pad_cuboid(w, id);
    match c::pi16(w, id, p::PAD_STEP) {
        0 => {
            turn_to(w, id, yaw);
            if seq_b(w, id) != 1 { blend(w, id, 1, 10); }
            if c::dist2(c::pos(w, id), centre) < 1.0 && c::diff_rots(w.m(id).rotation[2], yaw) < 0.174_532_92 {
                c::set_pi16(w, id, p::PAD_STEP, 1);
            }
        }
        1 => {
            if seq_b(w, id) == 7 {
                if wrapped(w, id) {
                    blend(w, id, 0, 10);
                    c::set_pf(w, id, p::TURN_V, 0.0);
                    c::set_pi16(w, id, p::PAD_STEP, 2);
                }
            } else {
                blend(w, id, 7, 10);
            }
        }
        2 => {
            let q = spline(w, c::pi32(w, id, p::PATH)).map(|s| node_point(w, s, c::pi32(w, id, p::NODE))).unwrap_or([0.0; 4]);
            turn_to(w, id, heading_to(c::pos(w, id), q));
            if c::diff_rots(w.m(id).rotation[2], heading_to(c::pos(w, id), q)) < 0.174_532_92 {
                set_st(w, id, 4);
                set_hold(w, id, 240);
            }
        }
        _ => {}
    }
    centre
}

/// `0x26d270(0.25, radius, 0.5, 30°, m, &p, &to, flags)` from a copy of the position, kept unless it leaves the area.
fn step_to(w: &mut World, id: MobyId, to: c::V, radius: f32) -> i32 {
    let mut from = c::pos(w, id);
    let mut to = to;
    let ok = walker::move_ground(w, id, 0.25, radius, 0.5, SLOPE, &mut from, &mut to, 0);
    if in_area(w, id, from) {
        let m = w.mm(id);
        m.position[0] = from[0];
        m.position[1] = from[1];
        m.position[2] = from[2];
    }
    ok
}

fn ahead(w: &World, id: MobyId, a: f32, s: f32) -> c::V {
    let me = c::pos(w, id);
    [me[0] + a.cos() * s, me[1] + a.sin() * s, me[2], me[3]]
}

/// State 3's move: around the goal by quadrants (module doc).
fn strafe(w: &mut World, id: MobyId, goal: c::V) {
    let me = c::pos(w, id);
    let yaw = w.m(id).rotation[2];
    let mut seq = seq_b(w, id);
    let to = if DT * 3.0 + DT * 3.0 <= c::dist2(me, goal) {
        let rel = c::sub_rot(heading_to(me, goal), yaw);
        let mut r = c::add_rot(rel, FRAC_PI_4);
        if r < 0.0 { r += 2.0 * PI; }
        let cur = |w: &World| sv::fl(sv::normalize_angle(sv::normalize_angle(pf(c::pu8(w, id, p::QUADRANT) as f32 * FRAC_PI_2))));
        if 1.745_329_3 < c::diff_rots(cur(w), rel) {
            let q = ((r * 1.999) / PI) as i32 as u8;
            c::set_pu8(w, id, p::QUADRANT, q);
            seq = STRAFE[(q & 3) as usize];
        }
        let a = c::add_rot(yaw, sv::fl(sv::normalize_angle(pf(c::pu8(w, id, p::QUADRANT) as f32 * FRAC_PI_2))));
        ahead(w, id, a, DT * 3.0)
    } else {
        seq = 0;
        me
    };
    let mut from = me;
    let mut to = to;
    let ok = walker::move_ground(w, id, 0.25, 0.57, 0.5, SLOPE, &mut from, &mut to, 0);
    if in_area(w, id, from) {
        let m = w.mm(id);
        m.position[0] = from[0];
        m.position[1] = from[1];
        m.position[2] = from[2];
        if ok == 0 { seq = 0; }
    } else {
        seq = 0;
    }
    if c::dec_timer_pvar_s16(w, id, p::STRAFE_T) != 0 && seq != seq_b(w, id) {
        blend(w, id, seq, 10);
        let t = w.ticks(15);
        c::set_pi16(w, id, p::STRAFE_T, t as i16);
    }
}

/// The moves after the state code (module doc).
fn moves(w: &mut World, id: MobyId, goal: c::V, kt: f32) {
    match st(w, id) {
        4 => {
            if seq_b(w, id) != 1 { blend(w, id, 1, 10); }
            let to = ahead(w, id, w.m(id).rotation[2], DT + DT);
            step_to(w, id, to, 0.57);
        }
        3 => strafe(w, id, goal),
        s if s == 5 || (s == 7 && seq_a(w, id) == 7 && 15.0 < kt) => {
            let me = c::pos(w, id);
            if DT + DT < c::dist2(me, goal) {
                let to = ahead(w, id, heading_to(me, goal), DT + DT);
                step_to(w, id, to, 1.25);
            }
        }
        6 | 8 => {}
        _ => {
            let mut from = c::pos(w, id);
            let mut to = from;
            walker::move_ground(w, id, 0.25, 0.57, 0.5, SLOPE, &mut from, &mut to, 0);
            let m = w.mm(id);
            m.position[0] = from[0];
            m.position[1] = from[1];
        }
    }
}

/// The exhaust puff (type 22) at joint 0 (module doc).
fn exhaust(w: &mut World, id: MobyId) {
    let x = w.rng.randf(1.0, -1.0) * DT * 0.1;
    let y = w.rng.randf(1.0, -1.0) * DT * 0.1;
    let z = w.rng.randf(0.8, 1.2) * DT * -2.5;
    let n = w.rng.randi(100);
    let at = w.joint_point(id, 0);
    let r = w.m(id).rows;
    let v: c::V = std::array::from_fn(|k| if k == 3 { 0.0 } else { r[0][k] * x + r[1][k] * y + r[2][k] * z });
    let c1 = if n & 1 != 0 { 0x400f_0f7f } else { 0x407f_7f7f };
    let size = w.rng.randf(75000.0, 100000.0);
    let life = if n & 1 == 0 { w.rng.rand_range(20, 30) } else { w.rng.rand_range(10, 15) };
    let life = w.ticks(life);
    c::projectile::part22(w, &crate::particles::type22::Spawn { size, pos: at, vel: v, c1, c2: 0x27_2727, life });
}

/// Level04 0x2c2270 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < p::SIZE { return; }
    let yaw0 = w.m(id).rotation[2];
    crate::moby_update::manip::big_head(w, 2.9, id, 7, id, p::BIG_HEAD);
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| x.to_f32());
        if c::dist3(c::pos(w, id), cam) < 27.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x15;
        }
    }
    let t = target::acquire(w, id, range(w, id));
    let kt = c::ground::key_time(w, id);
    let d = c::dist2(c::pos(w, id), t.pos);
    let mut goal = c::pos(w, id);
    w.mm(id).update_dist = 0x20;
    engine(w, id);
    if matches!(st(w, id), 1 | 3) && c::dist3(c::pos(w, id), super::hero_pos(w)) < 1.5 {
        blend(w, id, 7, 10);
        set_hold(w, id, 60);
        set_st(w, id, 7);
    }
    let mut g = c::pu8(w, id, p::GUARD);
    sv::fast_dec_timer_u8(&mut g);
    c::set_pu8(w, id, p::GUARD, g);
    hits(w, id);
    if w.m(id).state >= 0xfd { return; }
    w.mm(id).hit_slot = 0xff;
    if c::pi32(w, id, p::LURE) != 0 { set_alert(w, id); }
    c::set_pi32(w, id, p::LURE, 0);
    c::dec_timer_pvar_s16(w, id, p::ALERT);
    if st(w, id) == 7 && seq_a(w, id) == 7 && 15.0 < kt && kt < 44.0 { swing(w, id, t.moby); }
    if st(w, id) != 0 && c::pu8(w, id, p::D + 0xe) != 1 {
        let voice = c::pi32(w, id, p::VOICE);
        if voice != -1 { w.release_sound(voice, id); }
        c::set_pi32(w, id, p::VOICE, -1);
        w.delete_moby(id);
        return;
    }
    c::dec_timer_pvar_s16(w, id, p::HOLD);
    let mut turn_a = None;
    match st(w, id) {
        0 => init(w, id),
        1 => {
            let old = c::pos(w, id);
            walker::wander(w, id, 0.35, 0.25, p::WANDER);
            if !in_area(w, id, c::pos(w, id)) {
                let m = w.mm(id);
                m.position[0] = old[0];
                m.position[1] = old[1];
                m.position[2] = old[2];
            }
            if seq_a(w, id) != 0 && wrapped(w, id) { blend(w, id, 0, 5); }
            engage(w, id, &t, d, false);
        }
        2 => {
            if seq_b(w, id) == 5 {
                if wrapped(w, id) {
                    blend(w, id, 0, 10);
                    set_st(w, id, 1);
                }
            } else {
                blend(w, id, 5, 5);
                c::set_pf(w, id, p::FALL, -(DT * 7.668));
            }
        }
        3 => {
            let mut a = yaw0;
            if d < range(w, id) && (alerted(w, id) || in_area(w, id, t.pos)) {
                let (h, gl) = flock(w, id, range(w, id));
                a = h;
                goal = gl;
                if d < 2.0 && hold(w, id) == 0 && nearest(w, id) { to_swing(w, id); }
            } else if !alerted(w, id) {
                let home = c::pv4(w, id, p::HOME);
                goal = [home[0], home[1], goal[2], goal[3]];
                if c::dist2(c::pos(w, id), goal) < 1.0 {
                    blend(w, id, 0, 10);
                    set_st(w, id, 1);
                }
            }
            turn_a = Some(a);
        }
        4 => {
            let (gl, a) = patrol(w, id, &t, d);
            goal = [gl[0], gl[1], goal[2], goal[3]];
            turn_a = Some(a);
        }
        5 => {
            let gl = pad(w, id, &t, d);
            goal = [gl[0], gl[1], goal[2], goal[3]];
        }
        6 => {
            let on = if seq_a(w, id) == 8 { 30.0 < kt || wrapped(w, id) } else { wrapped(w, id) };
            if on {
                blend(w, id, 0, 10);
                set_st(w, id, 1);
                c::set_pi16(w, id, p::HOLD, 0);
            }
        }
        7 => {
            let (a, gl) = flock(w, id, range(w, id));
            goal = gl;
            if hold(w, id) == 0 && (44.0 < kt || wrapped(w, id)) {
                blend(w, id, 0, 5);
                set_hold(w, id, 120);
                set_st(w, id, 1);
                let f = c::pi16(w, id, p::FLAGS);
                c::set_pi16(w, id, p::FLAGS, f | 2);
            }
            turn_a = Some(a);
        }
        8 => {
            if wrapped(w, id) { return die(w, id); }
        }
        9 if react::carried(w, id, p::K) != 0 => {
            set_st(w, id, 0);
            c::set_pi16(w, id, p::SUCK + react::rec::STATE, 0);
        }
        _ => {}
    }
    if let Some(a) = turn_a { turn_to(w, id, a); }
    let push = c::pf(w, id, p::PUSH);
    if 0.0 < push {
        let a = c::add_rot(w.m(id).rotation[2], PI);
        let to = ahead(w, id, a, push);
        let k = gscale(w, 15.0);
        c::set_pf(w, id, p::PUSH, push + -10.0 / (k * k));
        let mut from = c::pos(w, id);
        let mut to = to;
        walker::move_ground(w, id, 0.25, 0.57, 0.5, SLOPE, &mut from, &mut to, 1);
        let m = w.mm(id);
        m.position[0] = from[0];
        m.position[1] = from[1];
    }
    if st(w, id) == 9 { return; }
    moves(w, id, goal, kt);
    let v = c::pf(w, id, p::FALL) + DT2 * 9.8;
    c::set_pf(w, id, p::FALL, v);
    w.mm(id).position[2] -= v - 2.0;
    let gz = c::ground::ground(w, c::pos(w, id), 0.5, 0).z;
    let z = w.m(id).position[2] - 2.0;
    w.mm(id).position[2] = z;
    if z < gz {
        w.mm(id).position[2] = gz;
        c::set_pf(w, id, p::FALL, 0.0);
    }
    if w.m(id).position[2] < c::pf(w, id, p::HOME + 8) - 5.0 { return die(w, id); }
    let cam = w.camera.map(|x| x.to_f32());
    if c::dist2(c::pos(w, id), cam) < 20.0 { exhaust(w, id); }
    flash::update(w, id, p::F);
}
