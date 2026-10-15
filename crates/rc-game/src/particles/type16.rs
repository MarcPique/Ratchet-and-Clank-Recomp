//! Particle type 16, the falling / rising smoke puff (level01 spawner `PartType16Spawn` 0x280f30, update 0x2810e8,
//! read from the disassembly): the burning buildings' smoke (class 700 `ImpactSmokeEmitterUpdate` 0x2f8c58), the bomb
//! scorch's child smoke (type 64) and its own landing smoke.
//!
//! **Spawn** `(size, pos, vel, c1, c2, life, kind)`: nothing for life 0; the frame-load throttle (`randi(3)` over 0.9,
//! `randi(2)` over 0.95, `randi(1)` over 1.0 of 0x15f5d0: a 0 drops the spawn, the caller's draws, see
//! [`throttled`]); then a record: pos, RGBA c1, sprite (byte1 0), size, ALPHA 0x44, byte9 0x44 (near 1 u, far 128 u),
//! rotation = the low byte of one raw `rand()`, texture `def[16][kind]` (kind 3: `def[16][0]`), timer +0x0a = +0x38 =
//! life, +0x3c size, +0x30 c1, +0x34 c2, +0x3a kind, velocity +0x20, and +0x2c = pos.z − 0.5 (the floor; callers
//! may patch it).
//!
//! **Update**: kind 3 with the frame load above 0.95 steps the timer twice; the timer fires → killed. Colour =
//! `FastTweenColor(t/N, c2, c1)`; its alpha: × t/`ticks(6)` in the last six ticks, killed within 4 units of the camera
//! (0x167240), × (d − 4)/4 within 8. Then pos += vel, vel.z −= g[kind]·dt² (g = 4.8, 24.5, 9.8, −0.24: kind 3
//! rises); below the floor: kind 1 turns into a kind-3 smoke puff (`randf(0.25, 1)`, `randf(0.5, 1)` tweens from
//! 0x7f000000 to 0x7f182030 / 0 to 0x5f5f5f, vel.z = 0, size `randf(200000, 300000)`, life `rand_range(ticks(90),
//! ticks(150))`) and every kind is killed; else rotation byte + 1. Standard `f32`.

use super::{fast_dec_timer, rec, tween_color, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 16;

/// The spawner's arguments.
#[derive(Clone, Copy, Debug)]
pub struct Spawn {
    pub size: f32,
    pub pos: [f32; 4],
    pub vel: [f32; 4],
    pub c1: u32,
    pub c2: u32,
    pub life: i32,
    pub kind: i16,
}

/// The throttle draws of `PartType16Spawn` for frame load `load` (0x15f5d0): true when the spawn is dropped. Made
/// by the caller (with or without a particle system) before [`spawn`].
pub fn throttled(rng: &mut Rng, life: i32, load: f32) -> bool {
    if life == 0 { return true; }
    for (k, n) in [(0.9f32, 3), (0.95, 2), (1.0, 1)] {
        if k < load && rng.randi(n) == 0 { return true; }
    }
    false
}

/// The record of `PartType16Spawn` (after [`throttled`]): one `rand()` with a record.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, a: &Spawn) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_frame(TYPE, if a.kind == 3 { 0 } else { a.kind.max(0) as usize });
    let rot = rng.rand() as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, a.pos);
    rec::set_u32(r, 4, a.c1);
    r[1] = 0;
    rec::set_ff(r, 0xc, a.size);
    r[3] = 0x44;
    r[9] = 4 + 0x40;
    r[8] = rot;
    rec::set_i16(r, 0xa, a.life as i16);
    r[2] = def;
    rec::set_i16(r, 0x38, a.life as i16);
    rec::set_ff(r, 0x3c, a.size);
    rec::set_u32(r, 0x30, a.c1);
    rec::set_u32(r, 0x34, a.c2);
    rec::set_i16(r, 0x3a, a.kind);
    rec::set_v4(r, 0x20, a.vel);
    rec::set_ff(r, 0x2c, a.pos[2] - 0.5);
    Some(i)
}

/// Update 0x2810e8.
pub fn update(sys: &mut Particles, i: usize, rng: &mut Rng) {
    let dt2 = f32::from_bits(sys.time.dt2);
    let g = [dt2 * 4.8, dt2 * 24.5, dt2 * 9.8, -(dt2 * 0.24)];
    let load = f32::from_bits(sys.frame_load[0]);
    let t6 = sys.time.ticks(6);
    let t90 = sys.time.ticks(90);
    let t150 = sys.time.ticks(150);
    let cam = sys.camera.map(f32::from_bits);
    let r = &mut sys.pool.recs[i];
    let kind = rec::i16(r, 0x3a);
    if kind == 3 && 0.95 < load { fast_dec_timer(r, 0xa); }
    if fast_dec_timer(r, 0xa) != 0 {
        sys.kill_part(i);
        return;
    }
    let t = rec::i16(r, 0xa) as i32;
    let n = rec::i16(r, 0x38) as i32;
    let c = tween_color((t as f32 / n as f32).to_bits(), rec::u32(r, 0x34), rec::u32(r, 0x30));
    rec::set_u32(r, 4, c);
    let mut a = (c as i32) >> 24;
    let p = rec::pos(r);
    let d = ((p[0] - cam[0]).powi(2) + (p[1] - cam[1]).powi(2) + (p[2] - cam[2]).powi(2)).sqrt();
    if t < t6 && t6 != 0 { a = a * t / t6; }
    if d < 4.0 {
        sys.kill_part(i);
        return;
    }
    if d < 8.0 { a = (a as f32 * ((d - 4.0) * 0.25)) as i32; }
    rec::set_u32(r, 4, (a as u32) << 24 | c & 0xff_ffff);
    let v = rec::v3(r, 0x20);
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    rec::set_ff(r, 0x28, v[2] - g[(kind.clamp(0, 3)) as usize]);
    let pos = rec::pos(r);
    if pos[2] < rec::ff(r, 0x2c) {
        if kind == 1 {
            let f1 = rng.randf(0.25, 1.0);
            let c1 = tween_color(f1.to_bits(), 0x7f00_0000, 0x7f18_2030);
            let f2 = rng.randf(0.5, 1.0);
            let c2 = tween_color(f2.to_bits(), 0, 0x5f_5f5f);
            rec::set_ff(r, 0x28, 0.0);
            let vel = rec::v3(r, 0x20);
            let size = rng.randf(200000.0, 300000.0);
            let life = rng.rand_range(t90, t150);
            let a = Spawn { size, pos: [pos[0], pos[1], pos[2], 0.0], vel: [vel[0], vel[1], vel[2], 0.0], c1, c2, life, kind: 3 };
            if !throttled(rng, life, load) { spawn(sys, rng, &a); }
        }
        sys.kill_part(i);
        return;
    }
    r[8] = r[8].wrapping_add(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sys() -> Particles {
        let mut s = Particles::new(None, Vec::new());
        s.camera = [100.0f32, 0.0, 0.0].map(f32::to_bits);
        s
    }

    #[test]
    fn falls_fades_and_dies_on_the_floor() {
        let (mut s, mut rng) = (sys(), Rng::new());
        let a = Spawn { size: 1000.0, pos: [0.0, 0.0, 10.0, 0.0], vel: [0.0, 0.0, 0.0, 0.0], c1: 0x0f08_1020, c2: 0x0008_1020, life: 60, kind: 2 };
        let i = spawn(&mut s, &mut rng, &a).unwrap();
        assert_eq!(rec::ff(&s.pool.recs[i], 0x2c), 9.5);
        let mut n = 0;
        while s.pool.count > 0 && n < 100 {
            s.update_parts(&mut rng);
            n += 1;
        }
        // 9.8·dt² a tick after each move: z = 10 − 9.8/3600·k(k−1)/2 < 9.5 on the 20th update.
        assert_eq!(n, 20);
    }

    #[test]
    fn kind_one_leaves_rising_smoke() {
        let (mut s, mut rng) = (sys(), Rng::new());
        let a = Spawn { size: 1000.0, pos: [0.0, 0.0, 10.0, 0.0], vel: [0.1, 0.0, -1.0, 0.0], c1: 0x7f00_00ff, c2: 0, life: 60, kind: 1 };
        spawn(&mut s, &mut rng, &a);
        s.update_parts(&mut rng);
        let live: Vec<_> = s.pool.live().map(|(_, r)| (r[0], rec::i16(r, 0x3a), rec::ff(r, 0x28))).collect();
        assert_eq!(live, [(16, 3, 0.0)]);
    }

    #[test]
    fn fades_near_the_camera() {
        let (mut s, mut rng) = (sys(), Rng::new());
        s.camera = [6.0f32, 0.0, 10.0].map(f32::to_bits);
        let a = Spawn { size: 1000.0, pos: [0.0, 0.0, 10.0, 0.0], vel: [0.0; 4], c1: 0x7c00_0000, c2: 0x7c00_0000, life: 60, kind: 3 };
        let i = spawn(&mut s, &mut rng, &a).unwrap();
        s.update_parts(&mut rng);
        // d = 6: the tweened alpha (0x7c at t/N = 59/60 truncates to 123 on the VU0 model) · (6 − 4)/4 = 61 (the alpha
        // is read as a signed byte, `sra 24`: the callers keep it below 0x80).
        assert_eq!(rec::u32(&s.pool.recs[i], 4) >> 24, 61);
    }
}
