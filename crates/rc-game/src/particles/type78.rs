//! Particle type 78, the Morph-o-Ray beam's homing sparks (level01 spawner `PartType78Spawn` 0x28ad08, update 0x28ae98,
//! read from the decomp; both the same code on all 19 levels, `overlay-diff`). The beam `0x2d2d08` makes a pair (a
//! coloured one and a white core) one tick in two.
//!
//! **Spawn** `(s1 f12, s2 f13, pos a0, life a1, rgba a2, mode a3, spin t0, vel t1, target t2)`: position = pos, RGBA,
//! ALPHA 0x48 (additive), byte9 `trunc(4) + 0x20` = 0x24, byte1 0, texture `def[78][0]`, timer = life (+0x0a) and
//! +0x28, size = s1·210000 (+0x0c, +0x20), +0x24 = s2·210000, rotation: mode 0 → 0, 1 → 0x20, else `randi(255)` (the
//! only draw), +0x2a the spin, +0x2b the start alpha (`rgba >> 24`), +0x38 the target moby, +0x3c = the target's
//! damage record +0x10 when it has one (`FUN_002711f8`: mode bit 0x20 and pvar +0 pointer; the update never reads
//! it), +0x2c..+0x34 = vel.
//!
//! **Update**: rotation += spin. The first 5 ticks (`e = life − t < 6`, f = e/5): size = s2 + ((s1 + s2)/2 − s2)·f,
//! alpha = a0 + (a0/2 − a0)·f; after: f = t/(life − 6), size = s1 + ((s1 + s2)/2 − s1)·f, alpha = (a0/2)·f; A =
//! `trunc(alpha)`. pos += vel. With a target: d = target − pos; when d.x ≤ 2 and d.y ≤ 2 (**signed**: the game's test,
//! kept) pos += unit(d × gravity)·0.2 (gp−0x69b4, 0.2 in the image), vel = unit(d)·|vel| (or ·0.03 once d.x and d.y are
//! both ≤ 0.5) and vel.z += 0.02. Then the timer fires → killed. No RNG.
//!
//! **Target and gravity.** The record keeps the target's index + 1 at +0x38; its position comes from
//! [`Particles::moby_frames`] (the game reads the pointer with no state test; a target missing there is not homed on
//! [L]); gravity is 0x13f5e0 ([`Particles::gravity`], (0, 0, −1) unless the hero hook writes it). Native `f32`.

use super::{fast_dec_timer, rec, Particles, Record};
use crate::rng::Rng;

pub const TYPE: u8 = 78;

/// The spawn's arguments (0x28ad08).
#[derive(Clone, Copy, Debug)]
pub struct Spawn {
    pub s1: f32,
    pub s2: f32,
    pub pos: [f32; 4],
    pub life: i16,
    pub rgba: u32,
    /// a3: 0 → rotation 0, 1 → 0x20, else `randi(255)`.
    pub mode: i32,
    pub spin: u8,
    pub vel: [f32; 3],
    /// t2: the moby homed on.
    pub target: Option<usize>,
    /// `*(FUN_002711f8(target) + 0x10)` (0 without a damage record).
    pub target_word: u32,
}

/// `PartType78Spawn` 0x28ad08: `randi(255)` when mode is neither 0 nor 1 and a record was free.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, s: Spawn) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let rot = match s.mode {
        0 => 0,
        1 => 0x20,
        _ => rng.randi(0xff) as u8,
    };
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, s.pos);
    rec::set_u32(r, 4, s.rgba);
    r[3] = 0x48;
    r[1] = 0;
    r[9] = 4 + 0x20;
    rec::set_i16(r, 10, s.life);
    r[2] = def;
    rec::set_ff(r, 0xc, s.s1 * 210000.0);
    r[8] = rot;
    rec::set_i16(r, 0x28, s.life);
    rec::set_ff(r, 0x20, s.s1 * 210000.0);
    r[0x2a] = s.spin;
    rec::set_ff(r, 0x24, s.s2 * 210000.0);
    r[0x2b] = (s.rgba >> 24) as u8;
    rec::set_u32(r, 0x38, s.target.map_or(0, |t| t as u32 + 1));
    rec::set_u32(r, 0x3c, if s.target.is_some() { s.target_word } else { 0 });
    rec::set_v3(r, 0x2c, s.vel);
    Some(i)
}

/// The moby a live type-78 record homes on.
pub fn moby_of(r: &Record) -> Option<usize> {
    (r[0] == TYPE && r[1] & super::FLAG_DEAD == 0).then(|| (rec::u32(r, 0x38) as usize).checked_sub(1)).flatten()
}

fn unit(v: [f32; 3], l: f32) -> [f32; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n > 0.0 { v.map(|x| x * l / n) } else { [0.0; 3] }
}

/// Update 0x28ae98 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let target = moby_of(&sys.pool.recs[i]).and_then(|m| sys.moby_frames.get(&m)).map(|f| f.pos);
    let g = sys.gravity;
    let r = &mut sys.pool.recs[i];
    r[8] = r[8].wrapping_add(r[0x2a]);
    let (t, life) = (rec::i16(r, 10) as i32, rec::i16(r, 0x28) as i32);
    let (s1, s2, a0) = (rec::ff(r, 0x20), rec::ff(r, 0x24), r[0x2b]);
    let e = life - t;
    let alpha = if e < 6 {
        let f = e as f32 / 5.0;
        rec::set_ff(r, 0xc, s2 + ((s1 + s2) * 0.5 - s2) * f);
        a0 as f32 + ((a0 >> 1) as i32 - a0 as i32) as f32 * f
    } else {
        let f = t as f32 / (life - 6) as f32;
        rec::set_ff(r, 0xc, s1 + ((s1 + s2) * 0.5 - s1) * f);
        (a0 >> 1) as f32 * f + 0.0
    };
    rec::set_u32(r, 4, ((alpha as i32) as u32) << 24 | rec::u32(r, 4) & 0xff_ffff);
    let mut v = rec::v3(r, 0x2c);
    let p = rec::v3(r, 0x10);
    let mut p = [p[0] + v[0], p[1] + v[1], p[2] + v[2]];
    if let Some(tp) = target {
        let d = [tp[0] - p[0], tp[1] - p[1], tp[2] - p[2]];
        if d[0] <= 2.0 && d[1] <= 2.0 {
            let c = unit([d[1] * g[2] - d[2] * g[1], d[2] * g[0] - d[0] * g[2], d[0] * g[1] - d[1] * g[0]], 0.2);
            p = [p[0] + c[0], p[1] + c[1], p[2] + c[2]];
            let l = if 0.5 < d[0] || 0.5 < d[1] { (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() } else { f32::from_bits(0x3cf5_c28f) };
            v = unit(d, l);
            v[2] += 0.02;
            rec::set_v3(r, 0x2c, v);
        }
    }
    rec::set_v3(r, 0x10, p);
    if fast_dec_timer(r, 10) != 0 { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particles::MobyFrame;

    fn spark(target: Option<usize>, mode: i32) -> Spawn {
        Spawn { s1: 0.1, s2: 1.0, pos: [0.0, 0.0, 0.0, 1.0], life: 16, rgba: 0x7f20_7f7f, mode, spin: 3, vel: [0.1, 0.0, 0.0], target, target_word: 0 }
    }

    /// The draw only for modes other than 0 / 1; the size and alpha ramps (in over 5 ticks, out over the rest); the
    /// life; no draws in the update.
    #[test]
    fn ramps_and_lives() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let before = rng;
        let i = spawn(&mut s, &mut rng, spark(None, 0)).unwrap();
        assert_eq!(s.pool.recs[i][8], 0);
        let k = spawn(&mut s, &mut rng, spark(None, 1)).unwrap();
        assert_eq!(s.pool.recs[k][8], 0x20);
        assert_eq!(rng, before);
        let j = spawn(&mut s, &mut rng, spark(None, 2)).unwrap();
        let mut twin = before;
        assert_eq!(s.pool.recs[j][8], twin.randi(0xff) as u8);
        assert_eq!(rng, twin);
        let r = &s.pool.recs[i];
        assert_eq!((r[9], r[0x2b], rec::i16(r, 0x28)), (0x24, 0x7f, 16));
        s.update_parts(&mut rng);
        let r = &s.pool.recs[i];
        // e = 16 − 16 = 0: size s2, alpha a0; then the timer steps.
        assert_eq!(rec::ff(r, 0xc), 210000.0);
        assert_eq!(rec::u32(r, 4) >> 24, 0x7f);
        assert_eq!(r[8], 3);
        assert_eq!(rec::v3(r, 0x10), [0.1, 0.0, 0.0]);
        for _ in 0..5 { s.update_parts(&mut rng); }
        // e = 16 − 11 = 5 on the 6th: alpha a0/2, size the middle.
        assert_eq!(rec::u32(&s.pool.recs[i], 4) >> 24, 63);
        let (s1, s2) = (rec::ff(&s.pool.recs[i], 0x20), rec::ff(&s.pool.recs[i], 0x24));
        assert_eq!(rec::ff(&s.pool.recs[i], 0xc), s2 + ((s1 + s2) * 0.5 - s2));
        let mut n = 6;
        while s.pool.recs[i][1] & crate::particles::FLAG_DEAD == 0 {
            s.update_parts(&mut rng);
            n += 1;
        }
        assert_eq!(n, 16);
        assert_eq!(rng, twin);
    }

    /// Homing: within 2 in x and y (signed) the spark is pushed sideways 0.2 and turned at the target, speed kept,
    /// +0.02 up; a target behind by more than 2 in x is ignored.
    #[test]
    fn homes_on_its_target() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn(&mut s, &mut rng, spark(Some(9), 0)).unwrap();
        s.moby_frames.insert(9, MobyFrame { pos: [1.1, 1.0, 0.0], rows: [[0.0; 3]; 3], state: 1, o_class: 1 });
        s.update_parts(&mut rng);
        let r = &s.pool.recs[i];
        // p = (0.1, 0, 0), d = (1, 1, 0); d × (0, 0, −1) = (−1, 1, 0) → ·0.2/√2.
        let k = 0.2 / 2.0f32.sqrt();
        let p = rec::v3(r, 0x10);
        assert!((p[0] - (0.1 - k)).abs() < 1e-6 && (p[1] - k).abs() < 1e-6 && p[2] == 0.0, "{p:?}");
        let v = rec::v3(r, 0x2c);
        let u = 0.1 / 2.0f32.sqrt();
        assert!((v[0] - u).abs() < 1e-6 && (v[1] - u).abs() < 1e-6 && v[2] == 0.02, "{v:?}");
        let j = spawn(&mut s, &mut rng, Spawn { pos: [-5.0, 0.0, 0.0, 1.0], ..spark(Some(9), 0) }).unwrap();
        s.update_parts(&mut rng);
        assert_eq!(rec::v3(&s.pool.recs[j], 0x2c), [0.1, 0.0, 0.0], "d.x = 6: not homed");
    }
}
