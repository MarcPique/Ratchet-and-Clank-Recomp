//! Particle type 61, the pulsing glow on a moby's joint, pulled toward the camera (level01 update `PartType61Update`
//! 0x2888f0, the same code on all 19 levels, `overlay-diff`; spawner level07 `0x29b8d8`, one cluster on levels 07, 15,
//! 17 (`clusters.tsv` dd572bc4…; level01 has none); read from the decomp). The census's unit U254 (class 1106, level
//! 07) calls the spawner (G-PRT-001 consumer; its class is not ported).
//!
//! **Spawn** `(size f12, pull f13, moby a0, joint a1)`: +0x20 the moby, timer `ticks(80)`, alpha +0x24 = 0, spin
//! +0x28 = `randf(0, 1)`, pull +0x34, frame index +0x2c = `rand() & 15`, rotation = one raw `rand()` (then overwritten
//! by `trunc(spin·255)`), RGBA 0x7f7f7f (alpha 0), byte9 `trunc(4) + 0x40` = 0x44, ALPHA 0x48, byte1 0, size, texture
//! `def[61][index]`, +0x2e the joint list, alpha step +0x30 = 0.015·speed; position = the joint's point (`0x2645a8`)
//! moved `pull` toward the camera (level07 0x166e40 = level01 0x167240). Draws: randf, rand, rand.
//!
//! **Update**: the timer fires, the moby pointer is 0 or its state ≥ 0x80 → killed. Else the frame index steps (mod
//! 16); the size eases to 159600 (+3990 a tick from below, −15960 from above, clamped there); spin += 0.001 (rotation
//! `trunc(spin·255)`); texture `def[61][index]`; at size ≤ 159600, or while rising, alpha += step; above the limit (0.3
//! at size ≤ 159600, else 0.15) the step turns and is taken back; below 0 → killed; A = `trunc(alpha·256)` added over
//! the colour; pull += (1 − pull)·0.175; position = the joint point moved `pull` toward the camera. No RNG.
//!
//! **The moby.** +0x20 keeps the moby index + 1; its state comes from [`Particles::moby_frames`] and its joint from
//! [`Particles::joint_frames`] (a joint missing there is the moby's origin), the camera from [`Particles::camera`].
//! Native `f32`.

use super::{fast_dec_timer, rec, Particles, Record};
use crate::rng::Rng;

pub const TYPE: u8 = 61;
const REST: f32 = 159600.0;

fn toward(p: [f32; 3], cam: [f32; 3], l: f32) -> [f32; 3] {
    let d = [cam[0] - p[0], cam[1] - p[1], cam[2] - p[2]];
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if n > 0.0 { [p[0] + d[0] * l / n, p[1] + d[1] * l / n, p[2] + d[2] * l / n] } else { p }
}

fn camera(sys: &Particles) -> [f32; 3] { sys.camera.map(f32::from_bits) }

/// Level07 `0x29b8d8(size, pull, moby, joint)` with the joint's point: three draws with a record; none when the pool
/// is full.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, size: f32, pull: f32, moby: usize, joint: i16, at: [f32; 3]) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let t80 = sys.time.ticks(0x50) as i16;
    let spin = rng.randf(0.0, 1.0);
    let k = (rng.rand() & 0xf) as i16;
    let _rot = rng.rand();
    let tex = sys.def_frame(TYPE, k as usize);
    let pos = toward(at, camera(sys), pull);
    let r = &mut sys.pool.recs[i];
    rec::set_u32(r, 0x20, moby as u32 + 1);
    rec::set_i16(r, 10, t80);
    rec::set_ff(r, 0x24, 0.0);
    rec::set_ff(r, 0x28, spin);
    rec::set_ff(r, 0x34, pull);
    rec::set_i16(r, 0x2c, k);
    rec::set_u32(r, 4, 0x7f7f7f);
    r[9] = 4 + 0x40;
    r[3] = 0x48;
    r[1] = 0;
    rec::set_ff(r, 0xc, size);
    r[2] = tex;
    rec::set_i16(r, 0x2e, joint);
    rec::set_ff(r, 0x30, 0.015);
    r[8] = (spin * 255.0) as i32 as u8;
    rec::set_v3(r, 0x10, pos);
    Some(i)
}

/// The moby and joint list a live type-61 record holds.
pub fn joint_of(r: &Record) -> Option<(usize, u8)> {
    (r[0] == TYPE && r[1] & super::FLAG_DEAD == 0).then(|| (rec::u32(r, 0x20) as usize).checked_sub(1).map(|m| (m, rec::i16(r, 0x2e) as u8))).flatten()
}

/// Update 0x2888f0 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let held = joint_of(&sys.pool.recs[i]).and_then(|(m, l)| {
        let f = sys.moby_frames.get(&m)?;
        (f.state < 0x80).then(|| sys.joint_frames.get(&(m, l)).map_or(f.pos, |j| [j[3][0], j[3][1], j[3][2]]))
    });
    let cam = camera(sys);
    let r = &mut sys.pool.recs[i];
    let fired = fast_dec_timer(r, 10) != 0;
    let Some(point) = held.filter(|_| !fired) else {
        sys.kill_part(i);
        return;
    };
    let k = (rec::i16(r, 0x2c) + 1) & 0xf;
    rec::set_i16(r, 0x2c, k);
    let s = rec::ff(r, 0xc);
    if s < REST {
        let s = s + 3989.9998;
        rec::set_ff(r, 0xc, if REST < s { REST } else { s });
    } else if REST < s {
        let s = s - 15959.999;
        rec::set_ff(r, 0xc, if s < REST { REST } else { s });
    }
    let spin = rec::ff(r, 0x28) + 0.001;
    rec::set_ff(r, 0x28, spin);
    r[8] = (spin * 255.0) as i32 as u8;
    let size = rec::ff(r, 0xc);
    let step = rec::ff(r, 0x30);
    if size <= REST || 0.0 < step { rec::set_ff(r, 0x24, rec::ff(r, 0x24) + step); }
    let a = rec::ff(r, 0x24);
    let lim = if size <= REST { 0.3 } else { 0.15 };
    if lim < a {
        let st = rec::ff(r, 0x30);
        rec::set_ff(r, 0x30, -st);
        rec::set_ff(r, 0x24, a + -st);
    } else if a < 0.0 {
        sys.kill_part(i);
        return;
    }
    let a = (rec::ff(r, 0x24) * 256.0) as i32;
    rec::set_u32(r, 4, (rec::u32(r, 4) & 0xff_ffff).wrapping_add((a as u32).wrapping_mul(0x100_0000)));
    let pull = rec::ff(r, 0x34);
    let pull = pull + (1.0 - pull) * 0.175;
    rec::set_ff(r, 0x34, pull);
    rec::set_v3(r, 0x10, toward(point, cam, pull));
    let tex = sys.def_frame(TYPE, k as usize);
    sys.pool.recs[i][2] = tex;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particles::MobyFrame;

    /// The three draws, the pull toward the camera, the size easing down to 159600, the alpha pulse up to 0.3 and
    /// back (killed below 0: 41 updates), the moby's state ≥ 0x80 killing it.
    #[test]
    fn glow_pulses_on_the_joint() {
        let mut s = Particles::new(None, Vec::new());
        s.camera = [10.0f32, 0.0, 0.0].map(f32::to_bits);
        s.moby_frames.insert(5, MobyFrame { pos: [0.0; 3], rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], state: 1, o_class: 1106 });
        s.joint_frames.insert((5, 2), [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 1.0, 1.0]]);
        let mut rng = Rng::new();
        let mut t = rng;
        let i = spawn(&mut s, &mut rng, 200000.0, 0.5, 5, 2, [0.0, 0.0, 1.0]).unwrap();
        let spin = t.randf(0.0, 1.0);
        let k = t.rand() & 0xf;
        t.rand();
        assert_eq!(rng, t);
        let r = &s.pool.recs[i];
        assert_eq!((rec::i16(r, 0x2c) as i32, r[8], rec::i16(r, 10), rec::u32(r, 4)), (k, (spin * 255.0) as u8, 80, 0x7f7f7f));
        let p = rec::pos(r);
        assert!((p[0] - 0.5 * 10.0 / 101.0f32.sqrt()).abs() < 1e-6);
        s.update_parts(&mut rng);
        let r = &s.pool.recs[i];
        assert_eq!(rec::ff(r, 0xc), 200000.0 - 15959.999);
        assert_eq!(rec::ff(r, 0x24), 0.015, "rising: stepped above the rest size");
        let mut n = 1;
        let mut peak = 0.0f32;
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            n += 1;
            if s.pool.count > 0 { peak = peak.max(rec::ff(&s.pool.recs[i], 0x24)); }
        }
        assert_eq!(rng, t, "no draws in the update");
        assert!(peak <= 0.3 + 1e-6 && peak > 0.28, "peak {peak}");
        assert!((40..=42).contains(&n), "lived {n}");
        assert_eq!(rec::ff(&s.pool.recs[i], 0xc), REST);
        let j = spawn(&mut s, &mut rng, 200000.0, 0.5, 5, 2, [0.0, 0.0, 1.0]).unwrap();
        s.moby_frames.get_mut(&5).unwrap().state = 0x80;
        s.update_parts(&mut rng);
        assert_eq!(s.pool.recs[j][1] & crate::particles::FLAG_DEAD, crate::particles::FLAG_DEAD);
    }
}
