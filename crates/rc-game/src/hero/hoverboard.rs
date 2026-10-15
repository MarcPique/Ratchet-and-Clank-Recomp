//! **The Hoverboard: hero states 0x6b..0x6f and the race the board keeps** (movement group 0x16; levels 5 and 16;
//! level05's code, which carries level 16's branches, though level 16's own copy has more: its physics `0x214598`
//! writes 0x13fbde (the board camera's orbit) and is not ported: G-HERO-008). Read from the level05
//! decomp and disassembly: SetState `0x24cee8`, the physics `0x244a70`, the transitions `0x255960` and the board's
//! helpers. docs/plan/hero_states.md "Hoverboard".
//!
//! **Mounting.** The board is class 439 (level05 `0x2f87a8`, `crate::moby_update::classes::units::hoverboard`):
//! Ratchet within 5.5 of it, outside groups 0x15 / 0x16 → the board moby is stored in 0x13fbbc and `SetState(0x6b, 1)`.
//! Its pvars name the race: +0x20 the race line (a path), +0x24 the respawn path, +0x28 the race host (Rilgar's race girl
//! 918), +0x2c.. the racers (+0x40 of them), +0x44 / +0x48 / +0x4c the moby groups of the boost hoops 1139, the boost
//! pads 1140 and the weapon pickups 470 (level 16).
//!
//! **States.** 0x6b riding, 0x6c the ramp jump, 0x6d the crash (thrown off), 0x6e into the water, 0x6f into a wall. The
//! crash states end with `HeroTeleport` to the respawn path's point behind Ratchet in state 0x6b.
//!
//! **What the board reads and writes outside the hero block** goes through [`BoardWorld`] (the snapshot the tick
//! builds before the hero update: the board's pvars, its paths, the racers, the pickups' groups and the game-state
//! words the race tests) and [`BoardCmd`] (the stores into other mobys and the game state, applied at the start of
//! the next moby loop, before any class reads them: `crate::moby_update::classes::units::hoverboard::apply`). The
//! board's carry under Ratchet (`0x24bdc0`'s tail) is applied by the tick right after the hero update.
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | 0x24cee8 0x6b | group 0x16, 0x1415d4 = 8, 0x141402 = 1, 0x1413f7 = 1, jump / airborne 0; from 0x6d..0x6f: the waypoint back to the race line's point nearest Ratchet when that is behind, 0x13fbff / 0x13fc14 / 0x13fc08 / 0x13fc1e = 0; from another group: −(gravity · displacement) kept (group 0x16 again) or the start: level 5 places Ratchet at (247, 287, 76) yaw −2.1, the block 0x13fa10..0x13fc1f cleared (the board moby kept), gravity 22·dt², the host, 0x13fbcc = 70°, the HUD handle −1; global flag 0 set: the slot-0x10 meter queued; the board yaw = the facing; HUD slots 5 / 7 queued; SetAnim(ticks(5), 0x52 / 0x7d, 0) | [`entry`] (slots 5 / 7: [`BoardCmd::RaceHud`]; the slot-0x10 meter: G-LVL-007, logged) |
//! | 0x24cee8 0x6c | group 0x16, 0x1415d4 = 8, 0x1413f7, 0x13fbc0 = 0; SetAnim(ticks(8), 0x55 / 0x7f, 5) | [`entry`] |
//! | 0x24cee8 0x6d | 0x1413f7, 0x1415d4 = 0, loops stopped, 0x13fc14 = 0, vel = eff·0.7, vel.z = 10·dt; SetAnim(ticks(4), 0x61, 3) | [`entry`] |
//! | 0x24cee8 0x6e | as 0x6d; water timer ticks(70), water level = 0x13fbf0, splash (3, min(300·\|disp.z\|, 40), 1), voice 3, the float's fields, momentum = eff in the plane (≤ 9.5·dt), blink 0x68; SetAnim(ticks(17), 0x3a, 0) | [`entry`] |
//! | 0x24cee8 0x6f | as 0x6d, blink 0x68; SetAnim(ticks(4), 0x62, 4) | [`entry`] |
//! | 0x244a70 0x6b / 0x6c | off the map → respawn; airborne / landed; Ratchet's after-images while boosting; the board's loops (slot 6 hum unless airborne, slot 7 boost); trick mode = jumping with L1 / R1 / L2 / R2; boost (□ with fuel, or a boost pickup's timer): 0x13fbcc spring, boost speed → 11·dt; the up vector (ground normal near the ground, else up) and `0x236098` aligning the body; base speed 15·dt (0 on a capsule hit), SpeedStep, the board yaw (set from the facing on the ground, the facing springs to it in the air), the steering stick spring 0x237488 and TurnTo; the lean records 0 / 1 / 3; the trick spin `0x22a7d0` about the trick's pivot; the race (`0x254608`); vz from the move with gravity 0x13fbb0 and the ground clamp; the hit sphere (0.7 at body + 3·disp: crates and the boost pickups 133 get a hit, damage 1, flags 0x10000) | [`physics`] |
//! | 0x244a70 0x6d / 0x6e / 0x6f | 0x6d: board +0xbc = 2, xy speed → 0 by 15·dt², gravity 27·dt², the tumble anim timed to the landing; 0x6e: momentum decay, the float `0x240c78`; 0x6f: the facing to the wall's yaw at 500°/s; all: the body straightens at 350°/s | [`physics`] |
//! | 0x255960 0x6b | the jump lockout; the crash test `0x255208`; the jump count; the height ring 0x13fb20 (its max is computed and never read); the launch speed; ✕ within ticks(9) → the jump (on a ramp surface 5 within ticks(7): 0x6c), ✕ held / early → more lift; `0x253850` (tricks, landing anims), `0x253b48` (pickups), `0x254058` (score); the jump anim timed to the landing; ride ↔ boost anims | [`transitions`] |
//! | 0x255960 0x6c / 0x6d / 0x6e / 0x6f | 0x6c: the jump anim timed (≤ 2.2), the crash test, tricks / pickups / score; 0x6d: below the water (level 5: 61.4, 16: 76) → 0x6e, else as 0x6f; 0x6e: after the water timer or off the map → respawn; 0x6f: at the anim's wrap → respawn | [`transitions`] |
//! | 0x255208 | the crash test: fast and a wall within 0.55 + vel ahead at 0.75 up (slope > 70°, facing it within 45°) → 0x6f (0x6d above 1); a capsule hit with displacement < ¼ of vel → 0x6d; the water levels → 0x6e; level 16 surface 0xa near the ground → 0x6d; level 5 on the ground more than 0.5 below the race line and 4 from it → 0x6d; the landing: tilted > 60° / yaw off the board's > 80° / a trick unfinished → 0x6d, else the landing (skill point 7: level 5, four trick kinds, more than three flips, race won) and 0x6c → 0x6b | [`crash_check`] |
//! | 0x253850 | the landing ETA; a trick (L1 0x69, R1 0x6a, L2 0x6b, R2 0x6c) held: its anim with its loop window, or counting while inside; out of a trick: the landing anim; airborne in the ride anims → the jump anim; the jump anim wrapped on the ground → 9 | [`tricks`] |
//! | 0x253b48 | hoops 1139 (box 0.5 × 2.9 × 2.9 at the body) and pads 1140 (2.8 × 1.25 × 1.5 at the feet; their glow colour pulses; not while airborne): boost timer += the pickup's pvar, its cooldown ticks(60), board sound 1; weapon pickups 470 within 1.5: count + 1 (≤ 3) | [`pickups`] (the weapon's hand switch: level 16, G-HERO-008) |
//! | 0x254058 | race won: move record 30 bumped with a score; flips (> 190° spins), trick kinds, the multiplier table 0x2169f8, the jump's score | [`score`] |
//! | 0x254608 | the race: ticks, the waypoint along the race line (laps at point 0), level 16's shortcuts, wrong way (behind the line's direction for ticks(180), or below it on 16) → fade, respawn; the racers' waypoints and laps, the places; three laps: records, bolts, skill point 9 (level 5 under ticks(5700)), the host's finish (+0xbc = 1) or the freeze dialog (lost, or every race once won), the HUD handle | [`race`] |
//! | 0x254358 | the respawn point: of the respawn path's even points ahead of Ratchet (the next-but-one point more than 90° from him), the nearest in xy (level 16: within 4 in z of the race line); z = GroundHeight(0.5) 2 above; yaw along the path | [`respawn`] |
//! | 0x237488 | the stick spring: −stick.x and stick.y approach by k, clamped to ±1 | [`StickSpring::step`] |
//! | 0x236098 | the body's up toward a vector: the angle springs (0x270b58 with the velocity 0x13f3f4), the rotation about their cross axis, Euler written back | [`align_up`] |
//! | 0x2551b8 / 0x2551d0 | the jump anim 0x55 / 0x7f; the ride anim 0x52 / 0x7d or boost 0x68 / 0x7e (0x7x with the weapon) | [`jump_seq`], [`ride_seq`] |
//! | 0x24bdc0 tail | groups 0x15 / 0x16: the board at the feet + 0.21 up turned by record 0's angles, its rows Ratchet's turned by them | [`carry`] |
//! | 0x2413e0 group 0x16 | the velocity clamp only when a line along it hits the world (or classes 0x1f6 / 0x59f) | `Hero::move_collide` |
//! | 0x240ed0 group 0x16 | the capsule passes: a sphere of 0.5 at 0.7 up the body | `Hero::capsule_try` |
//! | 0x2402a8 groups 0x15 / 0x16 | a crate under the feet: the ground point 1 lower | `Hero::ground_probe` |
//! | 0x2515d0 | groups 0x15 / 0x16 never enter the water | `Hero::water_entry_check` |
//! | 0x25b538 / 0x2478f8 | SetAnim plays the board's own sequence (table 0x17c068: 0x55 / 0x7f → 5, 0x69..0x6c → 1..4; others → 0) | `Hero::set_anim` ([`BoardCmd::Anim`]) |
//! | 0x229348 | the hero's fade 0x15f3fc (on / speed / phase: up to 1, phase 1 for a tick, back to 0) | [`Fade`] (stepped by the tick) |
//!
//! Standard `f32` for the board block (the hero block's PS2 floats converted at the boundary); the random draws are the
//! game's, in its order.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::anim::AnimCtl;
use super::idle::joint;
use super::physics::*;
use super::states::Ctx;
use super::Hero;
use crate::moby_runtime::MobyId;
use crate::pad::{button, fast_arctan, fast_diff_rots};
use crate::ps2v::Pf;
use crate::rng::Rng;

/// The state ids.
pub mod id {
    pub const RIDE: i32 = 0x6b;
    pub const JUMP: i32 = 0x6c;
    pub const CRASH: i32 = 0x6d;
    pub const WATER: i32 = 0x6e;
    pub const WALL: i32 = 0x6f;
}
/// The movement group (0x1413dc).
pub const GROUP: i32 = 0x16;
/// The board class.
pub const BOARD_CLASS: i16 = 439;
/// The pickups' classes (0x473 hoops, 0x474 pads, 0x1d6 weapons) and the boost pickup the hit sphere hits (0x85).
pub const HOOP_CLASS: i16 = 0x473;
pub const PAD_CLASS: i16 = 0x474;
pub const WEAPON_CLASS: i16 = 0x1d6;
pub const BOOST_CLASS: i16 = 0x85;
/// The hand item the weapon pickups switch to.
pub const WEAPON_ITEM: i32 = 0x24;
/// Skill points: 7 the board's tricks (level 5), 9 the race's time (level 5).
pub const SKILL_TRICKS: usize = 7;
pub const SKILL_TIME: usize = 9;

const DTF: f32 = 1.0 / 60.0;
const DT2F: f32 = 1.0 / 3600.0;
const PI: f32 = std::f32::consts::PI;
const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;

fn f(x: Pf) -> f32 { x.to_f32() }
fn p(x: f32) -> Pf { Pf::f(x) }

/// The trick table 0x179bc0: each trick's loop window (start, end key).
pub const TRICK_LOOP: [(i32, i32); 4] = [(6, 21), (6, 21), (9, 24), (9, 24)];
/// 0x179be0: each trick's spin pivot in the body's frame.
pub const TRICK_PIVOT: [[u32; 3]; 4] = [[0, 0xbdf6_9446, 0x3f1a_f4f1], [0, 0xbc9b_a5e3, 0x3f14_5a1d], [0xbe91_4120, 0xbca9_930c, 0x3f22_4745], [0, 0xbe14_1206, 0x3f1a_fb7f]];
/// gp−0x7568 (0x15f698): the key a trick must reach before the landing.
pub const TRICK_LAND: [i32; 4] = [25, 25, 28, 28];
/// 0x2169f8: the score multiplier by the number of trick kinds in a jump.
pub const MULTIPLIER: [f32; 6] = [1.0, 1.0, 1.5, 2.25, 4.0, 0.0];
/// The board's sequence for Ratchet's (`0x2478f8`, table 0x17c068).
pub const BOARD_SEQS: [(u8, u8); 6] = [(0x55, 5), (0x7f, 5), (0x69, 1), (0x6a, 2), (0x6b, 3), (0x6c, 4)];

/// The stick spring of `0x237488` (+4: the y channel, +8: the −x channel).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StickSpring {
    pub y: f32,
    pub x: f32,
}

impl StickSpring {
    /// `0x237488(k, out, blk)`: `Approach(−stick.x, k, +8)` and `Approach(stick.y, k, +4)`, each clamped to ±1;
    /// returns `(+8, +4)`.
    pub fn step(&mut self, h: &Hero, k: f32) -> [f32; 2] {
        let mut x = p(self.x);
        approach(p(-f(h.stick[0])), p(k), &mut x);
        self.x = f(x).clamp(-1.0, 1.0);
        let mut y = p(self.y);
        approach(h.stick[1], p(k), &mut y);
        self.y = f(y).clamp(-1.0, 1.0);
        [self.x, self.y]
    }
}

/// The hero's fade of 0x15f3fc (`0x229348`, gp−0x7580 / −0x7584 / −0x7580): on, speed, phase.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fade {
    pub on: bool,
    pub speed: f32,
    pub phase: i32,
}

impl Fade {
    /// `0x229348` on the global fade `v` (0x15f3fc): phase 0 up to 1 (then phase 1), phase 1 → 2, phase 2 down to 0
    /// (then off).
    pub fn step(&mut self, v: &mut f32) {
        if !self.on { return; }
        match self.phase {
            0 => {
                *v = approach_f(1.0, self.speed, *v);
                if *v == 1.0 { self.phase = 1; }
            }
            1 => self.phase = 2,
            2 => {
                *v = approach_f(0.0, self.speed, *v);
                if *v == 0.0 { self.on = false; }
            }
            _ => {}
        }
    }
}

fn approach_f(t: f32, step: f32, x: f32) -> f32 {
    let mut v = p(x);
    approach(p(t), p(step), &mut v);
    f(v)
}

/// The board block 0x13fa10..0x13fc1f (cleared at the race's start but for the board moby) and the board's other
/// globals. Native `f32`.
#[derive(Clone, Debug, PartialEq)]
pub struct Board {
    /// 0x13fa10: the trick stick spring; 0x13fa20 / 24 / 28: the trick spins; 0x13fa30: the trick kinds used.
    pub trick_stick: StickSpring,
    pub spin: [f32; 3],
    pub used: [i32; 4],
    /// 0x13fa40 trick kinds, 0x13fa44 multiplier, 0x13fa48 ticks in tricks, 0x13fa4c trick score, 0x13fa50 flips per
    /// axis, 0x13fa5c most flips, 0x13fa60 flip score, 0x13fa64 the jump's score.
    pub kinds: i32,
    pub multiplier: f32,
    pub trick_ticks: i32,
    pub trick_score: i32,
    pub flips: [i32; 3],
    pub max_flips: i32,
    pub flip_score: i32,
    pub jump_score: i32,
    /// 0x13fa70: the steering stick spring.
    pub steer: StickSpring,
    /// 0x13fad0: the up the body aligns to; 0x13fae0: the displacement's direction.
    pub up: [f32; 4],
    pub disp_dir: [f32; 4],
    /// 0x13faf0 / 0x13fb00 / 0x13fb10: the racers' laps, waypoints and places.
    pub racer_laps: [i16; 8],
    pub racer_wp: [i16; 8],
    pub racer_place: [i16; 8],
    /// 0x13fb20: the height ring; 0x13fc00 / 0x13fc02 its count / next slot.
    pub heights: [f32; 32],
    pub hist_count: i16,
    pub hist_idx: i16,
    /// 0x13fba4: −(gravity · displacement) on a re-entry from group 0x16.
    pub fba4: f32,
    /// 0x13fba8 / 0x13fbcc: the boost spring's velocity / value; 0x13fbac: the base speed; 0x13fbb0: gravity;
    /// 0x13fbb4: the boost fuel (ticks); 0x13fbb8: the boost speed.
    pub boost_vel: f32,
    pub boost_spring: f32,
    pub base_speed: f32,
    pub gravity: f32,
    pub fuel: i32,
    pub boost_speed: f32,
    /// 0x13fbbc: the board moby (kept across the block's clear).
    pub moby: Option<MobyId>,
    /// 0x13fbc0 (0x6c's entry clears it; no reader in the board code).
    pub fbc0: i32,
    /// 0x13fbc4 / 0x13fbc8: the board's yaw and its spring velocity.
    pub yaw: f32,
    pub yaw_vel: f32,
    /// 0x13fbd0: the slot-0x10 HUD handle (−1 none).
    pub hud: i32,
    /// 0x13fbd4: boosting; 0x13fbd8: ticks to the landing; 0x13fbdd: trick mode; 0x13fbdf: the trick.
    pub boosting: i16,
    pub land_eta: i16,
    pub trick_mode: bool,
    pub trick: u8,
    /// 0x13fbe0: the host moby (the race's +0x28); 0x13fbe4: race ticks; 0x13fbe8 / 0x13fbea: waypoint and lap.
    pub host: Option<MobyId>,
    pub race_ticks: i32,
    pub waypoint: i16,
    pub lap: i16,
    /// 0x13fbec: jump ticks; 0x13fbee: the jump lockout; 0x13fbf0: the water level; 0x13fbf4: the wall's yaw;
    /// 0x13fbf8: the race score; 0x13fbfc: the ramp window; 0x13fbfe: wrong-way ticks; 0x13fbff: fading back.
    pub jump: i16,
    pub jump_lock: i16,
    pub water: f32,
    pub wall_yaw: f32,
    pub score: i32,
    pub ramp: i16,
    pub wrong_way: u8,
    pub fading: bool,
    /// 0x13fc04: the race line is set; 0x13fc08: last tick's ground z; 0x13fc0c: airborne; 0x13fc0e: the water timer;
    /// 0x13fc10: the place; 0x13fc14: the boost pickups' timer; 0x13fc18: the race line's point nearest Ratchet;
    /// 0x13fc1a / 0x13fc1c: the weapon's / the boost's hold timers; 0x13fc1e / 0x13fc1f: weapons and their cooldown.
    pub race_line: bool,
    pub last_ground_z: f32,
    pub airborne: i16,
    pub water_timer: i16,
    pub place: i32,
    pub boost_timer: i32,
    pub nearest: i16,
    pub weapon_hold: i16,
    pub boost_hold: i16,
    pub weapons: u8,
    pub weapon_lock: u8,
    /// 0x141920: the board weapon's shots (the game's statistic; its −1 stops the count; the port's own, not saved).
    pub shots: i16,
    /// A shot of this tick's weapon check for the hand item's update to fire (`0x2a3e30` there: the port's item update
    /// has the table): Ratchet's yaw and pitch, the target.
    pub shot: Option<Shot>,
    /// 0x141402 (set by 0x6b, cleared by every on-foot SetState).
    pub f141402: u8,
    /// gp−0x7558 (0x15f6a8): a time trial was finished (the board's □ help waits for it).
    pub trial_done: bool,
    /// The fade (`0x229348`).
    pub fade: Fade,
    /// `mode_freezeInit(0, 0)` this tick (the engine opens "Quit Race?").
    pub freeze: bool,
    /// The stores into other mobys and the game state, applied by the next moby loop.
    pub cmds: Vec<BoardCmd>,
    /// The board's carry under Ratchet this tick (`0x24bdc0`'s tail), applied by the tick after the hero update.
    pub carry: Option<([f32; 4], [[f32; 4]; 3])>,
    /// The time trials' boost meter (the slot-0x10 element's update and its combo, [`Meter`]).
    pub meter: Meter,
}

/// The time trials' boost meter (level05 slot 0 \| 0x10: init `0x262ae8`, update `0x262b58`, draw `0x262f50`; queued by
/// 0x6b's entry once Rilgar's race is won, the HUD handle 0x13fbd0). Its update is gameplay (the jump's score and fuel,
/// the trick counters reset), so the port runs it here every tick after the hero update ([`meter_tick`]) and the HUD
/// element (`crate::hud::Element::RaceMeter`) draws from it. Its draw's part that writes the game (the combo's score
/// 0x15fb00 and its id 0x15faf0) runs here too, right after the update.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Meter {
    /// The element is in its slot and updating (queued at 0x6b's start, released at the race's end) [L: the game's
    /// keeps updating until another element takes slot 0].
    pub on: bool,
    /// Slot +0x72 phase (0 idle, 1 in the air, 2 crashed, 3 landed), +0x70 its counter (u8), +0x71 the colour fade.
    pub phase: u8,
    pub t: u8,
    pub fade: u8,
    /// 0x15fad4 the flips shown, 0x15fad8 the trick kinds, 0x15fae0 the tricks used, 0x15faf0..0x15fafc the last four
    /// combos (−1 none), 0x15fb00 the combo's score.
    pub flips: i32,
    pub kinds: i32,
    pub used: [i32; 4],
    pub recent: [i32; 4],
    pub combo: i32,
}

/// 0x17e918: the combos' scores (by combo + 5 · flip tier).
pub const COMBO_SCORES: [i32; 15] = [200, 215, 225, 235, 300, 325, 350, 375, 400, 500, 525, 550, 575, 600, 650];

/// `0x262ae8`'s part outside the slot: the last four combos cleared.
pub fn meter_init(h: &mut Hero) {
    h.board.meter = Meter { on: true, recent: [-1; 4], ..Meter::default() };
}

/// `0x262b58` (and `0x262f50`'s combo) for one tick (module doc of [`Meter`]).
pub fn meter_tick(h: &mut Hero) {
    if !h.board.meter.on { return; }
    meter_update(h);
    meter_combo(h);
}

/// `0x262b58`.
fn meter_update(h: &mut Hero) {
    let fade_len = 0x14u8;
    let (state, grounded, air, max_flips) = (h.state, h.grounded_ticks, h.air_ticks, h.board.max_flips);
    let b = &mut h.board;
    let m = &mut b.meter;
    match m.phase {
        1 => {
            if m.t < 10 { m.t += 1; }
            if m.fade != 0 { m.fade -= 1; }
            m.kinds = b.kinds;
            m.used = b.used;
            if m.flips < b.max_flips {
                m.flips = b.max_flips;
                m.fade = fade_len;
            }
            if grounded == 0 && !(id::CRASH..=id::WALL).contains(&state) { return; }
            m.combo = 0;
            b.max_flips = 0;
            b.kinds = 0;
            b.used = [0; 4];
            b.flips = [0; 3];
            if state == id::RIDE {
                if 2 < m.kinds {
                    m.recent = [-1, m.recent[0], m.recent[1], m.recent[2]];
                }
                m.phase = 3;
                m.fade = fade_len;
                m.t = ticks(0x78) as u8;
            } else {
                m.phase = 2;
                m.fade = fade_len;
                m.t = ticks(0x3c) as u8;
            }
        }
        0 => {
            if (air as i32) < 6 || (max_flips as f32) < 1.0 { return; }
            m.flips = max_flips;
            m.kinds = 0;
            m.used = [0; 4];
            m.phase = 1;
            m.t = 0;
            m.fade = fade_len;
        }
        2 | 3 => {
            if m.t == 0 {
                m.combo = 0;
                m.flips = 0;
                m.kinds = 0;
                m.used = [0; 4];
                m.phase = 0;
            } else if m.phase == 3 && m.t as i32 == ticks(0x6e) {
                b.score += m.combo;
                b.fuel += m.combo * 5;
                let cap = (ticks(0x11) as f32 * 60.0) as i32;
                if cap < b.fuel { b.fuel = cap; }
            }
            m.t = m.t.wrapping_sub(1);
        }
        _ => {}
    }
}

/// `0x262f50`'s combo (phase 3, on the board): the combo of three or four trick kinds (by which ones) and the flip
/// tier, its score 0x17e918 divided by one plus the times it is among the last three; else the flips · 10 (one kind) or
/// · 20 (two).
fn meter_combo(h: &mut Hero) {
    let m = &mut h.board.meter;
    if m.phase != 3 || h.group != GROUP { return; }
    let mut combo = -1;
    if m.kinds == 3 {
        combo = 0;
        if m.used[3] != 0 { combo = if m.used[2] == 0 { 1 } else if m.used[1] != 0 { 3 } else { 2 }; }
    } else if m.kinds == 4 {
        combo = 4;
    }
    if 0 <= combo {
        let tier = if 4 <= m.flips { 2 } else { (2 <= m.flips) as i32 };
        let k = combo + tier * 5;
        m.recent[0] = k;
        let n = 1 + m.recent[1..].iter().filter(|&&r| r == k).count() as i32;
        m.combo = COMBO_SCORES[k as usize] / n;
    } else if 0 < m.kinds {
        m.combo = m.flips * if m.kinds == 1 { 10 } else { 20 };
    }
}

/// The meter as the HUD draws it (`0x262f50`): the phase and its counters, the texts' values, the combo, the wrong-way
/// counter 0x13fbfe and the fuel 0x13fbb4.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeterView {
    pub meter: Meter,
    pub wrong_way: u8,
    pub fuel: i32,
}

impl Default for Board {
    fn default() -> Self {
        Board {
            trick_stick: StickSpring::default(), spin: [0.0; 3], used: [0; 4], kinds: 0, multiplier: 0.0, trick_ticks: 0, trick_score: 0,
            flips: [0; 3], max_flips: 0, flip_score: 0, jump_score: 0, steer: StickSpring::default(), up: [0.0; 4], disp_dir: [0.0; 4],
            racer_laps: [0; 8], racer_wp: [0; 8], racer_place: [0; 8], heights: [0.0; 32], hist_count: 0, hist_idx: 0, fba4: 0.0,
            boost_vel: 0.0, boost_spring: 0.0, base_speed: 0.0, gravity: 0.0, fuel: 0, boost_speed: 0.0, moby: None, fbc0: 0, yaw: 0.0,
            yaw_vel: 0.0, hud: -1, boosting: 0, land_eta: 0, trick_mode: false, trick: 0, host: None, race_ticks: 0, waypoint: 0, lap: 0,
            jump: 0, jump_lock: 0, water: 0.0, wall_yaw: 0.0, score: 0, ramp: 0, wrong_way: 0, fading: false, race_line: false,
            last_ground_z: 0.0, airborne: 0, water_timer: 0, place: 0, boost_timer: 0, nearest: 0, weapon_hold: 0, boost_hold: 0,
            weapons: 0, weapon_lock: 0, shots: 0, shot: None, f141402: 0, trial_done: false, fade: Fade::default(), freeze: false, cmds: Vec::new(), carry: None,
            meter: Meter::default(),
        }
    }
}

impl Board {
    /// `FastMemSet(0x13fa10, 0, 0x210)`: the block cleared but for the board moby (SetState 0x6b restores it) and the
    /// globals outside the block.
    fn clear_block(&mut self) {
        let keep = Board { moby: self.moby, f141402: self.f141402, trial_done: self.trial_done, fade: self.fade, freeze: self.freeze, cmds: std::mem::take(&mut self.cmds), carry: self.carry, meter: self.meter, hud: 0, ..Board::default() };
        *self = keep;
    }

    /// `FastMemSet(0x13fa10, 0, 0x60)`: the jump's trick fields.
    fn clear_tricks(&mut self) {
        self.trick_stick = StickSpring::default();
        self.spin = [0.0; 3];
        self.used = [0; 4];
        (self.kinds, self.multiplier, self.trick_ticks, self.trick_score) = (0, 0.0, 0, 0);
        (self.flips, self.max_flips, self.flip_score, self.jump_score) = ([0; 3], 0, 0, 0);
    }
}

// ------------------------------------------------------------------------------------------------
// The world outside the hero block.

/// One member of a pickup group as the moby loop left it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Member {
    pub id: MobyId,
    pub o_class: i16,
    pub pos: [f32; 4],
    /// +0xc0: the rows.
    pub rows: [[f32; 4]; 3],
    /// +0xbc: the cooldown byte the hero code counts down.
    pub bc: u8,
    /// The first pvar word (`**(moby + 0x78)`): the boost it gives.
    pub value: i32,
}

/// What the board code reads outside the hero block, as the moby loop left it (built by the tick before the hero
/// update; `crate::moby_update::classes::units::hoverboard::world`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoardWorld {
    /// 0x13fbbc's class and its pvar words +0x00..+0x5c.
    pub board: MobyId,
    pub o_class: i16,
    pub pv: [i32; 0x18],
    /// The paths of pvar +0x20 (the race line) and +0x24 (the respawn path); empty for −1.
    pub race: Vec<[f32; 4]>,
    pub spawn: Vec<[f32; 4]>,
    /// The racers' positions (pvar +0x2c + 4·k, k < +0x40; None: no such moby).
    pub racers: Vec<Option<[f32; 4]>>,
    /// The five racer words +0x2c..+0x3c whatever the count (the board weapon's aim, `0x223350` case 0x24): the moby and
    /// its position; None for −1.
    pub aim: [Option<(MobyId, [f32; 4])>; 5],
    /// The groups of pvar +0x44 / +0x48 / +0x4c (None: −1).
    pub hoops: Option<Vec<Member>>,
    pub pads: Option<Vec<Member>>,
    pub weapons: Option<Vec<Member>>,
    /// Global flag 0 (0x13d388: Rilgar's race won once), the skill points (0x13d408: 7 / 9 the board's, 0x13d40f /
    /// 0x13d411), PAL (0x15ed80). (Owned item 31, 0x13d4df, is the hero's mirror `Hero::owned`.)
    pub flag0: bool,
    pub skill: [bool; 32],
    pub pal: bool,
}

impl BoardWorld {
    /// pvar word at byte offset `o`.
    pub fn pv(&self, o: usize) -> i32 { self.pv.get(o / 4).copied().unwrap_or(-1) }
}

/// A board-weapon shot waiting for the hand item's update (`Board::shot`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub yaw: f32,
    pub pitch: f32,
    pub target: Option<MobyId>,
}

/// A store of the board code outside the hero block.
#[derive(Clone, Debug, PartialEq)]
pub enum BoardCmd {
    /// `moby +0xbc = v`.
    Bc { id: MobyId, v: u8 },
    /// `moby +0x30 = v`.
    Byte30 { id: MobyId, v: u8 },
    /// The pads' glow: `+0x34 |= 0x10`, `+0x90 = rgba`.
    Glow { id: MobyId, rgba: u32 },
    /// `0x2478f8`: the board's animation (`MobyAnimBlend(board, seq, frame, ticks)`); `seq` None: back to 0 over ticks(7)
    /// when it plays another.
    Anim { id: MobyId, seq: Option<u8>, frame: i32, ticks: i32 },
    /// `PlayClassSound(index, 0, moby)`.
    Sound { id: MobyId, index: i32 },
    /// Skill point `k` (awarded unless earned: the jingle and the banner).
    Skill(usize),
    /// The race's end on level `k` (0: 5, 1: 16): the best time / score records and the bolts (+1000 each).
    Records { k: usize, ticks: i32, score: i32 },
    /// 0x15ee38 / 0x15ee3c + 1 (the finished races of level `k`).
    Finished(usize),
    /// Move record `rec` bumped (`count + 1`, time, the level's bit).
    MoveBump(usize),
    /// `queue_animation_update(5, …)` / `(7, …)`: the race's lap / place and time elements (`crate::hud::Element::RaceLap`
    /// / `RaceTime`).
    RaceHud,
    /// The time trial's meter queued (`queue_animation_update(0x10, …)`) / its flags set to 0 (`FUN_0024b090(h, 0)`).
    Meter(bool),
    /// The HUD calls of the race not ported (slot 0x10's boost meter, `update_resource_counter`: G-LVL-007), logged.
    Hud(&'static str),
    /// `Help_Request(msg, rec)`: the board weapon's hint.
    Help { msg: i32, rec: i32 },
}

/// `0x2551b8`: the jump anim (0x7f with the weapon).
pub fn jump_seq(h: &Hero) -> u8 { if h.board.weapons != 0 { 0x7f } else { 0x55 } }

/// `0x2551d0`: the ride anim (0x52 / 0x7d), boosting 0x68 / 0x7e.
pub fn ride_seq(h: &Hero) -> u8 {
    let w = h.board.weapons != 0;
    match (h.board.boosting != 0, w) {
        (false, false) => 0x52,
        (false, true) => 0x7d,
        (true, false) => 0x68,
        (true, true) => 0x7e,
    }
}

/// The board's own sequence for Ratchet's `seq` (`0x247898` on 0x17c068).
pub fn board_seq(seq: u8) -> Option<u8> { BOARD_SEQS.iter().find(|(s, _)| *s == seq).map(|(_, b)| *b) }

/// `SetAnim`'s tail on levels 5 / 16 (`0x25b538` + `0x2478f8`): in groups 0x15 / 0x16 with a board, the board plays
/// its sequence for Ratchet's.
pub(super) fn link_anim(h: &mut Hero, blend: Pf, seq: u8, frame: i32) {
    if !matches!(h.group, 0x15 | 0x16) { return; }
    let Some(b) = h.board.moby else { return };
    let ticks = f(blend) as i32;
    match board_seq(seq) {
        Some(s) => h.board.cmds.push(BoardCmd::Anim { id: b, seq: Some(s), frame, ticks }),
        None => h.board.cmds.push(BoardCmd::Anim { id: b, seq: None, frame: 0, ticks: super::physics::ticks(7) }),
    }
}

fn world<'a>(env: &'a Env) -> Option<&'a BoardWorld> { env.world.and_then(|w| w.board()) }

fn pt(path: &[[f32; 4]], i: i32) -> [f32; 4] { usize::try_from(i).ok().and_then(|i| path.get(i)).copied().unwrap_or([0.0; 4]) }
fn atan2v(d: [f32; 2]) -> f32 { f(fast_arctan(p(d[0]), p(d[1]))) }
fn diff(a: f32, b: f32) -> f32 { f(fast_diff_rots(p(a), p(b))) }
fn dist2f(a: [f32; 4], b: [f32; 4]) -> f32 { ((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1])).sqrt() }
fn dist3f(a: [f32; 4], b: [f32; 4]) -> f32 { ((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1]) + (a[2] - b[2]) * (a[2] - b[2])).sqrt() }
fn pos4(h: &Hero) -> [f32; 4] { [f(h.pos[0]), f(h.pos[1]), f(h.pos[2]), f(h.pos[3])] }
fn v4f(v: V4) -> [f32; 4] { v.map(f) }

/// `FastDecTimer` on an s16.
fn dec16(t: &mut i16) -> bool {
    if *t == 0 { return true; }
    *t = (*t).max(1) - 1;
    *t == 0
}

/// `FastDecTimer` on an i32.
fn dec32(t: &mut i32) {
    if *t != 0 { *t = (*t).max(1) - 1; }
}

/// `0x220ed8` on a byte: 1 when already 0, else − 1 → 2 on reaching 0, 0 otherwise.
fn dec_byte(b: u8) -> (i32, u8) {
    if b == 0 { return (1, 0); }
    let n = b.max(1) - 1;
    (if n < 1 { 2 } else { 0 }, n)
}

/// `0x270ac0(target, step, &x, 0)`: the angle approaches the target by at most `step`.
fn approach_angle(t: Pf, step: Pf, x: &mut Pf) {
    let mut d = angle_diff(t, *x, 0);
    if !(step < d) && d < -step { d = -step; } else if step < d { d = step; }
    *x = fast_add_rotations(*x, d);
}

// ------------------------------------------------------------------------------------------------
// SetState 0x24cee8.

/// SetState's entries of 0x6b..0x6f. `None`: SetState's epilogue follows.
pub(super) fn entry(h: &mut Hero, c: &mut Ctx, id: i32, play: bool, _old_sub: i32) -> Option<bool> {
    let t = |n: i32| Pf::from_i32(ticks(n));
    match id {
        id::RIDE => {
            h.group = GROUP;
            h.f15d4 = 8;
            h.board.f141402 = 1;
            h.items.f13f7 = 1;
            h.board.jump = 0;
            h.board.airborne = 0;
            if matches!(h.prev_state, id::CRASH | id::WALL | id::WATER) {
                if h.board.race_line {
                    if let Some(w) = world(c.env) {
                        let k = nearest(&w.race, pos4(h));
                        if (k as i16) < h.board.waypoint { h.board.waypoint = k as i16; }
                    }
                }
                h.board.fading = false;
                h.board.boost_timer = 0;
                h.board.last_ground_z = 0.0;
                h.board.weapons = 0;
            }
            if h.prev_group == GROUP {
                h.board.fba4 = -f(dot3(h.gravity_dir, h.disp));
            } else {
                if h.idle.level == 5 {
                    h.pos[0] = p(247.0);
                    h.pos[1] = p(287.0);
                    h.pos[2] = p(76.0);
                    h.rot[2] = Pf::b(0xc006_6666);
                }
                h.board.clear_block();
                h.board.gravity = DT2F * 22.0;
                h.board.host = world(c.env).and_then(|w| usize::try_from(w.pv(0x28)).ok());
                h.board.boost_spring = f32::from_bits(0x3f9c_61aa);
                h.board.hud = -1;
                if world(c.env).is_some_and(|w| w.flag0) {
                    // 0x264298: the slot of 0x15f980 released (another of the level's elements; logged); the meter:
                    // queue_animation_update(0x10, …, 0x13fbb4, ticks(17)·60) (the handle 0x13fbd0: the request itself).
                    h.board.cmds.push(BoardCmd::Hud("0x264298"));
                    h.board.cmds.push(BoardCmd::Meter(true));
                    meter_init(h);
                    h.board.hud = 1;
                }
                h.board.yaw = f(h.rot[2]);
                h.board.cmds.push(BoardCmd::RaceHud);
            }
            if play {
                let seq = if h.board.weapons == 0 { 0x52 } else { 0x7d };
                h.set_anim(c.anim, c.rng, t(5), seq, 0);
            }
        }
        id::JUMP => {
            h.group = GROUP;
            h.f15d4 = 8;
            h.items.f13f7 = 1;
            h.board.fbc0 = 0;
            if play {
                let seq = jump_seq(h);
                h.set_anim(c.anim, c.rng, t(8), seq, 5);
            }
        }
        id::CRASH => {
            h.items.f13f7 = 1;
            h.f15d4 = 0;
            super::packs::stop_loops(h);
            h.board.boost_timer = 0;
            h.vel = vscale(h.eff, Pf::b(0x3f33_3333));
            h.vel[2] = DT * Pf::b(0x4120_0000);
            h.set_anim(c.anim, c.rng, t(4), 0x61, 3);
        }
        id::WATER => {
            h.items.f13f7 = 1;
            h.f15d4 = 0;
            super::packs::stop_loops(h);
            h.board.boost_timer = 0;
            h.board.water_timer = ticks(0x46) as i16;
            h.water_level = p(h.board.water);
            let n = ((fabs(h.disp[2]) * Pf::b(0x4396_0000)).to_f32() as i32).min(0x28);
            super::swim::effects::splash(h, c.rng, 3, n, true);
            if !c.voice(3, 0) { h.packs.sounds.push(super::packs::SoundCmd::Voice { index: 3, flags: 0 }); }
            h.swim.splash = ticks(0x4b) as i16;
            let mut bv = DT * Pf::b(0xc0b6_6666);
            if bv <= h.disp[2] * Pf::b(0x3e99_999a) { bv = h.disp[2] * Pf::b(0x3e99_999a); }
            h.swim.bob_vel = bv;
            h.swim.level = p(h.board.water);
            h.swim.level_vel = Pf::ZERO;
            let b = ((h.pos[2] - p(h.board.water)) - Pf::b(0xbdf5_c28f)).to_f32().clamp(-0.2, 0.2);
            h.swim.bob = p(b);
            h.vel = V0;
            h.momentum = h.eff;
            h.momentum[2] = Pf::ZERO;
            super::common::clamp_len_2745f0(&mut h.momentum, DT * Pf::b(0x4118_0000));
            h.idle.blink_period = 0x68;
            if play { h.set_anim(c.anim, c.rng, t(0x11), 0x3a, 0); }
        }
        _ => {
            h.items.f13f7 = 1;
            h.f15d4 = 0;
            super::packs::stop_loops(h);
            h.board.boost_timer = 0;
            h.idle.blink_period = 0x68;
            if play { h.set_anim(c.anim, c.rng, t(4), 0x62, 4); }
        }
    }
    None
}

// ------------------------------------------------------------------------------------------------
// The physics 0x244a70.

/// The per-state physics of 0x6b..0x6f.
pub(super) fn physics(h: &mut Hero, env: &Env, anim: &mut dyn AnimCtl, rng: &mut Rng) -> bool {
    match h.state {
        id::RIDE | id::JUMP => ride(h, env, anim, rng),
        id::CRASH => {
            if let Some(b) = h.board.moby { h.board.cmds.push(BoardCmd::Bc { id: b, v: 2 }); }
            let mut l = len2(h.vel);
            approach(Pf::ZERO, DT2 * Pf::b(0x4170_0000), &mut l);
            h.vel = set_len2(h.vel, l);
            let g = DT2 * Pf::b(0x41d8_0000);
            h.gravity_from(h.eff_v[2], g);
            let eta = {
                let c = Ctx { env, anim: &mut *anim, rng: &mut *rng, voice: None };
                f(h.land_eta(&c, Pf::b(0x4270_0000), g, Pf::b(0xbf80_0000))) as i32
            };
            let v = anim.view();
            if eta == 0 || 16.0 - v.frame <= 1.0 {
                h.anim_speed = Pf::ONE;
            } else {
                h.anim_speed_for_ticks(Pf::b(0x4180_0000), Pf::from_i32(eta), Pf::b(0x3f00_0000), Pf::b(0xbf80_0000), &v);
                let s = f(h.anim_speed).clamp(0.33, 1.2);
                h.anim_speed = p(s);
                if h.height < Pf::b(0x3f00_0000) && s < 1.0 { h.anim_speed = Pf::ONE; }
            }
            straighten(h);
        }
        id::WATER => {
            if let Some(b) = h.board.moby { h.board.cmds.push(BoardCmd::Bc { id: b, v: 2 }); }
            h.vel = V0;
            h.momentum_decay(DT2 * Pf::b(0x4090_0000));
            h.surface_bob();
            straighten(h);
        }
        _ => {
            h.vel = V0;
            straighten(h);
            let (wy, mut yaw) = (p(h.board.wall_yaw), h.rot[2]);
            approach_angle(wy, DT * Pf::b(0x410b_a3d2), &mut yaw);
            h.rot[2] = yaw;
        }
    }
    true
}

/// The crash states' straightening: rot.y then rot.x toward 0 at 350°/s.
fn straighten(h: &mut Hero) {
    let s = DT * Pf::b(0x40c3_7b48);
    let (mut y, mut x) = (h.rot[1], h.rot[0]);
    approach_angle(Pf::ZERO, s, &mut y);
    approach_angle(Pf::ZERO, s, &mut x);
    (h.rot[1], h.rot[0]) = (y, x);
}

/// 0x244a70 cases 0x6b / 0x6c.
fn ride(h: &mut Hero, env: &Env, anim: &mut dyn AnimCtl, rng: &mut Rng) {
    if h.pos[0] < Pf::b(0x40a0_0000) || h.pos[1] < Pf::b(0x40a0_0000) {
        let (pos, rot) = respawn(h, env);
        h.board.yaw = rot[2];
        let mut c = Ctx { env, anim: &mut *anim, rng: &mut *rng, voice: None };
        h.teleport(&mut c, pos, rot, id::RIDE, true);
    }
    let b = &mut h.board;
    if b.airborne == 0 {
        if 0.3 < b.last_ground_z - f(h.ground_z) { b.airborne = 1; }
    } else if h.f65c == 0 && b.jump == 0 {
        b.airborne = 0;
    }
    b.last_ground_z = f(h.ground_z);
    // Ratchet's after-images while boosting (0x277400 / 0x277428 / 0x277508 on 0x1409c0).
    let full = DTF * 11.0;
    let trail = b.boosting != 0 && b.boost_speed != full;
    if trail && !h.fx.trails.hero.active {
        let t = &mut h.fx.trails.hero;
        t.start(0);
        t.add(0x28, 1);
        t.add(0x14, 2);
        t.add(10, 3);
    }
    let fade = if h.board.boosting == 0 || h.board.boost_speed == full { 1 } else { 0 };
    super::packs::hero_trail_update(h, &anim.view(), fade);
    // The board's loops: slot 7 the boost, slot 6 the hum on the ground.
    if let Some(m) = h.board.moby {
        let o_class = world(env).map_or(BOARD_CLASS, |w| w.o_class);
        if h.board.boosting == 0 { super::packs::release_loop(h, 7); } else { super::packs::loop_sound_on(h, 7, m, o_class, 2); }
        if h.board.airborne == 0 { super::packs::loop_sound_on(h, 6, m, o_class, 0); } else { super::packs::release_loop(h, 6); }
    }
    let held = env.pad.held;
    h.board.trick_mode = h.board.jump != 0 && held & 0xf != 0;
    let b = &mut h.board;
    if ((held & button::SQUARE == 0 && b.boost_hold == 0) || b.fuel == 0) && b.boost_timer == 0 {
        b.boosting = 0;
        spring_f(f32::from_bits(0x3f9c_61aa), 0.007, 0.3, DTF * std::f32::consts::FRAC_PI_6, &mut b.boost_spring, &mut b.boost_vel);
        b.boost_speed = approach_f(0.0, DT2F * 10.0, b.boost_speed);
    } else {
        dec16(&mut b.boost_hold);
        if b.boosting == 0 { b.boost_hold = ticks(0x14) as i16; }
        b.boosting = 1;
        spring_f(f32::from_bits(0x3f9c_61aa), 0.01, 0.3, DTF * 0.698_131_7, &mut b.boost_spring, &mut b.boost_vel);
        b.boost_speed = approach_f(DTF * 11.0, DT2F * 20.0, b.boost_speed);
        if b.boost_timer == 0 { dec32(&mut b.fuel); } else { dec32(&mut b.boost_timer); }
        if let Some(m) = b.moby { b.cmds.push(BoardCmd::Bc { id: m, v: 1 }); }
    }
    // The body's up.
    if h.air_ticks == 0 {
        if h.height < Pf::ONE {
            h.board.disp_dir = v4f(set_len3(h.disp, Pf::ONE));
            h.board.up = v4f(set_len3(h.ground_normal, Pf::ONE));
        }
        let up = h.board.up;
        align_up(h, DT2F * 52.359_88, DT2F * 0.003_490_658_5, 0.0, up);
    } else if !h.board.trick_mode {
        h.board.up = [0.0, 0.0, 1.0, 0.0];
        align_up(h, DT2F * 43.633_232, 0.2, 0.0, [0.0, 0.0, 1.0, 0.0]);
    }
    let b = &mut h.board;
    b.base_speed = if h.cap_hit == 0 { approach_f(DTF * 15.0, DT2F * 17.0, b.base_speed) } else { approach_f(0.0, DT2F * 5.0, b.base_speed) };
    h.target_speed = p(b.base_speed + b.boost_speed);
    h.speed_step(DT2 * Pf::b(0x4210_0000), DT2 * Pf::b(0x4210_0000));
    if h.air_ticks == 0 {
        h.board.yaw = f(h.rot[2]);
        h.board.yaw_vel = 0.0;
        h.set_planar_vel(Pf::b(0x47c3_4f80));
    } else {
        h.set_planar_vel(p(h.board.yaw));
        let lim = Pf::b(0x3f32_b8c2);
        if !h.board.trick_mode && fabs(h.rot[1]) < lim && fabs(h.rot[0]) < lim {
            let (mut y, mut v) = (h.rot[2], p(h.board.yaw_vel));
            turn_spring(p(h.board.yaw), SCALE64 * Pf::b(0x3d23_d70a), SCALE64 * Pf::b(0x3e99_999a), DT * Pf::b(0x40df_66f3), &mut y, &mut v, 0);
            h.rot[2] = y;
            h.board.yaw_vel = f(v);
        }
    }
    // Steering.
    if h.f65c == 0 || !h.board.trick_mode {
        let mut s = h.board.steer;
        let o = s.step(h, 0.17);
        h.board.steer = s;
        let a = fast_arctan(p(-o[1]), p(o[0]));
        h.target_yaw = fast_add_rotations(env.cam_yaw, a);
        if Pf::b(0x3e4c_cccd) < h.stick_mag {
            let (mut rate, mut k) = (2.967_059_6f32, 0.005f32);
            let d = SCALE64 * Pf::b(0x3e99_999a);
            if h.f65c != 0 {
                rate = 1.745_329_3;
                k = 0.0037;
            }
            let max = p(DTF * rate) * h.stick_mag;
            let k = SCALE64 * p(k);
            let mut saved = h.yaw_vel;
            h.turn_to(k, d, max);
            if h.air_ticks != 0 {
                let mut y = p(h.board.yaw);
                turn_spring(h.target_yaw, k, d, max, &mut y, &mut saved, 0);
                h.board.yaw = f(y);
            }
        }
    } else {
        h.board.yaw_vel = 0.0;
        h.yaw_vel = Pf::ZERO;
        h.board.steer = StickSpring::default();
    }
    // The lean records 3 / 0 / 1.
    let v = if h.stick_mag < Pf::b(0x3e4c_cccd) { 0.0 } else { f(h.yaw_vel) };
    {
        let j = &mut h.idle.joints;
        (j[joint::HEAD].k, j[joint::HEAD].d) = (f32::from_bits(0x3ca3_d70a), f32::from_bits(0x3e4c_cccd));
        j[joint::HEAD].target[1] = v * -22.0;
        j[joint::HEAD].target[2] = v * 37.0;
        if 0.0 < v { j[joint::HEAD].target[0] = j[joint::HEAD].target[1]; }
        (j[joint::REC0].k, j[joint::REC0].d) = (f32::from_bits(0x3b03_126f), f32::from_bits(0x3dcc_cccd));
        j[joint::REC0].target[0] = (j[joint::REC0].target[0] + v * -11.0).clamp(-0.663_225_1, 0.663_225_1);
        (j[joint::NECK].k, j[joint::NECK].d) = (f32::from_bits(0x3c44_9ba6), f32::from_bits(0x3e61_47ae));
        j[joint::NECK].target[1] = v * 25.0;
        j[joint::NECK].target[0] = v * -10.0;
    }
    // The trick's spin about its pivot.
    if h.air_ticks != 0 && h.board.trick_mode {
        h.yaw_vel = Pf::ZERO;
        let pv = TRICK_PIVOT[h.board.trick as usize & 3].map(f32::from_bits);
        let mut s = h.board.trick_stick;
        let o = s.step(h, 0.2);
        h.board.trick_stick = s;
        let ax = -o[0] * DTF * 12.217_304;
        let ay = -o[1] * DTF * 12.217_304;
        h.board.spin[0] += ax;
        h.board.spin[1] += ay;
        h.turn_about([ax, ay, 0.0], pv);
    }
    race(h, env, anim, rng);
    // vz from the move, gravity, the ground.
    let air = h.air_ticks;
    let mut vz = if h.cap_hit == 0 { h.disp[2] } else { h.eff_v[2] };
    let g = p(h.board.gravity);
    vz = vz - g;
    if air == 0 {
        if !(h.disp[2] < Pf::ZERO) {
            let lo = -g * Pf::b(0x3fc0_0000);
            if vz < lo { vz = lo; }
        } else if vz < h.disp[2] {
            vz = h.disp[2];
        }
    }
    if h.board.jump == 0 && air < ticks(5) as i16 && h.height < Pf::b(0x3f00_0000) { vz = vz - DT2 * Pf::b(0x41a0_0000); }
    h.vel[2] = vz;
    if h.pos[2] < h.ground_z { h.pos[2] = h.ground_z; }
    // The hit sphere: crates and the boost pickups 133.
    let Some(sc) = env.mobys else { return };
    let centre = vadd(vscale(h.disp, Pf::b(0x4040_0000)), h.body_point);
    let listed = crate::collision_query::coll_sphere_mobys(sc, to_f32x3(centre), 0.7, crate::collision_query::QueryFlags(0), env.hero_moby);
    if listed.is_empty() { return; }
    let dir = vscale(h.disp, Pf::b(0x40e0_0000));
    for m in listed {
        let class = sc.mobys.moby(m).map_or(-1, |x| x.o_class);
        if (500..=540).contains(&class) || class == BOOST_CLASS {
            let tmpl = super::packs::template(env, 1.0, 0x1_0000, dir);
            h.packs.hits.push(super::packs::PackHit::Moby { id: m, tmpl });
        }
    }
}

/// `Spring(target, k·SCALE64, d·SCALE64, max, &x, &v)` on the board's native floats.
fn spring_f(t: f32, k: f32, d: f32, max: f32, x: &mut f32, v: &mut f32) {
    let (mut xx, mut vv) = (p(*x), p(*v));
    spring(p(t), SCALE64 * p(k), SCALE64 * p(d), p(max), &mut xx, &mut vv);
    (*x, *v) = (f(xx), f(vv));
}

/// `0x236098(k, d, max, up, &rows, 0)`: Ratchet's body turned so its up approaches `up`: the angle between them
/// (springing through 0x270b58 with the velocity 0x13f3f4, from 0 each tick: the step), about their cross axis
/// (brought onto z by −yaw then −pitch, turned, brought back), the Euler angles written back (`0x2721f0`).
pub fn align_up(h: &mut Hero, k: f32, d: f32, max: f32, up: [f32; 4]) {
    use crate::moby_update::services::{mat4_mul, rows_euler};
    let u = [p(up[0]), p(up[1]), p(up[2]), p(up[3])];
    let mut rows = h.rows;
    let c = dot3(u, rows[2]);
    let s = Pf::ZERO + (Pf::ONE - c * c).sqrt();
    let ang = fast_arctan(c, s);
    let r2 = rows[2];
    // `FastVecCross(&axis, up, &rows[2])` = rows[2] × up (the operands' order reversed: `b × a`).
    let mut axis = [r2[1] * u[2] - r2[2] * u[1], r2[2] * u[0] - r2[0] * u[2], r2[0] * u[1] - r2[1] * u[0], Pf::ZERO];
    let rot = |e: V4| crate::moby_update::services::euler_rows_fpu(e);
    let apply = |m: &[V4; 4], r: &[V4; 4]| -> [V4; 4] { mat4_mul(m, r) };
    let vmul = |v: V4, m: &[V4; 4]| -> V4 { std::array::from_fn(|k| ((m[0][k] * v[0] + m[1][k] * v[1]) + m[2][k] * v[2]) + m[3][k] * v[3]) };
    let z = Pf::ZERO;
    let (yaw, pitch) = if axis[2] < Pf::b(0x3f7d_70a4) {
        let yaw = fast_arctan(axis[0], axis[1]);
        let m = rot([z, z, -yaw, z]);
        rows = apply(&m, &rows);
        axis = vmul(axis, &m);
        let pitch = fast_arctan(axis[2], axis[0]);
        let m = rot([z, -pitch, z, z]);
        rows = apply(&m, &rows);
        axis = vmul(axis, &m);
        (yaw, pitch)
    } else {
        (z, z)
    };
    let _ = axis;
    let mut x = Pf::ZERO;
    let mut v = h.swim.euler_vel[1];
    turn_spring(ang, p(k), p(d), p(max), &mut x, &mut v, 0);
    h.swim.euler_vel[1] = v;
    rows = apply(&rot([z, z, x, z]), &rows);
    rows = apply(&rot([z, pitch, z, z]), &rows);
    rows = apply(&rot([z, z, yaw, z]), &rows);
    let e = rows_euler(&rows);
    h.rot[0] = e[0];
    h.rot[1] = e[1];
    h.rot[2] = e[2];
}

// ------------------------------------------------------------------------------------------------
// The transitions 0x255960.

/// The per-state transitions of 0x6b..0x6f.
pub(super) fn transitions(h: &mut Hero, c: &mut Ctx) {
    match h.state {
        id::RIDE => tr_ride(h, c),
        id::JUMP => {
            let v = c.anim.view();
            if v.seq_b == 0x55 || v.seq_b == 0x7f {
                let eta = f(h.land_eta(c, Pf::b(0x4270_0000), p(h.board.gravity), Pf::b(0xbf80_0000))) as i32;
                h.board.land_eta = eta as i16;
                if eta != 0 { h.anim_speed_for_ticks(Pf::b(0x41d8_0000), Pf::from_i32(eta), Pf::b(0x3f00_0000), Pf::b(0xbf80_0000), &v); }
                if Pf::b(0x400c_cccd) < h.anim_speed { h.anim_speed = Pf::b(0x400c_cccd); }
            }
            if !crash_check(h, c) {
                tricks(h, c);
                pickups(h, c);
                score(h, c);
            }
        }
        id::CRASH => {
            let level = h.idle.level;
            let water = if level == 5 && h.pos[2] < Pf::b(0x4275_999a) {
                Some(Pf::b(0x4275_999a))
            } else if level == 0x10 && h.pos[2] < Pf::b(0x4298_0000) {
                Some(Pf::b(0x4298_0000))
            } else {
                None
            };
            match water {
                Some(wl) => {
                    h.board.water = f(wl);
                    h.set_state(c, id::WATER, true);
                    let t = ticks(0x46) - (h.timer & 0xffff);
                    h.board.water_timer = t as i16;
                    if !(ticks(0x1e) <= ((t << 16) >> 16)) { h.board.water_timer = ticks(0x1e) as i16; }
                }
                None => wall_end(h, c),
            }
        }
        id::WATER => {
            if Pf::b(0x40a0_0000) <= h.pos[0] && Pf::b(0x40a0_0000) <= h.pos[1] && h.timer <= h.board.water_timer as i32 { return; }
            respawn_teleport(h, c);
        }
        _ => wall_end(h, c),
    }
}

/// 0x6f (and 0x6d out of the water): back on the board at the anim's wrap.
fn wall_end(h: &mut Hero, c: &mut Ctx) {
    if c.anim.view().flags & 2 != 0 { respawn_teleport(h, c); }
}

fn respawn_teleport(h: &mut Hero, c: &mut Ctx) {
    let (pos, rot) = respawn(h, c.env);
    h.board.yaw = rot[2];
    h.teleport(c, pos, rot, id::RIDE, true);
}

/// 0x255960 case 0x6b.
fn tr_ride(h: &mut Hero, c: &mut Ctx) {
    dec16(&mut h.board.jump_lock);
    if crash_check(h, c) { return; }
    let b = &mut h.board;
    if b.jump != 0 {
        if h.air_ticks == 0 {
            b.airborne = 0;
            b.jump = 0;
            b.jump_lock = ticks(5) as i16;
        }
        if b.jump != 0 { b.jump += 1; }
    }
    // The height ring (its maximum over ticks(30) is computed and never read).
    let i = b.hist_idx as usize & 31;
    b.heights[i] = f(h.height);
    b.hist_count = (b.hist_count + 1).min(0x20);
    b.hist_idx = ((b.hist_idx as i32 + 1) & 31) as i16;
    // The launch speed.
    let mut up = f(h.eff[2]);
    let mut ez = f(h.eff[2]);
    if -0.025 < up {
        let s = (f(fast_sin(h.slope)) * f(h.eff_len_xy)).max(0.0);
        up = (f(h.eff_len_xy) * 0.5).min(s);
    }
    if ez < 0.0 { ez = 0.0; }
    if up < 0.0 { up = 0.0; }
    if h.surf.f0638 != 0 { h.board.ramp = ticks(7) as i16; }
    dec16(&mut h.board.ramp);
    if h.board.jump == 0 {
        if (h.air_ticks < ticks(4) as i16 || h.board.airborne == 0) && h.board.jump_lock == 0 && c.env.pad.pressed_within(button::CROSS, ticks(9)).is_some() {
            h.board.airborne = 1;
            h.board.jump = 1;
            h.board.clear_tricks();
            if h.board.ramp != 0 {
                let z = p(up) + DT * Pf::b(0x416b_3333);
                (h.disp[2], h.eff[2], h.eff_v[2]) = (z, z, z);
                h.set_state(c, id::JUMP, true);
                return;
            }
            let t = ticks(10);
            let seq = jump_seq(h);
            h.set_anim(c.anim, c.rng, Pf::from_i32(t), seq, 5);
            if let Some(m) = h.board.moby { h.board.cmds.push(BoardCmd::Anim { id: m, seq: Some(6), frame: 5, ticks: t }); }
            let z = p(up) + DT * Pf::b(0x40b0_0000);
            (h.disp[2], h.eff[2], h.eff_v[2]) = (z, z, z);
            return;
        }
    } else if h.board.jump < ticks(0x11) as i16 && h.state != id::JUMP && (c.env.pad.held & button::CROSS != 0 || h.board.jump < ticks(4) as i16) {
        h.eff[2] = p(ez) + DT * Pf::b(0x3f17_0a3d);
        h.disp[2] = h.disp[2] + DT * Pf::b(0x3f17_0a3d);
        h.eff_v[2] = h.eff[2];
    }
    tricks(h, c);
    pickups(h, c);
    score(h, c);
    let v = c.anim.view();
    if (v.seq_b == 0x55 || v.seq_b == 0x7f) && v.frame_b < 0xc {
        if h.board.land_eta != 0 { h.anim_speed_for_ticks(Pf::b(0x41d8_0000), Pf::from_i32(h.board.land_eta as i32), Pf::b(0x3f00_0000), Pf::b(0xbf80_0000), &v); }
        if Pf::b(0x3f99_999a) < h.anim_speed { h.anim_speed = Pf::b(0x3f99_999a); }
    }
    let s = ride_seq(h);
    let sb = c.anim.view().seq_b;
    if (sb == 0x52 || sb == 0x7d) && (s == 0x68 || s == 0x7e) {
        h.set_anim(c.anim, c.rng, Pf::from_i32(ticks(7)), s, 0);
    } else if (sb == 0x68 || sb == 0x7e) && (s == 0x52 || s == 0x7d) {
        h.set_anim(c.anim, c.rng, Pf::from_i32(ticks(0x11)), s, 0);
    }
    if h.f65c == 0 {
        h.anim_speed = Pf::ONE;
        let v = c.anim.view();
        if (v.seq_b == 0x55 || v.seq_b == 0x7f) && v.frame < 25.0 {
            let seq = jump_seq(h);
            h.set_anim(c.anim, c.rng, Pf::from_i32(ticks(8)), seq, 0xe);
        }
    }
    let v = c.anim.view();
    if v.flags & 2 == 0 || v.seq_b == s { return; }
    h.set_anim(c.anim, c.rng, Pf::from_i32(ticks(10)), s, 0);
}

/// `0x255208`: the crash test (true: the state changed).
fn crash_check(h: &mut Hero, c: &mut Ctx) -> bool {
    let to = |h: &mut Hero, c: &mut Ctx, s: i32| { h.set_state(c, s, true); true };
    if DT * Pf::b(0x4140_0000) < h.eff_len_xy {
        let a = from_f32x3(super::packs::local(h, [0.0, 0.0, 0.75]));
        let mut d = vadd(set_len3(h.vel, Pf::b(0x3f0c_cccd)), h.vel);
        d[2] = Pf::ZERO;
        let e = vadd(a, d);
        if let Some(o) = c.env.line(a, e, 2) {
            if o.moby.is_none() && o.kind > 0 {
                let n = from_f32x3(o.normal);
                if Pf::b(0x3f9c_61aa) < fast_arctan(n[2], len2(n)) {
                    let w = fast_add_rotations(fast_arctan(n[0], n[1]), Pf::b(0x4049_0fdb));
                    h.board.wall_yaw = f(w);
                    let vy = fast_arctan(h.vel[0], h.vel[1]);
                    if fast_diff_rots(vy, w) < Pf::b(0x3f49_0fdb) {
                        let s = if Pf::ONE <= h.height { id::CRASH } else { id::WALL };
                        return to(h, c, s);
                    }
                }
            }
        }
    }
    let level = h.idle.level;
    if h.cap_hit != 0 && len2(h.disp) < len2(h.vel) * Pf::b(0x3e80_0000) { return to(h, c, id::CRASH); }
    if level == 5 && h.pos[2] < Pf::b(0x4275_999a) {
        h.board.water = f(Pf::b(0x4275_999a));
        return to(h, c, id::WATER);
    }
    if level == 0x10 {
        if h.pos[2] < Pf::b(0x4298_0000) {
            h.board.water = 76.0;
            return to(h, c, id::WATER);
        }
        if h.f063a != 0 && h.height < Pf::b(0x3dcc_cccd) { return to(h, c, id::CRASH); }
    }
    if h.f65c == 0 {
        if h.board.race_line {
            if let Some(w) = world(c.env) {
                let k = nearest(&w.race, pos4(h));
                let n = w.race.len() as i32;
                let k2 = if n == 0 { 0 } else { (k - 1 + n) % n };
                h.board.nearest = k as i16;
                if level == 5 {
                    let z = pt(&w.race, k2)[2].min(pt(&w.race, k)[2]);
                    let d = dist2f(pos4(h), pt(&w.race, k));
                    if 0.5 < z - f(h.pos[2]) && 4.0 < d { return to(h, c, id::CRASH); }
                }
            }
        }
        let v = c.anim.view();
        if h.f65c == 0 && h.board.airborne != 0 && h.board.jump == 0 && (v.seq_b == 0x55 || v.seq_b == 0x7f) && v.frame_b < 0xd {
            let seq = jump_seq(h);
            h.set_anim(c.anim, c.rng, Pf::from_i32(ticks(7)), seq, 0xe);
        }
    }
    let mut landed = h.f65c == 0;
    if h.cap_hit != 0 && fabs(h.contact_point[2] - h.ground_z) < Pf::b(0x3d4c_cccd) { landed = true; }
    if !landed || h.board.jump == 0 || !(h.vel[2] < Pf::ZERO) { return false; }
    let lim = Pf::b(0x3f86_0a92);
    let v = c.anim.view();
    let trick_done = !(0x69..=0x6c).contains(&v.seq_b) || TRICK_LAND[h.board.trick as usize & 3] as f32 <= v.frame;
    if lim < fabs(h.rot[1]) || lim < fabs(h.rot[0]) || Pf::b(0x3fb2_b8c2) < fast_diff_rots(h.rot[2], p(h.board.yaw)) || !trick_done {
        return to(h, c, id::CRASH);
    }
    if world(c.env).is_some_and(|w| w.flag0) {
        if level == 5 && h.board.kinds == 4 && 3 < h.board.max_flips && !world(c.env).is_some_and(|w| w.skill[SKILL_TRICKS]) { h.board.cmds.push(BoardCmd::Skill(SKILL_TRICKS)); }
        h.board.jump_score = 0;
    }
    let sb = c.anim.view().seq_b;
    let land_anim = |h: &mut Hero, c: &mut Ctx| {
        if sb != 0x55 && sb != 0x7f {
            let seq = jump_seq(h);
            h.set_anim(c.anim, c.rng, Pf::from_i32(ticks(7)), seq, 0xe);
        }
    };
    if h.state != id::JUMP {
        land_anim(h, c);
        return false;
    }
    h.set_state(c, id::RIDE, false);
    land_anim(h, c);
    true
}

/// `0x253850`: the tricks and the landing anims.
fn tricks(h: &mut Hero, c: &mut Ctx) {
    h.board.land_eta = f(h.land_eta(c, Pf::b(0x4270_0000), p(h.board.gravity), Pf::b(0xbf80_0000))) as i16;
    let held = c.env.pad.held;
    if h.board.trick_mode && held & 0xf != 0 {
        h.board.trick = if held & button::R1 != 0 {
            1
        } else if held & button::L2 != 0 {
            2
        } else if held & button::R2 != 0 {
            3
        } else {
            0
        };
        let k = h.board.trick as usize;
        let seq = h.board.trick + 0x69;
        let v = c.anim.view();
        if v.seq_b == seq {
            if (c.anim.loop_start() as f32) - 2.0 < v.frame {
                h.board.used[k] = 1;
                h.board.trick_ticks += 1;
            }
        } else {
            h.set_anim(c.anim, c.rng, Pf::from_i32(ticks(5)), seq, 2);
            c.anim.set_loop(TRICK_LOOP[k].0, TRICK_LOOP[k].1);
        }
    }
    let v = c.anim.view();
    let (start, end) = (c.anim.loop_start(), c.anim.loop_state().1);
    if (h.air_ticks == 0 || held & 0xf == 0) && (0x69..=0x6c).contains(&v.seq_b) && !v.blending() && start < v.frame_b as i32 && (v.frame_b as i32) < end {
        let eta = h.board.land_eta as i32;
        let seq = jump_seq(h);
        if ticks(0x1e) < eta {
            h.set_anim(c.anim, c.rng, Pf::from_i32(ticks(0x11)), seq, 9);
        } else {
            let n = eta.max(ticks(10));
            h.set_anim(c.anim, c.rng, Pf::from_i32(n), seq, 0xd);
        }
    }
    let sb = c.anim.view().seq_b;
    if h.board.airborne != 0 && matches!(sb, 0x52 | 0x7d | 0x68 | 0x7e) {
        let seq = jump_seq(h);
        h.set_anim(c.anim, c.rng, Pf::from_i32(ticks(10)), seq, 0xb);
    }
    if c.anim.view().flags & 2 != 0 && h.f65c != 0 {
        let seq = jump_seq(h);
        h.set_anim(c.anim, c.rng, Pf::from_i32(ticks(7)), seq, 9);
    }
}

/// `0x253b48`: the boost hoops, the pads and the weapons.
fn pickups(h: &mut Hero, c: &mut Ctx) {
    let Some(board) = h.board.moby else { return };
    let Some(w) = world(c.env) else { return };
    let local = |m: &Member, from: [f32; 4]| -> [f32; 3] {
        let d = [m.pos[0] - from[0], m.pos[1] - from[1], m.pos[2] - from[2]];
        std::array::from_fn(|k| d[0] * m.rows[k][0] + d[1] * m.rows[k][1] + d[2] * m.rows[k][2])
    };
    let mut cmds = Vec::new();
    let mut boost = 0;
    let body = v4f(h.body_point);
    for m in w.hoops.iter().flatten() {
        if m.o_class != HOOP_CLASS { continue; }
        let (r, n) = dec_byte(m.bc);
        cmds.push(BoardCmd::Bc { id: m.id, v: n });
        if r == 0 { continue; }
        let l = local(m, body);
        if l[0].abs() < 0.5 && l[1].abs() < 2.9 && l[2].abs() < 2.9 {
            boost += m.value;
            cmds.push(BoardCmd::Bc { id: m.id, v: ticks(0x3c) as u8 });
            cmds.push(BoardCmd::Sound { id: board, index: 1 });
        }
    }
    // The pads' glow pulse (`(counter % ticks(50)) / ticks(50)`).
    let n = ticks(0x32);
    let t = (h.idle.counter as i64).rem_euclid(n as i64) as f32 / n as f32;
    let s = f(fast_sin(p((t + t) * PI - PI)));
    let r = (s * 48.0) as i32;
    let g = ((s * 64.0) as i32 + 0xd0).min(0xff);
    let bl = (s * 24.0) as i32;
    if let Some(pads) = &w.pads {
        if h.board.airborne == 0 {
            for m in pads {
                if m.o_class != PAD_CLASS { continue; }
                let rgba = (((bl + 0x3c) as u32) << 16) | 0x8000_0000 | ((g as u32) << 8) | ((r + 0x68) as u32);
                cmds.push(BoardCmd::Glow { id: m.id, rgba });
                let (rr, nn) = dec_byte(m.bc);
                cmds.push(BoardCmd::Bc { id: m.id, v: nn });
                if rr == 0 { continue; }
                let l = local(m, pos4(h));
                if l[0].abs() < 2.8 && l[1].abs() < 1.25 && l[2].abs() < 1.5 {
                    boost += m.value;
                    cmds.push(BoardCmd::Bc { id: m.id, v: ticks(0x3c) as u8 });
                    cmds.push(BoardCmd::Sound { id: board, index: 1 });
                }
            }
        }
    }
    for m in w.weapons.iter().flatten() {
        if m.o_class != WEAPON_CLASS { continue; }
        let (rr, nn) = dec_byte(m.bc);
        cmds.push(BoardCmd::Bc { id: m.id, v: nn });
        if rr == 0 || !(dist3f(body, m.pos) < 1.5) { continue; }
        cmds.push(BoardCmd::Bc { id: m.id, v: ticks(300) as u8 });
        // The hand switch to the board weapon: not holding it → 0x141408 = 0x24 and `UpdateWrenchSelected(0)` with
        // 0x1413fc cleared around it (made by the tick right after the hero update: `Gadgets::board_select`); then a
        // weapon (at most 3), the request 0x24 again and its ammo 0x13d4b8 + 1.
        if h.items.slot.id != WEAPON_ITEM {
            h.gadgets.hand_request = WEAPON_ITEM;
            h.gadgets.board_select = true;
        }
        h.board.weapons = (h.board.weapons + 1).min(3);
        h.gadgets.hand_request = WEAPON_ITEM;
        if let Some(a) = h.weapons.ammo.get_mut(WEAPON_ITEM as usize) { *a += 1; }
    }
    h.board.boost_timer += boost;
    h.board.cmds.extend(cmds);
}

/// `0x254058`: the jump's score (with Rilgar's race won).
fn score(h: &mut Hero, c: &mut Ctx) {
    if !world(c.env).is_some_and(|w| w.flag0) { return; }
    let b = &mut h.board;
    if b.score != 0 { b.cmds.push(BoardCmd::MoveBump(30)); }
    for i in 0..3 {
        if 3.316_125_6 < b.spin[i].abs() {
            b.spin[i] += if 0.0 < b.spin[i] { -std::f32::consts::TAU } else { std::f32::consts::TAU };
            b.flips[i] += 1;
        }
    }
    b.max_flips = b.flips.iter().copied().fold(0, i32::max);
    b.flip_score = (0..b.max_flips).map(|k| 100 + 50 * k).sum();
    b.kinds = b.used.iter().filter(|&&u| u != 0).count() as i32;
    b.multiplier = MULTIPLIER[b.kinds as usize];
    let s = ((b.trick_ticks * 100) / 60) as f32;
    b.trick_score = s as i32 + b.kinds * 25;
    b.trick_score = (b.trick_score as f32 * b.multiplier) as i32;
    b.jump_score = b.flip_score + b.trick_score;
}

// ------------------------------------------------------------------------------------------------
// The race 0x254608 and the respawn 0x254358.

/// `0x2a0260(0, pos, path)`: the path point nearest `pos`.
fn nearest(path: &[[f32; 4]], pos: [f32; 4]) -> i32 {
    let pts: Vec<crate::path::Point> = path.iter().map(|q| q.map(f32::to_bits)).collect();
    crate::path::nearest_at_distance(&pts, 0.0, pos)
}

/// The waypoint `i` passed: the next point's segment direction against the direction to `at` within 90°.
fn passed(path: &[[f32; 4]], i: i32, at: [f32; 4]) -> Option<i32> {
    let n = path.len() as i32;
    if n == 0 { return None; }
    let j = (i + n + 1) % n;
    let (a, b) = (pt(path, i), pt(path, j));
    let seg = atan2v([b[0] - a[0], b[1] - a[1]]);
    let to = atan2v([at[0] - a[0], at[1] - a[1]]);
    (diff(seg, to) < HALF_PI).then_some(j)
}

/// `0x254608`: the race (from the physics of 0x6b / 0x6c).
fn race(h: &mut Hero, env: &Env, anim: &mut dyn AnimCtl, rng: &mut Rng) {
    h.board.race_ticks += 1;
    if h.board.moby.is_none() { return; }
    let Some(w) = world(env) else { return };
    if w.pv(0x20) == -1 || w.race.is_empty() { return; }
    h.board.race_line = true;
    let path = &w.race;
    let level = h.idle.level;
    let me = pos4(h);
    if level == 0x10 {
        if dist3f(me, [163.0, 119.0, 80.0, 0.0]) < 11.0 && 0x43 < (h.board.waypoint as u16).wrapping_sub(3) {
            if 2 < h.board.waypoint { h.board.lap += 1; }
            h.board.waypoint = 0xd;
        }
        if dist3f(me, [223.0, 281.0, 93.0, 0.0]) < 7.0 && h.board.waypoint < 0x3b { h.board.waypoint = 0x3b; }
    }
    if let Some(j) = passed(path, h.board.waypoint as i32, me) {
        h.board.waypoint = j as i16;
        if j == 0 { h.board.lap += 1; }
    }
    let mut below = false;
    if level == 0x10 && f(h.pos[2]) < pt(path, h.board.waypoint as i32)[2] - 2.0 {
        below = true;
        if (h.board.wrong_way as i32) < ticks(0x46) { h.board.wrong_way = ticks(0x46) as u8; }
    }
    let n = path.len() as i32;
    let i = h.board.nearest as i32;
    let j = (i + n + 1) % n;
    let (a, b2) = (pt(path, i), pt(path, j));
    let seg = atan2v([b2[0] - a[0], b2[1] - a[1]]);
    if below || (HALF_PI < diff(f(h.rot[2]), seg) && !h.board.trick_mode) {
        h.board.wrong_way = h.board.wrong_way.wrapping_add(1);
        if ticks(0xb4) < h.board.wrong_way as i32 {
            if !h.board.fade.on { h.board.fade = Fade { on: true, speed: 0.08, phase: 0 }; }
            h.board.fading = true;
            if h.board.fade.phase == 1 {
                h.board.fading = false;
                let (pos, rot) = respawn(h, env);
                h.board.yaw = rot[2];
                h.board.wrong_way = 0;
                let mut c = Ctx { env, anim: &mut *anim, rng: &mut *rng, voice: None };
                h.teleport(&mut c, pos, rot, id::RIDE, true);
                let k = nearest(path, pos4(h));
                if (k as i16) < h.board.waypoint { h.board.waypoint = k as i16; }
            }
        }
    } else if !h.board.fading {
        h.board.wrong_way = 0;
    }
    // The racers.
    let count = w.pv(0x40).max(0) as usize;
    for k in 0..count.min(8) {
        let Some(rp) = w.racers.get(k).copied().flatten() else { continue };
        if let Some(j) = passed(path, h.board.racer_wp[k] as i32, rp) {
            h.board.racer_wp[k] = j as i16;
            if j == 0 { h.board.racer_laps[k] += 1; }
        }
    }
    let (lap, wp) = (h.board.lap, h.board.waypoint);
    let ahead = |l: i16, p: i16, l2: i16, p2: i16| l < l2 || (l == l2 && p < p2);
    h.board.place = 1;
    for k in 0..count.min(8) {
        let (rl, rw) = (h.board.racer_laps[k], h.board.racer_wp[k]);
        if lap < rl || (lap == rl && wp <= rw) { h.board.place += 1; }
    }
    for k in 0..count.min(8) {
        let (rl, rw) = (h.board.racer_laps[k], h.board.racer_wp[k]);
        let mut place = if ahead(rl, rw, lap, wp) { 2 } else { 1 };
        for m in 0..count.min(8) {
            if ahead(rl, rw, h.board.racer_laps[m], h.board.racer_wp[m]) { place += 1; }
        }
        h.board.racer_place[k] = place;
    }
    if h.board.lap < 3 { return; }
    // The race's end.
    if w.flag0 { h.board.trial_done = true; }
    let k = if level == 5 { 0 } else { 1 };
    if level == 5 || level == 0x10 { h.board.cmds.push(BoardCmd::Finished(k)); }
    let host = usize::try_from(w.pv(0x28)).ok();
    if level == 0x10 {
        let Some(host) = host else { return };
        if !h.owned.has(31) && h.board.place == 1 {
            h.board.cmds.push(BoardCmd::Bc { id: host, v: 1 });
        } else {
            h.board.host = Some(host);
            h.board.freeze = true;
        }
    } else if level == 5 {
        let limit = ticks(if w.pal { 0x1680 } else { 0x1644 });
        if h.board.race_ticks <= limit && !w.skill[SKILL_TIME] { h.board.cmds.push(BoardCmd::Skill(SKILL_TIME)); }
        let Some(host) = host else { return };
        if !w.flag0 && h.board.place == 1 {
            h.board.cmds.push(BoardCmd::Bc { id: host, v: 1 });
        } else {
            h.board.host = Some(host);
            h.board.freeze = true;
        }
    } else {
        return;
    }
    let host = host.expect("checked above");
    h.board.cmds.push(BoardCmd::Records { k, ticks: h.board.race_ticks, score: h.board.score });
    h.board.cmds.push(BoardCmd::Byte30 { id: host, v: 0xff });
    if h.board.hud != -1 {
        // FUN_0024b090(0x13fbd0, 0): the meter no longer persistent.
        h.board.cmds.push(BoardCmd::Meter(false));
        h.board.meter.on = false;
        h.board.hud = -1;
    }
    h.board.cmds.push(BoardCmd::Hud("update_resource_counter"));
}

/// `0x254358`: the respawn point and rotation (Euler; Ratchet's own without a board).
pub fn respawn(h: &Hero, env: &Env) -> ([f32; 4], [f32; 4]) {
    let me = pos4(h);
    let w = world(env).filter(|_| h.board.moby.is_some());
    let Some(w) = w else { return (me, v4f(h.rot)) };
    let path = &w.spawn;
    let n = path.len() as i32;
    let mut best = 9_999_999.0f32;
    let mut bi = 0;
    let mut i = 0;
    while i < n {
        let q = pt(path, i);
        let a1 = atan2v([q[0] - me[0], q[1] - me[1]]);
        let q2 = pt(path, (i + n + 2) % n);
        let a2 = atan2v([q2[0] - q[0], q2[1] - q[1]]);
        if HALF_PI < diff(a1, a2) {
            let near = if h.idle.level == 0x10 && h.board.race_line {
                let z = pt(&w.race, h.board.nearest as i32)[2].max(pt(&w.race, h.board.waypoint as i32)[2]);
                (z - q[2]).abs() <= 4.0
            } else {
                true
            };
            if near {
                let d = dist2f(me, q);
                if d < best {
                    best = d;
                    bi = i;
                }
            }
        }
        i += 2;
    }
    let q = pt(path, bi);
    let mut pos = q;
    let probe = [q[0], q[1], q[2] + 2.0];
    pos[2] = ground_height(env, 0.5, probe);
    let q1 = pt(path, bi + 1);
    let rot = [0.0, 0.0, atan2v([q1[0] - q[0], q1[1] - q[1]]), 0.0];
    (pos, rot)
}

/// `GroundHeight(up, p, 0)` 0x26e618: the z of the floor under `p` (a line, flags 2, from `p.z + up` down to 0.01; 0
/// without one).
fn ground_height(env: &Env, up: f32, q: [f32; 3]) -> f32 {
    let a = from_f32x3([q[0], q[1], q[2] + up]);
    let b = from_f32x3([q[0], q[1], f32::from_bits(0x3c23_d70a)]);
    env.line(a, b, 2).map_or(0.0, |o| o.point[2])
}

/// `0x24bdc0`'s tail in groups 0x15 / 0x16: the board at Ratchet's feet + rows·(0, 0, 0.21) turned by record 0's
/// angles, its rows Ratchet's turned by them (queued for the tick).
pub(super) fn carry(h: &mut Hero) {
    if !matches!(h.group, 0x15 | 0x16) || h.board.moby.is_none() { return; }
    let e = h.idle.joints[joint::REC0].cur;
    let m = crate::moby_update::services::euler_rows([p(e[0]), p(e[1]), p(e[2]), Pf::ZERO]).map(v4f);
    let r = h.moby_rows.map(v4f);
    let up = [0.0, 0.0, f32::from_bits(0x3e57_0a3d), 0.0];
    let v: [f32; 3] = std::array::from_fn(|k| up[0] * r[0][k] + up[1] * r[1][k] + up[2] * r[2][k]);
    let v: [f32; 3] = std::array::from_fn(|k| v[0] * m[0][k] + v[1] * m[1][k] + v[2] * m[2][k]);
    let pos = pos4(h);
    let pos = [pos[0] + v[0], pos[1] + v[1], pos[2] + v[2], pos[3]];
    let rows: [[f32; 4]; 3] = std::array::from_fn(|i| std::array::from_fn(|k| if k == 3 { 0.0 } else { m[0][k] * r[i][0] + m[1][k] * r[i][1] + m[2][k] * r[i][2] }));
    h.board.carry = Some((pos, rows));
}

/// `HeroPdaGadget` case 0x24 (level16 `0x223350`): the board weapon. While weapons are held and none has been fired
/// yet (0x141920; 0x141d80 is 0 [L]), the hold timer 0x13fc1a counts and past `ticks(20)·60` asks for the hint
/// `Help_Request(0x3e81, 0x83)`. With the hand moby: the cooldown 0x13fc1f steps, and its end spends a weapon; then
/// with a weapon left, the cooldown out and ○ (the slot's fire mask) pressed within `ticks(7)`: the target is the
/// racer (the board's +0x2c.. words, −1 skipped) within 45° of Ratchet's yaw and 45° of level, by the least
/// `d·(1 + 2·yaw off + 2·pitch off)` (d the xy distance; the first one kept on a tie); the item's +0xbc = `ticks(7)`;
/// the cooldown `ticks(50)`; the shots + 1 (unless −1); the play-time and per-level weapon-use stats (0x141922 /
/// 0x141924) are not kept [L]; then the hand item's class sound 0 and the missile `0x2a3e30` at its joint list 0
/// ([`weapon_update`], the item's update with the table, the same tick).
pub fn weapon_fire(h: &mut Hero, c: &mut Ctx) {
    let b = &mut h.board;
    if b.weapons != 0 && b.shots == 0 {
        b.weapon_hold = b.weapon_hold.wrapping_add(1);
        if ((ticks(0x14) as f32 * 60.0) as i32) < b.weapon_hold as i32 { b.cmds.push(BoardCmd::Help { msg: 0x3e81, rec: 0x83 }); }
    }
    if h.items.slot.item.is_none() { return; }
    let (r, n) = dec_byte(h.board.weapon_lock);
    h.board.weapon_lock = n;
    if r == 2 {
        if h.board.weapons == 0 { return; }
        h.board.weapons -= 1;
    }
    if h.board.weapons == 0 || h.board.weapon_lock != 0 { return; }
    if c.env.pad.pressed_within(h.items.slot.fire_mask, ticks(7)).is_none() { return; }
    let me = to_f32x3(h.pos);
    let yaw = h.rot[2].to_f32();
    let mut best = 9_999_999.0f32;
    let mut target = None;
    for (id, p) in world(c.env).map(|w| w.aim).unwrap_or_default().into_iter().flatten() {
        let a = fast_arctan(Pf::f(p[0] - me[0]), Pf::f(p[1] - me[1]));
        let yd = fast_diff_rots(Pf::f(yaw), a).to_f32();
        if !(yd <= std::f32::consts::FRAC_PI_4) { continue; }
        let d = ((me[0] - p[0]).powi(2) + (me[1] - p[1]).powi(2)).sqrt();
        let pa = fast_arctan(Pf::f(d), Pf::f(p[2] - me[2]));
        let pd = fast_diff_rots(pa, Pf::ZERO).to_f32();
        if !(pd <= std::f32::consts::FRAC_PI_4) { continue; }
        let d = ((p[0] - me[0]).powi(2) + (p[1] - me[1]).powi(2)).sqrt();
        let score = d + d * yd + d * yd + d * pd + d * pd;
        if target.is_some() && best <= score { continue; }
        best = score;
        target = Some(id);
    }
    h.gadgets.item_bc = ticks(7) as u8;
    h.board.weapon_lock = ticks(0x32) as u8;
    if h.board.shots != -1 { h.board.shots += 1; }
    h.board.shot = Some(Shot { yaw, pitch: h.rot[1].to_f32(), target });
}

/// The board weapon's item update (item 0x24, in the slot loop): the shot [`weapon_fire`] made this tick: the item's
/// class sound 0 and the missile (`0x2a3e30`, `units::board_missile`) at the item's joint list 0.
pub fn weapon_update(hero: &mut Hero, table: &mut crate::moby_runtime::MobyTable, _anim: &dyn AnimCtl, env: &super::items::ItemEnv, hits: &mut dyn super::items::HitSink, _rng: &mut crate::rng::Rng) {
    let Some(s) = hero.board.shot.take() else { return };
    hero.fx.item_sounds.push(0);
    let at = super::guns::item_point(hero, env, 0);
    crate::moby_update::classes::units::board_missile::spawn(table, hits, env.frame as u64, (s.yaw, s.pitch), env.hero_moby, at, s.target);
}
