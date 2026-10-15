//! Particle type 25, the grind / cable spark (the rails' and the cable's sparks: `hero::boots`), read from the
//! level01 code (the particle code hash-matches in every overlay):
//!
//! * spawner [`spawn`] = `PartType25Spawn(pos, vel, variant)` 0x2825f8: kind-0 sprite, additive (0x48), byte9 0x44
//!   (near 1 u, far 128 u), texture `def[25][0]`, rotation 0, colour channels (+0x30..+0x38) 1.0 (negated for the
//!   variant), alpha +0x3c 0.6, RGBA packed from them, life `ticks(80)`, size `randf(5000, 30000)` (the one draw,
//!   made by the caller at the game's point: [`crate::hero::fx::spark`]), position and velocity (its w = the
//!   gravity per tick, 0 = the default −0.0063) as given;
//! * [`update`] = 0x282760.
//!
//! Standard `f32` (the result, not the FPU's rounding: docs/plan/hardware_fidelity_layers.md).

#![allow(clippy::needless_range_loop)] // record offsets by component, as the game's stores.
use super::{fast_dec_timer, rec, Particles, Record};
use crate::rng::Rng;

/// Type id.
pub const TYPE: u8 = 25;

fn f(r: &Record, o: usize) -> f32 { f32::from_bits(rec::u32(r, o)) }
fn set(r: &mut Record, o: usize, v: f32) { rec::set_u32(r, o, v.to_bits()); }

/// `0x270ea0(r, g, b, a)`: `trunc(x·255)` per channel, R in the low byte.
pub fn pack_rgba(c: [f32; 4]) -> u32 {
    c.iter().enumerate().fold(0u32, |o, (k, &x)| o | (((x * 255.0) as i32 as u32) & 0xff) << (8 * k))
}

/// Type 65: the same spark with type 25's update (level01 `PartType65Update` 0x289388 is 0x282760 without the
/// variant's swapped rates: its only spawner, level07 `0x29c4e8` (also on 15 and 17), gives positive channels, so the
/// variant branch is unreachable and [`update`] is the same function for it).
pub const TYPE65: u8 = 65;

/// Level07 `0x29c4e8(pos, vel)`: RGBA `0x270ea0(1, 1, 1, 0.6)`, byte1 0, byte9 0x44, ALPHA 0x48, size `randf(20000,
/// 120000)` (the one draw), rotation 0, texture `def[65][0]`, channels 1 and alpha 0.6, timer `ticks(80)`, velocity
/// (w = gravity, 0 → −0.0063). None when the pool is full (no draw).
pub fn spawn65(sys: &mut Particles, rng: &mut Rng, pos: [f32; 4], vel: [f32; 4]) -> Option<usize> {
    let i = sys.create_part(TYPE65)?;
    let def = sys.def_first(TYPE65);
    let ticks = sys.time.ticks(0x50);
    let size = rng.randf(20000.0, 120000.0);
    let r = &mut sys.pool.recs[i];
    for k in 0..4 { set(r, 0x10 + 4 * k, pos[k]); }
    rec::set_u32(r, 4, pack_rgba([1.0, 1.0, 1.0, 0.6]));
    r[1] = 0;
    r[9] = 4 + 0x40;
    r[3] = 0x48;
    set(r, 0xc, size);
    r[8] = 0;
    r[2] = def;
    for o in [0x38, 0x30, 0x34] { set(r, o, 1.0); }
    set(r, 0x3c, 0.6);
    rec::set_i16(r, 0xa, ticks as i16);
    for k in 0..4 { set(r, 0x20 + 4 * k, vel[k]); }
    Some(i)
}

/// `PartType25Spawn(pos, vel, variant)` 0x2825f8 with the spawner's size draw `size` (already made). None when the
/// pool is full.
pub fn spawn(sys: &mut Particles, pos: [f32; 4], vel: [f32; 4], variant: bool, size: f32) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let ticks = sys.time.ticks(80);
    let r = &mut sys.pool.recs[i];
    for k in 0..4 { set(r, 0x10 + 4 * k, pos[k]); }
    rec::set_u32(r, 4, pack_rgba([1.0, 1.0, 1.0, 0.6]));
    let c = if variant { -1.0 } else { 1.0 };
    for o in [0x38, 0x30, 0x34] { set(r, o, c); }
    r[1] = 0;
    r[9] = 4 + 0x40;
    r[3] = 0x48;
    set(r, 0xc, size);
    r[8] = 0;
    r[2] = def;
    set(r, 0x3c, 0.6);
    rec::set_i16(r, 0xa, ticks as i16);
    for k in 0..4 { set(r, 0x20 + 4 * k, vel[k]); }
    Some(i)
}

/// Update 0x282760 (no RNG): the size shrinks by 100 a tick; the spark moves by its velocity unless the line to the
/// new point hits the world (`CollLine_Fix(pos, new, 2, Ratchet)`: then it sits at the hit point, stops, and its
/// size doubles); while it moves, gravity (+0x2c, or −0.0063·speed) bends it down. Killed on the timer, a size
/// below 0, the alpha at 0, or the new point below 0.01 on any axis; otherwise the colour cools (R −0.01, G −0.03,
/// B −0.05 a tick, clamped at 0: white → yellow → orange → red; the variant swaps the R and G rates) and the
/// rotation byte turns −1. The line tests the world mesh only ([`Particles::coll`]; the game's `CollLine_Fix`
/// also tests the mobys, not here).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let speed = f32::from_bits(sys.time.speed);
    let coll = sys.coll.clone();
    let r = &mut sys.pool.recs[i];
    let (mut cr, mut cg, mut cb) = (f(r, 0x30), f(r, 0x34), f(r, 0x38));
    let variant = cr < 0.0 || cg < 0.0 || cb < 0.0;
    if variant { (cr, cg, cb) = (-cr, -cg, -cb); }
    set(r, 0x3c, f(r, 0x3c) + speed * 0.0);
    set(r, 0xc, f(r, 0xc) + speed * -100.0);
    let pos = [f(r, 0x10), f(r, 0x14), f(r, 0x18)];
    let vel = [f(r, 0x20), f(r, 0x24), f(r, 0x28)];
    let new = [pos[0] + vel[0], pos[1] + vel[1], pos[2] + vel[2]];
    if vel != [0.0; 3] {
        let hit = coll.as_deref().and_then(|c| crate::collision_query::coll_line_m(c, None, pos, new, crate::collision_query::QueryFlags(2), None));
        match hit {
            None => {
                for k in 0..3 { set(r, 0x10 + 4 * k, new[k]); }
                let g = f(r, 0x2c);
                set(r, 0x28, f(r, 0x28) + if g == 0.0 { speed * -0.0063 } else { g });
            }
            Some(h) => {
                for k in 0..3 { set(r, 0x10 + 4 * k, h.point[k]); }
                // 0x221170: the velocity (w too) cleared.
                r[0x20..0x30].fill(0);
                set(r, 0xc, f(r, 0xc) + f(r, 0xc));
            }
        }
    }
    if f(r, 0x3c) <= 0.0 || fast_dec_timer(r, 0xa) != 0 || f(r, 0xc) < 0.0 || new.iter().any(|&x| x < 0.01) {
        sys.kill_part(i);
        return;
    }
    let (fast, slow) = (speed * -0.030_01, speed * -0.01);
    if variant {
        cg = (cg + slow).max(0.0);
        cr = (cr + fast).max(0.0);
    } else {
        cr = (cr + slow).max(0.0);
        cg = (cg + fast).max(0.0);
    }
    cb = (cb + speed * -0.05).max(0.0);
    let a = (f(r, 0x3c) + speed * 0.0).max(0.0);
    set(r, 0x3c, a);
    rec::set_u32(r, 4, pack_rgba([cr, cg, cb, a]));
    let s = if variant { -1.0 } else { 1.0 };
    set(r, 0x30, cr * s);
    set(r, 0x34, cg * s);
    set(r, 0x38, cb * s);
    r[8] = r[8].wrapping_sub(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A spark with the grind's velocity in the open air: white-hot at the spawn (RGBA 0x99ffffff), then cooling
    /// to yellow and red, falling under its gravity, shrinking 100 a tick, gone on the 80th update (the timer); no
    /// draw.
    #[test]
    fn spark_cools_falls_and_dies_on_the_timer() {
        let mut s = Particles::new(None, Vec::new());
        let g = -30.0 / 3600.0;
        let i = spawn(&mut s, [100.0, 100.0, 50.0, 1.0], [0.1, 0.0, 0.07, g], false, 20000.0).unwrap();
        assert_eq!(rec::u32(&s.pool.recs[i], 4), 0x99ff_ffff);
        assert_eq!(s.pool.recs[i][9], 0x44);
        let mut rng = Rng::new();
        let mut last = (0.0, 0.0);
        for t in 1..=80 {
            s.update_parts(&mut rng);
            if s.pool.count == 0 {
                assert_eq!(t, 80, "killed on the timer");
                break;
            }
            let r = &s.pool.recs[i];
            last = (f(r, 0x18), f(r, 0xc));
            if t == 10 {
                let c = rec::u32(r, 4);
                assert_eq!((c & 0xff, c >> 8 & 0xff, c >> 16 & 0xff, c >> 24), (229, 178, 127, 153), "R 0.9 G 0.7 B 0.5 after 10");
            }
        }
        assert_eq!(s.pool.count, 0);
        assert!((last.1 - (20000.0 - 79.0 * 100.0)).abs() < 1.0, "size {}", last.1);
        assert!(last.0 < 50.0, "fell: z {}", last.0);
        assert_eq!(rng.state, Rng::new().state);
    }

    /// The variant cools G slowly and R fast (and keeps the negated channels).
    #[test]
    fn variant_swaps_the_rates() {
        let mut s = Particles::new(None, Vec::new());
        let i = spawn(&mut s, [100.0, 100.0, 50.0, 1.0], [0.0; 4], true, 20000.0).unwrap();
        for _ in 0..10 { s.update_parts(&mut Rng::new()); }
        let c = rec::u32(&s.pool.recs[i], 4);
        assert_eq!((c & 0xff, c >> 8 & 0xff), (178, 229));
        assert!(f(&s.pool.recs[i], 0x30) < 0.0);
    }

    /// Type 65 (level07 `0x29c4e8`): one size draw, the non-variant spark, run by the same update (table row 65).
    #[test]
    fn type65_is_the_plain_spark() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let mut t = rng;
        let i = spawn65(&mut s, &mut rng, [5.0, 5.0, 5.0, 1.0], [0.1, 0.0, 0.0, 0.0]).unwrap();
        assert_eq!(f(&s.pool.recs[i], 0xc), t.randf(20000.0, 120000.0));
        assert_eq!(rng, t);
        assert_eq!(s.pool.recs[i][0], 65);
        s.update_parts(&mut rng);
        let r = &s.pool.recs[i];
        assert_eq!((f(r, 0x30), f(r, 0x34), f(r, 0x38)), (0.99, 1.0 - 0.030_01, 0.95), "the plain rates");
        assert_eq!(s.stats.unported_kills[65], 0);
    }
}
