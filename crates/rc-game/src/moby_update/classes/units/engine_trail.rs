//! The engine-trail blob of the path flyers: one type-2 smoke blob (`PartType02Spawn` 0x27dc98) behind a moving moby,
//! the same code shape in two level-private copies with their own constants (level08 0x2dec90, the Batalia fighters
//! 438: `batalia_fighter`; level02 0x2eca30 (and its level09 copy), the path ships 1212: `path_ship`). A new consumer
//! passes its [`Blob`] row and the point the blob starts from.
//!
//! The blob (draws in this order): d = position − the position before this tick's move; v1 = d·k1 with w =
//! `randf(w1)`; v2 = d·k2 + `rand_vec(0, jitter·dt)` (0x26cb58) with w = `randf(w2)`; colour 1 = `FastTweenColor(randf(0,
//! 1), c1)`, colour 2 = `FastTweenColor(randf(0, 1), c2)`; phases `trunc(multiply_global_scale(t0·randf(0, 1) + 1))`,
//! `trunc(scale(t1·(randf(−s, s) + 1)))`, `trunc(scale(t2·(randf(−s, s) + 1)))`; `PartType02Spawn(point, v1, v2, colour
//! 1, colour 2, phases, −1)`, the record's byte 9 (its near / far fade) = `trunc(8) + byte9`.

use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::{pf as to_pf, World};

/// One copy's constants.
#[derive(Clone, Copy, Debug)]
pub struct Blob {
    pub k1: f32,
    pub k2: f32,
    pub jitter: f32,
    pub w1: (f32, f32),
    pub w2: (f32, f32),
    pub c1: (u32, u32),
    pub c2: (u32, u32),
    pub t: [i32; 3],
    pub spread: f32,
    /// Added to `trunc(8)` for the record's byte 9 (0x60 for the ships, −0x70 for the fighters).
    pub byte9: i32,
    /// The sizes (w) divided by it after their draws (1; 1.5 for the big path ships on level 10, level02 0x2ed658).
    pub size_div: f32,
}

/// `truncate_float_to_s32(multiply_global_scale(x))`.
fn gscale(w: &World, x: f32) -> i32 { w.svc.timing.scale(to_pf(x)).to_f32() as i32 }

fn tween(f: f32, c: (u32, u32)) -> u32 { crate::particles::tween_color(f.to_bits(), c.0, c.1) }

/// One blob of row `b` at `point` for the move `d` (module doc).
pub fn blob(w: &mut World, b: &Blob, point: [f32; 4], d: [f32; 4]) {
    let mut v1 = c::scale(d, b.k1);
    let mut v2 = c::scale(d, b.k2);
    let j = w.rng.rand_vec(0.0, b.jitter * DT);
    v2 = c::add(v2, [j[0], j[1], j[2], 0.0]);
    v1[3] = w.rng.randf(b.w1.0, b.w1.1);
    v2[3] = w.rng.randf(b.w2.0, b.w2.1);
    if b.size_div != 1.0 {
        v1[3] /= b.size_div;
        v2[3] /= b.size_div;
    }
    let c1 = tween(w.rng.randf(0.0, 1.0), b.c1);
    let c2 = tween(w.rng.randf(0.0, 1.0), b.c2);
    let r0 = w.rng.randf(0.0, 1.0);
    let t0 = gscale(w, b.t[0] as f32 * r0 + 1.0);
    let r1 = w.rng.randf(-b.spread, b.spread);
    let t1 = gscale(w, b.t[1] as f32 * (r1 + 1.0));
    let r2 = w.rng.randf(-b.spread, b.spread);
    let t2 = gscale(w, b.t[2] as f32 * (r2 + 1.0));
    let a = crate::particles::type02::Spawn { pos: point, v1, v2, c1, c2, t: [t0, t1, t2], def: -1 };
    *w.svc.fx.part_spawns.entry(2).or_default() += 1;
    match w.particles.as_deref_mut() {
        None => {
            w.rng.randf(0.0, 255.0);
        }
        Some(p) => match crate::particles::type02::spawn(p, w.rng, &a) {
            Some(i) => p.pool.recs[i][9] = (8i32 + b.byte9) as u8,
            None => w.svc.fx.part_failed += 1,
        },
    }
}
