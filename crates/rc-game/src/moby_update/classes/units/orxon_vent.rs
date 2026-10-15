//! Orxon's particle vents, class 1544: level10 0x2ea1f0 (census U349 of the 2026-10-01 run; 14 created
//! instances), with its three emitters 0x2ea4e8 (puffs in bursts), 0x2eaa08 (drips timed to the ground) and 0x2ea7b0
//! (a steady column). One vent emits at its own position (+0x04 = −1) or at each point (at most 32) of spline
//! +0x04, one timer pair per point. Every particle is a type-2 blob (`PartType02Spawn`). Read from the level10
//! decomp and disassembly (0x2ea1f0..0x2eac9c; the constants are its `$gp` words 0x161f68..0x162050). Native `f32`;
//! the `rand` draws in the game's order on the shared stream.
//!
//! **Pvar block**: +0x00 the kind (0 puffs, 1 drips, 2 column), +0x04 the spline (−1: the vent itself), +0x08 s16[32]
//! the emit timers, +0x48 s16[32] the on / off cycles (kinds 0, 1), +0x88 the ground z (kind 1 on its own position).
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | kind 0 → 1; kind 1: `GroundHeight(0.5, p, 0)` (`0x24c188` = L01 0x26e618) of the vent → +0x88, or of every point → the point's w (written into the level's spline), → 2; kind 2 → 3; other kinds stay | [`update`] (`World::ground_height`) |
//! | states 1 / 2 / 3 | the emitter at the vent, or at each point of the spline (count clamped to 32) with its own timer pair | [`update`] |
//! | 0x2ea4e8 (kind 0) | cycle < 0: += 1, not 0 → nothing; 0 → cycle = trunc(randf(100, 300)); cycle ≥ 0: `FastDecTimer` done → cycle = −trunc(randf(100, 300)), nothing | [`puffs`] (`cycle`) |
//! | | \|camera − p\| > 32 → nothing; `FastBSphereCheck(32, (p, 2))` culled and `randi(3)` ≠ 0 → nothing; the emit timer running → nothing | [`puffs`] |
//! | | v1 = (0, 0, 0.5·dt, randf(0.5, 1)); v2 = `rand_vec(0, 0.333·dt)` + (0, 0, 0.333·dt), w `randf(1, 1.5)`; f = randf(0.9, 1.1); t = trunc((30, 60, 60)·f); c1 = tween(randf(0, 1), 0x4000ff80, 0x4010ffff), c2 = tween(randf(0, 1), 0x2000ff80, 0x200080ff); `PartType02Spawn(p, v1, v2, c1, c2, t, def −1)`; emit timer = trunc(randf(15, 20)) | [`puffs`] (`fx::part02`) |
//! | 0x2eaa08 (kind 1) | the cycle as kind 0 with on randf(20, 30), off randf(100, 300) | [`drips`] |
//! | | `FastBSphereCheck(32, (p − 4z, 3))` culled → nothing (no distance test, no `randi` fallback); the emit timer running → nothing | [`drips`] |
//! | | f = randf(0.5, 1.5); tA = trunc(30·f), tC = trunc(10·f); n = trunc(sqrt((g + g) / (10·dt²))·0.22·f) (g = the ground z as stored, sic); v1 = (0, 0, 0, randf(0.2, 0.1)); v2 = `rand_vec(0, dt)` − (0, 0, 10·dt²·n), w `randf(0.2, 0.1)`; c1 tween(0x80002040, 0x80104040), c2 tween(0x30002040, 0x30004020); `PartType02Spawn(p, v1, v2, c1, c2, (tA, n, tC), def 0x18)`; emit timer = trunc(randf(3, 6)) | [`drips`] |
//! | 0x2ea7b0 (kind 2) | no cycle; \|camera − p\| > 32 → nothing; `FastBSphereCheck(32, (p + 6z, 5))` culled and `randi(3)` ≠ 0 → nothing; the emit timer | [`column`] |
//! | | v1 = (0, 0, 3·dt, randf(2.5, 3.5)); v2 = `rand_vec(0, 0.5·dt)` + (0, 0, 0·dt), w `randf(5, 6)`; f = randf(0.9, 1.1); t = trunc(120·f) ×3; c1 tween(0x400080ff, 0x4010ffff), c2 tween(0x2000c0ff, 0x200080ff); def −1; emit timer = trunc(randf(50, 60)) | [`column`] |
//! | | no sound, light, hit, save flag, other moby | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx::{in_view, part02};
use crate::moby_update::creature::{self as c, pi16, pi32, set_pf, set_pi16, DT};
use crate::moby_update::services::{fast_dec_timer_s16, World};
use crate::particles::type02::Spawn;
use crate::ps2v::Pf;

/// The update in the level10 class table.
pub const UPDATE_FN: u32 = 0x2e_a1f0;
pub const REFERENCE_LEVEL: u32 = 10;
pub const CLASSES: [i16; 1] = [1544];
/// At most this many spline points emit.
pub const MAX_POINTS: usize = 32;
/// The pvar offsets: the emit timers and the cycles (s16 each), the ground z.
pub const TIMERS: usize = 0x08;
pub const CYCLES: usize = 0x48;
pub const GROUND: usize = 0x88;
const SIZE: usize = 0x8c;

fn dt2() -> f32 { DT * DT }

fn tween(w: &mut World, a: u32, b: u32) -> u32 { let t = w.rng.randf(0.0, 1.0); crate::hud::tween_color(t, a, b) }

fn randt(w: &mut World, a: i32, b: i32) -> i32 { w.rng.randf(a as f32, b as f32) as i32 }

fn dec(w: &mut World, id: MobyId, o: usize) -> bool {
    let mut t = pi16(w, id, o);
    let r = fast_dec_timer_s16(&mut t);
    set_pi16(w, id, o, t);
    r != 0
}

/// The on / off cycle of kinds 0 and 1 (`on`, `off`: the `randf` ranges): true when the emitter runs this tick.
fn cycle(w: &mut World, id: MobyId, o: usize, on: (i32, i32), off: (i32, i32)) -> bool {
    let c0 = pi16(w, id, o);
    if c0 < 0 {
        let n = c0.wrapping_add(1);
        set_pi16(w, id, o, n);
        if n != 0 { return false; }
        let t = randt(w, on.0, on.1);
        set_pi16(w, id, o, t as i16);
        true
    } else if dec(w, id, o) {
        let t = randt(w, off.0, off.1);
        set_pi16(w, id, o, (-t) as i16);
        false
    } else {
        true
    }
}

fn camera_near(w: &World, p: [f32; 4]) -> bool {
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    let d = c::dist3(cam, p);
    d.is_nan() || d <= 32.0
}

/// 0x2ea4e8 (module doc): `k` = the timer pair's index.
pub fn puffs(w: &mut World, id: MobyId, p: [f32; 4], k: usize) {
    if !cycle(w, id, CYCLES + 2 * k, (100, 300), (100, 300)) { return; }
    if !camera_near(w, p) { return; }
    if !in_view(w, 32.0, p, 2.0) && w.rng.randi(3) != 0 { return; }
    if !dec(w, id, TIMERS + 2 * k) { return; }
    let mut v1 = [0.0, 0.0, 0.5 * DT, 0.0];
    let r = w.rng.rand_vec(0.0, f32::from_bits(0x3eaa_7efa) * DT);
    let mut v2 = [r[0], r[1], r[2] + f32::from_bits(0x3eaa_7efa) * DT, 0.0];
    v1[3] = w.rng.randf(0.5, 1.0);
    v2[3] = w.rng.randf(1.0, 1.5);
    let f = w.rng.randf(1.0 - 0.1, 0.1 + 1.0);
    let t = [(30.0 * f) as i32, (60.0 * f) as i32, (60.0 * f) as i32];
    let c1 = tween(w, 0x4000_ff80, 0x4010_ffff);
    let c2 = tween(w, 0x2000_ff80, 0x2000_80ff);
    part02(w, &Spawn { pos: p, v1, v2, c1, c2, t, def: -1 });
    let e = randt(w, 15, 20);
    set_pi16(w, id, TIMERS + 2 * k, e as i16);
}

/// 0x2eaa08 (module doc): `g` = the stored ground z.
pub fn drips(w: &mut World, id: MobyId, g: f32, p: [f32; 4], k: usize) {
    if !cycle(w, id, CYCLES + 2 * k, (20, 30), (100, 300)) { return; }
    if !in_view(w, 32.0, [p[0], p[1], p[2] - 4.0, p[3]], 3.0) { return; }
    if !dec(w, id, TIMERS + 2 * k) { return; }
    let f = w.rng.randf(1.0 - 0.5, 0.5 + 1.0);
    let ta = (30.0 * f) as i32;
    let tc = (10.0 * f) as i32;
    let n = (((g + g) / (10.0 * dt2())).sqrt() * 0.22 * f) as i32;
    let mut v1 = [0.0; 4];
    let r = w.rng.rand_vec(0.0, 1.0 * DT);
    let mut v2 = [r[0], r[1], r[2] - 10.0 * dt2() * n as f32, 0.0];
    v1[3] = w.rng.randf(0.2, 0.1);
    v2[3] = w.rng.randf(0.2, 0.1);
    let c1 = tween(w, 0x8000_2040, 0x8010_4040);
    let c2 = tween(w, 0x3000_2040, 0x3000_4020);
    part02(w, &Spawn { pos: p, v1, v2, c1, c2, t: [ta, n, tc], def: 0x18 });
    let e = randt(w, 3, 6);
    set_pi16(w, id, TIMERS + 2 * k, e as i16);
}

/// 0x2ea7b0 (module doc).
pub fn column(w: &mut World, id: MobyId, p: [f32; 4], k: usize) {
    if !camera_near(w, p) { return; }
    if !in_view(w, 32.0, [p[0], p[1], p[2] + 6.0, p[3]], 5.0) && w.rng.randi(3) != 0 { return; }
    if !dec(w, id, TIMERS + 2 * k) { return; }
    let mut v1 = [0.0, 0.0, 3.0 * DT, 0.0];
    let r = w.rng.rand_vec(0.0, 0.5 * DT);
    let mut v2 = [r[0], r[1], r[2] + 0.0 * DT, 0.0];
    v1[3] = w.rng.randf(2.5, 3.5);
    v2[3] = w.rng.randf(5.0, 6.0);
    let f = w.rng.randf(1.0 - 0.1, 0.1 + 1.0);
    let t = [(120.0 * f) as i32; 3];
    let c1 = tween(w, 0x4000_80ff, 0x4010_ffff);
    let c2 = tween(w, 0x2000_c0ff, 0x2000_80ff);
    part02(w, &Spawn { pos: p, v1, v2, c1, c2, t, def: -1 });
    let e = randt(w, 50, 60);
    set_pi16(w, id, TIMERS + 2 * k, e as i16);
}

/// The emit points: the vent (None) or its spline's first 32 points (with their w).
fn points(w: &World, id: MobyId) -> Vec<[f32; 4]> {
    let s = pi32(w, id, 4);
    if s == -1 { return vec![w.m(id).position]; }
    w.svc.splines.get(s as usize).map(|pts| pts.iter().take(MAX_POINTS).map(|q| q.map(f32::from_bits)).collect()).unwrap_or_default()
}

/// Level10 0x2ea1f0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < SIZE { return; }
    let s = pi32(w, id, 4);
    match w.m(id).state {
        0 => match pi32(w, id, 0) {
            0 => w.mm(id).state = 1,
            1 => {
                if s == -1 {
                    let p = w.m(id).position.map(Pf::f);
                    let g = w.ground_height(Pf::f(0.5), p, 0).to_f32();
                    set_pf(w, id, GROUND, g);
                } else if let Some(n) = w.svc.splines.get(s as usize).map(|v| v.len().min(MAX_POINTS)) {
                    for k in 0..n {
                        let p = w.svc.splines[s as usize][k].map(Pf::b);
                        let g = w.ground_height(Pf::f(0.5), p, 0);
                        w.svc.splines[s as usize][k][3] = g.0;
                    }
                }
                w.mm(id).state = 2;
            }
            2 => w.mm(id).state = 3,
            _ => {}
        },
        st @ 1..=3 => {
            let pts = points(w, id);
            for (k, p) in pts.into_iter().enumerate() {
                match st {
                    1 => puffs(w, id, p, k),
                    2 => {
                        let g = if s == -1 { c::pf(w, id, GROUND) } else { p[3] };
                        drips(w, id, g, p, k);
                    }
                    _ => column(w, id, p, k),
                }
            }
        }
        _ => {}
    }
}
