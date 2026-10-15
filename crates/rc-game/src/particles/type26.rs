//! Particle type 26, the glow sprite that rides a moby (level01 spawner `PartType26Spawn` 0x282b00, update 0x282c40,
//! read from the decomp): the gunship's shells and the path enemies' shots (two each at launch).
//!
//! **Spawn** `(size, moby, rgba, life, joint)`: +0x20 the moby, +0x24 s16 timer = life, +0x26 s16 the joint list (−1:
//! the moby's position), +0x28 life, +0x2c the start alpha (`rgba >> 24`, arithmetic), position = the moby's (or the
//! joint list's point moved 0.4 toward the camera), rotation = the low byte of one raw `rand()`, RGBA, byte9 0x74,
//! ALPHA 0x48 (additive), size, sprite, texture `def[26][0]`.
//!
//! **Update**: the timer fires, or the moby is gone (its state ≥ 0x80) → killed; else in the last fifth of the life
//! alpha = `a0·t / (life/3)` (integer), and the position follows the moby (or its joint point, 0.4 toward the
//! camera). The moby's place comes from [`Particles::joint_anchors`], which the moby loop fills for the mobys the
//! live type-26 records name (`moby_update::services::World::refresh_particle_anchors`); a moby missing there is gone.
//! Standard `f32`.

use super::{fast_dec_timer, rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 26;

/// `PartType26Spawn(size, moby, rgba, life, joint)` 0x282b00 with the moby's point `at` (its position, or the joint
/// list's point already moved toward the camera): one `rand()` with a record.
#[allow(clippy::too_many_arguments)]
pub fn spawn(sys: &mut Particles, rng: &mut Rng, size: f32, moby: usize, rgba: u32, life: i32, joint: i16, at: [f32; 3]) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let rot = rng.rand() as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_u32(r, 0x20, moby as u32 + 1);
    rec::set_i16(r, 0x24, life as i16);
    rec::set_u32(r, 0x2c, ((rgba as i32) >> 24) as u32);
    rec::set_i16(r, 0x26, joint);
    rec::set_u32(r, 0x28, life as u32);
    rec::set_v3(r, 0x10, at);
    rec::set_u32(r, 4, rgba);
    r[8] = rot;
    r[9] = 4 + 0x70;
    r[3] = 0x48;
    rec::set_ff(r, 0xc, size);
    r[1] = 0;
    r[2] = def;
    Some(i)
}

/// The moby and joint list a live type-26 record follows.
pub fn anchor_of(r: &super::Record) -> Option<(usize, i16)> {
    (r[0] == TYPE && r[1] & super::FLAG_DEAD == 0).then(|| (rec::u32(r, 0x20) as usize).checked_sub(1).map(|m| (m, rec::i16(r, 0x26)))).flatten()
}

/// Update 0x282c40 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let anchor = anchor_of(&sys.pool.recs[i]).and_then(|k| sys.joint_anchors.get(&k).copied());
    let r = &mut sys.pool.recs[i];
    let (fired, Some(p)) = (fast_dec_timer(r, 0x24) != 0, anchor) else {
        sys.kill_part(i);
        return;
    };
    if fired {
        sys.kill_part(i);
        return;
    }
    let (t, life) = (rec::i16(r, 0x24) as i32, rec::u32(r, 0x28) as i32);
    if t < life / 5 && life / 3 != 0 {
        let a = (rec::u32(r, 0x2c) as i32 * t) / (life / 3);
        rec::set_u32(r, 4, (rec::u32(r, 4) & 0xff_ffff).wrapping_add((a as u32) << 24));
    }
    rec::set_v3(r, 0x10, p);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_its_moby_fades_and_dies_with_it() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn(&mut s, &mut rng, 5000.0, 7, 0x7f20_40ff, 30, -1, [1.0, 2.0, 3.0]).unwrap();
        s.joint_anchors.insert((7, -1), [4.0, 5.0, 6.0]);
        s.update_parts(&mut rng);
        assert_eq!(rec::pos(&s.pool.recs[i]), [4.0, 5.0, 6.0]);
        for _ in 0..24 { s.update_parts(&mut rng); }
        // t = 5 < 30/5: alpha = 0x7f·5/10.
        assert_eq!(rec::u32(&s.pool.recs[i], 4) >> 24, 0x7f * 5 / 10);
        s.joint_anchors.clear();
        s.update_parts(&mut rng);
        assert_eq!(s.pool.count, 0);
    }
}
