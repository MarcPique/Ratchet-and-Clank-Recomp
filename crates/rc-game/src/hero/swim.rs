//! Swimming: the hero's water states, ported once from level01 and driven by the level's water data only.
//! Spec: `docs/plan/player_controller.md` §14 "Swimming".
//!
//! | id | group | what | entry anim | physics / transitions |
//! |---|---|---|---|---|
//! | 0x37 | 0x12 | surface, treading water | seq 0x3a, blend 22 | [`Hero::phys_surface`] / [`Hero::tr_surface_idle`] |
//! | 0x36 | 0x12 | surface, swimming | seq 0x65, blend 17 | [`Hero::phys_surface`] / [`Hero::tr_surface_swim`] |
//! | 0x33 | 0x11 | underwater, stroking | seq 0x3b, blend 12 | [`Hero::phys_underwater`] / [`Hero::tr_underwater`] |
//! | 0x34 | 0x11 | underwater, drifting | seq 0x3c, blend 35 (curve −2 after 0x35) | same |
//! | 0x35 | 0x11 | underwater, Hydro-Pack thrust (R1/R2 held) | seq 0x3d, blend 11 (15 after 0x33) | same |
//! | 0x12 | 4 | jump out of the water | seq 0x72, curve −1, frame 1 | the jump group (`jump.rs`) |
//! | 0x73 | 1 | wading (depth 0.25..0.85) | seq 0x60, blend 8 | the walk (state 2, `walk.rs`) |
//! | 0x6a | 0x14 | drowned (no air left) | seq 0x5f, blend 10 | [`Hero::phys_drown`] / [`Hero::tr_drown`] |
//!
//! **Where the water comes from.** Only the level data: the collision faces with surface id 0 (the ground probe
//! `0x232dc0` hits them with flags 2, records the level and re-casts with 0x24 through them to the floor), and
//! the level's water-height tables (`0x26ed38`: the active patch of the level's ripple module under the hit (751 on
//! Novalis, the patch managers of 05 / 07 / 11 / 12 / 13), else the flat plane `0x1612dc..ec` (level 05's class 982),
//! else the hit's own z) through [`WaterQuery`] (`crate::water::world::WaterWorld::water_height`). The moving water
//! of the patch managers is their mobys' collision (surface-0 faces at the moby's z), so the probe finds it like
//! any water face. Nothing here knows a level, a pool or an
//! object; the surface reaction `0x22cd48` turns the probe's result into the per-tick water flags (0x140634 in
//! water, 0x1415f4 depth = level − floor, 0x1413f9 wading) that every rule below reads.
//!
//! **Arithmetic.** The shared hero primitives (`Spring`/`Approach`/`TurnTo`/`SpeedStep`/`StickTarget`, the wall
//! check, the momentum decay) are the existing ported helpers; the swim-specific formulas are written in
//! standard `f32` and stored back into the hero block's fields ([`Pf`]) with [`Pf::f`]. No hardware modelling
//! is added.
//!
//! **Effects** ([`effects`]): the splashes, wakes, rings, spray, bubbles and breath bubbles at the game's call points,
//! with their random draws there (docs/plan/hero_states.md "Swim effects"); the ripple disturbances and the splash
//! records go to the engine as [`SwimEvent`]s; the voices play at their call points ([`super::states::Ctx::voice`]).
//!
//! **The lean** ([`Hero::swim_lean`], 0x33..0x35): the neck and feet joint records from the roll and the pitch change.
//!
//! **The oxygen meter** (`0x2406b0`: `queue_animation_update(4, …)` every tick under water without the O2 Mask) is
//! `crate::hud`'s, derived each tick from [`Swim::oxygen`] and the hero's group (`hud::HudState::tick`'s `frame_calls`).
//!
//! **Not ported**: the ✕-tap stats records and the water currents (classes 613 / 679: they write 0x13f528 and push the
//! hero).
#![allow(clippy::neg_cmp_op_on_partial_ord)] // FPU-style compare order kept from the original.

use super::anim::AnimCtl;
use super::physics::*;
use super::states::Ctx;
use super::Hero;
use crate::pad::button;
use crate::ps2v::Pf;
use crate::rng::Rng;

pub mod effects;

/// The level's water-height tables (`0x26ed38` minus its last fallback): the height of the water surface at
/// `p` (a point on a surface-0 face) from the ripple module's patches or the flat water plane, `None` when no
/// table covers `p` (the caller then uses the face's own z, as the game does).
pub trait WaterQuery {
    fn water_height(&self, p: [f32; 3]) -> Option<f32>;
}

/// The ripple module alone (`RippleHeightQuery` 0x2b8910 through `0x26ed38`).
impl WaterQuery for crate::water::RippleSim {
    fn water_height(&self, p: [f32; 3]) -> Option<f32> { self.patch_height(p[0], p[1], p[2]) }
}

/// The level's water (`SetWaterLevel` 0x26ed38: the ripple module, then the flat plane).
impl WaterQuery for crate::water::world::WaterWorld {
    fn water_height(&self, p: [f32; 3]) -> Option<f32> { crate::water::world::WaterWorld::water_height(self, p) }
}

/// Hero water state ids (`0x1413d4`).
pub mod id {
    pub const WATER_JUMP: i32 = 0x12;
    pub const UNDERWATER: i32 = 0x33;
    pub const UNDERWATER_IDLE: i32 = 0x34;
    pub const HYDRO: i32 = 0x35;
    pub const SURFACE_SWIM: i32 = 0x36;
    pub const SURFACE_IDLE: i32 = 0x37;
    pub const DROWN: i32 = 0x6a;
    pub const WADE: i32 = 0x73;
}

/// Item ids of the game state's owned table (0x13d4c0 + id).
pub const ITEM_HYDRO_PACK: usize = 4;
pub const ITEM_O2_MASK: usize = 6;
/// `0x1415f0`: a full breath.
pub const OXYGEN_MAX: i32 = 10000;
/// The stroke-speed curve at 0x17c440, indexed by Ratchet's swim frame (`0x22a718(0x17c440, 30)`).
pub const STROKE: [f32; 30] = [
    0.4, 0.4, 0.45, 0.52, 0.6, 0.68, 0.78, 0.89, 1.0, 1.0, 1.0, 1.0, 0.95, 0.9, 0.85, 0.8, 0.75, 0.7, 0.65, 0.6,
    0.55, 0.5, 0.45, 0.4, 0.4, 0.4, 0.4, 0.4, 0.4, 0.4,
];
/// The deep-water jump's vertical script at 0x17c3e0: rows (acceleration ·dt² set at the row start, or
/// −999999 = keep; increment ·dt² per further tick; ticks).
pub const WATER_JUMP_CURVE: [(f32, f32, i32); 8] =
    [(-27.0, 0.0, 9), (40.0, 0.0, 1), (55.0, 0.0, 1), (75.0, 0.0, 1), (90.5, 0.0, 9), (75.0, 0.0, 4), (40.0, 0.0, 3), (0.0, 0.0, 100)];

/// Something the swim code wants shown or heard (the engine drains [`Swim::events`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SwimEvent {
    /// `RippleDisturb(x, y, radius, amplitude, patches, count, 0)` (0x2b82a8) on the level's ripple patches.
    Ripple { x: f32, y: f32, r: f32, amp: f32 },
    /// A splash `0x22b3a8(rings, drops, big)` at the hero on the water level.
    Splash { rings: i32, drops: i32, big: bool },
    /// Hero sound `0x236738(id)` (3 = splash in, 0x11 = landing in water), queued for `super::fx::flush` (a SetState
    /// from outside the hero update).
    Sound(i32),
    /// The same, played at its call point ([`super::states::Ctx::voice`]); a record only.
    Played(i32),
    /// Queued voice `0x236810(id, delay)` (7 / 8 = gasps after a long dive).
    Voice(i32, i32),
    /// `0x2319b0` from the drowned state: deaths++, fade to black, respawn.
    Drowned,
    /// `0x167494 = 0` (`0x2406b0`, the surface jump out of the water into state 6): the camera's underwater flag
    /// cleared before its next test. The engine stores it as `water::world::UnderwaterStore::Off` into
    /// `WaterWorld::underwater_store` with the tick's counter (G-REN-032).
    UnderwaterOff,
}

/// The hero block's swim fields (level01 addresses; boot .bss, same in every level).
#[derive(Clone, Debug, Default)]
pub struct Swim {
    /// 0x1415f0: air left, 0..10000 (drains 11 per tick under water without the O2 mask: 15 s).
    pub oxygen: i32,
    /// 0x13f9e0 / 0x13f9e4: surface bob offset and its velocity.
    pub bob: Pf,
    pub bob_vel: Pf,
    /// 0x13f9e8: surface stroke timer (the swim keeps going this long after entering 0x36).
    pub stroke_timer: i32,
    /// 0x13f9ec / 0x13f9f0: the smoothed water level the hero floats on, and its spring velocity.
    pub level: Pf,
    pub level_vel: Pf,
    /// 0x13fc30: ticks of the dive-in (substate 1).
    pub dive_ticks: i32,
    /// 0x13fc34 / 0x13fc38: roll / pitch spring velocities; 0x13fc3c: the yaw velocity copy (the roll input).
    pub roll_vel: Pf,
    pub pitch_vel: Pf,
    pub yaw_rate: Pf,
    /// 0x13fc44: underwater timer (set on entry, only counted down).
    pub f44: i32,
    /// 0x13fc4c: sink / surface-spring velocity (0x6a sinks at −1.5·dt; 0x36 springs the bob with it).
    pub sink: Pf,
    /// 0x13fc5a: splash countdown (bubbles while > 0).
    pub splash: i16,
    /// 0x13f528 / 0x13f52e: water-current locks on the surface jump / dive (only currents set them).
    pub jump_lock: i16,
    pub dive_lock: i16,
    /// 0x13f3f0 / 0x13f3f4: Euler x / y spring velocities of the straightening `0x236520`.
    pub euler_vel: [Pf; 2],
    /// 0x13fc54 / 0x13fc56 / 0x13fc58: the tick counters of the wake `0x22ac40`, the spray `0x22af48` and the bow
    /// rings `0x22ad38` ([`effects`]).
    pub fx_counters: [i16; 3],
    /// 0x13fc40: ticks to the next breath bubble ([`effects::breath_underwater`]).
    pub breath: i32,
    /// 0x13fc48: ticks to the Hydro-Pack's next jet bubbles ([`effects::jets`]).
    pub jets: i16,
    /// Effects and sounds for the engine.
    pub events: Vec<SwimEvent>,
}

impl Swim {
    pub fn new() -> Swim { Swim { oxygen: OXYGEN_MAX, ..Default::default() } }
}

fn f(x: Pf) -> f32 { x.to_f32() }
fn p(x: f32) -> Pf { Pf::f(x) }
const DTF: f32 = 1.0 / 60.0;
const DT2F: f32 = 1.0 / 3600.0;
/// ✕ / □ / R1|R2.
const X: u32 = button::CROSS;
const SQ: u32 = button::SQUARE;
const R12: u32 = button::CROUCH;
/// 78°: the full dive / climb pitch.
const PITCH_MAX: f32 = 1.361_356_9;

/// `0x277b50(s, yaw, pitch, &v)`: the 3-D velocity along yaw / pitch.
fn sph(s: f32, yaw: f32, pitch: f32) -> [f32; 3] { [yaw.cos() * s * pitch.cos(), yaw.sin() * s * pitch.cos(), pitch.sin() * s] }

fn wrap(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    let r = (a + std::f32::consts::PI).rem_euclid(t) - std::f32::consts::PI;
    if r < -std::f32::consts::PI { r + t } else { r }
}

impl Hero {
    fn water_depth(&self) -> f32 { f(self.f15f4) }

    /// `0x236738(id, 0)` from SetState / the transitions: at the call point when the update gave a player, else queued.
    fn swim_voice(&mut self, c: &mut Ctx, id: i32) {
        let e = if c.voice(id, 0) { SwimEvent::Played(id) } else { SwimEvent::Sound(id) };
        self.swim.events.push(e);
    }

    /// The dive state a □ / R1 press picks: 0x35 with the Hydro-Pack and R1|R2 held, else 0x33.
    fn dive_state(&self, held: u32) -> i32 { if self.owned.has(ITEM_HYDRO_PACK) && held & R12 != 0 { id::HYDRO } else { id::UNDERWATER } }

    /// `0x22a718(0x17c440, 30)`: the stroke factor at Ratchet's frame readout: `frac·T[i] + (step − frac)·T[i−1]`
    /// (`T[29]` right after a wrap), `step` = how far the readout moved this tick.
    fn stroke(anim: &dyn AnimCtl) -> f32 {
        let v = anim.view();
        let n = STROKE.len() as i32;
        let i = (v.frame as i32).clamp(0, n - 1);
        let fr = v.frame - v.frame.trunc();
        let prev = if v.flags & 2 != 0 { n - 1 } else { (i + n - 1) % n };
        fr * STROKE[i as usize] + (v.frame_step - fr) * STROKE[prev as usize]
    }

    // --------------------------------------------------------------------------------------------
    // The water checks at the top of the transitions (after the weapon check).

    /// `0x2408e8`: enter the water. From the air or the ground: the feet 0.8 below the water level (a jump only
    /// once descending), or a fall reaching the level with ≥ 0.8 of water below; from under water: rising
    /// within 0.4 of the level; a jump out that falls back 0.7 below. Every 16 ticks outside the water groups a
    /// line from 4 (16 every 64 ticks) above the feet down to 1.3 above them finds water with nothing solid
    /// under its surface: the hero stands submerged → 0x34. Returns whether the state changed.
    pub fn water_entry_check(&mut self, c: &mut Ctx) -> bool {
        if self.state == id::DROWN { return false; }
        // The Hoverboard's groups and crash states never enter the water this way (levels 5 / 16: `0x2515d0`).
        if matches!(self.group, 0x15 | 0x16) || matches!(self.state, 0x6d..=0x6f) { return false; }
        let (w, z) = (f(self.water_level), f(self.pos[2]));
        let dz = f(self.disp[2]);
        let descending = self.jump.descending != 0;
        let mut enter = false;
        if self.f0634 != 0 {
            if self.group == 0x11 {
                if self.substate != 1 && 0.0 < dz && w - 0.4 < z { enter = true; }
            } else if self.group != 0x12 && self.group != 3 {
                let a = f(self.vel[2]).abs() + 0.07;
                let lim = if 0.2 < a { a } else { 0.2 };
                let ok_group = self.group != 4 || descending;
                if (w - (z + 0.45)).abs() < lim && ok_group && 0.8 < self.water_depth() && dz < 0.0 { enter = true; }
                if z < w - 0.8 && ok_group { enter = true; }
            }
        }
        if self.state == id::WATER_JUMP && descending && z < w - 0.7 { enter = true; }
        let counter = self.idle.counter.wrapping_sub(1);
        if !(0x11..=0x12).contains(&self.group) && self.group != 7 && self.group != 0x14 && self.state != id::WATER_JUMP && counter & 0xf == 0 {
            let up = if counter & 0x3f == 0 { 16.0 } else { 4.0 };
            let mut a = self.pos;
            a[2] = p(z + up);
            let mut b = self.pos;
            b[2] = p((z + 1.3).max(0.0));
            if let Some(o) = c.env.line(a, b, 2) {
                self.surface_id = o.surface_id() as i16;
                if self.surface_id == 0 {
                    a[2] = p(o.point[2] - 0.01);
                    if c.env.line(a, b, 2).is_none() {
                        self.set_state(c, id::UNDERWATER_IDLE, true);
                        return true;
                    }
                }
            }
        }
        if enter {
            self.set_state(c, id::SURFACE_IDLE, true);
            self.swim_voice(c, 3);
        }
        enter
    }

    /// `0x2406b0`: under water the air drains (not with the O2 mask); at none left the hero drowns unless he is
    /// near the surface, pitched up and rising; 0.4 above the level he falls out (state 6). On the surface, a
    /// solid face just above the water (a line from level + 0.01 to + 0.31) pushes him under.
    pub fn underwater_check(&mut self, c: &mut Ctx) -> bool {
        if self.group == 0x11 {
            let drain = if self.owned.has(ITEM_O2_MASK) { 0 } else { 10000 / (ticks(60) * 15) };
            self.swim.oxygen = (self.swim.oxygen - drain).max(0);
            let (w, z) = (f(self.water_level), f(self.pos[2]));
            if self.swim.oxygen == 0 {
                let saved = w - 2.0 < z && f(self.rot[1]) < -0.872_664_6 && DTF < f(self.disp[2]);
                if !saved {
                    self.set_state(c, id::DROWN, true);
                    return true;
                }
            }
            if w + 0.4 < z {
                // `0x167494 = 0` before the SetState (the camera is out of the water with him).
                self.swim.events.push(SwimEvent::UnderwaterOff);
                self.set_state(c, 6, true);
                return true;
            }
        } else if self.state != id::DROWN && self.state != 0x76 {
            self.swim.oxygen = OXYGEN_MAX;
        }
        if self.group != 0x12 { return false; }
        let mut a = self.pos;
        a[2] = p(f(self.water_level) + 0.01);
        let mut b = a;
        b[2] = p(f(a[2]) + 0.3);
        if c.env.line(a, b, 2).is_none() { return false; }
        let s = self.dive_state(c.env.pad.held);
        self.set_state(c, s, true);
        true
    }

    // --------------------------------------------------------------------------------------------
    // SetState 0x23cf98 for the water states.

    /// The SetState cases 0x33..0x37 and 0x6a (0x12 is in the jump entry, 0x73 in the walk entry).
    pub fn swim_entry(&mut self, c: &mut Ctx, sid: i32, play: bool) {
        let blend = |n: i32| Pf::from_i32(ticks(n));
        match sid {
            id::UNDERWATER | id::HYDRO => {
                self.group = 0x11;
                self.f15d4 = 0xb;
                self.items.f13f7 = 1;
                if sid == id::UNDERWATER { self.swim.sink = Pf::ZERO; }
                if self.prev_group == 0x12 {
                    self.swim_voice(c, 3);
                    self.substate = 1;
                } else if sid == id::UNDERWATER {
                    self.swim.f44 = ticks(45);
                }
                if sid == id::HYDRO { self.swim.f44 = ticks(60); }
                self.idle.blink_period = 0x68;
                self.swim.dive_ticks = 0;
                if play {
                    if sid == id::UNDERWATER {
                        self.set_anim(c.anim, c.rng, blend(12), 0x3b, 0);
                    } else {
                        let b = if self.prev_state == id::UNDERWATER { 15 } else { 11 };
                        self.set_anim(c.anim, c.rng, blend(b), 0x3d, 0);
                    }
                }
            }
            id::UNDERWATER_IDLE => {
                self.group = 0x11;
                self.f15d4 = 0xb;
                self.items.f13f7 = 1;
                self.momentum = self.eff;
                self.idle.blink_period = 0x68;
                let b = if self.prev_state == id::HYDRO { Pf::b(0xc000_0000) } else { blend(35) };
                if play { self.set_anim(c.anim, c.rng, b, 0x3c, 0); }
            }
            id::SURFACE_SWIM => {
                self.items.f13f7 = 1;
                self.group = 0x12;
                self.f15d4 = 0;
                self.swim.sink = Pf::ZERO;
                self.swim.stroke_timer = ticks(45);
                self.idle.blink_period = 0x68;
                if play { self.set_anim(c.anim, c.rng, blend(17), 0x65, 0); }
            }
            id::SURFACE_IDLE => {
                self.group = 0x12;
                self.items.f13f7 = 1;
                self.f15d4 = 0;
                self.swim.splash = 0;
                let (x, y) = (f(self.pos[0]), f(self.pos[1]));
                let dz = f(self.disp[2]);
                if self.prev_group == 0x11 {
                    // Surfacing (0x23d654): 0x22b3a8(3, 10, 0), RippleDisturb(x, y, 0.4, 0.3).
                    effects::splash(self, c.rng, 3, 10, false);
                    self.swim.events.push(SwimEvent::Splash { rings: 3, drops: 10, big: false });
                    self.swim.events.push(SwimEvent::Ripple { x, y, r: 0.4, amp: 0.3 });
                } else if dz < DTF * -0.5 {
                    // Falling in (0x23d6f0): 0x22b3a8(3, min(trunc(300·|dz|), 40), 1), RippleDisturb(x, y, 0.5, −0.4).
                    let n = ((dz.abs() * 300.0) as i32).min(40);
                    effects::splash(self, c.rng, 3, n, true);
                    self.swim.events.push(SwimEvent::Splash { rings: 3, drops: n, big: true });
                    self.swim.events.push(SwimEvent::Ripple { x, y, r: 0.5, amp: -0.4 });
                    self.swim.splash = ticks(75) as i16;
                }
                self.swim.stroke_timer = 0;
                if self.prev_group == 0x11 {
                    // The gasp is queued twice (the game repeats the block; the second one waits 40 ticks).
                    let o = self.swim.oxygen as f32;
                    for d in [27, 40] {
                        if o < 2500.0 { self.swim.events.push(SwimEvent::Voice(8, ticks(30))); } else if o < 7500.0 { self.swim.events.push(SwimEvent::Voice(7, ticks(d))); }
                    }
                }
                self.swim.bob_vel = p((dz * 0.37).max(DTF * -7.0));
                self.swim.level = self.water_level;
                self.swim.level_vel = Pf::ZERO;
                self.swim.bob = p((f(self.pos[2]) - f(self.water_level)) + 0.12);
                self.momentum = [self.eff[0], self.eff[1], Pf::ZERO, self.eff[3]];
                super::states::clamp_len_2745f0(&mut self.momentum, DT * Pf::f(5.5));
                self.idle.blink_period = 0x68;
                if play { self.set_anim(c.anim, c.rng, blend(22), 0x3a, 0); }
            }
            id::DROWN => {
                self.group = 0x14;
                self.items.f13f7 = 1;
                self.f15d4 = 0;
                self.health = 0;
                self.momentum = self.eff;
                if play { self.set_anim(c.anim, c.rng, blend(10), 0x5f, 0); }
                // Group 0x14 (SetState's tail): the hand item is put away, 0x1413fc = 1.
                self.items.f13fc = 1;
            }
            _ => {}
        }
    }

    // --------------------------------------------------------------------------------------------
    // Per-state physics 0x2370b8.

    /// 0x33 / 0x34 / 0x35: 3-D swimming. □ (or the dive-in, substate 1) pitches down to 78°, ✕ up (full
    /// pitch when the air is short), the pitch rate follows the button pressure (40..70°/s); the stick
    /// steers the yaw (TurnTo 0.007 / 0.08 / 300°/s) and sets the speed: 3 u/s stroking (0x33, pulsed by the
    /// stroke curve ×5), 6 u/s on the dive-in, the Hydro-Pack 7 u/s with R1|R2 (2.8 u/s × stick without);
    /// 0x34 drifts on its momentum. The body rolls into turns.
    pub fn phys_underwater(&mut self, env: &Env, anim: &dyn AnimCtl, rng: &mut Rng) {
        let s = self.state;
        let pad = env.pad;
        // The Hydro-Pack's loop (0x236798(5, Ratchet, 0x13)); out of 0x35 slot 5 is released while still Ratchet's.
        if s == id::HYDRO { super::packs::loop_sound(self, 5, 0x13); } else { super::packs::release_loop(self, 5); }
        // The splash countdown 0x13fc5a: FastDecTimer, then min(t / 4, 8) bubbles at the feet.
        if self.swim.splash != 0 {
            self.swim.splash = self.swim.splash.max(1) - 1;
            let n = ((self.swim.splash >> 2) as i32).min(8);
            super::fx::bubbles(self, rng, n);
        }
        let v = anim.view();
        // Stroking: four bubbles at the hands (lists 0 / 0xe) in the first 10 frames of the stroke (not blending).
        if s == id::UNDERWATER && !v.blending() && 0.0 < v.frame && v.frame < 10.0 { super::fx::bubbles_at(self, rng, Some(anim), 4, 1); }
        // Drifting right after the Hydro-Pack: a thinning trail at the hands for 15 ticks.
        if s == id::UNDERWATER_IDLE && self.prev_state == id::HYDRO && self.timer < ticks(15) {
            super::fx::bubbles_at(self, rng, Some(anim), (ticks(20) - self.timer) / 3 + 1, 1);
        }
        if self.substate == 1 {
            if self.timer == 1 {
                let (x, y) = (f(self.pos[0]), f(self.pos[1]));
                // The dive-in: 0x22b3a8(3, 16, 0), RippleDisturb(x, y, 0.4, −0.3), splash countdown 70.
                effects::splash(self, rng, 3, 16, false);
                self.swim.events.push(SwimEvent::Splash { rings: 3, drops: 16, big: false });
                self.swim.events.push(SwimEvent::Ripple { x, y, r: 0.4, amp: -0.3 });
                self.swim.splash = ticks(70) as i16;
            }
            self.swim.dive_ticks += 1;
            if ticks(40) < self.swim.dive_ticks { self.substate = 0; }
        }
        // Pitch target (DualShock 2 pressure mode 0x79: the rate and the depth of the dive follow the pressure).
        let (px, psq) = (f(pad.pressure[6]), f(pad.pressure[7]));
        let pr = if psq <= px { px } else { psq };
        let rate = DTF * 0.698_131_7 + (DTF * 1.221_730_5 - DTF * 0.698_131_7) * pr;
        let mut tgt = 0.0f32;
        if pad.held & (X | SQ) == SQ || self.substate == 1 {
            tgt = (psq * 1.5).clamp(0.2, 1.0);
            if self.substate == 1 && tgt < 0.35 { tgt = 0.35; }
            tgt *= PITCH_MAX;
        } else if pad.held & X != 0 {
            tgt = (px * 1.5).clamp(0.2, 1.0);
            if (self.swim.oxygen as f32) < 1500.0 { tgt = 1.0; }
            tgt *= -PITCH_MAX;
        }
        if s == id::UNDERWATER_IDLE { tgt = 0.0; }
        let (mut pitch, mut pv) = (self.rot[1], self.swim.pitch_vel);
        let pitch0 = pitch;
        turn_spring(p(tgt), Pf::f(0.015), Pf::f(0.2), p(rate), &mut pitch, &mut pv, 0);
        // 0x237f18: this tick's pitch change (`fast_subtract_rotations`), the lean's input below.
        let dpitch = f(fast_subtract_rotations(pitch, pitch0));
        self.rot[1] = pitch;
        self.swim.pitch_vel = pv;
        self.stick_target(env, Pf::ONE);
        self.turn_to(Pf::f(0.007), Pf::f(0.08), p(DTF * 5.235_987_7));
        self.swim.yaw_rate = self.yaw_vel;
        if self.swim.f44 != 0 { self.swim.f44 -= 1; }
        // Target speed.
        let r_held = pad.held & R12 != 0;
        let mut ts = if self.substate == 1 {
            6.0 * DTF
        } else if s != id::HYDRO {
            3.0 * DTF
        } else if !r_held {
            DTF * 7.0 * 0.4
        } else {
            DTF * 7.0
        };
        let stick = f(self.stick_mag);
        let mut k = stick;
        if (pad.held & (X | SQ) != 0 || self.substate == 1) && stick < 1.0 { k = 1.0; }
        if 0.15 < stick {
            let m = if s == id::HYDRO { 0.4 } else { 0.35 };
            if k < m { k = m; }
        }
        if s != id::HYDRO || !r_held { ts *= k; }
        self.target_speed = p(ts);
        // The breath bubbles 0x13fc40 (at the mouth, joint list 4), then the Hydro-Pack's jets 0x13fc48.
        effects::breath_underwater(self, rng, anim);
        if s == id::HYDRO { effects::jets(self, rng); }
        if s == id::HYDRO { self.speed_step(p(DT2F * 13.0), p(DT2F * 8.0)); } else { self.speed_step(p(DT2F * 7.0), p(DT2F * 4.0)); }
        let yaw = f(self.rot[2]);
        let down = wrap(-f(self.rot[1]));
        let speed = f(self.speed);
        match s {
            id::HYDRO => self.set_vel3(sph(speed, yaw, down)),
            id::UNDERWATER => {
                let mut v = speed * Self::stroke(anim) * 5.0;
                if self.prev_state == id::HYDRO && self.timer < ticks(15) && v < DTF * 4.5 { v = DTF * 4.5; }
                self.set_vel3(sph(v, yaw, down));
            }
            _ => {
                let m = [f(self.momentum[0]), f(self.momentum[1]), f(self.momentum[2])];
                let l = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt();
                let dec = if self.prev_state == id::HYDRO { 8.0 } else { 4.0 } * DT2F;
                let nl = (l - dec).max(0.0);
                let q = if 0.0 < l { nl / l } else { 0.0 };
                self.momentum = [p(m[0] * q), p(m[1] * q), p(m[2] * q), self.momentum[3]];
                self.vel = self.momentum;
            }
        }
        self.wall_check(env, 0);
        // Roll into the turn.
        let roll = f(self.swim.yaw_rate) * down.cos() * -7.0;
        let (mut r, mut rv) = (self.rot[0], self.swim.roll_vel);
        turn_spring(p(roll), Pf::b(0x3be5_6042), Pf::b(0x3e2e_147b), p(DTF * 1.221_730_5), &mut r, &mut rv, 0);
        self.rot[0] = r;
        self.swim.roll_vel = rv;
        self.swim_lean(roll, dpitch);
    }

    /// The swim lean (0x238634..0x2386f8, the joint records 0x17ab00: `crate::hero::idle`): the springs of records 1 / 2
    /// / 3 (`0x22b5e0(0.009, 0.22)`, `0x22b5f8(0.04, 0.2)`, `0x22b610(0.02, 0.2)`); record 1 (the neck, list 10) leans
    /// by the roll target and the pitch change: (1.4·roll, 47·Δpitch within ±50°, 1.5·roll); the feet (records 4 / 5)
    /// kick with (0, 40·Δpitch, 1.1·roll) on springs (0.03, 0.22).
    pub(super) fn swim_lean(&mut self, roll: f32, dpitch: f32) {
        use super::idle::joint::{FOOT_L, FOOT_R, HEAD, NECK, REC2};
        let j = &mut self.idle.joints;
        let kd = |b: u32| f32::from_bits(b);
        (j[NECK].k, j[NECK].d) = (kd(0x3c13_74bc), kd(0x3e61_47ae));
        (j[REC2].k, j[REC2].d) = (kd(0x3d23_d70a), kd(0x3e4c_cccd));
        (j[HEAD].k, j[HEAD].d) = (kd(0x3ca3_d70a), kd(0x3e4c_cccd));
        let lim = kd(0x3f5f_66f3);
        j[NECK].target[1] = (dpitch * 47.0).clamp(-lim, lim);
        j[NECK].target[2] = roll * 1.5;
        j[NECK].target[0] = roll * kd(0x3fb3_3333);
        for k in [FOOT_L, FOOT_R] {
            j[k].target[1] = dpitch * 40.0;
            j[k].target[2] = roll * kd(0x3f8c_cccd);
            (j[k].k, j[k].d) = (kd(0x3cf5_c28f), kd(0x3e61_47ae));
        }
    }

    fn set_vel3(&mut self, v: [f32; 3]) {
        self.vel[0] = p(v[0]);
        self.vel[1] = p(v[1]);
        self.vel[2] = p(v[2]);
    }

    /// 0x36 / 0x37: on the surface. The stick sets the target (3 u/s × stick, at least 1.5 u/s while a direction
    /// is held; 3 u/s during the first 45 ticks of 0x36 without one; 0 when the turn is over 45°), TurnTo
    /// (0.007, 0.08, 300°/s), SpeedStep (4, 5)·dt², velocity = speed × stroke curve × 5.5 along the yaw, flat;
    /// 0x37 stops. The momentum decays by 4.2·dt², then the bob `0x240c78` sets the height.
    pub fn phys_surface(&mut self, env: &Env, anim: &dyn AnimCtl, rng: &mut Rng) {
        // The effects at the top of the case (0x238a24..): treading water leaves the wake 0x22ac40(15, 30); moving
        // faster than 1.5 u/s throws spray 0x22af48(4, 12); swimming faster than 1 u/s pushes the bow rings
        // 0x22ad38(0, 1); the splash countdown 0x13fc5a: t − 1, then min(t / 8, 6) bubbles at the feet.
        let v = f(self.eff_len_xy);
        if self.state == id::SURFACE_IDLE { effects::wake(self, rng, 15, 30); }
        if DTF * 1.5 < v { effects::spray(self, rng, 4, 12); }
        if self.state == id::SURFACE_SWIM && DTF < v { effects::bow_rings(self, rng, 0, 1); }
        if self.swim.splash != 0 {
            self.swim.splash -= 1;
            let n = ((self.swim.splash >> 3) as i32).min(6);
            super::fx::bubbles(self, rng, n);
        }
        self.anim_speed = Pf::f(0.6);
        self.stick_target(env, p(DTF * 3.0));
        if !env.pad.no_direction && f(self.target_speed) < DTF * 3.0 * 0.5 { self.target_speed = p(DTF * 3.0 * 0.5); }
        self.turn_to(Pf::f(0.007), Pf::f(0.08), p(DTF * 5.235_987_7));
        if self.swim.stroke_timer != 0 { self.swim.stroke_timer -= 1; }
        if self.state == id::SURFACE_SWIM {
            if env.pad.no_direction && self.swim.stroke_timer != 0 { self.target_speed = p(DTF * 3.0); }
            if std::f32::consts::FRAC_PI_4 < f(self.yaw_residual).abs() { self.target_speed = Pf::ZERO; }
            self.speed_step(p(DT2F * 4.0), p(DT2F * 5.0));
            let v = f(self.speed) * Self::stroke(anim) * 5.5;
            let yaw = f(self.rot[2]);
            self.vel[0] = p(yaw.cos() * v);
            self.vel[1] = p(yaw.sin() * v);
            self.vel[2] = Pf::ZERO;
        } else {
            self.speed = Pf::ZERO;
            self.vel = V0;
        }
        // Swimming: four bubbles at the hands (lists 0 / 0xe) in the first 10 frames of the stroke (not blending).
        let av = anim.view();
        if self.state == id::SURFACE_SWIM && !av.blending() && 0.0 < av.frame && av.frame < 10.0 { super::fx::bubbles_at(self, rng, Some(anim), 4, 1); }
        self.momentum_decay(p(DT2F * 4.2));
        self.wall_check(env, 0);
        self.surface_bob();
    }

    /// `0x240c78`: float on the water. The level the hero rides springs toward the water level (0.027, 0.3);
    /// the bob offset settles from the entry speed: a damped oscillator while treading water (0x37: acc −0.005·off
    /// − 0.045·vel), a spring back to 0 while swimming (0x36, never above 0). The spring starts after 10 ticks,
    /// at once for an offset below 0 or out of the underwater group. `z = level − 0.12 + off`.
    pub fn surface_bob(&mut self) {
        let sw = &mut self.swim;
        let mut off = f(sw.bob);
        let mut v = f(sw.bob_vel);
        if ticks(10) < self.timer || off < 0.0 || self.prev_group == 0x11 || self.prev_prev_group == 0x11 {
            if self.state == id::SURFACE_SWIM {
                let (mut o, mut sv) = (p(off), sw.sink);
                spring(Pf::ZERO, Pf::f(0.03), Pf::f(0.3), p(DTF * 1.5), &mut o, &mut sv);
                off = f(o);
                sw.sink = sv;
                v = 0.0;
            } else {
                v -= 0.005 * off + 0.045 * v;
                if off.abs() < 0.001 && v < 0.0001 { v = 0.0; }
            }
        }
        off += v;
        if self.state == id::SURFACE_SWIM && 0.0 < off {
            if 0.0 < v { v = 0.0; }
            off = if off - DTF * 0.7 < 0.0 { 0.0 } else { off - DTF * 0.7 };
        }
        sw.bob = p(off);
        sw.bob_vel = p(v);
        if self.state != 0x7f {
            let t = self.water_level;
            let (mut l, mut lv) = (sw.level, sw.level_vel);
            spring(t, Pf::f(0.027), Pf::f(0.3), Pf::ZERO, &mut l, &mut lv);
            sw.level = l;
            sw.level_vel = lv;
        }
        self.pos[2] = p(f(self.swim.level) + -0.12 + f(self.swim.bob));
    }

    /// 0x6a (and 0x82): drowned. The momentum runs out (4·dt²), the last breath bubbles rise
    /// ([`effects::breath_drowning`]) and the body sinks toward −1.5 u/s.
    pub fn phys_drown(&mut self, anim: &dyn AnimCtl, rng: &mut Rng) {
        let m = [f(self.momentum[0]), f(self.momentum[1]), f(self.momentum[2])];
        let l = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt();
        let q = if 0.0 < l { (l - DT2F * 4.0).max(0.0) / l } else { 0.0 };
        self.momentum = [p(m[0] * q), p(m[1] * q), p(m[2] * q), self.momentum[3]];
        self.vel = self.momentum;
        effects::breath_drowning(self, rng, anim);
        let mut s = f(self.swim.sink).min(0.0);
        let t = -(DTF * 1.5);
        s = if s - t > DT2F * 1.5 { s - DT2F * 1.5 } else if t - s > DT2F * 1.5 { s + DT2F * 1.5 } else { t };
        self.swim.sink = p(s);
        self.vel[2] = self.swim.sink;
    }

    // --------------------------------------------------------------------------------------------
    // Transitions 0x242930.

    /// 0x33 / 0x35: stroking and the Hydro-Pack. 0x33 plays at 15 × speed (0.25..0.75) and drifts (0x34) between
    /// strokes when nothing is held; R1|R2 with the pack switches to 0x35 after 20 ticks; releasing them leaves
    /// 0x35 for 0x33 (stick > 0.5) or 0x34.
    pub fn tr_underwater(&mut self, c: &mut Ctx) {
        let held = c.env.pad.held;
        let stick = f(self.stick_mag);
        if self.state == id::UNDERWATER {
            self.anim_speed = p((f(self.speed) * 15.0).clamp(0.25, 0.75));
            let v = c.anim.view();
            if held & (X | SQ) == 0 && 9.0 < v.frame && v.frame < 19.0 && self.substate != 1 && stick < 0.1 && f(self.speed) < DTF + DTF {
                self.set_state(c, id::UNDERWATER_IDLE, true);
                return;
            }
        }
        if !(self.owned.has(ITEM_HYDRO_PACK) && held & R12 != 0) {
            if self.state != id::HYDRO { return; }
            if 0.5 < stick {
                self.set_state(c, id::UNDERWATER, true);
                return;
            }
        } else if self.state == id::UNDERWATER && ticks(20) < self.timer {
            self.set_state(c, id::HYDRO, true);
            return;
        }
        if self.state != id::HYDRO || held & (X | SQ) != 0 || self.substate == 1 || 0.7 <= stick || held & R12 != 0 { return; }
        self.set_state(c, id::UNDERWATER_IDLE, true);
    }

    /// 0x34: drifting. Loops its sequence; the stick or □ / ✕ (or R1|R2 with the pack) swims on.
    pub fn tr_underwater_idle(&mut self, c: &mut Ctx) {
        let v = c.anim.view();
        if v.flags & 2 != 0 && v.seq_b != 0x3c { self.set_anim(c.anim, c.rng, Pf::from_i32(ticks(11)), 0x3c, 0); }
        let held = c.env.pad.held;
        if f(self.stick_mag) <= 0.2 && held & (X | SQ) == 0 && (!self.owned.has(ITEM_HYDRO_PACK) || held & R12 == 0) { return; }
        let s = self.dive_state(held);
        self.set_state(c, s, true);
    }

    /// 0x36: swimming on the surface. ✕ → jump out (0x12), □ → dive; shallow enough → wade (0x73); stopped
    /// after the stroke window → tread water (0x37).
    pub fn tr_surface_swim(&mut self, c: &mut Ctx) {
        let pad = c.env.pad;
        if self.swim.jump_lock == 0 && pad.pressed_within(X, ticks(8)).is_some() {
            self.set_state(c, id::WATER_JUMP, true);
            return;
        }
        if self.swim.dive_lock == 0 && pad.pressed_within(SQ, ticks(7)).is_some() {
            let s = self.dive_state(pad.held);
            self.set_state(c, s, true);
            return;
        }
        if self.f13f9 != 0 && self.water_depth() < 0.7 {
            self.set_state(c, id::WADE, false);
            self.set_anim(c.anim, c.rng, Pf::from_i32(ticks(17)), 0x60, 0);
            self.disp[2] = Pf::ZERO;
            self.vel[2] = Pf::ZERO;
        }
        if 0.1 <= f(self.stick_mag) || self.speed != Pf::ZERO || self.swim.stroke_timer != 0 { return; }
        self.set_state(c, id::SURFACE_IDLE, true);
    }

    /// 0x37: treading water. ✕ after 10 ticks → jump out, □ (or R1|R2 with the pack) → dive; shallow → stand
    /// (idle); the stick after 10 ticks → swim (0x36).
    pub fn tr_surface_idle(&mut self, c: &mut Ctx) {
        let pad = c.env.pad;
        if self.swim.jump_lock == 0 && pad.pressed_within(X, ticks(8)).is_some() && ticks(10) < self.timer {
            self.set_state(c, id::WATER_JUMP, true);
            return;
        }
        self.items.f13f7 = 1;
        if self.swim.dive_lock == 0
            && (pad.pressed_within(SQ, ticks(7)).is_some() || (self.owned.has(ITEM_HYDRO_PACK) && pad.pressed_within(R12, ticks(7)).is_some()))
        {
            let s = self.dive_state(pad.held);
            self.set_state(c, s, true);
            return;
        }
        if self.f13f9 != 0 && self.water_depth() < 0.4 && self.set_state(c, 0, false) {
            let seq = self.idle_seq();
            self.set_anim(c.anim, c.rng, Pf::from_i32(ticks(18)), seq, 0);
        }
        if f(self.stick_mag) <= 0.2 || self.timer <= ticks(10) { return; }
        self.set_state(c, id::SURFACE_SWIM, true);
    }

    /// 0x6a (the death group's rule): when the sequence ends, `0x2319b0` (deaths, fade, 0x141401 = 1).
    pub fn tr_drown(&mut self, c: &mut Ctx) {
        if c.anim.view().flags & 2 != 0 && self.fell_out == 0 {
            // The one death sequence (deaths, the killer's mission, 0x141401 = 1): super::damage.
            super::damage::death_fade(self);
            self.swim.events.push(SwimEvent::Drowned);
        }
    }

    /// The Euler x / y straightening `0x236520` (post-move, not while frozen): outside the underwater group and
    /// a few special states, roll and pitch spring back to 0 (0.015, 0.3, 300°/s). Skipped while both angles
    /// and their velocities are 0 (the game's spring then only writes −0.0 over +0.0).
    pub fn straighten_euler(&mut self) {
        let m = if self.f548 != 0 { 1 } else { self.gravity_mode };
        if m != 0 { return; } // 0x236358 (gravity modes 1, 2) is not ported.
        if matches!(self.state, 0x25 | 0x26 | 0x77 | 0x31 | 0x2d | 0x2c) || matches!(self.group, 0x11 | 0x15 | 0x16) { return; }
        let z = Pf::ZERO;
        if self.rot[0] == z && self.rot[1] == z && self.swim.euler_vel == [z, z] { return; }
        let max = p(DTF * 5.235_987_7);
        for k in 0..2 {
            let (mut a, mut v) = (self.rot[k], self.swim.euler_vel[k]);
            turn_spring(Pf::b(0x8000_0000), Pf::f(0.015), Pf::f(0.3), max, &mut a, &mut v, 0);
            self.rot[k] = a;
            self.swim.euler_vel[k] = v;
        }
    }
}

#[cfg(test)]
mod tests;
