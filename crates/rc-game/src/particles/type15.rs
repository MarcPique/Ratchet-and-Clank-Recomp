//! Particle type 15, the explosion streak (level01 spawner `PartType15Spawn` 0x280bd0, update 0x280d08): the creature
//! explosions' streaks (`SpawnBeamExplosion` 0x273310) and the bomb's under-water sparks. A streak with the split flag
//! leaves a trail of child streaks, one on every odd tick.
//!
//! **Record** (only for life ≠ 0): position +0x10, velocity +0x20, colour 1 at +0x04 / +0x30, colour 2 at +0x34, byte9
//! 0xa4, byte3 = the blend argument (−1 → 0x48), byte1 0 (sprite), size and +0x3c = `size`, rotation byte = the low byte
//! of one raw `rand()` (**one draw**, with a record), texture = the def argument (−1 → `*def[15]`), +0x0a / +0x38 life,
//! +0x3a split flag.
//!
//! **Update**: `FastDecTimer` first (kill when it fires); size = `t·size0/life`, colour = `FastTweenColor(t/life, c2,
//! c1)`, pos += velocity, vel.z −= 14.6·dt². Without the split flag, or on even ticks of 0x15f5cc: velocity ·= 0.96.
//! Otherwise a child: a random direction (3 × `randf(−1, 1)`) of length `|vel|·randf(0.15, 0.25)`, plus the velocity,
//! times `randf(0.75, 0.95)`, life `ticks(rand_range(15, 30))`, spawned with the current size and colour, colour 2, no
//! split, the parent's texture and blend (6 draws, then the child's own).

use super::{fast_dec_timer, rec, tween_color, Particles};
use crate::rng::Rng;

/// The spawner's arguments.
#[derive(Clone, Copy, Debug)]
pub struct Spawn {
    pub size: f32,
    pub pos: [f32; 4],
    pub vel: [f32; 4],
    pub c1: u32,
    pub c2: u32,
    pub life: i32,
    pub split: i16,
    /// −1: `*def[15]`.
    pub def: i32,
    /// −1: 0x48.
    pub blend: i32,
}

/// `PartType15Spawn` 0x280bd0.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, a: &Spawn) -> Option<usize> {
    if a.life == 0 { return None; }
    let i = sys.create_part(15)?;
    let def = if a.def == -1 { sys.def_first(15) } else { a.def as u8 };
    let rot = rng.rand() as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, a.pos);
    rec::set_u32(r, 4, a.c1);
    r[9] = 0xa4;
    r[3] = if a.blend == -1 { 0x48 } else { a.blend as u8 };
    r[1] = 0;
    rec::set_ff(r, 0xc, a.size);
    r[8] = rot;
    r[2] = def;
    rec::set_i16(r, 0xa, a.life as i16);
    rec::set_i16(r, 0x38, a.life as i16);
    rec::set_ff(r, 0x3c, a.size);
    rec::set_u32(r, 0x30, a.c1);
    rec::set_u32(r, 0x34, a.c2);
    rec::set_i16(r, 0x3a, a.split);
    rec::set_v4(r, 0x20, a.vel);
    Some(i)
}

fn norm_to(v: [f32; 3], l: f32) -> [f32; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n == 0.0 { [0.0; 3] } else { v.map(|x| x * l / n) }
}

/// 0x280d08.
pub fn update(sys: &mut Particles, i: usize, rng: &mut Rng) {
    let dt2 = f32::from_bits(sys.time.dt2);
    let odd = sys.counter & 1 != 0;
    let r = &mut sys.pool.recs[i];
    if fast_dec_timer(r, 0xa) != 0 { sys.kill_part(i); return; }
    let (t, life) = (rec::i16(r, 0xa) as f32, rec::i16(r, 0x38) as f32);
    rec::set_ff(r, 0xc, (t * rec::ff(r, 0x3c)) / life);
    let c = tween_color((t / life).to_bits(), rec::u32(r, 0x34), rec::u32(r, 0x30));
    rec::set_u32(r, 4, c);
    let mut v = rec::v3(r, 0x20);
    let p = rec::v3(r, 0x10);
    let p = [p[0] + v[0], p[1] + v[1], p[2] + v[2]];
    rec::set_v3(r, 0x10, p);
    v[2] -= dt2 * 14.6;
    if rec::i16(r, 0x3a) == 0 || !odd {
        rec::set_v3(r, 0x20, v.map(|x| x * f32::from_bits(0x3f75_c28f)));
        return;
    }
    rec::set_v3(r, 0x20, v);
    let (size, c2, def, blend) = (rec::ff(r, 0xc), rec::u32(r, 0x34), r[2] as i32, r[3] as i32);
    let d = [rng.randf(-1.0, 1.0), rng.randf(-1.0, 1.0), rng.randf(-1.0, 1.0)];
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() * rng.randf(0.15, 0.25);
    let d = norm_to(d, len);
    let k = rng.randf(0.75, 0.95);
    let cv = [0, 1, 2].map(|j| (d[j] + v[j]) * k);
    let n = rng.rand_range(15, 30);
    let life_c = sys.time.ticks(n);
    let a = Spawn { size, pos: [p[0], p[1], p[2], 0.0], vel: [cv[0], cv[1], cv[2], 0.0], c1: c, c2, life: life_c, split: 0, def, blend };
    spawn(sys, rng, &a);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_streak_spawns_children_on_odd_ticks() {
        let mut sys = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let a = Spawn { size: 40000.0, pos: [10.0, 10.0, 10.0, 0.0], vel: [0.1, 0.0, 0.1, 0.0], c1: 0x4f00_7fff, c2: 0x1f00_007f, life: 60, split: 1, def: -1, blend: -1 };
        spawn(&mut sys, &mut rng, &a).unwrap();
        for c in 0..10u64 {
            sys.counter = c;
            sys.update_parts(&mut rng);
        }
        // Updates on odd counters 1, 3, 5, 7, 9 each spawn one child (children do not split).
        assert_eq!(sys.live_by_type()[15], 6);
        assert_eq!(sys.pool.recs[0][3], 0x48);
    }
}
