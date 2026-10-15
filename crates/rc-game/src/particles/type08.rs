//! Particle type 8, the explosion puff (level01 spawner `PartType08Spawn` 0x27f2b0, update 0x27f3a0): the creature
//! explosions' puffs (`SpawnBeamExplosion` 0x273310).
//!
//! **Record** (only for life ≠ 0; no draws): position +0x10, velocity +0x20, colour 1 at +0x04 / +0x30, colour 2 at
//! +0x34, byte9 0xa4, byte3 0x48 (additive), byte1 0 (sprite), rotation 0, texture `*def[8]`, size 0, +0x0a life (s16),
//! +0x38 life (s32), +0x3c growth per tick = `size / life`.
//!
//! **Update**: pos += velocity; `FastDecTimer` (kill when it fires); else size += growth, colour =
//! `FastTweenColor(t/life, c2, c1)`, below `ticks(6)` the alpha becomes `t·A / ticks(6)` (integer), texture
//! `def[8][(life − t)·10 / life]` (an 11-frame animation).

use super::{fast_dec_timer, rec, tween_color, Particles};
use crate::rng::Rng;

/// `PartType08Spawn(size, pos, vel, c1, c2, life)` 0x27f2b0.
pub fn spawn(sys: &mut Particles, size: f32, pos: [f32; 4], vel: [f32; 4], c1: u32, c2: u32, life: i32) -> Option<usize> {
    if life == 0 { return None; }
    let i = sys.create_part(8)?;
    let def = sys.def_first(8);
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, c1);
    r[9] = 0xa4;
    r[3] = 0x48;
    r[1] = 0;
    rec::set_ff(r, 0xc, 0.0);
    r[8] = 0;
    r[2] = def;
    rec::set_v4(r, 0x20, vel);
    rec::set_i16(r, 0xa, life as i16);
    rec::set_u32(r, 0x38, life as u32);
    rec::set_u32(r, 0x34, c2);
    rec::set_u32(r, 0x30, c1);
    rec::set_ff(r, 0x3c, size / life as f32);
    Some(i)
}

/// 0x27f3a0.
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let t6 = sys.time.ticks(6);
    let frames = sys.defs.as_ref().and_then(|d| d.start(8).map(|s| (d.blob.clone(), s)));
    let r = &mut sys.pool.recs[i];
    let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    if fast_dec_timer(r, 0xa) != 0 { sys.kill_part(i); return; }
    rec::set_ff(r, 0xc, rec::ff(r, 0xc) + rec::ff(r, 0x3c));
    let life = rec::u32(r, 0x38) as i32;
    let t = rec::i16(r, 0xa) as i32;
    let c = tween_color((t as f32 / life as f32).to_bits(), rec::u32(r, 0x34), rec::u32(r, 0x30));
    rec::set_u32(r, 4, c);
    if t < t6 && t6 != 0 {
        let a = (t * r[7] as i32) / t6;
        rec::set_u32(r, 4, (rec::u32(r, 4) & 0xff_ffff) | (a as u32) << 24);
    }
    if life != 0 {
        if let Some((blob, s)) = frames {
            let k = ((life - t) * 10) / life;
            if let Some(&f) = blob.get(s + k.max(0) as usize) { r[2] = f; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_and_fades_out_in_the_last_six_ticks() {
        let mut sys = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        assert!(spawn(&mut sys, 200000.0, [1.0; 4], [0.0; 4], 0x4f00_8fff, 0x2f00_5f7f, 0).is_none());
        let i = spawn(&mut sys, 200000.0, [1.0, 1.0, 1.0, 0.0], [0.0, 0.0, 0.01, 0.0], 0x4f00_8fff, 0x2f00_5f7f, 30).unwrap();
        let mut n = 0;
        let mut last_a = 0x4f;
        while sys.pool.count > 0 {
            sys.update_parts(&mut rng);
            n += 1;
            if sys.pool.count > 0 {
                let a = sys.pool.recs[i][7];
                if rec::i16(&sys.pool.recs[i], 0xa) < 6 { assert!(a <= last_a); }
                last_a = a;
            }
        }
        assert_eq!(n, 30);
        assert!((rec::ff(&sys.pool.recs[i], 0xc) - 29.0 * 200000.0 / 30.0).abs() < 1.0);
    }
}
