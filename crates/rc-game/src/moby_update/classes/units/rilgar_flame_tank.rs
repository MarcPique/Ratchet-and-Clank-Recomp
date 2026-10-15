//! **Rilgar's flame tanks, class 625** (level05 `0x304320` with its hits `0x305178` and flame hits `0x305898`, 4
//! placed; census U190; the name is descriptive [L]). A tracked machine with a turret that drives up and down its path
//! (pvar +0xd4), turning back at the ends of an open one. When Ratchet is within its sight (20, 14 for 4 s after a
//! lure) in xy and 4 in height, inside its region (+0x190, when it has one) and in view from 3 above it, it sprays
//! flames from joint list 2 along its turret until he leaves; every 8th tick one puff is kept and burns what it
//! touches (damage 1, unless Ratchet wears head item 6). Its treads (0x368 / 0x369) roll with its speed and turn. Six
//! points of health (a zero-damage hit kills too, the game's); death: three blasts, eight body pieces, the bolts.
//! Read from the level05 decomp and disassembly (the particle and blast arguments) and its words gp−0x5110..−0x5084.
//! Native `f32`.
//!
//! **Pvars** (0x1c0; the creature header: damage +0x20, flash +0x60, walker +0x70): +0x20 the damage record (health
//! 6, meter +0x24 6, +0x28 column 3), +0x2e (1 at init; 2 deletes it: no setter on level 05 [L]), +0x38 the lure, +0x40
//! the walk's move, +0x58 / +0x5a bytes 14 / 13, +0x60 the flash, +0x70 the walker J (+0x84 its turn velocity, +0x94 its
//! speed 2·dt), +0xc0 the path node, +0xc4 the direction (s8), +0xd4 the path, +0xf0 / +0x130 the turret's yaw / pitch
//! manipulators, +0x174 the sight, +0x178 the lure timer, +0x180 (`ticks(60)`, not read), +0x184 / +0x188 the treads
//! (moby + 1 here), +0x190 the region path (−1 none), +0x194 / +0x198 the turret's yaw (world) / pitch, +0x19e (s16,
//! `ticks(5)` at death, not read), +0x1a0 eight kept flames (the particle record + 1 here).
//!
//! | address | what | port |
//! |---|---|---|
//! | top | the hits `0x305178`, the flame hits `0x305898`; +0x2e = 2 → the treads and itself deleted | [`update`] |
//! | top | drawn and within 27 (3-D) of the camera → the shadow probe (`0x2843d8` = `0x26f020`), +0x7f 0x15 | [`update`] (`shadows::probe_down`) |
//! | state 0 | the treads (0x368 / 0x369: update / draw distance 0x40, drawn, Ratchet's light, mode 0x4000, here, no collision); `AttachManipulator` 0 (+0xf0) and 1 (+0x130); mode \|= 0x1000, Ratchet's light; `SeedJumpPattern(J)`, J0 0x38d, J3 2, J2 4, J15 0.01, J16 0.3, J17 0.1; node 1, at the path's point 0; 1, seq 0; J9 2·dt; move 0; health 6, +0x29 0, meter 6, column 3, +0x3e \|= 2; +0x180 `ticks(60)`; pitch 0, yaw = its yaw; +0x58 = 14, +0x5a = trunc(1.7·8) | [`init`] |
//! | states 1 / 2 | the next node (node + dir mod n); `0x283308` (= `0x26de80`) walk to it (out +0x40); within 1 (xy): at the end (n − 1 going on, 0 going back) of an open path (ends more than 5 apart) the direction flips; node = next | [`drive`] (`walker::walk_to`) |
//! | state 1 | Ratchet in sight ([`sees`]) → seq 0 (3 ticks), 2; the turret: within 16 (xy) toward him and pitch 5°, else toward its yaw and 0 (`0x285e78` = `0x270ac0`, π/2·dt) | [`update`] ([`aim`]) |
//! | state 2 | joint list 2's point (`0x279a30`); two puffs (type 2, ±0.065, out 8·dt along the turret, down `randf(0.9, 1.1)·dt`, size 0.75; second velocity 1·dt at ±90°, up `randf(1.8, 2.2)·dt`, size 1.5; phases `scale(randf(20, 40))`, `scale(randf(15, 18))`, `scale(randf(40, 48))`; colours 0x30108010 / 0x10108010), the first kept every 8th tick (`0x15f5cc & 7`); two sparks (±0.2, ±10° out at 8·dt, down `randf(0.7, 1.5)·dt`, ±90° at 1·dt, up `randf(1.4, 3)·dt`, sizes 0.1, colour 0x8010ff10); Ratchet out of sight → seq 0 (3 ticks), 1; the turret toward him | [`flames`], [`update`] |
//! | state 3 | the treads and itself deleted | [`update`] |
//! | `0x305178` | `MobyGetHitMessage(0x210000)` of itself and both treads, the strongest (`0x2899f8`: the largest +0x2c, later on ties); none from itself, nor from an attacker of class 0 / 0x47; the resolver (column 4); out5 ≠ 1 outside state 3: health −= damage; ≤ 0, or no damage → sound 3, `SetDeathBits(m, 0, −1)`, three `SpawnBeamExplosion(0, 0, 4, 2, 9, 1, 15, …, 20, 8, 20, −1, shake, 1)` (3 above it, 2 ahead, 2 behind), 3, `BreakFxB` 0x6dd, 0x6df, 0x6e0, 0x6fe ×2, 0x6ff ×3 at its position / rotation; else sound 2, flash colour 200, the flash | [`hits`] (`damage::resolve`, `fx::beam_explosion`, `fx::break_piece`) |
//! | `0x305178` tail | +0xa4 = 0xff (and the treads'); a lure with the timer out → timer `ticks(240)`, lure 0, Ratchet behind it (more than 90°) → the direction flips; the timer running → sight 14, else 20; the treads: anim speed `|move| / (2.5·dt) ± J5·30`, its rotation and position; the flash | [`hits`] |
//! | `0x305898` | kept flames no longer type 2 or at +0x32 = 0 dropped; head item ≠ 6: each hits a sphere of 0.75 at its position (`coll_sphere_mobys(…, 0, m, tmpl)`: push 1.5 in xy toward Ratchet, z 1, w 5627.95; damage 1, flags 1, type 0 / 1, the class) | [`flame_hits`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, damage, flash, fx, turn, walker};
use crate::moby_update::manip;
use crate::moby_update::services::{pf, pv as v4, HitTemplate, World};
use crate::particles::type02;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 5;
pub const UPDATE_FN: u32 = 0x30_4320;
pub const CLASSES: [i16; 1] = [625];
/// The treads (left, right).
pub const TREADS: [i16; 2] = [0x368, 0x369];
/// The body pieces of its death.
pub const PIECES: [i16; 8] = [0x6dd, 0x6df, 0x6e0, 0x6fe, 0x6fe, 0x6ff, 0x6ff, 0x6ff];
/// The head item that spares Ratchet from the flames (0x1404a8).
pub const SPARING_HEAD_ITEM: i32 = 6;
/// `SpawnBeamExplosion(0, 0, 4, 2, 9, 1, 15, m, _, p, 20, 8, 20, −1, 1, 1, −1, 0)`.
pub const BLAST: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 0x14, sparks: 8, puffs: 0x14, debris: 1, sound: -1, shake: true };
/// gp−0x5110 (2.5: the treads' speed ×dt), −0x510c (30: their turn), −0x5108 (2: the speed ×dt).
const TREAD_SPEED: f32 = 2.5;
const TREAD_TURN: f32 = 30.0;
const SPEED: f32 = 2.0;
/// The turret's pitch at Ratchet (0x3db2b8c2, 5°) and its turn (π/2 a second).
const PITCH: f32 = f32::from_bits(0x3db2_b8c2);
/// gp−0x50f4..−0x50c8: the puffs (colours, phases 20 / 15 / 40, out 8, down 1, second 1, up 2, sizes 0.75 / 1.5,
/// spread 90°); −0x50c4..−0x5094 the sparks (colour, the same phases and speeds, sizes 0.1, spread 90° / 10°).
const PUFF_COLOURS: (u32, u32) = (0x3010_8010, 0x1010_8010);
const SPARK_COLOUR: u32 = 0x8010_ff10;
const DEG: f32 = 0.017_453_292;

/// Pvar offsets (module doc).
pub mod pv {
    pub const D: usize = 0x20;
    pub const DESPAWN: usize = 0x2e;
    pub const LURE: usize = 0x38;
    pub const MOVE: usize = 0x40;
    pub const F: usize = 0x60;
    pub const J: usize = 0x70;
    pub const NODE: usize = 0xc0;
    pub const DIR: usize = 0xc4;
    pub const PATH: usize = 0xd4;
    pub const YAW_M: usize = 0xf0;
    pub const PITCH_M: usize = 0x130;
    pub const SIGHT: usize = 0x174;
    pub const LURE_T: usize = 0x178;
    pub const W180: usize = 0x180;
    pub const TREADS: usize = 0x184;
    pub const REGION: usize = 0x190;
    pub const YAW: usize = 0x194;
    pub const PITCH: usize = 0x198;
    pub const DEATH_T: usize = 0x19e;
    pub const FLAMES: usize = 0x1a0;
    pub const SIZE: usize = 0x1c0;
}
const SLOTS: usize = 8;

fn gscale(w: &World, x: f32) -> f32 { w.svc.timing.scale(Pf::f(x)).to_f32() }
fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    usize::try_from(c::pi32(w, id, o) - 1).ok().filter(|&m| m < w.table.mobys.len())
}
fn path(w: &World, o: i32) -> Option<Vec<c::V>> {
    usize::try_from(o).ok().and_then(|p| w.svc.splines.get(p)).map(|s| s.iter().map(|q| q.map(f32::from_bits)).collect())
}

/// The treads and itself deleted (state 3, +0x2e = 2).
fn delete_all(w: &mut World, id: MobyId) {
    for k in 0..2 {
        if let Some(t) = link(w, id, pv::TREADS + 4 * k) { w.delete_moby(t); }
    }
    w.delete_moby(id);
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    let pos = c::pos(w, id);
    for (k, class) in TREADS.into_iter().enumerate() {
        if c::pi32(w, id, pv::TREADS + 4 * k) != 0 { continue; }
        let Some(t) = w.create_moby(class) else { continue };
        c::set_pi32(w, id, pv::TREADS + 4 * k, t as i32 + 1);
        let m = w.mm(t);
        m.update_dist = 0x40;
        m.draw_dist = 0x40;
        m.visible = 1;
        super::take_hero_light(w, t);
        let m = w.mm(t);
        m.mode = 0x4000;
        m.position = pos;
        m.has_collision = false;
        w.build_matrix(t);
    }
    manip::attach(w, id, 0, id, pv::YAW_M);
    manip::attach(w, id, 1, id, pv::PITCH_M);
    w.mm(id).mode |= mode::TARGETABLE;
    super::take_hero_light(w, id);
    let j = pv::J;
    walker::seed(&mut w.mm(id).pvars, j);
    c::set_pi32(w, id, j, 0x38d);
    c::set_pf(w, id, j + 0xc, 2.0);
    c::set_pf(w, id, j + 8, 4.0);
    c::set_pf(w, id, j + 0x3c, f32::from_bits(0x3c23_d70a));
    c::set_pf(w, id, j + 0x40, f32::from_bits(0x3e99_999a));
    c::set_pf(w, id, j + 0x44, f32::from_bits(0x3dcc_cccd));
    c::set_pi32(w, id, pv::NODE, 1);
    if let Some(p) = path(w, c::pi32(w, id, pv::PATH)).and_then(|p| p.first().copied()) { c::set_pos(w, id, p); }
    w.mm(id).state = 1;
    c::blend_to(w, id, 0, 0, 0);
    c::set_pf(w, id, j + 0x24, SPEED * c::DT);
    c::set_pv4(w, id, pv::MOVE, [0.0; 4]);
    c::set_pu8(w, id, pv::DESPAWN, 1);
    c::set_pu8(w, id, pv::D + 9, 0);
    c::set_pi16(w, id, pv::D + 4, 6);
    c::set_pu8(w, id, pv::D + 8, 3);
    c::set_pf(w, id, pv::D, 6.0);
    let fl = c::pi16(w, id, pv::D + 0x1e) as u16 | 2;
    c::set_pi16(w, id, pv::D + 0x1e, fl as i16);
    let t = w.ticks(60);
    c::set_pi32(w, id, pv::W180, t);
    c::set_pf(w, id, pv::PITCH, 0.0);
    let yaw = c::yaw(w, id);
    c::set_pf(w, id, pv::YAW, yaw);
    c::set_pu8(w, id, 0x58, 14);
    c::set_pu8(w, id, 0x5a, (f32::from_bits(0x3fd9_999a) * 8.0) as u8);
}

/// States 1 / 2: the walk to the next node (module doc).
fn drive(w: &mut World, id: MobyId) {
    let Some(pts) = path(w, c::pi32(w, id, pv::PATH)) else { return };
    let n = pts.len() as i32;
    if n == 0 { return; }
    let dir = c::pu8(w, id, pv::DIR) as i8 as i32;
    let end = if 0 <= dir { n - 1 } else { 0 };
    let next = (c::pi32(w, id, pv::NODE) + n + dir).rem_euclid(n);
    let target = pts[next as usize];
    let mut out = [0.0; 4];
    walker::walk_to(w, id, pv::J, target, &mut out);
    c::set_pv4(w, id, pv::MOVE, out);
    if c::dist2(c::pos(w, id), target) < 1.0 {
        if next == end && 5.0 < c::dist3(pts[n as usize - 1], pts[0]) { c::set_pu8(w, id, pv::DIR, (-dir) as i8 as u8); }
        c::set_pi32(w, id, pv::NODE, next);
    }
}

/// Ratchet within the sight (xy; at most it while flaming) and 4 in height, in the region, and in view from 3 above
/// it.
fn sees(w: &World, id: MobyId, flaming: bool) -> bool {
    let (h, p) = (super::hero_pos(w), c::pos(w, id));
    let (d, sight) = (c::dist2(p, h), c::pf(w, id, pv::SIGHT));
    if (if flaming { sight < d } else { sight <= d }) || 4.0 <= (p[2] - h[2]).abs() { return false; }
    let region = c::pi32(w, id, pv::REGION);
    if region != -1 {
        let Some(poly) = path(w, region) else { return false };
        if !crate::moby_update::triggers::point_in_path_polygon([h[0], h[1], h[2]], &poly) { return false; }
    }
    let from = [p[0], p[1], p[2] + 3.0, p[3]];
    w.coll_line(v4(from), w.hero.body_point, 2, Some(id)).is_none()
}

/// The turret toward `yaw` / `pitch`, then its manipulators.
fn aim(w: &mut World, id: MobyId, yaw: f32, pitch: f32) {
    let step = c::DT * std::f32::consts::FRAC_PI_2;
    let (mut y, mut p) = (c::pf(w, id, pv::YAW), c::pf(w, id, pv::PITCH));
    turn::approach_rot(yaw, step, &mut y);
    turn::approach_rot(pitch, step, &mut p);
    c::set_pf(w, id, pv::YAW, y);
    c::set_pf(w, id, pv::PITCH, p);
    manip::set_axis(w, id, id, pv::PITCH_M, p, 1);
    let rel = c::sub_rot(y, c::yaw(w, id));
    manip::set_axis(w, id, id, pv::YAW_M, rel, 2);
}

fn heading_to_hero(w: &World, id: MobyId) -> f32 {
    let (h, p) = (super::hero_pos(w), c::pos(w, id));
    c::atan(h[0] - p[0], h[1] - p[1])
}

/// One flame particle (the three phases drawn here, after the vectors).
fn puff(w: &mut World, p: c::V, v1: c::V, v2: c::V, colours: (u32, u32)) -> Option<usize> {
    let r0 = w.rng.randf(20.0, 40.0);
    let t0 = gscale(w, r0) as i32;
    let r1 = w.rng.randf(15.0, 15.0 * 1.2);
    let t1 = gscale(w, r1) as i32;
    let r2 = w.rng.randf(40.0, 40.0 * 1.2);
    let t2 = gscale(w, r2) as i32;
    let s = type02::Spawn { pos: p, v1, v2, c1: colours.0, c2: colours.1, t: [t0, t1, t2], def: -1 };
    fx::part02_rec(w, &s)
}

/// State 2's flames (module doc).
fn flames(w: &mut World, id: MobyId) {
    let base = w.joint_point(id, 2);
    let yaw = c::pf(w, id, pv::YAW);
    let dt = c::DT;
    for i in 0..2 {
        let j = f32::from_bits(0x3d85_1eb8);
        let mut p = base;
        for q in p.iter_mut().take(3) { *q += w.rng.randf(-j, j); }
        let (cy, sy) = c::cs(yaw);
        let z1 = w.rng.randf(-dt * 0.9, -dt * 1.1);
        let r = w.rng.randf(-90.0, 90.0);
        let (c2, s2) = c::cs(c::add_rot(yaw, r * DEG));
        let z2 = w.rng.randf(2.0 * dt * 0.9, 2.0 * dt * 1.1);
        let v1 = [cy * 8.0 * dt, sy * 8.0 * dt, z1, 0.75];
        let v2 = [c2 * dt, s2 * dt, z2, 1.5];
        let kept = puff(w, p, v1, v2, PUFF_COLOURS);
        if i == 0 && w.counter & 7 == 0 {
            if let Some(r) = kept {
                if let Some(s) = (0..SLOTS).find(|&s| c::pi32(w, id, pv::FLAMES + 4 * s) == 0) {
                    c::set_pi32(w, id, pv::FLAMES + 4 * s, r as i32 + 1);
                }
            }
        }
    }
    for _ in 0..2 {
        let mut p = base;
        for q in p.iter_mut().take(3) { *q += w.rng.randf(-0.2, 0.2); }
        let r1 = w.rng.randf(-10.0, 10.0);
        let (c1, s1) = c::cs(c::add_rot(yaw, r1 * DEG));
        let z1 = w.rng.randf(-dt * 0.7, -dt * 1.5);
        let r2 = w.rng.randf(-90.0, 90.0);
        let (c2, s2) = c::cs(c::add_rot(yaw, r2 * DEG));
        let z2 = w.rng.randf(2.0 * dt * 0.7, 2.0 * dt * 1.5);
        let v1 = [c1 * 8.0 * dt, s1 * 8.0 * dt, z1, 0.1];
        let v2 = [c2 * dt, s2 * dt, z2, 0.1];
        puff(w, p, v1, v2, (SPARK_COLOUR, SPARK_COLOUR));
    }
}

/// `0x305898`: the kept flames' hits (module doc).
fn flame_hits(w: &mut World, id: MobyId) {
    let mut live = Vec::new();
    for s in 0..SLOTS {
        let o = pv::FLAMES + 4 * s;
        let v = c::pi32(w, id, o);
        if v == 0 { continue; }
        let rec = w.particles.as_deref().and_then(|p| p.pool.recs.get((v - 1) as usize).copied());
        match rec {
            Some(r) if r[0] == 2 && i16::from_le_bytes([r[0x32], r[0x33]]) != 0 => {
                let f = |k: usize| f32::from_le_bytes([r[k], r[k + 1], r[k + 2], r[k + 3]]);
                live.push([f(0x10), f(0x14), f(0x18), f(0x1c)]);
            }
            _ => c::set_pi32(w, id, o, 0),
        }
    }
    if w.hero.head_slot.id == SPARING_HEAD_ITEM { return; }
    let (h, p) = (super::hero_pos(w), c::pos(w, id));
    let d = c::set_len2([h[0] - p[0], h[1] - p[1], 0.0, 0.0], 1.5);
    let tmpl = HitTemplate { dir: [pf(d[0]), pf(d[1]), Pf::ONE, Pf::b(0x45af_df66)], attacker: Some(id), flags: 1, b18: 0, b19: 1, h1a: w.m(id).o_class as u16, damage: Pf::ONE, w20: 1 };
    for at in live { w.sphere_mobys(pf(0.75), v4(at), 0, Some(id), Some(&tmpl)); }
}

/// `0x305178`: the hits (module doc).
fn hits(w: &mut World, id: MobyId) {
    let treads = [link(w, id, pv::TREADS), link(w, id, pv::TREADS + 4)];
    let mut best = w.get_hit(id, 0x21_0000, false);
    for t in treads.into_iter().flatten() {
        let h = w.get_hit(t, 0x21_0000, false);
        if let Some(n) = h {
            if best.is_none_or(|b| f32::from_bits(b.damage.0) <= f32::from_bits(n.damage.0)) { best = Some(n); }
        }
    }
    let best = best.filter(|b| b.attacker != Some(id)).filter(|b| b.attacker.is_none_or(|a| {
        let k = w.m(a).o_class;
        k != 0x47 && k != 0
    }));
    let res = damage::resolve(w, id, best, pv::D, 0, 4);
    if res.out5 != 1 && w.m(id).state != 3 {
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        if hp <= 0.0 || res.damage <= 0.0 {
            die(w, id);
            return;
        }
        w.play_sound(2, 0, id);
        c::set_pu8(w, id, pv::F + 7, 200);
        flash::start(w, id, pv::F);
    }
    w.mm(id).hit_slot = 0xff;
    for t in treads.into_iter().flatten() { w.mm(t).hit_slot = 0xff; }
    if c::pi32(w, id, pv::LURE) != 0 && c::pi32(w, id, pv::LURE_T) == 0 {
        let t = w.ticks(240);
        c::set_pi32(w, id, pv::LURE_T, t);
        c::set_pi32(w, id, pv::LURE, 0);
        if std::f32::consts::FRAC_PI_2 < c::diff_rots(c::yaw(w, id), heading_to_hero(w, id)) {
            let dir = c::pu8(w, id, pv::DIR) as i8;
            c::set_pu8(w, id, pv::DIR, (-dir) as u8);
        }
    }
    let sight = if c::dec_timer_pvar_i32(w, id, pv::LURE_T) == 0 { 14.0 } else { 20.0 };
    c::set_pf(w, id, pv::SIGHT, sight);
    let speed = c::len3(c::pv4(w, id, pv::MOVE)) / (TREAD_SPEED * c::DT);
    let turn_v = c::pf(w, id, pv::J + 0x14) * TREAD_TURN;
    let (pos, rot) = (c::pos(w, id), w.m(id).rotation);
    for (t, sign) in [(treads[0], 1.0), (treads[1], -1.0)] {
        let Some(t) = t else { continue };
        let m = w.mm(t);
        m.anim.speed = speed + sign * turn_v;
        m.rotation = rot;
        m.position = pos;
    }
    flash::update(w, id, pv::F);
}

/// The death in `0x305178` (module doc).
fn die(w: &mut World, id: MobyId) {
    w.play_sound(3, 0, id);
    set_death_bits(w, id, 0, -1);
    let (pos, rot) = (c::pos(w, id), w.m(id).rotation);
    let yaw = rot[2];
    fx::beam_explosion(w, &BLAST, Some(id), [pos[0], pos[1], pos[2] + 3.0, pos[3]]);
    let (cy, sy) = c::cs(yaw);
    fx::beam_explosion(w, &BLAST, Some(id), [pos[0] + cy + cy, pos[1] + sy + sy, pos[2], pos[3]]);
    let (cb, sb) = c::cs(c::add_rot(yaw, std::f32::consts::PI));
    fx::beam_explosion(w, &BLAST, Some(id), [pos[0] + cb + cb, pos[1] + sb + sb, pos[2], pos[3]]);
    let t = w.ticks(5);
    c::set_pi16(w, id, pv::DEATH_T, t as i16);
    w.mm(id).state = 3;
    for class in PIECES { fx::break_piece(w, id, class, pos, rot, 0, 0); }
}

/// Level05 `0x304320` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    // A death sets state 3 and returns from the hits early; the tick goes on to delete it below.
    hits(w, id);
    flame_hits(w, id);
    if c::pu8(w, id, pv::DESPAWN) == 2 {
        delete_all(w, id);
        return;
    }
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 27.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x15;
        }
    }
    match w.m(id).state {
        0 => init(w, id),
        1 => {
            drive(w, id);
            if sees(w, id, false) {
                c::blend_to(w, id, 0, 0, 3);
                w.mm(id).state = 2;
            }
            if c::dist2(c::pos(w, id), super::hero_pos(w)) < 16.0 {
                let h = heading_to_hero(w, id);
                aim(w, id, h, PITCH);
            } else {
                let y = c::yaw(w, id);
                aim(w, id, y, 0.0);
            }
        }
        2 => {
            drive(w, id);
            flames(w, id);
            if !sees(w, id, true) {
                c::blend_to(w, id, 0, 0, 3);
                w.mm(id).state = 1;
            }
            let h = heading_to_hero(w, id);
            aim(w, id, h, PITCH);
        }
        3 => delete_all(w, id),
        _ => {}
    }
}
