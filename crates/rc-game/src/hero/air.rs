//! Fall 6 (group 2): the SetState entry (0x23cf98 case 6 / 0x77), physics (0x2370b8, case 0x23af54) and
//! transitions (0x242930 case 6 / 0x2d). Spec: docs/plan/player_controller.md §3.1, §4.3; docs/plan/hero_states.md.
//! (0x77, the death fall, shares the entry in the game: it is [`super::damage`]'s; 0x2d, the fall after a swing,
//! shares the transitions: [`super::swingshot`].)
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::assign_op_pattern)] // FPU compare semantics and op order are spelled out on purpose.

use super::common::*;
use super::physics::*;
use super::states::Ctx;
use super::Hero;
use crate::ps2v::Pf;

impl Hero {
    /// SetState entry of 6.
    pub(super) fn fall_entry(&mut self, c: &mut Ctx, play: bool) -> Option<bool> {
        self.group_gravity = DT2 * Pf::b(0x41c0_0000);
        self.fc94 = DT2 * Pf::b(0x40e0_0000);
        self.group = 2;
        self.f15d4 = 0;
        self.substate = 0;
        self.vel = self.eff;
        self.idle.blink_period = 0x68;
        if play {
            if Pf::b(0x3fe0_0000) < self.height {
                self.set_anim(c.anim, c.rng, blend(12), 0xb, 0);
                self.substate = 1;
            } else {
                self.set_anim(c.anim, c.rng, blend(10), 10, 0);
            }
        }
        None
    }

    /// Fall 6 (0x23af54).
    pub(super) fn phys_fall(&mut self, env: &Env) {
        self.stick_target_table(env);
        if self.lockout == 0 { self.air_control(env); }
        self.lean(); // HeroLean 0x235638: super::idle.
        self.drag(Pf::b(0x3f33_3333), DT * Pf::ZERO);
        let z = self.vel[2];
        self.gravity_from(z, self.group_gravity);
        let floor = -(DT * Pf::b(0x4248_0000));
        if self.vel[2] < floor { self.vel[2] = floor; }
    }

    pub(super) fn tr_fall(&mut self, c: &mut Ctx) {
        if self.substate == 0 && (ticks(18) <= self.timer || Pf::b(0x3fe0_0000) < self.height) {
            self.set_anim(c.anim, c.rng, blend(16), 0xb, 0);
            self.substate = 1;
            self.f15d4 = 0;
        }
        if self.air_ticks == 0 {
            // HeroFootstepSound(0x14063d, 0 / 1, 1): the landing's two steps (super::fx::footstep, played after the
            // transitions).
            self.fx.footsteps.extend([(self.footstep, 0), (self.footstep, 1)]);
            let lim = DT * Pf::b(0xc110_0000);
            if self.disp[2] < lim { self.disp[2] = lim; }
            if self.vel[2] < lim { self.vel[2] = lim; }
            if self.health <= 0 { self.set_state(c, 0x3d, true); return; }
            let off_moby = self.ground_moby.is_none();
            if !(self.timer < ticks(90)) {
                if self.set_state(c, 0, false) { self.set_anim(c.anim, c.rng, blend(6), 0xc, 4); }
                if off_moby { self.lockout = ticks(22); }
                // `0x248920`: the landing's foot motes (super::pose).
                self.land_motes(super::pose::motes::LAND_HARD);
                return;
            }
            if self.substate == 1 {
                if self.set_state(c, 0, false) { self.set_anim(c.anim, c.rng, Pf::b(0xbf80_0000), 0xc, 9); }
                self.momentum[2] = Pf::ZERO;
                clamp_len_2745f0(&mut self.momentum, DT * Pf::b(0x40e0_0000));
                self.lockout = ticks(7);
                if self.prev_state == 0x2d { self.lockout = ticks(10); }
                if off_moby && ticks(50) < self.timer { self.lockout = ticks(18); }
                self.land_motes(super::pose::motes::LAND_ROLL);
                return;
            }
            if Pf::b(0x3f00_0000) < self.stick_mag && self.lockout == 0 {
                self.set_state(c, 2, true);
                self.land_motes(super::pose::motes::LAND_WALK);
                return;
            }
            if DT * Pf::b(0x4040_0000) < self.eff_len_xy {
                if self.set_state(c, 3, false) {
                    self.set_anim(c.anim, c.rng, blend(12), 6, 0);
                    clamp_len_2745f0(&mut self.momentum, DT * Pf::b(0x40f6_6666));
                }
                return;
            }
            self.set_state(c, 0, true);
            self.land_motes(super::pose::motes::LAND);
            return;
        }
        if self.health < 1 { return; }
        // Heli-Pack glide 8 (✕ held, Heli-Pack owned, height > 0.6): super::packs.
        if super::packs::fall_to_glide(self, c) { return; }
        if ticks(20) < self.f532 as i32 {
            // 0x13f524 (set by the pack glide 8) shortens the jump buffer to this tick.
            let n = if self.f524 == 0 { ticks(6) } else { 1 };
            if self.try_jump(c, n, false) { return; }
        }
        // Thruster-Pack ✕ + R1 combo → 0x10: super::packs.
        if super::packs::fall_to_thruster(self, c) { return; }
        // Ledge grab (0x13f838, set by the ledge probes of super::ledge) → 0x18; grind rail (0x13f8bc) → 0x28;
        // cable (0x13f94c) → 0x74 (super::boots).
        if self.f838 != 0 { self.set_state(c, 0x18, true); return; }
        if super::boots::rail_contact(self) { self.set_state(c, 0x28, true); return; }
        if super::boots::cable_contact(self) { self.set_state(c, 0x74, true); }
    }
}
