//! Particle type 12, the Pyrocitor's flame (level01 spawner `PartType12Spawn` 0x280138, update `PartType12Update`
//! 0x280408; the only spawner is the Pyrocitor's update 0x2cd458, `crate::hero::pyrocitor`).
//!
//! **Spawn** `(length, pos, vel, flags)` (flags bit 0: the glow puff, bit 2: a weapon flame; gold weapons scale the
//! life): position +0x10, velocity +0x20, the flame's remaining length +0x30, flags +0x3f, byte9 0x44, byte1 0.
//! * flame (bit 0 clear): colour `0x20202080`, size 0.25 (×210000), life `trunc(randf(20, 30))`, start colour
//!   `FastTweenColor(randf(0, 1), 0x20202080, 0x20208080)` (+0x34), end colour
//!   `FastTweenColor(randf(0, 1), 0x1820c0c0, 0x182080c0)` (+0x38), texture `def[12][randi(8)]` (+8 with a gold
//!   weapon), ALPHA 0x44;
//! * glow puff (bit 0): colour `0x08002040`, size 1.0, life `trunc(randf(25, 35))`, +0x34 / +0x38 from
//!   `0x08002040..0x08004040` / `0x10004040..0x10002040`, texture `def[24][0]`, additive (ALPHA 0x48);
//! * then the rotation `randi(0xff)` and its speed `trunc(randf_sym(1, 4))` (+0x3e); life ×(gold + 2)/2 for a weapon
//!   flame; +0x3d = the life. Draws, in order: `randf`, `randf`, `randf`, (`randi(8)` for a flame), `randi(0xff)`,
//!   `randf_sym` ([`Draws`]).
//!
//! **Update** (no draws): the rotation turns by +0x3e; `pos += vel`; the speed follows the remaining length
//! (`0x270830(0, 0, 48·dt², −|v|, &len, &v)`: constant speed until the flame is within its stopping distance
//! `v²/(2·48·dt²)` of its length, then it slows by 48·dt² a tick to rest exactly at its end); it rises `1·dt` a tick;
//! size `((s0 − s1·g)·t + s1·g)·210000` with `t = life / life₀` (flame 0.25 → 1.25, glow 1 → 3; `g = 1 + gold/2` for a
//! weapon flame); colour `FastTweenColor(t, +0x38, +0x34)`, fading out over its last 15 ticks
//! (`FastTweenColor(life / 10, c & 0xffffff, c)`); killed outside 2..1021 on any axis or when its life runs out; a
//! flame at rest (length < 0.01) burns out twice as fast.
//!
//! Standard `f32`: [`Flame`] is the record's motion as a plain value, stepped by [`Flame::step`] here and by the
//! Pyrocitor's own list of the flames it hits with (`0x2ce0a0` reads their records), so both see one motion.

use super::{rec, tween_color, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 12;
/// `0x1601e8`: the flames' deceleration (×dt²).
pub const DECEL: f32 = 48.0;
/// `0x1601ec`: the rise (×dt).
pub const RISE: f32 = 1.0;
const DT: f32 = 1.0 / 60.0;

/// The spawner's random draws, made where the game makes them (the hero's item update: `crate::hero::fx`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Draws {
    pub life: f32,
    pub c1: f32,
    pub c2: f32,
    /// `randi(8)` (flames only).
    pub tex: i32,
    pub rot: u8,
    pub spin: f32,
}

impl Draws {
    /// `PartType12Spawn`'s draws for `flags`, in its order.
    pub fn draw(rng: &mut Rng, flags: u8) -> Draws {
        let glow = flags & 1 != 0;
        let (lo, hi) = if glow { (25.0, 35.0) } else { (20.0, 30.0) };
        let life = rng.randf(lo, hi);
        let c1 = rng.randf(0.0, 1.0);
        let c2 = rng.randf(0.0, 1.0);
        let tex = if glow { 0 } else { rng.randi(8) };
        let rot = rng.randi(0xff) as u8;
        let spin = rng.randf_sym(1.0, 4.0);
        Draws { life, c1, c2, tex, rot, spin }
    }
}

/// A flame's motion (record +0x10 position, +0x20 velocity, +0x30 remaining length, +0x0a life).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flame {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub len: f32,
    pub life: i16,
    /// +0x0c: size (×210000).
    pub size: f32,
    /// +0x3f bit 0 (the glow puff) and bit 2 (a weapon flame).
    pub flags: u8,
    /// +0x3d: the life it started with.
    pub life0: u8,
}

/// `FUN_00270728(target, step, &x)`: `x` moves toward `target` by at most `step`.
fn approach(x: &mut f32, target: f32, step: f32) { *x += (target - *x).clamp(-step, step); }

/// `0x270830(0, 0, b, −|v|, &len, &v)` as the flame update calls it: returns the new speed (≥ 0).
fn slow_to_end(len: &mut f32, speed: f32, b: f32) -> f32 {
    let d = -*len;
    let mut v = -speed;
    if v * d < 0.0 || d == 0.0 {
        approach(&mut v, 0.0, b);
        *len += v;
        return -v;
    }
    let s = v * v / b * 0.5;
    if d.abs() < s {
        let step = if s < d.abs() + v.abs() { b } else { b * 1.1 };
        approach(&mut v, 0.0, step);
    }
    // Far from the end the target speed is approached by a step of 0: the speed is kept.
    if d.abs() <= v.abs() {
        *len = 0.0;
        return -d;
    }
    *len += v;
    -v
}

impl Flame {
    /// The sizes (start, end) of the flame or the glow puff.
    fn sizes(&self) -> (f32, f32) { if self.flags & 1 != 0 { (1.0, 3.0) } else { (0.25, 1.25) } }

    /// One `PartType12Update` of the motion, size and life; false when the record dies. `gold` = 0x13e530.
    pub fn step(&mut self, gold: u8) -> bool {
        let g = if self.flags & 4 != 0 { 1.0 + gold as f32 * 0.5 } else { 1.0 };
        for k in 0..3 { self.pos[k] += self.vel[k]; }
        let speed = (self.vel[0] * self.vel[0] + self.vel[1] * self.vel[1] + self.vel[2] * self.vel[2]).sqrt();
        let v = slow_to_end(&mut self.len, speed, DECEL * DT * DT);
        self.vel = if speed == 0.0 { [0.0; 3] } else { self.vel.map(|x| x / speed * v) };
        self.pos[2] += RISE * DT;
        let t = self.life as f32 / self.life0 as f32;
        let (s0, s1) = self.sizes();
        self.size = ((s0 - s1 * g) * t + s1 * g) * 210000.0;
        if self.pos.iter().any(|&p| !(2.0..=1021.0).contains(&p)) { return false; }
        // FastDecTimer: dies when the life reaches 0.
        if self.life == 0 { return false; }
        self.life -= 1;
        if self.life <= 0 { return false; }
        if self.len < 0.01 { self.life -= 1; }
        true
    }
}

/// `PartType12Spawn` 0x280138 (the draws already made). `gold` = 0x13e530. Returns the record and its motion.
pub fn spawn(sys: &mut Particles, len: f32, pos: [f32; 4], vel: [f32; 4], flags: u8, gold: u8, d: &Draws) -> Option<(usize, Flame)> {
    let i = sys.create_part(TYPE)?;
    let glow = flags & 1 != 0;
    let tex = if glow { sys.def_first(24) } else { sys.def_frame(TYPE, (d.tex + if flags & 4 != 0 && gold != 0 { 8 } else { 0 }) as usize) };
    let (c0, a1, b1, a2, b2, size) =
        if glow { (0x0800_2040, 0x0800_2040, 0x0800_4040, 0x1000_4040, 0x1000_2040, 1.0) } else { (0x2020_2080, 0x2020_2080, 0x2020_8080, 0x1820_c0c0, 0x1820_80c0, 0.25) };
    let mut life = d.life as i32 as i16;
    if flags & 4 != 0 { life = ((life as i32 * (gold as i32 + 2)) / 2) as i16; }
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_v4(r, 0x20, vel);
    rec::set_u32(r, 4, c0);
    rec::set_ff(r, 0xc, size * 210000.0);
    rec::set_u32(r, 0x34, tween_color(d.c1.to_bits(), a1, b1));
    rec::set_u32(r, 0x38, tween_color(d.c2.to_bits(), a2, b2));
    r[3] = if glow { 0x48 } else { 0x44 };
    r[2] = tex;
    r[1] = 0;
    r[9] = 0x44;
    r[8] = d.rot;
    r[0x3e] = d.spin as i32 as u8;
    rec::set_ff(r, 0x30, len);
    r[0x3f] = flags;
    rec::set_i16(r, 0xa, life);
    r[0x3d] = life as u8;
    let f = Flame { pos: [pos[0], pos[1], pos[2]], vel: [vel[0], vel[1], vel[2]], len, life, size: size * 210000.0, flags, life0: life as u8 };
    Some((i, f))
}

/// `PartType12Update` 0x280408 on the record (no draws).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let gold = sys.gold;
    let r = &mut sys.pool.recs[i];
    r[8] = r[8].wrapping_add(r[0x3e]);
    let mut f = Flame {
        pos: rec::v3(r, 0x10),
        vel: rec::v3(r, 0x20),
        len: rec::ff(r, 0x30),
        life: rec::i16(r, 0xa),
        size: rec::ff(r, 0xc),
        flags: r[0x3f],
        life0: r[0x3d],
    };
    let alive = f.step(gold);
    rec::set_v3(r, 0x10, f.pos);
    rec::set_v3(r, 0x20, f.vel);
    rec::set_ff(r, 0x30, f.len);
    rec::set_ff(r, 0xc, f.size);
    // The colour of this tick (from the life before the countdown, as the game computes it first).
    let life = rec::i16(r, 0xa);
    let t = life as f32 / f.life0 as f32;
    let mut c = tween_color(t.to_bits(), rec::u32(r, 0x38), rec::u32(r, 0x34));
    if life < 15 {
        c = tween_color((life as f32 / 10.0).to_bits(), c & 0xff_ffff, c);
    }
    rec::set_u32(r, 4, c);
    rec::set_i16(r, 0xa, f.life);
    if !alive { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flame launched at 16 u/s with 12 units to go keeps its speed, then slows to rest at its end, and rises 1 u/s.
    #[test]
    fn flies_its_length_then_stops() {
        let v = 16.0 * DT;
        let mut f = Flame { pos: [100.0; 3], vel: [v, 0.0, 0.0], len: 12.0, life: 1000, size: 0.0, flags: 4, life0: 255 };
        let mut n = 0;
        while f.len > 0.0 && n < 400 {
            assert!(f.step(0));
            n += 1;
        }
        // The length is used up at the braking's end; what speed is left then dies away by 48·dt² a tick.
        let travelled = f.pos[0] - 100.0;
        assert!((12.0..12.0 + v).contains(&travelled), "travelled {travelled}");
        assert!(f.vel[0] < v * 0.5, "still at {}", f.vel[0] / DT);
        assert!((f.pos[2] - 100.0 - n as f32 * DT).abs() < 1e-3);
        // 12 units at 16 u/s: ~45 ticks plus the braking.
        assert!((45..60).contains(&n), "{n}");
    }

    /// The life counts down (twice as fast at rest); the size grows from 0.25 to 1.25 over it; out of bounds dies.
    #[test]
    fn life_size_and_bounds() {
        let mut f = Flame { pos: [100.0; 3], vel: [0.0; 3], len: 0.0, life: 20, size: 0.0, flags: 0, life0: 20 };
        assert!(f.step(0));
        assert_eq!(f.life, 18);
        assert!((f.size - 0.25 * 210000.0).abs() < 1.0);
        let mut g = Flame { pos: [1021.5, 100.0, 100.0], ..f };
        assert!(!g.step(0));
    }

    #[test]
    fn spawn_draw_order_and_record() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let mut probe = rng;
        let d = Draws::draw(&mut rng, 4);
        assert_eq!(d.life, probe.randf(20.0, 30.0));
        let _ = (probe.randf(0.0, 1.0), probe.randf(0.0, 1.0), probe.randi(8), probe.randi(0xff), probe.randf_sym(1.0, 4.0));
        assert_eq!(rng, probe);
        let (i, f) = spawn(&mut s, 5.0, [10.0, 10.0, 10.0, 0.0], [0.1, 0.0, 0.0, 0.0], 4, 0, &d).unwrap();
        let r = &s.pool.recs[i];
        assert_eq!((r[0], r[3], r[9], r[0x3f]), (TYPE, 0x44, 0x44, 4));
        assert_eq!(rec::i16(r, 0xa), d.life as i16);
        assert_eq!(f.len, 5.0);
        s.update_parts(&mut rng);
        assert!((rec::ff(&s.pool.recs[i], 0x10) - 10.1).abs() < 1e-5);
    }
}
