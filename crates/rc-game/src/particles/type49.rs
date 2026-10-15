//! Particle type 49, the tracer line (level01 update `PartType49Update` 0x286eb8, the same code on all 19 levels (the
//! level-05 `U` of `overlay-diff` is the matcher pairing a neighbouring spawner [L]); spawner level03 `0x260130`, one
//! code cluster on levels 03, 07, 15; read from the decomp). The census's callers are unported classes (G-PRT-001).
//!
//! **Spawn** `(moby a0, tail a1, head a2, vel a3, timer t0)` (level03 `0x260130`): RGBA2 (+0x0c) 0x0fffffff, RGBA1
//! 0x7fffffff, ALPHA 0x44, byte9 `trunc(2) + 0x70` = 0x72, **render kind 2** (the untextured line), end 1 (+0x10) =
//! head, end 2 (+0x20) = tail, velocity +0x30 = vel.xyz, +0x3c = the moby (the line test ignores it), timer. No RNG.
//!
//! **Update**: both ends += (vel, 0); the timer fires → killed; else `CollLine_Fix(tail, head, 0x10, moby)`: a hit →
//! the timer 0 (killed on the next update, `FastDecTimer` of 0) and the head (xyzw) = the hit point. The line tests the
//! world mesh ([`Particles::coll`]; the game's call also tests the other mobys, not here [L]). No RNG.

use super::{fast_dec_timer, rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 49;

/// Level03 `0x260130(moby, tail, head, vel, timer)` (no RNG).
pub fn spawn(sys: &mut Particles, moby: u32, tail: [f32; 4], head: [f32; 4], vel: [f32; 3], timer: i16) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let r = &mut sys.pool.recs[i];
    rec::set_u32(r, 0xc, 0x0fff_ffff);
    rec::set_u32(r, 4, 0x7fff_ffff);
    r[3] = 0x44;
    r[9] = 2 + 0x70;
    r[1] = 2;
    rec::set_v4(r, 0x10, head);
    rec::set_v4(r, 0x20, tail);
    rec::set_v3(r, 0x30, vel);
    rec::set_u32(r, 0x3c, moby);
    rec::set_i16(r, 10, timer);
    Some(i)
}

/// Update 0x286eb8 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let coll = sys.coll.clone();
    let r = &mut sys.pool.recs[i];
    let v = rec::v3(r, 0x30);
    for o in [0x10, 0x20] {
        let p = rec::v3(r, o);
        rec::set_v3(r, o, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    }
    if fast_dec_timer(r, 10) != 0 {
        sys.kill_part(i);
        return;
    }
    let (tail, head) = (rec::v3(r, 0x20), rec::v3(r, 0x10));
    let hit = coll.as_deref().and_then(|c| crate::collision_query::coll_line(c, tail, head, crate::collision_query::QueryFlags(0x10)));
    if let Some(h) = hit {
        rec::set_i16(r, 10, 0);
        rec::set_v3(r, 0x10, h.point);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The line flies by its velocity for its timer (no mesh: no hit) and draws as kind 2.
    #[test]
    fn tracer_flies_its_timer() {
        let mut s = Particles::new(None, Vec::new());
        let i = spawn(&mut s, 0, [0.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0], [0.5, 0.0, 0.0], 4).unwrap();
        let r = &s.pool.recs[i];
        assert_eq!((r[1], r[3], r[9], rec::u32(r, 4), rec::u32(r, 0xc)), (2, 0x44, 0x72, 0x7fff_ffff, 0x0fff_ffff));
        let mut rng = Rng::new();
        s.update_parts(&mut rng);
        assert_eq!((rec::pos(&s.pool.recs[i]), rec::v3(&s.pool.recs[i], 0x20)), ([1.5, 0.0, 0.0], [0.5, 0.0, 0.0]));
        let mut n = 1;
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            n += 1;
        }
        assert_eq!(n, 4);
    }
}
