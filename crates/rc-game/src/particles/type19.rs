//! Particle types 19 and 55, the textured ribbons (render kind 3; level01 spawners `PartType19Spawn` 0x281748 /
//! `PartType55Spawn` 0x2878a0, updates 0x281850 / 0x2879a0, read from the decomp).
//!
//! **19**, the streak `(pos, vel)`: RGBA `0x78404000 | (0x40 + 6·randi(6))` at both ends (+0x04 / +0x0c), byte9 0x24,
//! additive, kind 3, texture `def[19][0]`, end 1 = pos (+0x10), velocity +0x30, end 2 = pos − 2·vel (+0x20), width
//! 0.09 (+0x1c) and end-2 factor 0.5 (+0x2c). Update: both ends += vel, both alphas − 6 (`−0x06000000` on the word);
//! killed when it goes negative. Spawned by the Comet-Strike's hits (`0x2bdd20`) and `0x2c72c8`.
//!
//! **55**, the joint ribbon `(w1, k, moby, list, rgba1, rgba2)`: RGBA1 / RGBA2, byte9 0x20, additive, kind 3, texture
//! `def[55][0]`, end 1 = joint list `list`'s point of the moby, end 2 = end 1, width w1 (+0x1c, +0x3c), factor k
//! (+0x2c), the moby (+0x30), the list (+0x34), its class (+0x38). Update: killed when the moby is gone or its class
//! changed (or its state is −2 / −3); else end 2 = the old end 1, end 1 = the joint point, and end 2 is pulled to a
//! distance of `max(|end2 − end1|, 0.0625) + randf(−0.021, 0.021)` from end 1; width = w1·`randf(0.5, 1)`. The joint
//! points come from [`Particles::joint_anchors`] (`(moby, list)`; the moby loop fills them). Standard `f32`.

use super::{rec, Particles};
use crate::rng::Rng;

/// `PartType19Spawn(pos, vel)` 0x281748: one `randi(6)` with a record.
pub fn spawn19(sys: &mut Particles, rng: &mut Rng, pos: [f32; 4], vel: [f32; 4]) -> Option<usize> {
    let i = sys.create_part(19)?;
    let def = sys.def_first(19);
    let c = (rng.randi(6) * 6 + 0x40) as u32 | 0x7840_4000;
    let r = &mut sys.pool.recs[i];
    rec::set_u32(r, 0xc, c);
    rec::set_u32(r, 4, c);
    r[9] = 4 + 0x20;
    r[3] = 0x48;
    r[1] = 3;
    r[2] = def;
    rec::set_v4(r, 0x10, pos);
    rec::set_v4(r, 0x30, vel);
    rec::set_v4(r, 0x20, [pos[0] - 2.0 * vel[0], pos[1] - 2.0 * vel[1], pos[2] - 2.0 * vel[2], pos[3] - 2.0 * vel[3]]);
    rec::set_ff(r, 0x1c, f32::from_bits(0x3db8_51ec));
    rec::set_ff(r, 0x2c, 0.5);
    Some(i)
}

/// Update 0x281850 (no RNG).
pub fn update19(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    let v = rec::v3(r, 0x30);
    for o in [0x10, 0x20] {
        let p = rec::v3(r, o);
        rec::set_v3(r, o, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    }
    let c = (rec::u32(r, 4) as i32).wrapping_sub(0x0600_0000);
    rec::set_u32(r, 4, c as u32);
    rec::set_u32(r, 0xc, c as u32);
    if c < 0 { sys.kill_part(i); }
}

/// `PartType55Spawn(w1, k, moby, list, rgba1, rgba2)` 0x2878a0 with the joint point `at` and the moby's class.
#[allow(clippy::too_many_arguments)]
pub fn spawn55(sys: &mut Particles, w1: f32, k: f32, moby: usize, list: i16, class: i16, rgba1: u32, rgba2: u32, at: [f32; 3]) -> Option<usize> {
    let i = sys.create_part(55)?;
    let def = sys.def_first(55);
    let r = &mut sys.pool.recs[i];
    rec::set_u32(r, 4, rgba1);
    rec::set_u32(r, 0xc, rgba2);
    r[9] = 0x20;
    r[1] = 3;
    r[3] = 0x48;
    r[2] = def;
    rec::set_v3(r, 0x10, at);
    rec::set_v3(r, 0x20, at);
    rec::set_ff(r, 0x2c, k);
    rec::set_ff(r, 0x1c, w1);
    rec::set_u32(r, 0x30, moby as u32 + 1);
    rec::set_i16(r, 0x38, class);
    rec::set_ff(r, 0x3c, w1);
    rec::set_i16(r, 0x34, list);
    Some(i)
}

/// The moby and joint list a live type-55 record follows.
pub fn anchor55(r: &super::Record) -> Option<(usize, i16)> {
    (r[0] == 55 && r[1] & super::FLAG_DEAD == 0).then(|| (rec::u32(r, 0x30) as usize).checked_sub(1).map(|m| (m, rec::i16(r, 0x34)))).flatten()
}

/// Update 0x2879a0 (2 draws a tick while alive). The class check is the anchor's: a moby whose class changed is
/// not listed under its old record.
pub fn update55(sys: &mut Particles, i: usize, rng: &mut Rng) {
    let Some(p) = anchor55(&sys.pool.recs[i]).and_then(|k| sys.joint_anchors.get(&k).copied()) else {
        sys.kill_part(i);
        return;
    };
    let f = rng.randf(0.5, 1.0);
    let r = &mut sys.pool.recs[i];
    let w = rec::ff(r, 0x3c) * f;
    let old = rec::v3(r, 0x10);
    rec::set_v3(r, 0x10, p);
    let d = [old[0] - p[0], old[1] - p[1], old[2] - p[2]];
    let jitter = rng.randf(f32::from_bits(0xbcac_0831), f32::from_bits(0x3cac_0831));
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let l = if 0.0625 <= len { len } else { 0.0625 } + jitter;
    let d = if len == 0.0 { d } else { d.map(|x| x * l / len) };
    rec::set_v3(r, 0x20, [p[0] + d[0], p[1] + d[1], p[2] + d[2]]);
    rec::set_ff(r, 0x1c, w);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_streak_moves_and_fades() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn19(&mut s, &mut rng, [5.0, 5.0, 5.0, 0.0], [0.1, 0.0, 0.0, 0.0]).unwrap();
        assert_eq!(rec::v3(&s.pool.recs[i], 0x20), [4.8, 5.0, 5.0]);
        let mut n = 0;
        while s.pool.count > 0 && n < 100 {
            s.update_parts(&mut rng);
            n += 1;
        }
        // Alpha 0x78 − 6·n < 0 at n = 21.
        assert_eq!(n, 21);
    }

    #[test]
    fn a_joint_ribbon_trails_its_joint() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn55(&mut s, 0.05, 0.01, 3, 1, 700, 0x8080_8080, 0x1080_8080, [0.0, 0.0, 0.0]).unwrap();
        s.joint_anchors.insert((3, 1), [1.0, 0.0, 0.0]);
        s.update_parts(&mut rng);
        let r = &s.pool.recs[i];
        assert_eq!(rec::v3(r, 0x10), [1.0, 0.0, 0.0]);
        let e2 = rec::v3(r, 0x20);
        assert!((e2[0] - 0.0).abs() < 0.03 && e2[1] == 0.0);
        s.joint_anchors.clear();
        s.update_parts(&mut rng);
        assert_eq!(s.pool.count, 0);
    }
}
