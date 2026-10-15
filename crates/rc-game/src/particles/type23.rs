//! Particle type 23, the glowing puff (level01 spawner `PartType23Spawn` 0x282060, update `PartType23Update`
//! 0x282210): the cutscene FX driver's ship trail and thruster puffs (`FUN_00278810`, class 1546), the fire / smoke
//! fields' smoke (class 760 with +0x42, level 00). Native `f32`.
//!
//! **Spawn** `(jitter, grow_lo, grow_hi, size, pos, spin, vel, rgba)`: position +0x10 = pos + 3 × `randf_sym(0,
//! jitter)`, colour +0x04 = rgba | 0x20000000 (alpha 0x20) and the base colour +0x20 = rgba, byte9 = 0x74, byte3 0x48
//! (additive), byte1 0 (sprite), rotation byte8 0, texture `*def[23]`, size +0x0c, timer +0x0a = `ticks(10)`, phase
//! +0x24 = 0, spin +0x28 = ±spin·[0x15ed60] (`randi(2)` picks the sign), growth +0x2c = `randf(grow_lo, grow_hi)`,
//! velocity +0x30. Callers patch the timer and phase after it (e.g. phase 2 with +0x2a = 0x7f, +0x2b = the timer).
//!
//! **Update**: rotation += spin (byte), pos += velocity, size ·= (growth − 1)·[0x15ed60] + 1; then by phase:
//! * 0 (fade in): timer out → phase 1, timer `ticks(20)`, alpha 0x7f; else alpha = 0x20 + (ticks(10) − t)·96/ticks(10);
//! * 1 (fade out): timer out → kill; else alpha = t·127/ticks(20);
//! * 2 (timed fade): timer out → kill; else alpha = +0x2a·t/+0x2b (colour bits of +0x20 kept, its alpha replaced);
//! * 3 (fade in then 2): alpha = +0x2a·min(1, (+0x2b − t)/ticks(30)); after `ticks(30)` → phase 2, +0x2b −= ticks(30).
//!
//! In phases 0 and 1 the alpha is OR'd into the whole base colour word (the game's `a << 24 | +0x20`).

use super::{fast_dec_timer, rec, Particles};
use crate::rng::Rng;

/// `PartType23Spawn(jitter, grow_lo, grow_hi, size, pos, spin, vel, rgba)` 0x282060.
#[allow(clippy::too_many_arguments)]
pub fn spawn(sys: &mut Particles, rng: &mut Rng, jitter: f32, grow_lo: f32, grow_hi: f32, size: f32, pos: [f32; 4], spin: i32, vel: [f32; 4], rgba: u32) -> Option<usize> {
    let i = sys.create_part(23)?;
    let def = sys.def_first(23);
    let t10 = sys.time.ticks(10);
    let speed = f32::from_bits(sys.time.speed);
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, rgba | 0x2000_0000);
    r[9] = 4 + 0x70;
    r[3] = 0x48;
    rec::set_ff(r, 0xc, size);
    r[1] = 0;
    r[8] = 0;
    r[2] = def;
    for k in 0..3 {
        let d = rng.randf_sym(0.0, jitter);
        rec::set_ff(r, 0x10 + 4 * k, rec::ff(r, 0x10 + 4 * k) + d);
    }
    rec::set_i16(r, 0xa, t10 as i16);
    rec::set_u32(r, 0x24, 0);
    rec::set_u32(r, 0x20, rgba);
    let s = if rng.randi(2) == 0 { -(spin as f32) * speed } else { spin as f32 * speed };
    rec::set_i16(r, 0x28, s as i32 as i16);
    let g = rng.randf(grow_lo, grow_hi);
    rec::set_ff(r, 0x2c, g);
    rec::set_v4(r, 0x30, vel);
    Some(i)
}

/// `PartType23Update` 0x282210.
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let (t10, t20, t30) = (sys.time.ticks(10), sys.time.ticks(20), sys.time.ticks(30));
    let speed = f32::from_bits(sys.time.speed);
    let r = &mut sys.pool.recs[i];
    r[8] = r[8].wrapping_add(r[0x28]);
    let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x30));
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    rec::set_ff(r, 0xc, rec::ff(r, 0xc) * ((rec::ff(r, 0x2c) - 1.0) * speed + 1.0));
    let base = rec::u32(r, 0x20);
    let alpha = match rec::u32(r, 0x24) {
        0 => {
            if fast_dec_timer(r, 0xa) != 0 {
                rec::set_u32(r, 0x24, 1);
                rec::set_i16(r, 0xa, t20 as i16);
                rec::set_u32(r, 4, base | 0x7f00_0000);
                return;
            }
            let t = rec::i16(r, 0xa) as i32;
            (((t10 - t) as f32 * (96.0 / t10 as f32)) as i32 + 0x20) as u32
        }
        2 => {
            if fast_dec_timer(r, 0xa) != 0 { sys.kill_part(i); return; }
            let (a, n, t) = (r[0x2a] as f32, r[0x2b] as f32, rec::i16(r, 0xa) as f32);
            let a = (a * (t / n)) as i32 as u32;
            rec::set_u32(r, 4, a << 24 | base & 0xff_ffff);
            return;
        }
        3 => {
            fast_dec_timer(r, 0xa);
            let t = rec::i16(r, 0xa) as i32;
            let f = ((r[0x2b] as i32 - t) as f32 / t30 as f32).min(1.0);
            let a = (r[0x2a] as f32 * f) as i32 as u32;
            rec::set_u32(r, 4, a << 24 | base & 0xff_ffff);
            if r[0x2b] as i32 - t >= t30 {
                rec::set_u32(r, 0x24, 2);
                r[0x2b] = r[0x2b].wrapping_sub(t30 as u8);
            }
            return;
        }
        _ => {
            if fast_dec_timer(r, 0xa) != 0 { sys.kill_part(i); return; }
            let t = rec::i16(r, 0xa) as i32;
            (t as f32 * (127.0 / t20 as f32)) as i32 as u32
        }
    };
    rec::set_u32(r, 4, alpha << 24 | base);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fades_in_then_out_and_grows() {
        let mut sys = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn(&mut sys, &mut rng, 0.0, 1.0, 1.0, 1000.0, [1.0, 2.0, 3.0, 0.0], 16, [0.0, 0.0, 0.5, 0.0], 0x0020_4080).unwrap();
        assert_eq!(rec::u32(&sys.pool.recs[i], 4) >> 24, 0x20);
        let mut n = 0;
        while sys.pool.count > 0 && n < 100 {
            sys.update_parts(&mut rng);
            n += 1;
        }
        // 10 ticks of fade-in, 20 of fade-out.
        assert_eq!(n, 30);
        assert_eq!(rec::v3(&sys.pool.recs[i], 0x10)[2], 3.0 + 30.0 * 0.5);
    }
}
