//! Particle type 6, the data-driven emitter particle, and its owner, the class-27 emitter moby
//! (docs/plan/particles.md §4, §9). Read from the level01 disassembly:
//!
//! * [`emitter_update`] = class 27 update 0x2bd100,
//! * [`spawn`] = type-6 spawner 0x27e900 (`CreatePart(6)` 0x27c498),
//! * [`update`] = type-6 update 0x27eb08.
//!
//! All behaviour comes from the owner's pvars P (0xe0 bytes). Field map (offsets into P):
//!
//! | P+ | type | use |
//! |---|---|---|
//! | 0x00 | vec4 | acceleration added to the velocity each tick (flag 8; rotated by the owner if 0x8000) |
//! | 0x10 / 0x20 | vec3 | kill box min / max on the position |
//! | 0x30 | f32 | size step per tick (× speed) |
//! | 0x34 | f32 | angle step per tick (turns, × speed) |
//! | 0x38 | f32 | alpha step (flag 4, sign from +0x3b bit 0x80) |
//! | 0x3c | f32 | bounce factor of the collision branch (not ported) |
//! | 0x40 | f32 | speed limit after the acceleration (`fun_001f9c90`, rescales, not drag) |
//! | 0x44 / 0x48 | f32 | alpha min / max (flip +0x3b bit 0x80 at max if 0x800, at min if 0x80) |
//! | 0x4c / 0x50 / 0x54 | f32 | colour steps R, G, B (flag 2; signs from +0x3b bits 0x10/0x20/0x40) |
//! | 0x58..0x60 / 0x64..0x6c | f32 ×3 | colour min / max (flip at max if 0x100/0x200/0x400, at min if 0x10/0x20/0x40) |
//! | 0x70 | u32 | flags: 1 collide, 2 colour, 4 alpha, 8 accelerate, 0x1000 velocity = P+0x80 rotated by a random Euler, 0x2000 rotate the offset by the moby, 0x4000 add the moby rotation, 0x8000 rotate the acceleration by the owner |
//! | 0x74 / 0x78 | f32 | size min / max |
//! | 0x7c | ptr | the owner moby, written by the emitter every tick (port: owner index) |
//! | 0x80 / 0x90 | vec3 | velocity: base (0x1000) or range low / high (else) / Euler half-range in degrees (0x1000) |
//! | 0xa0 / 0xa4 | f32 | angle range (turns) |
//! | 0xa8 / 0xac | f32 | size range (units of 1/210000 world unit) |
//! | 0xb0..0xb8, 0xbc | f32 | spawn colour, alpha |
//! | 0xc0 | s32 | life in ticks (`ticks()`) |
//! | 0xc4 / 0xc5 | u8 | particles per burst / ticks between bursts |
//! | 0xc6 | u8 | draw byte: byte9 = (c6 >> 5)·16 + 4 (near 1 u, far 32·(c6 >> 5) u), byte3 = c6 & 1 ? 0x48 : 0x44 |
//! | 0xc7 | u8 | part_defs type whose first frame is the texture |
//! | 0xc8 | s32 | burst countdown (state) |
//! | 0xcc / 0xd0 / 0xd4 | f32 | spawn offset ranges |
//!
//! Record fields: +0x20 velocity, +0x2c owner, +0x30 alpha (f32), +0x34 angle (f32 turns), +0x38..0x3a the
//! low bytes of the 16-bit colour channels, +0x3b direction bits; +0x3c..0x3f are never written (stale).
//!
//! **Pinned emitter arguments** (0x2bd1b4..0x2bd1ec; the decompiler garbled them): offset x = `randf(−P.cc,
//! P.cc)`, **y = `randf(−P.d0, P.d4)`** (the game pairs the negated y range with the z range), z =
//! `randf(−P.d4, P.d4)`. With flag 0x1000 the Euler angles are `randf(−P.9k, P.9k)·π/180` (mul then div), and
//! with 0x4000 the game adds the moby rotation as `ex = add_rot(ex, rx); ey = add_rot(ex, ry); ez = add_rot(ex, rz)`
//! (every call is passed the *new* ex: `mov.s f12, f0` at 0x2bd2a4 and `lwc1 f12, 0x80(sp)` at 0x2bd2b4).
//! Rand order per particle: 3 offsets, 3 velocity/Euler draws, size (P.a8..ac), angle (P.a0..a4): 8 `rand()`.

use super::{fast_dec_timer, le, lt, rec, BSphereView, Owner, Particles, Record};
use crate::ps2v::{self, F, ONE};
use crate::rng::{Rng, PI};
use rc_formats::moby_light::rotation_rows;

/// Update distance of the emitter's visibility sphere test (0x43700000) and its radius (0x41a00000).
pub const EMIT_FAR: f32 = 240.0;
pub const EMIT_RADIUS: f32 = 20.0;
/// 65535.0 (0x477fff00) and 255.0 (0x437f0000).
const K65535: F = 0x477f_ff00;
const K255: F = 0x437f_0000;
const K180: F = 0x4334_0000;

type V4 = [F; 4];

/// `fun_001fa050` (0x221980) + `fun_001f9d20` (0x221608): rows from Euler angles (VU0 28259 at 0xd18, row 3 =
/// vf23 = (0, 0, 0, 1)), then `((r0·v.x + r1·v.y) + r2·v.z) + r3·v.w` per lane (`vmulax/vmadday/vmaddaz/vmaddw`).
fn rotate(euler: [F; 3], v: V4) -> V4 {
    let [r0, r1, r2] = rotation_rows(euler.map(f32::from_bits));
    let r3: V4 = [0, 0, 0, ONE];
    [0, 1, 2, 3].map(|k| {
        let acc = ps2v::add(ps2v::mul(r0[k], v[0]), ps2v::mul(r1[k], v[1]));
        let acc = ps2v::add(acc, ps2v::mul(r2[k], v[2]));
        ps2v::add(acc, ps2v::mul(r3[k], v[3]))
    })
}

/// `fast_add_rotations` 0x221ff8: `a + b` wrapped once into [−π, π) with two `sub.s`/`add.s` of π; the
/// lower test uses the unwrapped sum (it is evaluated in the branch delay slot).
pub fn fast_add_rotations(a: F, b: F) -> F {
    let (pi, npi) = (PI, PI | ps2v::SIGN);
    let s = ps2v::add(a, b);
    let below = lt(s, npi);
    if !lt(s, pi) { return ps2v::sub(ps2v::sub(s, pi), pi); }
    if below { return ps2v::add(ps2v::add(s, pi), pi); }
    s
}

/// `fun_001f9c90` (0x2214a8): if `|v| > max` (or `max − |v|` is ±0) scale v to length max; `|v|` = VU `sqrt` of
/// `(x² + y²) + 1·z²`. No change when the length and y² are both zero bits.
fn clamp_length(v: [F; 3], max: F) -> [F; 3] {
    let p = [0, 1, 2].map(|k| ps2v::mul(v[k], v[k]));
    let len2 = ps2v::add(ps2v::add(p[0], p[1]), ps2v::mul(ONE, p[2]));
    let len = ps2v::add(0, ps2v::sqrt(len2));
    let d = ps2v::sub(max, len);
    if len == 0 && p[1] == 0 { return v; }
    if d & ps2v::SIGN == 0 && d != 0 { return v; }
    let q = ps2v::div(max, len);
    v.map(|x| ps2v::mul(x, q))
}

/// Pack `trunc(c·255)` bytes (0x270ea0).
fn pack_rgba(c: [F; 4]) -> u32 {
    let b = c.map(|x| ps2v::ftoi0(ps2v::mul(x, K255)) as u32);
    (b[0] & 0xff) | (b[1] & 0xff) << 8 | (b[2] & 0xff) << 16 | b[3] << 24
}

/// Spawner 0x27e900: `(vel, size, angle, rgb, alpha, owner, pos, ticks, c6, c7, blend)`. `blend = 0xff` derives
/// byte9/byte3 from `c6`; otherwise byte9 = c6 and byte3 = blend. Returns the record, `None` if the pool is full.
#[allow(clippy::too_many_arguments)]
pub fn spawn(sys: &mut Particles, vel: [F; 3], size: F, angle: F, rgb: [F; 3], alpha: F, owner: u32, pos: V4, ticks: i32, c6: u8, c7: u8, blend: u8) -> Option<usize> {
    let i = sys.create_part(6)?;
    let first = sys.def_first(c7);
    let t = sys.time.ticks(ticks);
    let r: &mut Record = &mut sys.pool.recs[i];
    for (k, v) in pos.iter().enumerate() { rec::set_u32(r, 0x10 + 4 * k, *v); }
    if blend == 0xff {
        r[9] = ((c6 >> 5) << 4).wrapping_add(ps2v::ftoi0(0x4080_0000) as u8);
        r[3] = if c6 & 1 != 0 { 0x48 } else { 0x44 };
    } else {
        r[9] = c6;
        r[3] = blend;
    }
    r[1] = 0;
    r[2] = first;
    for (k, v) in vel.iter().enumerate() { rec::set_f(r, 0x20 + 4 * k, *v); }
    rec::set_f(r, 0x34, angle);
    for k in 0..3 { r[0x38 + k] = ps2v::ftoi0(ps2v::mul(rgb[k], K65535)) as u8; }
    rec::set_f(r, 0x30, alpha);
    rec::set_u32(r, 0x2c, owner);
    rec::set_f(r, 0xc, size);
    r[8] = ps2v::ftoi0(ps2v::mul(angle, K255)) as u8;
    rec::set_i16(r, 0xa, t as i16);
    r[0x3b] = 0;
    rec::set_u32(r, 4, pack_rgba([rgb[0], rgb[1], rgb[2], alpha]));
    Some(i)
}

/// Update 0x27eb08 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let speed = sys.time.speed;
    let owner_ix = rec::u32(&sys.pool.recs[i], 0x2c) as usize;
    let Some(owner) = sys.owners.get(owner_ix) else {
        // No such owner: the PS2 would read through a stale pointer. Treat as unported and drop it.
        sys.stats.unported_kills[6] += 1;
        sys.kill_part(i);
        return;
    };
    let p = |o: usize| owner.pf(o);
    let flags = owner.pu32(0x70);
    // Owner rotation for flag 0x8000 (P+0x7c → moby+0x40).
    let accel_rot = (flags & 0x8000 != 0).then(|| {
        let m = owner.pu32(0x7c) as usize;
        sys.owners.get(m).map_or([0.0; 3], |o| o.rot)
    });
    let accel: V4 = [p(0), p(4), p(8), p(0xc)];
    let pv: Vec<F> = (0..0xe0 / 4).map(|k| p(4 * k)).collect();
    let q = |o: usize| pv[o / 4];
    let mut unported_collision = false;

    let r = &mut sys.pool.recs[i];
    // (1) p' = pos + vel (FPU adds); the collision branch (flag 1: CollLine_Fix, reflect, re-place) is not ported.
    let np = [0, 1, 2].map(|k| ps2v::add(rec::f(r, 0x10 + 4 * k), rec::f(r, 0x20 + 4 * k)));
    if flags & 1 != 0 { unported_collision = true; }
    // The game stores the whole stack qw, whose w is an uninitialised stack word; +0x1c is left as it was here.
    for (k, v) in np.iter().enumerate() { rec::set_f(r, 0x10 + 4 * k, *v); }

    let flip = |r: &mut Record, bit: u8| r[0x3b] ^= bit;
    if flags & 2 != 0 {
        // (2) 16-bit colour channels: hi byte from +4, lo byte from +0x38..0x3a.
        let w = rec::u32(r, 4);
        let ch = [(r[4] as u32) << 8 | r[0x38] as u32, (w & 0xff00) | r[0x39] as u32, ((w >> 8) & 0xff00) | r[0x3a] as u32];
        let mut c = ch.map(|v| ps2v::div(ps2v::itof0(v as i32), K65535));
        let dirs = r[0x3b];
        for (k, bit) in [0x10u8, 0x20, 0x40].into_iter().enumerate() {
            let step = ps2v::mul(q(0x4c + 4 * k), speed);
            c[k] = if dirs & bit != 0 { ps2v::sub(c[k], step) } else { ps2v::add(c[k], step) };
        }
        // Clamp to [min, max]; flip the channel's direction at the max if flags 0x100 << k, at the min if 0x10 << k.
        for (k, ck) in c.iter_mut().enumerate() {
            let (lo, hi, bit) = (q(0x58 + 4 * k), q(0x64 + 4 * k), 0x10u8 << k);
            if lt(hi, *ck) {
                *ck = hi;
                if flags & (0x100 << k) != 0 { flip(r, bit); }
            }
            if lt(*ck, lo) {
                *ck = lo;
                if flags & (0x10 << k) != 0 { flip(r, bit); }
            }
        }
        for k in 0..3 { r[0x38 + k] = ps2v::ftoi0(ps2v::mul(c[k], K65535)) as u8; }
        let alpha = rec::f(r, 0x30);
        rec::set_u32(r, 4, pack_rgba([c[0], c[1], c[2], alpha]));
    } else if flags & 4 != 0 {
        let a = ps2v::ftoi0(ps2v::mul(rec::f(r, 0x30), K255)) as u32;
        let w = rec::u32(r, 4);
        rec::set_u32(r, 4, (w & 0x00ff_ffff) | a << 24);
    }
    if flags & 4 != 0 {
        // (3) alpha ± P.38·speed, clamp [P.44, P.48].
        let step = ps2v::mul(q(0x38), speed);
        let a = rec::f(r, 0x30);
        let mut a = if r[0x3b] & 0x80 != 0 { ps2v::sub(a, step) } else { ps2v::add(a, step) };
        if lt(q(0x48), a) {
            a = q(0x48);
            if flags & 0x800 != 0 { flip(r, 0x80); }
        }
        if lt(a, q(0x44)) {
            a = q(0x44);
            if flags & 0x80 != 0 { flip(r, 0x80); }
        }
        rec::set_f(r, 0x30, a);
    }
    // (4) rotation byte from the angle before this tick's step.
    r[8] = ps2v::ftoi0(ps2v::mul(rec::f(r, 0x34), K255)) as u8;
    if flags & 8 != 0 {
        // (5) vel += acceleration (rotated by the owner with 0x8000), then limit |vel| to P.40.
        let a = match accel_rot { Some(rot) => rotate(rot.map(f32::to_bits), accel), None => accel };
        let v = [0, 1, 2].map(|k| ps2v::add(rec::f(r, 0x20 + 4 * k), a[k]));
        let v = clamp_length(v, q(0x40));
        for (k, x) in v.iter().enumerate() { rec::set_f(r, 0x20 + 4 * k, *x); }
    }
    // (6) size += P.30·speed, clamp [P.74, P.78].
    let mut s = ps2v::add(rec::f(r, 0xc), ps2v::mul(q(0x30), speed));
    if lt(s, q(0x74)) { s = q(0x74); }
    if lt(q(0x78), s) { s = q(0x78); }
    rec::set_f(r, 0xc, s);
    // (7) angle += P.34·speed.
    rec::set_f(r, 0x34, ps2v::add(rec::f(r, 0x34), ps2v::mul(q(0x34), speed)));
    // (8) kill: alpha ≤ 0, the timer, or outside the box.
    let kill = le(rec::f(r, 0x30), 0) || fast_dec_timer(r, 0xa) != 0 || {
        let pos = [rec::f(r, 0x10), rec::f(r, 0x14), rec::f(r, 0x18)];
        (0..3).any(|k| lt(pos[k], q(0x10 + 4 * k))) || (0..3).any(|k| lt(q(0x20 + 4 * k), pos[k]))
    };
    if unported_collision { sys.stats.unported_collision += 1; }
    if kill { sys.kill_part(i); }
}

/// Class-27 emitter update 0x2bd100 for owner `owner` (index into [`Particles::owners`]). `view` is the camera
/// state of the previous frame's render; `level` is 0x15ed84.
///
/// `view = None` is the view before the level's first render, as in the load pass (`MobyLoadTimeUpdatePass`,
/// from `LoadLevelCoreData`): 0x16d140..0x16d17f lie in the level overlay's `.data` and hold its file bytes,
/// all zero, until `BuildRotationViewProj` (DrawWorld) first writes them. `FastBSphereCheck` 0x2221f0 scales
/// the sphere by the view's w (0x16d17c, 1024 once built) with `vmulw`, so on the zero view the radius is 0
/// and "behind the eye" (`0 − (r + c.z)` = +0, sign clear) returns −1: on Novalis every emitter is culled in
/// the load pass and draws nothing (the savestate's stream: nothing between the last bolt and the first
/// crate). Other levels skip the test, so their emitters spawn there.
pub fn emitter_update(sys: &mut Particles, rng: &mut Rng, owner: usize, view: Option<&BSphereView>, level: u32) {
    let o: &Owner = &sys.owners[owner];
    if level == 1 {
        let s = [o.pos[0], o.pos[1], o.pos[2], EMIT_RADIUS];
        match view {
            None => return,
            Some(v) if v.culled(EMIT_FAR, s) => return,
            Some(_) => {}
        }
    }
    let o = &mut sys.owners[owner];
    o.set_pu32(0x7c, owner as u32);
    let count = o.pu32(0xc8) as i32;
    if count >= 1 {
        o.set_pu32(0xc8, (count - 1) as u32);
        return;
    }
    o.set_pu32(0xc8, o.pvars[0xc5] as u32);
    if o.pvars[0xc4] == 0 { return; }
    let speed = sys.time.speed;
    let mut k = 0;
    loop {
        let o = &sys.owners[owner];
        let p = |off: usize| o.pf(off);
        let flags = o.pu32(0x70);
        let mut pos: V4 = [o.pos[0].to_bits(), o.pos[1].to_bits(), o.pos[2].to_bits(), o.pos[3].to_bits()];
        let neg = |x: F| x ^ ps2v::SIGN;
        let mut off: V4 = [
            rng.randf_bits(neg(p(0xcc)), p(0xcc)),
            rng.randf_bits(neg(p(0xd0)), p(0xd4)),
            rng.randf_bits(neg(p(0xd4)), p(0xd4)),
            0, // sp+0x3c: stack word, only multiplied by row 3's zero xyz
        ];
        let rot = o.rot.map(f32::to_bits);
        if flags & 0x2000 != 0 { off = rotate(rot, off); }
        for k in 0..3 { pos[k] = ps2v::add(pos[k], off[k]); }
        let vel: V4 = if flags & 0x1000 == 0 {
            let mut v: V4 = [0, 1, 2, 3].map(|k| if k < 3 { ps2v::mul(rng.randf_bits(p(0x80 + 4 * k), p(0x90 + 4 * k)), speed) } else { 0 });
            if flags & 0x4000 != 0 { v = rotate(rot, v); }
            v
        } else {
            let mut e = [0, 1, 2].map(|k| ps2v::div(ps2v::mul(rng.randf_bits(neg(p(0x90 + 4 * k)), p(0x90 + 4 * k)), PI), K180));
            if flags & 0x4000 != 0 {
                let ex = fast_add_rotations(e[0], rot[0]);
                e = [ex, fast_add_rotations(ex, rot[1]), fast_add_rotations(ex, rot[2])];
            }
            rotate(e, [p(0x80), p(0x84), p(0x88), p(0x8c)])
        };
        let size = rng.randf_bits(p(0xa8), p(0xac));
        let angle = rng.randf_bits(p(0xa0), p(0xa4));
        let (rgb, alpha, ticks) = ([p(0xb0), p(0xb4), p(0xb8)], p(0xbc), o.pu32(0xc0) as i32);
        let (c6, c7) = (o.pvars[0xc6], o.pvars[0xc7]);
        spawn(sys, [vel[0], vel[1], vel[2]], size, angle, rgb, alpha, owner as u32, pos, ticks, c6, c7, 0xff);
        k += 1;
        if k >= sys.owners[owner].pvars[0xc4] as i32 { break; }
    }
}

/// [`emitter_update`] with the owner's live moby: the game's emitter reads its moby (+0x10 position, +0x40
/// rotation) and its pvar block P every tick, and other mobys write both: the Blarg flyers (class 660,
/// `FlyerPathDriver` 0x2f5168) put their linked emitters at a joint every tick, with P+0x80..0x8b = the unit
/// direction joint − flyer and P+0x70 |= 0x1000. The owner's copy ([`Owner`]) takes the moby's position,
/// rotation, P+0x70 and P+0x80..0x8b first; the emitter's own state (P+0x7c, the burst countdown P+0xc8) stays in
/// the owner. The cull, the spawn position, the velocity and the particles' flag reads (type 6 update) then see
/// the live values.
pub fn emitter_update_live(sys: &mut Particles, rng: &mut Rng, owner: usize, view: Option<&BSphereView>, level: u32, moby: &crate::moby_runtime::Moby) {
    let Some(o) = sys.owners.get_mut(owner) else { return };
    o.pos = moby.position;
    o.rot = [moby.rotation[0], moby.rotation[1], moby.rotation[2]];
    if moby.pvars.len() >= 0x8c && o.pvars.len() >= 0x8c {
        o.pvars[0x70..0x74].copy_from_slice(&moby.pvars[0x70..0x74]);
        o.pvars[0x80..0x8c].copy_from_slice(&moby.pvars[0x80..0x8c]);
    }
    emitter_update(sys, rng, owner, view, level);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(x: f32) -> F { x.to_bits() }

    #[test]
    fn live_owner_follows_the_moby() {
        let mut s = system();
        let mut rng = Rng::new();
        rng.srand(1234);
        let mut m = crate::moby_runtime::Moby::init_instance(285, 27, None);
        m.pvars = novalis_pvars();
        // The flyer moved the emitter 1000 units away: the camera no longer sees it, so nothing spawns.
        m.position = [213.26, 1380.3, 79.9, 0.0];
        emitter_update_live(&mut s, &mut rng, 0, Some(&seeing()), 1, &m);
        assert_eq!((s.pool.count, rng.state), (0, 1234));
        // Back in view with a new direction: the particle starts at the moby and flies along P+0x80.
        m.position = [213.26, 380.3, 79.9, 0.0];
        for (k, x) in [0.0f32, 0.0, -1.0].iter().enumerate() { m.pvars[0x80 + 4 * k..0x84 + 4 * k].copy_from_slice(&x.to_bits().to_le_bytes()); }
        emitter_update_live(&mut s, &mut rng, 0, Some(&seeing()), 1, &m);
        assert_eq!(s.pool.count, 1);
        let r = &s.pool.recs[0];
        let v = [0x20, 0x24, 0x28].map(|o| f32::from_bits(rec::u32(r, o)));
        assert!(v[2] < -0.99 && v[0].abs() < 0.01 && v[1].abs() < 0.01, "{v:?}");
        let p = rec::pos(r);
        assert!((p[1] - 380.3).abs() < 1.0);
    }

    /// The Novalis class-27 pvar block (all ten instances share it), as floats and words.
    fn novalis_pvars() -> Vec<u8> {
        let mut p = vec![0u8; 0xe0];
        let mut put = |o: usize, w: u32| p[o..o + 4].copy_from_slice(&w.to_le_bytes());
        for (o, w) in [
            (0x20, 0x447a0000), (0x24, 0x447a0000), (0x28, 0x447a0000),
            (0x30, 0x43fa0000), (0x34, 0xbb032336), (0x38, 0xbbdb8bac), (0x3c, 0x3be53650),
            (0x40, 0x3c75c28f), (0x48, 0x3f800000), (0x4c, 0x3a9bf9c6), (0x50, 0x3a902de0), (0x54, 0x3a83126f),
            (0x64, 0x3f800000), (0x68, 0x3f800000), (0x6c, 0x3f800000), (0x70, 0x170e), (0x78, 0x4a927c00),
            (0x80, 0x3f09881a), (0x84, 0x3f54f55e), (0x88, 0xbe0e8816), (0x90, 0x3e99999a), (0x94, 0x3e99999a), (0x98, 0x3e99999a),
            (0xa4, 0x3f800000), (0xa8, 0x48435000), (0xac, 0x48975e00),
            (0xb0, 0x3e19999a), (0xb4, 0x3e19999a), (0xb8, 0x3e19999a), (0xbc, 0x3f800000), (0xc0, 0x96), (0xc4, 0x04f10001),
        ] { put(o, w); }
        p
    }

    fn system() -> Particles {
        let owner = Owner { instance: 285, pos: [213.26, 380.3, 79.9, 0.0], rot: [0.0, 0.0, 0.165_785_63], pvars: novalis_pvars() };
        Particles::new(None, vec![owner])
    }

    /// A camera 30 units in front of the emitter, looking at it.
    fn seeing() -> BSphereView {
        BSphereView::from_camera([213.26, 350.3, 79.9], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.63, 0.48825)
    }

    #[test]
    fn emitter_spawns_one_per_tick_with_novalis_pvars() {
        let mut s = system();
        let mut rng = Rng::new();
        rng.srand(1234);
        emitter_update(&mut s, &mut rng, 0, Some(&seeing()), 1);
        assert_eq!(s.pool.count, 1);
        // Eight rand() calls per particle.
        let mut check = Rng::new();
        check.srand(1234);
        for _ in 0..8 { check.rand(); }
        assert_eq!(rng.state, check.state);
        let r = &s.pool.recs[0];
        assert_eq!((r[0], r[1], r[2], r[3], r[9]), (6, 0, 0, 0x48, 0x74), "type, flags, texture def[4][0], additive, near 1 / far 224");
        assert_eq!(rec::i16(r, 0xa), 150);
        assert_eq!(rec::u32(r, 4), 0xff26_2626, "RGBA = trunc(0.15·255) ×3, A = trunc(1·255)");
        assert_eq!(&r[0x38..0x3c], &[0x66, 0x66, 0x66, 0], "low bytes of trunc(0.15·65535) = 9830 = 0x2666");
    }

    #[test]
    fn particle_lives_150_ticks_and_follows_the_pvars() {
        let mut s = system();
        let mut rng = Rng::new();
        rng.srand(1234);
        emitter_update(&mut s, &mut rng, 0, Some(&seeing()), 1);
        // Stop further emission: burst size 0.
        s.owners[0].pvars[0xc4] = 0;
        let r0 = s.pool.recs[0];
        let v0 = [0x20, 0x24, 0x28].map(|o| f32::from_bits(rec::u32(&r0, o)));
        // Velocity: P+0x80 turned by ≤ 0.3° per axis → length 1 ± 1e-5, direction close to (0.537, 0.832, −0.139).
        let len = v0.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((len - 1.0).abs() < 1e-3, "{v0:?}");
        let size0 = f32::from_bits(rec::u32(&r0, 0xc));
        assert!((200_000.0..310_000.0).contains(&size0));
        let mut ticks = 0;
        let mut prev = rec::pos(&r0);
        let mut ref_red = (r0[4] as u32, r0[0x38] as u32);
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            ticks += 1;
            if s.pool.count == 0 { break; }
            let r = &s.pool.recs[0];
            let p = rec::pos(r);
            let v = [0x20, 0x24, 0x28].map(|o| f32::from_bits(rec::u32(r, o)));
            let speed = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            // After the first tick the speed is clamped to 0.015 (the accel is 0).
            assert!(speed <= 0.015 * (1.0 + 1e-6), "tick {ticks}: speed {speed}");
            let step = (0..3).map(|k| (p[k] - prev[k]).powi(2)).sum::<f32>().sqrt();
            if ticks == 1 { assert!((step - len).abs() < 1e-3, "first move uses the spawn velocity"); }
            prev = p;
            let a = f32::from_bits(rec::u32(r, 0x30));
            assert!((a - (1.0 - 0.0067 * ticks as f32)).abs() < 1e-4, "alpha {a} at {ticks}");
            let sz = f32::from_bits(rec::u32(r, 0xc));
            assert!((sz - (size0 + 500.0 * ticks as f32)).abs() < 1.0, "size");
            // Red: nominally +0.00119/tick from 0.15, but each tick rebuilds the 16-bit value from
            // trunc(v·255) << 8 | (trunc(v·65535) & 0xff), which is up to 256 short (255·256 ≠ 65535), so the
            // channel creeps up at ~0.0004/tick and stalls near 0.2: the game's own arithmetic. Reference in f64.
            let (h, l) = ref_red;
            let x = ((h << 8 | l) as f64) / 65535.0 + 0.00119f32 as f64;
            ref_red = (((x * 255.0) as u32) & 0xff, ((x * 65535.0) as u32) & 0xff);
            let red = (r[4] as u32) << 8 | r[0x38] as u32;
            let want = ref_red.0 << 8 | ref_red.1;
            assert!(red.abs_diff(want) <= 2, "tick {ticks}: red {red} vs reference {want}");
        }
        // Alpha 1 − 0.0067·t reaches ≤ 0 at t = 150 as the timer runs out: the particle dies on its 150th update.
        assert_eq!(ticks, 150);
        assert_eq!(s.pool.hw, -1);
    }

    #[test]
    fn clamp_length_rescales_only_when_too_long() {
        let v = [f(0.3), f(0.4), f(0.0)];
        let c = clamp_length(v, f(0.1));
        let len = c.iter().map(|&x| f32::from_bits(x).powi(2)).sum::<f32>().sqrt();
        assert!((len - 0.1).abs() < 1e-6);
        assert_eq!(clamp_length(v, f(1.0)), v);
        assert_eq!(clamp_length([0, 0, 0], f(1.0)), [0, 0, 0]);
    }

    #[test]
    fn add_rotations_wraps_once() {
        let g = |a: f32, b: f32| f32::from_bits(fast_add_rotations(f(a), f(b)));
        assert_eq!(g(1.0, 1.0), 2.0);
        assert!((g(3.0, 1.0) - (4.0 - 2.0 * std::f32::consts::PI)).abs() < 1e-6);
        assert!((g(-3.0, -1.0) - (-4.0 + 2.0 * std::f32::consts::PI)).abs() < 1e-6);
    }

    #[test]
    fn culled_emitter_does_not_draw_random_numbers() {
        let mut s = system();
        let mut rng = Rng::new();
        rng.srand(1234);
        // Camera 1000 units away looking away from the emitter.
        let v = BSphereView::from_camera([213.0, 1380.0, 80.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.63, 0.48825);
        emitter_update(&mut s, &mut rng, 0, Some(&v), 1);
        assert_eq!(s.pool.count, 0);
        assert_eq!(rng.state, 1234);
        // Another level skips the test.
        emitter_update(&mut s, &mut rng, 0, Some(&v), 2);
        assert_eq!(s.pool.count, 1);
    }

    #[test]
    fn load_pass_view_culls_on_novalis_only() {
        let mut s = system();
        let mut rng = Rng::new();
        rng.srand(1234);
        emitter_update(&mut s, &mut rng, 0, None, 1);
        assert_eq!((s.pool.count, rng.state), (0, 1234), "the zero view before the first render culls: no draws");
        emitter_update(&mut s, &mut rng, 0, None, 2);
        assert_eq!(s.pool.count, 1, "no view test on other levels");
    }
}
