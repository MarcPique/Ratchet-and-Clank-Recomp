//! **Package P2 — damage, knockback and death.** The hit intake, the hurt states, every death state and the
//! death sequence, ported once as general systems (docs/plan/hero_states.md "P2"). Addresses level01 unless
//! noted (`L00` = level00, the superset build: 0x3c's parameters, 0x7c, the grind branch of the intake).
//!
//! **One hit path.** Every hit reaches the hero as the moby hit message of Ratchet's moby (`+0xa4` → the record
//! at `0x178580 + 0x40·slot`), whoever sent it (a moby's `FUN_0026e968` / `coll_sphere_mobys` / the debug hit):
//! the tick reads the record out of the hit log before the hero update ([`Damage::hit`], a [`HeroHit`]) and the
//! write-back `HeroSyncMoby` 0x229f20 clears `+0xa4` after it. Hazard surfaces (burn 0x3c / 0x7c, pits 0x79,
//! sinking 0x7f) and the death plane (0x77) are SetStates of the surface reaction / the transitions' prologue;
//! drowning is the swim's 0x6a. All of them end in the one death sequence [`death_fade`].
//!
//! * **Hit intake** `0x231580` (L00 `0x210ce8`), the first call of the transitions ([`hit_intake`]): nothing in
//!   group 0x14 / 7, state 0x32, while invulnerable (0x13f510) or without a record for Ratchet with flag bit 0;
//!   the attacker becomes the killer 0x1415d0 ([`Damage::killer`]); hits counted (0x15eea8, 0x13df88[level]);
//!   group 0xf (grind) only raises 0x13f90e (P5's grind hurt reads it). The push direction: the record's own
//!   vector when `+0x30 & 1` (exact when its w is 5627.925), else away from the attacker (`pos − attacker`), else
//!   straight back (`yaw + π`). The state: 0x80 for the hazard classes 0x4eb / 0x558 (no push), 0x6d on the
//!   Hoverboard, 0x75 on the water surface / 0x76 under water / 0x82 for class 0x28f in the water groups (and on
//!   a ledge), else 0x16; the other bodies' 0x56 / 0x46. Knockback `0x231518(5.7·dt, 2.4·dt)` (1.7·dt up and
//!   no push on the magnetic floor 0x140637): xy scaled to the speed and z = up, or the exact vector scaled per
//!   axis (z doubled for attack type 4); then added to the velocity 0x13f430 **and** the displacement 0x13f450.
//! * **Hurt entries** (0x16 / 0x75 / 0x76): motion ×0.5 (`0x2342d8`), hit flash 0x13f53e = 45, velocity =
//!   effective (0x16, clamped to 7·dt) or the displacement (water), `HeroTakeDamage(min(damage, 1))` with the
//!   damage of the hit message (1 without one), invulnerability 0x13f510 = 77, anim 0x10 / 0x70 / 0x6f on the
//!   eased curve −3 from frame 3 with its first step taken at once.
//! * **Hurt physics / transitions**: 0x16 brakes |vel.xy| by 4·dt² (10·dt² on the ground after 10 ticks), sinks
//!   0.004 per tick; leaves to the fall 6 (after frame 12.5 in the air above 1), then after 27 ticks to the death
//!   0x3d (no health), the Thruster hover 0x81 (entered from it) or idle 0. 0x75 / 0x76: flat, 8 %/tick braking,
//!   the float `0x240c78` on the surface; to 0x37 / 0x34 after 50 ticks (0x76 also on the wrap) or to the drowned
//!   0x6a (0x82 on levels 15 / 17 under water) without health.
//! * **Deaths**: 0x3d (group 0x14, anim 0x45; ground physics) and 0x80 (hazard: health 0, momentum ≤ 3.5·dt,
//!   anim 0x7c) and 0x82 (eaten: anim 0x80, the drowned physics) fade on the anim wrap; 0x77 death fall (fall
//!   entry + a voice and two random spin rates; tumbles about a point 0.6 up, 10 % braking, fades after 120
//!   ticks or at once below the level-3 / 6 / 16 heights); 0x79 pit fall (slides down the slope at 5.5 u/s,
//!   fades after 300 ticks); 0x3c burn bounce (jump group: h 5.5, damage 1; the second in a row or at no health
//!   → 0x7c; level 10 → 0x7c at once); 0x7c burn death (sinks at dt, fades 1.2 below the ground or after 100);
//!   0x7f sinking death (floats down the water, fades after 220).
//! * **Death sequence** `0x2319b0` ([`death_fade`]): deaths counted (0x15eeac, 0x13dfd8[level]; the killer's
//!   mission `+0xb0`: 0x14e990 / 0x14ee90), `FadeToBlack(16)`, **0x141401 = 1** ([`Hero::fell_out`]). The main
//!   loop then does the death reload `LoadLevelCoreData(0, 1)` at the end of that frame (no catch-up tick): the
//!   engine respawns on this flag (docs/plan/hero_states.md P2), never on entering a state.
//! * **Hero voice sounds** (`0x236738(index, flags)` = `PlayClassSound` on Ratchet's moby: 0x77's 0x17 / 0x20,
//!   the underwater hit's 0x1c on levels 15 / 17, 0x7c's 9): queued by the entry and played through
//!   [`super::HeroSounds::voice`] right after the transitions ([`flush`]), with 0x77's two spin draws after the
//!   voice: the order of the game's draws (voice pitch bend, spin, spin) is kept because nothing between the
//!   SetState and the end of the transitions draws (Ratchet's sequences 10 / 0xb / 0x6f / 0x71 have no back-item
//!   rows).
//!
//! **Water effects** (`super::swim::effects`): 0x75's surface wake `0x22ac40(15, 30)`, 0x76's bubbles
//! `0x22b140(n, 0)` (`super::fx::bubbles`), 0x82's breath bubbles (the drowned physics, as 0x6a's).
//!
//! **The burns' fire** (2026-10-04): 0x7c's every tick (L00 `0x217450`, also 0x7b's entry on level 15) and 0x3c's
//! while it rises (`0x209ec8`: the side spark and the fire ring on their timers 0x13f7ee / 0x13f7ec):
//! [`super::fx::burn_fire`], [`super::fx::burn_bounce_fire`].
//!
//! **The ice of 0x7f** (L01 `0x2370b8` case 0x7f): while 0x13f9f4 is 0, `CreateMoby(0x52a)` at the hero (0x13f3d0,
//! rotation 0x13f3e0), update distance 0, drawn, draw distance 0x40, alpha 0, mode | 0x800, `MobyBuildMatrix`,
//! `PlayClassSound(0, 0)`; then every tick its alpha += `trunc(5·[0x15ed60])` (at most 0x80) and it takes the hero
//! moby's position and rotation (`MobyBuildMatrix`): [`ICE_CLASS`], `super::fx::create_mobys` (after the hero update,
//! so its sound's draw comes after the state's [L]).
//!
//! **Not ported** (cosmetic, and they draw from `rand`, so the stream diverges from the PS2 in these states only):
//! 0x7c's level-6 fog colour (0x141644 / 0x141648);
//! the help-flag clear `0x225938` and the level-15 flag 0x15f5a4 of the death sequence; the game mode 0x15f5c4
//! test of the intake (0 in gameplay ticks); the Giant Clank branch of the intake (body 2, not ported).
//!
//! Arithmetic: the shared primitives (`clamp_len2`, `set_len2`, `approach`, gravity) are the ported [`Pf`]
//! helpers; the new formulas (0x77's tumble, the knockback) are standard `f32` stored back with [`Pf::f`].

use super::anim::AnimCtl;
use super::common::blend;
use super::physics::*;
use super::states::Ctx;
use super::{Hero, HeroSounds};
use crate::moby_runtime::{Moby, MobyId};
use crate::ps2v::Pf;
use crate::rng::Rng;

/// The record's w that marks an exact push vector (`0x231580`: `== 5627.925`).
pub const EXACT_PUSH_W: f32 = 5627.925;

/// The moby that sent a hit (`0x1415d0`, the "killer"): what the hero code reads of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attacker {
    pub id: MobyId,
    /// +0xa6.
    pub o_class: i16,
    /// +0x10.
    pub pos: [f32; 3],
    /// +0xb0: mission byte index (0xff = none).
    pub mission: u8,
}

/// The hit message Ratchet's moby holds this tick (the record of slot `+0xa4` when it targets him), as the
/// tick hands it to the hero ([`Damage::hit`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeroHit {
    /// +0x20 (None = 0).
    pub attacker: Option<Attacker>,
    /// +0x24: damage flags (bit 0: hurts the hero).
    pub flags: u32,
    /// +0x28: attack type byte.
    pub b28: u8,
    /// +0x2c: damage.
    pub damage: f32,
    /// +0x30 (bit 0: use the record's push vector).
    pub w30: u32,
    /// +0x10: push vector.
    pub dir: [f32; 4],
}

/// What the tick's owner applies to the game state after the tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageEvent {
    /// A hit taken (0x15eea8 / 0x13df88[level] += 1).
    Hit,
    /// `0x2319b0`: deaths (0x15eeac / 0x13dfd8[level] += 1), the killer's mission deaths (0x14e990 /
    /// 0x14ee90 [killer +0xb0], when the killer has one); 0x141401 is set.
    Died { killer_mission: Option<u8>, killer_class: Option<i16> },
}

/// The damage fields of the hero block.
#[derive(Clone, Debug, Default)]
pub struct Damage {
    /// This tick's hit message for Ratchet (set by the tick before the hero update; None: `+0xa4` = 0xff or a
    /// record for another moby).
    pub hit: Option<HeroHit>,
    /// `0x1415d0`: the moby of the last hit taken (kept until the next intake finds none).
    pub killer: Option<Attacker>,
    /// Hits taken and deaths (drained by the tick's owner).
    pub events: Vec<DamageEvent>,
    /// 0x13fc98 / 0x13fc9c: 0x77's tumble rates; 0x13fca0 / 0x13fca4: their targets (the entry's draws).
    pub spin: [f32; 2],
    pub spin_target: [f32; 2],
    /// 0x141602: the voice slot 0x77 started.
    pub voice_slot: i32,
    /// 0x1409b0: burn bounces in a row.
    pub burns: i32,
    /// 0x13f90e: hit while grinding (P5's grind hurt).
    pub grind_hit: u8,
    /// 0x13f644 as 0x7c sets it: the ground height it sinks from.
    pub burn_floor: f32,
    /// 0x13f7ee / 0x13f7ec: the burn bounce's spark and fire-ring timers ([`super::fx::burn_bounce_fire`]).
    pub burn_spark: i16,
    pub burn_ring: i16,
    /// 0x13f9f4: the ice of the sinking death 0x7f (class [`ICE_CLASS`], created by its first physics tick; the hero
    /// init clears it).
    pub ice: Option<MobyId>,
    /// Voices queued by the entries, played by [`flush`]; 0x77's spin draws follow them.
    pending: Vec<(i32, u32)>,
    pending_spin: bool,
}

/// The sinking death's ice (`CreateMoby(0x52a)`).
pub const ICE_CLASS: i16 = 0x52a;

// ------------------------------------------------------------------------------------------------
// Shared pieces.

/// `0x2342d8(k)`: disp, eff, vel, momentum (xyz) and the speed scaled by `k`.
fn scale_motion(h: &mut Hero, k: Pf) {
    h.disp = vscale(h.disp, k);
    h.eff = vscale(h.eff, k);
    h.vel = vscale(h.vel, k);
    h.momentum = vscale(h.momentum, k);
    h.speed = h.speed * k;
}

/// `HeroTakeDamage(n)` 0x226fa8: `n ≠ 0` takes `min(n, 1)` (a negative n gives health back), health ≥ 0; the
/// HUD's health show `HudShowHealth` follows the change (the HUD watches the health).
pub fn take_damage(h: &mut Hero, n: i32) {
    if n == 0 { return; }
    let d = if n < 2 { n } else { 1 };
    h.health = (h.health - d).max(0);
}

/// `MobyGetHitMessage(Ratchet, 1, 0)` then `HeroTakeDamage`: the message's damage (truncated), 1 without one.
fn take_hit_damage(h: &mut Hero) {
    let n = match h.damage.hit.filter(|m| m.flags & 1 != 0) {
        Some(m) => m.damage as i32,
        None => 1,
    };
    take_damage(h, n);
}

/// `SetAnim(−3, seq, 3)` plus the entries' immediate first curve step (`moby+0x54 = curve[0x13fdf4++]`).
fn hurt_anim(h: &mut Hero, c: &mut Ctx, seq: u8) {
    h.set_anim(c.anim, c.rng, Pf::b(0xc040_0000), seq, 3);
    c.anim.curve_step();
}

/// `0x2319b0`: the death sequence (module doc). Also the swim's drowned state 0x6a ends here.
pub fn death_fade(h: &mut Hero) {
    let k = h.damage.killer;
    h.damage.events.push(DamageEvent::Died { killer_mission: k.map(|a| a.mission).filter(|&m| m != 0xff), killer_class: k.map(|a| a.o_class) });
    h.fell_out = 1;
}

/// `0x231518(speed, up, dir, exact)`: the knockback vector.
fn knock(speed: f32, up: f32, dir: &mut [f32; 4], exact: bool) {
    if exact {
        dir[0] *= speed;
        dir[1] *= speed;
        dir[2] *= up;
    } else {
        let l2 = dir[0] * dir[0] + dir[1] * dir[1];
        if l2 == 0.0 {
            dir[0] = 0.0;
            dir[1] = 0.0;
        } else {
            let q = speed / l2.sqrt();
            dir[0] *= q;
            dir[1] *= q;
        }
        dir[2] = up;
    }
}

const DTF: f32 = 1.0 / 60.0;

// ------------------------------------------------------------------------------------------------
// The hit intake 0x231580.

/// The hit intake 0x231580 at the top of the transitions; true when it took the hit (the pass ends).
pub(super) fn hit_intake(h: &mut Hero, c: &mut Ctx) -> bool {
    if h.group == 0x14 || h.group == 7 || h.state == 0x32 { return false; }
    if h.f510 != 0 { return false; }
    // Giant Clank (body 2) takes the hits with flag bit 2, everyone else bit 0 (L00 0x210ce8).
    let mask = if h.mode == 2 { 2 } else { 1 };
    let Some(hit) = h.damage.hit.filter(|m| m.flags & mask != 0) else {
        h.damage.killer = None;
        return false;
    };
    h.damage.killer = hit.attacker;
    h.damage.events.push(DamageEvent::Hit);
    // L00: on a grind rail only the grind's flag (the grind hurt 0x42 is P5's).
    if h.group == 0xf {
        h.damage.grind_hit = 1;
        return false;
    }
    // Body 2: the damage comes off Giant Clank's energy 0x140980 (truncated, not below 0).
    if h.mode == 2 { h.bodies.energy = (h.bodies.energy - hit.damage as i32).max(0); }
    let mut exact = false;
    let mut dir = if hit.w30 & 1 == 0 {
        match hit.attacker {
            None => {
                let y = fast_add_rotations(h.rot[2], PI);
                [fast_cos(y).to_f32(), fast_sin(y).to_f32(), 0.0, 0.0]
            }
            Some(a) => {
                let p = to_f32x3(h.pos);
                [p[0] - a.pos[0], p[1] - a.pos[1], p[2] - a.pos[2], 0.0]
            }
        }
    } else {
        exact = hit.dir[3] == EXACT_PUSH_W;
        hit.dir
    };
    let class = hit.attacker.map(|a| a.o_class);
    match h.mode {
        0 => {}
        1 => {
            h.set_state(c, 0x46, true);
            knock(DTF * 5.0, DTF * 3.0, &mut dir, exact);
            add_push(h, dir);
            return true;
        }
        3 => {
            h.set_state(c, 0x56, true);
            knock(DTF * 5.0, DTF * 2.4, &mut dir, exact);
            add_push(h, dir);
            return true;
        }
        // Giant Clank (body 2): an attack with flag 4 or no energy left → his hurt 0x5d, the beam ended (`0x2a9be0`), and on
        // the ground the knockback (7, 3.5)·dt; otherwise nothing but the energy (the push is zero).
        2 => {
            if hit.flags & 4 != 0 || h.bodies.energy < 1 {
                h.set_state(c, 0x5d, true);
                super::bodies::giant::beam_end(h);
                if h.air_ticks == 0 {
                    knock(DTF * 7.0, DTF * 3.5, &mut dir, exact);
                    add_push(h, dir);
                }
            }
            return true;
        }
        // The disguise is 3 (above); other words take no push.
        _ => return true,
    }
    if matches!(class, Some(0x4eb) | Some(0x558)) {
        h.set_state(c, 0x80, true);
        return true;
    }
    if h.group == 0x16 {
        h.set_state(c, 0x6d, true);
        h.eff_v[2] = DT * Pf::f(7.0);
        return true;
    }
    let eaten = class == Some(0x28f);
    match h.group {
        0x12 => {
            if eaten { h.set_state(c, 0x82, true); return true; }
            h.set_state(c, 0x75, true);
        }
        0x11 => {
            if eaten { h.set_state(c, 0x82, true); return true; }
            if matches!(h.idle.level, 0xf | 0x11) && matches!(class, Some(0x28f) | Some(0x7b) | Some(0x29d)) {
                h.damage.pending.push((0x1c, 0));
            }
            h.set_state(c, 0x76, true);
        }
        3 => {
            if eaten { h.set_state(c, 0x82, true); return true; }
            h.set_state(c, 0x16, true);
        }
        _ => {
            h.set_state(c, 0x16, true);
        }
    }
    let (speed, up) = if h.f0637 != 0 { (0.0, DTF * 1.7) } else { (DTF * 5.7, DTF * 2.4) };
    knock(speed, up, &mut dir, exact);
    if exact && hit.b28 == 4 { dir[2] += dir[2]; }
    add_push(h, dir);
    true
}

/// The intake's tail: `0x13f430 += dir`, `0x13f450 += dir`.
fn add_push(h: &mut Hero, dir: [f32; 4]) {
    let d = [Pf::f(dir[0]), Pf::f(dir[1]), Pf::f(dir[2]), Pf::ZERO];
    h.vel = vadd(h.vel, d);
    h.disp = vadd(h.disp, d);
}

// ------------------------------------------------------------------------------------------------
// SetState 0x23cf98.

/// SetState entry of 0x16, 0x3c, 0x3d, 0x75, 0x76, 0x77, 0x79, 0x7c, 0x7f, 0x80, 0x82. `None`: continue with
/// SetState's epilogue.
pub(super) fn entry(h: &mut Hero, c: &mut Ctx, id: i32, play: bool, old_sub: i32) -> Option<bool> {
    match id {
        0x16 => {
            h.group = 7;
            h.f15d4 = 0;
            scale_motion(h, Pf::b(0x3f00_0000));
            h.f53e = ticks(45) as i16;
            h.vel = h.eff;
            super::common::clamp_len_2745f0(&mut h.vel, DT * Pf::f(7.0));
            take_hit_damage(h);
            h.f510 = ticks(77);
            if play { hurt_anim(h, c, 0x10); }
        }
        0x75 | 0x76 => {
            h.group = 7;
            h.f15d4 = 0;
            scale_motion(h, Pf::b(0x3f00_0000));
            h.items.f13f7 = 1;
            h.vel = h.disp;
            take_hit_damage(h);
            h.f510 = ticks(77);
            if play { hurt_anim(h, c, if id == 0x75 { 0x70 } else { 0x6f }); }
        }
        0x77 => {
            // The fall's case (0x23cf98 case 6 / 0x77) with the voice and the two spin draws (after the voice,
            // in `flush`).
            if h.mode == 0 { h.damage.pending.push((0x17, 0x20)); }
            h.damage.pending_spin = true;
            return h.fall_entry(c, play);
        }
        0x79 => {
            h.group = 2;
            h.f15d4 = 0;
            h.vel = h.eff;
            if play { h.set_anim(c.anim, c.rng, blend(12), 0xb, 0); }
        }
        0x3d => {
            h.group = 0x14;
            h.f15d4 = 0;
            h.momentum = h.eff;
            if play { h.set_anim(c.anim, c.rng, blend(12), 0x45, 0); }
        }
        0x80 => {
            h.group = 0x14;
            h.f15d4 = 0;
            h.health = 0;
            h.momentum = h.eff;
            super::common::clamp_len_2745f0(&mut h.momentum, DT * Pf::f(3.5));
            if play { h.set_anim(c.anim, c.rng, blend(12), 0x7c, 0); }
        }
        0x82 => {
            h.items.f13f7 = 1;
            h.group = 0x14;
            h.f15d4 = 0;
            h.health = 0;
            h.momentum = h.eff;
            if play { h.set_anim(c.anim, c.rng, blend(10), 0x80, 0); }
        }
        0x7f => {
            let sw = &mut h.swim;
            sw.bob_vel = DT * Pf::f(-7.0);
            h.group = 0x14;
            sw.level = Pf::f(h.water_level.to_f32() - 0.37);
            h.f15d4 = 0;
            h.health = 0;
            let slow = Pf::f(h.disp[2].to_f32() * 0.2);
            if sw.bob_vel <= slow { sw.bob_vel = slow; }
            h.items.f13f7 = 1;
            sw.bob = Pf::f(h.pos[2].to_f32() - sw.level.to_f32());
            sw.level_vel = Pf::ZERO;
            if play { h.set_anim(c.anim, c.rng, blend(11), 0x74, 0); }
        }
        0x7c => {
            // L00 0x2223f8 case 0x7c.
            if h.idle.level == 6 {
                h.set_state(c, 0x3d, true);
                return Some(false);
            }
            h.group = 0x14;
            h.damage.burn_floor = h.ground_z.to_f32();
            h.no_vel_clamp = 10000;
            h.f15d4 = 0;
            h.health = 0;
            h.damage.pending.push((9, 0));
            if play { h.set_anim(c.anim, c.rng, blend(8), 0x71, 0); }
        }
        0x3c => return burn_bounce_entry(h, c, play, old_sub),
        _ => {}
    }
    None
}

/// 0x3c, the jump group's entry (L00 0x2223f8 case 0x3c): the jump lockout, the shared jump block, then
/// h 5.5 (5.5..5.55, ramp 2), takeoff 5, frames 18 / 26 / 13, air 120, damage 1; level 10, or a second burn in a
/// row (or no health left) → the burn death 0x7c; fall-over after 150, gravity 15·dt²; anim 0x43 over 4.
fn burn_bounce_entry(h: &mut Hero, c: &mut Ctx, play: bool, old_sub: i32) -> Option<bool> {
    if h.jump_lockout != 0 {
        h.state = h.prev_state;
        h.timer = h.prev_timer;
        h.substate = old_sub;
        return Some(false);
    }
    h.jump_block_defaults(c.rng);
    let j = &mut h.jump;
    j.h = Pf::f(5.5);
    j.takeoff = ticks(5);
    j.bottom784 = Pf::b(0x3f19_999a);
    (j.f_apex, j.f_hold, j.f_land) = (Pf::f(18.0), Pf::f(26.0), Pf::f(13.0));
    j.hmax = Pf::f(5.55);
    j.hmin = Pf::f(5.5);
    j.ramp = ticks(2) as i16;
    j.max_air = ticks(120) as i16;
    take_damage(h, 1);
    if h.idle.level == 10 {
        h.set_state(c, 0x7c, true);
        return Some(false);
    }
    if h.idle.level != 6 {
        if h.prev_state == 0x3c || h.prev_prev_state == 0x3c || h.health == 0 {
            h.damage.burns += 1;
            if 1 < h.damage.burns || h.health == 0 {
                h.set_state(c, 0x7c, true);
                return Some(false);
            }
        } else {
            h.damage.burns = 0;
        }
    }
    let j = &mut h.jump;
    j.fallover_after = ticks(150) as i16;
    j.g = DT2 * Pf::f(15.0);
    // The fire ring and spark timers (L00 0x2223f8: 0x13f7ec = ticks(5), 0x13f7ee = ticks(4)).
    h.damage.burn_ring = ticks(5) as i16;
    h.damage.burn_spark = ticks(4) as i16;
    if play { h.set_anim(c.anim, c.rng, blend(4), 0x43, 0); }
    None
}

// ------------------------------------------------------------------------------------------------
// Per-state physics 0x2370b8.

/// Per-state physics; false = not ported (the hero freezes).
pub(super) fn physics(h: &mut Hero, env: &Env, anim: &mut dyn AnimCtl, rng: &mut Rng) -> bool {
    match h.state {
        0x16 => {
            let l = len2(h.vel);
            let mut dec = DT2 * Pf::f(4.0);
            if h.air_ticks == 0 && ticks(10) < h.timer { dec = DT2 * Pf::f(10.0); }
            let mut s = l - dec;
            if s < Pf::ZERO { s = Pf::ZERO; }
            if h.f0637 != 0 { h.edge_brake(env, Pf::b(0x406c_cccd), Pf::ZERO); }
            h.vel = set_len2(h.vel, s);
            let z = h.vel[2];
            h.gravity_from(z, Pf::b(0x3b83_126f));
        }
        0x75 | 0x76 | 0x7f => {
            if h.state == 0x7f { h.fx.mobys.push(super::fx::MobySpawn::Ice); }
            // 0x75 (hurt on the surface): the wake `0x22ac40(15, 30)` (0x2387bc, top of the case).
            if h.state == 0x75 { super::swim::effects::wake(h, rng, 15, 30); }
            // 0x76 (hurt under water): a burst of bubbles `0x22b140(min(10 − timer/3, 8), 0)`, fewer each 3 ticks.
            if h.state == 0x76 { super::fx::bubbles(h, rng, (10 - h.timer / 3).min(8)); }
            h.vel[2] = Pf::ZERO;
            let l = len2(h.vel);
            let mut v = h.vel;
            clamp_len2(&mut v, ((SCALE60 * Pf::b(0xbda3_d708)) * l) + l);
            h.vel = v;
            if h.state == 0x76 {
                h.wall_check(env, 0);
            } else {
                if h.state == 0x75 { h.wall_check(env, 0); }
                h.surface_bob();
            }
        }
        0x77 => {
            h.items.f13fc = 1;
            let step = DT2.to_f32() * 8.726_646;
            for k in 0..2 {
                let (mut x, t) = (Pf::f(h.damage.spin[k]), Pf::f(h.damage.spin_target[k]));
                approach(t, Pf::f(step), &mut x);
                h.damage.spin[k] = x.to_f32();
            }
            tumble(h, h.damage.spin[0], h.damage.spin[1], 0.0);
            let l = len2(h.vel);
            let mut v = h.vel;
            clamp_len2(&mut v, ((SCALE60 * Pf::b(0xbdcc_ccd0)) * l) + l);
            h.vel = v;
            let z = h.vel[2];
            h.gravity_from(z, h.group_gravity);
            let floor = -(DT * Pf::f(50.0));
            if h.vel[2] < floor { h.vel[2] = floor; }
        }
        0x79 => {
            h.target_speed = DT * Pf::f(5.5);
            h.target_yaw = h.slope_yaw;
            h.air_accel(DT2 * Pf::f(25.0));
            h.gravity_from(h.eff_v[2], DT2 * Pf::f(18.0));
        }
        0x7c => {
            // L00 0x217970 case 0x7c: the fire (0x217450), then the sinking.
            let floor = h.damage.burn_floor;
            super::fx::burn_fire(h, rng, floor);
            h.vel = V0;
            h.vel[2] = -DT;
        }
        0x3d | 0x80 => h.phys_ground(env, anim, rng),
        0x82 => h.phys_drown(anim, rng),
        0x3c => {
            h.phys_jump(env);
            // L00 0x21f290: rising faster than 1.5·dt (0x13f438) and not descending → the fire (0x209ec8).
            if h.state == 0x3c && h.jump.descending == 0 && DT * Pf::f(1.5) < h.vel[2] { super::fx::burn_bounce_fire(h, rng); }
        }
        _ => return false,
    }
    true
}

/// `0x22a8d8(ax, ay, az)` → `0x22a7d0` about the point 0.6 up the body ([`Hero::turn_about`]).
fn tumble(h: &mut Hero, ax: f32, ay: f32, az: f32) { h.turn_about([ax, ay, az], [0.0, 0.0, 0.6]); }

// ------------------------------------------------------------------------------------------------
// Transitions 0x242930.

/// Per-state transitions.
pub(super) fn transitions(h: &mut Hero, c: &mut Ctx) {
    let wrapped = c.anim.view().flags & 2 != 0;
    match h.state {
        0x16 => {
            if 12.5 < c.anim.view().frame && h.air_ticks != 0 && Pf::ONE < h.height {
                h.set_state(c, 6, true);
                return;
            }
            if h.timer <= ticks(27) { return; }
            if h.health < 1 {
                h.set_state(c, 0x3d, true);
            } else if h.prev_state == 0x81 {
                h.set_state(c, 0x81, true);
            } else {
                h.set_state(c, 0, false);
            }
        }
        0x75 => {
            if h.health < 1 {
                h.set_state(c, 0x6a, true);
                h.set_anim(c.anim, c.rng, blend(18), 0x5f, 0x25);
                return;
            }
            if ticks(50) < h.timer { h.set_state(c, 0x37, false); }
        }
        0x76 => {
            if h.health < 1 {
                if !matches!(h.idle.level, 0xf | 0x11) {
                    h.set_state(c, 0x6a, true);
                    h.set_anim(c.anim, c.rng, blend(18), 0x5f, 0x28);
                } else {
                    h.set_state(c, 0x82, true);
                }
                return;
            }
            if wrapped {
                h.set_state(c, 0x34, true);
            } else if ticks(50) < h.timer {
                h.set_state(c, 0x34, false);
            }
        }
        0x3d | 0x80 | 0x82 => {
            if wrapped { death_fade(h); }
        }
        0x77 => {
            let z = h.pos[2].to_f32();
            let now = match h.idle.level {
                3 => z < 5.0,
                6 => z < 50.0,
                0x10 => z < 77.0 || (ticks(20) < h.timer && h.cap_hit != 0 && Pf::ZERO <= h.disp[2]),
                _ => false,
            };
            if now || ticks(120) < h.timer { death_fade(h); }
        }
        0x79 => {
            if ticks(300) < h.timer { death_fade(h); }
            if h.f063a != 0 { return; }
            let s = if h.air_ticks == 0 { 0 } else { 6 };
            h.set_state(c, s, true);
        }
        0x7c => {
            if h.pos[2].to_f32() < h.damage.burn_floor - 1.2 || ticks(100) < h.timer { death_fade(h); }
        }
        0x7f => {
            if ticks(220) < h.timer { death_fade(h); }
        }
        0x3c => h.tr_jump(c),
        _ => {}
    }
}

// ------------------------------------------------------------------------------------------------
// The hero update's side.

/// Right after the transitions (`hero_update_with_sounds`): the voices the entries queued, then 0x77's two spin
/// draws `randf_sym(dt·π, dt·4.363323)` (module doc: the game's order).
pub(super) fn flush(h: &mut Hero, moby: &Moby, sounds: &mut dyn HeroSounds, rng: &mut Rng) {
    for (index, flags) in std::mem::take(&mut h.damage.pending) {
        let slot = sounds.voice(moby, index, flags, rng);
        if index == 0x17 { h.damage.voice_slot = slot; }
    }
    if std::mem::take(&mut h.damage.pending_spin) {
        let a = (DT * Pf::b(0x4049_0fdb)).to_f32();
        let b = (DT * Pf::f(4.363_323)).to_f32();
        h.damage.spin_target[0] = rng.randf_sym(a, b);
        h.damage.spin_target[1] = rng.randf_sym(a, b);
    }
}
