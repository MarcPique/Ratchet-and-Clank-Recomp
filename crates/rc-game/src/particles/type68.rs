//! Particle type 68 (0x44), the flickering ribbon held on a moby's joint (level13 spawner `0x280400`, level01 update
//! `0x289cc0`: the update is the same code on all 19 levels, `overlay-diff --fn L01:289cc0` = everywhere; read from
//! the decomp and the disassembly). Gemlik's flying biter 63 (`units::flying_biter`) holds one while it flies.
//!
//! **Spawn** `(w1 f12, k f13, len f14, moby a0, list a1, rgba1 a2, rgba2 a3, jitter t0)`: RGBA1 (+0x04), RGBA2 (+0x0c),
//! ALPHA 0x48 (additive), **render kind 3** (byte1 = 3: the textured ribbon), byte9 = `(moby+0x32 >> 5) << 4` +
//! `trunc(0.4)` (the far nibble from the moby's draw distance, near 0), texture `def[68][0]` (level13 `*0x1b2490` =
//! the def table 0x1b2380 + 68·4), end 1 (+0x10) = joint list `list`'s point (`0x2645a8`), end 2 (+0x20) = end 1,
//! +0x2c = k (end 2's width factor), +0x1c = w1 (end 1's half-width), +0x30 the moby, +0x34 the list, +0x35 the jitter
//! (percent), +0x36 the moby's class (+0xa6), +0x38 w1, +0x3c len. No RNG.
//!
//! **Update** (no timer): the moby pointer 0, the moby's class no longer +0x36, or its state 0xfe / 0xfd → killed.
//! Else, in this order: `w = randf(0.05, 1)`; end 1 = the joint point (`0x2645a8`); the joint's world matrix
//! (`MobyAttachToJoint` 0x264508); `j = randf(−jitter, jitter)`; end 2 = end 1 + unit(row 0)·`(j/100 − 1)·len` (the
//! ribbon trails behind the joint's x axis, 0.75–1.25 × len long); +0x2c kept (the vector writes cross it); +0x1c =
//! `w1·w` (the width flickers). Two draws a tick.
//!
//! **The moby.** The record keeps the moby index + 1 at +0x30. The moby's state and class come from
//! [`Particles::moby_frames`], its joint's world matrix from [`Particles::joint_frames`] (row 3 = the joint point,
//! row 0 = the axis), both written by the moby loop after its pass (`World::refresh_particle_anchors`). A moby missing
//! from `moby_frames` is gone; a joint missing from `joint_frames` is the moby's origin and rows (as `0x2645a8` does
//! for a class without the joint list). Native `f32`.

use super::{rec, Particles, Record};
use crate::rng::Rng;

pub const TYPE: u8 = 68;

/// The spawn's arguments (level13 `0x280400`).
#[derive(Clone, Copy, Debug)]
pub struct Spawn {
    /// f12: end 1's half-width (kept at +0x38; +0x1c flickers from it).
    pub w1: f32,
    /// f13: end 2's width factor (+0x2c).
    pub k: f32,
    /// f14: the ribbon's length (+0x3c).
    pub len: f32,
    /// a0: the moby (index).
    pub moby: usize,
    /// a1: the joint list.
    pub list: u8,
    pub rgba1: u32,
    pub rgba2: u32,
    /// t0: the length jitter in percent (+0x35).
    pub jitter: u8,
    /// moby+0x32: its draw distance.
    pub draw_dist: i16,
    /// moby+0xa6: its class.
    pub o_class: i16,
    /// `0x2645a8(moby, list)`: the joint point.
    pub at: [f32; 4],
}

/// Level13 `0x280400`: the record (no RNG); none when the pool is full.
pub fn spawn(sys: &mut Particles, s: Spawn) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let r = &mut sys.pool.recs[i];
    rec::set_u32(r, 4, s.rgba1);
    rec::set_u32(r, 0xc, s.rgba2);
    r[3] = 0x48;
    r[1] = 3;
    r[9] = (((s.draw_dist as i32) >> 5) << 4) as u8; // + trunc(0.4) = 0
    r[2] = def;
    rec::set_v4(r, 0x10, s.at);
    rec::set_v4(r, 0x20, s.at);
    rec::set_ff(r, 0x2c, s.k);
    rec::set_ff(r, 0x1c, s.w1);
    rec::set_u32(r, 0x30, s.moby as u32 + 1);
    r[0x34] = s.list;
    r[0x35] = s.jitter;
    rec::set_i16(r, 0x36, s.o_class);
    rec::set_ff(r, 0x38, s.w1);
    rec::set_ff(r, 0x3c, s.len);
    Some(i)
}

/// The moby and joint list a live type-68 record holds.
pub fn joint_of(r: &Record) -> Option<(usize, u8)> {
    (r[0] == TYPE && r[1] & super::FLAG_DEAD == 0).then(|| (rec::u32(r, 0x30) as usize).checked_sub(1).map(|m| (m, r[0x34]))).flatten()
}

/// Update 0x289cc0: two draws while the moby holds it.
pub fn update(sys: &mut Particles, i: usize, rng: &mut Rng) {
    let r = &sys.pool.recs[i];
    let held = joint_of(r).and_then(|(m, list)| {
        let f = sys.moby_frames.get(&m)?;
        (f.o_class == rec::i16(r, 0x36) && f.state != 0xfe && f.state != 0xfd).then_some((m, list, *f))
    });
    let Some((m, list, frame)) = held else {
        sys.kill_part(i);
        return;
    };
    let w = rng.randf(f32::from_bits(0x3d4c_cccd), 1.0);
    let (point, axis) = match sys.joint_frames.get(&(m, list)) {
        Some(j) => (j[3], [j[0][0], j[0][1], j[0][2]]),
        None => ([frame.pos[0], frame.pos[1], frame.pos[2], 1.0], frame.rows[0]),
    };
    let r = &mut sys.pool.recs[i];
    let (w1, k, len, jit) = (rec::ff(r, 0x38), rec::ff(r, 0x2c), rec::ff(r, 0x3c), r[0x35] as f32);
    rec::set_v4(r, 0x10, point);
    let j = rng.randf(-jit, jit);
    let l = (j / 100.0 - 1.0) * len;
    let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let v = if n > 0.0 { axis.map(|a| a * l / n) } else { [0.0; 3] };
    rec::set_v3(r, 0x20, [point[0] + v[0], point[1] + v[1], point[2] + v[2]]);
    rec::set_ff(r, 0x2c, k);
    rec::set_ff(r, 0x1c, w1 * w);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particles::MobyFrame;

    fn biter(sys: &mut Particles) -> usize {
        let s = Spawn { w1: 0.2, k: 0.2, len: 0.8, moby: 4, list: 0, rgba1: 0x8080_8080, rgba2: 0x1080_8080, jitter: 0x19, draw_dist: 0x80, o_class: 63, at: [1.0, 2.0, 3.0, 1.0] };
        spawn(sys, s).unwrap()
    }

    /// The flying biter's call `0x280400(0.2, 0.2, 0.8, m, 0, 0x80808080, 0x10808080, 0x19)`: the record's bytes.
    #[test]
    fn spawn_record() {
        let mut s = Particles::new(None, Vec::new());
        let i = biter(&mut s);
        let r = &s.pool.recs[i];
        assert_eq!((r[0], r[1], r[3], r[9]), (68, 3, 0x48, 0x40));
        assert_eq!((rec::u32(r, 4), rec::u32(r, 0xc)), (0x8080_8080, 0x1080_8080));
        assert_eq!(rec::v3(r, 0x10), [1.0, 2.0, 3.0]);
        assert_eq!(rec::v3(r, 0x20), [1.0, 2.0, 3.0]);
        assert_eq!((rec::ff(r, 0x1c), rec::ff(r, 0x2c), rec::ff(r, 0x38), rec::ff(r, 0x3c)), (0.2, 0.2, 0.2, 0.8));
        assert_eq!((rec::u32(r, 0x30), r[0x34], r[0x35], rec::i16(r, 0x36)), (5, 0, 0x19, 63));
    }

    /// Each tick: end 1 on the joint, end 2 0.6..1.0 behind the joint's x axis, the width w1·randf(0.05, 1), two
    /// draws in that order; the class changing or the moby deleted kills it.
    #[test]
    fn follows_the_joint_flickers_and_dies_with_the_moby() {
        let mut s = Particles::new(None, Vec::new());
        let i = biter(&mut s);
        s.moby_frames.insert(4, MobyFrame { pos: [0.0; 3], rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], state: 1, o_class: 63 });
        let joint = [[0.0, 2.0, 0.0, 0.0], [-1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [5.0, 6.0, 7.0, 1.0]];
        s.joint_frames.insert((4, 0), joint);
        let mut rng = Rng::new();
        let mut twin = rng;
        s.update_parts(&mut rng);
        let w = twin.randf(f32::from_bits(0x3d4c_cccd), 1.0);
        let j = twin.randf(-25.0, 25.0);
        assert_eq!(rng.rand(), twin.rand(), "two draws");
        let r = &s.pool.recs[i];
        assert_eq!(rec::v3(r, 0x10), [5.0, 6.0, 7.0]);
        let l = (j / 100.0 - 1.0) * 0.8;
        assert_eq!(rec::v3(r, 0x20), [5.0, 6.0 + l, 7.0]);
        assert!((-1.0..=-0.6).contains(&l));
        assert_eq!(rec::ff(r, 0x1c), 0.2 * w);
        assert_eq!(rec::ff(r, 0x2c), 0.2);
        // A missing joint: the moby's origin and row 0.
        s.joint_frames.clear();
        s.moby_frames.get_mut(&4).unwrap().pos = [9.0, 9.0, 9.0];
        s.update_parts(&mut rng);
        let r = &s.pool.recs[i];
        assert_eq!(rec::v3(r, 0x10), [9.0, 9.0, 9.0]);
        assert!(rec::ff(r, 0x20) < 9.0 && rec::ff(r, 0x24) == 9.0);
        // State 0xfe / a new class / no moby: killed.
        let kills: [fn(&mut MobyFrame); 3] = [|f| f.state = 0xfe, |f| f.state = 0xfd, |f| f.o_class = 64];
        for kill in kills {
            let mut s2 = Particles::new(None, Vec::new());
            biter(&mut s2);
            let mut f = MobyFrame { pos: [0.0; 3], rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], state: 1, o_class: 63 };
            kill(&mut f);
            s2.moby_frames.insert(4, f);
            s2.update_parts(&mut rng);
            assert_eq!(s2.pool.count, 0);
        }
        s.moby_frames.clear();
        s.update_parts(&mut rng);
        assert_eq!(s.pool.count, 0);
        assert_eq!(s.stats.unported_kills[68], 0);
    }
}
