//! Particle type 34, the bubble (the hero under water: the hurt 0x76's burst, the swim's splash countdown, the
//! sinking floor), read from the level01 code:
//!
//! * spawner [`spawn`] = `PartType34Spawn(size, level, pos, vel)` 0x2840e0: kind-0 sprite, normal blend (0x44), byte9
//!   0x44 (near 1 u, far 128 u), texture `def[34][0]`, RGBA 0x88a0a0a0, start size 5250, the size cap
//!   `randf(0.75, 1.25)·size` (+0x2c), rotation `rand_range(−10, 10) − 0x80`, life `rand_range(ticks(70),
//!   ticks(140))`, the surface it pops at (+0x30: `level`, or with −1 the hero's z + 0.4 in the sinking floor 0x31,
//!   else the water level 0x13f640), the wobble angle `rand_angle` (+0x34), its rate `randf(1.745·dt, 6.98·dt)`
//!   (+0x38) and amplitude `randf(0.5·dt, 1.4·dt)` (+0x3c); position and velocity as given. Its six draws are made
//!   by the caller at the game's point ([`crate::hero::fx::bubbles`]) and passed in [`Draws`].
//! * [`update`] = 0x2842e0.
//!
//! Standard `f32`.

#![allow(clippy::needless_range_loop)] // record offsets by component, as the game's stores.
use super::{fast_dec_timer, rec, Particles, Record};
use crate::rng::Rng;

pub const TYPE: u8 = 34;

fn f(r: &Record, o: usize) -> f32 { f32::from_bits(rec::u32(r, o)) }
fn set(r: &mut Record, o: usize, v: f32) { rec::set_u32(r, o, v.to_bits()); }

/// The spawner's six draws, in its order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Draws {
    /// `randf(0.75, 1.25)`: the size cap factor.
    pub cap: f32,
    /// `rand_range(−10, 10)`: the rotation offset.
    pub rot: i32,
    /// `rand_range(ticks(70), ticks(140))`: the life.
    pub life: i32,
    /// `rand_angle`, `randf(1.745·dt, 6.98·dt)`, `randf(0.5·dt, 1.4·dt)`: the wobble's angle, rate and amplitude.
    pub angle: f32,
    pub rate: f32,
    pub amp: f32,
}

impl Draws {
    /// The six draws from `rng` (the spawner's order).
    pub fn draw(rng: &mut Rng) -> Draws {
        let cap = rng.randf(0.75, 1.25);
        let rot = rng.rand_range(-10, 10);
        let life = rng.rand_range(crate::hero::physics::ticks(70), crate::hero::physics::ticks(140));
        let angle = rng.rand_angle();
        let dt = 1.0 / 60.0;
        let rate = rng.randf(dt * 1.745_329_3, dt * 6.981_317);
        let amp = rng.randf(dt * 0.5, dt * 1.4);
        Draws { cap, rot, life, angle, rate, amp }
    }
}

/// `PartType34Spawn(size, level, pos, vel)` 0x2840e0 with its draws `d` and the pop level already resolved (`level`).
/// None when the pool is full.
pub fn spawn(sys: &mut Particles, size: f32, level: f32, pos: [f32; 4], vel: [f32; 3], d: &Draws) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let r = &mut sys.pool.recs[i];
    rec::set_u32(r, 4, 0x88a0_a0a0);
    r[1] = 0;
    r[3] = 0x44;
    r[9] = 4 + 0x40;
    set(r, 0x2c, d.cap * size);
    set(r, 0xc, f32::from_bits(0x45a4_1000));
    r[8] = (d.rot - 0x80) as u8;
    r[2] = def;
    rec::set_i16(r, 0xa, d.life as i16);
    set(r, 0x30, level);
    set(r, 0x34, d.angle);
    set(r, 0x38, d.rate);
    set(r, 0x3c, d.amp);
    for k in 0..3 { set(r, 0x20 + 4 * k, vel[k]); }
    for k in 0..4 { set(r, 0x10 + 4 * k, pos[k]); }
    Some(i)
}

fn wrap(a: f32) -> f32 {
    let (p, t) = (std::f32::consts::PI, std::f32::consts::TAU);
    let mut x = a;
    while x >= p { x -= t; }
    while x < -p { x += t; }
    x
}

/// Update 0x2842e0: killed on the timer; above its level it stops rising and gets at most `ticks(14) − 2` to live;
/// within 0.87 under it each tick `randi(20) == 0` cuts its life the same way (the one draw of the update); in its
/// last `ticks(14)` the alpha fades (`A·t / ticks(14)`); the size grows 840 a tick up to its cap; the horizontal
/// velocity loses 3 % a tick, pos += vel, the vertical velocity approaches 1.3 u/s by 5·dt² (it rises); then the
/// wobble: the angle turns by its rate and the bubble moves `sin(angle)·amp` sideways to the camera
/// ([`Particles::cam_yaw`] + 90°).
pub fn update(sys: &mut Particles, i: usize, rng: &mut Rng) {
    let speed = f32::from_bits(sys.time.speed);
    let t14 = sys.time.ticks(14);
    let cam_yaw = sys.cam_yaw;
    let r = &mut sys.pool.recs[i];
    if fast_dec_timer(r, 0xa) != 0 {
        sys.kill_part(i);
        return;
    }
    let (level, z) = (f(r, 0x30), f(r, 0x18));
    if level < z {
        set(r, 0x28, 0.0);
        if t14 - 2 < rec::i16(r, 0xa) as i32 { rec::set_i16(r, 0xa, (t14 - 2) as i16); }
    }
    if level < z + (1.0 / 60.0) * 1.3 * 40.0 && rng.randi(0x14) == 0 && t14 - 2 < rec::i16(r, 0xa) as i32 {
        rec::set_i16(r, 0xa, (t14 - 2) as i16);
    }
    let t = rec::i16(r, 0xa) as i32;
    if t < t14 && t14 != 0 {
        let a = (r[7] as i32 * t) / t14;
        rec::set_u32(r, 4, (a as u32) << 24 | (rec::u32(r, 4) & 0xff_ffff));
    }
    let s = (f(r, 0xc) + speed * 840.000_06).min(f(r, 0x2c));
    set(r, 0xc, s);
    let (mut vx, mut vy, mut vz) = (f(r, 0x20), f(r, 0x24), f(r, 0x28));
    let l = (vx * vx + vy * vy).sqrt();
    if l != 0.0 {
        let k = (speed * -0.029_999_971 * l + l) / l;
        vx *= k;
        vy *= k;
    }
    for (k, v) in [vx, vy, vz].into_iter().enumerate() { set(r, 0x10 + 4 * k, f(r, 0x10 + 4 * k) + v); }
    let (target, step) = ((1.0 / 60.0) * 1.3, (1.0 / 3600.0) * 5.0);
    vz += (target - vz).clamp(-step, step);
    set(r, 0x20, vx);
    set(r, 0x24, vy);
    set(r, 0x28, vz);
    let angle = wrap(f(r, 0x34) + f(r, 0x38));
    set(r, 0x34, angle);
    let side = wrap(cam_yaw + std::f32::consts::FRAC_PI_2);
    let d = angle.sin() * f(r, 0x3c);
    set(r, 0x10, f(r, 0x10) + side.cos() * d);
    set(r, 0x14, f(r, 0x14) + side.sin() * d);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bubble deep under the surface rises (its vertical speed reaching 1.3 u/s), grows to its cap and draws
    /// nothing; one above its level stops rising and pops within `ticks(14) − 2` ticks.
    #[test]
    fn bubble_rises_and_pops_at_the_surface() {
        let mut s = Particles::new(None, Vec::new());
        let d = Draws { cap: 1.0, rot: 0, life: 140, angle: 0.0, rate: 0.05, amp: 0.01 };
        let i = spawn(&mut s, 6000.0, 50.0, [100.0, 100.0, 40.0, 0.0], [0.0; 3], &d).unwrap();
        let mut rng = Rng::new();
        for _ in 0..60 { s.update_parts(&mut rng); }
        let r = &s.pool.recs[i];
        assert!((f(r, 0x28) - 1.3 / 60.0).abs() < 1e-6, "vz {}", f(r, 0x28) * 60.0);
        assert!(f(r, 0x18) > 40.8, "z {}", f(r, 0x18));
        assert_eq!(f(r, 0xc), 6000.0);
        assert_eq!(rng.state, Rng::new().state, "no draw far under the surface");
        let mut s = Particles::new(None, Vec::new());
        let i = spawn(&mut s, 6000.0, 50.0, [100.0, 100.0, 50.5, 0.0], [0.0, 0.0, 0.02], &d).unwrap();
        s.update_parts(&mut rng);
        assert_eq!(f(&s.pool.recs[i], 0x28), 1.3 / 60.0 * 0.0 + (5.0 / 3600.0), "vz reset to 0, then +5·dt²");
        let mut n = 1;
        while s.pool.count > 0 { s.update_parts(&mut rng); n += 1; }
        assert_eq!(n, 13, "capped to ticks(14) − 2 = 12 on the first update, killed 12 updates later");
    }
}
