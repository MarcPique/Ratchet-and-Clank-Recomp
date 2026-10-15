//! Particle type 73, the weather's falling flake / drop sprite (level01 update `PartType73Update` 0x28a5b8, the same
//! code on all 19 levels (level 09 differs only in a callee), `overlay-diff`; spawner level08 `0x27f5f0`, also on 12
//! and 14; read from the decomp). The weather emitter class 1400 calls the spawner (G-PRT-001 consumer, not ported).
//!
//! **Spawn** `(floor f12, pos a0, kind a1, vel a2)`: kind 0 → alpha 0x20, size 0.12 (and one `randf(0.25, 0.25)`
//! whose value is dropped); kind 1 → 0xd, 0.5 (`randf(1, 1)`, dropped); timer `ticks(55)` (never read); RGBA `alpha <<
//! 24 | 0xffffff`, byte9 `trunc(4) + 0x20` = 0x24, ALPHA 0x48, byte1 0, texture `def[73][kind]`, size·210000,
//! rotation `trunc(randf(0, 360))`, position = pos, velocity +0x20 = vel, +0x2c = floor (over vel.w).
//!
//! **Update**: outside [0, 512] in x or y → killed. The floor above the position → a splash (`SpawnImpactSparks`,
//! [`super::type01::impact_sparks_here`]) at the position, killed. Else pos += (vel.xyz, 0) + the camera step; in a
//! new height-grid cell, floor = the weather floor or the grid's height when that is not below it, and a position under
//! it → splash, killed; else vel.z −= dt²/2 and +0x2c = floor + the camera step's z. No RNG of its own (the splash
//! makes none).

use super::{rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 73;

/// Level08 `0x27f5f0(floor, pos, kind, vel)`: two draws (kinds 0 / 1) or one with a record; none when the pool is
/// full.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, floor: f32, pos: [f32; 4], kind: i32, vel: [f32; 4]) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let (a, size) = match kind {
        0 => {
            rng.randf(0.25, 0.25);
            (0x20u32, 0.12f32)
        }
        1 => {
            rng.randf(1.0, 1.0);
            (0xd, 0.5)
        }
        _ => (0, 0.0),
    };
    let t = sys.time.ticks(0x37) as i16;
    let tex = sys.def_frame(TYPE, kind.max(0) as usize);
    let rot = rng.randf(0.0, 360.0) as i32 as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_i16(r, 10, t);
    rec::set_u32(r, 4, a << 24 | 0xff_ffff);
    r[9] = 4 + 0x20;
    r[3] = 0x48;
    r[1] = 0;
    rec::set_ff(r, 0xc, size * 210000.0);
    r[2] = tex;
    r[8] = rot;
    rec::set_v4(r, 0x10, pos);
    rec::set_v4(r, 0x20, vel);
    rec::set_ff(r, 0x2c, floor);
    Some(i)
}

/// Update 0x28a5b8 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let (wind, wfloor, dt2) = (sys.weather.wind, sys.weather.floor, f32::from_bits(sys.time.dt2));
    let grid = sys.grid.clone();
    let r = &mut sys.pool.recs[i];
    let p = rec::v4(r, 0x10);
    if p[0] < 0.0 || p[1] < 0.0 || 512.0 < p[0] || 512.0 < p[1] {
        sys.kill_part(i);
        return;
    }
    let floor = rec::ff(r, 0x2c);
    let splash = if floor <= p[2] {
        let v = rec::v3(r, 0x20);
        let q = [p[0] + v[0] + wind[0], p[1] + v[1] + wind[1], p[2] + v[2] + wind[2], p[3] + wind[3]];
        rec::set_v4(r, 0x10, q);
        let fl = if p[0] as i32 == q[0] as i32 && p[1] as i32 == q[1] as i32 {
            Some(floor)
        } else {
            let h = grid.as_deref().and_then(|g| g.height(q[0], q[1])).unwrap_or(0.0);
            let fl = if wfloor <= h { h } else { wfloor };
            (fl <= q[2]).then_some(fl)
        };
        match fl {
            Some(fl) => {
                rec::set_ff(r, 0x28, rec::ff(r, 0x28) - dt2 * 0.5);
                rec::set_ff(r, 0x2c, fl + wind[2]);
                None
            }
            None => Some(q),
        }
    } else {
        Some(p)
    };
    if let Some(at) = splash {
        super::type01::impact_sparks_here(sys, at);
        sys.kill_part(i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particles::Weather;
    use rc_formats::level::HeightGrid;

    /// A flake drifts down with the camera step and, reaching a cell whose height is above it, splashes: a type-1
    /// record where the line down from one step above the grid meets the world (none here without a mesh: only the
    /// kill), with the dropped draws of the spawn in order.
    #[test]
    fn flake_falls_and_splashes() {
        let mut s = Particles::new(None, Vec::new());
        s.grid = Some(std::sync::Arc::new(HeightGrid { width: 512, rows: 512, low: 0.0, high: 10.0, cells: vec![255; 512 * 512] }));
        s.weather = Weather { wind: [0.0, 0.0, 0.0, 0.0], floor: -100.0 };
        let mut rng = Rng::new();
        let mut t = rng;
        let i = spawn(&mut s, &mut rng, -100.0, [10.5, 10.5, 3.0, 1.0], 0, [0.6, 0.0, -0.5, 0.0]).unwrap();
        t.randf(0.25, 0.25);
        let rot = t.randf(0.0, 360.0) as i32 as u8;
        assert_eq!(rng, t);
        let r = &s.pool.recs[i];
        assert_eq!((r[8], r[9], r[3], rec::u32(r, 4), rec::ff(r, 0xc)), (rot, 0x24, 0x48, 0x20ff_ffff, 0.12 * 210000.0));
        let mut n = 0;
        while s.pool.count > 0 && n < 50 {
            s.update_parts(&mut rng);
            n += 1;
        }
        // z = 3 − 0.5·n (+ gravity): the first new cell with z below 0 splashes.
        assert!((6..=9).contains(&n), "splashed after {n}");
        assert_eq!(s.stats.unported_kills[1], 0);
    }
}
