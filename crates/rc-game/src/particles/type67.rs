//! Particle type 67, the orbiting spark and its fading trail (level01 spawner `PartType67Spawn` 0x289850, update
//! 0x2899c0, read from the decomp and the spawner's disassembly; the update is the same code on every level but 13,
//! whose copy is level 00's superset build (`overlay-diff` `s`)). The census's unit U203 (class 1139, levels 05 / 16)
//! calls the spawner (G-PRT-001 consumer; its class is not ported).
//!
//! **Spawn** `(a f12, b f13, moby a0, pos a1, life a2, mode a3, red t0, dir t1)`: position = pos, +0x2c the moby, +0x30
//! / +0x34 the two orbit angles, RGBA `0x409b9b00 | red`, byte1 0, byte9 `trunc(4) + 0x40` = 0x44, ALPHA 0x48, size
//! 52500, rotation `randi(255)` (the one draw), +0x3c the mode (s16), +0x38 0 (s16, the distance along dir), texture
//! mode 0 → `def[67][0]` else `def[67][1]`; mode 0: +0x3a = `trunc(127 / life)` (the fade step); dir given → +0x20 =
//! dir.xyz; timer = life.
//!
//! **Update, mode 0** (a trail puff): alpha −= step (`0x26e3b0`: clamped at 0; 0 → killed); a moby → position =
//! moby + dir·(f32)(+0x38). No timer.
//! **Update, other modes** (the head): the timer fires → killed; under `ticks(10)` left, alpha −= 0x10 (0 → killed).
//! Mode 1 leaves a trail puff `(0, 0, no moby, pos, ticks(10), 0, alpha byte, none)`; other modes one riding the moby
//! `(0, 0, moby, pos, ticks(10), 0, alpha byte, dir)` that copies +0x38 (the caller moves the head's +0x38). Then mode
//! 1 orbits: position = moby + rows·(`(0, 2.25·sin a, 2.25·cos a)` + `Euler(0, a, 0)·(0, 0.28·sin b, 0.28·cos b)`), a
//! += 0.9599·dt, b += 4.3633·dt (`fast_add_rotations`, dt 1/60). The trail puff's red is the head's alpha byte (the
//! game passes byte 7). One draw a tick for a head (its puff's), none for a puff.
//!
//! **The moby.** The record keeps the moby index + 1 at +0x2c; its position and rows come from
//! [`Particles::moby_frames`] (the game reads the pointer with no state test: a moby missing there leaves the
//! position as it is [L]). Native `f32`.

use super::{fast_dec_timer, rec, MobyFrame, Particles, Record};
use crate::rng::Rng;
use std::f32::consts::PI;

pub const TYPE: u8 = 67;

/// `PartType67Spawn` 0x289850's arguments.
#[derive(Clone, Copy, Debug, Default)]
pub struct Spawn {
    pub a: f32,
    pub b: f32,
    pub moby: Option<usize>,
    pub pos: [f32; 4],
    pub life: i32,
    pub mode: i16,
    pub red: u8,
    pub dir: Option<[f32; 3]>,
}

/// `PartType67Spawn` 0x289850: one `randi(255)` with a record; none when the pool is full.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, s: Spawn) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let tex = sys.def_frame(TYPE, if s.mode == 0 { 0 } else { 1 });
    let rot = rng.randi(0xff) as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, s.pos);
    rec::set_u32(r, 0x2c, s.moby.map_or(0, |m| m as u32 + 1));
    rec::set_ff(r, 0x30, s.a);
    rec::set_ff(r, 0x34, s.b);
    rec::set_u32(r, 4, s.red as u32 | 0x409b_9b00);
    r[1] = 0;
    r[9] = 4 + 0x40;
    r[3] = 0x48;
    rec::set_ff(r, 0xc, 52500.0);
    r[8] = rot;
    rec::set_i16(r, 0x3c, s.mode);
    rec::set_i16(r, 0x38, 0);
    r[2] = tex;
    if s.mode == 0 { rec::set_i16(r, 0x3a, (127.0 / s.life as f32) as i32 as i16); }
    if let Some(d) = s.dir { rec::set_v3(r, 0x20, d); }
    rec::set_i16(r, 10, s.life as i16);
    Some(i)
}

/// The moby a live type-67 record holds.
pub fn moby_of(r: &Record) -> Option<usize> {
    (r[0] == TYPE && r[1] & super::FLAG_DEAD == 0).then(|| (rec::u32(r, 0x2c) as usize).checked_sub(1)).flatten()
}

/// `0x26e3b0(&rgba, step)`: the alpha byte (read as the word's arithmetic top) minus step, clamped at 0; true at 0.
fn fade(r: &mut Record, step: i32) -> bool {
    let a = ((rec::u32(r, 4) as i32) >> 24) - step;
    let a = a.max(0);
    rec::set_u32(r, 4, rec::u32(r, 4) & 0xff_ffff | (a as u32) << 24);
    a == 0
}

fn add_rot(a: f32, b: f32) -> f32 {
    let s = a + b;
    if s >= PI { (s - PI) - PI } else if s < -PI { (s + PI) + PI } else { s }
}

/// Update 0x2899c0.
pub fn update(sys: &mut Particles, i: usize, rng: &mut Rng) {
    let frame: Option<MobyFrame> = moby_of(&sys.pool.recs[i]).and_then(|m| sys.moby_frames.get(&m).copied());
    let mode = rec::i16(&sys.pool.recs[i], 0x3c);
    if mode == 0 {
        let r = &mut sys.pool.recs[i];
        let step = rec::i16(r, 0x3a) as i32;
        if fade(r, step) {
            sys.kill_part(i);
            return;
        }
        if let Some(f) = frame {
            let (d, k) = (rec::v3(r, 0x20), rec::i16(r, 0x38) as f32);
            rec::set_v3(r, 0x10, [f.pos[0] + d[0] * k, f.pos[1] + d[1] * k, f.pos[2] + d[2] * k]);
        }
        return;
    }
    let t10 = sys.time.ticks(10);
    let r = &mut sys.pool.recs[i];
    if fast_dec_timer(r, 10) != 0 || ((rec::i16(r, 10) as i32) < t10 && fade(r, 0x10)) {
        sys.kill_part(i);
        return;
    }
    let (pos, red, moby) = (rec::v4(r, 0x10), r[7], moby_of(r));
    let (dir, d38) = (rec::v3(r, 0x20), rec::i16(r, 0x38));
    let child = Spawn { pos, life: t10, red, ..Default::default() };
    if mode == 1 {
        spawn(sys, rng, child);
    } else if let Some(c) = spawn(sys, rng, Spawn { moby, dir: Some(dir), ..child }) {
        rec::set_i16(&mut sys.pool.recs[c], 0x38, d38);
    }
    if mode != 1 { return; }
    let r = &mut sys.pool.recs[i];
    let (a, b) = (rec::ff(r, 0x30), rec::ff(r, 0x34));
    if let Some(f) = frame {
        let v1 = [0.0, a.sin() * 2.25, a.cos() * 2.25];
        let v2 = [0.0, b.sin() * 0.28, b.cos() * 0.28];
        let e = rc_formats::moby_light::rotation_rows([0.0, a, 0.0]).map(|row| row.map(f32::from_bits));
        let v2: [f32; 3] = std::array::from_fn(|l| e[0][l] * v2[0] + e[1][l] * v2[1] + e[2][l] * v2[2]);
        let v = [v1[0] + v2[0], v1[1] + v2[1], v1[2] + v2[2]];
        let w: [f32; 3] = std::array::from_fn(|l| f.rows[0][l] * v[0] + f.rows[1][l] * v[1] + f.rows[2][l] * v[2]);
        rec::set_v3(r, 0x10, [f.pos[0] + w[0], f.pos[1] + w[1], f.pos[2] + w[2]]);
    }
    let dt = 1.0 / 60.0;
    rec::set_ff(r, 0x30, add_rot(a, 0.959_931_1 * dt));
    rec::set_ff(r, 0x34, add_rot(b, 4.363_323 * dt));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> MobyFrame { MobyFrame { pos: [10.0, 10.0, 10.0], rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], state: 1, o_class: 1139 } }

    /// A mode-1 head orbits its moby at 2.25 (± the 0.28 wobble) in the moby's yz plane, drops a trail puff a tick
    /// (one draw each), fades over its last 10 ticks; the puffs fade by 127/10 a tick and die in 10 updates.
    #[test]
    fn head_orbits_and_leaves_a_fading_trail() {
        let mut s = Particles::new(None, Vec::new());
        s.moby_frames.insert(2, frame());
        let mut rng = Rng::new();
        let mut t = rng;
        let h = spawn(&mut s, &mut rng, Spawn { a: 0.0, b: 0.0, moby: Some(2), pos: [10.0, 10.0, 10.0, 1.0], life: 40, mode: 1, red: 0x80, dir: None }).unwrap();
        assert_eq!(s.pool.recs[h][8], t.randi(0xff) as u8);
        assert_eq!(rec::u32(&s.pool.recs[h], 4), 0x409b_9b80);
        s.update_parts(&mut rng);
        t.randi(0xff);
        assert_eq!(rng, t, "the puff's draw");
        let p = rec::pos(&s.pool.recs[h]);
        // a = 0, b = 0: (0, 0, 2.25) + (0, 0, 0.28).
        assert!((p[0] - 10.0).abs() < 1e-5 && (p[1] - 10.0).abs() < 1e-5 && (p[2] - 12.53).abs() < 1e-5, "{p:?}");
        let puff = s.pool.live().find(|(k, r)| *k != h && r[0] == TYPE).map(|(k, _)| k).unwrap();
        assert_eq!(rec::u32(&s.pool.recs[puff], 4), 0x409b_9b40, "the puff's red is the head's alpha");
        assert_eq!(rec::i16(&s.pool.recs[puff], 0x3a), 12);
        let mut radii = Vec::new();
        let mut n = 1;
        while s.pool.recs[h][1] & super::super::FLAG_DEAD == 0 {
            s.update_parts(&mut rng);
            n += 1;
            let q = rec::pos(&s.pool.recs[h]);
            radii.push(((q[1] - 10.0).powi(2) + (q[2] - 10.0).powi(2)).sqrt());
        }
        assert_eq!(n, 34, "alpha 0x40 fades by 0x10 from t = 9: gone on update 34 of its 40");
        assert!(radii.iter().all(|&r| (1.96..=2.54).contains(&r)), "{radii:?}");
        // The last puffs outlive the head by up to 6 updates (0x40 in steps of 12).
        for _ in 0..12 { s.update_parts(&mut rng); }
        assert_eq!(s.pool.count, 0);
    }

    /// A mode-2 head's puffs ride the moby at +0x38 along dir.
    #[test]
    fn beam_puffs_ride_the_moby() {
        let mut s = Particles::new(None, Vec::new());
        s.moby_frames.insert(2, frame());
        let mut rng = Rng::new();
        let h = spawn(&mut s, &mut rng, Spawn { moby: Some(2), pos: [10.0, 10.0, 10.0, 1.0], life: 40, mode: 2, red: 0x80, dir: Some([1.0, 0.0, 0.0]), ..Default::default() }).unwrap();
        rec::set_i16(&mut s.pool.recs[h], 0x38, 3);
        s.update_parts(&mut rng);
        let puff = s.pool.live().find(|(k, r)| *k != h && r[0] == TYPE).map(|(k, _)| k).unwrap();
        assert_eq!(rec::i16(&s.pool.recs[puff], 0x38), 3);
        s.moby_frames.get_mut(&2).unwrap().pos = [20.0, 0.0, 0.0];
        s.update_parts(&mut rng);
        assert_eq!(rec::pos(&s.pool.recs[puff]), [23.0, 0.0, 0.0]);
        assert_eq!(rec::pos(&s.pool.recs[h]), [10.0, 10.0, 10.0], "a mode-2 head stays");
    }
}
