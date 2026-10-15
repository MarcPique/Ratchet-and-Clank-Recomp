//! Particle type 35, the water drop (level01 spawner `PartType35Spawn` 0x2845a8, update 0x2846c0, read from the
//! decomp): the Plumber's splash in the Novalis scene 1 (`CutsceneFxUpdate`), the bomb's water entry.
//!
//! **Spawn** `(pos, vel, kind, life)`: RGBA 0x80808080, sprite, byte9 0x12 (near 0.5 u, far 32 u), ALPHA 0x44, size
//! `randf(10500, 16800)`, texture `def[35][0]`, velocity +0x20, life +0x34, kind +0x30, the floor +0x38 = pos.z (0
//! for kind 2), gravity +0x3c = `randf(10.5·dt², 18·dt²)` (two draws, with a record).
//!
//! **Update** by kind: 0 — pos += vel; 1 — the step as a world line (`CollLine_Fix(pos, pos + vel, 2)`): on a hit of
//! surface 0 a type-45 ring at the hit (`PartType45Spawn(0.2, 5250, hit, 0, −1)`), killed on any hit; 3 — pos += vel,
//! below the water level (0x13f640) it sits on it, leaves a type-45 ring there and is killed; 2 (and others) — the
//! line: on a hit of a surface other than 0 the hit point is raised 0.05 and a type-66 ring spawns there
//! (`PartType66Spawn(0.25, 3150, hit, −1)`), killed on any hit. Then (no hit): vel.z −= g, rotation + 2, life − 1, and
//! once the life is out or the drop fell below its floor the alpha drops 0x10 a tick; killed below 0. Standard `f32`.

use super::{rec, type45, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 35;

/// `PartType35Spawn(pos, vel, kind, life)` 0x2845a8.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, pos: [f32; 4], vel: [f32; 4], kind: i32, life: i32) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let dt2 = f32::from_bits(sys.time.dt2);
    let size = rng.randf(10500.0, 16800.0);
    let g = rng.randf(dt2 * 10.5, dt2 * 18.0);
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, 0x8080_8080);
    r[1] = 0;
    r[9] = 2 + 0x10;
    r[3] = 0x44;
    rec::set_ff(r, 0xc, size);
    r[2] = def;
    rec::set_v4(r, 0x20, vel);
    rec::set_u32(r, 0x34, life as u32);
    rec::set_u32(r, 0x30, kind as u32);
    rec::set_ff(r, 0x38, if kind == 2 { 0.0 } else { pos[2] });
    rec::set_ff(r, 0x3c, g);
    Some(i)
}

fn line(sys: &Particles, a: [f32; 3], b: [f32; 3]) -> Option<([f32; 3], i32)> {
    let c = sys.coll.as_deref()?;
    let h = crate::collision_query::coll_line_m(c, None, a, b, crate::collision_query::QueryFlags(2), None)?;
    Some((h.point, h.surface_id()))
}

/// Update 0x2846c0.
pub fn update(sys: &mut Particles, i: usize, rng: &mut Rng) {
    let r = &sys.pool.recs[i];
    let kind = rec::u32(r, 0x30) as i32;
    let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
    let np = [p[0] + v[0], p[1] + v[1], p[2] + v[2]];
    match kind {
        0 => {}
        3 => {
            if np[2] < sys.water_z {
                let at = [np[0], np[1], sys.water_z, rec::ff(r, 0x1c)];
                sys.pool.recs[i][0x10..0x1c].copy_from_slice(&[at[0], at[1], at[2]].map(f32::to_le_bytes).concat());
                type45::spawn45(sys, rng, 0.2, 5250.0, at, u32::MAX);
                sys.kill_part(i);
                return;
            }
        }
        _ => {
            if let Some((hit, surface)) = line(sys, p, np) {
                if kind == 1 {
                    if surface == 0 { type45::spawn45(sys, rng, 0.2, 5250.0, [hit[0], hit[1], hit[2], 0.0], u32::MAX); }
                } else if surface != 0 {
                    type45::spawn66(sys, rng, 0.25, 3150.0, [hit[0], hit[1], hit[2] + 0.05, 0.0], u32::MAX);
                }
                sys.kill_part(i);
                return;
            }
        }
    }
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, np);
    rec::set_ff(r, 0x28, v[2] - rec::ff(r, 0x3c));
    r[8] = r[8].wrapping_add(2);
    let life = rec::u32(r, 0x34) as i32 - 1;
    rec::set_u32(r, 0x34, life as u32);
    if life < 1 || np[2] < rec::ff(r, 0x38) {
        let c = (rec::u32(r, 4) as i32).wrapping_sub(0x1000_0000);
        rec::set_u32(r, 4, c as u32);
        if c < 0 { sys.kill_part(i); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kind_zero_drop_falls_then_fades_below_its_start() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn(&mut s, &mut rng, [5.0, 5.0, 5.0, 0.0], [0.0, 0.0, 0.05, 0.0], 0, 100).unwrap();
        let mut n = 0;
        while s.pool.count > 0 && n < 200 {
            s.update_parts(&mut rng);
            n += 1;
        }
        // Up, back below 5 after a few dozen ticks, then 8 fading ticks (0x80 → −0x10).
        assert!(n > 20 && n < 100, "{n}");
        assert!(rec::ff(&s.pool.recs[i], 0x18) < 5.0);
    }

    #[test]
    fn a_kind_three_drop_rings_on_the_water() {
        let mut s = Particles::new(None, Vec::new());
        s.water_z = 4.9;
        let mut rng = Rng::new();
        spawn(&mut s, &mut rng, [5.0, 5.0, 5.0, 0.0], [0.0, 0.0, -0.2, 0.0], 3, 100).unwrap();
        s.update_parts(&mut rng);
        let live: Vec<u8> = s.pool.live().map(|(_, r)| r[0]).collect();
        assert_eq!(live, [45]);
    }
}
