//! Particle type 59, the short-lived hero sparkle (the nanotech cluster's healing ring around Ratchet,
//! `moby_update::classes::pickup`), read from the level01 code:
//!
//! * spawner [`spawn`] = `PartType59Spawn(size, pos, rgba, rot, tex_type, soft, life, attach)` 0x288410: kind-0
//!   sprite, byte9 `trunc(1.0) + 0x20`, blend 0x48 (0x44 when `soft` ≠ 0), rotation `rot`, size `size·210000`, frame
//!   `*def[tex_type]`, life `life`; `attach` 1 keeps the offset from the hero position 0x13f3d0 at +0x20
//!   ([`Particles::hero`]). No draw;
//! * [`update`] = 0x288530: attached → position = hero + offset; killed when the life timer runs out.

use super::{fast_dec_timer, rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 59;

/// `PartType59Spawn` 0x288410. None when the pool is full.
#[allow(clippy::too_many_arguments)]
pub fn spawn(sys: &mut Particles, size: f32, pos: [f32; 4], rgba: u32, rot: u8, tex_type: u8, soft: bool, life: i16, attach: i32) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(tex_type);
    let hero = sys.hero;
    let r = &mut sys.pool.recs[i];
    rec::set_u32(r, 0x30, attach as u32);
    if attach == 1 { rec::set_v3(r, 0x20, [pos[0] - hero[0], pos[1] - hero[1], pos[2] - hero[2]]); }
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, rgba);
    r[9] = 1 + 0x20;
    r[3] = if soft { 0x44 } else { 0x48 };
    r[8] = rot;
    r[1] = 0;
    rec::set_ff(r, 0xc, size * 210000.0);
    rec::set_i16(r, 0xa, life);
    r[2] = def;
    Some(i)
}

/// Update 0x288530 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let hero = sys.hero;
    let r = &mut sys.pool.recs[i];
    if rec::u32(r, 0x30) == 1 {
        let o = rec::v3(r, 0x20);
        rec::set_v3(r, 0x10, [hero[0] + o[0], hero[1] + o[1], hero[2] + o[2]]);
    }
    if fast_dec_timer(r, 0xa) != 0 { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-tick sparkle attached to the hero follows him and dies on its second update.
    #[test]
    fn follows_the_hero_and_dies_on_its_timer() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        s.hero = [100.0, 100.0, 50.0];
        let i = spawn(&mut s, 4.0, [100.0, 100.3, 51.0, 0.0], 0x407f_2020, 7, 53, false, 2, 1).unwrap();
        assert_eq!(s.pool.recs[i][3], 0x48);
        s.hero = [101.0, 100.0, 50.0];
        s.update_parts(&mut rng);
        assert_eq!(rec::pos(&s.pool.recs[i]), [101.0, 100.3, 51.0]);
        assert_eq!(s.pool.recs[i][1] & 0x80, 0);
        s.update_parts(&mut rng);
        assert_ne!(s.pool.recs[i][1] & 0x80, 0, "dead after its life");
    }
}
