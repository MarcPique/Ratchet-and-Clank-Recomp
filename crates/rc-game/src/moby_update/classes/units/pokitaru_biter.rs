//! U375 (census 2026-09-30; U371 / U359 in older runs): class 1246, Pokitaru's small biters (level11 `0x314318`: 126
//! created instances, the only copy). A ground creature of the shared layer (mode 0x20 header: damage record +0x20,
//! flash +0x110, knockback +0x120, suck record +0x60, walker +0x180) with three ways to live, chosen by its instance
//! data: **on the beach** (95 instances: graze about home inside its area path, go for a target in range, bite it with
//! a sphere every tick of state 5), **swimming** a path loop (+0x238 = 2: one instance, biting Ratchet when he is
//! near), **hopping ashore** from a path's first point to its second (+0x238 = 1: none on the disc) or **waiting for a
//! boat** (+0x254 ≥ 0, a class-1075 moby: 30 instances; hidden until the boat's state passes 1, then five at most jump
//! aboard, ride it and bite from it). Any damaging hit knocks it back (reaction 3–8; the wrench always) or kills it
//! (health 2: a death flight, `SetDeathBits`, the small explosion); a knocked one that lands in the water (below 222)
//! or outside its area is deleted. The Taunter alerts it (a chase of `randf(180, 240)` ticks, range + 64); the Suck
//! Cannon takes it in the states it walks in ([`react::POKITARU_1246`], held state 0x14).
//!
//! **Pvars** (0x2c0): +0x20 damage record (health, +0x24 the morph meter, +0x28 column, +0x29, +0x30), +0x38 the lure,
//! +0x40 the jump velocity (+0x48 its z), +0x58 / +0x5a (bytes 8 / 2), +0x60 the suck record (+0xc8 its state, +0xd0
//! its sequence table), +0x110 the flash, +0x120 the knockback record (+0x17c its water count), +0x180 the walker J,
//! +0x1d0 the target record (+0x210 the moby, the port stores moby + 1; +0x214 the kind), +0x220 home, +0x230 the area
//! path, +0x234 the swim / hop path, +0x238 the mode (0 / 1 / 2), +0x23c the range (20), +0x244 the alert timer,
//! +0x248 the range this tick, +0x24c the side offset, +0x250 the turn velocity, +0x254 the boat, +0x258 the boat's
//! boarding line (`randf(−3, 3)`), +0x25c the swim node, +0x260 the graze point, +0x280 the big-head cheat's.
//!
//! The level11 `$gp` words (gp = 0x166c00): gp−0x4940 the Suck Cannon sequence table (nine 0s), −0x4934 1 (the scale
//! factor and the bite sphere factor), −0x4930 20 (the flights' gravity ×dt²), −0x492c / −0x4928 6 / 6 (the knockback
//! up / out ×dt), −0x4924 / −0x4920 8 / 8 (the death flight's), −0x491c 12 (the bite range), −0x4918 30 (the side
//! offset, degrees), −0x4914 4 (the jump's xy speed ×dt), −0x4910 20 (its gravity ×dt²), −0x4904 6 (the walker's top
//! speed ×dt).
//!
//! ## Coverage
//!
//! **The tick** `0x315968` (before the states, when the state is not 0x14):
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | scale = class scale × 1; J+0x24 = 6·dt | | [`pre`] |
//! | boat ≥ 0, state ∉ {0, 9, 0xb, 0xc, 0xd, 0x11, 0x12}, z < boat z + 0.5: `0x284c28(0.5, 10, m, pos, −1)` (= `0x2742a8`), `DeleteMoby` | a boarder that fell into the sea | [`pre`] (`fx::piece_explosion`); the game runs the rest of the tick on the deleted moby (no visible effect), the port stops [L] |
//! | state 0xc, boat ≥ 0: `0x30a830(boat)` (its state 5) while not drawn → `DeleteMoby`; boat state > 1 → drawn (+0x31 = 1), shown (mode &= ~1), the class's collision | the boat arrives / leaves | [`pre`] |
//! | `MobyGetHitMessage(m, 0x330000, 0)` (0x280300), `0x280358(m, hit, +0x20, 0, …, col 4)` | every weapon's hit through the resolver | [`pre`] (`World::get_hit`, `damage::resolve`) |
//! | a hit (out5 ≠ 1) out of 0xb: health −= damage; reaction = 3 for an attacker of class 0x47, else the resolver's; health ≤ 0 → 1; K: radius 0x200, height 0.2, drag 0, gravity 20·dt², flags 9, +0x15d = 0 | | [`pre`] |
//! | 1 / 2: untargetable; keys 7.5 / 11; 8·dt / 8·dt; `0x280a28` aim; `0x2823f8(angle, m, K, 3, 1, 0)`; state 0xb (0x12 above 0xb); flash 0xf0; `SetDeathBits(m, 0, −1)` | the death flight, the save bit, the bolts | [`pre`] (`knock::start`, `crate_::set_death_bits`) |
//! | 3–8: keys 7.5 / 11; 6·dt / 6·dt; aim; start (3, 1, 0); state 9 (0x11 above 0xb); flash 0x78 | the knockback | [`pre`] |
//! | 9 / 10: flash 0xfa | | [`pre`] |
//! | `0x2832f8` flash start; +0xa4 = 0xff; `0x2833d8` flash update | the hit flash | [`pre`] (`flash::start`, `flash::update`) |
//! | alert counting: state 2 → 10 (blend 2), state 3 wrapped → 10 | the Taunter's chase | [`pre`] |
//! | lure +0x38 → alert = `ticks(randf(180, 240))`, lure 0; `FastDecTimer(+0x244)`; range = +0x23c (+64 while alerted) | | [`pre`] |
//! | `0x2854f8(range, m, +0x1d0, 0, 0, area path)` (= `0x274df8`); found: the distance from the path's second point (state 1) or the moby, over range or more than 3 below → kind 2 | the target search (decoys win) | [`pre`] (`target::acquire_in`) |
//! | no target moby → Ratchet's moby and position | | [`pre`] |
//!
//! **The update** `0x314318`:
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | state ≠ 0x14: record state 0, the tick | | [`update`] |
//! | `0x288f40(2.5, m, 0, +0x280)` (= `0x278720`) | the big-head cheat manipulator | [`update`] (`manip::big_head`) |
//! | drawn and within 28 of the camera: `0x280000` shadow probe, +0x7f = 0x16 | | [`update`] (`shadows::probe_down`) |
//! | 0: health 2, meter 2, column 1, +0x29 1, +0x30 0.25, +0x58 8, +0x5a 2, +0xd0 the sequence table, `SeedJumpPattern(J)`, J radius 0x200, 0.5 / 0.5 / 0.5, flag 0x10; `rand() & 1` → mirror; mode 2 → 0x13 (blend 4, `ticks(10)`), at the path's first point, node 1; mode 1 → 1 at the first point 1 lower, home its second; no boat → `GroundHeight(0.5, pos, 0x20)`, 2 (blend 0, 5), home; a boat → 0xc, `randf(−3, 3)`, hidden without collision | init | [`init`] |
//! | 1: turn to the path's second point (8.73·dt² / 12.57·dt); a target and `randi(9) == 0` → 7 (blend 0, 5), the jump: 4·dt in xy, `0x280ad0` lob (gravity −20·dt²) | the hop ashore | [`update`] (`knock::lob_up`) |
//! | 2: a target → side `randf(−30°, 30°)`, 4 (blend 1); none, wrapped, `randi(3) ≠ 0`, drawn within 32 of the camera → graze point home + `randf(1, 3)` along `rand_angle`, 3 (blend 1) | idle / graze | [`update`] |
//! | 3: [`walk`] to the graze point; a target → 4 (side); none, wrapped, `randi(3) == 0` → 2 (blend 0) | | [`update`] |
//! | 4: not wrapped or 12 or farther: walk to the target moby; else 5 (blend 2); lost → 6 | go for it | [`update`] |
//! | 5: walk; wrapped and farther than 12 → 4 (blend 1), lost → 6; every tick `0x27f830(0.6, 1, 1, m, pos + 0.333 z, 1, 0, 1, 0)` (= `0x26e830`) | the bite (Ratchet, a decoy, any moby in the sphere) | [`update`] (`attack::sphere_hit`) |
//! | 6: home within 1 → 2 (blend 0); lost: walk home; a target → 4 (side); wrapped out of sequence 1 → blend 1 (5) | go home | [`update`] |
//! | 7 / 8: pos += v, vz −= 20·dt²; falling below `GroundHeight(0.5, pos + 1 z, 0)`: 7 → on it, 2; 8 → 1 below it, 1 | the jump | [`update`] |
//! | 9 / 0x11: `0x282538(m, K)` (= `0x271558`) & 0x40: at 222 or higher inside the area path → 2 / 0xe (blend 0), done; else deleted; & 0x120 → deleted | the knockback's end | [`update`] (`knock::update`, `region::point_in_polygon`) |
//! | 0xb / 0x12: & 0x160 → `0x284c28(0.5, 10, m, pos, −1)`, deleted | the death | [`update`] (`fx::piece_explosion`) |
//! | 10: 2.5 toward the target inside the area path (`0x286d40` = `0x276640`): walk to it; else turn to the target moby (12.57·dt² / ·dt); wrapped: lost and the alert out → 6, a target → 4 | the chase | [`chase`] (`region::crosses`) |
//! | 0xc: a target, fewer than 5 of the group boarding (`0x316090`: class 1246 in 0xd..0x11), in the boat's frame x below +0x258 and above −3.1 → 0xd (blend 1, 5): a point 1.5–2 to its side (`randf`), 1 up, the lob (anim speed `ticks(60)`·4·dt / distance), facing the jump | board the boat | [`board`] |
//! | 0xd: pos += v + the boat's +0x70 (its velocity); vz −= 20·dt²; falling below the ground → anim speed 1, 0xe | the jump aboard | [`update`] |
//! | 0xe / 0xf / 0x10 / 0x11 / 0x12: `0x2859c0(m, boat, pos, rot, pos, rot)` (= `0x2752c0`) first | carried by the boat | [`update`] (`triggers::carrier` / `carried`) |
//! | 0xe: a target → 0xf (side, blend 1) | aboard | [`update`] |
//! | 0xf / 0x10: as 4 / 5 aboard (the bite's push 0) | | [`update`] |
//! | 0x13: to the node (z at most the boat's − 1), turn 12.57, `0x27e660(0.5, 0.5, 0, m, v, 0x10)` (= `0x26d610`: 2·dt ahead, z ±4·dt); within 1 → the next node (wraps); at a key (+0x54 = 0): sequence 5 → 6; 4 and Ratchet within 6 → 5; 6: beyond 8 → 7, within 4 → the bite `0x27f830(0.55, 1, 1, m, pos + 0.5 z, 1, 0, 1, 0)`; 7 → 4 | the swimmer | [`swim`] (`walker::move_collide`, `attack::sphere_hit`) |
//! | 0x14: record state ≠ 0: `0x305c88(m, K)` (= `0x305260`) done → the saved state (+0xbc), record state 0 | the Suck Cannon's carried update | [`update`] (`react::carried`) |
//! | tail: +0x17c (in the water) in 9 / 0xb / 0x11 / 0x12: three bubbles `0x294450(randf(2100, 14700), 222.5, pos + (±0.2, ±0.2, ±0.2 + 0.8), (0.2–1.3)·dt out, (−0.2–0.4)·dt up)` | a knocked biter in the sea | [`tail`] (`bomb_water::part34`) |
//!
//! **Walk** `0x315f58(m, point)`: turn to the point plus the side offset (8.73·dt² / 12.57·dt, `0x281ca0`); not
//! blending and the key time strictly between 8 and 22: J+0x24 = 6·dt and `0x27e9f8(1, m, J, 2·(cos yaw, sin yaw),
//! &out)` (= `0x26d9a8`; the direction as the target, as 577 / 572: `walker` module doc). [`walk`].
//!
//! **The reaction table** (level11 0x21c038: 0x316160, 0x316210, 0x316270, 0x3162d0, 0x3162f0 → pvar +0x60,
//! `DeleteMoby`): [`react::POKITARU_1246`].
//!
//! **The game's, noted:** the one swimmer (#569, path 51) has the tide water 1158 (#453) as its +0x254, and the tick's
//! sea test runs in its state 0x13 too: its path lies below the tide + 0.5, so it blows up on its first tick after the
//! init (the tide moves between 132.36 and 137; the port does the same). The boats 1075 are [`super::pokitaru_boat`]
//! (U363): they show the boarders once past state 1 and move the boarders' area path (+0x230 = the boat's +0xa8) with
//! them; only the commando 114 sets them off (U353, [`super::pokitaru_commando`]): until he starts a boat the 30
//! boarders stay hidden at sea.
//!
//! **Not the game's, noted [L]:** the deleted-boarder case above. Native `f32`; the rand draws at the game's points.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, attack, damage, flash, fx, ground, knock, react, region, target, turn, walker};
use crate::moby_update::services::World;

/// The update in the level11 class table.
pub const UPDATE_FN: u32 = 0x31_4318;
pub const REFERENCE_LEVEL: u32 = 11;
pub const CLASSES: [i16; 1] = [1246];
/// The boats' class.
pub const BOAT_CLASS: i16 = 1075;

/// Pvar offsets (module doc).
pub mod pv {
    pub const D: usize = 0x20;
    pub const LURE: usize = 0x38;
    pub const VEL: usize = 0x40;
    pub const VZ: usize = 0x48;
    pub const SUCK: usize = 0x60;
    pub const FLASH: usize = 0x110;
    pub const K: usize = 0x120;
    pub const J: usize = 0x180;
    pub const TGT: usize = 0x1d0;
    pub const TGT_MOBY: usize = 0x210;
    pub const TGT_KIND: usize = 0x214;
    pub const HOME: usize = 0x220;
    pub const AREA: usize = 0x230;
    pub const PATH: usize = 0x234;
    pub const MODE: usize = 0x238;
    pub const RANGE: usize = 0x23c;
    pub const ALERT: usize = 0x244;
    pub const RANGE_NOW: usize = 0x248;
    pub const SIDE: usize = 0x24c;
    pub const TURN_V: usize = 0x250;
    pub const BOAT: usize = 0x254;
    pub const BOARD_X: usize = 0x258;
    pub const NODE: usize = 0x25c;
    pub const GRAZE: usize = 0x260;
    pub const SIZE: usize = 0x2c0;
}

/// The level11 `$gp` words (module doc).
pub mod k {
    pub const SCALE: f32 = 1.0;
    pub const FLIGHT_G: f32 = 20.0;
    pub const KNOCK_UP: f32 = 6.0;
    pub const KNOCK_OUT: f32 = 6.0;
    pub const DEATH_UP: f32 = 8.0;
    pub const DEATH_OUT: f32 = 8.0;
    pub const BITE_RANGE: f32 = 12.0;
    pub const SIDE_DEG: f32 = 30.0;
    pub const JUMP: f32 = 4.0;
    pub const JUMP_G: f32 = 20.0;
    pub const WALK: f32 = 6.0;
    /// The sea's level (the knockback's landing test 222, the bubbles' 222.5).
    pub const SEA: f32 = 222.0;
    pub const BUBBLE_LEVEL: f32 = 222.5;
    /// 8.726646 / 12.566371 (the turn rates).
    pub const TURN_A: f32 = 8.726_646;
    pub const TURN_B: f32 = 12.566_371;
}

/// The states (module doc).
pub mod st {
    pub const INIT: u8 = 0;
    pub const HOP_WAIT: u8 = 1;
    pub const IDLE: u8 = 2;
    pub const GRAZE: u8 = 3;
    pub const GO: u8 = 4;
    pub const BITE: u8 = 5;
    pub const HOME: u8 = 6;
    pub const JUMP: u8 = 7;
    pub const JUMP_B: u8 = 8;
    pub const KNOCKED: u8 = 9;
    pub const CHASE: u8 = 10;
    pub const DYING: u8 = 0xb;
    pub const WAIT_BOAT: u8 = 0xc;
    pub const BOARD: u8 = 0xd;
    pub const ABOARD: u8 = 0xe;
    pub const ABOARD_GO: u8 = 0xf;
    pub const ABOARD_BITE: u8 = 0x10;
    pub const ABOARD_KNOCKED: u8 = 0x11;
    pub const ABOARD_DYING: u8 = 0x12;
    pub const SWIM: u8 = 0x13;
    pub const HELD: u8 = 0x14;
}

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
/// `if (m+0x53 != seq) fun_00212f90(m, seq, 0, ticks(n))`.
fn blend(w: &mut World, id: MobyId, seq: u8, n: i32) {
    let t = w.ticks(n);
    c::blend_to(w, id, seq, 0, t);
}
/// The same with a raw tick count (`fun_00212f90(m, seq, 0, 5)`).
fn blend_raw(w: &mut World, id: MobyId, seq: u8, t: i32) { c::blend_to(w, id, seq, 0, t); }
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn kind(w: &World, id: MobyId) -> i32 { c::pi32(w, id, pv::TGT_KIND) }
fn target_moby(w: &World, id: MobyId) -> Option<MobyId> {
    let v = c::pi32(w, id, pv::TGT_MOBY);
    (v > 0).then(|| (v - 1) as MobyId).filter(|&m| m < w.table.mobys.len())
}
fn path(w: &World, id: MobyId, o: usize) -> Option<usize> { usize::try_from(c::pi32(w, id, o)).ok().filter(|&p| p < w.svc.splines.len()) }
fn path_point(w: &World, p: usize, i: usize) -> c::V { w.svc.splines[p].get(i).map(|q| q.map(f32::from_bits)).unwrap_or([0.0; 4]) }
/// The boat (+0x254; −1 none).
fn boat(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, pv::BOAT)).ok().filter(|&b| b < w.table.mobys.len()) }
/// `randf(−30, 30)·π/180` into the side offset.
fn new_side(w: &mut World, id: MobyId) {
    let f = w.rng.randf(-k::SIDE_DEG, k::SIDE_DEG);
    c::set_pf(w, id, pv::SIDE, f * 0.017_453_292);
}
fn turn_to(w: &mut World, id: MobyId, to: c::V, a: f32, v: f32) {
    let p = c::pos(w, id);
    turn::turn_toward_pvar(w, id, c::atan(to[0] - p[0], to[1] - p[1]), c::DT2 * a, c::DT2 * a, c::DT * v, pv::TURN_V);
}

/// `0x274df8` into the record +0x1d0 (the record's layout: position, Euler, aim, body, moby, kind).
fn store_target(w: &mut World, id: MobyId, t: &target::Target) {
    c::set_pv4(w, id, pv::TGT, t.pos);
    c::set_pv4(w, id, pv::TGT + 0x10, t.rot);
    c::set_pv4(w, id, pv::TGT + 0x20, t.aim);
    c::set_pv4(w, id, pv::TGT + 0x30, t.body);
    c::set_pi32(w, id, pv::TGT_MOBY, t.moby.map_or(0, |m| m as i32 + 1));
    c::set_pi32(w, id, pv::TGT_KIND, t.kind as i32);
}

/// `0x315968`: the tick before the states (module doc). False: deleted.
fn pre(w: &mut World, id: MobyId) -> bool {
    let o = w.m(id).o_class;
    w.mm(id).scale = crate::moby_update::classes::units::class_scale(w, o) * k::SCALE;
    c::set_pf(w, id, pv::J + 0x24, k::WALK * c::DT);
    if let Some(b) = boat(w, id) {
        let s = state(w, id);
        if s == st::WAIT_BOAT {
            if w.m(b).state == 5 && w.m(id).visible == 0 {
                w.delete_moby(id);
                return false;
            }
            if 1 < w.m(b).state {
                let has = crate::moby_update::classes::units::class_collision(w, o);
                let m = w.mm(id);
                m.visible = 1;
                m.mode &= !mode::HIDDEN;
                m.has_collision = has;
            }
        } else if ![st::DYING, st::KNOCKED, st::ABOARD_DYING, st::ABOARD_KNOCKED, st::INIT, st::BOARD].contains(&s) && c::pos(w, id)[2] < c::pos(w, b)[2] + 0.5 {
            let p = c::pos(w, id);
            fx::piece_explosion(w, 0.5, 10.0, Some(id), p);
            w.delete_moby(id);
            return false;
        }
    }
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if res.out5 != 1 && state(w, id) != st::DYING {
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        let wrench = res.hit.and_then(|h| h.attacker).is_some_and(|a| w.m(a).o_class == 0x47);
        let mut reaction = if wrench { 3 } else { res.reaction };
        if hp <= 0.0 { reaction = 1; }
        let kr = pv::K;
        c::set_pi32(w, id, kr + knock::k::RADIUS, 0x200);
        c::set_pf(w, id, kr + knock::k::ZOFF, 0.2);
        c::set_pf(w, id, kr + knock::k::DRAG, 0.0);
        c::set_pf(w, id, kr + knock::k::GRAVITY, k::FLIGHT_G * c::DT2);
        c::set_pi32(w, id, kr + knock::k::FLAGS, 9);
        c::set_pu8(w, id, kr + 0x3d, 0);
        let dir = res.hit.map(|h| h.dir.map(|x| f32::from_bits(x.0))).unwrap_or([0.0; 4]);
        let fly = |w: &mut World, out: f32, up: f32| {
            c::set_pf(w, id, kr + knock::k::KEY_APEX, 7.5);
            c::set_pf(w, id, kr + knock::k::KEY_LAND, 11.0);
            let (mut sp, mut u) = (out * c::DT, up * c::DT);
            c::set_pf(w, id, kr + knock::k::SPEED, sp);
            c::set_pf(w, id, kr + knock::k::UP, u);
            let a = knock::aim(dir, &mut sp, &mut u);
            c::set_pf(w, id, kr + knock::k::SPEED, sp);
            c::set_pf(w, id, kr + knock::k::UP, u);
            knock::start(w, id, kr, a, 3, 1, 0);
        };
        match reaction {
            1 | 2 => {
                w.mm(id).mode &= !mode::TARGETABLE;
                fly(w, k::DEATH_OUT, k::DEATH_UP);
                let s = if st::DYING < state(w, id) { st::ABOARD_DYING } else { st::DYING };
                set_state(w, id, s);
                c::set_pu8(w, id, pv::FLASH + 7, 0xf0);
                set_death_bits(w, id, 0, -1);
            }
            3..=8 => {
                fly(w, k::KNOCK_OUT, k::KNOCK_UP);
                let s = if st::DYING < state(w, id) { st::ABOARD_KNOCKED } else { st::KNOCKED };
                set_state(w, id, s);
                c::set_pu8(w, id, pv::FLASH + 7, 0x78);
            }
            9 | 10 => c::set_pu8(w, id, pv::FLASH + 7, 0xfa),
            _ => {}
        }
        flash::start(w, id, pv::FLASH);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::FLASH);
    if c::pi32(w, id, pv::ALERT) != 0 {
        let s = state(w, id);
        if s == st::IDLE || (s == st::GRAZE && wrapped(w, id)) {
            set_state(w, id, st::CHASE);
            blend(w, id, 2, 10);
        }
    }
    if c::pi32(w, id, pv::LURE) != 0 {
        let f = w.rng.randf(180.0, 240.0);
        let t = w.svc.timing.scale(crate::ps2v::Pf::f(f)).to_i32();
        c::set_pi32(w, id, pv::ALERT, t);
        c::set_pi32(w, id, pv::LURE, 0);
    }
    c::dec_timer_pvar_i32(w, id, pv::ALERT);
    let mut range = c::pf(w, id, pv::RANGE);
    if c::pi32(w, id, pv::ALERT) != 0 { range += 64.0; }
    c::set_pf(w, id, pv::RANGE_NOW, range);
    let t = target::acquire_in(w, id, range, path(w, id, pv::AREA));
    store_target(w, id, &t);
    if t.kind != 2 {
        let from = if state(w, id) == st::HOP_WAIT { path(w, id, pv::PATH).map(|p| path_point(w, p, 1)).unwrap_or([0.0; 4]) } else { c::pos(w, id) };
        let tp = c::pv4(w, id, pv::TGT);
        if !(c::dist2(from, tp) <= range && from[2] - tp[2] <= 3.0) { c::set_pi32(w, id, pv::TGT_KIND, 2); }
    }
    if c::pi32(w, id, pv::TGT_MOBY) == 0 {
        let hm = w.hero_moby;
        c::set_pi32(w, id, pv::TGT_MOBY, hm.map_or(0, |m| m as i32 + 1));
        let hp = crate::moby_update::classes::units::hero_pos(w);
        c::set_pv4(w, id, pv::TGT, hp);
    }
    true
}

/// `0x315f58(m, point)`: one walking tick toward `point` (module doc).
fn walk(w: &mut World, id: MobyId, point: c::V) {
    let p = c::pos(w, id);
    let h = c::add_rot(c::atan(point[0] - p[0], point[1] - p[1]), c::pf(w, id, pv::SIDE));
    turn::turn_toward_pvar(w, id, h, c::DT2 * k::TURN_A, c::DT2 * k::TURN_A, c::DT * k::TURN_B, pv::TURN_V);
    let kt = ground::key_time(w, id);
    let a = &w.m(id).anim;
    if a.seq_a == a.seq_b && 8.0 < kt && kt < 22.0 {
        c::set_pf(w, id, pv::J + 0x24, k::WALK * c::DT);
        let (cy, sy) = c::cs(c::yaw(w, id));
        let mut out = [0.0; 4];
        walker::step(w, id, pv::J, 1.0, [cy + cy, sy + sy, 0.0, 0.0], &mut out);
    }
}

/// State 0 (`0x314318` case 0).
fn init(w: &mut World, id: MobyId) {
    c::set_pf(w, id, pv::D, 2.0);
    c::set_pi16(w, id, pv::D + 4, 2);
    c::set_pu8(w, id, pv::D + 8, 1);
    c::set_pf(w, id, pv::D + 0x10, 0.25);
    c::set_pu8(w, id, 0x58, 8);
    c::set_pu8(w, id, pv::D + 9, 1);
    c::set_pu8(w, id, 0x5a, 2);
    // +0xd0 = gp−0x4940 (nine 0s): the table's default sequences ([`react::POKITARU_1246`]).
    walker::seed(&mut w.mm(id).pvars, pv::J);
    c::set_pf(w, id, pv::J + 0xc, 0.5);
    c::set_pf(w, id, pv::J + 8, 0.5);
    c::set_pf(w, id, pv::J + 4, 0.5);
    c::set_pi32(w, id, pv::J, 0x200);
    let f = c::pi32(w, id, pv::J + 0x38) | 0x10;
    c::set_pi32(w, id, pv::J + 0x38, f);
    if w.rng.rand() & 1 != 0 { w.mm(id).mode |= mode::MIRROR; }
    match c::pi32(w, id, pv::MODE) {
        2 => {
            set_state(w, id, st::SWIM);
            blend(w, id, 4, 10);
            let p0 = path(w, id, pv::PATH).map(|p| path_point(w, p, 0)).unwrap_or([0.0; 4]);
            c::set_pos(w, id, p0);
            c::set_pi32(w, id, pv::NODE, 1);
        }
        1 => {
            set_state(w, id, st::HOP_WAIT);
            let pa = path(w, id, pv::PATH);
            let mut p0 = pa.map(|p| path_point(w, p, 0)).unwrap_or([0.0; 4]);
            p0[2] -= 1.0;
            c::set_pos(w, id, p0);
            let p1 = pa.map(|p| path_point(w, p, 1)).unwrap_or([0.0; 4]);
            c::set_pv4(w, id, pv::HOME, p1);
        }
        _ if c::pi32(w, id, pv::BOAT) < 0 => {
            let mut p = c::pos(w, id);
            p[2] = ground::ground(w, p, 0.5, 0x20).z;
            c::set_pos(w, id, p);
            set_state(w, id, st::IDLE);
            blend_raw(w, id, 0, 5);
            c::set_pv4(w, id, pv::HOME, p);
        }
        _ => {
            set_state(w, id, st::WAIT_BOAT);
            let f = w.rng.randf(-3.0, 3.0);
            c::set_pf(w, id, pv::BOARD_X, f);
            let m = w.mm(id);
            m.visible = 0;
            m.has_collision = false;
            m.mode |= mode::HIDDEN;
        }
    }
}

/// `0x316090(m)`: the members of the moby's group of class 1246 in the boarding states 0xd..0x11.
fn boarding(w: &World, id: MobyId) -> i32 {
    let g = w.m(id).group;
    if g < 0 { return 0; }
    let Some(Some(list)) = w.svc.groups.lists.get(g as usize) else { return 0 };
    let mut n = 0;
    for &e in list {
        if let Some(m) = w.table.mobys.get((e & 0x7fff) as usize) {
            if m.o_class == CLASSES[0] && st::WAIT_BOAT < m.state && m.state < st::ABOARD_DYING { n += 1; }
        }
        if e & 0x8000 != 0 { break; }
    }
    n
}

/// State 0xc: wait for the boat and jump aboard (module doc).
fn board(w: &mut World, id: MobyId) {
    if kind(w, id) == 2 || 5 <= boarding(w, id) { return; }
    let Some(b) = boat(w, id) else { return };
    let rows = w.m(b).rows;
    let bp = c::pos(w, b);
    let v = c::sub(c::pos(w, id), bp);
    let l = [0, 1, 2].map(|i| v[0] * rows[i][0] + v[1] * rows[i][1] + v[2] * rows[i][2]);
    if c::pf(w, id, pv::BOARD_X) <= l[0] || l[0] <= -3.1 { return; }
    let side = if 0.0 < l[1] { w.rng.randf(1.5, 2.0) } else { w.rng.randf(-2.0, -1.5) };
    let loc = [l[0], side, 1.0];
    let mut t = [0.0f32; 4];
    for i in 0..3 { t[i] = loc[0] * rows[0][i] + loc[1] * rows[1][i] + loc[2] * rows[2][i] + bp[i]; }
    t[3] = bp[3];
    set_state(w, id, st::BOARD);
    blend_raw(w, id, 1, 5);
    let p = c::pos(w, id);
    let mut vel = c::sub(t, p);
    let n = w.ticks(60);
    let sp = k::JUMP * c::DT;
    w.mm(id).anim.speed = (n as f32 * sp) / c::len3(vel);
    vel = c::set_len2(vel, sp);
    c::set_pv4(w, id, pv::VEL, vel);
    let mut time = 0.0;
    let vz = knock::lob_up(sp, -(k::JUMP_G * c::DT2), p, t, &mut time);
    c::set_pf(w, id, pv::VZ, vz);
    c::set_yaw(w, id, c::atan(vel[0], vel[1]));
}

/// `FUN_002859c0(m, boat, pos, rot, pos, rot)`: carried by the boat this tick.
fn carry(w: &mut World, id: MobyId) {
    let Some(b) = boat(w, id) else { return };
    let Some(k) = crate::moby_update::triggers::carrier(w.m(b)) else { return };
    let p = c::pos(w, id);
    let r = w.m(id).rotation;
    let (q, rot) = crate::moby_update::triggers::carried(&k, [p[0], p[1], p[2]], [r[0], r[1], r[2]]);
    let m = w.mm(id);
    m.position = [q[0], q[1], q[2], p[3]];
    m.rotation = [rot[0], rot[1], rot[2], r[3]];
}

/// State 0x13: the swimmer (module doc).
fn swim(w: &mut World, id: MobyId) {
    let pa = path(w, id, pv::PATH);
    let node = c::pi32(w, id, pv::NODE);
    let mut t = pa.map(|p| path_point(w, p, node.max(0) as usize)).unwrap_or([0.0; 4]);
    if let Some(b) = boat(w, id) {
        let bz = c::pos(w, b)[2] - 1.0;
        if bz < t[2] { t[2] = bz; }
    }
    turn_to(w, id, t, k::TURN_B, k::TURN_B);
    let yaw = c::yaw(w, id);
    let lim = c::DT * 4.0;
    let dz = (t[2] - c::pos(w, id)[2]).clamp(-lim, lim);
    let mut v = [yaw.cos() * (c::DT + c::DT), yaw.sin() * (c::DT + c::DT), dz, 0.0];
    walker::move_collide(w, id, 0.5, 0.5, 0.0, &mut v, 0x10);
    if c::dist3(t, c::pos(w, id)) < 1.0 {
        let n = pa.map_or(0, |p| w.svc.splines[p].len()) as i32;
        // `% count`: a zero count traps (`break 7`) in the game; the port keeps the node.
        if n != 0 { c::set_pi32(w, id, pv::NODE, (node + 1) % n); }
    }
    if w.m(id).anim.t != 0.0 { return; }
    let hp = crate::moby_update::classes::units::hero_pos(w);
    let d = c::dist2(c::pos(w, id), hp);
    match w.m(id).anim.seq_b {
        5 => blend(w, id, 6, 10),
        4 if d < 6.0 => blend(w, id, 5, 10),
        6 if 8.0 < d => blend(w, id, 7, 10),
        6 if d < 4.0 => {
            let mut p = c::pos(w, id);
            p[2] += 0.5;
            attack::sphere_hit(w, k::SCALE * 0.55, 1.0, 1.0, id, p, 1, 0, 1, 0);
        }
        7 => blend(w, id, 4, 10),
        _ => {}
    }
}

/// State 10: the chase (module doc).
fn chase(w: &mut World, id: MobyId) {
    let p = c::pos(w, id);
    let t = c::pv4(w, id, pv::TGT);
    let mut v = c::sub(t, p);
    v[2] = 0.0;
    let q = c::add(c::set_len3(v, 2.5), p);
    let crossed = path(w, id, pv::AREA).is_some_and(|a| region::crosses(w, a, p, q));
    if !crossed {
        walk(w, id, t);
    } else {
        let tp = target_moby(w, id).map(|m| c::pos(w, m)).unwrap_or([0.0; 4]);
        turn_to(w, id, tp, k::TURN_B, k::TURN_B);
    }
    if wrapped(w, id) {
        if kind(w, id) == 2 {
            if c::pi32(w, id, pv::ALERT) == 0 { set_state(w, id, st::HOME); }
        } else {
            set_state(w, id, st::GO);
        }
    }
}

/// The bite sphere (states 5 and 0x10): `0x27f830(0.6, 1, push, m, pos + 0.333 z, 1, 0, 1, 0)`.
fn bite(w: &mut World, id: MobyId, push: f32) {
    let mut p = c::pos(w, id);
    p[2] += 0.333;
    attack::sphere_hit(w, k::SCALE * 0.6, 1.0, push, id, p, 1, 0, 1, 0);
}

/// The knockback's end (states 9 / 0x11). False: done (a state change or deleted).
fn knocked(w: &mut World, id: MobyId, back: u8) -> bool {
    let r = knock::update(w, id, pv::K);
    if r & 0x40 != 0 {
        let p = c::pos(w, id);
        if k::SEA <= p[2] && path(w, id, pv::AREA).is_some_and(|a| region::point_in_polygon(w, a, p)) {
            set_state(w, id, back);
            blend(w, id, 0, 10);
            return false;
        }
    } else if r & 0x120 == 0 {
        return true;
    }
    w.delete_moby(id);
    false
}

/// The death flight's end (states 0xb / 0x12). False: deleted.
fn dying(w: &mut World, id: MobyId) -> bool {
    if knock::update(w, id, pv::K) & 0x160 == 0 { return true; }
    let p = c::pos(w, id);
    fx::piece_explosion(w, 0.5, 10.0, Some(id), p);
    w.delete_moby(id);
    false
}

/// A jump flight's tick (states 7 / 8 / 0xd): position += v (+ the boat's velocity), gravity. Some(ground z) when it
/// fell below the ground.
fn fly(w: &mut World, id: MobyId, extra: c::V) -> Option<f32> {
    let v = c::pv4(w, id, pv::VEL);
    let p = c::add(c::add(c::pos(w, id), v), extra);
    c::set_pos(w, id, p);
    let vz = c::pf(w, id, pv::VZ) - k::JUMP_G * c::DT2;
    c::set_pf(w, id, pv::VZ, vz);
    if 0.0 <= vz { return None; }
    let g = ground::ground(w, [p[0], p[1], p[2] + 1.0, p[3]], 0.5, 0).z;
    (p[2] < g).then_some(g)
}

/// `0x314318`.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    if state(w, id) != st::HELD {
        c::set_pi16(w, id, pv::SUCK + react::rec::STATE, 0);
        if !pre(w, id) { return; }
    }
    let tm = target_moby(w, id);
    // 0x288f40(2.5, m, 0, +0x280): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 2.5, id, 0, id, 0x280);
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 28.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x16;
        }
    }
    let tpos = |w: &World| tm.map(|m| c::pos(w, m)).unwrap_or([0.0; 4]);
    match state(w, id) {
        st::INIT => init(w, id),
        st::HOP_WAIT => {
            let h = path(w, id, pv::PATH).map(|p| path_point(w, p, 1)).unwrap_or([0.0; 4]);
            turn_to(w, id, h, k::TURN_A, k::TURN_B);
            if kind(w, id) != 2 && w.rng.randi(9) == 0 {
                set_state(w, id, st::JUMP);
                blend_raw(w, id, 0, 5);
                let p = c::pos(w, id);
                let sp = k::JUMP * c::DT;
                let v = c::set_len2(c::sub(h, p), sp);
                c::set_pv4(w, id, pv::VEL, v);
                let mut time = 0.0;
                let vz = knock::lob_up(sp, -(k::JUMP_G * c::DT2), p, h, &mut time);
                c::set_pf(w, id, pv::VZ, vz);
            }
        }
        st::IDLE => {
            if kind(w, id) != 2 {
                new_side(w, id);
                set_state(w, id, st::GO);
                blend(w, id, 1, 10);
            } else if wrapped(w, id) && w.rng.randi(3) != 0 {
                let cam = w.camera.map(|x| f32::from_bits(x.0));
                if w.m(id).visible != 0 && c::dist3(c::pos(w, id), cam) < 32.0 {
                    let a = w.rng.rand_angle();
                    let r = w.rng.randf(1.0, 3.0);
                    let h = c::pv4(w, id, pv::HOME);
                    let g = c::add([a.cos() * r, a.sin() * r, 0.0, c::pf(w, id, pv::GRAZE + 0xc)], h);
                    c::set_pv4(w, id, pv::GRAZE, g);
                    set_state(w, id, st::GRAZE);
                    blend(w, id, 1, 10);
                }
            }
        }
        st::GRAZE => {
            let g = c::pv4(w, id, pv::GRAZE);
            walk(w, id, g);
            if kind(w, id) == 2 {
                if wrapped(w, id) && w.rng.randi(3) == 0 {
                    set_state(w, id, st::IDLE);
                    blend(w, id, 0, 10);
                }
            } else {
                new_side(w, id);
                set_state(w, id, st::GO);
                blend(w, id, 1, 10);
            }
        }
        st::GO | st::ABOARD_GO => {
            if state(w, id) == st::ABOARD_GO { carry(w, id); }
            let s = state(w, id);
            let t = tpos(w);
            if s == st::GO {
                if !wrapped(w, id) || k::BITE_RANGE <= c::dist2(c::pos(w, id), t) {
                    walk(w, id, t);
                } else {
                    set_state(w, id, st::BITE);
                    blend(w, id, 2, 10);
                }
                if kind(w, id) == 2 { set_state(w, id, st::HOME); }
            } else {
                walk(w, id, t);
                if wrapped(w, id) && c::dist2(c::pos(w, id), t) < k::BITE_RANGE {
                    set_state(w, id, st::ABOARD_BITE);
                    blend(w, id, 2, 10);
                }
                if kind(w, id) == 2 { set_state(w, id, st::ABOARD); }
            }
        }
        st::BITE | st::ABOARD_BITE => {
            let aboard = state(w, id) == st::ABOARD_BITE;
            if aboard { carry(w, id); }
            let t = tpos(w);
            walk(w, id, t);
            if !wrapped(w, id) || c::dist2(c::pos(w, id), t) <= k::BITE_RANGE {
                if kind(w, id) == 2 { set_state(w, id, if aboard { st::ABOARD } else { st::HOME }); }
            } else {
                set_state(w, id, if aboard { st::ABOARD_GO } else { st::GO });
                blend(w, id, 1, 10);
            }
            bite(w, id, if aboard { 0.0 } else { 1.0 });
        }
        st::HOME => {
            let h = c::pv4(w, id, pv::HOME);
            if c::dist2(c::pos(w, id), h) < 1.0 {
                set_state(w, id, st::IDLE);
                blend(w, id, 0, 10);
            } else if kind(w, id) == 2 {
                walk(w, id, h);
            } else {
                new_side(w, id);
                set_state(w, id, st::GO);
                blend(w, id, 1, 10);
            }
            if wrapped(w, id) && w.m(id).anim.seq_b != 1 { w.anim_blend(id, 1, 0, 5); }
        }
        s @ (st::JUMP | st::JUMP_B) => {
            if let Some(g) = fly(w, id, [0.0; 4]) {
                if s == st::JUMP {
                    w.mm(id).position[2] = g;
                    set_state(w, id, st::IDLE);
                } else {
                    set_state(w, id, st::HOP_WAIT);
                    w.mm(id).position[2] = g - 1.0;
                }
            }
        }
        st::KNOCKED => {
            if !knocked(w, id, st::IDLE) { return; }
        }
        st::ABOARD_KNOCKED => {
            carry(w, id);
            if !knocked(w, id, st::ABOARD) { return; }
        }
        st::CHASE => chase(w, id),
        st::DYING => {
            if !dying(w, id) { return; }
        }
        st::ABOARD_DYING => {
            carry(w, id);
            if !dying(w, id) { return; }
        }
        st::WAIT_BOAT => board(w, id),
        st::BOARD => {
            let bv = boat(w, id).filter(|&b| w.m(b).pvars.len() >= 0x80).map(|b| c::pv4(w, b, 0x70)).unwrap_or([0.0; 4]);
            if fly(w, id, bv).is_some() {
                w.mm(id).anim.speed = 1.0;
                set_state(w, id, st::ABOARD);
            }
        }
        st::ABOARD => {
            carry(w, id);
            if kind(w, id) != 2 {
                new_side(w, id);
                set_state(w, id, st::ABOARD_GO);
                blend(w, id, 1, 10);
            }
        }
        st::SWIM => swim(w, id),
        st::HELD if c::pi16(w, id, pv::SUCK + react::rec::STATE) != 0 && react::carried(w, id, pv::K) != 0 => {
            let s = w.m(id).cmd;
            set_state(w, id, s);
            c::set_pi16(w, id, pv::SUCK + react::rec::STATE, 0);
        }
        _ => {}
    }
    tail(w, id);
}

/// The tail of `0x314318`: a knocked biter in the sea blows bubbles.
fn tail(w: &mut World, id: MobyId) {
    if c::pf(w, id, pv::K + knock::k::WATER) == 0.0 { return; }
    if ![st::KNOCKED, st::DYING, st::ABOARD_KNOCKED, st::ABOARD_DYING].contains(&state(w, id)) { return; }
    for _ in 0..3 {
        let p0 = c::pos(w, id);
        let x = p0[0] + w.rng.randf_sym(0.0, 0.2);
        let y = p0[1] + w.rng.randf_sym(0.0, 0.2);
        let z = p0[2] + w.rng.randf_sym(0.0, 0.2) + 0.8;
        let a = w.rng.rand_angle();
        let r = w.rng.randf(0.2, 1.3) * c::DT;
        let vz = w.rng.randf(-0.2, 0.4) * c::DT;
        let size = w.rng.randf(2100.0, 14700.0);
        crate::moby_update::classes::bomb_water::part34(w, size, k::BUBBLE_LEVEL, [x, y, z, p0[3]], [a.cos() * r, a.sin() * r, vz]);
    }
}
