//! Particle type 31, the line drawn into a moby (level01 update 0x283be8, no Ghidra function until 2026-09-29, the same
//! code on all 19 levels, `overlay-diff`; spawner level15 `0x25e2c0`, also on 18; read from the decomp). Its callers
//! are levels 15 / 18's unported classes (G-PRT-001).
//!
//! Small-data constants no code writes (image values on 15 and 18): the pulls gp−0x6a0c = 0.93 (the head) and
//! gp−0x6a08 = 0.929 (the tail), the life gp−0x6a04 = 90.
//!
//! **Spawn** `(rgba a0, pos a1, moby a2)`: end 1 = moby + (pos − moby)·0.25, end 2 = pos, RGBA1 = rgba, RGBA2 = rgba at
//! alpha 0, byte9 `trunc(4) + 0x70` = 0x74, ALPHA 0x44, **render kind 2** (the untextured line), timer `ticks(90)`,
//! +0x30 the moby. No RNG.
//!
//! **Update**: the moby pointer 0, its state 0xfe / 0xfd, or the timer → killed. Else each end = moby + (end −
//! moby)·((k − 1)·speed + 1) (k = 0.93 / 0.929: both close in on the moby, the tail a touch faster); the ends within
//! 0.1 of each other, or the head within 1 of the moby → killed. No RNG. The moby comes from
//! [`Particles::moby_frames`].

use super::{fast_dec_timer, rec, Particles, Record};
use crate::rng::Rng;

pub const TYPE: u8 = 31;

/// Level15 `0x25e2c0(rgba, pos, moby)` with the moby's position (no RNG).
pub fn spawn(sys: &mut Particles, rgba: u32, pos: [f32; 4], moby: usize, at: [f32; 3]) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let t = sys.time.ticks(90) as i16;
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, std::array::from_fn(|k| at[k] + (pos[k] - at[k]) * 0.25));
    rec::set_u32(r, 4, rgba);
    rec::set_u32(r, 0xc, rgba & 0xff_ffff);
    rec::set_v4(r, 0x20, pos);
    r[9] = 4 + 0x70;
    r[3] = 0x44;
    r[1] = 2;
    rec::set_u32(r, 0x30, moby as u32 + 1);
    rec::set_i16(r, 10, t);
    Some(i)
}

/// The moby a live type-31 record closes on.
pub fn moby_of(r: &Record) -> Option<usize> {
    (r[0] == TYPE && r[1] & super::FLAG_DEAD == 0).then(|| (rec::u32(r, 0x30) as usize).checked_sub(1)).flatten()
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() }

/// Update 0x283be8 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let f = moby_of(&sys.pool.recs[i]).and_then(|m| sys.moby_frames.get(&m).copied()).filter(|f| f.state != 0xfe && f.state != 0xfd);
    let r = &mut sys.pool.recs[i];
    let Some(f) = f.filter(|_| fast_dec_timer(r, 10) == 0) else {
        sys.kill_part(i);
        return;
    };
    let m = f.pos;
    for (o, k) in [(0x10, f32::from_bits(0x3f6e_147b)), (0x20, f32::from_bits(0x3f6d_d2f2))] {
        let e = rec::v3(r, o);
        let s = (k - 1.0) + 1.0;
        rec::set_v3(r, o, std::array::from_fn(|l| (e[l] - m[l]) * s + m[l]));
    }
    let (h, t) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
    if dist(h, t) < 0.1 || dist(h, m) < 1.0 { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particles::MobyFrame;

    /// Both ends close in on the moby; killed when the head comes within 1.
    #[test]
    fn line_closes_on_the_moby() {
        let mut s = Particles::new(None, Vec::new());
        s.moby_frames.insert(1, MobyFrame { pos: [0.0; 3], ..Default::default() });
        let i = spawn(&mut s, 0x80ff_ffff, [40.0, 0.0, 0.0, 1.0], 1, [0.0; 3]).unwrap();
        let r = &s.pool.recs[i];
        assert_eq!((r[1], rec::pos(r), rec::v3(r, 0x20), rec::u32(r, 0xc)), (2, [10.0, 0.0, 0.0], [40.0, 0.0, 0.0], 0x00ff_ffff));
        let mut rng = Rng::new();
        let mut n = 0;
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            n += 1;
        }
        // 10·0.93^n < 1 at n = 32.
        assert_eq!(n, 32);
    }
}
