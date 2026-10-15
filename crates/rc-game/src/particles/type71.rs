//! Particle type 71, the barrier's drifting mote (level01 update `PartType71Update` 0x28a0e0; the spawner exists on
//! level15 `0x2647f0` / level17 for the energy barriers 78, `crate::moby_update::classes::units::barrier_field`;
//! update level15 `0x264958`). Read from the level15 decomp and the level01 disassembly; native `f32`.
//!
//! **Spawn** `(pos, v1, v2, c1, c2, A, B, C, moby)`: position = pos, RGBA c1, ALPHA 0x44, byte9 `trunc(4) + 0x40`,
//! render kind 0, size `trunc(v1.w·210000)`, rotation `randi(255)` (**one draw**), texture `*def[60]` (level15
//! 0x1b2770: def table 0x1b2680), timer A, the two velocities packed at +0x20 / +0x24 ([`super::type02::pack`]), c1 /
//! c2 at +0x28 / +0x2c, the phase lengths A / B / C at +0x30..+0x34, the moby's pvar block at +0x38 (the port: the
//! moby + 1).
//!
//! **Update** (the rotation byte +1 on odd ticks): d = |pos − Ratchet's body point (0x13f420)|. Within 5 and with a
//! touch point (the moby's pvar +0x00, its z > 0): farther than 0.5 from it, the mote moves 0.1/d of the way toward it;
//! closer, v2 = 0.005 along that way (packed). The colour `FastTweenColor(clamp((2.5 − (d − 0.5))/2.5, 0, 1), c1,
//! c2)`. Then the phases: A pos += v1, colour tweened from transparent by 1 − t/A; B pos += lerp(t/B, v2, v1); C pos
//! += v2, colour tweened toward transparent by t/C, killed at the end.

use super::{fast_dec_timer, rec, tween_color, type02::{pack, unpack}, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 71;
/// The def entry its spawner takes the texture from (level15 `*0x1b2770`).
const DEF: u8 = 60;

/// The spawner's arguments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spawn {
    pub pos: [f32; 4],
    /// Velocity 1 (xyz) and the size (w).
    pub v1: [f32; 4],
    pub v2: [f32; 3],
    pub c1: u32,
    pub c2: u32,
    /// Phase lengths A, B, C.
    pub t: [i16; 3],
    /// The moby whose pvar block holds the touch point (+ 1; 0 none).
    pub moby: u32,
}

/// Level15 `0x2647f0`. None: the pool is full.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, a: &Spawn) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(DEF);
    let rot = rng.randi(0xff) as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, a.pos);
    rec::set_u32(r, 4, a.c1);
    r[3] = 0x44;
    r[9] = 4 + 0x40;
    r[1] = 0;
    rec::set_ff(r, 0xc, (a.v1[3] * 210_000.0) as i32 as f32);
    r[8] = rot;
    rec::set_i16(r, 10, a.t[0]);
    r[2] = def;
    rec::set_u32(r, 0x20, pack([a.v1[0], a.v1[1], a.v1[2]]));
    rec::set_u32(r, 0x24, pack(a.v2));
    rec::set_u32(r, 0x28, a.c1);
    rec::set_u32(r, 0x2c, a.c2);
    rec::set_i16(r, 0x30, a.t[0]);
    rec::set_i16(r, 0x32, a.t[1]);
    rec::set_i16(r, 0x34, a.t[2]);
    rec::set_u32(r, 0x38, a.moby);
    Some(i)
}

/// The moby a record reads the touch point of (+0x38: moby + 1).
pub fn moby_of(r: &super::Record) -> Option<usize> {
    if r[0] != TYPE { return None; }
    (rec::u32(r, 0x38) as usize).checked_sub(1)
}

fn tween(f: f32, a: u32, b: u32) -> u32 { tween_color(f.to_bits(), a, b) }
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn len(a: [f32; 3]) -> f32 { (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt() }
fn set_len(a: [f32; 3], l: f32) -> [f32; 3] { let n = len(a); if n == 0.0 { a } else { a.map(|x| x * l / n) } }

/// Update 0x28a0e0 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let odd = sys.counter & 1 != 0;
    let body = sys.hero_body;
    let touch = moby_of(&sys.pool.recs[i]).and_then(|m| sys.pvar_heads.get(&m).copied());
    let r = &mut sys.pool.recs[i];
    if odd { r[8] = r[8].wrapping_add(1); }
    let mut p = rec::v3(r, 0x10);
    let d = len(sub(p, body));
    if d < 5.0 {
        if let Some(t) = touch.filter(|t| 0.0 < t[2]) {
            let v = sub(t, p);
            if 0.5 < len(v) {
                let v = set_len(v, 0.1 / d);
                p = [p[0] + v[0], p[1] + v[1], p[2] + v[2]];
                rec::set_v3(r, 0x10, p);
            } else {
                rec::set_u32(r, 0x24, pack(set_len(v, 0.005)));
            }
        }
    }
    let f = ((2.5 - (d - 0.5)) / 2.5).clamp(0.0, 1.0);
    rec::set_u32(r, 4, tween(f, rec::u32(r, 0x28), rec::u32(r, 0x2c)));
    let t = rec::i16(r, 10) as f32;
    let add = |r: &mut super::Record, v: [f32; 3]| {
        let p = rec::v3(r, 0x10);
        rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    };
    if rec::i16(r, 0x30) != 0 {
        add(r, unpack(rec::u32(r, 0x20)));
        let c = rec::u32(r, 4);
        rec::set_u32(r, 4, tween(1.0 - t / rec::i16(r, 0x30) as f32, c & 0xff_ffff, c));
        if fast_dec_timer(r, 10) != 0 {
            let b = rec::i16(r, 0x32);
            rec::set_i16(r, 0x30, 0);
            rec::set_i16(r, 10, b);
        }
    } else if rec::i16(r, 0x32) != 0 {
        let (v1, v2) = (unpack(rec::u32(r, 0x20)), unpack(rec::u32(r, 0x24)));
        let k = t / rec::i16(r, 0x32) as f32;
        add(r, [0, 1, 2].map(|j| v2[j] + (v1[j] - v2[j]) * k));
        if fast_dec_timer(r, 10) != 0 {
            let c = rec::i16(r, 0x34);
            rec::set_i16(r, 0x32, 0);
            rec::set_i16(r, 10, c);
        }
    } else {
        add(r, unpack(rec::u32(r, 0x24)));
        let c = rec::u32(r, 4);
        rec::set_u32(r, 4, tween(t / rec::i16(r, 0x34) as f32, c & 0xff_ffff, c));
        if fast_dec_timer(r, 10) != 0 { sys.kill_part(i); }
    }
}
