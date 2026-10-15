//! U162 (census 2026-10-02): class 217, Eudora's loggers (level04 0x2ba520, the only copy: 5 placed). The name is
//! descriptive [L]. Read from the level04 decomp and disassembly. Native `f32`; the `rand` draws in the game's order.
//!
//! A big creature carrying a prop (class 570, made on its joint 11 and held at joint 13) that idles at its home and
//! growls, turns its head toward Ratchet, walks a path once he enters its cuboid (or when alerted), and swings at him
//! within 4.5 (damage 1 over keys 17–20 of its attack). Hits make it flinch (a yaw on joint list 0), stagger back from
//! Ratchet, or knock it down; a kill flings it (a knock flight) and it blows up into three pieces. During scenes (game
//! mode 2) it and its prop are hidden.
//!
//! **Pvars** (0x450): header (+0x00 D, +0x0c F, +0x10 K); +0x20 D (health 3, meter 3, column 2, +0x2e 1 while alive,
//! +0x30 1.5, +0x38 the lure); +0x58 / +0x5a bytes 10 / 12; +0xb0 F; +0xc0 K; +0x120 / +0x160 manipulators (joint lists 0
//! / 12: the flinch, the head); +0x3f0 home; +0x400 byte starts in 3; +0x401 the last state; +0x402 s16 the hold; +0x404
//! s16 the stagger guard; +0x406 byte the alert timer; +0x407 byte the growl timer; +0x408 the flinch yaw; +0x410 the
//! turn velocity; +0x414 / +0x418 a dash's heading and speed; +0x420 the fall speed; +0x424 the prop (port: index + 1);
//! +0x428 the head's reference yaw; +0x42c the stagger push; +0x430 the trigger cuboid, +0x434 the path, +0x438 s16 its
//! node, +0x43a s16 walked; +0x43c the range; +0x448 s32 the flinch timer.
//!
//! | address | what | port |
//! |---|---|---|
//! | top | alerted = lure or the alert timer; `0x2563b8(2.5, m, 0, +0x120)` the big-head cheat; the shadow probe within 27 (+0x7f 0x15) and again within 32 (+0x7f 0x1a); the target (range ×2 alerted), its xy distance and heading; key time | [`update`] |
//! | top | game mode 2 → it and the prop hidden (mode \|= 0x41); else a hidden one shown | [`update`] |
//! | hits | the guard timer; not in 0 / 10: `MobyGetHitMessage(m, 0x210000)`, the resolver (column 4); a hit with damage: K (flags 9, drag 12·dt², gravity 35·dt², speed 7·dt, +0x3d 0); health −= damage (≤ 0 → 1); the guard running and not killed → the flinch (yaw −30° unless one runs, `ticks(30)`), the flash (colour 0x78); else by reaction (module table below) | [`hits`] |
//! | reaction 1 | K +0x28 1, radius 0x400, `0x255150(5.7, 4)` (`knock::ballistic`), aimed along the push, the flight (seq 7), keys 12 / 25, 10, the flash, `SetDeathBits` | [`hits`] |
//! | 3 / 6 / 8 | class sound 11; the flinch; Ratchet's hit, or more than 1 damage: unless in seq 4 at keys 17–20, knocked down (`cmd` \|= 5, seq 5 over `ticks(6)` on it and the prop, both at anim speed 2, 8); the stagger push (3.4/`scale(7)`)·½ | [`hits`] |
//! | 4 / 5 | class sound 11; seq 6 from frame 2 (`ticks(4)`), hold `ticks(40)`, 1; push 3.4/`scale(7)` | [`hits`] |
//! | 7 | class sound 11; the flinch; knocked down as 3 and a flight (seq 5), keys 5 / 10; push 3.4/`scale(7)` | [`hits`] |
//! | after a stagger | the flash (0x78), guard `ticks(70)`, alert `ticks(240)` | [`hits`] |
//! | | +0xa4 = 0xff; the lure → alert `ticks(240)`; the alert timer; not in 0 and D+0x0e ≠ 1 → the prop and itself deleted; the hold timer | [`update`] |
//! | 0 | D fields, `cmd` 0, home, the manipulators (lists 0 and 12), reference yaw, +0x58 10 / +0x5a 12; +0x400 → seq 10 (`ticks(12)`), 3; else 1 | [`init`] |
//! | 1 | seq 6 at hold `ticks(37)` → class sound 2; idle: out of range on a wrap → seq 0 or 1 (`randi(6)`: 1 in 6); in range → seq 13 (`ticks(24)`); the path: alerted or Ratchet in the cuboid → `cmd` \|= 1, 4; on seq 13 the growl (class sound 10 every `ticks(180)`); hold out: within 4.5 → seq 4 from frame 4, 7; in range and `cmd` & 1 clear → seq 2, 2, `cmd` \|= 1 | [`idle`] |
//! | 2 | seq 2 to its end → seq 13 in range else 0 (`ticks(12)`), 1 | [`update`] |
//! | 3 | wrapped → seq 11; hold out: as 1's | [`update`] |
//! | 4 | seq 3; within 4.5 → seq 4, 7 (the walk goes on); the node reached (1, xy) → the next; the end → seq 13 or 0 (`ticks(24)`), 1; `SpringTurn` to the node (4π·dt², 2π·dt², π·dt), 6·dt ahead (`0x26d270(0.25, 1.5, 0.75, 50°, …, 1)`) | [`walk`] |
//! | 7 | keys 17–20 of its sequence, 2 in height, within 4.5: `0x26eaa8(1, target, m, 1, target + 0.75 up, 0.2·(cos, sin)(heading))`; past key 20 the anim speed eases to 1 by 3·dt; seq 4 to its end → hold `ticks(90)`, seq 13 or 0, 1 | [`update`] (`attack::hit_moby`) |
//! | 8 | knocked down: seq 5 past key 13 with `cmd` & 4 → seq 4 from frame 4 (`ticks(6)`), anim speed 1.5, 7; else the dash (none on the reachable paths: speed 0) → 1, hold 0 | [`update`] |
//! | 10 | the flight; landed / wrapped or below z 1 → [`blow_up`] | [`update`] (`knock::update`) |
//! | tail | the flash; the stagger: pushed from Ratchet (`0x26d270`, flag 1), eased by −3.4/`scale(7)`² while on the ground; the flinch yaw ×0.96 into list 0's node (`0x221e38`); the fall (29.8·dt²) onto `GroundHeight(0.5)`; 8 below home → [`blow_up`]; else the head turns by the change in the heading to the target less the turn of the body (list 12's node ⊗ `q(z)`); the prop made at joint 11 (`0x2d34d8`) or held at joint 13 (`moby_attach_to_joint`) | [`update`] |
//!
//! **[`blow_up`]**: `SpawnBeamExplosion(0, 0, 2, 1, 100000, 1.5, 15, m, 0, joint 6, 10, 3, 4, −1, 1, 1)` (`0x2ba470`), the
//! prop deleted, `SetDeathBits`, `BreakFxB` 0x798 / 0x799 / 0x79a, `DeleteMoby`.
//!
//! **The game's, not ported:** states 5, 6 and 9 (a leg-walker run with dashes inside an area, `0x295de8` /
//! `0x295778` on +0x1a0) form a closed cycle no other state or class enters, and the walker's sequence table there is
//! never filled; they are left out. The head's matrix Euler (`0x2721f0`) is a dead store.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, attack, damage, flash, fx, knock, react, target, turn, walker, DT, DT2};
use crate::moby_update::manip;
use crate::moby_update::services::{self as sv, World};
use crate::ps2v::Pf;
use std::f32::consts::PI;

pub const REFERENCE_LEVEL: u32 = 4;
pub const UPDATE_FN: u32 = 0x2b_a520;
pub const CLASSES: [i16; 1] = [217];
/// The prop it carries (`0x2d34d8`).
pub const PROP: i16 = 570;
/// The pieces of [`blow_up`].
pub const PIECES: [i16; 3] = [0x798, 0x799, 0x79a];

mod p {
    pub const D: usize = 0x20;
    pub const LURE: usize = 0x38;
    pub const F: usize = 0xb0;
    pub const K: usize = 0xc0;
    pub const FLINCH: usize = 0x120;
    pub const HEAD: usize = 0x160;
    pub const HOME: usize = 0x3f0;
    pub const START3: usize = 0x400;
    pub const LAST: usize = 0x401;
    pub const HOLD: usize = 0x402;
    pub const GUARD: usize = 0x404;
    pub const ALERT: usize = 0x406;
    pub const GROWL: usize = 0x407;
    pub const FLINCH_A: usize = 0x408;
    pub const TURN_V: usize = 0x410;
    pub const DASH_A: usize = 0x414;
    pub const DASH_V: usize = 0x418;
    pub const FALL: usize = 0x420;
    pub const PROP: usize = 0x424;
    pub const HEAD_REF: usize = 0x428;
    pub const PUSH: usize = 0x42c;
    pub const CUBOID: usize = 0x430;
    pub const PATH: usize = 0x434;
    pub const NODE: usize = 0x438;
    pub const WALKED: usize = 0x43a;
    pub const RANGE: usize = 0x43c;
    pub const FLINCH_T: usize = 0x448;
    pub const SIZE: usize = 0x450;
}

/// `SpawnBeamExplosion(0, 0, 2, 1, 100000, 1.5, 15, m, 0, joint 6, 10, 3, 4, −1, 1, 1, −1, 0)` (0x2ba504).
pub const BEAM: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 100000.0, scale: 1.5, light: 15.0, streaks: 10, sparks: 3, puffs: 4, debris: 1, sound: -1, shake: true };

fn st(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_st(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn prop(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, p::PROP) - 1).ok().filter(|&g| g < w.table.mobys.len()) }
/// `0x2ba3e0(m, seq, frame, ticks)`: `MobyAnimBlend` on it and the prop (`t` in ticks).
fn blend(w: &mut World, id: MobyId, seq: u8, frame: i32, t: i32) {
    let t = w.ticks(t);
    w.anim_blend(id, seq, frame, t);
    if let Some(g) = prop(w, id) { w.anim_blend(g, seq, frame, t); }
}
/// `0x2ba448(speed, m)`: the anim speed of it and the prop.
fn anim_speed(w: &mut World, id: MobyId, s: f32) {
    w.mm(id).anim.speed = s;
    if let Some(g) = prop(w, id) { w.mm(g).anim.speed = s; }
}
fn hold(w: &World, id: MobyId) -> i16 { c::pi16(w, id, p::HOLD) }
fn set_ticks16(w: &mut World, id: MobyId, o: usize, t: i32) {
    let t = w.ticks(t);
    c::set_pi16(w, id, o, t as i16);
}
fn gscale(w: &World, x: f32) -> f32 { sv::fl(w.svc.timing.scale(Pf::f(x))) }
fn heading_to(a: c::V, b: c::V) -> f32 { c::atan(b[0] - a[0], b[1] - a[1]) }
fn alerted(w: &World, id: MobyId) -> bool { c::pi32(w, id, p::LURE) != 0 || c::pu8(w, id, p::ALERT) != 0 }
fn range(w: &World, id: MobyId) -> f32 {
    let r = c::pf(w, id, p::RANGE);
    if alerted(w, id) { r + r } else { r }
}
/// `0x26d270(0.25, 1.5, 0.75, 50°, m, pos, to, 1)` on the moby's own position.
fn step(w: &mut World, id: MobyId, to: c::V) {
    let mut from = c::pos(w, id);
    let mut to = to;
    walker::move_ground(w, id, 0.25, 1.5, 0.75, f32::from_bits(0x3f5f_66f3), &mut from, &mut to, 1);
    let m = w.mm(id);
    m.position[0] = from[0];
    m.position[1] = from[1];
}

/// The facts of this tick.
struct Tick {
    t: target::Target,
    d: f32,
    heading: f32,
    kt: f32,
}

/// The flinch: −30° unless one runs, its timer `ticks(30)`.
fn flinch(w: &mut World, id: MobyId) {
    if c::pi32(w, id, p::FLINCH_T) < 1 { c::set_pf(w, id, p::FLINCH_A, f32::from_bits(0xbf06_0a92)); }
    let t = w.ticks(30);
    c::set_pi32(w, id, p::FLINCH_T, t);
}

/// Knocked down: `cmd` |= 5, seq 5 over `ticks(6)`, anim speed 2, 8.
fn knock_down(w: &mut World, id: MobyId) {
    w.mm(id).cmd |= 5;
    blend(w, id, 5, 0, 6);
    anim_speed(w, id, 2.0);
    set_st(w, id, 8);
}

/// `0x2ba470` and the rest of the death (module doc).
fn blow_up(w: &mut World, id: MobyId) {
    let j = w.joint_point(id, 6);
    fx::beam_explosion(w, &BEAM, Some(id), j);
    if let Some(g) = prop(w, id) { w.delete_moby(g); }
    set_death_bits(w, id, 0, -1);
    let (pos, rot) = (c::pos(w, id), w.m(id).rotation);
    for k in PIECES { fx::break_piece(w, id, k, pos, rot, 0, 0); }
    w.delete_moby(id);
}

/// The hits (module doc).
fn hits(w: &mut World, id: MobyId, k: &Tick) {
    c::dec_timer_pvar_s16(w, id, p::GUARD);
    let s = st(w, id);
    if s == 10 || s == 0 { return; }
    let hit = w.get_hit(id, 0x21_0000, false);
    let res = damage::resolve(w, id, hit, p::D, 0, 4);
    let Some(h) = hit else { return };
    if res.damage == 0.0 { return; }
    let me = c::pos(w, id);
    let from = match h.attacker.filter(|&a| Some(a) != w.hero_moby && a < w.table.mobys.len()) {
        Some(a) => w.m(a).position,
        None => super::hero_pos(w),
    };
    let mut heading = c::atan(me[0] - from[0], me[1] - from[1]);
    let kr = p::K;
    c::set_pi32(w, id, kr + knock::k::FLAGS, 9);
    c::set_pf(w, id, kr + knock::k::DRAG, DT2 * 12.0);
    c::set_pu8(w, id, kr + 0x3d, 0);
    c::set_pf(w, id, kr + knock::k::GRAVITY, DT2 * 35.0);
    c::set_pf(w, id, kr + knock::k::SPEED, DT * 7.0);
    let hp = c::pf(w, id, p::D) - res.damage;
    c::set_pf(w, id, p::D, hp);
    let reaction = if hp <= 0.0 { 1 } else { res.reaction };
    if c::pi16(w, id, p::GUARD) != 0 && reaction != 1 {
        flinch(w, id);
        c::set_pu8(w, id, p::F + 7, 0x78);
        flash::start(w, id, p::F);
        return;
    }
    let push = |w: &mut World, half: bool| {
        let v = 3.4 / gscale(w, 7.0);
        c::set_pf(w, id, p::PUSH, if half { v * 0.5 } else { v });
    };
    match reaction {
        1 => {
            c::set_pf(w, id, kr + knock::k::ZOFF, 1.0);
            c::set_pi32(w, id, kr + knock::k::RADIUS, 0x400);
            knock::ballistic(w, id, f32::from_bits(0x40b6_6666), 4.0, kr);
            let dir = h.dir.map(|x| f32::from_bits(x.0));
            let (mut sp, mut up) = (c::pf(w, id, kr + knock::k::SPEED), c::pf(w, id, kr + knock::k::UP));
            heading = knock::aim(dir, &mut sp, &mut up);
            c::set_pf(w, id, kr + knock::k::SPEED, sp);
            c::set_pf(w, id, kr + knock::k::UP, up);
            knock::start(w, id, kr, heading, 7, 1, 0);
            c::set_pf(w, id, kr + knock::k::KEY_APEX, 12.0);
            c::set_pf(w, id, kr + knock::k::KEY_LAND, 25.0);
            set_st(w, id, 10);
            c::set_pu8(w, id, p::F + 7, 0x78);
            flash::start(w, id, p::F);
            set_death_bits(w, id, 0, -1);
            return;
        }
        3 | 6 | 8 => {
            w.play_sound(0xb, 0, id);
            flinch(w, id);
            let by_ratchet = h.attacker.filter(|&a| a < w.table.mobys.len()).is_some_and(|a| w.m(a).o_class == 0);
            if by_ratchet || 1.0 < sv::fl(h.damage) {
                let spared = w.m(id).anim.seq_a == 4 && 17.0 <= k.kt && k.kt <= 20.0;
                if !spared { knock_down(w, id); }
            }
            push(w, true);
        }
        4 | 5 => {
            w.play_sound(0xb, 0, id);
            blend(w, id, 6, 2, 4);
            set_ticks16(w, id, p::HOLD, 40);
            set_st(w, id, 1);
            push(w, false);
        }
        7 => {
            w.play_sound(0xb, 0, id);
            flinch(w, id);
            knock_down(w, id);
            knock::start(w, id, kr, heading, 5, 1, 0);
            c::set_pf(w, id, kr + knock::k::KEY_LAND, 10.0);
            c::set_pf(w, id, kr + knock::k::KEY_APEX, 5.0);
            push(w, false);
        }
        _ => return,
    }
    c::set_pu8(w, id, p::F + 7, 0x78);
    flash::start(w, id, p::F);
    set_ticks16(w, id, p::GUARD, 70);
    let t = w.ticks(240);
    c::set_pu8(w, id, p::ALERT, t as u8);
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    let d = p::D;
    c::set_pf(w, id, d, 3.0);
    c::set_pu8(w, id, d + 8, 2);
    c::set_pf(w, id, d + 0x10, 1.5);
    c::set_pi16(w, id, d + 4, 3);
    c::set_pu8(w, id, d + 0xe, 1);
    c::set_pu8(w, id, d + 9, 0);
    w.mm(id).cmd = 0;
    c::set_pf(w, id, p::FALL, 0.0);
    c::set_pi32(w, id, p::PROP, 0);
    c::set_pu8(w, id, p::GROWL, 0);
    let pos = c::pos(w, id);
    c::set_pv4(w, id, p::HOME, pos);
    if !manip::attached(w, id, p::FLINCH) { manip::attach(w, id, 0, id, p::FLINCH); }
    manip::attach(w, id, 0xc, id, p::HEAD);
    let y = w.m(id).rotation[2];
    c::set_pf(w, id, p::HEAD_REF, y);
    c::set_pu8(w, id, 0x58, 10);
    c::set_pu8(w, id, 0x5a, 0xc);
    if c::pu8(w, id, p::START3) == 0 {
        set_st(w, id, 1);
    } else {
        set_st(w, id, 3);
        blend(w, id, 10, 0, 12);
    }
}

/// The hold-out checks of states 1 and 3: within 4.5 → the swing; in range, not yet roused → seq 2, 2.
fn rouse(w: &mut World, id: MobyId, k: &Tick) {
    if hold(w, id) != 0 { return; }
    if k.d < 4.5 {
        blend(w, id, 4, 4, 12);
        set_st(w, id, 7);
    } else if k.d < range(w, id) && w.m(id).cmd & 1 == 0 {
        blend(w, id, 2, 0, 12);
        set_st(w, id, 2);
        w.mm(id).cmd |= 1;
    }
}

/// Back to 1 with seq 13 in range, else 0.
fn settle(w: &mut World, id: MobyId, k: &Tick, t: i32) {
    let s = if range(w, id) <= k.d { 0 } else { 0xd };
    blend(w, id, s, 0, t);
    set_st(w, id, 1);
}

/// State 1 (module doc).
fn idle(w: &mut World, id: MobyId, k: &Tick) {
    if w.m(id).anim.seq_b == 6 && hold(w, id) as i32 == w.ticks(0x25) { w.play_sound(2, 0, id); }
    let in_range = k.d < range(w, id);
    if wrapped(w, id) && range(w, id) < k.d {
        let mut table = [0u8; 6];
        table[5] = 1;
        let i = w.rng.randi(6) as usize;
        blend(w, id, table[i], 0, 24);
    } else {
        let s = w.m(id).anim.seq_b;
        if s != 0xd && !(s == 6 && !wrapped(w, id)) && in_range { blend(w, id, 0xd, 0, 24); }
    }
    let (cub, path) = (c::pi32(w, id, p::CUBOID), c::pi32(w, id, p::PATH));
    if cub != -1 && path != -1 && c::pi16(w, id, p::WALKED) == 0 {
        let h = super::hero_pos(w);
        if alerted(w, id) || w.in_cuboid([h[0], h[1], h[2]], cub) {
            set_st(w, id, 4);
            w.mm(id).cmd |= 1;
            c::set_pi16(w, id, p::WALKED, 1);
            return;
        }
    }
    if w.m(id).anim.seq_a == 0xd {
        let mut g = c::pu8(w, id, p::GROWL);
        let out = sv::fast_dec_timer_u8(&mut g) != 0;
        c::set_pu8(w, id, p::GROWL, g);
        if out {
            w.play_sound(10, 0, id);
            let t = w.ticks(0xb4);
            c::set_pu8(w, id, p::GROWL, t as u8);
        }
    }
    rouse(w, id, k);
}

/// State 4 (module doc).
fn walk(w: &mut World, id: MobyId, k: &Tick) {
    if w.m(id).anim.seq_b != 3 { blend(w, id, 3, 0, 12); }
    if k.d < 4.5 {
        blend(w, id, 4, 4, 12);
        set_st(w, id, 7);
    }
    let Some(path) = usize::try_from(c::pi32(w, id, p::PATH)).ok().filter(|&s| s < w.svc.splines.len()) else { return };
    let n = w.svc.splines[path].len() as i16;
    let node = c::pi16(w, id, p::NODE);
    let q = |w: &World, i: i16| usize::try_from(i).ok().and_then(|i| w.svc.splines[path].get(i)).map(|q| q.map(f32::from_bits)).unwrap_or([0.0; 4]);
    if c::dist2(q(w, node), c::pos(w, id)) < 1.0 {
        let next = node + 1;
        c::set_pi16(w, id, p::NODE, next);
        if next == n {
            let s = if range(w, id) < k.d { 0 } else { 0xd };
            blend(w, id, s, 0, 24);
            return set_st(w, id, 1);
        }
    }
    let at = q(w, c::pi16(w, id, p::NODE));
    let a = heading_to(c::pos(w, id), at);
    let mut v = c::pf(w, id, p::TURN_V);
    let y = turn::spring_turn(w.m(id).rotation[2], a, DT2 * 4.0 * PI, DT2 * 2.0 * PI, DT * PI, &mut v);
    c::set_pf(w, id, p::TURN_V, v);
    w.mm(id).rotation[2] = y;
    let me = c::pos(w, id);
    step(w, id, [me[0] + y.cos() * DT * 6.0, me[1] + y.sin() * DT * 6.0, me[2], me[3]]);
}

/// The prop at joint 11 / 13 (module doc).
fn carry(w: &mut World, id: MobyId) {
    let Some(g) = prop(w, id) else {
        let at = w.joint_point(id, 0xb);
        let Some(g) = w.create_moby(PROP) else { return };
        let (dd, yaw, light, ambient) = { let m = w.m(id); (m.draw_dist, m.rotation[2], m.light, m.ambient) };
        let m = w.mm(g);
        m.update_dist = 0xff;
        m.visible = 1;
        m.draw_dist = dd;
        m.rotation = [0.0, 0.0, yaw, 0.0];
        m.light = light;
        m.ambient = ambient;
        m.position = at;
        w.build_matrix(g);
        c::set_pi32(w, id, p::PROP, g as i32 + 1);
        return;
    };
    let mtx = w.joint_matrix(id, 0xd);
    w.mm(g).position = mtx[3];
    crate::moby_update::anim_sound::advance(w, g);
    let mut rows = [mtx[0].map(f32::to_bits), mtx[1].map(f32::to_bits), mtx[2].map(f32::to_bits)];
    rc_formats::moby_anim::normalise_columns(&mut rows);
    {
        let m = w.mm(g);
        for (row, r) in m.rows.iter_mut().zip(rows) { *row = r.map(f32::from_bits); }
    }
    react::sphere_lerp(w, g);
    w.mm(g).mode |= 6;
}

/// Level04 0x2ba520 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < p::SIZE { return; }
    manip::big_head(w, 2.5, id, 0, id, p::FLINCH);
    let cam = w.camera.map(|x| x.to_f32());
    if w.m(id).visible != 0 && c::dist3(c::pos(w, id), cam) < 27.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x15;
    }
    let t = target::acquire(w, id, range(w, id));
    let yaw0 = w.m(id).rotation[2];
    let me = c::pos(w, id);
    let k = Tick { d: c::dist2(me, t.pos), heading: heading_to(me, t.pos), kt: c::ground::key_time(w, id), t };
    if w.svc.game_mode == 2 {
        w.mm(id).mode |= 0x41;
        if let Some(g) = prop(w, id) { w.mm(g).mode |= 0x41; }
    } else if w.m(id).mode & mode::HIDDEN != 0 {
        w.mm(id).mode &= !0x41;
        if let Some(g) = prop(w, id) { w.mm(g).mode &= !0x41; }
    }
    if w.m(id).visible != 0 && c::dist3(c::pos(w, id), cam) < 32.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x1a;
    }
    hits(w, id, &k);
    if w.m(id).state >= 0xfd { return; }
    w.mm(id).hit_slot = 0xff;
    if c::pi32(w, id, p::LURE) != 0 {
        let t = w.ticks(240);
        c::set_pu8(w, id, p::ALERT, t as u8);
    }
    c::set_pi32(w, id, p::LURE, 0);
    let mut al = c::pu8(w, id, p::ALERT);
    sv::fast_dec_timer_u8(&mut al);
    c::set_pu8(w, id, p::ALERT, al);
    if st(w, id) != 0 && c::pu8(w, id, p::D + 0xe) != 1 {
        if let Some(g) = prop(w, id) { w.delete_moby(g); }
        w.delete_moby(id);
        return;
    }
    c::dec_timer_pvar_s16(w, id, p::HOLD);
    match st(w, id) {
        0 => init(w, id),
        1 => idle(w, id, &k),
        2 => {
            if w.m(id).anim.seq_a == 2 && wrapped(w, id) { settle(w, id, &k, 12); }
        }
        3 => {
            if w.m(id).anim.seq_a != 0 && wrapped(w, id) { blend(w, id, 0xb, 0, 12); }
            rouse(w, id, &k);
        }
        4 => walk(w, id, &k),
        7 => {
            let a = &w.m(id).anim;
            let dz = (w.m(id).position[2] - k.t.pos[2]).abs();
            if a.seq_a == a.seq_b && 17.0 <= k.kt && k.kt < 20.0 && dz < 2.0 && k.d < 4.5 {
                if let Some(t) = k.t.moby {
                    let push = [k.heading.cos() * 0.2, k.heading.sin() * 0.2, 0.0, 0.0];
                    let at = [k.t.pos[0], k.t.pos[1], k.t.pos[2] + 0.75, k.t.pos[3]];
                    attack::hit_moby(w, t, id, 1.0, 1, at, push);
                }
            }
            let sp = w.m(id).anim.speed;
            if 20.0 < k.kt && 1.0 < sp { anim_speed(w, id, (sp - DT * 3.0).max(1.0)); }
            if w.m(id).anim.seq_a == 4 && wrapped(w, id) {
                set_ticks16(w, id, p::HOLD, 0x5a);
                settle(w, id, &k, 12);
            }
        }
        8 => {
            let a = &w.m(id).anim;
            let getting_up = a.seq_b == 5 && (a.seq_a != 5 || k.kt <= 13.0);
            if getting_up {
                // Still down.
            } else if w.m(id).cmd & 4 != 0 {
                w.mm(id).cmd &= !4;
                blend(w, id, 4, 4, 6);
                anim_speed(w, id, 1.5);
                set_st(w, id, 7);
            } else {
                // The dash of the unreachable walker states (speed +0x418 is 0 on every reachable path).
                let v = c::pf(w, id, p::DASH_V);
                if v <= 0.0 {
                    set_st(w, id, 1);
                    c::set_pi16(w, id, p::HOLD, 0);
                } else {
                    let v = v - DT2 * 200.0;
                    c::set_pf(w, id, p::DASH_V, v);
                    let mv = fx::polar(v, c::pf(w, id, p::DASH_A), w.m(id).rotation[1]);
                    let pos = c::add(c::pos(w, id), mv);
                    c::set_pos(w, id, pos);
                    let mut tv = c::pf(w, id, p::TURN_V);
                    let y = turn::spring_turn(w.m(id).rotation[2], k.heading, DT2 * PI, DT2 * 2.0 * PI, DT * 4.188_790_3, &mut tv);
                    c::set_pf(w, id, p::TURN_V, tv);
                    w.mm(id).rotation[2] = y;
                }
            }
        }
        10 if knock::update(w, id, p::K) & 0x40 != 0 || w.m(id).position[2] < 1.0 => return blow_up(w, id),
        _ => {}
    }
    flash::update(w, id, p::F);
    let push = c::pf(w, id, p::PUSH);
    if 0.0 < push {
        let me = c::pos(w, id);
        let h = super::hero_pos(w);
        let a = c::atan(me[0] - h[0], me[1] - h[1]);
        let to = [me[0] + a.cos() * push, me[1] + a.sin() * push, me[2], me[3]];
        if c::pf(w, id, p::FALL) == 0.0 {
            let s = gscale(w, 7.0);
            c::set_pf(w, id, p::PUSH, push + -3.4 / (s * s));
        }
        step(w, id, to);
    }
    let fa = c::pf(w, id, p::FLINCH_A) * 0.96;
    c::set_pf(w, id, p::FLINCH_A, fa);
    c::dec_timer_pvar_i32(w, id, p::FLINCH_T);
    manip::set_axis(w, id, id, p::FLINCH, fa, 1);
    if st(w, id) != 8 {
        let s = st(w, id);
        c::set_pu8(w, id, p::LAST, s);
    }
    let v = c::pf(w, id, p::FALL) + DT2 * 29.8;
    c::set_pf(w, id, p::FALL, v);
    w.mm(id).position[2] -= v - 2.0;
    let gz = c::ground::ground(w, c::pos(w, id), 0.5, 0).z;
    let z = w.m(id).position[2] - 2.0;
    w.mm(id).position[2] = z;
    if z < gz {
        w.mm(id).position[2] = gz;
        c::set_pf(w, id, p::FALL, 0.0);
    }
    if w.m(id).position[2] < c::pf(w, id, p::HOME + 8) - 8.0 {
        // `0x2ba470`, the prop and itself deleted (no death bits, no pieces).
        let j = w.joint_point(id, 6);
        fx::beam_explosion(w, &BEAM, Some(id), j);
        if let Some(g) = prop(w, id) { w.delete_moby(g); }
        return w.delete_moby(id);
    }
    if st(w, id) != 0 {
        let turned = c::sub_rot(w.m(id).rotation[2], yaw0);
        let r = c::add_rot(c::pf(w, id, p::HEAD_REF), turned);
        let q = crate::hero::idle::axis_quat(c::sub_rot(k.heading, r), 2);
        c::set_pf(w, id, p::HEAD_REF, k.heading);
        let node = manip::node(w, id, p::HEAD).quat;
        manip::set_quat(w, id, id, p::HEAD, rc_formats::moby_anim::quat_product(node, q));
    }
    carry(w, id);
}
