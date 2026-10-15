//! Particle type 64, the flying scorch / spark that bursts on the ground (level01 spawner `PartType64Spawn` 0x288d90,
//! update 0x288f10, read from the decomp): the bomb's underwater debris burst (`0x2bfe40`, 150 of them).
//!
//! **Spawn** `(size, floor, g, pos, vel, life, rgba, rgba2, additive)`: byte9 0x31 within √80 of the hero (0x13f3d0),
//! else 0x21; ALPHA 0x48 / 0x44; sprite; texture `def[64][1]`; RGBA; size `size·210000`; rotation `trunc(randf(0,
//! 255))` (one draw, with a record); timer life; velocity; +0x30 the floor, +0x34 the gravity, +0x38 rgba2.
//!
//! **Update**: once the timer is out vel.z −= g (every tick after); pos += vel; rotation + 2; below the floor it lands
//! (z = floor) and turns into a type-15 streak and dies: with odds `1/(0x160228 − 1)` (0x160228 = 15) a fast bounce
//! (vel.x, vel.y × `randf(1, 3.5)`, vel.z × `randf(−1, −1.5)`, streak size ½, life `rand_range(ticks(60),
//! ticks(120))`), else a skid (vel.x, vel.y + `randf(−5, 5)·dt`, vel.z × `randf(−0.5, −1)`, size ¼, life
//! `rand_range(ticks(30), ticks(60))`); on level 10 (Rilgar's water) also 1 in 19 a type-45 ring and 1 in 9 a type-16
//! smoke puff (both at the landing point). Standard `f32`.

use super::{fast_dec_timer, rec, tween_color, type15, type16, type45, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 64;
/// 0x160228: the bounce odds' base (level01 .sdata).
const BOUNCE_BASE: i32 = 15;

/// The spawner's arguments.
#[derive(Clone, Copy, Debug)]
pub struct Spawn {
    pub size: f32,
    pub floor: f32,
    pub g: f32,
    pub pos: [f32; 4],
    pub vel: [f32; 4],
    pub life: i16,
    pub rgba: u32,
    pub rgba2: u32,
    pub additive: bool,
}

/// `PartType64Spawn` 0x288d90 (the hero's position from [`Particles::hero`]).
pub fn spawn(sys: &mut Particles, rng: &mut Rng, a: &Spawn) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_frame(TYPE, 1);
    let h = sys.hero;
    let d2 = (0..3).map(|k| (h[k] - a.pos[k]).powi(2)).sum::<f32>();
    let rot = rng.randf(0.0, 255.0);
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, a.pos);
    r[9] = if d2 < 80.0 { 1 + 0x30 } else { 1 + 0x20 };
    r[3] = if a.additive { 0x48 } else { 0x44 };
    r[1] = 0;
    rec::set_u32(r, 4, a.rgba);
    r[2] = def;
    rec::set_ff(r, 0xc, a.size * 210000.0);
    rec::set_i16(r, 0xa, a.life);
    r[8] = rot as i32 as u8;
    rec::set_v4(r, 0x20, a.vel);
    rec::set_u32(r, 0x38, a.rgba2);
    rec::set_ff(r, 0x30, a.floor);
    rec::set_ff(r, 0x34, a.g);
    Some(i)
}

/// Update 0x288f10.
pub fn update(sys: &mut Particles, i: usize, rng: &mut Rng) {
    let dt = f32::from_bits(sys.time.speed) / 60.0;
    let (t30, t60, t90, t120, t150) = (sys.time.ticks(30), sys.time.ticks(60), sys.time.ticks(90), sys.time.ticks(120), sys.time.ticks(150));
    let level = sys.level;
    let load = f32::from_bits(sys.frame_load[0]);
    let r = &mut sys.pool.recs[i];
    if fast_dec_timer(r, 0xa) != 0 { rec::set_ff(r, 0x28, rec::ff(r, 0x28) - rec::ff(r, 0x34)); }
    let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    r[8] = r[8].wrapping_add(2);
    let floor = rec::ff(r, 0x30);
    if rec::ff(r, 0x18) >= floor { return; }
    let c = rec::u32(r, 4);
    rec::set_ff(r, 0x18, floor);
    let mut v = rec::v3(r, 0x20);
    let (size, life) = if rng.randi(BOUNCE_BASE - 1) == 0 {
        v[0] *= rng.randf(1.0, 3.5);
        v[1] *= rng.randf(1.0, 3.5);
        v[2] *= rng.randf(-1.0, -1.5);
        (rec::ff(r, 0xc) * 0.5, (t60, t120))
    } else {
        v[0] += rng.randf(-5.0, 5.0) * dt;
        v[1] += rng.randf(-5.0, 5.0) * dt;
        v[2] *= rng.randf(-0.5, -1.0);
        (rec::ff(r, 0xc) * 0.25, (t30, t60))
    };
    rec::set_v3(r, 0x20, v);
    let life = rng.rand_range(life.0, life.1);
    let (pos, vel, c2, tex, blend) = (rec::v3(r, 0x10), rec::v3(r, 0x20), rec::u32(r, 0x38), r[2], r[3]);
    let s = type15::Spawn { size, pos: [pos[0], pos[1], pos[2], 0.0], vel: [vel[0], vel[1], vel[2], 0.0], c1: c, c2, life, split: 0, def: tex as i32, blend: blend as i32 };
    type15::spawn(sys, rng, &s);
    if level == 10 {
        let rgb = c & 0xff_ffff;
        if rng.randi(0x13) == 0 {
            let a = rng.randi(6);
            type45::spawn45(sys, rng, 0.05, 10500.0, [pos[0], pos[1], pos[2] + 0.1, 0.0], ((a + 0x34) as u32) << 24 | rgb);
        }
        if rng.randi(9) == 0 {
            let c1 = tween_color(rng.randf(0.25, 1.0).to_bits(), 0x7f00_0000, rgb | 0x7f00_0000);
            let c2 = tween_color(rng.randf(0.5, 1.0).to_bits(), 0, 0x5f00);
            let mut d = [rng.randf(-1.0, 1.0), rng.randf(-1.0, 1.0), 0.0];
            let len = rng.randf(1.5, 3.0) * dt;
            let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
            if l != 0.0 { d = [d[0] * len / l, d[1] * len / l, 0.0]; }
            d[2] = dt * 3.0;
            let size = rng.randf(420000.0, 630000.0);
            let life = rng.rand_range(t90, t150);
            let a = type16::Spawn { size, pos: [pos[0], pos[1], pos[2] - 1.5, 0.0], vel: [d[0], d[1], d[2], 0.0], c1, c2, life, kind: 3 };
            if !type16::throttled(rng, life, load) { type16::spawn(sys, rng, &a); }
        }
    }
    sys.kill_part(i);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_and_bursts_into_a_streak() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let a = Spawn { size: 0.5, floor: 5.0, g: 0.01, pos: [10.0, 10.0, 6.0, 0.0], vel: [0.0, 0.0, 0.0, 0.0], life: 0, rgba: 0x7f60_6016, rgba2: 0, additive: false };
        spawn(&mut s, &mut rng, &a).unwrap();
        let mut n = 0;
        while s.pool.live().all(|(_, r)| r[0] == TYPE) && n < 100 {
            s.update_parts(&mut rng);
            n += 1;
        }
        // g every tick: z = 6 − 0.01·k(k+1)/2 < 5 at k = 14.
        assert_eq!(n, 14);
        let live: Vec<u8> = s.pool.live().map(|(_, r)| r[0]).collect();
        assert_eq!(live, [15]);
    }
}
