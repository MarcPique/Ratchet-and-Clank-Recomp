//! Particle type 1, the rain splash, and `SpawnImpactSparks` 0x2780b0 that places it (level01 spawner
//! `PartType01Spawn` 0x27daf0, update 0x27dbc8, read from the decomp; all three the same code on every level that has
//! them, `overlay-diff`: `SpawnImpactSparks` only differs on level 09 in a callee). Callers: the weather emitter class
//! 1400 (levels 08, 12, 14: the census's U399 / U293, not ported) and type 73's update.
//!
//! **`SpawnImpactSparks(pos)`** (a splash where a raindrop at `pos.xy` lands, despite the name): dz = `(high −
//! low)/256` of the level height grid (0x15fca4 − 0x15fca0); z = `height(pos.xy)` (`0x278020`) + dz; only when z is
//! below the camera's z + 5: `CollLine_Fix((x, y, z), (x, y, z − 2dz), 0x12)`; on a hit at h, a splash
//! `PartType01Spawn(h, alpha)` with alpha = `trunc((|h − camera|_xy / 60)·191 + 64) & 0xff` (`fun_001f9b20` is the xy
//! length: far splashes are more opaque). No RNG.
//!
//! **Spawn** `(pos, alpha)`: pos.z += 0.1 (the game adds it to the caller's vector, the collision hit 0x1742e0),
//! position = pos, RGBA `alpha << 24 | 0x907070`, byte9 `trunc(2) + 0x40` = 0x42, ALPHA 0x48 (additive), byte1 0, size
//! 31500 (0x46f61801), texture `def[1][0]`, rotation 0xa0, timer `ticks(27)`, +0x20 = alpha. No RNG.
//!
//! **Update**: A = `t·alpha / ticks(27)` (integer, the timer before its step); the timer fires → killed; else the frame
//! = `def[1][(ticks(27) − t)·9 / ticks(27)]` (a 10-frame splash). No RNG.

use super::{fast_dec_timer, rec, Particles};
use crate::rng::Rng;
use rc_formats::level::HeightGrid;

pub const TYPE: u8 = 1;

/// `PartType01Spawn(pos, alpha)` 0x27daf0 (no RNG).
pub fn spawn(sys: &mut Particles, pos: [f32; 4], alpha: u32) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let t27 = sys.time.ticks(0x1b) as i16;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, [pos[0], pos[1], pos[2] + 0.1, pos[3]]);
    rec::set_u32(r, 4, alpha << 24 | 0x90_7070);
    r[9] = 2 + 0x40;
    r[3] = 0x48;
    r[1] = 0;
    rec::set_u32(r, 0xc, 0x46f6_1801);
    r[2] = def;
    r[8] = 0xa0;
    rec::set_i16(r, 10, t27);
    rec::set_u32(r, 0x20, alpha);
    Some(i)
}

/// `SpawnImpactSparks(pos)` 0x2780b0 with the level's height grid, the camera (0x167240) and the line test
/// `CollLine_Fix(from, to, 0x12, 0, 0)` (the hit point, or none). The splash's record, if one was made.
pub fn impact_sparks(sys: &mut Particles, grid: &HeightGrid, camera: [f32; 3], pos: [f32; 4], coll_line: &mut dyn FnMut([f32; 3], [f32; 3]) -> Option<[f32; 3]>) -> Option<usize> {
    let dz = (grid.high - grid.low) * 0.003_906_25;
    let z = grid.height(pos[0], pos[1])? + dz;
    if z >= camera[2] + 5.0 { return None; }
    let hit = coll_line([pos[0], pos[1], z], [pos[0], pos[1], z - (dz + dz)])?;
    let d = [hit[0] - camera[0], hit[1] - camera[1]];
    let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
    let alpha = ((l / 60.0) * 191.0 + 64.0) as i32 as u32 & 0xff;
    spawn(sys, [hit[0], hit[1], hit[2], pos[3]], alpha)
}

/// `SpawnImpactSparks(pos)` with the system's own grid ([`Particles::grid`]), camera ([`Particles::camera`]) and world
/// mesh ([`Particles::coll`], flags 0x12; the game's `CollLine_Fix` also tests the mobys, not here [L]): type 73's
/// call. Nothing without a grid.
pub fn impact_sparks_here(sys: &mut Particles, pos: [f32; 4]) -> Option<usize> {
    let grid = sys.grid.clone()?;
    let coll = sys.coll.clone();
    let cam = sys.camera.map(f32::from_bits);
    let mut line = |a: [f32; 3], b: [f32; 3]| coll.as_deref().and_then(|c| crate::collision_query::coll_line(c, a, b, crate::collision_query::QueryFlags(0x12))).map(|o| o.point);
    impact_sparks(sys, &grid, cam, pos, &mut line)
}

/// Update 0x27dbc8 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let t27 = sys.time.ticks(0x1b).max(1);
    let r = &mut sys.pool.recs[i];
    let a = (rec::i16(r, 10) as i32 * rec::u32(r, 0x20) as i32) / t27;
    rec::set_u32(r, 4, rec::u32(r, 4) & 0xff_ffff | (a as u32) << 24);
    if fast_dec_timer(r, 10) != 0 {
        sys.kill_part(i);
        return;
    }
    let k = ((t27 - rec::i16(r, 10) as i32) * 9) / t27;
    let f = sys.def_frame(TYPE, k as usize);
    sys.pool.recs[i][2] = f;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> HeightGrid { HeightGrid { width: 4, rows: 4, low: 0.0, high: 25.6, cells: vec![155; 16] } }

    /// The splash: 27 updates, the alpha falling linearly from its start, the frame stepping 0..9.
    #[test]
    fn splash_fades_over_27_ticks() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn(&mut s, [1.0, 1.0, 1.0, 1.0], 0xa0).unwrap();
        let r = &s.pool.recs[i];
        assert_eq!((rec::u32(r, 4), r[9], r[3], r[8], rec::i16(r, 10)), (0xa090_7070, 0x42, 0x48, 0xa0, 27));
        assert_eq!(rec::pos(r), [1.0, 1.0, 1.1]);
        s.update_parts(&mut rng);
        assert_eq!(rec::u32(&s.pool.recs[i], 4) >> 24, 0xa0, "t = 27 before the step");
        s.update_parts(&mut rng);
        assert_eq!(rec::u32(&s.pool.recs[i], 4) >> 24, 26 * 0xa0 / 27);
        let mut n = 2;
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            n += 1;
        }
        assert_eq!(n, 27);
        assert_eq!(rng, Rng::new());
    }

    /// `SpawnImpactSparks`: the line from one step above the ground to one below; nothing above the camera + 5 or
    /// without a hit; the alpha from the xy distance to the camera.
    #[test]
    fn impact_places_a_splash_where_the_line_hits() {
        let mut s = Particles::new(None, Vec::new());
        let g = grid();
        // Cell 155: height = 25.6·(1 − 155/255) ≈ 10.04; dz = 0.1.
        let h = g.height(1.0, 1.0).unwrap();
        let mut lines = Vec::new();
        let mut coll = |a: [f32; 3], b: [f32; 3]| {
            lines.push((a, b));
            Some([a[0], a[1], h])
        };
        let i = impact_sparks(&mut s, &g, [31.0, 1.0, 20.0], [1.0, 1.0, 50.0, 1.0], &mut coll).unwrap();
        assert_eq!(lines, vec![([1.0, 1.0, h + 0.1], [1.0, 1.0, h + 0.1 - 0.2])]);
        let a = ((30.0f32 / 60.0) * 191.0 + 64.0) as u32;
        assert_eq!(rec::u32(&s.pool.recs[i], 4) >> 24, a);
        assert_eq!(rec::pos(&s.pool.recs[i]), [1.0, 1.0, h + 0.1]);
        assert!(impact_sparks(&mut s, &g, [0.0, 0.0, 4.0], [1.0, 1.0, 0.0, 1.0], &mut |_, _| Some([0.0; 3])).is_none(), "above the camera + 5");
        assert!(impact_sparks(&mut s, &g, [0.0, 0.0, 20.0], [1.0, 1.0, 0.0, 1.0], &mut |_, _| None).is_none(), "no hit");
    }
}
