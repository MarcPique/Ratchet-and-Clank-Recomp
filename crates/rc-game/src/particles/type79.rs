//! Particle types 79 and 80, level 17's sparks and flashes (level01 updates `PartType79Update` 0x28b160 and
//! `PartType80Update` 0x28b2a8, both the same code on all 19 levels, `overlay-diff`; spawners level17 `0x26fb60` and
//! `0x26fdd0`, read from the decomp). Both take **`def[53][0]`** (level17 `*0x1b2a54` = its def table 0x1b2980 +
//! 53·4, the sparkle list). Their callers are level 17's unported classes (G-PRT-001).
//!
//! **79, spawn** `(moby a0, pos a1, vel a2)`: `k = randi(5)`, `ticks(k + 2)`, `randi(255)` — made **before**
//! `CreatePart`, so also when the pool is full; position, velocity +0x30; with a moby: +0x20 = pos − moby, timer
//! `ticks(k + 2) & 0xff`; without: timer `(ticks(k + 2) & 0xff) << 2`; +0x3c the moby; RGBA 0x604040ff, byte9
//! `trunc(1) + 0x20` = 0x21, ALPHA 0x48, size 42000, rotation, byte1 0.
//! **79, update**: free: pos += vel; on a moby: offset += vel, pos = moby + offset. The timer fires → killed; else the
//! colour tweens `FastTweenColor((t − 1)/t, target, colour)` toward 0 (free) or its own RGB at alpha 0 (on a moby).
//!
//! **80, spawn** `(pos a0, kind a1, k a2)`: n = kind ? 5 : 15; k > n → none (no draw). kind 0: RGBA 0x60ff3020, ALPHA
//! 0x48, size 630000, timer 15 − k; else RGBA 0x80d0a080, ALPHA 0x44, size 420000, timer 5 − k; k ≠ 0 → one tween
//! step toward the update's target at once; +0x20 = kind, byte9 `trunc(1) + 0x70` = 0x71, byte1 0, rotation
//! `randi(255)` (with a record).
//! **80, update**: outside [2, 1021]³ or the timer fires → killed; kind 0: size ·0.95, colour tween toward 0x603f1008;
//! else size ·0.99, toward 0x30ffffff. No RNG in either update.

use super::{fast_dec_timer, rec, tween_color, Particles, Record};
use crate::rng::Rng;

pub const TYPE79: u8 = 79;
pub const TYPE80: u8 = 80;

/// Level17 `0x26fb60(moby, pos, vel)` with the moby's position: three draws, then the record (none when full).
pub fn spawn79(sys: &mut Particles, rng: &mut Rng, moby: Option<(usize, [f32; 3])>, pos: [f32; 4], vel: [f32; 3]) -> Option<usize> {
    let k = rng.randi(5);
    let t = sys.time.ticks(k + 2) as u32 & 0xff;
    let rot = rng.randi(0xff) as u8;
    let i = sys.create_part(TYPE79)?;
    let def = sys.def_first(53);
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_v3(r, 0x30, vel);
    match moby {
        None => rec::set_i16(r, 10, (t << 2) as i16),
        Some((_, mp)) => {
            rec::set_v3(r, 0x20, [pos[0] - mp[0], pos[1] - mp[1], pos[2] - mp[2]]);
            rec::set_i16(r, 10, t as i16);
        }
    }
    rec::set_u32(r, 0x3c, moby.map_or(0, |(m, _)| m as u32 + 1));
    rec::set_u32(r, 4, 0x6040_40ff);
    r[9] = 1 + 0x20;
    r[3] = 0x48;
    rec::set_ff(r, 0xc, 42000.0);
    r[8] = rot;
    r[1] = 0;
    r[2] = def;
    Some(i)
}

/// The moby a live type-79 record rides.
pub fn moby_of(r: &Record) -> Option<usize> {
    (r[0] == TYPE79 && r[1] & super::FLAG_DEAD == 0).then(|| (rec::u32(r, 0x3c) as usize).checked_sub(1)).flatten()
}

fn step(t: i16) -> u32 { ((t as i32 - 1) as f32 / t as f32).to_bits() }

/// Update 0x28b160 (no RNG).
pub fn update79(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let m = moby_of(&sys.pool.recs[i]);
    let mp = m.and_then(|m| sys.moby_frames.get(&m)).map(|f| f.pos);
    let r = &mut sys.pool.recs[i];
    let v = rec::v3(r, 0x30);
    match m {
        None => {
            let p = rec::v3(r, 0x10);
            rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
        }
        Some(_) => {
            let o = rec::v3(r, 0x20);
            let o = [o[0] + v[0], o[1] + v[1], o[2] + v[2]];
            rec::set_v3(r, 0x20, o);
            // A moby missing from the frames: the position stays [L] (the game reads the pointer regardless).
            if let Some(mp) = mp { rec::set_v3(r, 0x10, [mp[0] + o[0], mp[1] + o[1], mp[2] + o[2]]); }
        }
    }
    if fast_dec_timer(r, 10) != 0 {
        sys.kill_part(i);
        return;
    }
    let c = rec::u32(r, 4);
    let target = if m.is_none() { 0 } else { c & 0xff_ffff };
    rec::set_u32(r, 4, tween_color(step(rec::i16(r, 10)), target, c));
}

/// Level17 `0x26fdd0(pos, kind, k)`: one `randi(255)` with a record; none when `k` is past the kind's life or the
/// pool is full.
pub fn spawn80(sys: &mut Particles, rng: &mut Rng, pos: [f32; 4], kind: i32, k: i32) -> Option<usize> {
    let n = if kind != 0 { 5 } else { 15 };
    if n - k < 0 { return None; }
    let i = sys.create_part(TYPE80)?;
    let def = sys.def_first(53);
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 0x20, kind as u32);
    r[9] = 1 + 0x70;
    let (c, blend, size, target) = if kind == 0 { (0x60ff_3020u32, 0x48, 630000.0f32, 0x603f_1008u32) } else { (0x80d0_a080, 0x44, 420000.0, 0x30ff_ffff) };
    rec::set_u32(r, 4, c);
    r[3] = blend;
    rec::set_ff(r, 0xc, size);
    rec::set_i16(r, 10, (n - k) as i16);
    if k != 0 { rec::set_u32(r, 4, tween_color(step((n - k) as i16), target, c)); }
    r[1] = 0;
    r[8] = rng.randi(0xff) as u8;
    r[2] = def;
    Some(i)
}

/// Update 0x28b2a8 (no RNG).
pub fn update80(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    let p = rec::pos(r);
    if p.iter().any(|&x| !(2.0..=1021.0).contains(&x)) || fast_dec_timer(r, 10) != 0 {
        sys.kill_part(i);
        return;
    }
    let (k, target) = if rec::u32(r, 0x20) == 0 { (0.95, 0x603f_1008) } else { (0.99, 0x30ff_ffff) };
    rec::set_ff(r, 0xc, rec::ff(r, 0xc) * k);
    let c = rec::u32(r, 4);
    rec::set_u32(r, 4, tween_color(step(rec::i16(r, 10)), target, c));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particles::MobyFrame;

    /// 79: the three draws come first (also with a full pool), free sparks live 4× longer and fade to 0; on a moby they
    /// ride it.
    #[test]
    fn sparks_draw_first_ride_and_fade() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let mut t = rng;
        let i = spawn79(&mut s, &mut rng, None, [5.0; 4], [0.1, 0.0, 0.0]).unwrap();
        let k = t.randi(5);
        let rot = t.randi(0xff) as u8;
        assert_eq!(rng, t);
        assert_eq!((rec::i16(&s.pool.recs[i], 10) as i32, s.pool.recs[i][8]), ((k + 2) << 2, rot));
        let j = spawn79(&mut s, &mut rng, Some((3, [4.0, 5.0, 5.0])), [5.0; 4], [0.1, 0.0, 0.0]).unwrap();
        s.moby_frames.insert(3, MobyFrame { pos: [10.0, 10.0, 10.0], ..Default::default() });
        s.update_parts(&mut rng);
        assert_eq!(rec::pos(&s.pool.recs[j]), [11.1, 10.0, 10.0]);
        assert_eq!(rec::pos(&s.pool.recs[i]), [5.1, 5.0, 5.0]);
        let mut last = 0;
        while s.pool.recs[i][1] & crate::particles::FLAG_DEAD == 0 {
            last = rec::u32(&s.pool.recs[i], 4);
            s.update_parts(&mut rng);
        }
        assert!(last >> 24 < 0x30, "faded {last:#x}");
        let mut full = Particles::new(None, Vec::new());
        for _ in 0..crate::particles::POOL_RECORDS { full.create_part(1); }
        let before = rng;
        assert!(spawn79(&mut full, &mut rng, None, [5.0; 4], [0.0; 3]).is_none());
        assert_ne!(rng, before, "drawn anyway");
    }

    /// 80: the kind's life minus k, the size shrinking, the tween; k past the life spawns nothing.
    #[test]
    fn flashes_shrink_and_tween() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        assert!(spawn80(&mut s, &mut rng, [5.0; 4], 1, 6).is_none());
        assert_eq!(rng, Rng::new());
        let i = spawn80(&mut s, &mut rng, [5.0; 4], 0, 0).unwrap();
        let r = &s.pool.recs[i];
        assert_eq!((rec::u32(r, 4), r[3], r[9], rec::i16(r, 10)), (0x60ff_3020, 0x48, 0x71, 15));
        let mut n = 0;
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            n += 1;
            if n == 1 { assert_eq!(rec::ff(&s.pool.recs[i], 0xc), 630000.0 * 0.95); }
        }
        assert_eq!(n, 15);
    }
}
