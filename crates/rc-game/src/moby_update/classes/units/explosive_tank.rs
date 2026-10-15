//! Gemlik's explosive tanks, class 1261 (level 13, 55 instances): level13 0x307b10 with its private burst 0x3073c8,
//! scorch 0x3072c0 and fireball 0x30c138 (census unit U417). A tank takes hits through the creature hit resolver,
//! flashes and rocks when hit, and at zero health explodes: a green Bomb-Glove-style burst, a growing damage sphere
//! for 20 ticks, a scorch mark. Read from the level13 decomp and disassembly; the tuning words are the level's `$gp`
//! (gp 0x166c00, cited by address). The name is descriptive [L]. Native `f32`.
//!
//! **Pvar block** (P, bytes): +0x20 the creature damage record `D` (health at +0x20, the classes' hit cooldown +0x26,
//! +0x29 cleared every tick; `creature::damage`), +0x60 the hit flash record `F` (`creature::flash`; its colour red
//! byte +0x67), +0x70 i32 the explosion's tick.
//!
//! * **0**: +0x29 = 0; the rock (rotation x / y) ·0.7; `MobyGetHitMessage(0x330001)` (the game prints it) →
//!   `0x26f378(moby, hit, D, 0, &out5, 0, 0, col 4)`; reaction 1 / 2: health 0. With `out5 > 1` and the record's damage
//!   `d`: health ≤ d destroys it (health 0, off the target list (mode 0x1000), `SetDeathBits(0, −1)`, flash red byte
//!   0x78 and `0x272318`, the burst, a scorch, class sound 0, → **1**); else health −= d, flash byte 0xfa and
//!   `0x272318`, D+6 = `ticks(60)`, a rock `randf(±2°)` on x and y. +0xa4 = 0xff; `0x2723f8` (the flash).
//! * **1** exploding (t = +0x70): the sphere around pos + (0, 0, 1) of radius `0.5 + 4·t/ticks(20)` hits every moby
//!   in it (`coll_sphere_mobys` 0x10 + `0x26f8f8`: damage 4, push 1 / 1 up, flags 0x830001, type 2 / 1); past half
//!   (`scale(10) < t`) it is hidden (mode 0x41) and leaves a scorch each tick, else alpha ·7/8; after `ticks(20)`: a
//!   scorch, deleted.
//!
//! **Burst** (0x3073c8; the Bomb Glove's dry explosion, `classes::bomb`, at k = 1.5 with this level's colours, no
//! sound / shake / light): at pos + (0, 0, 1), base velocity (0, 0, 3·dt), up normal: 10 low fireballs (`randf(3.5,
//! 6.5)`·1.5·dt, `rand_range(ticks(60), ticks(120))`), 4 high (`randf(6.5, 10)`, `(ticks(60), ticks(90))`), one toward
//! the camera 0x1670c0 (`World::camera`), the smoke rings (type 11, 1..4 by the camera distance, 6e5, the level's
//! green colour tables 0x1f5560 / 0x1f5578), the flashes 1192 (6 ×2 beyond 9, 6, 5.25, 4.5; green). Ported as the
//! shared `bomb::blast` with [`TANK_BLAST`] (docs/plan/explosions.md §B).
//! **Fireball** (0x30c138, `bomb::spawn_fireball` with [`TANK_FIREBALL`]): `CreateMoby(1634)`, drawn, draw / update distance 0xff, alpha `rand_range(0x40, 0x80)`,
//! ambient (0x20, 0x60, 0x10) (`0x2650d0`), velocity, spins `randf(2π, 4π)·dt` ×2, life, the low ones' scale
//! ·`randf(0.5, 0.75)`, +0x1c = scale, +0xbc = type, scale ·`randf(2, 5)`, `MobyBuildMatrix`.
//! **Scorch** (0x3072c0): `PartType52Spawn(randf(0, 1.5), randf(1.5, 5.7), pos + r·(cos a, sin a) + (0, 0, 0.025),
//! 0x38002028, 0x2020, rand_range(100, 180))`, a = `rand_angle`, r = `randf(0, 0.8)`.

#![allow(clippy::needless_range_loop)] // lane loops spelled out.

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::bomb;
use crate::moby_update::creature::{self as c, attack, damage, flash, fx, DT};
use crate::moby_update::services::{pf, pv, World};

pub const UPDATE_FN: u32 = 0x30_7b10;
pub const REFERENCE_LEVEL: u32 = 13;
pub const TANK: i16 = 1261;
pub const CLASSES: [i16; 1] = [TANK];
/// The burst's fireball class (0x662).
pub const FIREBALL: i16 = 1634;
/// Its update in the level13 table.
pub const FIREBALL_FN: u32 = 0x30_c3b8;
pub const FIREBALL_CLASSES: [i16; 1] = [FIREBALL];

pub mod pv_ {
    pub const D: usize = 0x20;
    pub const HEALTH: usize = 0x20;
    pub const COOLDOWN: usize = 0x26;
    pub const D9: usize = 0x29;
    pub const F: usize = 0x60;
    pub const F_RED: usize = 0x67;
    pub const T: usize = 0x70;
    pub const LEN: usize = 0x74;
}
use pv_ as o;

/// The level's words (`$gp`, level13 0x161e2c..0x161e54).
mod k {
    pub const TICKS: i32 = 20; // 0x161e2c
    pub const R: (f32, f32) = (0.5, 4.5); // 0x161e30, 0x161e34
    pub const DAMAGE: f32 = 4.0; // 0x161e38
    pub const SCORCH_C: (u32, u32) = (0x3800_2028, 0x2020); // 0x161e3c, 0x161e40
    pub const SCORCH_S: (f32, f32) = (1.5, 5.7); // 0x161e44, 0x161e48
    pub const SCORCH_LIFE: (i32, i32) = (100, 180); // 0x161e4c, 0x161e50
    pub const SCORCH_R: f32 = 0.8; // 0x161e54
    /// The rings' colours (0x1f5560, 0x1f5578).
    pub const RING_C1: [u32; 6] = [0x4f00_ff8f, 0x4f00_ff8f, 0x4f00_ff7f, 0x4f00_ff6f, 0x2fff_ffff, 0x2fff_ffff];
    pub const RING_C2: [u32; 6] = [0x2f00_7f5f, 0x2f00_7f4f, 0x2f00_7f3f, 0x2f00_4f00, 0x2f00_0000, 0x3f00_0000];
}
/// 2° (0x3d0efa35).
const ROCK: f32 = f32::from_bits(0x3d0e_fa35);

/// Level13 0x307b10 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < o::LEN { return; }
    match w.m(id).state {
        0 => idle(w, id),
        1 => exploding(w, id),
        _ => {}
    }
}

fn idle(w: &mut World, id: MobyId) {
    c::set_pu8(w, id, o::D9, 0);
    let m = w.mm(id);
    m.rotation[0] *= 0.7;
    m.rotation[1] *= 0.7;
    let hit = w.get_hit(id, 0x33_0001, false);
    let r = damage::resolve(w, id, hit, o::D, 0, 4);
    if r.reaction == 1 || r.reaction == 2 { c::set_pf(w, id, o::HEALTH, 0.0); }
    if 1 < r.out5 {
        let d = r.hit.or(hit).map(|h| crate::moby_update::services::fl(h.damage)).unwrap_or(0.0);
        let health = c::pf(w, id, o::HEALTH);
        if health <= d {
            c::set_pf(w, id, o::HEALTH, 0.0);
            w.mm(id).mode &= !0x1000;
            crate::moby_update::classes::crate_::set_death_bits(w, id, 0, -1);
            c::set_pu8(w, id, o::F_RED, 0x78);
            flash::start(w, id, o::F);
            c::set_pi32(w, id, o::T, 0);
            burst(w, id);
            scorch(w, id, &TANK_SCORCH);
            w.play_sound(0, 0, id);
            w.mm(id).state = 1;
        } else {
            c::set_pf(w, id, o::HEALTH, health - d);
            c::set_pu8(w, id, o::F_RED, 0xfa);
            let t = w.ticks(60);
            c::set_pi16(w, id, o::COOLDOWN, t as i16);
            flash::start(w, id, o::F);
            let x = w.rng.randf(-ROCK, ROCK);
            w.mm(id).rotation[0] = x;
            let y = w.rng.randf(-ROCK, ROCK);
            w.mm(id).rotation[1] = y;
        }
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, o::F);
}

fn exploding(w: &mut World, id: MobyId) {
    let mut centre = c::pos(w, id);
    centre[2] += 1.0;
    let t = c::pi32(w, id, o::T);
    let span = w.ticks(k::TICKS);
    let r = k::R.0 + (k::R.1 - k::R.0) * (t as f32 / span as f32);
    attack::area_hit(w, r, centre, id, k::DAMAGE, 1.0, 1.0, None, 0x83_0001, 2, 1);
    let half = crate::moby_update::services::fl(w.svc.timing.scale(pf(k::TICKS as f32 * 0.5)));
    if half < t as f32 {
        w.mm(id).mode |= 0x41;
        scorch(w, id, &TANK_SCORCH);
    } else {
        let m = w.mm(id);
        m.alpha = ((m.alpha as u32 * 7) >> 3) as u8;
    }
    c::set_pi32(w, id, o::T, t + 1);
    if w.ticks(k::TICKS) < t + 1 {
        scorch(w, id, &TANK_SCORCH);
        w.delete_moby(id);
    }
}

/// A scorch mark's words: colours, sizes, life range, radius (cluster 98f51a1cea64: level13 0x3072c0 / 0x30c2b0,
/// level10 0x2e3de8 (the Orxon brawler 1202's death, `orxon_brawler`), level12 0x2e3420; each copy reads its own `$gp`
/// words).
pub(crate) struct Scorch {
    pub(crate) c: (u32, u32),
    pub(crate) s: (f32, f32),
    pub(crate) life: (i32, i32),
    pub(crate) r: f32,
}

/// The tank's (0x3072c0: `$gp` 0x161e3c..0x161e54) and the fireball's (0x30c2b0: 0x161f6c..0x161f84): one code
/// shape, two sets of words.
const TANK_SCORCH: Scorch = Scorch { c: k::SCORCH_C, s: k::SCORCH_S, life: k::SCORCH_LIFE, r: k::SCORCH_R };
const BALL_SCORCH: Scorch = Scorch { c: (0x6800_2028, 0x2020), s: (1.5, 5.7), life: (100, 180), r: 0.8 };

/// Level13 0x3072c0 / 0x30c2b0 (module doc).
pub(crate) fn scorch(w: &mut World, id: MobyId, q: &Scorch) {
    let a = w.rng.rand_angle();
    let r = w.rng.randf(0.0, q.r);
    let s1 = w.rng.randf(0.0, q.s.0);
    let s2 = w.rng.randf(q.s.0, q.s.1);
    let life = w.rng.rand_range(q.life.0, q.life.1);
    let mut p = c::add([a.cos() * r, a.sin() * r, 0.0, 0.0], c::pos(w, id));
    p[2] += 0.025;
    fx::part52(w, s1, s2, p, q.c.0, q.c.1, life);
}

/// Level13 0x30c138: the Bomb Glove's fireball spawner `0x2c4c20` compiled with these constants (module doc;
/// `bomb::spawn_fireball`).
pub const TANK_FIREBALL: bomb::FireballRow = bomb::FireballRow { class: FIREBALL, alpha: Some((0x40, 0x80)), ambient: [0x20, 0x60, 0x10], grow: Some((2.0, 5.0)), gold_bits: false };

/// Level13 0x3073c8: the Bomb Glove's blast code at k = 1.5 with the level's green tables (0x1f5560 / 0x1f5578, the
/// bomb's with red and green swapped) and green flashes (`bomb::blast`).
pub const TANK_BLAST: bomb::BlastRow = bomb::BlastRow {
    fireball: TANK_FIREBALL,
    ring_c1: k::RING_C1,
    ring_c2: k::RING_C2,
    flashes: [
        bomb::BlastFlash { size: 4.0, t: 15, rgba: [0x7f, 0x7f, 0x7f, 0x20] },
        bomb::BlastFlash { size: 4.0, t: 24, rgba: [0x20, 0x7f, 0, 0x20] },
        bomb::BlastFlash { size: 4.0, t: 20, rgba: [0x3f, 0x7f, 0, 0x30] },
        bomb::BlastFlash { size: 3.5, t: 27, rgba: [0x10, 0x60, 0, 0x40] },
        bomb::BlastFlash { size: 3.0, t: 29, rgba: [0, 0x20, 0, 0x20] },
    ],
};

/// Level13 0x3073c8 (module doc): at pos + (0, 0, 1), base `(0, 0, 2·dt)·1.5`, the up normal.
fn burst(w: &mut World, id: MobyId) {
    const K: f32 = 1.5;
    let base: c::V = [0.0, 0.0, (DT + DT) * K, 0.0];
    let up: c::V = [0.0, 0.0, 1.0, 0.0];
    let mut pos = c::pos(w, id);
    pos[2] += 1.0;
    bomb::blast(w, &TANK_BLAST, id, pos, base, up, K, 1, true, 0);
}

/// The fireball's trail words (`$gp` level13 0x161f08..0x161f68): per pass (jitter, colours, v1 z and its spread,
/// v2 speed, v2 z and its spread, heading spread (°), v2 size, phase lengths A / B / C and their spreads).
struct Trail {
    jitter: f32,
    c: (u32, u32),
    v1z: (f32, f32, f32),
    v2: f32,
    v2z: (f32, f32, f32),
    spread: f32,
    s2: f32,
    t: [f32; 3],
    tk: f32,
    extra_draw: bool,
}

const TRAILS: [Trail; 2] = [
    Trail { jitter: 0.065, c: (0x3010_8010, 0x1010_8010), v1z: (2.0, 0.9, 1.1), v2: 2.0, v2z: (2.0, 0.9, 1.1), spread: 90.0, s2: 1.5, t: [0.0, 15.0, 40.0], tk: 1.2, extra_draw: false },
    Trail { jitter: 0.1, c: (0x8010_ff10, 0x8010_ff10), v1z: (-1.0, 0.7, 1.5), v2: 3.0, v2z: (2.0, 0.7, 1.5), spread: 90.0, s2: 0.1, t: [0.0, 15.0, 40.0], tk: 1.2, extra_draw: true },
];

/// Level13 0x30c3b8: the burst's fireball 1634 (module doc; the Bomb Glove fireball's code with the high ones'
/// trail and scorch). The high ones (+0xbc = 1) leave two passes of two type-2 blobs a tick (`TRAILS`), and a
/// `CollLine_Fix(old, new, 2, Ratchet)` hit leaves a scorch (0x30c2b0) and deletes them. Moves, spins, falls
/// 14.6·dt², shrinks over the last quarter of its life, deleted when it runs out or leaves the positive octant.
pub fn fireball_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x20 { w.mm(id).pvars.resize(0x80, 0); }
    let old = c::pos(w, id);
    if w.m(id).cmd == 1 {
        let vel = c::pv4(w, id, 0);
        for t in &TRAILS {
            for _ in 0..2 {
                let mut p = old;
                for k in 0..3 { p[k] += w.rng.randf(-t.jitter, t.jitter); }
                if t.extra_draw { w.rng.randf(-30.0, 30.0); }
                let v1z = w.rng.randf(t.v1z.0 * DT * t.v1z.1, t.v1z.0 * DT * t.v1z.2);
                let a = w.rng.randf(-t.spread, t.spread) * 0.017_453_292;
                let h = c::add_rot(c::atan(vel[0], vel[1]), a);
                let v2 = [h.cos() * t.v2 * DT, h.sin() * t.v2 * DT, w.rng.randf(t.v2z.0 * DT * t.v2z.1, t.v2z.0 * DT * t.v2z.2), t.s2];
                let sc = w.svc.timing.timer_scale.to_f32();
                let ta = (w.rng.randf(t.t[0], t.t[0] + t.t[0]) * sc) as i32;
                let tb = (w.rng.randf(t.t[1], t.t[1] * t.tk) * sc) as i32;
                let tc = (w.rng.randf(t.t[2], t.t[2] * t.tk) * sc) as i32;
                let a = crate::particles::type02::Spawn { pos: p, v1: [0.0, 0.0, v1z, 0.0], v2, c1: t.c.0, c2: t.c.1, t: [ta, tb, tc], def: -1 };
                fx::part02(w, &a);
            }
        }
    }
    let (sa, sb) = (c::pf(w, id, 0x10), c::pf(w, id, 0x14));
    let mut v = c::pv4(w, id, 0);
    let m = w.mm(id);
    m.rotation[0] = c::add_rot(m.rotation[0], sa);
    m.rotation[1] = c::add_rot(m.rotation[1], sb);
    for k in 0..3 { m.position[k] += v[k]; }
    v[2] -= crate::moby_update::creature::DT2 * 14.6;
    c::set_pv4(w, id, 0, v);
    let p = c::pos(w, id);
    if p[0] < 0.0 || p[1] < 0.0 || p[2] < 0.0 {
        w.delete_moby(id);
        return;
    }
    let life0 = c::pi16(w, id, 0x1a) as i32;
    let q = if life0 < 0 { (life0 + 3) >> 2 } else { life0 >> 2 };
    let timer = c::pi16(w, id, 0x18);
    if (timer as i32) < q {
        let s = c::pf(w, id, 0x1c) * timer as f32 / q as f32;
        w.mm(id).scale = s;
    }
    if c::dec_timer_pvar_s16(w, id, 0x18) != 0 {
        w.delete_moby(id);
        return;
    }
    if w.m(id).cmd != 1 { return; }
    let hero = w.hero_moby;
    if w.coll_line(pv(old), pv(p), 2, hero).is_some() {
        scorch(w, id, &BALL_SCORCH);
        w.delete_moby(id);
    }
}
