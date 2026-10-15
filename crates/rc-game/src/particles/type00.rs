//! Particle type 0, the weather's rain streak (level01 update `PartType00Update` 0x27d928, the same code on all 19
//! levels, `overlay-diff`; spawner level08 `0x272cb8`, one cluster on levels 08, 12, 14 (`clusters.tsv` b376dbd2…);
//! read from the decomp). The weather emitter class 1400 (`0x307cf0` on level 08: the census's U399 / U293, not
//! ported) calls the spawner and writes the weather globals the update reads (G-PRT-001 consumer).
//!
//! **Spawn** `(floor f12, pos a0, kind a1, vel a2)` (level08 `0x272cb8`): kind 1 → alpha 0xd, half-width 0.5, length
//! `randf(1, 5)`; kind 0 → alpha 0x14, 0.015, `randf(0.01, 1)`; kind 2 → alpha 0xd, 1.0, `randf(20, 30)` and texture
//! 1 (other kinds: 0, 0, no draw). Timer +0x0a = the kind; RGBA1 `(alpha >> 2) << 24 | 0x606060`, RGBA2 (+0x0c)
//! `alpha << 24 | 0x907070`; byte9 `trunc(4) + 0x60` = 0x64, **render kind 3** (the textured ribbon), ALPHA 0x44,
//! texture `def[0][k]` (level08 0x1b2880 = its def table); end 1 = pos, end 2 (+0x20) = pos + unit(vel)·length,
//! +0x1c = the half-width, +0x2c = 1, velocity +0x30 = vel, +0x3c = floor.
//!
//! **Update**: the floor +0x3c above end 1, or end 1 outside [0, 512] in x or y → killed. Else d = vel +
//! the weather's camera step (0x160250: the camera's move this tick, written by 1400); end 2 += d, end 1 += d (xyz: the
//! w lanes hold the half-width +0x1c and the width factor +0x2c). Unless
//! the kind is 2, when end 2 crossed into another height-grid cell: floor = the weather floor (0x160260) or, when that
//! is not above the grid's height at end 2, that height (`0x278020`); end 2 below it → killed. vel.z −= 9.8·dt²; +0x3c
//! = floor (the old one when no new cell) + the camera step's z. No RNG.
//!
//! The weather state lives in [`Particles::weather`], the height grid in [`Particles::grid`]. Native `f32`.

use super::{rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 0;

/// Level08 `0x272cb8(floor, pos, kind, vel)`: one draw for kinds 0–2 with a record; none when the pool is full.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, floor: f32, pos: [f32; 4], kind: i32, vel: [f32; 4]) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let (mut w, mut a, mut len, mut tex) = (0.0f32, 0i32, 0.0f32, kind);
    match kind {
        1 => (w, a, len) = (0.5, 0xd, rng.randf(1.0, 5.0)),
        0 => (w, a, len) = (f32::from_bits(0x3c75_c28f), 0x14, rng.randf(f32::from_bits(0x3c23_d70a), 1.0)),
        2 => (w, a, len, tex) = (1.0, 0xd, rng.randf(20.0, 30.0), 1),
        _ => {}
    }
    let n = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
    let d = if n > 0.0 { [vel[0] * len / n, vel[1] * len / n, vel[2] * len / n] } else { [0.0; 3] };
    let def = sys.def_frame(TYPE, tex.max(0) as usize);
    let r = &mut sys.pool.recs[i];
    rec::set_i16(r, 10, kind as i16);
    rec::set_u32(r, 4, ((a >> 2) as u32) << 24 | 0x60_6060);
    rec::set_u32(r, 0xc, (a as u32) << 24 | 0x90_7070);
    r[9] = 4 + 0x60;
    r[1] = 3;
    r[3] = 0x44;
    r[2] = def;
    rec::set_v4(r, 0x10, pos);
    rec::set_v4(r, 0x20, [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2], pos[3]]);
    rec::set_ff(r, 0x1c, w);
    rec::set_ff(r, 0x2c, 1.0);
    rec::set_v4(r, 0x30, vel);
    rec::set_ff(r, 0x3c, floor);
    Some(i)
}

/// Update 0x27d928 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let (wind, wfloor, dt2) = (sys.weather.wind, sys.weather.floor, f32::from_bits(sys.time.dt2));
    let grid = sys.grid.clone();
    let r = &mut sys.pool.recs[i];
    let p = rec::v3(r, 0x10);
    let mut floor = rec::ff(r, 0x3c);
    if p[2] < floor || p[0] < 0.0 || p[1] < 0.0 || 512.0 < p[0] || 512.0 < p[1] {
        sys.kill_part(i);
        return;
    }
    let (ox, oy) = (rec::ff(r, 0x20), rec::ff(r, 0x24));
    let v = rec::v4(r, 0x30);
    // xyz only: the ends' w lanes are the half-width (+0x1c) and the width factor (+0x2c), the velocity's the floor.
    let d: [f32; 3] = std::array::from_fn(|k| v[k] + wind[k]);
    for o in [0x20, 0x10] {
        let q = rec::v3(r, o);
        rec::set_v3(r, o, std::array::from_fn(|k| q[k] + d[k]));
    }
    if rec::i16(r, 10) != 2 {
        let e2 = rec::v3(r, 0x20);
        if ox as i32 != e2[0] as i32 || oy as i32 != e2[1] as i32 {
            let h = grid.as_deref().and_then(|g| g.height(e2[0], e2[1])).unwrap_or(0.0);
            floor = if wfloor <= h { h } else { wfloor };
            if e2[2] < floor {
                sys.kill_part(i);
                return;
            }
        }
    }
    rec::set_ff(r, 0x38, rec::ff(r, 0x38) - dt2 * 9.8);
    rec::set_ff(r, 0x3c, floor + wind[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particles::Weather;
    use rc_formats::level::HeightGrid;

    /// The three kinds' records and draws; a streak falls with the camera step, speeds up under gravity, and dies
    /// when its tail crosses into a cell whose height is above it (or below the floor).
    #[test]
    fn streak_falls_and_dies_on_the_ground() {
        let mut s = Particles::new(None, Vec::new());
        s.grid = Some(std::sync::Arc::new(HeightGrid { width: 512, rows: 512, low: 0.0, high: 10.0, cells: vec![255; 512 * 512] }));
        s.weather = Weather { wind: [0.0, 0.0, 0.0, 0.0], floor: -100.0 };
        let mut rng = Rng::new();
        let mut t = rng;
        let i = spawn(&mut s, &mut rng, -100.0, [10.5, 10.5, 5.0, 1.0], 1, [1.0, 0.0, -1.0, 0.0]).unwrap();
        let len = t.randf(1.0, 5.0);
        assert_eq!(rng, t);
        let r = &s.pool.recs[i];
        assert_eq!((r[1], r[3], r[9], rec::u32(r, 4), rec::u32(r, 0xc), rec::ff(r, 0x1c)), (3, 0x44, 0x64, 0x0360_6060, 0x0d90_7070, 0.5));
        let e2 = rec::v3(r, 0x20);
        assert!((e2[0] - (10.5 + len / 2f32.sqrt())).abs() < 1e-5 && (e2[2] - (5.0 - len / 2f32.sqrt())).abs() < 1e-5);
        let mut n = 0;
        while s.pool.count > 0 && n < 100 {
            s.update_parts(&mut rng);
            n += 1;
        }
        // Cell height 0 (value 255 → low): the tail at z ≈ 5 − len/√2 − n drops below it after a few cells.
        let z0 = 5.0 - len / 2f32.sqrt();
        assert!(n as f32 >= z0.floor() && (n as f32) <= z0 + 2.0, "died after {n} (tail start {z0})");
        assert_eq!(rng, t, "no draws in the update");
        for (k, a) in [(0, 0x14u32), (2, 0xd)] {
            let j = spawn(&mut s, &mut rng, 0.0, [1.0; 4], k, [0.0, 0.0, -1.0, 0.0]).unwrap();
            assert_eq!(rec::u32(&s.pool.recs[j], 0xc) >> 24, a);
        }
    }
}
