//! U129 (census 2026-09-28; U131 in G-ENM-011's wording): class 573, Kerwan's small charging creatures (level03
//! `0x2c5bb8`, the only copy: 87 placed). The name is descriptive [L]. A ground creature of the shared layer (header:
//! damage record +0x20, flash +0x110, knockback +0x120, suck record +0x60, walker +0x180) that waits at its home
//! (+0x200, its placed position), notices Ratchet within its range of home, turns to him and **charges** (its walker
//! driven at its speed +0x234 along its facing), **bites** him at key 25 of its bite (damage 1, a 0.2 push), and runs
//! back home when he leaves the range. Some wait beside a trooper 574 (+0x248: the trooper's +0x284 names the same
//! 573) until the trooper waves them on (its sequence 10 past key 10, its knock / death flight, or its 573 link
//! dropped); some run a path (+0x270) when their group is alerted, Ratchet enters a cuboid (+0x27c) or comes within
//! +0x274 of them, and charge from its end. Any hit kills it: a death flight, on landing a death explosion, two body
//! pieces 0x6ce / 0x6cf and the burst of 0x77d pieces (`BreakFxBurst`), `SetDeathBits` (the bolts) and `DeleteMoby`;
//! the hit also alerts its group (+0xbc = 1) and, while running a path, its state 5 alerts the troopers of its group
//! (`kerwan_trooper::group_alerted`).
//!
//! **Pvars** (0x2c0): +0x20 damage record (health 1, meter +0x24 = 1, +0x29 0, +0x38 the lure), +0x40 the walk's
//! output move, +0x58 / +0x5a bytes 8 / 7, +0x60 the suck record (its +0x70 the sequence table gp−0x5330: the class
//! keeps the default reaction table, so the Suck Cannon never takes it), +0x110 the flash, +0x120 the knockback record,
//! +0x180 the walker J, +0x200 home, +0x220 the turn velocity, +0x228 (30, not read), +0x22c the sight range, +0x230
//! this tick's range (doubled for `ticks(240)` after an alert), +0x234 the speed (units a second), +0x23c the charge's
//! re-roll timer, +0x240 the alert timer, +0x244 the "don't bite yet" timer, +0x248 the trooper it waits by, +0x260 /
//! +0x264 / +0x26c (0, 1.0, 0 at init; not read), +0x270 the run path, +0x274 the path's trigger range, +0x278 its node,
//! +0x27c its trigger cuboid, +0x280 the big-head manipulator.
//!
//! ## Coverage
//!
//! **The update** `0x2c5bb8`:
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `0x24e830(+0x230, m, &t)` (= `0x274b78`) | the target (Ratchet or a decoy) | [`update`] (`target::acquire`) |
//! | `0x251d00(2.1, m, 0, +0x280)` | the big-head cheat (0x15edb7): attach a manipulator at 2.1; off: detach it if attached (never attached without the cheat) | [`update`] (`manip::big_head`) |
//! | `0x2c6a20` | the hits | [`hits`] |
//! | `FastDecTimer(+0x244)` | | [`update`] |
//! | 0: mode \|= 0x1000; home = pos; +0x7f = 0x18; 1; health 1.0, +0x29 0, alert 0, meter 1; `SeedJumpPattern(J)`; J+0x40 4π·dt², J+0x44 π·dt, J+8 2, J+0 radius 0x38d, J+0x24 speed·dt, J flags \|= 0x20, J+0x28 2.0954, J+0xc 2, J+0x3c 2π·dt², +0x264 1.0, +0x244 0, +0x26c 0, +0x260 0, J+0x2c = J+0x24·scale(6), +0x58 8, +0x5a 7; `rand() & 1` → mode \|= 0x8000 (mirrored) | init | [`init`] |
//! | 0: +0xd0 (suck record +0x70) = gp−0x5330 (the sequence table) | | n/a (the default reaction table: the record is never read) |
//! | 0: a trooper and no path → seq 0 (0 ticks) unless on it, state 2; else seq 1 (0 ticks) unless on it | | [`init`] |
//! | 1, path: at its point 0 facing point 1; `randi(14)` = 0 and the group alerted (`0x2c6d98`) → node 0, 5; else Ratchet in the cuboid (`0x24e3a8`), or within +0x274 (3-D) and 4 in height → node 0, 5 (seq 3, `ticks(10)`, unless on it) | | [`idle`] |
//! | 1, no path: `randi(14)` ≠ 0 → nothing; Ratchet within the range of home (3-D) and 3 in height → 3, seq 2 (`ticks(10)`), anim speed 0.878 | notice | [`idle`] |
//! | 2: `randi(14)` ≠ 0 → nothing; a live trooper link not ready (`0x2c6ca0`) → nothing; else +0x23c `ticks(trunc(randf(30, 90)))`, 4, seq 3 | waiting by a trooper | [`update`] |
//! | 3: `SpringTurn2(atan(t − pos), 0.02, 0.3, 0.1)`; anim done → the re-roll timer, 4, seq 3 | turn to Ratchet | [`update`] |
//! | 4: `SpringTurn2(…, 0.05, 0.3, 0.2)`; the walker step toward 2·(cos, sin) of the yaw (`0x247738` = `0x26d9a8`); `FastDecTimer(+0x23c)` out → re-rolled (`randf(30, 90)`); xy distance to Ratchet < r·0.5·scale(40) + r·scale(10) + 1.5 (r = speed·dt) → 6, seq 5 over `trunc(scale(10))` ticks; else Ratchet beyond the range of home (3-D) or 3 in height → 7 | charge | [`charge`] |
//! | 5: the path's nearest segment (`0x24c9b0`, 1000 / 5) at least the node; `0x247c10` (= `0x26de80`) walk to the node (out +0x40); arrived: next node, or at the end: path −1, 1 (seq 1, `ticks(10)`), and with a trooper seq 0, 2; then as 2 (`randi(14)`, the trooper's readiness) → path −1, re-roll timer, 4, seq 3 | run the path | [`run_path`] |
//! | 6: `SpringTurn2(…, 0.05, 0.3, 0.2)`; the walker step (out +0x40); in seq 5: J+0x24 `Approach`ed to 0 by speed·dt / scale(40) (`0x24a2b0`); key time within 0.25 of 25, 1 in height, facing within 15°, a target moby, xy distance < 2 → `0x248630` (= `0x26eaa8`): damage 1 to the target, flags 1, at the target + 0.75 z, push (0.2 cos, 0.2 sin, 0) | bite | [`bite`] (`attack::hit_moby`) |
//! | 6: anim done → J+0x24 = speed·dt, 7, seq 3 (`ticks(3)`), +0x244 `ticks(120)` | | [`bite`] |
//! | 7: `SpringTurn2(atan(home − pos), 0.02, 0.3, 0.1)`; the walker step; within 1 (xy) of home → 1, seq 1 (`ticks(3)`); else +0x244 out and the bite distance → 6, seq 5 | return home | [`update`] |
//! | 8: `0x24b0e0` (= `0x271558`) the flight; landed / wrapped → 4, seq 3; else z < 2 → `SetDeathBits(m, 0, −1)`, `DeleteMoby` | knocked (no setter on the disc) | [`update`] (`knock::update`) |
//! | 9: `0x2d7d28` (= `0x305260`) the Suck Cannon's carried update; done → 0, suck record state 0 | held (no setter: the default reaction table) | [`update`] (`react::carried`) |
//! | 0x15: mode &= ~0x1000; the flight; landed / wrapped → `0x24dad8(0.5, 13, m, pos, −1)` (= `0x273f50`, the death explosion), `BreakFxB` 0x6ce / 0x6cf at pos / rot (`0x2520b0`), `0x251f08(m, 0x77d, 1, 0x77d, 1, 3, 2)` (`BreakFxBurst`), `SetDeathBits(m, 0, −1)` (`0x245fe0`), `DeleteMoby`; else z < 2 → `SetDeathBits`, `DeleteMoby` | death flight | [`dying`] (`fx::death_explosion`, `fx::break_piece`, `breakables::burst_pieces`, `crate_::set_death_bits`) |
//! | tail: drawn and within 32 (3-D) of the camera (0x166ec0) → `0x248ba8` (= `0x26f020`) | the shadow probe | [`tail`] (`shadows::probe_down`) |
//! | tail: `0x24bf80` (= `0x2723f8`) the flash; `cmd` (+0xbc) = 0 | | [`tail`] |
//!
//! **The hits** `0x2c6a20`: the lure (+0x38 ≠ 0) or a ready trooper → the alert timer `ticks(240)`; the range +0x230 =
//! 2·+0x22c while the alert runs, else +0x22c; `MobyGetHitMessage(m, 0x330000, 0)`, the resolver (`0x248f00` =
//! `0x26f378`, column 4); out5 ≠ 1 and not in 0x15: the group's `cmd` = 1 (`0x247da0` = `0x26e090`), K flags 9,
//! +0x15d 0, health −= damage, K gravity 50·dt², mode &= ~0x1000, K up 10·dt (overwritten next), `0x250a78(4.5, 2, K)`
//! (the ballistic arc: up √(2·2·g), speed 2·4.5 / t, drag 2·4.5 / t², t = 2·up / g), keys 5 / 12, `0x2495d0` the aim
//! along the push (the heading from the attacker is a dead store), `0x24afa0` (= `0x271418`) the flight (seq 4, 1
//! tick, frame 0), 0x15, flash colour 0xf0, `0x24bea0` (= `0x272318`) the flash. Always +0xa4 = 0xff. Any hit that gets
//! through kills it (the health is not tested). [`hits`].
//!
//! **The trooper's readiness** `0x2c6ca0(m)` (573's own code): a live 574 whose sequence (+0x52) is 10 at key ≥ 10, or
//! in state 10 / 13, or whose 573 link (+0x284) is −1. [`trooper_ready`]. **Called by the troopers**: `0x2c5b78` (a
//! live 573 in state 5, `kerwan_trooper::group_alerted`).
//!
//! **The game's, noted:** states 8 and 9 have no setter on level 3 (the class's reaction table is the default one, so
//! the Suck Cannon never pulls it); the charge passes a direction (2·(cos, sin)) as the walker's target, as 577 does.
//! The run path's nearest-point search reads its output uninitialised when it fails in the game (the port keeps the
//! node) [L].

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, damage, flash, fx, knock, react, target, turn, walker};
use crate::moby_update::services::World;
use crate::ps2v::Pf;

/// The update in the level03 class table.
pub const UPDATE_FN: u32 = 0x2c_5bb8;
pub const REFERENCE_LEVEL: u32 = 3;
pub const CLASSES: [i16; 1] = [573];
/// The trooper it waits by (0x23e).
pub const TROOPER: i16 = 574;
/// The body pieces (`BreakFxB`) and the burst's piece class.
pub const PIECES: [i16; 2] = [0x6ce, 0x6cf];
pub const BURST: i16 = 0x77d;

/// Pvar offsets (module doc).
pub mod pv {
    pub const D: usize = 0x20;
    pub const METER: usize = 0x24;
    pub const LURE: usize = 0x38;
    pub const MOVE: usize = 0x40;
    pub const SUCK: usize = 0x60;
    pub const F: usize = 0x110;
    pub const K: usize = 0x120;
    pub const J: usize = 0x180;
    pub const HOME: usize = 0x200;
    pub const TURN_V: usize = 0x220;
    pub const SIGHT_BASE: usize = 0x22c;
    pub const SIGHT: usize = 0x230;
    pub const SPEED: usize = 0x234;
    pub const REROLL: usize = 0x23c;
    pub const ALERT: usize = 0x240;
    pub const HOLD: usize = 0x244;
    pub const TROOPER: usize = 0x248;
    pub const W260: usize = 0x260;
    pub const W264: usize = 0x264;
    pub const W26C: usize = 0x26c;
    pub const PATH: usize = 0x270;
    pub const PATH_RANGE: usize = 0x274;
    pub const NODE: usize = 0x278;
    pub const CUBOID: usize = 0x27c;
    pub const SIZE: usize = 0x2c0;
}

/// The states (module doc).
pub mod st {
    pub const INIT: u8 = 0;
    pub const IDLE: u8 = 1;
    pub const WAIT: u8 = 2;
    pub const NOTICE: u8 = 3;
    pub const CHARGE: u8 = 4;
    pub const RUN: u8 = 5;
    pub const BITE: u8 = 6;
    pub const HOME: u8 = 7;
    pub const KNOCKED: u8 = 8;
    pub const HELD: u8 = 9;
    pub const DYING: u8 = 0x15;
}

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn done(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
/// `if (m+0x53 != seq) fun_00212f90(m, seq, 0, t)` (`t` already in ticks).
fn blend(w: &mut World, id: MobyId, seq: u8, t: i32) { c::blend_to(w, id, seq, 0, t); }
fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> { usize::try_from(c::pi32(w, id, o)).ok().filter(|&m| m < w.table.mobys.len()) }
fn alive(w: &World, m: MobyId) -> bool { let s = w.m(m).state; s != 0xfe && s != 0xfd }
fn spline(w: &World, o: i32) -> Option<usize> { usize::try_from(o).ok().filter(|&p| p < w.svc.splines.len()) }
fn point(w: &World, p: Option<usize>, i: i32) -> c::V {
    p.zip(usize::try_from(i).ok()).and_then(|(p, i)| w.svc.splines[p].get(i)).map(|q| q.map(f32::from_bits)).unwrap_or([0.0; 4])
}
fn heading_to(p: c::V, t: c::V) -> f32 { c::atan(t[0] - p[0], t[1] - p[1]) }
/// `multiply_global_scale(x)` (the timer scale, 1 on NTSC).
fn gscale(w: &World, x: f32) -> f32 { w.svc.timing.scale(Pf::f(x)).to_f32() }

/// `0x2c6ca0(trooper)`: the trooper lets its 573 go (module doc).
pub fn trooper_ready(w: &World, t: MobyId) -> bool {
    if !alive(w, t) || w.m(t).o_class != TROOPER { return false; }
    if w.m(t).anim.seq_a == 10 && 10.0 <= c::ground::key_time(w, t) { return true; }
    let s = w.m(t).state;
    s == 10 || s == 13 || (w.m(t).pvars.len() >= 0x288 && c::pi32(w, t, super::kerwan_trooper::pv::POD) == -1)
}

/// The trooper link (+0x248) is live and not yet ready: keep waiting.
fn held_by_trooper(w: &World, id: MobyId) -> bool { link(w, id, pv::TROOPER).is_some_and(|t| alive(w, t) && !trooper_ready(w, t)) }

/// `+0x23c = ticks(trunc(randf(30, 90)))`.
fn reroll(w: &mut World, id: MobyId) {
    let f = w.rng.randf(30.0, 90.0);
    let t = w.ticks(f as i32);
    c::set_pi32(w, id, pv::REROLL, t);
}

/// To 4 (the charge) with seq 3 over `ticks(10)` (`LAB_002c6850`).
fn to_charge(w: &mut World, id: MobyId) {
    reroll(w, id);
    set_state(w, id, st::CHARGE);
    let t = w.ticks(10);
    blend(w, id, 3, t);
}

/// The walker step toward `2·(cos yaw, sin yaw, 0)` (`0x247738(1, m, J, &dir, &out)`).
fn step(w: &mut World, id: MobyId) -> c::V {
    let (cy, sy) = c::cs(c::yaw(w, id));
    let mut out = [0.0; 4];
    walker::step(w, id, pv::J, 1.0, [cy + cy, sy + sy, 0.0, 0.0], &mut out);
    out
}

/// The bite distance: `r·0.5·scale(40) + r·scale(10) + 1.5`, r = speed·dt.
fn bite_reach(w: &World, id: MobyId) -> f32 {
    let r = c::pf(w, id, pv::SPEED) * c::DT;
    r * 0.5 * gscale(w, 40.0) + r * gscale(w, 10.0) + 1.5
}

/// To 6 with seq 5 over `trunc(scale(10))` ticks.
fn to_bite(w: &mut World, id: MobyId) {
    set_state(w, id, st::BITE);
    let t = gscale(w, 10.0) as i32;
    blend(w, id, 5, t);
}

/// `0x2c6a20`: the hits (module doc).
fn hits(w: &mut World, id: MobyId) {
    let ready = link(w, id, pv::TROOPER).is_some_and(|t| trooper_ready(w, t));
    if c::pi32(w, id, pv::LURE) != 0 || ready {
        let t = w.ticks(240);
        c::set_pi32(w, id, pv::ALERT, t);
    }
    let r = c::dec_timer_pvar_i32(w, id, pv::ALERT);
    let base = c::pf(w, id, pv::SIGHT_BASE);
    c::set_pf(w, id, pv::SIGHT, if r == 0 { base + base } else { base });
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if let Some(h) = res.hit.filter(|_| res.out5 != 1 && state(w, id) != st::DYING) {
        let g = w.m(id).group;
        if g != -1 { crate::moby_update::scheduler::group_cmd(w, g, 1); }
        let kr = pv::K;
        c::set_pi32(w, id, kr + knock::k::FLAGS, 9);
        c::set_pu8(w, id, kr + 0x3d, 0);
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        c::set_pf(w, id, kr + knock::k::GRAVITY, c::DT2 * 50.0);
        // `FastArcTan(pos − attacker)`: a dead store (the aim below overwrites it).
        w.mm(id).mode &= !mode::TARGETABLE;
        c::set_pf(w, id, kr + knock::k::UP, c::DT * 10.0);
        knock::ballistic(w, id, 4.5, 2.0, kr);
        c::set_pf(w, id, kr + knock::k::KEY_APEX, 5.0);
        c::set_pf(w, id, kr + knock::k::KEY_LAND, 12.0);
        let dir = h.dir.map(|x| f32::from_bits(x.0));
        let (mut sp, mut up) = (c::pf(w, id, kr + knock::k::SPEED), c::pf(w, id, kr + knock::k::UP));
        let a = knock::aim(dir, &mut sp, &mut up);
        c::set_pf(w, id, kr + knock::k::SPEED, sp);
        c::set_pf(w, id, kr + knock::k::UP, up);
        knock::start(w, id, kr, a, 4, 1, 0);
        set_state(w, id, st::DYING);
        c::set_pu8(w, id, pv::F + 7, 0xf0);
        flash::start(w, id, pv::F);
    }
    w.mm(id).hit_slot = 0xff;
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    w.mm(id).mode |= mode::TARGETABLE;
    let p = c::pos(w, id);
    c::set_pv4(w, id, pv::HOME, p);
    w.mm(id).b7f = 0x18;
    set_state(w, id, st::IDLE);
    c::set_pf(w, id, pv::D, 1.0);
    c::set_pu8(w, id, pv::D + 9, 0);
    c::set_pi32(w, id, pv::ALERT, 0);
    c::set_pi16(w, id, pv::METER, 1);
    walker::seed(&mut w.mm(id).pvars, pv::J);
    let j = pv::J;
    let sp = c::pf(w, id, pv::SPEED) * c::DT;
    c::set_pf(w, id, j + 0x40, c::DT2 * 12.566_371);
    c::set_pf(w, id, j + 0x44, c::DT * std::f32::consts::PI);
    c::set_pf(w, id, j + 8, 2.0);
    c::set_pi32(w, id, j, 0x38d);
    c::set_pf(w, id, j + 0x24, sp);
    let fl = c::pi32(w, id, j + 0x38) | 0x20;
    c::set_pi32(w, id, j + 0x38, fl);
    c::set_pf(w, id, j + 0x28, f32::from_bits(0x4006_1bba));
    c::set_pf(w, id, j + 0xc, 2.0);
    c::set_pf(w, id, j + 0x3c, c::DT2 * 6.283_185_5);
    let s6 = gscale(w, 6.0);
    c::set_pf(w, id, pv::W264, 1.0);
    c::set_pi32(w, id, pv::HOLD, 0);
    c::set_pi32(w, id, pv::W26C, 0);
    c::set_pi32(w, id, pv::W260, 0);
    c::set_pf(w, id, j + 0x2c, c::pf(w, id, j + 0x24) * s6);
    c::set_pu8(w, id, 0x58, 8);
    c::set_pu8(w, id, 0x5a, 7);
    if w.rng.rand() & 1 != 0 { w.mm(id).mode |= mode::MIRROR; }
    if c::pi32(w, id, pv::TROOPER) != -1 && c::pi32(w, id, pv::PATH) == -1 {
        blend(w, id, 0, 0);
        set_state(w, id, st::WAIT);
        return;
    }
    blend(w, id, 1, 0);
}

/// State 1 (module doc).
fn idle(w: &mut World, id: MobyId, t: &target::Target) {
    if let Some(p) = spline(w, c::pi32(w, id, pv::PATH)) {
        let (a, b) = (point(w, Some(p), 0), point(w, Some(p), 1));
        c::set_pos(w, id, a);
        c::set_yaw(w, id, heading_to(a, b));
        let group_go = w.rng.randi(14) == 0 && super::kerwan_trooper::group_alerted(w, w.m(id).group);
        if !group_go && !w.in_cuboid([t.pos[0], t.pos[1], t.pos[2]], c::pi32(w, id, pv::CUBOID)) {
            if c::pf(w, id, pv::PATH_RANGE) <= c::dist3(t.pos, c::pos(w, id)) { return; }
            if 4.0 <= (t.pos[2] - c::pos(w, id)[2]).abs() { return; }
        }
        c::set_pi32(w, id, pv::NODE, 0);
        set_state(w, id, st::RUN);
        let tk = w.ticks(10);
        blend(w, id, 3, tk);
        return;
    }
    if w.rng.randi(14) != 0 { return; }
    let home = c::pv4(w, id, pv::HOME);
    if c::dist3(t.pos, home) < c::pf(w, id, pv::SIGHT) && (t.pos[2] - c::pos(w, id)[2]).abs() < 3.0 {
        set_state(w, id, st::NOTICE);
        let tk = w.ticks(10);
        blend(w, id, 2, tk);
        w.mm(id).anim.speed = f32::from_bits(0x3f60_c7ce);
    }
}

/// State 4 (module doc).
fn charge(w: &mut World, id: MobyId, t: &target::Target) {
    let h = heading_to(c::pos(w, id), t.pos);
    turn::spring_turn2_pvar(w, id, h, 0.05, 0.3, 0.2, pv::TURN_V);
    step(w, id);
    if c::dec_timer_pvar_i32(w, id, pv::REROLL) != 0 { reroll(w, id); }
    if c::dist2(c::pos(w, id), t.pos) < bite_reach(w, id) {
        to_bite(w, id);
        return;
    }
    let home = c::pv4(w, id, pv::HOME);
    if c::pf(w, id, pv::SIGHT) < c::dist3(t.pos, home) || 3.0 < (t.pos[2] - c::pos(w, id)[2]).abs() { set_state(w, id, st::HOME); }
}

/// State 5 (module doc).
fn run_path(w: &mut World, id: MobyId) {
    let p = spline(w, c::pi32(w, id, pv::PATH));
    if let Some(pi) = p {
        let pts: Vec<[f32; 4]> = w.svc.splines[pi].iter().map(|q| q.map(f32::from_bits)).collect();
        let me = c::pos(w, id);
        if let Some((_, cur)) = crate::spline::nearest(&pts, false, 1000.0, 5.0, 0.0, [me[0], me[1], me[2]]) {
            if c::pi32(w, id, pv::NODE) < cur.seg { c::set_pi32(w, id, pv::NODE, cur.seg); }
        }
        let n = c::pi32(w, id, pv::NODE);
        let q = point(w, p, n);
        let mut out = [0.0; 4];
        let r = walker::walk_to(w, id, pv::J, q, &mut out);
        c::set_pv4(w, id, pv::MOVE, out);
        if r & 0x14 != 0 {
            if n < pts.len() as i32 - 1 {
                c::set_pi32(w, id, pv::NODE, n + 1);
            } else {
                c::set_pi32(w, id, pv::PATH, -1);
                set_state(w, id, st::IDLE);
                let tk = w.ticks(10);
                blend(w, id, 1, tk);
                if c::pi32(w, id, pv::TROOPER) != -1 {
                    let tk = w.ticks(10);
                    blend(w, id, 0, tk);
                    set_state(w, id, st::WAIT);
                }
            }
        }
    }
    if w.rng.randi(14) != 0 { return; }
    if c::pi32(w, id, pv::TROOPER) != -1 && held_by_trooper(w, id) { return; }
    c::set_pi32(w, id, pv::PATH, -1);
    to_charge(w, id);
}

/// State 6 (module doc).
fn bite(w: &mut World, id: MobyId, t: &target::Target) {
    let p = c::pos(w, id);
    turn::spring_turn2_pvar(w, id, heading_to(p, t.pos), 0.05, 0.3, 0.2, pv::TURN_V);
    let out = step(w, id);
    c::set_pv4(w, id, pv::MOVE, out);
    if w.m(id).anim.seq_a == 5 {
        let s = (c::pf(w, id, pv::SPEED) * c::DT) / gscale(w, 40.0);
        let mut v = c::pf(w, id, pv::J + 0x24);
        turn::approach(0.0, s, &mut v);
        c::set_pf(w, id, pv::J + 0x24, v);
    }
    let key = c::ground::key_time(w, id);
    let p = c::pos(w, id);
    if (key - 25.0).abs() <= 0.25 && (t.pos[2] - p[2]).abs() < 1.0 && c::diff_rots(heading_to(p, t.pos), c::yaw(w, id)) < 0.261_799_4 {
        if let Some(m) = t.moby {
            if c::dist2(p, t.pos) < 2.0 {
                let (cy, sy) = c::cs(c::yaw(w, id));
                let at = [t.pos[0], t.pos[1], t.pos[2] + 0.75, t.pos[3]];
                c::attack::hit_moby(w, m, id, 1.0, 1, at, [cy * 0.2, sy * 0.2, 0.0, 0.0]);
                return;
            }
        }
    }
    if !done(w, id) { return; }
    let sp = c::pf(w, id, pv::SPEED) * c::DT;
    c::set_pf(w, id, pv::J + 0x24, sp);
    set_state(w, id, st::HOME);
    let tk = w.ticks(3);
    blend(w, id, 3, tk);
    let t120 = w.ticks(120);
    c::set_pi32(w, id, pv::HOLD, t120);
}

/// The end of a flight below 2: `SetDeathBits(m, 0, −1)`, `DeleteMoby` (`LAB_002c6970`). True: deleted.
fn fell(w: &mut World, id: MobyId) -> bool {
    if 2.0 <= c::pos(w, id)[2] { return false; }
    set_death_bits(w, id, 0, -1);
    w.delete_moby(id);
    true
}

/// State 0x15 (module doc). True: deleted.
fn dying(w: &mut World, id: MobyId) -> bool {
    w.mm(id).mode &= !mode::TARGETABLE;
    let r = knock::update(w, id, pv::K);
    if r & 0x60 == 0 { return fell(w, id); }
    let (pos, rot) = (c::pos(w, id), w.m(id).rotation);
    fx::death_explosion(w, 0.5, 13.0, Some(id), pos, -1);
    for class in PIECES { fx::break_piece(w, id, class, pos, rot, 0, 0); }
    crate::moby_update::classes::breakables::burst_pieces(w, id, BURST, 1, BURST, 1, 3, 2);
    set_death_bits(w, id, 0, -1);
    w.delete_moby(id);
    true
}

/// The tail (module doc).
fn tail(w: &mut World, id: MobyId) {
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 32.0 { crate::shadows::probe_down(w, id); }
    }
    flash::update(w, id, pv::F);
    w.mm(id).cmd = 0;
}

/// `0x2c5bb8`.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    let range = c::pf(w, id, pv::SIGHT);
    let t = target::acquire(w, id, range);
    // 0x251d00(2.1, m, 0, +0x280): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 2.1, id, 0, id, 0x280);
    hits(w, id);
    c::dec_timer_pvar_i32(w, id, pv::HOLD);
    match state(w, id) {
        st::INIT => init(w, id),
        st::IDLE => idle(w, id, &t),
        st::WAIT => {
            if w.rng.randi(14) == 0 && !held_by_trooper(w, id) { to_charge(w, id); }
        }
        st::NOTICE => {
            let h = heading_to(c::pos(w, id), t.pos);
            turn::spring_turn2_pvar(w, id, h, 0.02, 0.3, 0.1, pv::TURN_V);
            if done(w, id) { to_charge(w, id); }
        }
        st::CHARGE => charge(w, id, &t),
        st::RUN => run_path(w, id),
        st::BITE => bite(w, id, &t),
        st::HOME => {
            let home = c::pv4(w, id, pv::HOME);
            turn::spring_turn2_pvar(w, id, heading_to(c::pos(w, id), home), 0.02, 0.3, 0.1, pv::TURN_V);
            step(w, id);
            if c::dist2(c::pos(w, id), home) < 1.0 {
                set_state(w, id, st::IDLE);
                let tk = w.ticks(3);
                blend(w, id, 1, tk);
            } else if c::pi32(w, id, pv::HOLD) == 0 && c::dist2(c::pos(w, id), t.pos) < bite_reach(w, id) {
                to_bite(w, id);
            }
        }
        st::KNOCKED => {
            if knock::update(w, id, pv::K) & 0x60 == 0 {
                if fell(w, id) { return; }
            } else {
                set_state(w, id, st::CHARGE);
                let tk = w.ticks(10);
                blend(w, id, 3, tk);
            }
        }
        st::HELD => {
            if react::carried(w, id, pv::K) != 0 {
                set_state(w, id, st::INIT);
                c::set_pi16(w, id, pv::SUCK + react::rec::STATE, 0);
            }
        }
        st::DYING if dying(w, id) => return,
        _ => {}
    }
    tail(w, id);
}
