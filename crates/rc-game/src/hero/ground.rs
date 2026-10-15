//! The standing ground states: idle 0 (group 0), stop / skid 3 (group 1) and crouch 4 (group 0xc) — their
//! SetState entries (0x23cf98), per-state physics (0x2370b8, case 0x23713c) and transitions (0x242930).
//! Spec: docs/plan/player_controller.md §3–§4; state table docs/plan/hero_states.md. The idle behaviour
//! (fidgets, head look, blinks, the back items) is [`super::idle`].
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::assign_op_pattern)] // FPU compare semantics and op order are spelled out on purpose.

use super::common::*;
use super::physics::*;
use super::states::Ctx;
use super::{state, Hero};
use crate::pad::{button, fast_diff_rots};
use crate::ps2v::Pf;

impl Hero {
    /// SetState entries of 0, 3 and 4. `None`: continue with the epilogue; `Some(r)`: SetState returns `r`.
    pub(super) fn ground_entry(&mut self, c: &mut Ctx, id: i32, play: bool) -> Option<bool> {
        match id {
            0 => {
                self.group = 0;
                self.f15d4 = 0;
                self.fidget_timer = c.rng.rand_range(ticks(50), ticks(100));
                self.idle.clear_look();
                // In a body (L00 case 0, `0x1413f4 ≠ 0`): the per-body idle `0x227638` instead, and SetState returns 1
                // (the nested SetState made its own epilogue; super::bodies).
                if self.mode != 0 {
                    super::bodies::body_idle(self, c);
                    return Some(true);
                }
                self.momentum = self.eff;
                // Sliding on a slippery floor (0x140632) → 0x2f (level00; super::surface).
                if let Some(r) = super::surface::idle_entry(self, c) { return Some(r); }
                if self.f063a != 0 && self.f65c == 0 { return Some(self.set_state(c, 0x79, true) && false); }
                self.idle.blink_period = 0x68;
                if play {
                    if self.idle_seq() == 0x54 { self.set_anim(c.anim, c.rng, blend(18), 0x54, 0); } else { self.set_anim(c.anim, c.rng, Pf::b(0xbf80_0000), 0, 0); }
                }
            }
            3 => {
                self.group = 1;
                self.f15d4 = 0;
                self.momentum = self.eff;
                // A slippery floor (0x140632) → 0x2f (level00; super::surface).
                if let Some(r) = super::surface::stop_entry(self, c) { return Some(r); }
                if self.f063a != 0 && self.f65c == 0 { return Some(self.set_state(c, 0x79, true) && false); }
                // 0x242858: a weapon out with the arm raised: its stance instead (super::weapons).
                if super::weapons::gun_stance(self, c) { return Some(false); }
                if self.idle_seq() == 0x54 {
                    if self.set_state(c, 0, false) { self.set_anim(c.anim, c.rng, blend(18), 0x54, 0); }
                    return Some(false);
                }
                if play {
                    let v = c.anim.view();
                    let seq = if v.seq_a == 4 && 3.0 < v.frame && v.frame < 16.0 { 5 } else { 6 };
                    self.set_anim(c.anim, c.rng, blend(9), seq, 4);
                }
            }
            4 => {
                self.group = 0xc;
                self.f15d4 = 0;
                let l = len2(self.eff);
                self.speed = l;
                let cap = DT * Pf::b(0x40e0_0000);
                if cap < l { self.speed = cap; }
                if self.f063a != 0 && self.f65c == 0 { return Some(self.set_state(c, 0x79, true) && false); }
                self.idle.blink_period = 0x68;
                self.momentum = self.eff;
                clamp_len_2745f0(&mut self.momentum, cap);
                if play {
                    let b = if c.anim.view().seq_a == self.idle_seq() { ticks(9) } else { ticks(14) };
                    self.set_anim(c.anim, c.rng, Pf::from_i32(b), 0xd, 2);
                    c.anim.set_loop(3, 18);
                }
            }
            _ => {}
        }
        None
    }

    /// Ground states 0, 3, 4 (0x23713c).
    pub(super) fn phys_ground(&mut self, env: &Env, anim: &mut dyn super::anim::AnimCtl, rng: &mut crate::rng::Rng) {
        // The drag 0x233850 (0.5, 2·dt; on a slippery floor 0.7, 0: super::surface).
        super::surface::ground_drag(self);
        self.edge_brake(env, Pf::b(0x406c_cccd), Pf::ZERO);
        self.stick_target(env, Pf::ONE);
        // The look stances under the first-person camera (0x1413f5): face the camera (super::stance).
        if (self.state == 1 || self.state == 0x1e) && self.f13f5 != 0 { super::stance::first_person_turn(self, env); }
        // The stop 3 with a weapon that keeps the arm raised out: its stance instead, and nothing more this tick
        // (`0x2370b8` → `0x242858`, then the physics' end; super::weapons::gun_stance).
        if self.state == state::STOP && self.f13f8 != 0 && self.f13fa != 0 {
            let mut c = super::states::Ctx { env, anim: &mut *anim, rng: &mut *rng, voice: None };
            if super::weapons::gun_stance(self, &mut c) { return; }
        }
        if self.state == state::CROUCH {
            let seq = anim.view().seq_b;
            if Pf::b(0x3e4c_cccd) < self.stick_mag || seq == 0xe || seq == 0xf {
                self.turn_to(SCALE64 * Pf::b(0x3b03_126f), SCALE64 * Pf::b(0x3d8f_5c29), DT * Pf::b(0x40df_66f3));
                if DT * Pf::b(0x3eb2_b8c2) < self.yaw_vel.abs() {
                    let s = 0xe + (self.yaw_vel < Pf::ZERO) as u8;
                    if anim.view().seq_b != s { self.set_anim(anim, rng, Pf::from_i32(ticks(5)), s, 0); }
                    let mut sp = self.yaw_vel.abs() / (DT * Pf::b(0x4054_3b67));
                    if sp < Pf::b(0x3f0c_cccd) { sp = Pf::b(0x3f0c_cccd); }
                    if Pf::b(0x4020_0000) < sp { sp = Pf::b(0x4020_0000); }
                    self.anim_speed = sp;
                } else if self.yaw_vel == Pf::ZERO && anim.view().seq_b != 0xd {
                    self.set_anim(anim, rng, Pf::from_i32(ticks(7)), 0xd, 0);
                }
            }
        }
        self.target_speed = Pf::ZERO;
        let dec = if self.state == state::STOP {
            let mut f = self.slope_ratio + Pf::ONE;
            if Pf::ONE < f { f = Pf::ONE; }
            let dec = (DT2 * Pf::b(0x4140_0000)) * f;
            self.stop_decel = dec;
            let v = anim.view();
            if v.seq_a == v.seq_b && v.seq_a != 0x14 {
                let r = self.eff_len_xy / dec;
                if Pf::from_i32(ticks(5)) < r && Pf::from_i32(v.frame_b as i32) < Pf::b(0x40b0_0000) {
                    // 0x22a620(13, r, 0.5, −1): playback speed so the skid anim lasts the braking time.
                    self.anim_speed_for_ticks(Pf::b(0x4150_0000), r, Pf::b(0x3f00_0000), Pf::b(0xbf80_0000), &v);
                    if Pf::b(0x3f8c_cccd) < self.anim_speed { self.anim_speed = Pf::b(0x3f8c_cccd); }
                } else if Pf::b(0x40d0_0000) < Pf::from_i32(v.frame_b as i32) {
                    self.anim_speed = Pf::ONE;
                }
            }
            dec
        } else if self.state == state::CROUCH {
            // 6.48·dt² (1.7·dt² on a slippery floor: super::surface).
            super::surface::crouch_decel(self)
        } else {
            DT2 * Pf::b(0x4149_999a)
        };
        self.speed_step(Pf::ZERO, dec);
        self.vel = V0;
        self.momentum_decay(dec);
        // 0x140637 (surface 2, magnetic) && 0x13d4dc (Magneboots owned): the magnetic-floor branch (super::boots).
        if super::boots::ground_magnet(self, env) { return; }
        self.wall_check(env, 0);
        if self.air_ticks != 0 {
            self.gravity_from(self.eff_v[2], DT2 * Pf::b(0x41c8_0000));
            self.climb_check(env);
        } else {
            let z = self.vel[2];
            self.gravity_from(z, DT2 * Pf::b(0x4258_0000));
        }
    }

    pub(super) fn tr_idle(&mut self, c: &mut Ctx) {
        let pad = c.env.pad;
        if pad.held & button::STRAFE != 0 { self.set_state(c, 1, true); return; }
        c.anim.clear_loop();
        // 0x14161a (→ state 0x81) is 0 on foot. The idle animation: fidgets, the return to the idle sequence,
        // the back items and Clank's fidget (super::idle).
        // 0x14161a latched (the Thruster-Pack hover 0x81 after a landing): super::packs.
        if super::packs::hover_latched(self) { self.set_state(c, 0x81, true); return; }
        self.idle_anim(c);
        self.stick_target(c.env, Pf::ONE);
        if self.f063a != 0 && self.f65c == 0 { self.set_state(c, 0x79, true); return; }
        let air = self.air_ticks as i32;
        if ticks(4) < air && Pf::b(0x3ecc_cccd) < self.height {
            if self.prev_state == 8 && self.timer < ticks(15) { return; }
            self.set_state(c, 6, true);
            return;
        }
        if self.lockout != 0 { return; }
        let ok = !((self.prev_state == 1 || self.prev_state == 0x1e) && self.timer < ticks(5));
        // 0x13f524 (set by the pack glide 8) shortens the jump buffer to this tick.
        let n = if self.f524 == 0 { ticks(7) } else { 1 };
        if ok && self.f658 != 1 && self.try_jump(c, n, false) { return; }
        if self.f658 == 1 && pad.pressed_within(button::CROSS, ticks(7)).is_some() { self.set_state(c, 0x71, true); return; }
        if pad.held & button::CROUCH != 0 && self.grounded_ticks != 0 {
            let v = c.anim.view();
            if !(v.seq_b == 0x1a && v.frame < 108.0) { self.set_state(c, 4, true); return; }
        }
        // A grind rail under the feet (0x13f8bc, level00): 0x28 (super::boots).
        if super::boots::rail_contact(self) { self.set_state(c, 0x28, true); return; }
        // A slippery floor steeper than 5° → the slide 0x2f (level00; super::surface).
        if super::surface::idle_slide(self, c) { return; }
        if (self.prev_state == 1 || self.prev_state == 0x1e) && self.timer < ticks(24) && self.f658 == 0 {
            self.stick_target(c.env, Pf::ONE);
            if Pf::b(0x3f9c_61aa) < fast_diff_rots(self.target_yaw, c.env.cam_yaw) { return; }
        }
        if Pf::b(0x3e61_47ae) < self.stick_mag { self.set_state(c, 2, true); }
    }

    pub(super) fn tr_stop(&mut self, c: &mut Ctx) {
        let pad = c.env.pad;
        if pad.held & button::STRAFE != 0 { self.set_state(c, 1, true); return; }
        if self.lockout == 0 {
            if pad.pressed & button::CROSS != 0 { self.set_state(c, 7, true); return; }
            if pad.held & button::CROUCH != 0 { self.set_state(c, 4, true); return; }
        }
        if ticks(8) < self.air_ticks as i32 && Pf::b(0x3e99_999a) < self.height { self.set_state(c, 6, true); return; }
        if c.anim.view().flags & 2 != 0 { self.set_state(c, 0, true); return; }
        // A grind rail under the feet (0x13f8bc, level00): 0x28 (super::boots).
        if super::boots::rail_contact(self) { self.set_state(c, 0x28, true); return; }
        if !(Pf::b(0x3e4c_cccd) < self.stick_mag) { return; }
        if self.health == 1 {
            if 10.0 < c.anim.view().frame && self.set_state(c, 0, false) { self.set_anim(c.anim, c.rng, blend(17), 0x54, 0); }
        } else if self.speed == Pf::ZERO {
            self.set_state(c, 0, false);
            return;
        }
        if Pf::b(0x3f00_0000) < self.stick_mag { self.set_state(c, 2, true); }
    }

    pub(super) fn tr_crouch(&mut self, c: &mut Ctx) {
        let pad = c.env.pad;
        if pad.held & button::STRAFE != 0 { self.set_state(c, 1, true); return; }
        if ticks(11) < self.timer && pad.held & button::CROUCH == 0 {
            self.set_state(c, 0, false);
            self.set_anim(c.anim, c.rng, Pf::b(0xc000_0000), self.idle_seq(), 0);
            return;
        }
        if self.f063a != 0 && self.f65c == 0 { self.set_state(c, 0x79, true); return; }
        if self.f658 == 1 {
            if pad.pressed_within(button::CROSS, ticks(7)).is_some() { self.set_state(c, 0x71, true); return; }
        } else if self.crouch_jump(c) {
            return;
        }
        // □ with the wrench (item 8) → the Comet-Strike 0x15 (super::comet; the fall test below still runs).
        if pad.pressed_within(button::SQUARE, ticks(0xf)).is_some() && self.items.slot.id == super::items::item::WRENCH { self.set_state(c, 0x15, true); }
        if ticks(8) < self.air_ticks as i32 {
            if self.prev_state == 8 && self.timer < ticks(15) { return; }
            self.set_state(c, 6, true);
        }
    }
}
