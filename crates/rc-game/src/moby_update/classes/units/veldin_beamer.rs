//! **Veldin's beam drones, class 1440, and their beam manager, class 1471** (level00 `0x2e0b88` and `0x2e1df0`, census
//! U36 / U37; nine 1440s around Clank's crash site, one 1471). A 1440 is a small creature on the shared creature layer
//! (damage record +0x20, hit flash +0x60, knockback +0x70, target record +0xd0): it flies in along its arrival path,
//! waits, and once it has a target turns to it and fires a crackling beam from its joint list 0, five units along its
//! facing, for three loops of its firing animation; the beam's line hits what it touches (Ratchet first). Two hits kill
//! it (a death flight, the death explosion, its pieces). The beams themselves belong to 1471: three slots (level00
//! .data 0x161bf8.., 0x1e6d20..) a 1440 claims, aims every tick and frees; 1471 bends their two strands, throws their
//! sparks and draws them. Read from the level00 decomp and disassembly. Native `f32`; the draws at the game's points.
//!
//! **1440 pvars** (0x1e0): the creature header (+0x00 the record offsets), +0x20 the damage record (health 2.0), +0x38
//! the lure, +0x60 the hit flash, +0x70 the knockback record, +0xd0 the target record (`target::Target`: +0x110 the
//! moby, +0x114 the kind), +0x120 the head's look-at record, +0x1a0 home, +0x1b4 the area path (the target region),
//! +0x1b8 the arrival path (−1 none), +0x1bc a second target moby (an instance index, −1 none), +0x1c0 the eye point,
//! +0x1d0 the turn velocity, +0x1d4 the path speed, +0x1d8 (s16) the beam's loops, +0x1da (s16) the alert timer, +0x1dc
//! the path node.
//!
//! ## Coverage: 1440 (`0x2e0b88`, `0x2e1678` the tick, `0x2e1aa8` the tail, `0x2e1c78` its draw)
//! | address | what | port |
//! |---|---|---|
//! | `0x2e1678` (state ≠ 0) | the lure → the alert timer `ticks(rand_range(180, 300))`, the timer steps; `MobyGetHitMessage(0x330000)` into the resolver (column 4): damage with out5 ≠ 1, not dying → the knockback record (gravity 28·dt², drag 5·dt², out 10·dt, up 9.5·dt, radius 0x200, z 0.5, air 2·dt); health − damage ≤ 0: flags 0x29, untargetable, `0x25ab48` aim, sequence 0xb, keys 7 / 16, state 8, no collision, red flash 0x78, `SetDeathBits`; else out ×0.35, up ×0.5, sequence 8 along the hit, keys −1, state 7, flash 0xfa, cooldown +0x26 = `ticks(60)`; the beam released (`0x2e2040`); the flash steps | [`pre`] |
//! | | the target: state 1 the region search (64, area path); not state 5: `0x25fcb8` (24, or 37 alerted), out of range → kind 2; none → Ratchet; the second target +0x1bc nearer (xy) → it, kind 1 | [`pre`] |
//! | head | the big-head scale (2.5, +0x120), with the cheat the look-at (0.03, 0.3, list 2); drawn and within 27 of the camera: the shadow probe, +0x7f = 0x15 | [`update`] |
//! | state 0 | home; health 2, +0x24 = 2, +0x28 = 1, +0x29 = 0, +0x58 = 14, +0x5a = 8; node 1; mirrored on `randi(2)`; no arrival path → 3; no area path → deleted; else at the path's first point → 1; sequence 1 (blend 5) | [`update`] |
//! | state 1 | at a wrap: sequence 1 ↔ 2 (`randi(4)` = 0); a target (kind ≠ 2) → 2, sequence 9 | [`update`] |
//! | state 2 | along the arrival path: `0x25be00` turn to the node (4π·dt², 4π·dt) and `0x25b970` the speed toward the distance to the path's end (8·dt², 16·dt², 12·dt); within twice the speed the next node; the last → 3, sequence 1 | [`update`] |
//! | state 3 | turn to the target; Ratchet within 20 (xy): sequence 4, else at a wrap 1 or 2; a target and a free beam slot (`0x2e1f28`) → 4, sequence 5 | [`update`] |
//! | state 4 | turn; at a wrap the loops 0 → 5, sequence 6 | [`update`] |
//! | state 5 | the turn to the target ∓ 0.15 rad (π/2·dt², 4π·dt); joint list 0's matrix; the beam's end 5 along the facing (xy); the hit template (along the facing, 1, 5627.97; flags 0x10000, \| 1 once the key time passes 1; damage 1; type 0 / 1; the class); `CollLine_Fix` joint → its centre + 0.5 and joint → end (the end cut there): Ratchet hit while it damages → the beam released, 6, sequence 7; `0x2e2190` the slot aimed; at a wrap the loops + 1, the fourth → released, 6, sequence 0xc | [`fire`] |
//! | state 6 | turn; at a wrap → 3, sequence 1 | [`update`] |
//! | state 7 | the knockback flight; landed → 3, sequence 1 | [`update`] |
//! | state 8 | the death flight; landed: the death explosion (0.75, 13), the pieces 0x7ab and `BreakFxB` 0x78d / 0x78e / 0x78f, deleted | [`update`] |
//! | `0x2e1aa8` | the eye (joint list 1, 0.333 down) into +0x1c0; a twinkle (type 69) drifting `fun_00213358(0.005, 0.03)` sized `randf(6000, 32000)`; three still ones (sized `randf(20000, 120000)`, the third 180000 on `randi(8)` = 0, life `ticks(2)`, mode 3); `RegisterDrawCallback2(0x2e1c78)`: the glow quad 0.2 at the eye | [`tail`], [`glow_quads`] |
//!
//! ## Coverage: 1471 and the beam slots
//! | address | what | port |
//! |---|---|---|
//! | `0x2e1df0` state 0 | the slots cleared, update distance 0xff → 1 | [`manager`] |
//! | state 1 | per slot: claimed (1) waits; not aimed this tick (4) → released (`0x2e20f8`); else `0x2e2250`, then 4; any → `RegisterDrawCallback2(0x2e2af0)` | [`manager`] |
//! | `0x2e1f28` / `0x2e2040` / `0x2e20f8` / `0x2e2190` | claim (the owner's slot, or the first free: lights, follow, phases 0, lights −1), release by owner / by slot (their point lights freed), aim (1 → 2, 4 → 3; the matrix and the end) | [`claim`], [`release`], [`aim`] |
//! | `0x2e2250` | the strands: on the first aim straight (20 points, step = length / 20); the phases + 10°, − 46°, + 3°; strand A z = (0.1 + 0.3·i/20)·sin(a + 36°·i); strand B y = 0.3·cos(b + 57°·i), z = 0.5·sin(c + 12°·i)·sin(b + 57°·i) + `randf_sym(0, 0.2)`; the frame (along the beam, `0x13f5e0` for up); strand A's point 18 is the beam's tip; sparks (type 69) at the source (along the beam's x / y, and one still 0.3 along x on `randi(3)` = 0) and the tip; the point lights (flags 1 / 2, radius 5 (gp−0x5010)) | [`effects`] |
//! | `0x2e2af0` | per drawn slot: the scroll − 0.2·speed (wrapping past −8); `0x2e3388` the glows; `0x2e2c88` the two ribbons of each strand | [`frame`] ([`glows`], [`ribbon`]) |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, damage, flash, fx, knock, target, turn};
use crate::moby_update::services::{self as sv, HitTemplate, World};
use crate::ps2v::Pf;

use super::{FxQuad, FxQuads, GlowQuad};

pub const REFERENCE_LEVEL: u32 = 0;
pub const UPDATE_FN: u32 = 0x2e_0b88;
pub const CLASSES: [i16; 1] = [1440];
/// 1440's draw callback (the eye's glow).
pub const EYE_FN: u32 = 0x2e_1c78;
pub const MANAGER_FN: u32 = 0x2e_1df0;
pub const MANAGER_CLASSES: [i16; 1] = [1471];
/// 1471's draw callback (the beams).
pub const BEAM_FN: u32 = 0x2e_2af0;

mod pv {
    pub const HP: usize = 0x20;
    pub const LURE: usize = 0x38;
    pub const FLASH: usize = 0x60;
    pub const K: usize = 0x70;
    pub const TGT: usize = 0xd0;
    pub const TGT_MOBY: usize = 0x110;
    pub const TGT_KIND: usize = 0x114;
    pub const HEAD: usize = 0x120;
    pub const HOME: usize = 0x1a0;
    pub const AREA: usize = 0x1b4;
    pub const ARRIVE: usize = 0x1b8;
    pub const ALT: usize = 0x1bc;
    pub const EYE: usize = 0x1c0;
    pub const TURN: usize = 0x1d0;
    pub const SPEED: usize = 0x1d4;
    pub const LOOPS: usize = 0x1d8;
    pub const ALERT: usize = 0x1da;
    pub const NODE: usize = 0x1dc;
    pub const LEN: usize = 0x1e0;
}

/// gp−0x5030..−0x5010 (0x161bd0..): the target range, the knockback's gravity / drag / out / up, the beam's length,
/// its loops, the aim's yaw offset, the lights' radius.
const RANGE: f32 = 24.0;
const GRAVITY: f32 = 28.0;
const DRAG: f32 = 5.0;
const OUT: f32 = 10.0;
const UP: f32 = 9.5;
const BEAM_LEN: f32 = 5.0;
const LOOPS: i16 = 3;
const AIM_OFFSET: f32 = -0.15;
const LIGHT_RADIUS: f32 = 5.0;

const DT: f32 = 1.0 / 60.0;
const DT2: f32 = DT * DT;
const FOUR_PI: f32 = f32::from_bits(0x4149_0fdb);
const HALF_PI: f32 = f32::from_bits(0x3fc9_0fdb);

/// The beam slots (level00 0x161bf8.. / 0x1e6d20..): the manager's and its drones' shared state.
#[derive(Clone, Debug, Default)]
pub struct Beams {
    pub slots: [Slot; 3],
    /// The quads the last draw-time callback (`0x2e2af0`) made, for the renderer.
    pub draw: Vec<FxQuads>,
}

/// One beam slot.
#[derive(Clone, Debug, Default)]
pub struct Slot {
    /// 0x161c88: the drone (None free).
    pub owner: Option<MobyId>,
    /// 0x161bf8: 0 free, 1 claimed, 2 aimed first, 3 aimed again, 4 run this tick.
    pub state: i32,
    /// 0x161c08 / 0x161c18 / 0x161c28: the strands' phases.
    pub phase: [f32; 3],
    /// 0x161c38 / 0x161c48: the point lights at the source and at the tip (−1 none).
    pub lights: [i32; 2],
    /// 0x161c58: which lights (1 the source, 2 the tip).
    pub light_flags: u32,
    /// 0x161c68: 1 → the still sparks follow (type-69 modes 1 / 2).
    pub follow: i32,
    /// 0x161c78: the ribbon's texture scroll.
    pub scroll: f32,
    /// 0x1e6d20 + 0x40·k: the drone's joint matrix (rows; row 3 the source).
    pub matrix: [[f32; 4]; 4],
    /// 0x1e6de0 + 0x10·k: the end the drone aims at.
    pub end: [f32; 4],
    /// 0x1e6e10 + 0x10·k: strand A's point 18 in the world (the tip).
    pub tip: [f32; 4],
    /// 0x1e6e40 + 0x140·(2k + s): the two strands' 20 points in the beam's frame.
    pub strands: [[[f32; 4]; 20]; 2],
}

fn beams<'a>(w: &'a mut World<'_>) -> &'a mut Beams { &mut w.svc.units.veldin_beams }
fn tgt_moby(w: &World, id: MobyId) -> Option<MobyId> {
    let v = c::pi32(w, id, pv::TGT_MOBY);
    (v > 0).then(|| (v - 1) as MobyId).filter(|&m| m < w.table.mobys.len())
}
fn kind(w: &World, id: MobyId) -> i32 { c::pi32(w, id, pv::TGT_KIND) }
fn path(w: &World, id: MobyId, o: usize) -> Option<usize> { usize::try_from(c::pi32(w, id, o)).ok().filter(|&p| p < w.svc.splines.len()) }
fn path_point(w: &World, p: usize, i: usize) -> c::V { w.svc.splines[p].get(i).map(|q| q.map(f32::from_bits)).unwrap_or([0.0; 4]) }
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn seq_b(w: &World, id: MobyId) -> u8 { w.m(id).anim.seq_b }
fn blend(w: &mut World, id: MobyId, seq: u8, t: i32) {
    let t = w.ticks(t);
    w.anim_blend(id, seq, 0, t);
}
/// `0x25be00` toward `q` (xy) with `accel`·dt² and `max`·dt (module table).
fn turn_to(w: &mut World, id: MobyId, q: c::V, accel: f32) {
    let p = c::pos(w, id);
    let a = c::atan(q[0] - p[0], q[1] - p[1]);
    turn_by(w, id, a, accel);
}
fn turn_by(w: &mut World, id: MobyId, a: f32, accel: f32) {
    let (mut yaw, mut v) = (c::yaw(w, id), c::pf(w, id, pv::TURN));
    turn::turn_toward(a, DT2 * accel, DT2 * accel, DT * FOUR_PI, &mut yaw, &mut v);
    c::set_yaw(w, id, yaw);
    c::set_pf(w, id, pv::TURN, v);
}
fn add3(a: [f32; 4], b: [f32; 4]) -> [f32; 4] { [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3]] }
fn sub3(a: [f32; 4], b: [f32; 4]) -> [f32; 4] { [a[0] - b[0], a[1] - b[1], a[2] - b[2], 0.0] }
fn scale3(a: [f32; 4], k: f32) -> [f32; 4] { [a[0] * k, a[1] * k, a[2] * k, a[3]] }
fn len3(a: [f32; 4]) -> f32 { (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt() }
/// `FastVecNormalize(l, out, v)`: `v` scaled to length `l` (w kept).
fn with_len(a: [f32; 4], l: f32) -> [f32; 4] {
    let n = len3(a);
    if n == 0.0 { return a; }
    scale3(a, l / n)
}
/// `FastVecCross(out, a, b)` = b × a.
fn cross(a: [f32; 4], b: [f32; 4]) -> [f32; 4] { [b[1] * a[2] - b[2] * a[1], b[2] * a[0] - b[0] * a[2], b[0] * a[1] - b[1] * a[0], 0.0] }
/// `M·v` with row 3 the translation (`fun_001f9d20`).
fn xform(m: &[[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] { std::array::from_fn(|k| if k == 3 { 1.0 } else { m[0][k] * v[0] + m[1][k] * v[1] + m[2][k] * v[2] + m[3][k] }) }
/// The rotation part only (`matrix_mul_vec3`).
fn rotate(m: &[[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] { std::array::from_fn(|k| if k == 3 { v[3] } else { m[0][k] * v[0] + m[1][k] * v[1] + m[2][k] * v[2] }) }
/// The gravity direction 0x13f5e0 (Ratchet's frame: down).
fn gravity(w: &World) -> [f32; 4] {
    let g = w.hero.gravity_dir.map(|x| f32::from_bits(x.0));
    [g[0], g[1], g[2], 0.0]
}
/// The beam's frame (`0x2e2250` / `0x2e2c88`): x along `v`, y = x × gravity (normalised), z = x × y; row 3 `origin`.
fn beam_frame(w: &World, origin: [f32; 4], v: [f32; 4]) -> [[f32; 4]; 4] {
    let f = with_len(v, 1.0);
    let r = with_len(cross(f, gravity(w)), -1.0);
    let u = cross(r, f);
    [f, r, u, [origin[0], origin[1], origin[2], 1.0]]
}
/// `fun_00213358(lo, hi, out)`: two angles, then the length.
fn rand_polar(w: &mut World, lo: f32, hi: f32) -> [f32; 4] {
    let a = w.rng.rand_angle();
    let b = w.rng.rand_angle();
    let l = w.rng.randf(lo, hi);
    let p = crate::targeting::polar(l, a, b);
    [p[0], p[1], p[2], 0.0]
}
/// `0x274948` (type 69), with its record when the pool has room.
fn part69(w: &mut World, pos: [f32; 4], vel: [f32; 4], moby: Option<MobyId>) -> Option<usize> {
    *w.svc.fx.part_spawns.entry(69).or_default() += 1;
    let sys = w.particles.as_deref_mut()?;
    let r = crate::particles::type69::spawn(sys, w.rng, pos, vel, 0x7f, moby.map_or(0, |m| m as u32 + 1));
    if r.is_none() { w.svc.fx.part_failed += 1; }
    r
}
/// A type-69 record's patch (`+0x0c` size, `+0x0a` timer, `+0x30` 1/timer, `+0x36` mode, `+0x38` colour).
fn patch69(w: &mut World, i: usize, size: f32, timer: Option<i32>, mode: Option<i16>) {
    let Some(sys) = w.particles.as_deref_mut() else { return };
    use crate::particles::rec;
    let r = &mut sys.pool.recs[i];
    rec::set_ff(r, 0xc, size);
    if let Some(t) = timer {
        rec::set_i16(r, 0xa, t as i16);
        rec::set_u32(r, 0x38, 0x7f_7f7f);
        rec::set_ff(r, 0x30, 1.0 / t as f32);
    }
    if let Some(m) = mode { rec::set_i16(r, 0x36, m); }
}

// ------------------------------------------------------------------------------------------------------------------
// 1440

/// Level00 `0x2e0b88` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, pv::LEN);
    if w.m(id).state != 0 { pre(w, id); }
    let tm = tgt_moby(w, id);
    crate::moby_update::manip::big_head_scale(w, 2.5, id, pv::HEAD);
    if w.svc.cheats.on(crate::cheats::slot::ENEMIES) { crate::moby_update::manip::look(w, id, id, pv::HEAD, 2, 0.03, 0.3); }
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 27.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x15;
        }
    }
    let tpos = |w: &World| tm.map(|m| c::pos(w, m)).unwrap_or([0.0; 4]);
    let st = w.m(id).state;
    match st {
        0 => {
            let p = c::pos(w, id);
            c::set_pv4(w, id, pv::HOME, p);
            c::set_pu8(w, id, pv::HP + 9, 0);
            c::set_pf(w, id, pv::HP, 2.0);
            c::set_pu8(w, id, pv::HP + 8, 1);
            c::set_pi16(w, id, pv::HP + 4, 2);
            c::set_pu8(w, id, 0x58, 14);
            c::set_pu8(w, id, 0x5a, 8);
            c::set_pi32(w, id, pv::NODE, 1);
            if w.rng.randi(2) != 0 { w.mm(id).mode |= 0x8000; }
            match path(w, id, pv::ARRIVE) {
                None => w.mm(id).state = 3,
                Some(a) => {
                    if c::pi32(w, id, pv::AREA) == -1 {
                        w.delete_moby(id);
                        return;
                    }
                    let q = path_point(w, a, 0);
                    w.mm(id).position = q;
                    w.mm(id).state = 1;
                }
            }
            if seq_b(w, id) != 1 { w.anim_blend(id, 1, 0, 5); }
        }
        1 => {
            if wrapped(w, id) {
                if seq_b(w, id) != 1 {
                    blend(w, id, 1, 0x14);
                } else if w.rng.randi(4) == 0 && seq_b(w, id) != 2 {
                    blend(w, id, 2, 0x14);
                }
            }
            if kind(w, id) != 2 {
                w.mm(id).state = 2;
                if seq_b(w, id) != 9 { blend(w, id, 9, 10); }
            }
        }
        2 => {
            let Some(a) = path(w, id, pv::ARRIVE) else { return tail(w, id) };
            let n = w.svc.splines[a].len() as i32;
            let node = c::pi32(w, id, pv::NODE);
            let q = path_point(w, a, node.max(0) as usize);
            turn_to(w, id, q, FOUR_PI);
            let p = c::pos(w, id);
            let d = sub3(q, p);
            let last = path_point(w, a, (n - 1).max(0) as usize);
            let to_end = c::dist3(p, last);
            let (mut x, mut v) = (0.0, c::pf(w, id, pv::SPEED));
            turn::spring(to_end, DT2 * 8.0, DT2 * 16.0, DT * 12.0, &mut x, &mut v);
            c::set_pf(w, id, pv::SPEED, v);
            let speed = c::pf(w, id, pv::SPEED);
            if len3(d) < speed + speed {
                c::set_pi32(w, id, pv::NODE, node + 1);
                if node + 1 == n {
                    w.mm(id).state = 3;
                    if seq_b(w, id) != 1 { blend(w, id, 1, 10); }
                }
            }
            let step = with_len(d, c::pf(w, id, pv::SPEED));
            let m = w.mm(id);
            m.position = add3(m.position, step);
        }
        3 => {
            let q = tpos(w);
            turn_to(w, id, q, FOUR_PI);
            let hp = super::hero_pos(w);
            if c::dist2(c::pos(w, id), hp) < 20.0 {
                if seq_b(w, id) != 4 { blend(w, id, 4, 0x14); }
            } else if wrapped(w, id) {
                let s = seq_b(w, id);
                if s as i32 != w.rng.randi(2) + 1 {
                    let k = w.rng.randi(2) + 1;
                    blend(w, id, k as u8, 0x14);
                }
            }
            if kind(w, id) != 2 && claim(w, id, 0, 0) {
                w.mm(id).state = 4;
                if seq_b(w, id) != 5 { blend(w, id, 5, 10); }
            }
        }
        4 => {
            let q = tpos(w);
            turn_to(w, id, q, FOUR_PI);
            if wrapped(w, id) {
                c::set_pi16(w, id, pv::LOOPS, 0);
                w.mm(id).state = 5;
                if seq_b(w, id) != 6 { blend(w, id, 6, 10); }
            }
        }
        5 => fire(w, id),
        6 => {
            let q = tpos(w);
            turn_to(w, id, q, FOUR_PI);
            if wrapped(w, id) {
                w.mm(id).state = 3;
                if seq_b(w, id) != 1 { blend(w, id, 1, 10); }
            }
        }
        7 => {
            if knock::update(w, id, pv::K) & 0x140 != 0 {
                w.mm(id).state = 3;
                if seq_b(w, id) != 1 { blend(w, id, 1, 10); }
            }
        }
        8 if knock::update(w, id, pv::K) & 0x140 != 0 => {
                let p = c::pos(w, id);
                fx::death_explosion(w, 0.75, 13.0, Some(id), p, -1);
                crate::moby_update::classes::breakables::burst_pieces(w, id, 0x7ab, 1, 0x7ab, 1, 4, 0);
                let rot = w.m(id).rotation;
                for class in [0x78d, 0x78e, 0x78f] { fx::break_piece(w, id, class, p, rot, 0, 0); }
                w.delete_moby(id);
                return;
        }
        _ => {}
    }
    tail(w, id);
}

/// State 5 (module table): the beam fires.
fn fire(w: &mut World, id: MobyId) {
    let t = c::pv4(w, id, pv::TGT);
    let p = c::pos(w, id);
    let a = c::atan(t[0] - p[0], t[1] - p[1]);
    let a = if w.m(id).mode & 0x8000 != 0 { c::add_rot(a, AIM_OFFSET) } else { c::sub_rot(a, AIM_OFFSET) };
    turn_by(w, id, a, HALF_PI);
    let m = w.joint_matrix(id, 0);
    let src = [m[3][0], m[3][1], m[3][2], m[3][3]];
    let rows = w.m(id).rows;
    let f = with_len([rows[0][0], rows[0][1], 0.0, rows[0][3]], BEAM_LEN);
    let mut end = add3(src, f);
    let yaw = c::yaw(w, id);
    let damaging = {
        let a = w.m(id).anim;
        a.seq_a == a.seq_b && 1.0 < crate::moby_update::creature::ground::key_time(w, id)
    };
    let tmpl = HitTemplate {
        dir: [Pf::f(yaw.cos()), Pf::f(yaw.sin()), Pf::ONE, Pf::b(0x45af_df66)],
        attacker: Some(id),
        flags: 0x1_0000 | damaging as u32,
        b18: 0,
        b19: 1,
        h1a: w.m(id).o_class as u16,
        damage: Pf::ONE,
        w20: 1,
    };
    let f4 = |a: [f32; 4]| [Pf::f(a[0]), Pf::f(a[1]), Pf::f(a[2]), Pf::ZERO];
    let body = [p[0], p[1], p[2] + 0.5, p[3]];
    let hero = w.hero_moby;
    let stop = |w: &mut World| {
        release(w, id);
        w.mm(id).state = 6;
        if seq_b(w, id) != 7 { blend(w, id, 7, 10); }
    };
    if let Some(h) = sv::line_hit_in(w.table, w.svc, w.classes, w.coll, f4(src), f4(body), 0, Some(id), &tmpl) {
        if damaging && h.moby == hero { stop(w); }
    }
    if let Some(h) = sv::line_hit_in(w.table, w.svc, w.classes, w.coll, f4(src), f4(end), 0, Some(id), &tmpl) {
        end = [h.point[0], h.point[1], h.point[2], 1.0];
        if damaging && h.moby == hero { stop(w); }
    }
    aim(w, id, m, end);
    if wrapped(w, id) {
        let n = c::pi16(w, id, pv::LOOPS);
        c::set_pi16(w, id, pv::LOOPS, n.wrapping_add(1));
        if n == LOOPS {
            release(w, id);
            w.mm(id).state = 6;
            if seq_b(w, id) != 0xc { blend(w, id, 0xc, 10); }
        }
    }
}

/// Level00 `0x2e1678` (module table): the tick before the states.
fn pre(w: &mut World, id: MobyId) {
    if c::pi32(w, id, pv::LURE) != 0 {
        c::set_pi32(w, id, pv::LURE, 0);
        let r = w.rng.rand_range(0xb4, 300);
        let t = w.ticks(r);
        c::set_pi16(w, id, pv::ALERT, t as i16);
    }
    c::dec_timer_pvar_s16(w, id, pv::ALERT);
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::HP, 0, 4);
    if res.out5 != 1 && w.m(id).state != 8 && res.damage != 0.0 {
        let kr = pv::K;
        c::set_pu8(w, id, kr + 0x3d, 0);
        c::set_pi32(w, id, kr + knock::k::RADIUS, 0x200);
        c::set_pf(w, id, kr + knock::k::GRAVITY, GRAVITY * DT2);
        c::set_pf(w, id, kr + knock::k::DRAG, DRAG * DT2);
        c::set_pf(w, id, kr + knock::k::SPEED, OUT * DT);
        c::set_pf(w, id, kr + knock::k::UP, UP * DT);
        c::set_pi32(w, id, kr + knock::k::FLAGS, 9);
        c::set_pf(w, id, kr + knock::k::AIR_SPEED, DT + DT);
        let hp = c::pf(w, id, pv::HP) - res.damage;
        c::set_pf(w, id, pv::HP, hp);
        c::set_pf(w, id, kr + knock::k::ZOFF, 0.5);
        let dir = res.hit.map(|h| h.dir.map(|x| f32::from_bits(x.0))).unwrap_or([0.0; 4]);
        if hp <= 0.0 {
            c::set_pi32(w, id, kr + knock::k::FLAGS, 0x29);
            w.mm(id).mode &= !mode::TARGETABLE;
            let (mut sp, mut up) = (c::pf(w, id, kr + knock::k::SPEED), c::pf(w, id, kr + knock::k::UP));
            let a = knock::aim(dir, &mut sp, &mut up);
            c::set_pf(w, id, kr + knock::k::SPEED, sp);
            c::set_pf(w, id, kr + knock::k::UP, up);
            knock::start(w, id, kr, a, 0xb, 1, 0);
            c::set_pf(w, id, kr + knock::k::KEY_APEX, 7.0);
            c::set_pf(w, id, kr + knock::k::KEY_LAND, 16.0);
            let m = w.mm(id);
            m.state = 8;
            m.has_collision = false;
            c::set_pu8(w, id, pv::FLASH + 7, 0x78);
            flash::start(w, id, pv::FLASH);
            set_death_bits(w, id, 0, -1);
        } else {
            c::set_pf(w, id, kr + knock::k::SPEED, OUT * DT * 0.35);
            c::set_pf(w, id, kr + knock::k::UP, UP * DT * 0.5);
            let (mut sp, mut up) = (c::pf(w, id, kr + knock::k::SPEED), c::pf(w, id, kr + knock::k::UP));
            knock::aim(dir, &mut sp, &mut up);
            c::set_pf(w, id, kr + knock::k::SPEED, sp);
            c::set_pf(w, id, kr + knock::k::UP, up);
            let a = c::atan(dir[0], dir[1]);
            knock::start(w, id, kr, a, 8, 1, 0);
            c::set_pf(w, id, kr + knock::k::KEY_APEX, -1.0);
            c::set_pf(w, id, kr + knock::k::KEY_LAND, -1.0);
            w.mm(id).state = 7;
            c::set_pu8(w, id, pv::FLASH + 7, 0xfa);
            let t = w.ticks(0x3c);
            c::set_pi16(w, id, pv::HP + 6, t as i16);
            flash::start(w, id, pv::FLASH);
        }
        release(w, id);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::FLASH);
    match w.m(id).state {
        1 => {
            let region = path(w, id, pv::AREA);
            let t = target::acquire_in(w, id, 64.0, region);
            store_target(w, id, &t);
        }
        5 => {}
        _ => {
            let range = if c::pi16(w, id, pv::ALERT) != 0 { 37.0 } else { RANGE };
            let t = target::acquire(w, id, range);
            store_target(w, id, &t);
            if t.kind != 2 && range < c::dist2(c::pos(w, id), t.pos) { c::set_pi32(w, id, pv::TGT_KIND, 2); }
        }
    }
    if c::pi32(w, id, pv::TGT_MOBY) == 0 {
        let hm = w.hero_moby;
        c::set_pi32(w, id, pv::TGT_MOBY, hm.map_or(0, |m| m as i32 + 1));
        let hp = super::hero_pos(w);
        c::set_pv4(w, id, pv::TGT, hp);
    }
    let alt = c::pi32(w, id, pv::ALT);
    if -1 < alt {
        match usize::try_from(alt).ok().filter(|&m| m < w.table.mobys.len() && !matches!(w.m(m).state, 0xfe | 0xfd)) {
            None => c::set_pi32(w, id, pv::ALT, -1),
            Some(m) => {
                let cur = tgt_moby(w, id).map_or([0.0; 4], |t| c::pos(w, t));
                let p = c::pos(w, id);
                if c::dist2(p, c::pos(w, m)) < c::dist2(p, cur) {
                    c::set_pi32(w, id, pv::TGT_MOBY, m as i32 + 1);
                    let q = c::pos(w, m);
                    c::set_pv4(w, id, pv::TGT, q);
                    c::set_pi32(w, id, pv::TGT_KIND, 1);
                }
            }
        }
    }
}

fn store_target(w: &mut World, id: MobyId, t: &target::Target) {
    c::set_pv4(w, id, pv::TGT, t.pos);
    c::set_pv4(w, id, pv::TGT + 0x10, t.rot);
    c::set_pv4(w, id, pv::TGT + 0x20, t.aim);
    c::set_pv4(w, id, pv::TGT + 0x30, t.body);
    c::set_pi32(w, id, pv::TGT_MOBY, t.moby.map_or(0, |m| m as i32 + 1));
    c::set_pi32(w, id, pv::TGT_KIND, t.kind as i32);
}

/// Level00 `0x2e1aa8` (module table): the eye's twinkles and its glow callback.
fn tail(w: &mut World, id: MobyId) {
    let mut eye = w.joint_point(id, 1);
    eye[2] -= 0.333;
    c::set_pv4(w, id, pv::EYE, eye);
    let v = rand_polar(w, f32::from_bits(0x3ba3_d70a), f32::from_bits(0x3cf5_c28f));
    if let Some(i) = part69(w, eye, v, Some(id)) {
        let s = w.rng.randf(6000.0, 32000.0);
        patch69(w, i, s, None, None);
    }
    for k in 0..3 {
        if let Some(i) = part69(w, eye, [0.0; 4], Some(id)) {
            let s = if k == 2 && w.rng.randi(8) == 0 { 180000.0 } else { w.rng.randf(20000.0, 120000.0) };
            let t = w.ticks(2);
            patch69(w, i, s, Some(t), Some(3));
        }
    }
    if let Some(i) = super::row(REFERENCE_LEVEL, EYE_FN) { w.svc.draw_callbacks.register2(Callback::UnitGlow(i), id); }
}

/// Level00 `0x2e1c78`: the eye's glow quad (size 0.2, FX 0xb, additive, white at 0x80) at +0x1c0.
pub fn glow_quads(table: &crate::moby_runtime::MobyTable, _svc: &crate::moby_update::Services, id: MobyId) -> Vec<GlowQuad> {
    let Some(m) = table.mobys.get(id) else { return Vec::new() };
    if m.pvars.len() < pv::LEN { return Vec::new(); }
    let e = sv::pvar::v4f(&m.pvars, pv::EYE);
    vec![GlowQuad { size: 0.2, pull: 0.0, point: [e[0], e[1], e[2]], rgba: 0x80ff_ffff }]
}

// ------------------------------------------------------------------------------------------------------------------
// The beam slots

/// Level00 `0x2e1f28(owner, lights, follow)`: the owner's slot, else the first free one. False when all are taken.
fn claim(w: &mut World, owner: MobyId, lights: u32, follow: i32) -> bool {
    let b = beams(w);
    if b.slots.iter().any(|s| s.owner == Some(owner)) { return true; }
    let Some(s) = b.slots.iter_mut().find(|s| s.owner.is_none()) else { return false };
    s.owner = Some(owner);
    s.lights = [-1, -1];
    s.light_flags = lights;
    s.follow = follow;
    s.state = 1;
    s.phase = [0.0; 3];
    s.scroll = 0.0;
    true
}

/// Level00 `0x2e2040(owner)`: its slot freed, with its point lights.
fn release(w: &mut World, owner: MobyId) {
    if let Some(k) = beams(w).slots.iter().position(|s| s.owner == Some(owner)) { release_slot(w, k); }
}

/// Level00 `0x2e20f8(k)`.
fn release_slot(w: &mut World, k: usize) {
    let s = &mut w.svc.units.veldin_beams.slots[k];
    s.state = 0;
    s.owner = None;
    let lights = std::mem::replace(&mut s.lights, [-1, -1]);
    for l in lights {
        if let Ok(i) = usize::try_from(l) { w.svc.point_lights.free(i); }
    }
}

/// Level00 `0x2e2190(owner, M, end)`: the slot aimed this tick (1 → 2, 4 → 3).
fn aim(w: &mut World, owner: MobyId, m: [[f32; 4]; 4], end: [f32; 4]) {
    let Some(s) = beams(w).slots.iter_mut().find(|s| s.owner == Some(owner)) else { return };
    if s.state == 1 { s.state = 2; } else if s.state == 4 { s.state = 3; }
    s.matrix = m;
    s.end = end;
}

// ------------------------------------------------------------------------------------------------------------------
// 1471

/// Level00 `0x2e1df0` (module table).
pub fn manager(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            for s in beams(w).slots.iter_mut() {
                s.state = 0;
                s.owner = None;
            }
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.state = 1;
        }
        1 => {
            let mut n = 0;
            for k in 0..3 {
                match beams(w).slots[k].state {
                    1 | 0 => {}
                    4 => release_slot(w, k),
                    _ => {
                        n += 1;
                        effects(w, k);
                        beams(w).slots[k].state = 4;
                    }
                }
            }
            if n != 0 {
                if let Some(i) = super::row(REFERENCE_LEVEL, BEAM_FN) {
                    w.svc.draw_callbacks.register2(Callback::UnitFrame(i), id);
                    w.svc.draw_callbacks.register2(Callback::UnitQuads(i), id);
                }
            }
        }
        _ => {}
    }
}

/// Level00 `0x2e2250(owner, k)` (module table): the strands, the sparks and the lights of slot `k`.
fn effects(w: &mut World, k: usize) {
    let (origin, end, m) = {
        let s = &beams(w).slots[k];
        (s.matrix[3], s.end, s.matrix)
    };
    let step = len3(sub3(end, origin)) / 20.0;
    {
        let s = &mut beams(w).slots[k];
        if s.state == 2 {
            s.state = 3;
            for i in 0..20 {
                let x = step * i as f32;
                s.strands[0][i] = [x, 0.0, 0.0, 1.0];
                s.strands[1][i] = [x, 0.0, 0.0, 1.0];
            }
        }
        s.phase[0] = c::add_rot(s.phase[0], f32::from_bits(0x3e32_b8c2));
        s.phase[1] = c::sub_rot(s.phase[1], f32::from_bits(0x3f4d_87ac));
        s.phase[2] = c::add_rot(s.phase[2], f32::from_bits(0x3d56_7770));
        let d = (360.0 / (20.0 * 0.5)) * f32::from_bits(0x3c8e_fa35);
        for i in 0..19 {
            let fi = i as f32;
            let a = crate::moby_update::classes::flyer::wrap_frac(s.phase[0] + fi * d);
            let z = ((i as f32 / 20.0) * 0.3 + 0.1) * a.sin();
            s.strands[0][i] = [step * fi, 0.0, z, 1.0];
        }
    }
    let fr = beam_frame(w, origin, sub3(end, origin));
    let tip = xform(&fr, beams(w).slots[k].strands[0][18]);
    beams(w).slots[k].tip = tip;
    for i in 0..19 {
        let fi = i as f32;
        let (pb, pc) = { let s = &beams(w).slots[k]; (s.phase[1], s.phase[2]) };
        let b = crate::moby_update::classes::flyer::wrap_frac(pb + fi * f32::from_bits(0x3f7e_adae));
        let cc = crate::moby_update::classes::flyer::wrap_frac(pc + fi * f32::from_bits(0x3e56_7770));
        let zc = cc.sin() * 0.5;
        let sb = b.sin();
        let r = w.rng.randf_sym(0.0, 0.2);
        let z = zc * sb + r;
        let y = b.cos() * 0.3;
        beams(w).slots[k].strands[1][i] = [step * fi, y, z, 1.0];
    }
    let (owner, follow, flags) = { let s = &beams(w).slots[k]; (s.owner, s.follow, s.light_flags) };
    // The source's sparks.
    let l = w.rng.randf(0.01, 0.05);
    let a = w.rng.randf_sym(f32::from_bits(0x3eb2_b8c2), f32::from_bits(0x3f32_b8c2));
    let v = rotate(&m, [a.cos() * l, a.sin() * l, 0.0, 1.0]);
    part69(w, origin, v, owner);
    if w.rng.randi(3) == 0 {
        let p = add3(scale3(m[0], 0.3), origin);
        if let Some(i) = part69(w, p, [0.0; 4], owner) {
            let t = w.ticks(4);
            patch69(w, i, 120000.0, Some(t), (follow == 1).then_some(1));
        }
    }
    // The tip's sparks (a `randf` drawn and dropped first).
    let _ = w.rng.randf(0.01, 0.05);
    let v = rand_polar(w, 0.01, 0.05);
    part69(w, tip, v, owner);
    if w.rng.randi(3) == 0 {
        if let Some(i) = part69(w, tip, [0.0; 4], owner) {
            let t = w.ticks(4);
            patch69(w, i, 120000.0, Some(t), (follow == 1).then_some(2));
        }
    }
    // The point lights (radius 5, colour (1, 1, 2), intensity 0): the source 0.25 down, the tip 0.5 up.
    if flags & 1 != 0 { light(w, k, 0, [origin[0], origin[1], origin[2] - 0.25]); }
    if flags & 2 != 0 { light(w, k, 1, [tip[0], tip[1], tip[2] + 0.5]); }
}

fn light(w: &mut World, k: usize, which: usize, pos: [f32; 3]) {
    use crate::point_lights::PointLight;
    let l = PointLight { color: [1.0, 1.0, 2.0], intensity: 0.0, pos, radius: LIGHT_RADIUS };
    if beams(w).slots[k].lights[which] == -1 {
        let load = f32::from_bits(w.svc.frame_load[1].0);
        let i = w.svc.point_lights.alloc(l, load).map_or(-1, |i| i as i32);
        beams(w).slots[k].lights[which] = i;
    }
    if let Ok(i) = usize::try_from(beams(w).slots[k].lights[which]) {
        let mut cur = w.svc.point_lights.slots[i].unwrap_or(l);
        cur.pos = pos;
        cur.radius = LIGHT_RADIUS;
        w.svc.point_lights.set(i, cur);
    }
}

// ------------------------------------------------------------------------------------------------------------------
// The beams' draw (`0x2e2af0`)

/// The state part of `0x2e2af0` (run by the frame's callbacks): the scroll, the glows' draws, the quads.
pub fn frame(w: &mut World, id: MobyId) {
    let (fx_a, fx_b) = {
        let p = &w.m(id).pvars;
        if p.len() < 8 { return; }
        (sv::pvar::i32(p, 4) + 0x28, sv::pvar::i32(p, 0) + 0x28)
    };
    // 0x15ed60, the time base's speed (1.0 NTSC, 1.2 PAL: `crate::particles::TimeBase`).
    let speed = w.particles.as_deref().map_or(1.0, |p| f32::from_bits(p.time.speed));
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    let g = gravity(w);
    let mut groups: Vec<FxQuads> = Vec::new();
    for k in 0..3 {
        let (owner, state) = { let s = &beams(w).slots[k]; (s.owner, s.state) };
        let Some(owner) = owner else { continue };
        if state < 2 { continue; }
        {
            let s = &mut beams(w).slots[k];
            s.scroll -= speed * 0.2;
            if s.scroll <= -8.0 { s.scroll += 8.0; }
        }
        groups.extend(glows(w, k, cam, g));
        let thin = w.table.mobys.get(owner).is_some_and(|m| m.o_class == 0x1c);
        let s = beams(w).slots[k].clone();
        let fr = frame_at(&s, g);
        groups.extend(ribbon(&s, &fr, &s.strands[0], cam, fx_a, fx_b, 0x80, 0x40, thin));
        groups.extend(ribbon(&s, &fr, &s.strands[1], cam, fx_a, fx_b, 0x30, 0x10, thin));
    }
    beams(w).draw = groups;
}

/// The quads [`frame`] left (`Callback::UnitQuads` of the beam row).
pub fn fx_quad_groups(svc: &crate::moby_update::Services) -> Vec<FxQuads> { svc.units.veldin_beams.draw.clone() }

/// The beam's frame from the slot (`0x2e2c88`'s: the matrix's row 3 to the end).
fn frame_at(s: &Slot, g: [f32; 4]) -> [[f32; 4]; 4] {
    let o = s.matrix[3];
    let f = with_len(sub3(s.end, o), 1.0);
    let r = with_len(cross(f, g), -1.0);
    let u = cross(r, f);
    [f, r, u, [o[0], o[1], o[2], 1.0]]
}

/// A camera-facing quad of the table 0x1e6ce0 ((0, ∓1, ±1)) of size `size` at `p` (+ `jitter`).
fn glow_quad(p: [f32; 4], cam: [f32; 4], g: [f32; 4], size: f32, jitter: [f32; 4], rgba: u32) -> FxQuad {
    let f = with_len(sub3(cam, p), 1.0);
    let r = with_len(cross(f, g), -1.0);
    let u = cross(r, f);
    let corner = |y: f32, z: f32| -> [f32; 3] {
        [p[0] + (r[0] * y + u[0] * z) * size + jitter[0], p[1] + (r[1] * y + u[1] * z) * size + jitter[1], p[2] + (r[2] * y + u[2] * z) * size + jitter[2]]
    };
    FxQuad { corners: [corner(-1.0, 1.0), corner(-1.0, -1.0), corner(1.0, 1.0), corner(1.0, -1.0)], st: [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]], rgba: [rgba; 4] }
}

/// Level00 `0x2e3388(owner, k)`: the source's glow (white, `randf(0.3, 0.325)`, jittered `randf_sym(0, 0.05)` × 3)
/// with a red halo three times its size, then the tip's (`randf(0.2, 0.225)`, the same jitter).
fn glows(w: &mut World, k: usize, cam: [f32; 4], g: [f32; 4]) -> Vec<FxQuads> {
    let (src, tip) = { let s = &beams(w).slots[k]; (s.matrix[3], s.tip) };
    let size = w.rng.randf(0.3, f32::from_bits(0x3ea6_6666));
    let j = [w.rng.randf_sym(0.0, 0.05), w.rng.randf_sym(0.0, 0.05), w.rng.randf_sym(0.0, 0.05), 0.0];
    let a = glow_quad(src, cam, g, size, j, 0xffff_ffff);
    let b = glow_quad(src, cam, g, size * 3.0, [0.0; 4], 0x607f_4040);
    let size2 = w.rng.randf(0.2, f32::from_bits(0x3e66_6666));
    let c2 = glow_quad(tip, cam, g, size2, j, 0xffff_ffff);
    let d = glow_quad(tip, cam, g, size2 * 3.0, [0.0; 4], 0x607f_4040);
    vec![
        FxQuads { fx: 0xb, additive: false, subtract: false, quads: vec![a] },
        FxQuads { fx: 0xb, additive: true, subtract: false, quads: vec![b] },
        FxQuads { fx: 0xb, additive: false, subtract: false, quads: vec![c2] },
        FxQuads { fx: 0xb, additive: true, subtract: false, quads: vec![d] },
    ]
}

/// `FUN_0026cc00(a, b, c, d, t)`: the cubic through the four values.
fn cubic(a: f32, b: f32, cc: f32, d: f32, t: f32) -> f32 {
    let e = (d - cc) - (a - b);
    e * t * t * t + ((a - b) - e) * t * t + (cc - a) * t + b
}

/// Level00 `0x2e2c88(fxA, fxB, strand, alphaA, alphaB, k)`: the strand's two camera-facing ribbons over its points
/// 0..19 (18 quads each): A `fxA`, ±w wide (0.6, 0.3 for an owner of class 0x1c, tapering to 0.05 along
/// `FUN_0026cc00(−1, 0, 1, 0, i/20)`), ST (s, 0)..(s + 1, 1) with the slot's scroll s, colour 0x7f7f7f at `alphaA`;
/// B `fxB`, ±2w, ST (1, 0)..(0, 1), 0x7f4040 at `alphaB`; both additive, the first quad's near corners at alpha 0.
#[allow(clippy::too_many_arguments)]
fn ribbon(s: &Slot, fr: &[[f32; 4]; 4], strand: &[[f32; 4]; 20], cam: [f32; 4], fx_a: i32, fx_b: i32, alpha_a: u32, alpha_b: u32, thin: bool) -> Vec<FxQuads> {
    let base = if thin { 0.3 } else { f32::from_bits(0x3f19_999a) };
    let ca = alpha_a << 24 | 0x7f_7f7f;
    let cb = alpha_b << 24 | 0x7f_4040;
    // The edge at point i: (p + N·w, p − N·w, p + N·2w, p − N·2w), N = unit(T × (cam − p)), T along to point i + 1.
    let edge = |i: usize, w: f32| -> [[f32; 3]; 4] {
        let p = xform(fr, strand[i]);
        let q = xform(fr, strand[i + 1]);
        let t = with_len(sub3(q, p), 1.0);
        let v = sub3(cam, p);
        let n = with_len(cross(v, t), 1.0);
        let at = |k: f32| [p[0] + n[0] * k, p[1] + n[1] * k, p[2] + n[2] * k];
        [at(w), at(-w), at(w + w), at(-(w + w))]
    };
    let st_a = |sc: f32| [[sc, 0.0], [sc, 1.0], [sc + 1.0, 0.0], [sc + 1.0, 1.0]];
    let st_b = [[1.0, 0.0], [1.0, 1.0], [0.0, 0.0], [0.0, 1.0]];
    let mut qa = Vec::new();
    let mut qb = Vec::new();
    let mut w0 = base;
    let mut prev = edge(0, w0);
    for i in 1..19 {
        let cur = edge(i, w0);
        let (mut ra, mut rb) = ([ca; 4], [cb; 4]);
        if i == 1 {
            ra[0] = 0x00ff_ffff;
            ra[1] = 0x00ff_ffff;
            rb[0] = 0x007f_4040;
            rb[1] = 0x007f_4040;
        }
        qa.push(FxQuad { corners: [prev[0], prev[1], cur[0], cur[1]], st: st_a(s.scroll), rgba: ra });
        qb.push(FxQuad { corners: [prev[2], prev[3], cur[2], cur[3]], st: st_b, rgba: rb });
        let c3 = cubic(-1.0, 0.0, 1.0, 0.0, i as f32 / 20.0);
        w0 = base + (0.05 - base) * c3;
        prev = cur;
        // The next segment's near edge is this point's edge at the new width (the game keeps the transformed far
        // corners, which were made with the old width: `prev` stays as drawn).
    }
    vec![
        FxQuads { fx: fx_a.max(0) as usize, additive: true, subtract: false, quads: qa },
        FxQuads { fx: fx_b.max(0) as usize, additive: true, subtract: false, quads: qb },
    ]
}
