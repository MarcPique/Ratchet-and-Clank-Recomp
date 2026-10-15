//! U120 (census 2026-10-02): class 1479, Aridia's fire vents (level02 0x2ef020, the only copy: 5 placed). Read from the
//! level02 decomp and disassembly. Native `f32`; the `rand` draws in the game's order.
//!
//! An emitter of type-2 flame particles along its row 0 (its facing): a steady stream (mode 1: one tick in four;
//! mode 2: every tick, and a quarter of the ticks while off screen) and, every 20–40 ticks, a burst of sparks (one,
//! or 5–9 one time in seven). Mode 2's stream alternates between long and short flames every 2–3 s. Off screen
//! (`FastBSphereCheck(120)`), mode 1 does nothing.
//!
//! **Pvars**: +0x00 byte the mode − 1 (state = it + 1 at the first update), +0xc0 s32 the burst timer, +0xc4 s32 mode 2's
//! flame-length timer, +0xc8 s32 mode 2's stream phase A (30 / 10).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0 | state = byte +0x00 + 1; its rows from its Euler (`0x1fa030`) | [`update`] (the rows `MobyBuildMatrix` keeps) |
//! | 1 | off screen → nothing; the burst; `randi(4)` ≠ 0 → no stream; the stream | [`update`] |
//! | 2 | the length timer out → phase A 10 when it was over 20, else 30; timer trunc(scale·`randf(120, 180)`); off screen and `randi(4)` ≠ 0 → nothing; the burst; the stream | [`update`] |
//! | 3 | `DeleteMoby` | [`update`] |
//! | **burst** | the timer (int) out: timer = trunc(scale·`randf(20, 40)`); n = `randi(7)` = 0 ? `randi(5)` + 5 : 1; each: r = `rand_vec(0, 4·dt)`; v1 = row 0 at k1·(1 + `randf(±j)`)·dt + r; v2 = v1 at k2·(1 + `randf(±j)`)·dt, z − 8·dt; sizes (w) `randf`; colours `FastTweenColor(randf(0, 1), …)` ×2; phases trunc(scale·(A·`randf(0, 1)` + 1)), trunc(scale·(B·(1 + `randf(±tj)`))), C likewise; `PartType02Spawn(pos, v1, v2, …, def 0x10019)` (0x26a730 = L01 0x27dc98) | [`burst`] (`fx::part02`) |
//! | **stream** | v1 = row 0 at k1·(1 + `randf(±j)`)·dt; v2 = (0, 0, k2·(1 + `randf(±j)`)·dt) + row 1 at `randf(±side)`·dt; sizes, colours, phases as the burst; def −1 (type 23's) | [`stream`] |
//!
//! The constants (gp−0x4bb8..−0x4a9c, level02): [`STREAM_1`], [`BURST_1`], [`STREAM_2`], [`BURST_2`].

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, fx, DT};
use crate::moby_update::services::World;
use crate::particles::type02;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 2;
pub const UPDATE_FN: u32 = 0x2e_f020;
pub const CLASSES: [i16; 1] = [1479];

const BURST_T: usize = 0xc0;
const LENGTH_T: usize = 0xc4;
const PHASE_A: usize = 0xc8;
const SIZE: usize = 0xcc;

/// One emission's constants.
pub struct Emit {
    /// v1's and v2's speeds (·dt) and their jitter.
    pub k1: f32,
    pub k2: f32,
    pub jitter: f32,
    /// The stream: row 1's sideways spread (·dt); the burst: v2's drop (·dt) and the random kick (·dt).
    pub side: f32,
    pub drop: f32,
    pub kick: f32,
    /// Phase lengths A, B, C (ticks) and the jitter of B and C.
    pub t: [i32; 3],
    pub tj: f32,
    /// Size ranges of v1 / v2 (their w).
    pub s1: (f32, f32),
    pub s2: (f32, f32),
    /// The colour pairs tweened.
    pub c1: (u32, u32),
    pub c2: (u32, u32),
    /// The burst's timer range and def word.
    pub timer: (i32, i32),
    pub def: i32,
}

/// Mode 1's stream (gp−0x4b78..).
pub const STREAM_1: Emit = Emit { k1: 4.0, k2: 2.0, jitter: 0.333, side: 0.5, drop: 0.0, kick: 0.0, t: [20, 40, 40], tj: 0.5, s1: (1.0, 1.5), s2: (1.5, 2.0), c1: (0x6000_ff80, 0x6020_ff40), c2: (0x2040_8040, 0x2020_6020), timer: (0, 0), def: -1 };
/// Mode 1's burst (gp−0x4b38..).
pub const BURST_1: Emit = Emit { k1: 16.0, k2: 4.0, jitter: 0.1, side: 0.0, drop: 8.0, kick: 4.0, t: [0, 40, 10], tj: 0.1, s1: (0.25, 0.5), s2: (0.25, 0.125), c1: (0x8040_80ff, 0x8040_ffff), c2: (0x8020_4040, 0x8020_4080), timer: (20, 40), def: 0x1_0019 };
/// Mode 2's stream (gp−0x4bb8..; phase A from +0xc8).
pub const STREAM_2: Emit = Emit { k1: 14.0, k2: 10.0, jitter: 0.333, side: 4.0, drop: 0.0, kick: 0.0, t: [30, 60, 180], tj: 0.5, s1: (1.0, 2.5), s2: (4.0, 6.0), c1: (0x4040_80ff, 0x4040_80c0), c2: (0x2080_4050, 0x2060_3038), timer: (0, 0), def: -1 };
/// Mode 2's burst (gp−0x4ae8..).
pub const BURST_2: Emit = Emit { k1: 16.0, k2: 10.0, jitter: 0.333, side: 0.0, drop: 8.0, kick: 4.0, t: [10, 40, 10], tj: 0.333, s1: (0.5, 1.0), s2: (0.25, 0.5), c1: (0x8040_80ff, 0x8040_ffff), c2: (0x8020_4040, 0x8020_4080), timer: (20, 40), def: 0x1_0019 };

fn scaled(w: &World, x: f32) -> i32 { crate::moby_update::services::fl(w.svc.timing.scale(Pf::f(x))) as i32 }

fn jit(w: &mut World, j: f32) -> f32 { w.rng.randf(-j, j) + 1.0 }

/// The sizes, colours and phases every particle draws (in the game's order), after its velocities.
fn finish(w: &mut World, e: &Emit, a: i32, v1: &mut c::V, v2: &mut c::V) -> (u32, u32, [i32; 3]) {
    v1[3] = w.rng.randf(e.s1.0, e.s1.1);
    v2[3] = w.rng.randf(e.s2.0, e.s2.1);
    let f = w.rng.randf(0.0, 1.0);
    let c1 = crate::hud::tween_color(f, e.c1.0, e.c1.1);
    let f = w.rng.randf(0.0, 1.0);
    let c2 = crate::hud::tween_color(f, e.c2.0, e.c2.1);
    let f = w.rng.randf(0.0, 1.0);
    let ta = scaled(w, a as f32 * f + 1.0);
    let f = jit(w, e.tj);
    let tb = scaled(w, e.t[1] as f32 * f);
    let f = jit(w, e.tj);
    let tc = scaled(w, e.t[2] as f32 * f);
    (c1, c2, [ta, tb, tc])
}

fn rows(w: &World, id: MobyId) -> (c::V, c::V) {
    let r = w.m(id).rows;
    ([r[0][0], r[0][1], r[0][2], 0.0], [r[1][0], r[1][1], r[1][2], 0.0])
}

/// The burst (module table).
fn burst(w: &mut World, id: MobyId, e: &Emit) {
    let t = c::pi32(w, id, BURST_T);
    let out = if t == 0 { true } else {
        c::set_pi32(w, id, BURST_T, t.max(1) - 1);
        t.max(1) - 1 < 1
    };
    if !out { return; }
    let r = w.rng.randf(e.timer.0 as f32, e.timer.1 as f32);
    let n = scaled(w, r);
    c::set_pi32(w, id, BURST_T, n);
    let count = if w.rng.randi(7) == 0 { w.rng.randi(5) + 5 } else { 1 };
    let (r0, _) = rows(w, id);
    for _ in 0..count {
        let k = w.rng.rand_vec(0.0, e.kick * DT);
        let s = e.k1 * jit(w, e.jitter) * DT;
        let mut v1 = c::add(c::set_len3(r0, s), [k[0], k[1], k[2], 0.0]);
        let s = e.k2 * jit(w, e.jitter) * DT;
        let mut v2 = c::set_len3(v1, s);
        v2[2] -= e.drop * DT;
        let (c1, c2, t) = finish(w, e, e.t[0], &mut v1, &mut v2);
        let pos = w.m(id).position;
        fx::part02(w, &type02::Spawn { pos, v1, v2, c1, c2, t, def: e.def });
    }
}

/// The stream (module table); phase A `a`.
fn stream(w: &mut World, id: MobyId, e: &Emit, a: i32) {
    let (r0, r1) = rows(w, id);
    let s = e.k1 * jit(w, e.jitter) * DT;
    let mut v1 = c::set_len3(r0, s);
    let s = e.k2 * jit(w, e.jitter) * DT;
    let side = w.rng.randf(-e.side, e.side) * DT;
    let mut v2 = c::add([0.0, 0.0, s, 0.0], c::set_len3(r1, side));
    let (c1, c2, t) = finish(w, e, a, &mut v1, &mut v2);
    let pos = w.m(id).position;
    fx::part02(w, &type02::Spawn { pos, v1, v2, c1, c2, t, def: e.def });
}

/// `FastBSphereCheck(120, pos)`: drawn last frame's view holds the vent (no view: not).
fn seen(w: &World, id: MobyId) -> bool {
    let p = w.m(id).position;
    w.view.is_some_and(|v| !v.culled(120.0, [p[0], p[1], p[2], p[3]]))
}

/// Level02 0x2ef020 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < SIZE { return; }
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = m.pvars[0].wrapping_add(1);
        }
        1 => {
            if !seen(w, id) { return; }
            burst(w, id, &BURST_1);
            if w.rng.randi(4) != 0 { return; }
            stream(w, id, &STREAM_1, STREAM_1.t[0]);
        }
        2 => {
            let t = c::pi32(w, id, LENGTH_T);
            let out = if t == 0 { true } else {
                c::set_pi32(w, id, LENGTH_T, t.max(1) - 1);
                t.max(1) - 1 < 1
            };
            if out {
                let a = if 20 < c::pi32(w, id, PHASE_A) { 10 } else { 30 };
                c::set_pi32(w, id, PHASE_A, a);
                let r = w.rng.randf(120.0, 180.0);
                let n = scaled(w, r);
                c::set_pi32(w, id, LENGTH_T, n);
            }
            if !seen(w, id) && w.rng.randi(4) != 0 { return; }
            burst(w, id, &BURST_2);
            let a = c::pi32(w, id, PHASE_A);
            stream(w, id, &STREAM_2, a);
        }
        3 => w.delete_moby(id),
        _ => {}
    }
}
