//! The hero state machine's driver: SetState `0x23cf98` (the refusals, the previous-state bookkeeping, the
//! per-state entry through [`super::registry`], the epilogue) and the transitions `0x242930` (the prologue checks
//! every state runs first, then the per-state case through the registry). Spec: `docs/plan/player_controller.md`
//! §3; the state inventory and which module owns which state: `docs/plan/hero_states.md`.
//!
//! The per-state code lives in the group modules ([`super::ground`], [`super::walk`], [`super::air`],
//! [`super::jump`], [`super::melee`], [`super::swim`]) and the package modules. Transitions into states the port
//! does not implement are still taken exactly as the game takes them; the next [`super::hero_update`] then
//! reports [`super::HeroTick::Unimplemented`] and freezes the hero.
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::assign_op_pattern)] // FPU compare semantics and op order are spelled out on purpose.

use super::anim::AnimCtl;
use super::physics::*;
use super::Hero;
use crate::ps2v::Pf;
use crate::rng::Rng;

pub use super::common::clamp_len_2745f0;

/// Everything the transitions and SetState touch besides the hero block.
pub struct Ctx<'a, 'b> {
    pub env: &'a Env<'b>,
    pub anim: &'a mut dyn AnimCtl,
    pub rng: &'a mut Rng,
    /// `0x236738(index, flags)` played at the call point (the voice's own draws, e.g. its pitch bend, land where the
    /// game makes them, before the splash draws that follow it): the hero update passes Ratchet's sound layer
    /// ([`super::HeroSounds::voice`]). None (a SetState from outside the update): [`Ctx::voice`] queues it for
    /// [`super::fx::flush`] instead.
    pub voice: Option<&'a mut VoiceSink<'a>>,
}

/// A voice player for [`Ctx::voice`]: `(index, flags, rng)`.
pub type VoiceSink<'a> = dyn FnMut(i32, u32, &mut Rng) + 'a;

impl Ctx<'_, '_> {
    /// `0x236738(index, flags)` from SetState or the transitions: played now when the update gave a player (true),
    /// else false (the caller queues it as before).
    pub fn voice(&mut self, index: i32, flags: u32) -> bool {
        match self.voice.as_deref_mut() {
            Some(v) => {
                v(index, flags, &mut *self.rng);
                true
            }
            None => false,
        }
    }
}

impl Hero {
    // --------------------------------------------------------------------------------------------
    // SetState 0x23cf98.

    /// `SetState(id, playAnim)` (0x23cf98). Returns false when refused.
    pub fn set_state(&mut self, c: &mut Ctx, id: i32, play: bool) -> bool {
        if self.group == 0x14 && id != 100 { return false; }
        if self.health == 0 && (self.group == 2 || self.group == 7)
            && !matches!(id, 6 | 0x3d | 0x47 | 0x57 | 0x62 | 0x77 | 0x7b | 0x7c | 0x7f | 0x6a | 0x80 | 0x82)
        {
            return false;
        }
        let old_sub = self.substate;
        self.prev_prev_state = self.prev_state;
        self.prev_prev_group = self.prev_group;
        self.prev_timer = self.timer;
        self.prev_state = self.state;
        self.prev_group = self.group;
        self.state = id;
        self.substate = 0;
        // 0x1413f7 (the water states keep the wrench in hand: `update_hand_selected`) and 0x1413fc are cleared;
        // leaving 0x1413f7 set → the restore request 0x14145c (tail).
        let f7 = self.items.f13f7;
        // 0x1413f5 (the first-person camera's flag: the camera sets it again while it is up) and 0x1413fc.
        self.f13f5 = 0;
        self.items.f13fc = 0;
        if self.mode == 0 {
            self.frozen = 0;
            self.items.f13f7 = 0;
            self.f13ff = 0;
            // 0x141402 (the Hoverboard's 0x6b sets it).
            self.board.f141402 = 0;
            // 0x141630 (the guards' alert on the disguise: super::hologuise) cleared.
            self.gadgets.disguise.alert = 0;
            // 0x277740(0x1409c0): Ratchet's after-images end (crate::afterimage).
            self.fx.trails.hero.kill();
        }
        // The disguise (body 3) and a state outside its own, the walk-to states and 0 / 1: `0x22cd18` → leave the body
        // (super::hologuise; `0x231450` ends with its own `SetState(0, 1)`, then this one goes on).
        if self.mode == super::bodies::body::DISGUISE && !(0x53..=0x59).contains(&id) && !(0x65..=0x67).contains(&id) && id != 0 && id != 1 {
            let mode = self.gadgets.game_mode;
            super::bodies::leave_body(self, c, mode);
        }
        let ok = self.set_state_body(c, id, play, old_sub);
        if ok && f7 != 0 && self.items.f13f7 == 0 { self.items.restore_pending = 1; }
        // The tail's feet restore requests 0x141460 (super::worn).
        if ok { self.worn_on_set_state(); }
        ok
    }

    /// The per-state part of SetState: Ratchet's blink period is reset (0 unless the entry sets 0x68), then the
    /// entry of the state's module ([`Hero::dispatch_entry`]), then the epilogue.
    fn set_state_body(&mut self, c: &mut Ctx, id: i32, play: bool, old_sub: i32) -> bool {
        // 0x22eca0(previous, new): the arm layers of a raised weapon when leaving the idle state (super::weapons).
        super::weapons::arm_on_state_change(self, c, self.prev_state, id);
        // 0x140348: Ratchet's blink period, 0 unless the state sets 0x68 (0, 2, 4, 6 here).
        self.idle.blink_period = 0;
        if let Some(r) = self.dispatch_entry(c, id, play, old_sub) { return r; }
        self.set_state_epilogue()
    }

    fn set_state_epilogue(&mut self) -> bool {
        if self.group != self.prev_group { self.f4ec = 0; }
        self.timer = 0;
        self.substate_timer = 0;
        self.seq_timer = 0;
        true
    }

    // --------------------------------------------------------------------------------------------
    // Transitions 0x242930.

    /// `0x242930`: one pass of the state machine (after the move, with the incremented timer).
    pub fn transitions(&mut self, env: &Env, anim: &mut dyn AnimCtl, rng: &mut Rng) { self.transitions_with_voice(env, anim, rng, None) }

    /// [`Hero::transitions`] with the voices played at their call points (`voice`: see [`Ctx::voice`]).
    pub fn transitions_with_voice(&mut self, env: &Env, anim: &mut dyn AnimCtl, rng: &mut Rng, voice: Option<&mut VoiceSink<'_>>) {
        // (The player's object lifetime shortened to the context's: a coercion `Option` does not do by itself.)
        let voice = voice.map(|v| v as &mut VoiceSink);
        let mut c = Ctx { env, anim, rng, voice };
        let sub0 = self.substate;
        let seq0 = c.anim.view().seq_b;
        self.transitions_inner(&mut c);
        if self.substate != sub0 { self.substate_timer = 0; }
        if c.anim.view().seq_b != seq0 { self.seq_timer = 0; }
    }

    fn transitions_inner(&mut self, c: &mut Ctx) {
        // Prologue: the hit intake 0x231580 (moby contact damage, knockback: super::damage).
        if super::damage::hit_intake(self, c) { return; }
        if self.health < 1 && self.group != 0x14 && (self.state == 0 || self.state == 2) {
            self.set_state(c, 0x3d, true);
            return;
        }
        // The weapon check 0x240ed8 (□ with the wrench: super::melee; the other hand items: super::gadgets),
        // the water entry 0x2408e8 and the underwater / surface check 0x2406b0 (super::swim), the holster check
        // 0x2405f8 (super::gadgets).
        if self.weapon_check(c) { return; }
        if self.water_entry_check(c) || self.underwater_check(c) { return; }
        if super::gadgets::holster_check(self, c) { return; }
        if self.pos[2] < c.env.death_z && Pf::b(0x4000_0000) < self.height && self.mode != 2 && self.state != 0x77 {
            self.set_state(c, 0x77, true);
            return;
        }
        self.dispatch_transitions(c);
    }
}
