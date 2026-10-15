//! **The Glove of Doom's bots** (class 186 = 0xba; one update on every level: level01 `0x2d5c10`), made by the
//! canister 230 (`0x2d5a70`, [`super::doom_canister`]), with their helpers: the target search `0x2d49f8`, the hop
//! direction solver `0x2d4cb8`, the jump planner `0x2d5658` and solver `0x2d5518`, the jump `0x270340`, the ground
//! probe `0x2d48e8`, the landing check `0x2d5b30`, the sphere `0x2d4718` and the save / load of their globals
//! (`0x2d47f8` / `0x2d4830` / `0x2d4868` / `0x2d48b0`). docs/plan/hero_gameplay.md §18.
//!
//! **What a bot does.** It drops out of the canister (state 0xe: gravity, pushed apart from the other bots), then
//! walks (the shared creature walker: `walker::walk_to` = `0x26de80`, `walker::step`, `walker::move_collide`) toward
//! a heading it picks with the hop solver (four probe lines over a 106° fan, then two whiskers that nudge it off
//! walls). One tick in four (`randi(4)`) it searches the **target list** 0x1abe80 for a creature (class type 5)
//! within 40 (score: the distance to its aim point 0.4 up + the squared yaw and pitch differences from the bot's
//! facing, +15 when it stands 3.25 above the bot, +5 when the line to it is blocked — within 30 a line test (flags 6)
//! unless more than 8 apart in height, farther the "drawn last frame" byte); none found and Ratchet more than 3.2 away
//! (xy) → it follows Ratchet. Chasing, one tick in four it plans a **jump** toward the target when the direct line is
//! blocked (`0x2d5658`: at most 5.2 × its size ahead, onto flat enough ground no more than 3 × its size up, with a clear
//! arc through the apex; halving the leap until 0.5), then turns to it (6) and jumps (7: wind-up sequence 2, air 3,
//! landing 4, gravity 29.7·dt², at most 0.1 a tick across and 7·dt up). Walls it cannot jump are backed off (state 8,
//! the target remembered as failed and 500 added to its patience counter). **It explodes** (state 0xb) when a
//! creature's primitive touches its sphere, when within the target's reach (1 + record radius / 8) its sphere
//! (flags 1) touches anything, when it stands on a creature or on water / lava (the landing check), when hit
//! (mask 0x800001: explosions and enemy hits), after 60 s (3600 ticks) or when its patience counter passes 4000:
//! `SpawnBeamExplosion` (flashes 2 / 1 beyond 9, light 15, 5 streaks, 2 spark pairs, 4 puffs, the shake, 5 debris),
//! then every moby its sphere (radius 1, 0.5 up, flags 0x15) lists gets a hit (damage 3, pushed 1 / 1 up from it,
//! flags 0x10000, type 2 / 3) and its class sound 0 plays; deleted.
//!
//! **States** (+0x20): 0 init → 0xe; 0xe dropping; 2 / 3 / 4 / 5 walking (4 near the target or Ratchet within 20,
//! run sequence 7; 5 far, sequence 0 / Ratchet's sequence 6); 6 turning to a jump; 7 jumping; 8 knocked back / backing
//! off; 0xb exploding. 1 / 9 / 10 (the sequence's wrap → 2), 0xc (the wrap → 4: never set, the game compares the state
//! it just wrote) and 0xd (never set) are ported as written.
//!
//! **Pvars** (the game's +0x00..+0x6c): +0x00 the walk / jump target point, +0x10 velocity (the walker's move out),
//! +0x20 the last position, +0x30 the patience counter, +0x34 age, +0x38 the target (the port: moby + 1), +0x3c the
//! last failed target, +0x40 / +0x44 / +0x48 / +0x4c the walker's speed, turn speed, vertical speed and step timer,
//! +0x50 / +0x54 / +0x58 the jump's vertical speed, speed and heading, +0x5c top speed, +0x60 the owner, +0x64 the
//! heading, +0x68 the jump's state. **The globals** 0x141210 (a walker record J) and 0x141260 (a jump record R) are
//! shared by every bot in the game, but every bot writes their constants each tick (or in its state 0) and loads its
//! own fields into them before each use (`0x2d47f8` / `0x2d4868`) and back after (`0x2d4830` / `0x2d48b0`), so the
//! port keeps one copy per bot, at [`pv::J`] / [`pv::R`] in its own pvar block (the port's layout) — the same
//! values at every read.
//!
//! **Quirks kept** [M]: a bot without a target treats a line that hits the world as clear (the game compares the hit
//! moby with the empty target: the hop solver returns the first direction at once, the walk's wall probe and the jump
//! arc pass); the proximity jump planned before the search is overwritten by the search's state 4 the same tick.
//!
//! Native `f32`; the rand draws are the game's. **Inferred [L]**: J +0x2c (the "arrived" distance), +0x34, +0x38
//! (the walker flags) and +0x4c are BSS the bots never write (0); the gold bots (0x13e534: size, reach, the anim
//! speed, class sound 4, the blast's colour shift) read the mirrored gold table (`Weapons::gold`); moving platforms (`0x2752c0`) are n/a (0x13f64c is never set);
//! the debug prints and lines (`STUB_printf`, `0x2d5478`, `0x26e3e8` only feeds them and the arc test) have no effect;
//! `+0xbc = 0xd` on a planned jump is written and read by nothing.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::{self as c, attack, fx, ground, knock, turn, walker, DT, DT2, SPEED, V};
use crate::moby_update::services::{pv as pvq, pvar as p, sphere_mobys_in, World};
use crate::ps2v::Pf;
use std::f32::consts::{FRAC_PI_2, PI};

pub const UPDATE_FN: u32 = 0x2d5c10;
pub const CLASS: i16 = 0xba;
pub const CLASSES: [i16; 1] = [CLASS];

/// Pvar offsets (module doc).
pub mod pv {
    pub const POINT: usize = 0x00;
    pub const VEL: usize = 0x10;
    pub const PREV: usize = 0x20;
    pub const PATIENCE: usize = 0x30;
    pub const AGE: usize = 0x34;
    pub const TARGET: usize = 0x38;
    pub const FAILED: usize = 0x3c;
    pub const SPEED: usize = 0x40;
    pub const TURN_V: usize = 0x44;
    pub const VZ: usize = 0x48;
    pub const STEP_T: usize = 0x4c;
    pub const JUMP_VZ: usize = 0x50;
    pub const JUMP_SPEED: usize = 0x54;
    pub const JUMP_HEADING: usize = 0x58;
    pub const TOP: usize = 0x5c;
    pub const OWNER: usize = 0x60;
    pub const HEADING: usize = 0x64;
    pub const JUMP_STATE: usize = 0x68;
    /// The walker globals 0x141210 (0x50 bytes), per bot.
    pub const J: usize = 0x70;
    /// The jump record 0x141260 (0x40 bytes), per bot.
    pub const R: usize = 0xc0;
    pub const SIZE: usize = 0x100;
}

/// The jump record's fields (R + offset).
mod r {
    pub const TARGET: usize = 0x00;
    pub const VZ: usize = 0x10;
    pub const SPEED: usize = 0x14;
    pub const HEADING: usize = 0x18;
    pub const GRAVITY: usize = 0x1c;
    pub const MAX_SPEED: usize = 0x20;
    pub const KEY: usize = 0x24;
    pub const AIR: usize = 0x28;
    pub const SEQ_WIND: usize = 0x2c;
    pub const SEQ_AIR: usize = 0x2d;
    pub const SEQ_LAND: usize = 0x2e;
    pub const STATE: usize = 0x2f;
    pub const TURN_V: usize = 0x30;
    pub const TURN: usize = 0x34;
}

/// States (+0x20).
pub mod st {
    pub const INIT: u8 = 0;
    pub const WALK_LO: u8 = 2;
    pub const NEAR: u8 = 4;
    pub const FAR: u8 = 5;
    pub const TURN: u8 = 6;
    pub const JUMP: u8 = 7;
    pub const BACK: u8 = 8;
    pub const EXPLODE: u8 = 0xb;
    pub const LAND_ANIM: u8 = 0xc;
    pub const PARKED: u8 = 0xd;
    pub const DROP: u8 = 0xe;
}

/// The explosion (module doc): `SpawnBeamExplosion(0, 0, 2, 1, 9, 1, 15, bot, (0, 0, 1, 1), pos, 5, 2, 4, −1, 1, 5,
/// −1, gold)`.
pub const BLAST: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 5, sparks: 2, puffs: 4, debris: 5, sound: -1, shake: true };

/// The gold Glove of Doom 0x13e534 (item 20, the byte as a number): `g` = 0x13e534 + 1 (size, speed, reach) and `gh` =
/// 0x13e534·0.5 + 1 (the queries' heights and the hop), as `0x2d5c10` / `0x2d4718` read it each time.
fn gold(w: &World) -> f32 { w.hero.weapons.gold.get(GLOVE as usize).copied().unwrap_or(0) as f32 }
fn g(w: &World) -> f32 { gold(w) + 1.0 }
fn gh(w: &World) -> f32 { gold(w) * 0.5 + 1.0 }
/// The Glove of Doom's item id.
const GLOVE: u8 = 20;

fn in_box(q: V) -> bool { (0..3).all(|k| (2.0..=1021.0).contains(&q[k])) }
fn pv4(v: V) -> [Pf; 4] { v.map(Pf::f) }
fn class_type(w: &World, m: MobyId) -> u8 { w.classes.info(w.m(m).o_class).map_or(0, |i| i.ty) }
/// A creature: a moby of class type 5.
fn creature(w: &World, m: Option<MobyId>) -> bool { m.is_some_and(|m| class_type(w, m) == 5) }
fn dead(w: &World, m: MobyId) -> bool { matches!(w.m(m).state, 0xfd | 0xfe) }

fn target(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, pv::TARGET) - 1).ok() }
fn set_target(w: &mut World, id: MobyId, t: Option<MobyId>) { c::set_pi32(w, id, pv::TARGET, t.map_or(0, |t| t as i32 + 1)); }
fn jf(w: &World, id: MobyId, word: usize) -> f32 { c::pf(w, id, pv::J + 4 * word) }
fn set_jf(w: &mut World, id: MobyId, word: usize, x: f32) { c::set_pf(w, id, pv::J + 4 * word, x) }
fn rf(w: &World, id: MobyId, o: usize) -> f32 { c::pf(w, id, pv::R + o) }
fn set_rf(w: &mut World, id: MobyId, o: usize, x: f32) { c::set_pf(w, id, pv::R + o, x) }
fn rb(w: &World, id: MobyId, o: usize) -> u8 { c::pu8(w, id, pv::R + o) }
/// The walker record's collision radius (J0 / 1024).
fn radius(w: &World, id: MobyId) -> f32 { c::pi32(w, id, pv::J) as f32 * (1.0 / 1024.0) }

/// `0x2d5a70(owner, pos, vel)`: a bot (`CreateMoby(0xba)`): update / draw distances 0xff, visible, state 0, +0xbc 0,
/// at `at` with velocity `vel`, the owner, Ratchet's lighting, a tenth of the class scale.
pub fn create(w: &mut World, owner: i32, at: V, vel: V) -> Option<MobyId> {
    let b = w.create_moby(CLASS)?;
    let hero_light = w.hero_moby.map(|h| (w.m(h).light, w.m(h).ambient));
    let cs = w.class_scale(CLASS).to_f32();
    let m = w.mm(b);
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
    m.visible = 1;
    m.state = st::INIT;
    m.cmd = 0;
    m.position = [at[0], at[1], at[2], at[3]];
    if m.pvars.len() < pv::SIZE { m.pvars.resize(pv::SIZE, 0); }
    p::set_v4f(&mut m.pvars, pv::VEL, vel);
    p::set_i32(&mut m.pvars, pv::OWNER, owner);
    if let Some((l, a)) = hero_light {
        m.light = l;
        m.ambient = a;
    }
    m.scale = cs * 0.1;
    Some(b)
}

// ------------------------------------------------------------------------------------------------
// The globals' load / save (per bot: module doc)

/// `0x2d47f8`: J4, J5, J6, J18, J9 ← +0x40, +0x44, +0x48, +0x4c, +0x5c.
fn load_j(w: &mut World, id: MobyId) {
    let pvs = &mut w.mm(id).pvars;
    for (j, o) in [(0x10, pv::SPEED), (0x14, pv::TURN_V), (0x18, pv::VZ), (0x48, pv::STEP_T), (0x24, pv::TOP)] {
        let x = p::u32(pvs, o);
        p::set_u32(pvs, pv::J + j, x);
    }
}

/// `0x2d4830`: the reverse of [`load_j`].
fn save_j(w: &mut World, id: MobyId) {
    let pvs = &mut w.mm(id).pvars;
    for (j, o) in [(0x10, pv::SPEED), (0x14, pv::TURN_V), (0x18, pv::VZ), (0x48, pv::STEP_T), (0x24, pv::TOP)] {
        let x = p::u32(pvs, pv::J + j);
        p::set_u32(pvs, o, x);
    }
}

/// `0x2d4868`: R +0x10 / +0x14 / +0x18 / +0x2f / +0x30 ← +0x50 / +0x54 / +0x58 / +0x68 / +0x44, the target ← +0x00.
fn load_r(w: &mut World, id: MobyId) {
    let pvs = &mut w.mm(id).pvars;
    for (o, q) in [(r::VZ, pv::JUMP_VZ), (r::SPEED, pv::JUMP_SPEED), (r::HEADING, pv::JUMP_HEADING), (r::TURN_V, pv::TURN_V)] {
        let x = p::u32(pvs, q);
        p::set_u32(pvs, pv::R + o, x);
    }
    pvs[pv::R + r::STATE] = pvs[pv::JUMP_STATE];
    for k in 0..4 {
        let x = p::u32(pvs, pv::POINT + 4 * k);
        p::set_u32(pvs, pv::R + r::TARGET + 4 * k, x);
    }
}

/// `0x2d48b0`: +0x50 / +0x54 / +0x58 / +0x68 / +0x44 ← R +0x10 / +0x14 / +0x18 / +0x2f / +0x30.
fn save_r(w: &mut World, id: MobyId) {
    let pvs = &mut w.mm(id).pvars;
    for (o, q) in [(r::VZ, pv::JUMP_VZ), (r::SPEED, pv::JUMP_SPEED), (r::HEADING, pv::JUMP_HEADING), (r::TURN_V, pv::TURN_V)] {
        let x = p::u32(pvs, pv::R + o);
        p::set_u32(pvs, q, x);
    }
    pvs[pv::JUMP_STATE] = pvs[pv::R + r::STATE];
}

/// State 0: the globals' constants, the bot's own fields, → 0xe (the drop) with the fall sequence 5.
fn init(w: &mut World, id: MobyId) {
    let f = SPEED;
    {
        let pvs = &mut w.mm(id).pvars;
        for b in &mut pvs[pv::R..pv::R + 0x40] { *b = 0; }
    }
    let j7 = g(w) * 0.01 * f;
    set_jf(w, id, 2, g(w) * 0.25);
    set_jf(w, id, 3, g(w) * 0.25);
    set_jf(w, id, 16, f * 0.01);
    set_jf(w, id, 7, j7);
    set_jf(w, id, 15, f * 0.02);
    set_jf(w, id, 17, f * 0.045);
    set_rf(w, id, r::GRAVITY, DT2 * 29.7);
    c::set_pf(w, id, pv::TURN_V, f * 0.2);
    set_rf(w, id, r::MAX_SPEED, g(w) * 0.1);
    c::set_pf(w, id, pv::SPEED, j7);
    set_jf(w, id, 8, 0.3);
    set_jf(w, id, 9, 2.7);
    set_jf(w, id, 6, 0.0);
    c::set_pf(w, id, pv::TOP, f * 0.066 * gh(w));
    c::set_pu8(w, id, pv::R + r::SEQ_LAND, 4);
    c::set_pu8(w, id, pv::R + r::SEQ_WIND, 2);
    c::set_pu8(w, id, pv::R + r::SEQ_AIR, 3);
    set_jf(w, id, 10, PI);
    set_rf(w, id, r::KEY, 7.5);
    set_rf(w, id, r::AIR, 11.5);
    let (a, d, m) = (jf(w, id, 15), jf(w, id, 16), jf(w, id, 17));
    set_rf(w, id, r::TURN, a);
    set_rf(w, id, r::TURN + 4, d);
    set_rf(w, id, r::TURN + 8, m);
    set_target(w, id, None);
    w.mm(id).state = st::DROP;
    c::set_pi32(w, id, pv::PATIENCE, 0);
    c::set_pf(w, id, pv::TOP, f * 0.1 * gh(w));
    load_j(w, id);
    load_r(w, id);
    let t5 = w.ticks(5);
    c::blend_to(w, id, 5, 0, t5);
}

// ------------------------------------------------------------------------------------------------
// The queries

/// `0x2d4718(bot, flags, at)`: a sphere of 0.1999 at `at` (or the bot) 0.2 up, the bot ignored; on a hit the pushed
/// centre is brought back 0.2 down.
fn sphere(w: &World, id: MobyId, flags: u32, at: Option<V>) -> Option<crate::collision_query::CollOutput> {
    let q = at.unwrap_or_else(|| c::pos(w, id));
    let centre = [q[0], q[1], q[2] + gh(w) * 0.2, q[3]];
    let mut o = w.coll_sphere(pv4(centre), Pf::f(gh(w) * 0.1999), flags, Some(id))?;
    let pc = o.pushed_centre.unwrap_or([centre[0], centre[1], centre[2]]);
    o.pushed_centre = Some([pc[0], pc[1], pc[2] - gh(w) * 0.2]);
    Some(o)
}

/// `0x2d48e8(point, bot)`: the ground under `q`: a line (flags 6, the bot ignored) from 6 above `max(q.z − 5, 0.1)`
/// down to it: its z (and normal), or 0 with no hit or on a pop surface (0, 1, 3, 8, 0xb, 0xc, 0xd) that is not a
/// creature.
fn ground_z(w: &World, id: MobyId, q: V) -> (f32, [f32; 3]) {
    let lo = (q[2] - 5.0).max(0.1);
    let bottom = [q[0], q[1], lo, q[3]];
    let top = [q[0], q[1], lo + 6.0, q[3]];
    let Some(o) = w.coll_line(pv4(top), pv4(bottom), 6, Some(id)) else { return (0.0, [0.0; 3]) };
    if creature(w, o.moby) { return (o.point[2], o.normal); }
    if matches!(o.surface_id(), 0 | 1 | 3 | 8 | 0xb | 0xc | 0xd) { return (0.0, o.normal); }
    (o.point[2], o.normal)
}

/// `0x2d5b30`: the landing check: a line from 1 above to 0.3 below the bot (flags 6): on a creature or a pop surface
/// → 0xb (explode), else the state unchanged.
fn landing_check(w: &World, id: MobyId) -> u8 {
    let q = c::pos(w, id);
    let a = [q[0], q[1], q[2] + 1.0, q[3]];
    let b = [q[0], q[1], q[2] - 0.3, q[3]];
    let s = w.m(id).state;
    match w.coll_line(pv4(a), pv4(b), 6, Some(id)) {
        Some(o) if creature(w, o.moby) || matches!(o.surface_id(), 0 | 1 | 3 | 8 | 0xb | 0xc | 0xd) => st::EXPLODE,
        _ => s,
    }
}

/// `0x2d49f8(bot, eye, rot, failed, all)`: the target search over the target list 0x1abe80 (module doc). `all` (the
/// update passes 1) scores blocked candidates too. The failed target is passed but not read.
pub fn search(w: &World, id: MobyId, eye: V, all: bool) -> Option<MobyId> {
    let me = c::pos(w, id);
    let rot = w.m(id).rotation;
    let mut best_score = 1e9f32;
    let mut best = None;
    let mut near = true;
    for &t in &w.svc.targets {
        if class_type(w, t) != 5 { continue; }
        let tp = w.m(t).position;
        let d = c::dist3(eye, tp);
        if 40.0 < d { continue; }
        if 30.0 < d { near = false; }
        let aim = [tp[0], tp[1], tp[2] + 0.4, tp[3]];
        let yd = c::diff_rots(rot[2], c::atan(aim[0] - eye[0], aim[1] - eye[1]));
        let pd = c::diff_rots(rot[1], c::atan(d, aim[2] - eye[2]));
        let blocked = if near {
            8.0 < (tp[2] - me[2]).abs() || w.coll_line(pv4(eye), pv4(aim), 6, Some(t)).is_some()
        } else {
            w.m(t).visible == 0
        };
        if !all && blocked { continue; }
        let mut s = c::dist3(eye, aim);
        if 3.25 < tp[2] - me[2] { s += 15.0; }
        s += if blocked { 5.0 } else { 0.0 };
        s = s + pd * pd + yd * yd;
        if s < best_score {
            best_score = s;
            best = Some(t);
        }
    }
    if best.is_none() {
        let feet = w.hero.pos.map(|x| x.to_f32());
        if 3.2 < c::dist2(me, feet) { best = w.hero_moby; }
    }
    best
}

/// `0x2d4cb8(h0, spread, n, up, len, back, bot, from, target, &clear)`: the heading to walk (module doc of §18): a
/// probe along the target's heading (h0 without a target), a fan of `n` probes from `h0 − spread/2` by `spread/n`
/// (the longest clear line or the farthest wall, the smallest turn from the first heading breaking ties), two whisker
/// lines beside the result nudging it off walls, at most 144° from the bot's yaw (else halfway). Every probe runs from
/// `back` ahead at `up` to `len` ahead at `2·up` (flags 4, the bot ignored); a clear probe needs ground under its end
/// ([`ground_z`]). A probe that meets the bot's target (or, without a target, the world) or a creature returns its
/// heading at once with `clear` = 100; else `clear` is the longest clear length found.
#[allow(clippy::too_many_arguments)]
fn hop(w: &World, id: MobyId, h0: f32, spread: f32, n: f32, up: f32, len: f32, back: f32, from: V, tgt: Option<V>, clear: &mut f32) -> f32 {
    let step = spread / n;
    let cnt = n as i32;
    let me = c::pos(w, id);
    let mine = target(w, id);
    *clear = 0.0;
    let ht = tgt.map_or(h0, |t| c::atan(t[0] - me[0], t[1] - me[1]));
    let half = spread * 0.5;
    let mut h = c::sub_rot(h0, half);
    let probe = |h: f32| -> (V, V) {
        let (co, si) = (h.cos(), h.sin());
        let a = [from[0] + co * back, from[1] + si * back, from[2] + up, from[3]];
        let b = [from[0] + co * len, from[1] + si * len, from[2] + up + up, from[3]];
        (a, b)
    };
    let hits_target = |o: &crate::collision_query::CollOutput| o.moby == mine || creature(w, o.moby);
    let mut best = h0;
    let mut best_len = 0.0f32;
    let mut best_diff = std::f32::consts::TAU;
    let (a, b) = probe(ht);
    match w.coll_line(pv4(a), pv4(b), 4, Some(id)) {
        None => {
            if ground_z(w, id, b).0 != 0.0 {
                *clear = len;
                best = ht;
                best_len = len;
            }
        }
        Some(o) => {
            if hits_target(&o) {
                *clear = 100.0;
                return ht;
            }
            best_len = c::dist3(a, [o.point[0], o.point[1], o.point[2], 0.0]);
            best_diff = 0.0;
            best = ht;
            if *clear < best_len { *clear = best_len; }
        }
    }
    for _ in 0..cnt {
        let (a, b) = probe(h);
        match w.coll_line(pv4(a), pv4(b), 4, Some(id)) {
            None => {
                if ground_z(w, id, b).0 != 0.0 {
                    let diff = c::diff_rots(h, ht);
                    if !(best_diff <= diff && len <= best_len) {
                        best = h;
                        best_len = len;
                        best_diff = diff;
                        if *clear < len { *clear = len; }
                    }
                }
            }
            Some(o) => {
                if hits_target(&o) {
                    *clear = 100.0;
                    return h;
                }
                let d = c::dist3(a, [o.point[0], o.point[1], o.point[2], 0.0]);
                let diff = c::diff_rots(h, ht);
                if best_len < d || (d == best_len && diff < best_diff) {
                    best = h;
                    best_len = d;
                    best_diff = diff;
                    if *clear < d { *clear = d; }
                }
            }
        }
        h = c::add_rot(h, step);
    }
    // The whiskers: from `back` to the side at `up / 8`, `len` along the result, 0.12 higher.
    let whisker = |side: f32, best: f32| -> (V, V) {
        let (co, si) = (side.cos(), side.sin());
        let a = [from[0] + co * back, from[1] + si * back, from[2] + up * 0.125, from[3]];
        let b = [a[0] + best.cos() * len, a[1] + best.sin() * len, a[2] + 0.12, a[3]];
        (a, b)
    };
    let side = c::sub_rot(best, FRAC_PI_2);
    let (a, b) = whisker(side, best);
    if let Some(o) = w.coll_line(pv4(a), pv4(b), 4, Some(id)) {
        if o.moby != mine && !creature(w, o.moby) {
            let d = c::dist3(a, [o.point[0], o.point[1], o.point[2], 0.0]);
            best = c::add_rot(best, ((len - d) * PI) / (len * 3.0));
        }
    }
    let side = c::add_rot(side, PI);
    let (a, b) = whisker(side, best);
    if let Some(o) = w.coll_line(pv4(a), pv4(b), 4, Some(id)) {
        if o.moby != mine && !creature(w, o.moby) {
            let d = c::dist3(a, [o.point[0], o.point[1], o.point[2], 0.0]);
            best = c::sub_rot(best, ((len - d) * PI) / (len * 3.0));
        }
    }
    let yaw = c::yaw(w, id);
    if 2.513_274_2 < c::diff_rots(best, yaw) { best = c::add_rot(yaw, c::sub_rot(best, yaw) * 0.5); }
    best
}

/// `0x2d5518(bot, R, t)`: the jump toward `t`: its speed across `min(g·35·max / (|dz| + 1) · (d² + 0.25), max)`, its
/// heading, the lob's vertical speed (`0x26faf0`, at most 7·dt; the speed across scaled down with it), the jump's
/// state 0 and turn speed 0.
fn solve_jump(w: &mut World, id: MobyId, t: V) {
    set_rf(w, id, r::TURN_V, 0.0);
    for (k, x) in t.iter().enumerate() { set_rf(w, id, r::TARGET + 4 * k, *x); }
    let me = c::pos(w, id);
    let d = c::dist2(me, t);
    let dz = (t[2] - me[2]).abs();
    let g = rf(w, id, r::GRAVITY);
    let max = rf(w, id, r::MAX_SPEED);
    let s = ((g * 35.0 * max) / (dz + 1.0)) * (d * d + 0.25);
    set_rf(w, id, r::SPEED, s.min(max));
    set_rf(w, id, r::HEADING, c::atan(t[0] - me[0], t[1] - me[1]));
    let mut time = 0.0;
    let up = knock::lob_up(rf(w, id, r::SPEED), -g, me, t, &mut time);
    set_rf(w, id, r::VZ, up.min(DT * 7.0));
    c::set_pu8(w, id, pv::R + r::STATE, 0);
    let vz = rf(w, id, r::VZ);
    let sp = rf(w, id, r::SPEED);
    // A target straight above or below (no distance across) gives 0 / 0 here; the PS2 FPU's result is not modelled
    // [L]: the speed is kept.
    if vz != 0.0 { set_rf(w, id, r::SPEED, sp * (up / vz)); }
}

/// `0x26e3e8(g, pos, v)`: the height of a lob's apex: `n = trunc(v.z / g)`, `pos.z + v.z·n − g·(n² + n)/2`.
fn apex(g: f32, z: f32, vz: f32) -> f32 {
    let n = (vz / g) as i32;
    (z + vz * n as f32) - ((n * n + n) >> 1) as f32 * g
}

/// `0x2d5658(bot, pvars, target, check)`: a jump toward `target` (module doc): true when one was planned (+0xbc = 0xd;
/// the jump record solved and saved).
fn plan_jump(w: &mut World, id: MobyId, target: V, check: bool) -> bool {
    let me = c::pos(w, id);
    let size = w.m(id).scale / w.class_scale(CLASS).to_f32();
    let mut d = c::sub(target, me);
    {
        let mut d4 = pvq(d);
        crate::hero::physics::clamp_len2(&mut d4, Pf::f(size * 5.2));
        d = d4.map(|x| x.to_f32());
    }
    let mut q = c::add(me, d);
    let half = c::scale(d, 0.5);
    let hl = c::len2(half);
    let mid = c::add(me, half);
    let (g, n) = ground_z(w, id, q);
    q[2] = g;
    if g != 0.0 && g - me[2] <= size * 3.0 {
        let n = c::set_len3([n[0], n[1], n[2], 0.0], 1.0);
        if n[0].abs() + n[1].abs() <= n[2].abs() {
            q[2] += size * 0.025;
            if sphere(w, id, 6, Some(q)).is_none() {
                q[2] -= size * 0.025;
                let a = [me[0], me[1], me[2] + size * 0.2, me[3]];
                let b = [q[0], q[1], q[2] + size * 0.2, q[3]];
                if !check || w.coll_line(pv4(a), pv4(b), 6, Some(id)).is_some() {
                    load_r(w, id);
                    solve_jump(w, id, q);
                    save_r(w, id);
                    let mut top = c::add(c::scale(d, 0.5), me);
                    top[2] = apex(rf(w, id, r::GRAVITY), me[2], c::pf(w, id, pv::JUMP_VZ));
                    let tgt = target_of(w, id);
                    let clear = |w: &World, x: V, y: V| w.coll_line(pv4(x), pv4(y), 6, Some(id)).is_none_or(|o| o.moby == tgt);
                    if clear(w, a, top) {
                        if clear(w, top, b) {
                            w.mm(id).cmd = 0xd;
                            return true;
                        }
                        if hl <= 0.5 { return false; }
                        return plan_jump(w, id, mid, true);
                    }
                }
            }
        }
    }
    if hl <= 0.5 { return false; }
    plan_jump(w, id, mid, true)
}

fn target_of(w: &World, id: MobyId) -> Option<MobyId> { target(w, id) }

/// `0x270340(bot, R)`: the jump (module doc): 0 the wind-up (sequence R+0x2c past key 7.5; on its wrap the air
/// sequence), 1 in the air (turning to the heading, moving by the speeds, gravity; falling and near enough above the
/// target's height — the fall over `2·R+0x28 + 10` ticks would pass it — the landing sequence, 2), 2 falling to the
/// ground (`GroundHeight`) → 3; the sequence's wrap → 4 (landed). Returns the state.
fn jump_step(w: &mut World, id: MobyId) -> u8 {
    let s = rb(w, id, r::STATE);
    let air = |w: &mut World| -> u8 {
        let (a, d, m) = (rf(w, id, r::TURN), rf(w, id, r::TURN + 4), rf(w, id, r::TURN + 8));
        turn::spring_turn2_pvar(w, id, rf(w, id, r::HEADING), a, d, m, pv::R + r::TURN_V);
        let (h, sp, vz) = (rf(w, id, r::HEADING), rf(w, id, r::SPEED), rf(w, id, r::VZ));
        let q = c::pos(w, id);
        c::set_pos(w, id, [q[0] + h.cos() * sp, q[1] + h.sin() * sp, q[2] + vz, q[3]]);
        let g = rf(w, id, r::GRAVITY);
        let v = vz - g;
        set_rf(w, id, r::VZ, v);
        if 0.0 <= v { return rb(w, id, r::STATE); }
        let t = w.svc.timing.scale(Pf::f(rf(w, id, r::AIR) + rf(w, id, r::AIR) + 10.0)).to_f32();
        if rf(w, id, r::TARGET + 8) - (v * t - g * 0.5 * t * t) < c::pos(w, id)[2] { return rb(w, id, r::STATE); }
        let seq = rb(w, id, r::SEQ_LAND);
        w.anim_blend(id, seq, 0, 10);
        c::set_pu8(w, id, pv::R + r::STATE, 2);
        2
    };
    match s {
        1 => air(w),
        0 => {
            if w.m(id).anim.seq_a != rb(w, id, r::SEQ_WIND) { return s; }
            if ground::key_time(w, id) <= rf(w, id, r::KEY) { return s; }
            if w.m(id).anim.flags & 2 != 0 {
                let seq = rb(w, id, r::SEQ_AIR);
                w.anim_blend(id, seq, 0, 10);
                c::set_pu8(w, id, pv::R + r::STATE, 1);
            }
            air(w)
        }
        2 => {
            let mut q = c::pos(w, id);
            q[2] += rf(w, id, r::VZ);
            c::set_pos(w, id, q);
            let v = rf(w, id, r::VZ) - rf(w, id, r::GRAVITY);
            set_rf(w, id, r::VZ, v);
            let g = ground::ground(w, q, 0.5, 0).z;
            if q[2] <= g {
                w.mm(id).position[2] = g;
                c::set_pu8(w, id, pv::R + r::STATE, 3);
            } else {
                let (h, sp) = (rf(w, id, r::HEADING), rf(w, id, r::SPEED));
                let m = w.mm(id);
                m.position[0] += h.cos() * sp;
                m.position[1] += h.sin() * sp;
            }
            if w.m(id).anim.flags & 2 != 0 { c::set_pu8(w, id, pv::R + r::STATE, 4); }
            rb(w, id, r::STATE)
        }
        3 => {
            if w.m(id).anim.flags & 2 != 0 { c::set_pu8(w, id, pv::R + r::STATE, 4); }
            rb(w, id, r::STATE)
        }
        _ => s,
    }
}

// ------------------------------------------------------------------------------------------------
// The update

/// How a state's case ends: through the landing check (`break` → `0x2d5b30`) or straight to the bounds test.
enum End {
    Check,
    Bounds,
}

/// `0x2d5c10`.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { w.mm(id).pvars.resize(pv::SIZE, 0); }
    if w.svc.game_mode == 2 {
        w.mm(id).mode |= 0x41;
        return;
    }
    w.mm(id).mode &= !0x41;
    if !in_box(c::pos(w, id)) {
        w.delete_moby(id);
        return;
    }
    if w.get_hit(id, 0x80_0001, false).is_some() { w.mm(id).state = st::EXPLODE; }
    let cs = w.class_scale(CLASS).to_f32();
    {
        let (g, gh) = (g(w), gh(w));
        let m = w.mm(id);
        m.scale += (cs * g - m.scale) * 0.05;
        m.anim.speed = if matches!(m.anim.seq_a, 0 | 6 | 7) { gh / g } else { 1.0 };
    }
    if w.m(id).state != st::EXPLODE {
        c::set_pi32(w, id, pv::J, (g(w) * 183.296) as i32 + 1);
        set_jf(w, id, 12, 0.0);
        set_jf(w, id, 1, g(w) * 0.198 + 0.02);
        if w.m(id).state == st::INIT { init(w, id); }
        if w.m(id).state == st::PARKED {
            if c::pi32(w, id, pv::OWNER) != 0 && in_box(c::pos(w, id)) { return; }
            w.delete_moby(id);
            return;
        }
        let age = c::pi32(w, id, pv::AGE) + 1;
        c::set_pi32(w, id, pv::AGE, age);
        if w.ticks(0xe10) < age { w.mm(id).state = st::EXPLODE; }
        c::dec_timer_pvar_i32(w, id, pv::PATIENCE);
        let q = c::pos(w, id);
        c::set_pv4(w, id, pv::PREV, q);
        if let Some(o) = sphere(w, id, 5, None) {
            let touch_creature = o.moby.is_some_and(|m| o.kind < 0 && w.m(m).mode & mode::TARGETABLE != 0 && class_type(w, m) == 5);
            if touch_creature {
                w.mm(id).state = st::EXPLODE;
            } else if o.moby.is_none() || o.kind < 0 {
                knocked(w, id, &o);
            }
        }
        if c::pos(w, id)[2] < 0.1 {
            w.delete_moby(id);
            return;
        }
        if 4000 < c::pi32(w, id, pv::PATIENCE) { w.mm(id).state = st::EXPLODE; }
    }
    let end = match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            End::Bounds
        }
        1 | 9 | 10 => {
            if w.m(id).anim.flags & 2 != 0 { w.mm(id).state = 2; }
            End::Check
        }
        2..=5 => chase(w, id),
        st::TURN => {
            let pt = c::pv4(w, id, pv::POINT);
            let q = c::pos(w, id);
            let h = c::atan(pt[0] - q[0], pt[1] - q[1]);
            load_j(w, id);
            let (a, d, m) = (jf(w, id, 15), jf(w, id, 16), jf(w, id, 17));
            turn::spring_turn2_pvar(w, id, h, a + a, d * 0.5, m + m, pv::J + 0x14);
            save_j(w, id);
            if c::diff_rots(c::yaw(w, id), h) < f32::from_bits(0x3db2_b8c2) {
                let seq = rb(w, id, r::SEQ_WIND);
                let t3 = w.ticks(3);
                w.anim_blend(id, seq, 0, t3);
                w.mm(id).state = st::JUMP;
            }
            End::Check
        }
        st::JUMP => {
            load_r(w, id);
            if jump_step(w, id) == 4 {
                w.mm(id).state = st::NEAR;
                let t3 = w.ticks(3);
                c::blend_to(w, id, 7, 0, t3);
                c::set_pf(w, id, pv::TOP, SPEED * 0.1 * gh(w));
            }
            save_r(w, id);
            End::Check
        }
        st::BACK => {
            let mut vel = c::pv4(w, id, pv::VEL);
            vel[2] -= DT2 * 9.0;
            load_j(w, id);
            let r = walker::move_collide(w, id, g(w) * 0.7, radius(w, id), jf(w, id, 12), &mut vel, 0);
            if r & 2 != 0 {
                vel[2] = 0.0;
                vel = c::scale(vel, 0.5);
            }
            let mut v4 = pvq(vel);
            crate::hero::physics::clamp_len2(&mut v4, Pf::f(SPEED * 0.05));
            vel = v4.map(|x| x.to_f32());
            c::set_pv4(w, id, pv::VEL, vel);
            if w.m(id).anim.flags & 2 != 0 {
                if r & 2 != 0 { w.mm(id).state = 2; } else { fall_anim(w, id); }
            }
            face_motion(w, id);
            End::Check
        }
        st::EXPLODE => {
            explode(w, id);
            return;
        }
        st::LAND_ANIM => {
            if w.m(id).anim.flags & 2 != 0 {
                w.mm(id).state = st::NEAR;
                let t10 = w.ticks(10);
                c::blend_to(w, id, 7, 0, t10);
                c::set_pf(w, id, pv::TOP, SPEED * 0.1 * gh(w));
            }
            End::Bounds
        }
        st::PARKED => End::Bounds,
        st::DROP => {
            drop_out(w, id);
            End::Check
        }
        _ => {
            w.mm(id).state = 0;
            End::Bounds
        }
    };
    if let End::Check = end {
        let s = landing_check(w, id);
        w.mm(id).state = s;
    }
    if !in_box(c::pos(w, id)) { w.delete_moby(id); }
}

/// The sphere's push (world or a moby's primitive): onto its pushed centre, the velocity along the face (at most
/// 0.05 a tick), backing off (8, the fall sequence 5); a target is remembered as failed (+500 patience).
fn knocked(w: &mut World, id: MobyId, o: &crate::collision_query::CollOutput) {
    let q = o.pushed_centre.unwrap_or([0.0; 3]);
    let w4 = w.m(id).position[3];
    c::set_pos(w, id, [q[0], q[1], q[2], w4]);
    // The game then tests `fabs(z − z)` (the old height twice) against 0.6 to hold the height back 0.01 above it:
    // never true, so the new height stands.
    let mut v = pvq([o.normal[0], o.normal[1], o.normal[2], 0.0]);
    crate::hero::physics::clamp_len3(&mut v, Pf::f(SPEED * 0.05));
    crate::hero::physics::clamp_len2(&mut v, Pf::f(SPEED * 0.05));
    c::set_pv4(w, id, pv::VEL, v.map(|x| x.to_f32()));
    w.mm(id).state = st::BACK;
    if let Some(t) = target(w, id) {
        c::set_pi32(w, id, pv::FAILED, t as i32 + 1);
        let n = c::pi32(w, id, pv::PATIENCE) + 500;
        c::set_pi32(w, id, pv::PATIENCE, n);
    }
    let t5 = w.ticks(5);
    c::blend_to(w, id, 5, 0, t5);
}

/// The fall sequence 5 (blended over 5 ticks) unless playing.
fn fall_anim(w: &mut World, id: MobyId) {
    let t5 = w.ticks(5);
    c::blend_to(w, id, 5, 0, t5);
}

/// `SpringTurn2(atan(vel), J7, J8, J9, bot, &+0x40)` then `0x2d4830` (which writes J4 over the +0x40 the turn kept:
/// the game's).
fn face_motion(w: &mut World, id: MobyId) {
    let v = c::pv4(w, id, pv::VEL);
    let h = c::atan(v[0], v[1]);
    let (a, d, m) = (jf(w, id, 7), jf(w, id, 8), jf(w, id, 9));
    turn::spring_turn2_pvar(w, id, h, a, d, m, pv::SPEED);
    save_j(w, id);
}

/// State 0xe: the drop out of the canister (module doc).
fn drop_out(w: &mut World, id: MobyId) {
    let q = c::pos(w, id);
    if let Some(o) = w.coll_sphere(pv4(q), Pf::f(g(w) * 0.5), 1, Some(id)) {
        if o.moby.is_some_and(|m| w.m(m).o_class == CLASS) {
            let pc = o.pushed_centre.unwrap_or([q[0], q[1], q[2]]);
            let mut d = [pc[0] - q[0], pc[1] - q[1], pc[2] - q[2], 0.0];
            let l = c::len3(d);
            d[2] = 0.0;
            let d = c::set_len3(d, l * 1.1);
            let q2 = c::add(q, d);
            c::set_pos(w, id, q2);
        }
    }
    let mut vel = c::pv4(w, id, pv::VEL);
    vel[2] -= DT2 * 9.0;
    load_j(w, id);
    let r = walker::move_collide(w, id, g(w) * 0.7, radius(w, id), jf(w, id, 12), &mut vel, 0);
    if r & 2 != 0 {
        vel[2] = 0.0;
        vel = c::scale(vel, 0.5);
    }
    let mut v4 = pvq(vel);
    crate::hero::physics::clamp_len3(&mut v4, Pf::f(SPEED * 0.05));
    c::set_pv4(w, id, pv::VEL, v4.map(|x| x.to_f32()));
    if r & 2 != 0 { w.mm(id).state = 2; } else { fall_anim(w, id); }
    face_motion(w, id);
}

/// States 2..5: the chase and the walk (module doc).
fn chase(w: &mut World, id: MobyId) -> End {
    let f = SPEED;
    // Near the target: explode on touching it, or plan the jump to it (overwritten by the search below: the game's).
    if let Some(t) = target(w, id).filter(|&t| !dead(w, t)) {
        let tp = w.m(t).position;
        let mut tz = tp[2];
        let mut reach = gh(w);
        if let Some(rec) = crate::targeting::record(w.m(t)) {
            let pvs = &w.m(t).pvars;
            reach = gh(w) + pvs[rec + 0x0a] as f32 * 0.125;
            tz += p::ff(pvs, rec + 0x10);
        }
        let me = c::pos(w, id);
        if c::dist3(me, tp) < reach && w.coll_sphere(pv4(me), Pf::f(gh(w)), 1, Some(id)).is_some() {
            w.mm(id).state = st::EXPLODE;
            return End::Bounds;
        }
        if c::dist2(me, tp) < reach * 0.5 && (tz - (me[2] + gh(w) * 0.1)).abs() < reach * 0.5 {
            load_r(w, id);
            solve_jump(w, id, tp);
            save_r(w, id);
            c::set_pv4(w, id, pv::POINT, tp);
            c::set_pf(w, id, pv::TOP, SPEED * 0.066 * gh(w));
            if w.m(id).anim.seq_b != 0 {
                let t2 = w.ticks(2);
                w.anim_blend(id, 0, 0, t2);
            }
            w.mm(id).state = st::TURN;
        }
    }
    let me = c::pos(w, id);
    let eye = [me[0], me[1], me[2] + g(w) * 0.3, me[3]];
    if w.rng.randi(4) == 0 {
        let t = search(w, id, eye, true);
        set_target(w, id, t);
    }
    c::set_pf(w, id, pv::TOP, f * 0.1 * gh(w));
    w.mm(id).state = st::NEAR;
    let mut clear = 0.0f32;
    let back = radius(w, id);
    let tgt = target(w, id);
    if tgt.is_none() {
        let h = w.rng.randf(-PI, PI);
        if w.rng.randi(4) == 0 {
            let hd = hop(w, id, h, f32::from_bits(0x3fec_8b1f), 4.0, g(w) * 0.35, 4.0, back, me, None, &mut clear);
            c::set_pf(w, id, pv::HEADING, hd);
        } else {
            clear = 5.0;
        }
    } else if let Some(t) = tgt.filter(|&t| Some(t) != w.hero_moby) {
        let ok = !dead(w, t) && w.classes.info(w.m(t).o_class).is_some() && class_type(w, t) == 5 && w.m(t).mode & mode::TARGETABLE != 0;
        if !ok {
            set_target(w, id, None);
            w.mm(id).state = st::WALK_LO;
            c::set_pf(w, id, pv::TOP, f * 0.0273 * gh(w));
            return End::Bounds;
        }
    }
    let before = w.m(id).state;
    match target(w, id) {
        None => {
            w.mm(id).state = st::FAR;
            let t10 = w.ticks(10);
            c::blend_to(w, id, 6, 0, t10);
            c::set_pf(w, id, pv::TOP, f * 0.0273 * gh(w));
        }
        Some(t) => {
            let tp = w.m(t).position;
            if w.rng.randi(4) == 0 && plan_jump(w, id, tp, true) {
                let tp = w.m(t).position;
                c::set_pv4(w, id, pv::POINT, tp);
                c::set_pf(w, id, pv::TOP, f * 0.066 * gh(w));
                if w.m(id).anim.seq_b != 0 {
                    let t2 = w.ticks(2);
                    w.anim_blend(id, 0, 0, t2);
                }
                w.mm(id).state = st::TURN;
                return End::Bounds;
            }
            let d = c::dist2(c::pos(w, id), w.m(t).position);
            if Some(t) == w.hero_moby {
                w.mm(id).state = st::FAR;
                if w.m(id).anim.flags & 2 != 0 && w.m(id).anim.seq_b != 6 {
                    let t0 = w.ticks(0);
                    w.anim_blend(id, 6, 0, t0);
                }
                c::set_pf(w, id, pv::TOP, f * 0.0273 * gh(w));
            }
            if d < 8.0 || d <= 20.0 {
                w.mm(id).state = st::NEAR;
                let t10 = w.ticks(10);
                c::blend_to(w, id, 7, 0, t10);
                c::set_pf(w, id, pv::TOP, f * 0.1 * gh(w));
            } else {
                w.mm(id).state = st::FAR;
                let t10 = w.ticks(10);
                c::blend_to(w, id, 0, 0, t10);
                c::set_pf(w, id, pv::TOP, f * 0.0273 * gh(w));
            }
            if w.rng.randi(4) == 0 {
                let yaw = c::yaw(w, id);
                let tp = w.m(t).position;
                let hd = hop(w, id, yaw, f32::from_bits(0x3fec_8b1f), 4.0, f32::from_bits(0x3eb3_3333), 4.0, back, me, Some(tp), &mut clear);
                c::set_pf(w, id, pv::HEADING, hd);
            } else {
                clear = 5.0;
            }
        }
    }
    if w.m(id).state == st::NEAR && before == st::FAR {
        w.mm(id).state = st::LAND_ANIM;
        let t10 = w.ticks(10);
        c::blend_to(w, id, 1, 0, t10);
        return End::Check;
    }
    walk(w, id, clear)
}

/// The walk along the heading (+0x64): the point 5 ahead (1 up); the turn; a probe 5 × the velocity ahead (0.15 up,
/// flags 6): clear (or the target, or a creature) → `walker::walk_to` (a height jump of more than 1 undone, the sphere
/// then pushing it); blocked → backing off. A blocked step or a clear length under 4 plans a jump there, else it backs
/// off (8).
fn walk(w: &mut World, id: MobyId, mut clear: f32) -> End {
    let me = c::pos(w, id);
    let h = c::pf(w, id, pv::HEADING);
    let pt = [me[0] + h.cos() * 5.0, me[1] + h.sin() * 5.0, me[2] + 1.0, c::pv4(w, id, pv::POINT)[3]];
    c::set_pv4(w, id, pv::POINT, pt);
    load_j(w, id);
    let (a, d, m) = (jf(w, id, 15), jf(w, id, 16), jf(w, id, 17));
    turn::spring_turn2_pvar(w, id, h, a, d, m, pv::TURN_V);
    let vel = c::pv4(w, id, pv::VEL);
    let mut ahead = c::scale(vel, 5.0);
    ahead[2] = 0.0;
    let q = c::pos(w, id);
    let from = [q[0], q[1], q[2] + g(w) * 0.15, q[3]];
    let to = c::add(from, ahead);
    let tgt = target(w, id);
    let bits = match w.coll_line(pv4(from), pv4(to), 6, Some(id)) {
        Some(o) if o.moby != tgt && !creature(w, o.moby) => {
            clear = 0.0;
            let q = c::pos(w, id);
            let v = c::set_len3([q[0] - pt[0], q[1] - pt[1], 0.01, vel[3]], 0.1);
            c::set_pv4(w, id, pv::VEL, v);
            2
        }
        _ => {
            let z0 = c::pos(w, id)[2];
            let mut out = [0.0; 4];
            let r = walker::walk_to(w, id, pv::J, pt, &mut out);
            c::set_pv4(w, id, pv::VEL, out);
            if 1.0 < (c::pos(w, id)[2] - z0).abs() {
                w.mm(id).position[2] = z0;
                if let Some(o) = sphere(w, id, 5, None) {
                    let pc = o.pushed_centre.unwrap_or([0.0; 3]);
                    let w4 = w.m(id).position[3];
                    c::set_pos(w, id, [pc[0], pc[1], pc[2], w4]);
                    if 1.0 < (c::pos(w, id)[2] - z0).abs() { w.mm(id).position[2] = z0; }
                }
            }
            r
        }
    };
    save_j(w, id);
    if bits & 2 != 0 || clear < 4.0 {
        let pt = c::pv4(w, id, pv::POINT);
        if plan_jump(w, id, pt, false) {
            c::set_pf(w, id, pv::TOP, SPEED * 0.066 * gh(w));
            if w.m(id).anim.seq_b != 0 {
                let t2 = w.ticks(2);
                w.anim_blend(id, 0, 0, t2);
            }
            w.mm(id).state = st::TURN;
            return End::Check;
        }
        if let Some(t) = target(w, id) {
            c::set_pi32(w, id, pv::FAILED, t as i32 + 1);
            let n = c::pi32(w, id, pv::PATIENCE) + 500;
            c::set_pi32(w, id, pv::PATIENCE, n);
        }
        let q = c::pos(w, id);
        let v = c::pv4(w, id, pv::VEL);
        let v = c::set_len3([q[0] - pt[0], q[1] - pt[1], 0.01, v[3]], 0.1);
        c::set_pv4(w, id, pv::VEL, v);
        w.mm(id).state = st::BACK;
        fall_anim(w, id);
        return End::Bounds;
    }
    End::Check
}

/// State 0xb: the blast (module doc) inside the world box, then deleted.
fn explode(w: &mut World, id: MobyId) {
    let q = c::pos(w, id);
    if in_box(q) {
        // The last argument is the gold byte: the gold bots' colour shift.
        let shift = gold(w) as u8;
        fx::beam_explosion_shift(w, &BLAST, Some(id), q, shift);
        let centre = [q[0], q[1], q[2] + g(w) * 0.5, q[3]];
        let list = sphere_mobys_in(w.table, w.svc, w.classes, Pf::f(gh(w)), pvq(centre), 0x15, Some(id), None);
        attack::area_push(w, &list, q, id, 3.0, 1.0, 1.0, None, 0x1_0000, 2, 3);
        // Class sound 0, or 4 for the gold bots.
        let snd = if gold(w) == 0.0 { 0 } else { 4 };
        w.play_sound(snd, 0, id);
    }
    w.delete_moby(id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_update::classes::bomb_water::tests::{pool, Bench};
    use crate::moby_update::classes::decoy::tests::floor;
    use crate::moby_update::services::{deliver_hit_in, HitTemplate};

    fn bot(b: &mut Bench, at: [f32; 3]) -> MobyId {
        let mut w = World::new(&mut b.table, &b.hero, &mut b.rng, &b.ct, &mut b.svc, b.counter);
        create(&mut w, 1, [at[0], at[1], at[2], 1.0], [0.05, 0.0, 0.02, 0.0]).unwrap()
    }

    fn explosion_sounds(b: &Bench, id: MobyId) -> usize { b.svc.sounds.iter().filter(|e| e.moby == id && e.o_class == CLASS && e.index == 0).count() }

    /// State 0 → 0xe → walking: the walker globals' constants (J0 = 184, the step-up 0.218, the turn, top speeds), the
    /// fall sequence; it lands and walks.
    #[test]
    fn drops_out_and_walks() {
        let mut b = Bench::new(floor(8.0, 0x22), &[CLASS], [20.0, 20.0, 8.0]);
        let x = bot(&mut b, [21.0, 20.0, 8.5]);
        b.tick(None);
        let m = &b.table.mobys[x];
        assert_eq!(m.state, st::DROP);
        assert_eq!(p::i32(&m.pvars, pv::J), 184);
        assert_eq!(p::ff(&m.pvars, pv::J + 4), 0.198 + 0.02);
        assert_eq!(p::ff(&m.pvars, pv::R + r::GRAVITY), DT2 * 29.7);
        let mut seen = Vec::new();
        for _ in 0..200 {
            b.tick(None);
            let s = b.table.mobys[x].state;
            if seen.last() != Some(&s) { seen.push(s); }
        }
        assert!(seen.contains(&2) && seen.iter().any(|s| (4..=5).contains(s)), "landed and walked: {seen:?}");
        assert!(b.table.mobys[x].state < 0xfd, "alive");
    }

    /// The lifetime: a bot with nothing to go for (Ratchet near) blows up on its 3601st update (age > `ticks(3600)`):
    /// `SpawnBeamExplosion` (a light, 5 streaks, 2 spark pairs, 4 puffs, the shake) and its class sound 0.
    #[test]
    fn lives_sixty_seconds() {
        let mut b = Bench::new(floor(8.0, 0x22), &[CLASS, fx::LIGHT_CLASS], [20.0, 20.0, 8.0]);
        let x = bot(&mut b, [21.0, 20.0, 8.5]);
        let mut gone = None;
        for t in 1..=3700 {
            let (s15, s11) = (b.parts(15), b.parts(11));
            b.tick(None);
            if b.table.mobys[x].is_deleted() {
                gone = Some(t);
                assert_eq!(b.parts(15) - s15, 5, "5 streaks");
                assert_eq!(b.parts(11) - s11, 4, "2 spark pairs");
                break;
            }
        }
        assert_eq!(gone, Some(3601), "exploded at the end of its life");
        assert_eq!(explosion_sounds(&b, x), 1, "class sound 0");
        assert!(b.alive(fx::LIGHT_CLASS) >= 1, "the explosion light");
        assert!(!b.svc.camera_shakes.is_empty(), "the shake");
    }

    fn hit(b: &mut Bench, x: MobyId, flags: u32) {
        let t = HitTemplate { dir: [Pf::ZERO; 4], attacker: None, flags, b18: 2, b19: 1, h1a: 0, damage: Pf::f(2.0), w20: flags };
        deliver_hit_in(&mut b.table, &mut b.svc.hits, x, &t);
    }

    /// `MobyGetHitMessage(0x800001)`: an explosion's hit (0x800000) or an enemy's (1) sets it off; a wrench-type hit
    /// (0x10000) does not.
    #[test]
    fn explodes_when_hit() {
        let mut b = Bench::new(floor(8.0, 0x22), &[CLASS, fx::LIGHT_CLASS], [20.0, 20.0, 8.0]);
        let x = bot(&mut b, [21.0, 20.0, 8.5]);
        for _ in 0..60 { b.tick(None); }
        hit(&mut b, x, 0x1_0000);
        b.tick(None);
        assert!(!b.table.mobys[x].is_deleted(), "0x10000 ignored");
        hit(&mut b, x, 0x80_0000);
        b.tick(None);
        assert!(b.table.mobys[x].is_deleted() && explosion_sounds(&b, x) == 1, "0x800000 sets it off");
        let y = bot(&mut b, [22.0, 22.0, 8.5]);
        for _ in 0..60 { b.tick(None); }
        hit(&mut b, y, 1);
        b.tick(None);
        assert!(b.table.mobys[y].is_deleted(), "an enemy's hit sets it off");
    }

    /// `0x2d5b30`: landing on a pop surface (the pool's floor, surface 1) sets it off.
    #[test]
    fn explodes_on_a_pop_surface() {
        let mut b = Bench::new(pool(8.0, 4.0), &[CLASS, fx::LIGHT_CLASS], [20.0, 20.0, 9.0]);
        let x = bot(&mut b, [21.0, 20.0, 9.0]);
        let mut gone = None;
        for t in 0..120 {
            b.tick(None);
            if b.table.mobys[x].is_deleted() { gone = Some(t); break; }
        }
        assert!(gone.is_some() && explosion_sounds(&b, x) == 1, "went off on the water's floor");
    }

    /// Patience: a counter above 4000 (500 per failed target, −1 a tick) sets it off.
    #[test]
    fn runs_out_of_patience() {
        let mut b = Bench::new(floor(8.0, 0x22), &[CLASS, fx::LIGHT_CLASS], [20.0, 20.0, 8.0]);
        let x = bot(&mut b, [21.0, 20.0, 8.5]);
        for _ in 0..30 { b.tick(None); }
        p::set_i32(&mut b.table.mobys[x].pvars, pv::PATIENCE, 4001);
        b.tick(None);
        assert!(!b.table.mobys[x].is_deleted(), "4000 after the tick's decrement: not over");
        p::set_i32(&mut b.table.mobys[x].pvars, pv::PATIENCE, 4002);
        b.tick(None);
        assert!(b.table.mobys[x].is_deleted(), "over 4000");
    }

    /// `0x26e3e8`: the apex of a lob.
    #[test]
    fn lob_apex() {
        let g = DT2 * 29.7;
        let vz = DT * 7.0;
        let n = (vz / g) as i32;
        assert_eq!(apex(g, 1.0, vz), 1.0 + vz * n as f32 - ((n * n + n) / 2) as f32 * g);
        assert!(apex(g, 1.0, vz) > 1.0);
    }
}
