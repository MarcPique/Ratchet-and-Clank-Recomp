//! Particle type 60, the glint (the Swingshot targets' sparkle: `moby_update::classes::swing_target`), read from the
//! level01 code:
//!
//! * spawner [`spawn`] = `PartType60Spawn(size, pos, vel, rgba, life, rot, attach)` 0x288588: refused (no record)
//!   outside [2, 1021]³; kind-0 sprite, additive (0x48), byte9 0x21 (near 0.25 u, far 64 u), texture
//!   `def[60][0]`, size `size·210000`, life `life & 0xff`, rotation `rot`; `attach` 1 keeps an offset from the hero
//!   position 0x13f3d0 at +0x30 ([`Particles::hero`]);
//! * [`update`] = 0x288740.

#![allow(clippy::needless_range_loop)] // record offsets by component, as the game's stores.
use super::{fast_dec_timer, rec, tween_color, Particles, Record};
use crate::rng::Rng;

pub const TYPE: u8 = 60;

fn f(r: &Record, o: usize) -> f32 { f32::from_bits(rec::u32(r, o)) }
fn set(r: &mut Record, o: usize, v: f32) { rec::set_u32(r, o, v.to_bits()); }
fn outside(p: [f32; 3]) -> bool { p.iter().any(|&x| !(2.0..=1021.0).contains(&x)) }

/// `PartType60Spawn(size, pos, vel, rgba, life, rot, attach)` 0x288588 (no draw: the caller's `rot` argument is
/// its draw). None outside the world box or when the pool is full.
#[allow(clippy::too_many_arguments)]
pub fn spawn(sys: &mut Particles, size: f32, pos: [f32; 4], vel: [f32; 4], rgba: u32, life: u16, rot: u8, attach: i32) -> Option<usize> {
    if outside([pos[0], pos[1], pos[2]]) { return None; }
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let hero = sys.hero;
    let r = &mut sys.pool.recs[i];
    for k in 0..4 { set(r, 0x10 + 4 * k, pos[k]); }
    for k in 0..4 { set(r, 0x20 + 4 * k, vel[k]); }
    rec::set_u32(r, 0x3c, attach as u32);
    if attach == 1 {
        for k in 0..3 { set(r, 0x30 + 4 * k, pos[k] - hero[k]); }
    }
    rec::set_u32(r, 4, rgba);
    r[9] = 1 + 0x20;
    r[3] = 0x48;
    r[8] = rot;
    r[1] = 0;
    set(r, 0xc, size * 210000.0);
    rec::set_i16(r, 0xa, (life & 0xff) as i16);
    r[2] = def;
    Some(i)
}

/// Update 0x288740 (no RNG): pos += vel (attached: from the hero's position plus the kept offset); killed outside
/// [2, 1021]³ or on the timer; else the alpha falls linearly to 0 over the life (`FastTweenColor((t − 1)/t, rgb|0,
/// rgba)`).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let hero = sys.hero;
    let r = &mut sys.pool.recs[i];
    if rec::u32(r, 0x3c) == 1 {
        for k in 0..3 { set(r, 0x10 + 4 * k, hero[k] + f(r, 0x30 + 4 * k)); }
        for k in 0..3 { set(r, 0x10 + 4 * k, f(r, 0x10 + 4 * k) + f(r, 0x20 + 4 * k)); }
        for k in 0..3 { set(r, 0x30 + 4 * k, f(r, 0x10 + 4 * k) - hero[k]); }
    } else {
        for k in 0..3 { set(r, 0x10 + 4 * k, f(r, 0x10 + 4 * k) + f(r, 0x20 + 4 * k)); }
    }
    if outside(rec::pos(r)) || fast_dec_timer(r, 0xa) != 0 {
        sys.kill_part(i);
        return;
    }
    let t = rec::i16(r, 0xa) as i32;
    let k = ((t - 1) as f32 / (rec::u32(r, 0xa) & 0xffff) as f32).to_bits();
    let c = rec::u32(r, 4);
    rec::set_u32(r, 4, tween_color(k, c & 0xff_ffff, c));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A swing target's glint: 30 ticks, the alpha 0x60 falling to 0, drifting 2 units back across.
    #[test]
    fn glint_fades_over_its_life() {
        let mut s = Particles::new(None, Vec::new());
        assert!(spawn(&mut s, 1.25, [1.0, 50.0, 50.0, 0.0], [0.0; 4], 0x6040_8080, 30, 7, 0).is_none(), "outside the box");
        let v = -2.0 / 30.0;
        let i = spawn(&mut s, 1.25, [101.0, 50.0, 50.0, 0.0], [v, 0.0, 0.0, 0.0], 0x6040_8080, 30, 7, 0).unwrap();
        assert_eq!(s.pool.recs[i][9], 0x21);
        let mut alphas = Vec::new();
        while s.pool.count > 0 {
            s.update_parts(&mut Rng::new());
            if s.pool.count > 0 { alphas.push(s.pool.recs[i][7]); }
        }
        assert_eq!(alphas.len(), 29);
        assert!(alphas.windows(2).all(|w| w[1] <= w[0]), "{alphas:?}");
        assert_eq!(*alphas.last().unwrap(), 0);
        assert!((f(&s.pool.recs[i], 0x10) - (101.0 - 2.0)).abs() < 1e-3);
    }
}
