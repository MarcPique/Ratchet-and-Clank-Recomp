//! The wrench melee: the □ trigger (`HeroPdaGadget` 0x240ed8, item 8), the ground combo 0x13 (three swings)
//! and the jump attack 0x14 — their SetState entries (0x23cf98), per-state physics (0x2370b8) and
//! transitions (0x242930) — the aim helper 0x2351d0, and the part of the wrench moby's update (0x2be1c0) that
//! tests the swing against mobys and sends the hit (`coll_sphere_mobys` 0x214468 / `CollLine_Fix` with a hit
//! template → 0x26e968). Spec: `docs/plan/player_controller.md` "Melee and item swap". Addresses level01.
//!
//! The comet strike 0x15 (crouch + □) has its entry here and its physics, the throw and the wrench's flight
//! states 10 / 11 in [`super::comet`]; the glove throw 0x23 has the group-6 entry here and the rest in
//! [`super::weapons`]; the gadget lunge 0x20 has the group-6 entry here and the rest in [`super::walloper`] (with the
//! melee aim search `0x22e238`, [`aim_search`]); the rebound 0x21 off a target whose record has flag 2 ([`Hero::rebound_entry`],
//! the physics and transitions of 0x7a: `super::packs`). Not ported: 0x51, the aim-assist target search `0x22e238` (targets need a
//! mode-0x20 record no ported class has: always none), the wall-hit spark line and the jump-attack
//! ground sparks (cosmetic + sounds) and the trail counters of the wrench (pvar +0x70/+0x74/+0x7c). The stats
//! records the entries bump (0x1416c0, 0x141848, 0x141850 + 0x1417a8) are counted in [`Melee::entered`] and
//! applied to the game state by the caller ([`apply_melee_stats`], [`MeleeRecords`]).
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::assign_op_pattern)] // FPU compare semantics and op order are spelled out on purpose.

use super::anim::{AnimCtl, AnimView};
use super::items::{HitSink, ItemEnv, HERO_LISTS};
use super::physics::*;
use super::states::Ctx;
use super::Hero;
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::services::HitTemplate;
use crate::pad::{button, fast_arctan, fast_diff_rots};
use crate::ps2v::Pf;
use rc_formats::moby_anim;

/// The wrench's look-stance reticle (`0x2be1c0`): its colour (R low) and FX texture.
pub const LOOK_MARKER_RGBA: u32 = 0xff91_7267;
pub const LOOK_MARKER_FX: usize = 0x25;

/// The melee table at 0x17c0a8 (stride 0x2c, indexed by 0x13fdb0): `[kind (0 ground combo, 1 jump attack, 2
/// comet), step, chain-before, input-ref, chain-from, jump-after, idle-after, hit-from, hit-to, +0x24,
/// +0x28]`, frames of Ratchet's sequence. Rows 0..2 = combo swings 0x17..0x19, 3 = comet 0x1a, 4 = jump attack 0x2b,
/// 5 / 6 = the Magneboots swings 0x5c / 0x5d (0x70; row 6 read from level01 memory 0x17c0a8 + 6·0x2c).
pub const COMBO: [[i32; 11]; 7] = [
    [0, 0, 33, 19, 26, 24, 31, 17, 23, 17, 24],
    [0, 1, 25, 7, 12, 13, 23, 8, 13, 6, 12],
    [0, 2, 0, 16, 23, 19, 21, 12, 16, 9, 23],
    [2, 0, 99, 83, 88, 87, 91, 99, 99, 99, 99],
    [1, 0, 99, 32, 28, 28, 30, 1, 28, 22, 25],
    [0, 0, 33, 19, 26, 24, 31, 18, 22, 17, 24],
    [0, 1, 25, 7, 17, 18, 23, 8, 13, 6, 12],
];
const C_KIND: usize = 0;
const C_STEP: usize = 1;
const C_CHAIN_BEFORE: usize = 2; // +0x08
const C_REF: usize = 3; // +0x0c
const C_CHAIN_FROM: usize = 4; // +0x10
const C_JUMP_AFTER: usize = 5; // +0x14
const C_IDLE_AFTER: usize = 6; // +0x18
const C_HIT_FROM: usize = 7; // +0x1c
const C_HIT_TO: usize = 8; // +0x20

const HALF_PI: Pf = Pf::b(0x3fc9_0fdb);
/// The "current yaw" sentinel of SetPlanarVel (99999.0).
const YAW_SENTINEL: Pf = Pf::b(0x47c3_4f80);
/// 15.009831 rad/s: the aim turn rate.
const AIM_TURN: Pf = Pf::b(0x4170_2845);
/// Wrench class (moby +0xa6).
pub const WRENCH_CLASS: i16 = 0x47;
/// The wrench's hit sound, `FUN_002bda88()`: class sound 1 when the moby hit (0x1742d8: the line's moby, or the
/// sphere's first listed moby, `coll_sphere_mobys` stores it there too) has mode bit 0x20 and the record its first
/// pvar word points to (`FUN_002711f8`, the creatures' damage record) has +9 = 1, else 0 (crates, props). The same
/// rule for the swing, the jump attack and the thrown wrench.
pub fn wrench_hit_sound(table: &MobyTable, hit: Option<MobyId>) -> i32 {
    let Some(m) = hit.and_then(|id| table.mobys.get(id)) else { return 0 };
    let rec = crate::moby_update::triggers::pvar_record(m);
    (rec.and_then(|o| m.pvars.get(o + 9)) == Some(&1)) as i32
}

/// The melee fields of the hero block (0x13fd40..0x13fdcc).
#[derive(Clone, Copy, Debug, Default)]
pub struct Melee {
    /// 0x13fd40 / 0x13fd50: Ratchet's hand (joint list 0) and the wrench head this tick; 0x13fd60 /
    /// 0x13fd70: last tick's.
    pub hand: V4,
    pub tip: V4,
    pub hand_prev: V4,
    pub tip_prev: V4,
    /// 0x13fda4: aim-assist target (never found in the port); 0x13fda8: aim set; 0x13fdac: aim yaw.
    pub target: Option<usize>,
    pub aimed: i32,
    pub aim_yaw: Pf,
    /// 0x13fdb0: row of [`COMBO`].
    pub combo: i32,
    /// 0x13fdb4: the swing hit something (one hit sound per swing).
    pub hit: i32,
    /// 0x13fdbc: combo speed factor (1.0 at entry); 0x13fdc0: jump attack descending.
    pub speed_k: Pf,
    pub falling: Pf,
    /// 0x13fdcc: toggled by each comet strike.
    pub comet_side: u32,
    /// Entries of 0x13 / 0x14 / 0x15 since the caller last applied their stats records ([`apply_melee_stats`];
    /// the hero has no game state of its own).
    pub entered: [u16; 3],
}

/// The stats records the melee SetState entries bump (0x23cf98, group 6), by name. Storage keeps the game's
/// addresses, so saves stay byte-compatible:
/// * 0x13 (ground combo) → **0x1416c0**, which the save descriptors see as level 8's slot of chunk 3007
///   (0x141680 + 8·8): the game aliases it, so it lives in `levels[8].entry`;
/// * 0x14 (jump attack) → 0x141848 = misc record 0 (chunk 17);
/// * 0x15 (comet strike) → 0x141850 = misc record 1 and 0x1417a8 = gadget-help record 17 (chunk 18).
pub trait MeleeRecords {
    fn wrench_combo_record(&mut self) -> &mut crate::game_state::HelpRec;
    fn jump_attack_record(&mut self) -> &mut crate::game_state::HelpRec;
    fn comet_records(&mut self) -> [&mut crate::game_state::HelpRec; 2];
}

impl MeleeRecords for crate::game_state::GameState {
    fn wrench_combo_record(&mut self) -> &mut crate::game_state::HelpRec { &mut self.levels[8].entry }
    fn jump_attack_record(&mut self) -> &mut crate::game_state::HelpRec { &mut self.global.move_help[0] }
    fn comet_records(&mut self) -> [&mut crate::game_state::HelpRec; 2] {
        let g = &mut self.global;
        [&mut g.move_help[1], &mut g.gadget_help[17]]
    }
}

/// One bump (the same code for every record): `count++` unless 0xffff, `time = max(time,
/// ScaleTicks(play time 0x15eea4) / 600)`, `mask |= 1 << level | 0x80000000`.
pub fn bump_record(r: &mut crate::game_state::HelpRec, level: i32, scaled_play_time: i32) {
    if r.count != 0xffff { r.count = r.count.wrapping_add(1); }
    let t = scaled_play_time / 600;
    if (r.time as i32) < t { r.time = t as u16; }
    r.mask |= (1u32 << (level as u32 & 31)) | 0x8000_0000;
}

/// Applies (and clears) the entries counted in `melee.entered` to `gs` (level 0x15ed84 = `gs.global.level`,
/// play time scaled by the session's tick scale).
pub fn apply_melee_stats(melee: &mut Melee, gs: &mut crate::game_state::GameState, session: &crate::game_state::SessionState) {
    if melee.entered == [0; 3] { return; }
    let (level, t) = (gs.global.level, session.scale_ticks(gs.global.play_time));
    for _ in 0..melee.entered[0] { bump_record(gs.wrench_combo_record(), level, t); }
    for _ in 0..melee.entered[1] { bump_record(gs.jump_attack_record(), level, t); }
    for _ in 0..melee.entered[2] { for r in gs.comet_records() { bump_record(r, level, t); } }
    melee.entered = [0; 3];
}

impl Hero {
    fn combo_row(&self) -> &'static [i32; 11] { &COMBO[(self.melee.combo.clamp(0, 6)) as usize] }

    /// `FUN_002351d0(range, cone, cone2)`: unless already aimed, aim at the stick direction when the stick is
    /// past 0.5, else the facing; then the target search `0x22e238(range, aim, cone, cone2)` over the
    /// targetable list (none in the port, see the module doc).
    /// [`Hero::melee_aim`] for the other weapon states (`super::comet`, `super::weapons`).
    pub(super) fn melee_aim_pub(&mut self, env: &Env) { self.melee_aim(env) }

    fn melee_aim(&mut self, env: &Env) {
        let mut a = self.melee.aim_yaw;
        if self.melee.aimed == 0 {
            a = self.rot[2];
            if Pf::b(0x3f00_0000) < self.stick_mag {
                self.stick_target(env, Pf::ONE);
                self.melee.aimed = 1;
                self.melee.target = None;
                a = self.target_yaw;
            }
        }
        self.melee.aim_yaw = a;
    }

    /// `FUN_0022dda0(0)` is a wrench (`0x231ac0`): the hand moby is ready and of class 0x47.
    fn hand_is_wrench(&self) -> bool { self.items.ready_item().is_some_and(|m| m.o_class == WRENCH_CLASS) }

    // --------------------------------------------------------------------------------------------
    // HeroPdaGadget 0x240ed8 (case 8: the wrench)

    /// `0x240ed8` (the weapon-use check at the top of the transitions), item 8; the other hand items go to
    /// [`super::gadgets::pda_item`] (the bomb glove's ○ throw, the Swingshot, … not ported: false). Returns true
    /// when it changed the state (the transitions then stop).
    pub fn weapon_check(&mut self, c: &mut Ctx) -> bool {
        let t0 = self.timer;
        let it = &self.items;
        // The board levels' copies let the board weapon (0x24) through the 0x1413fc gate (level16 0x223350); no other
        // level ever holds it.
        if it.f13fc != 0 && it.slot.id != super::hoverboard::WEAPON_ITEM && (it.slot.id != 8 || self.f658 != 1) { return false; }
        if self.mode != 0 || it.slot.item.is_none() { return false; }
        if it.slot.state != 2 { return false; }
        // The other hand items (Swingshot, Hologuise, PDA, …): super::gadgets.
        if it.slot.id != 8 { return super::gadgets::pda_item(self, c, t0); }
        let mask = it.slot.fire_mask;
        let pad = c.env.pad;
        let v = c.anim.view();
        let frame = Pf::f(v.frame);
        let blending = v.blending();
        let st = self.state;
        let new = 'pick: {
            if (self.group as u32) < 2 || (self.group == 4 && self.jump.landed != 0) {
                if self.f063a != 0 || pad.pressed_within(mask, ticks(7)).is_none() { break 'pick None; }
                let Some(m) = self.items.slot.item.as_ref() else { break 'pick None };
                if m.o_class != WRENCH_CLASS || m.mstate != 0 { break 'pick None; }
                let strafe = st == 1 || st == 0x1e;
                if pad.held & button::CROUCH == 0 && (!strafe || !(ticks(20) < self.timer)) {
                    break 'pick Some(if self.f658 == 1 { 0x70 } else { 0x13 });
                }
                if strafe && ticks(20) < self.timer {
                    // FUN_00236da0: the look stance's (first-person) wrench throw, no state change (super::comet).
                    super::comet::throw_wrench(self, c.env, true);
                    break 'pick None;
                }
                break 'pick Some(0x15);
            }
            let air = matches!(st, 7 | 9 | 8 | 6 | 0x2d | 0x11)
                || (st == 0xe && Pf::b(0x4200_0000) < frame)
                || (st == 0xb && if self.jump.kind7a0 != 3 { Pf::b(0x4190_0000) < frame } else { Pf::b(0x41a0_0000) < frame });
            if air && ticks(10) < self.timer {
                let n = if st == 0xb || st == 0xe { ticks(0x12) } else { ticks(10) };
                if pad.pressed_within(mask, n).is_none() { break 'pick None; }
                let h = if self.jump.descending == 0 { Pf::b(0x3dcc_cccd) } else { Pf::b(0x3f11_eb85) };
                if h < self.height || (st == 8 && Pf::b(0x3f11_eb85) < self.height) {
                    // The cable overlays (level00 0x227fa0, Kerwan 0x21aac8): □ at a cable (0x13f94c) is the
                    // wrench's grab, not the jump attack (super::boots; the flag stays 0 elsewhere).
                    if super::boots::cable_contact(self) { break 'pick None; }
                    break 'pick Some(0x14);
                }
                if self.f063a != 0 { break 'pick None; }
                break 'pick Some(0x13);
            }
            if self.group != 6 { break 'pick None; }
            let row = *self.combo_row();
            if st == 0x15 && ticks(0x1e) < self.timer {
                if blending { break 'pick None; }
                if Pf::from_i32(row[C_CHAIN_FROM]) <= frame && pad.held & button::CROUCH != 0
                    && pad.pressed_within(mask, ticks(0xf)).is_some()
                {
                    break 'pick Some(0x15);
                }
            }
            if blending || !(Pf::from_i32(row[C_CHAIN_FROM]) <= frame) { break 'pick None; }
            let mut n = frame.to_i32().wrapping_sub(row[C_REF]).wrapping_mul(2);
            if !(0 < n) { n = 1; }
            if ticks(0xf) < n { n = ticks(0xf); }
            if self.f063a != 0 || pad.pressed_within(mask, n).is_none() { break 'pick None; }
            if pad.held & button::CROUCH != 0 { break 'pick Some(0x15); }
            Some(if self.f658 == 1 { 0x70 } else { 0x13 })
        };
        if let Some(s) = new { self.set_state(c, s, true); }
        // The epilogue (state unchanged): the Thruster hover 0x81 / the 0x14161a latch (super::packs).
        if self.timer == t0 { super::packs::pda_epilogue(self, c); }
        self.timer < t0
    }

    // --------------------------------------------------------------------------------------------
    // SetState 0x23cf98, group 6

    /// The group-6 entries (0x13, 0x14, 0x15, 0x20, 0x23, 0x51): group 6, 0x13fdbc = 1, 0x1415d4 = 0, aim and
    /// target cleared, hit flag cleared; then per state.
    pub(super) fn melee_entry(&mut self, c: &mut Ctx, id: i32, play: bool) {
        self.group = 6;
        self.melee.speed_k = Pf::ONE;
        self.f15d4 = 0;
        self.melee.aimed = 0;
        self.melee.target = None;
        self.melee.hit = 0;
        if let Some(k) = match id { 0x13 => Some(0), 0x14 => Some(1), 0x15 => Some(2), _ => None } {
            self.melee.entered[k] = self.melee.entered[k].saturating_add(1);
        }
        match id {
            0x13 => {
                self.items.restore = 0;
                let old = *self.combo_row();
                let mut step = 0;
                if self.prev_group == 6 && old[C_KIND] == 0 && Pf::f(c.anim.view().frame) < Pf::from_i32(old[C_CHAIN_BEFORE]) {
                    step = (old[C_STEP] + 1) % 3;
                }
                self.melee.combo = step;
                self.melee_aim(c.env);
                if !play { return; }
                let (b, f2) = match step { 0 => (5, 0), 1 => (7, 2), _ => (7, 4) };
                let mut b = ticks(b);
                if self.melee.aimed != 0 && step < 2 {
                    b += (fast_diff_rots(self.melee.aim_yaw, self.rot[2]) * Pf::b(0x3fc0_0000)).to_i32();
                }
                self.set_anim(c.anim, c.rng, Pf::from_i32(b), (0x17 + step) as u8, f2 / 2);
                if self.hand_is_wrench() { self.items.pending_blend = Some(((step + 3) as u8, f2 / 2, b)); }
            }
            0x14 => {
                self.melee.combo = 4;
                self.f15d4 = 0x50;
                self.items.restore = 0;
                self.melee_aim(c.env);
                self.melee.falling = Pf::ZERO;
                self.items.slot.swap_timer = 10;
                if !play { return; }
                let mut b = ticks(0xb);
                if self.prev_state == 0xb && self.prev_timer < ticks(0x2a) { b = ticks(0x11); }
                self.set_anim(c.anim, c.rng, Pf::from_i32(b), 0x2b, 4);
                if self.hand_is_wrench() { self.items.pending_blend = Some((10, 4, b + 2)); }
            }
            0x23 => super::weapons::throw_entry(self, c, play),
            0x20 => super::walloper::entry(self, c, play),
            0x15 => {
                self.melee.combo = 3;
                self.items.restore = 0;
                self.melee_aim(c.env);
                self.melee.comet_side = (self.melee.comet_side == 0) as u32;
                if !play { return; }
                let mut b = ticks(10);
                if c.anim.view().seq_b == 0x1a { b = ticks(3); }
                if self.prev_state == 1 { b = ticks(0x11); }
                self.set_anim(c.anim, c.rng, Pf::from_i32(b), 0x1a, 0);
                c.anim.set_loop(6, 0x15);
            }
            _ => {}
        }
    }

    // --------------------------------------------------------------------------------------------
    // Per-state physics 0x2370b8

    /// 0x13, the ground combo (0x239780).
    pub(super) fn phys_combo(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        // On a slippery floor (0x140632) the combo only slides (level00 case 0x13 → 0x2167d0; super::surface).
        if super::surface::slippery(self) { super::surface::slide_move(self, env); return; }
        let v = anim.view();
        let frame = Pf::f(v.frame);
        let blending = v.blending();
        let row = *self.combo_row();
        if blending || frame < Pf::from_i32(row[C_HIT_TO]) { self.items.slot.swap_timer = 2; }
        self.target_speed = Pf::ZERO;
        // The target branch (0x13fda4 set, timer < 4: speed from the distance to it) is not reachable.
        let k = self.melee.speed_k;
        match row[C_STEP] {
            0 if frame < Pf::b(0x4190_0000) => self.target_speed = (DT * Pf::b(0x408c_cccd)) * k,
            1 if blending || frame < Pf::b(0x4110_0000) => {
                let c = if Pf::b(0x4100_0000) < frame {
                    Pf::b(0x4040_0000)
                } else if Pf::b(0x40e0_0000) < frame {
                    Pf::b(0x4090_0000)
                } else {
                    Pf::b(0x40b6_6666)
                };
                self.target_speed = (DT * c) * k;
            }
            2 if blending || frame < Pf::b(0x4140_0000) => self.target_speed = (DT * Pf::b(0x406c_cccd)) * k,
            _ => {}
        }
        approach(Pf::ONE, Pf::b(0x3e4c_cccd), &mut self.anim_speed);
        if self.melee.aimed != 0 || self.melee.target.is_some() {
            let mut f = SCALE64;
            if row[C_STEP] == 1 && Pf::ZERO < fast_subtract_rotations(self.melee.aim_yaw, self.rot[2]) {
                let d = fast_diff_rots(self.melee.aim_yaw, self.rot[2]);
                self.anim_speed = Pf::ONE / (d + Pf::ONE);
                f = SCALE64;
            }
            self.target_yaw = self.melee.aim_yaw;
            self.turn_to(f * Pf::b(0x3d4c_cccd), f * Pf::b(0x3e4c_cccd), DT * AIM_TURN);
        } else if !(row[C_STEP] == 1 && !blending && Pf::b(0x40e0_0000) < frame) {
            self.melee_aim(env);
        }
        self.speed_step(DT2 * Pf::b(0x4214_0000), DT2 * Pf::b(0x41e0_0000));
        let y = if self.melee.aimed != 0 { self.melee.aim_yaw } else { YAW_SENTINEL };
        self.set_planar_vel(y);
        if self.air_ticks != 0 {
            self.vel[2] = self.eff_v[2] - DT2 * Pf::b(0x41c8_0000);
        } else {
            self.vel[2] = self.vel[2] - DT2 * Pf::b(0x4258_0000);
        }
        self.edge_brake(env, Pf::b(0x406c_cccd), Pf::ZERO);
    }

    /// 0x14, the jump attack (0x2390b8). `hits` gets the landing shockwave (radius 0.4, flags 0x10000,
    /// damage 2, from Ratchet's moby) once past frame 23.5 while falling.
    pub(super) fn phys_jump_attack(&mut self, env: &Env, anim: &mut dyn AnimCtl) {
        let v = anim.view();
        let frame = Pf::f(v.frame);
        let blending = v.blending();
        let row = *self.combo_row();
        if blending || frame < Pf::from_i32(row[C_IDLE_AFTER]) { self.items.slot.swap_timer = 2; }
        self.target_speed = Pf::ZERO;
        self.speed_step(DT2 * Pf::b(0x4170_0000), DT2 * Pf::b(0x4170_0000));
        self.vel = set_len2(self.vel, self.speed);
        if self.melee.aimed != 0 || self.melee.target.is_some() {
            self.target_yaw = self.melee.aim_yaw;
            self.turn_to(SCALE64 * Pf::b(0x3d4c_cccd), SCALE64 * Pf::b(0x3e4c_cccd), DT * AIM_TURN);
        } else if frame < Pf::b(0x41c8_0000) {
            self.melee_aim(env);
        }
        let t9 = ticks(9);
        let mut up = DT * Pf::b(0x40f6_6666);
        if Pf::b(0x4010_0000) < self.height {
            up = up * Pf::b(0x3f00_0000);
        } else if Pf::b(0x3fe6_6666) < self.height {
            up = up * Pf::b(0x3f33_3333);
        }
        let acc = DT2 * Pf::b(0x4316_0000);
        let t14 = ticks(0xe);
        if !blending && Pf::b(0x41bc_0000) < frame && self.disp[2] < Pf::ZERO && self.air_ticks != 0 {
            // The shockwave (sent from here in the game; the port queues it for the hit sink of the
            // hand-item update later in this tick): sphere 0.4 at the feet − 0.5, dir = facing·2.
            let mut p = self.pos;
            p[2] = p[2] - Pf::b(0x3f00_0000);
            let c = fast_cos(self.rot[2]);
            let s = fast_sin(self.rot[2]);
            self.shockwave = Some((p, [c + c, s + s, Pf::ZERO, Pf::b(0x45af_df66)]));
        }
        if self.timer < t9 {
            let mut vz = self.vel[2];
            approach(up, acc, &mut vz);
            self.vel[2] = vz;
        } else if t14 < self.timer {
            self.vel[2] = self.vel[2] - DT2 * Pf::b(0x4302_0000);
            if self.vel[2] < Pf::ZERO { self.melee.falling = Pf::ONE; }
            let min = DT * Pf::b(0xc214_0000);
            if self.vel[2] < min { self.vel[2] = min; }
        }
    }

    // --------------------------------------------------------------------------------------------
    // Transitions 0x242930, 0x13 / 0x14 (0x243e44)

    pub(super) fn tr_melee(&mut self, c: &mut Ctx) {
        let v = c.anim.view();
        if self.state == 0x14 {
            let n = self.land_eta(c, Pf::b(0x4270_0000), DT2 * Pf::b(0x4302_0000), Pf::b(0xbf80_0000)).to_i32();
            let frame = Pf::f(v.frame);
            if self.melee.falling == Pf::ZERO {
                self.anim_speed = Pf::b(0x3ea8_f5c3);
            } else if self.air_ticks == 0 {
                self.anim_speed = if frame < Pf::b(0x41d0_0000) { Pf::b(0x4000_0000) } else { Pf::ONE };
            } else if n == 0 || !(Pf::ONE < Pf::b(0x41d0_0000) - frame) {
                self.anim_speed = Pf::ONE;
            } else {
                self.anim_speed_for_ticks(Pf::b(0x41d0_0000), Pf::from_i32(n), Pf::b(0x3f00_0000), Pf::b(0xbf80_0000), &v);
                if self.height < Pf::b(0x3f33_3333) && self.anim_speed < Pf::b(0x3f40_0000) { self.anim_speed = Pf::b(0x3f40_0000); }
            }
        }
        let v = c.anim.view();
        let frame = Pf::f(v.frame);
        let blending = v.blending();
        let row = *self.combo_row();
        let pad = c.env.pad;
        let mut idle_test = true;
        if Pf::from_i32(row[C_JUMP_AFTER]) < frame && blending {
            idle_test = false;
        } else if Pf::from_i32(row[C_JUMP_AFTER]) < frame && (self.state != 0x15 || self.f658 == 0) {
            let n = frame.to_i32().wrapping_sub(row[C_REF]).wrapping_mul(2);
            if pad.pressed_within(button::CROSS, n).is_some() {
                if pad.held & button::CROUCH == 0 {
                    if self.jump_lockout == 0 {
                        self.set_state(c, 7, true);
                        return;
                    }
                } else if self.crouch_jump(c) {
                    return;
                }
            }
        }
        if idle_test && !blending && Pf::from_i32(row[C_IDLE_AFTER]) < frame {
            self.set_state(c, 0, false);
            return;
        }
        // 0x15: the catch's loop exit / idle with another hand item (super::comet).
        if self.state == 0x15 { super::comet::transitions_tail(self, c); }
    }
}

// ------------------------------------------------------------------------------------------------
// The wrench moby's update 0x2be1c0, in hand (+0x20 == 0)

/// `FUN_002be110(v)`: the reach shrinks when the wrench points behind the facing: `d = min(|yaw − dir(v)|,
/// 105°)`, `v = setlen(v, |v|·(((105° − d)/105°)·0.3 + 0.7))`.
fn reach_by_facing(v: V4, yaw: Pf) -> V4 {
    let k = Pf::b(0x3fea_927f);
    let a = fast_arctan(v[0], v[1]);
    let mut d = fast_diff_rots(yaw, a);
    if k < d { d = k; }
    let l = len3(v);
    let f = (((k - d) / k) * Pf::b(0x3e99_999a)) + Pf::b(0x3f33_3333);
    set_len3(v, l * f)
}

/// World point of the last joint of `chain` of a moby: `FUN_002645a8(moby, list, out)` (partial evaluation,
/// `P.r3 · scale/1024`, rows, + position), on `rc_formats::moby_anim::{joint_translations, bone_points}`.
pub(super) fn list_point(p: &moby_anim::Rows, rows: &[moby_anim::V4; 3], pos: [f32; 3], scale: f32) -> V4 {
    let t = [p[3][0].to_bits(), p[3][1].to_bits(), p[3][2].to_bits(), p[3][3].to_bits()];
    let w = moby_anim::bone_points(&[t], rows, pos, scale)[0];
    w.map(Pf)
}

/// The in-hand part of the wrench update 0x2be1c0 (called from the slot loop with the slot ready): the
/// swap state byte 0x1403fc = 0, the wrench's sequence upkeep (back to sequence 1 when a sequence wraps or
/// when the hero leaves the combo on 3..5), its playback speed = Ratchet's 0x13fde0, and in group 6 the
/// swing test:
/// * hand = Ratchet's joint list 0, head = the wrench's list 1 (0x13fd40 / 0x13fd50; last tick's kept at
///   0x13fd60 / 0x13fd70); the head is re-derived as `hand + setlen(head − hand, |…| + 0.17)`, shortened by
///   [`reach_by_facing`] (not in 0x14);
/// * aim = the facing, or in 0x13 the direction to the head ± 90°;
/// * inside the hit window (not blending, frame in the combo row's [hit-from, hit-to], and the head within
///   90° of the facing except in 0x14): template {dir = (cos aim, sin aim)·(1 | 1.55), z 1, flags 0x10000,
///   damage 1 (2 for the jump attack), class 0x47}, a 5-step sweep of lines between last tick's and this
///   tick's hand → head (`FUN_0026ebe8`) and a sphere of radius 0.35 (0.47 in 0x14) at the head − 0.085
///   (`coll_sphere_mobys`); the first hit of the swing plays the wrench's hit sound (counted).
pub fn wrench_update(hero: &mut Hero, table: &mut MobyTable, anim: &dyn AnimCtl, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut crate::rng::Rng) {
    hero.items.slot.swap = 0;
    if hero.items.slot.state == 3 { return; }
    let Some(class) = hero.items.slot.item.as_ref().and_then(|m| env.data.class(m.o_class)) else { return };
    let st = hero.state;
    // Thrown (+0x20 = 10 / 11): the flight (super::comet); other non-zero states do nothing.
    match hero.items.slot.item.as_ref().map_or(0, |m| m.mstate) {
        super::comet::OUT | super::comet::BACK => return super::comet::thrown_update(hero, table, anim, env, hits, rng),
        _ => {}
    }
    // The look stance's reticle (0x2be1c0 in hand): state 1 with R1 / R2 held (0x13cae0 & 0xa), the screen-centre marker
    // `FUN_0020fb60(1.0, 0, 90°, wrench, 0xff917267, none, 0x25, −1, 4)` the Comet-Strike's first-person throw aims by.
    if st == 1 && env.pad.held & (button::R1 | button::R2) != 0 {
        let m = crate::targeting::Marker { size: 1.0, angle: 0.0, rgba: LOOK_MARKER_RGBA, at: None, fx: LOOK_MARKER_FX };
        hero.weapons.markers.register(env.frame as u64, m);
    }
    {
        let it = hero.items.slot.item.as_mut().unwrap();
        if it.mstate != 0 { return; }
        let seq = it.anim.seq_b;
        let wrapped = it.anim.flags & 2 != 0;
        let mut done = false;
        if seq == 0xe {
            if wrapped { moby_anim::set_sequence(&mut it.anim, &class.anim, 1, 0, ticks(5), &mut it.snapshot); }
            done = true;
        } else if seq != 0 && hero.items.f13fe != 0 {
            // (special groups only)
        } else if seq == 0xf || seq == 0x10 {
            moby_anim::set_sequence(&mut it.anim, &class.anim, 0xe, 0, ticks(2), &mut it.snapshot);
            done = true;
        }
        if done { return; }
        if wrapped && st != 0x3b && it.anim.seq_b != 1 {
            moby_anim::set_sequence(&mut it.anim, &class.anim, 1, 0, ticks(5), &mut it.snapshot);
        }
        // FUN_002bd968: state 0x3b only.
        if !matches!(st, 0x13 | 0x21 | 0x2b | 0x70 | 0x14) && (3..6).contains(&it.anim.seq_b) {
            moby_anim::set_sequence(&mut it.anim, &class.anim, 1, 0, ticks(4), &mut it.snapshot);
        }
        it.anim.speed = hero.anim_speed.to_f32();
    }
    if hero.group != 6 && st != 0x2b { return; }
    let v = anim.view();
    let frame = Pf::f(v.frame);
    let row = *hero.combo_row();
    let mut window = !(v.blending() || frame < Pf::from_i32(row[C_HIT_FROM]) || Pf::from_i32(row[C_HIT_TO]) < frame);
    let m = &hero.melee;
    let (hand_prev, tip_prev) = (m.hand, m.tip);
    // The two points (wrench list 1, Ratchet list 0).
    let it = hero.items.slot.item.as_ref().unwrap();
    let Some(wchain) = class.chains.get(1).filter(|c| !c.is_empty()) else { return };
    let wp = moby_anim::evaluate_chains(&class.anim, &it.anim, it.snapshot.as_ref(), &[wchain.as_slice()]);
    let tip = list_point(&wp[0], &it.rows, it.position, it.scale);
    let Some(hchain) = env.data.hero_chains.first().filter(|c| !c.is_empty()) else { return };
    let Some(hp) = anim.eval_chains_with(&[hchain.as_slice()], &hero.weapons.layers, &table.mobys[env.hero_moby].joint_mods).into_iter().next() else { return };
    let r = &table.mobys[env.hero_moby];
    let rrows: [moby_anim::V4; 3] = [0, 1, 2].map(|i| r.rows[i].map(f32::to_bits));
    let hand = list_point(&hp, &rrows, [r.position[0], r.position[1], r.position[2]], r.scale);
    let _ = HERO_LISTS;
    let yaw = hero.rot[2];
    let mut aim = yaw;
    if matches!(st, 0x13 | 0x70 | 0x2b) {
        let a = fast_arctan(tip[0] - hero.pos[0], tip[1] - hero.pos[1]);
        let q = if row[C_STEP] == 1 { -HALF_PI } else { HALF_PI };
        aim = fast_add_rotations(a, q);
    }
    let mut d = vsub(tip, hand);
    let l = len3(d);
    d = set_len3(d, l + Pf::b(0x3e2e_147b));
    // The head's turn off the facing (f20: 0 when not measured).
    let mut turn = Pf::ZERO;
    if hero.group != 0xf && st != 0x70 && st != 0x14 {
        turn = fast_diff_rots(yaw, fast_arctan(tip[0] - hero.pos[0], tip[1] - hero.pos[1]));
        if HALF_PI < turn { window = false; }
        d = reach_by_facing(d, yaw);
    }
    let tip = vadd(hand, d);
    hero.melee.hand_prev = hand_prev;
    hero.melee.tip_prev = tip_prev;
    hero.melee.hand = hand;
    hero.melee.tip = tip;
    wall_sparks(hero, table, env, hits, rng, &v, st, turn, aim);
    if !window { return; }
    let jump = st == 0x14;
    let (s, dmg) = if jump { (Pf::b(0x3fc6_6666), 2) } else { (Pf::ONE, 1) };
    let mut dir = [fast_cos(aim), fast_sin(aim), Pf::ZERO, Pf::ZERO];
    dir = set_len2(dir, s);
    dir[2] = Pf::ONE;
    dir[3] = Pf::b(0x45af_df66);
    let tmpl = HitTemplate { dir, attacker: None, flags: 0x1_0000, b18: 0, b19: 1, h1a: WRENCH_CLASS as u16, damage: Pf::from_i32(dmg), w20: 1 };
    let ignore = Some(env.hero_moby);
    // FUN_0026ebe8: five lines between last tick's and this tick's hand → head.
    let k = Pf::ONE / Pf::from_i32(5);
    let (dh, dt) = (vsub(hand, hand_prev), vsub(tip, tip_prev));
    let mut t = k;
    let mut swept = None;
    for _ in 0..5 {
        let a = vadd(vscale(dh, t), hand_prev);
        let b = vadd(vscale(dt, t), tip_prev);
        if let Some(h) = hits.line(table, a, b, 0, ignore, &tmpl) {
            swept = Some(h);
            break;
        }
        t = t + k;
    }
    // 0x2be5e4..: a moby whose target record has flag 2 (`FUN_002bdad8`) sends Ratchet back (0x21); any other plays
    // the hit sound once a swing, unless its class type is 0x14.
    let mut rebound = None;
    if let Some(Some(m)) = swept {
        if rebounds(table, m) {
            rebound = Some(m);
        } else if hero.melee.hit == 0 && class_type_of(table, hits, m) != Some(0x14) {
            hero.melee.hit = 1;
            hero.items.hit_sounds += 1;
            hero.fx.item_sounds.push(wrench_hit_sound(table, Some(m)));
        }
    }
    let mut v2 = vsub(tip, hand);
    let l2 = len3(v2);
    v2 = set_len3(v2, l2 - Pf::b(0x3dae_147b));
    let c = vadd(v2, hand);
    let mut r = Pf::b(0x3eb3_3333);
    if hero.group == 0xf { r = Pf::b(0x3f33_3333); }
    if jump { r = Pf::b(0x3ef0_a3d7); }
    let sphere = hits.sphere(table, r, c, 0, ignore, &tmpl);
    // (0x1742d8 is the last test's moby: the sphere's when it listed one, else still the line's.)
    let last = sphere.or(match swept { Some(Some(m)) => Some(m), _ => None });
    if let Some(m) = sphere.filter(|&m| class_type_of(table, hits, m) != Some(0x12)) {
        if rebounds(table, m) {
            rebound = Some(m);
        } else if hero.melee.hit == 0 {
            hero.melee.hit = 1;
            hero.items.hit_sounds += 1;
            hero.fx.item_sounds.push(wrench_hit_sound(table, sphere));
        }
    }
    // The rebound: `SetState(0x21, 1)` (after the slot loop, `Weapons::deferred`) and its direction 0x13fdb8: from the
    // moby hit last to Ratchet, else behind him.
    if rebound.is_some() {
        hero.weapons.deferred = Some(0x21);
        hero.packs.rebound_yaw = match last.and_then(|m| table.mobys.get(m)) {
            Some(m) => fast_arctan(hero.pos[0] - Pf::f(m.position[0]), hero.pos[1] - Pf::f(m.position[1])).to_f32(),
            None => fast_add_rotations(hero.rot[2], Pf::b(0x4049_0fdb)).to_f32(),
        };
    }
}

/// The swing's wall line and the jump attack's ground line (`0x2be7d4..0x2be9e0`):
/// * not in 0x2b / 0x70, the head within 70° of the facing (0 when not measured: the jump attack, grinding), more than
///   10 ticks into the state, nothing hit yet this swing (0x13fdb4): a line (flags 2) from 0.5 above Ratchet 1.4 toward
///   the head (its length by the facing, `0x2be110`); a hit on a surface (`CollType` ≠ 0) with no moby, a non-crate or
///   the crate class 502 (`0x273228`) → hit, the class sound 2, the burst ([`super::fx::wrench_burst`]), a moby hit
///   (damage 1, flags 0x10000, along the aim);
/// * 0x14 with Ratchet's key time passing 27: the class sound 2, a line (flags 2) from 0.7 above the head to 0.9 below
///   (`0x248d80`); a hit → the burst.
#[allow(clippy::too_many_arguments)]
fn wall_sparks(hero: &mut Hero, table: &mut MobyTable, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut crate::rng::Rng, view: &AnimView, st: i32, turn: Pf, aim: Pf) {
    let cam = env.camera.map_or([0.0; 3], |c| c.0);
    let p = hero.pos;
    if st != 0x2b && st != 0x70 && turn < Pf::b(0x3f9c_61aa) && ticks(10) < hero.timer && hero.melee.hit == 0 {
        let a = [p[0], p[1], p[2] + Pf::b(0x3f00_0000), p[3]];
        let tip = hero.melee.tip;
        let y = fast_arctan(tip[0] - p[0], tip[1] - p[1]);
        let k = Pf::b(0x3fb3_3333);
        let v = reach_by_facing([fast_cos(y) * k, fast_sin(y) * k, Pf::ZERO, Pf::ZERO], hero.rot[2]);
        let b = vadd(a, v);
        if let Some(Some(pr)) = hits.probe_moby(table, a, b, 2, None) {
            // `0x273228`: `FUN_00273278` (a live moby of class 500..540) false, or the class 502.
            let crate_ok = |m: MobyId| table.mobys.get(m).is_none_or(|x| (x.o_class as i32 - 500) as u32 & 0xffff >= 0x29 || x.o_class == 0x1f6);
            if pr.surface != 0 && pr.moby.is_none_or(crate_ok) {
                hero.melee.hit = 1;
                hero.fx.item_sounds.push(2);
                super::fx::wrench_burst(hero, rng, pr.point, cam);
                if let Some(m) = pr.moby {
                    let dir = [fast_cos(aim), fast_sin(aim), Pf::ZERO, Pf::ZERO];
                    let tmpl = HitTemplate { dir, attacker: None, flags: 0x1_0000, b18: 0, b19: 0, h1a: WRENCH_CLASS as u16, damage: Pf::ONE, w20: 1 };
                    hits.deliver(table, m, &tmpl);
                }
            }
        }
    }
    if st == 0x14 && super::comet::passed(view, 27.0) {
        hero.fx.item_sounds.push(2);
        let tip = hero.melee.tip;
        let a = [tip[0], tip[1], tip[2] + Pf::b(0x3f33_3333), tip[3]];
        let b = [tip[0], tip[1], tip[2] - Pf::b(0x3f66_6666), tip[3]];
        if let Some(Some(pr)) = hits.probe_moby(table, a, b, 2, None) { super::fx::wrench_burst(hero, rng, pr.point, cam); }
    }
}

/// `FUN_002bdad8`: the moby's target record (`targeting::record`, `FUN_002711f8`) has flag 2 in its +0x1e.
pub fn rebounds(table: &MobyTable, id: MobyId) -> bool {
    let Some(m) = table.mobys.get(id) else { return false };
    let Some(r) = crate::targeting::record(m) else { return false };
    m.pvars.get(r + 0x1e..r + 0x20).is_some_and(|b| u16::from_le_bytes([b[0], b[1]]) & 2 != 0)
}

/// The class type byte (+0x46) of moby `id`'s class, through the hit sink's class data.
fn class_type_of(table: &MobyTable, hits: &dyn HitSink, id: MobyId) -> Option<u8> { table.mobys.get(id).and_then(|m| hits.class_type(m.o_class)) }

impl Hero {
    /// SetState 0x21 (0x23cf98): group 10, speed 9·dt, 0x1415d4 = 0; with `play` Ratchet's sequence 0x27 + the combo
    /// row's step (2 for the jump attack's row) from frame 5 over ticks(5), and the wrench's 7 + step alike.
    pub(super) fn rebound_entry(&mut self, c: &mut Ctx, play: bool) {
        self.group = 10;
        self.speed = DT * Pf::from_i32(9);
        self.f15d4 = 0;
        if !play { return; }
        let row = *self.combo_row();
        let step = if row[C_KIND] == 1 { 2 } else { row[C_STEP] };
        let b = ticks(5);
        self.set_anim(c.anim, c.rng, Pf::from_i32(b), (0x27 + step) as u8, 5);
        if self.hand_is_wrench() { self.items.pending_blend = Some(((7 + step) as u8, 5, b)); }
    }
}

/// The jump attack's shockwave queued by 0x14's physics: `coll_sphere_mobys(0.4, centre, 0x10, Ratchet,
/// {dir, Ratchet's moby, flags 0x10000, damage 2, class 0x47})`.
pub fn jump_attack_shockwave(hero: &mut Hero, table: &mut MobyTable, env: &ItemEnv, hits: &mut dyn HitSink) {
    let Some((p, dir)) = hero.shockwave.take() else { return };
    let tmpl = HitTemplate { dir, attacker: Some(env.hero_moby), flags: 0x1_0000, b18: 0, b19: 1, h1a: WRENCH_CLASS as u16, damage: Pf::b(0x4000_0000), w20: 1 };
    hits.sphere(table, Pf::b(0x3ecc_cccd), p, 0x10, Some(env.hero_moby), &tmpl);
}

/// One moby of the target list 0x1abe80 as the melee aim search `0x22e238` reads it: its position (+0x10), its record's
/// health (`FUN_002711f8(m)` +0x00; mobys without a record are skipped when the list is built), mode 0x1000 and the
/// crate test `0x273278` (classes 500..=540).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeleeTarget {
    pub id: MobyId,
    pub pos: [f32; 3],
    pub health: f32,
    pub targetable: bool,
    pub is_crate: bool,
    /// The record's `+0x38` (the head look's range), `+0x39` (its priority), `+0x3a` (its height ×0.125):
    /// `HeroScanTargets`' bytes (`super::pose`).
    pub look: [u8; 3],
}

/// The target list `list` (0x1abe80, `crate::targeting::target_list`) as [`MeleeTarget`]s (the mobys with a record).
pub fn melee_targets(table: &MobyTable, list: &[MobyId]) -> Vec<MeleeTarget> {
    list.iter()
        .filter_map(|&id| {
            let m = table.mobys.get(id)?;
            let health = crate::targeting::record_health(m)?;
            let pos = [m.position[0], m.position[1], m.position[2]];
            let targetable = m.mode & crate::moby_runtime::mode::TARGETABLE != 0;
            let r = crate::targeting::record(m)?;
            let look = [0x38, 0x39, 0x3a].map(|o| m.pvars.get(r + o).copied().unwrap_or(0));
            Some(MeleeTarget { id, pos, health, targetable, is_crate: (0..=40).contains(&(m.o_class as i32 - 500)), look })
        })
        .collect()
}

/// `FUN_0022dff0(aim, range, cone, cone2, moby, &reject)`: the score of one target for the melee aim search, None when
/// rejected: farther than `range` (3D, from Ratchet's position; in the jump attack 0x14 from his ground height
/// 0x13f628), more than `cone` (> 0) from `aim` in yaw or more than `cone2` (> 0) in elevation; the score is `d +
/// yaw_off·d`, + 7 for a crate. The look stances' branch (states 1 / 0x1e: the angles from the camera) is not reachable
/// from its callers (their SetState entries and physics run in their own states) and is not ported.
pub fn aim_score(from: [f32; 3], aim: f32, range: f32, cone: f32, cone2: f32, t: &MeleeTarget) -> Option<f32> {
    let d = [t.pos[0] - from[0], t.pos[1] - from[1], t.pos[2] - from[2]];
    let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let mut reject = range < dist;
    let yaw_off = crate::moby_update::creature::diff_rots(d[1].atan2(d[0]), aim);
    let flat = (d[0] * d[0] + d[1] * d[1]).sqrt();
    let pitch_off = crate::moby_update::creature::diff_rots(d[2].atan2(flat), 0.0);
    if 0.0 < cone && cone < yaw_off { reject = true; }
    if 0.0 < cone2 && cone2 < pitch_off { reject = true; }
    if reject { return None; }
    let mut score = dist + yaw_off * dist;
    if t.is_crate { score += 7.0; }
    Some(score)
}

/// `FUN_0022e238(range, aim, cone, cone2)`: the best-scoring (lowest, the first on a tie) target of the list with a
/// non-zero health that is targetable ([`aim_score`]).
pub fn aim_search(targets: &[MeleeTarget], from: [f32; 3], aim: f32, range: f32, cone: f32, cone2: f32) -> Option<MeleeTarget> {
    let mut best = 1e8f32;
    let mut out = None;
    for t in targets {
        if t.health == 0.0 || !t.targetable { continue; }
        if let Some(s) = aim_score(from, aim, range, cone, cone2, t) {
            if s < best {
                best = s;
                out = Some(*t);
            }
        }
    }
    out
}

impl Hero {
    /// `FUN_002351d0(range, cone, cone2)` with its target search: unless already aimed (0x13fda8), the facing, or the
    /// stick's direction when it is past 0.5 (then aimed, the target cleared); a target of [`aim_search`] from there
    /// turns the aim toward it and becomes 0x13fda4 (a target kept from before with the stick not taken keeps the
    /// aim). Its callers in the port: the gadget lunge 0x20 (`super::walloper`, (11, 50°, −1)). The wrench states still
    /// use [`Hero::melee_aim`] without the search (G-WPN-008).
    pub(super) fn aim_assist(&mut self, env: &Env, range: f32, cone: f32, cone2: f32) {
        let mut a = self.melee.aim_yaw;
        if self.melee.aimed == 0 {
            a = self.rot[2];
            if Pf::b(0x3f00_0000) < self.stick_mag {
                self.stick_target(env, Pf::ONE);
                self.melee.aimed = 1;
                self.melee.target = None;
                a = self.target_yaw;
            }
            let mut from = super::physics::to_f32x3(self.pos);
            if self.state == 0x14 { from[2] = self.ground_z.to_f32(); }
            let targets = env.world.map_or(&[][..], |w| w.melee_targets());
            if let Some(t) = aim_search(targets, from, a.to_f32(), range, cone, cone2) {
                if self.melee.target.is_some() && self.melee.aimed == 0 {
                    self.melee.aim_yaw = a;
                    return;
                }
                let h = super::physics::to_f32x3(self.pos);
                a = fast_arctan(Pf::f(t.pos[0]) - Pf::f(h[0]), Pf::f(t.pos[1]) - Pf::f(h[1]));
                self.melee.target = Some(t.id);
            }
        }
        self.melee.aim_yaw = a;
    }
}

/// For the tests: the view fields the melee code reads.
pub fn frame_of(v: &AnimView) -> Pf { Pf::f(v.frame) }

/// The hit window of the in-hand wrench update: not blending and `frame` in the row's [hit-from, hit-to].
pub fn in_hit_window(combo: i32, frame: Pf, blending: bool) -> bool {
    let row = &COMBO[combo.clamp(0, 6) as usize];
    !(blending || frame < Pf::from_i32(row[C_HIT_FROM]) || Pf::from_i32(row[C_HIT_TO]) < frame)
}

#[cfg(test)]
mod tests {
    use super::super::items::{self, HandItem, ItemClass, ItemData, ItemDef, ItemEnv, ItemGlobals};
    use super::super::testkit::*;
    use super::super::{hero_update, AnimView, HeroTick};
    use super::*;
    use crate::pad::PadInput;
    use crate::rng::Rng;
    use rc_formats::moby_anim::{AnimState, MobyAnimClass};

    /// A data-free animation whose readout the tests can follow: `set_anim` blends for `blend` ticks (the
    /// readout jumps to the target frame), then the readout advances by the playback speed per tick; a
    /// sequence "ends" at frame 40.
    #[derive(Default)]
    struct FrameAnim {
        v: AnimView,
        blend_left: i32,
        calls: Vec<(Pf, u8, i32)>,
    }

    impl AnimCtl for FrameAnim {
        fn set_anim(&mut self, blend: Pf, seq: u8, frame: i32) {
            self.calls.push((blend, seq, frame));
            self.v.seq_a = 0xff;
            self.v.seq_b = seq;
            self.v.frame_b = frame as u8;
            self.blend_left = if Pf::ZERO < blend { blend.to_i32() } else { 1 };
        }
        fn advance(&mut self, speed: Pf) {
            self.v.flags = 0;
            let old = self.v.frame;
            if self.v.seq_a != self.v.seq_b {
                self.blend_left -= 1;
                if self.blend_left <= 0 {
                    self.v.seq_a = self.v.seq_b;
                    self.v.frame = self.v.frame_b as f32;
                }
            } else {
                self.v.frame += speed.to_f32();
                if self.v.frame >= 40.0 { self.v.flags = 2; }
            }
            self.v.frame_step = if old <= self.v.frame { self.v.frame - old } else { 0.0 };
        }
        fn view(&self) -> AnimView { self.v }
        fn frame_count(&self, _: u8) -> u8 { 40 }
        fn set_loop(&mut self, _: i32, _: i32) {}
        fn clear_loop(&mut self) {}
    }

    fn wrench() -> HandItem {
        let anim = AnimState { seq_a: 1, frame_a: 0, seq_b: 1, frame_b: 0, t: 0.0, speed: 1.0, rate: 1.0, flags: 0, trigger_count: 0, skip_advance: false };
        HandItem { o_class: WRENCH_CLASS, mstate: 0, anim, snapshot: None, scale: 1.0, position: [0.0; 3], rows: [[0; 4]; 3], hit_timer: 0, flight: Default::default() }
    }

    /// Ratchet on a flat floor with the wrench in hand (slot ready, fire mask □).
    fn runner() -> (crate::hero::testkit::Runner, rc_formats::collision::Collision, FrameAnim) {
        let coll = floor(100.0, 100, 106, 100, 106);
        let mut r = Runner::new([410.0, 410.0, 100.0], 0.0);
        r.hero.items.slot = items::HandSlot { item: Some(wrench()), fire_mask: button::SQUARE, state: 2, id: 8, ..Default::default() };
        let mut a = FrameAnim::default();
        for _ in 0..3 { tick(&mut r, &coll, &mut a, PadInput::neutral()); }
        (r, coll, a)
    }

    fn tick(r: &mut crate::hero::testkit::Runner, coll: &rc_formats::collision::Collision, a: &mut FrameAnim, input: PadInput) -> HeroTick {
        r.pad.update(Some(&input.bytes()), false);
        let env = Env { coll, pad: &r.pad, cam_yaw: r.cam_yaw, cam_rows: r.cam_rows, mirror: false, death_z: Pf::ZERO, mobys: None, hero_moby: None, water: None, world: None };
        hero_update(&mut r.hero, &mut r.moby, &env, a, &mut r.rng)
    }

    fn sq() -> PadInput { PadInput::neutral().press(button::SQUARE) }

    #[test]
    fn square_starts_the_combo_and_chains_in_the_window() {
        let (mut r, coll, mut a) = runner();
        tick(&mut r, &coll, &mut a, sq());
        assert_eq!((r.hero.state, r.hero.group, r.hero.melee.combo), (0x13, 6, 0));
        assert_eq!(a.calls.last(), Some(&(Pf::from_i32(5), 0x17, 0)));
        assert_eq!(r.hero.items.pending_blend, Some((3, 0, 5)));
        // Swing 1 plays; a second □ before frame 26 (chain-from) is ignored, but it stays buffered: the
        // window is (int(frame) − 19)·2 ticks (1..15), so a press at frame ≥ 22 still counts at 26.
        let mut chained_at = None;
        for t in 0..60 {
            let early = t == 10;
            let at_23 = (a.v.frame as i32) == 23 && a.v.seq_a == a.v.seq_b;
            let inp = if early || at_23 { sq() } else { PadInput::neutral() };
            tick(&mut r, &coll, &mut a, inp);
            if r.hero.melee.combo == 1 && chained_at.is_none() { chained_at = Some(a.calls.last().unwrap().2); break; }
        }
        assert_eq!(r.hero.state, 0x13);
        assert_eq!(r.hero.melee.combo, 1, "second swing");
        assert_eq!(a.calls.last().map(|c| (c.0, c.1)), Some((Pf::from_i32(7), 0x18)));
        assert_eq!(chained_at, Some(1));
    }

    /// The rebound 0x21: a target record with flag 2 (+0x1e) sends Ratchet back; the entry (group 10, 9 u/s, his
    /// sequence 0x27 + step from frame 5, the wrench's 7 + step), then 0x7a's brake along 0x13fdb8.
    #[test]
    fn a_flag_2_target_rebounds_the_wrench() {
        let mut m = crate::moby_runtime::Moby { mode: 0x20, pvars: vec![0; 0x40], ..crate::moby_runtime::Moby::default() };
        m.pvars[0..4].copy_from_slice(&8u32.to_le_bytes());
        let mut t = MobyTable::new(vec![m], 1);
        assert!(!rebounds(&t, 0));
        t.mobys[0].pvars[8 + 0x1e] = 2;
        assert!(rebounds(&t, 0));
        let (mut r, coll, mut a) = runner();
        tick(&mut r, &coll, &mut a, sq());
        assert_eq!(r.hero.state, 0x13);
        r.hero.weapons.deferred = Some(0x21);
        r.hero.packs.rebound_yaw = 1.0;
        {
            let env = Env { coll: &coll, pad: &r.pad, cam_yaw: r.cam_yaw, cam_rows: r.cam_rows, mirror: false, death_z: Pf::ZERO, mobys: None, hero_moby: None, water: None, world: None };
            let mut c = crate::hero::states::Ctx { env: &env, anim: &mut a, rng: &mut r.rng, voice: None };
            crate::hero::weapons::after_items(&mut r.hero, &mut c);
        }
        assert_eq!((r.hero.state, r.hero.group), (0x21, 10));
        assert_eq!(a.calls.last(), Some(&(Pf::from_i32(5), 0x27, 5)));
        let v0 = r.hero.speed;
        tick(&mut r, &coll, &mut a, PadInput::neutral());
        assert!(r.hero.speed < v0, "braking");
        assert!((r.hero.target_yaw.to_f32() - crate::moby_update::creature::add_rot(1.0, std::f32::consts::PI)).abs() < 1e-5, "faces away from the push");
    }

    #[test]
    fn a_late_press_does_not_chain_and_the_combo_ends_in_idle() {
        let (mut r, coll, mut a) = runner();
        tick(&mut r, &coll, &mut a, sq());
        let mut states = Vec::new();
        for _ in 0..60 {
            tick(&mut r, &coll, &mut a, PadInput::neutral());
            states.push((r.hero.state, a.v.frame));
        }
        // Blend 5 ticks, then the readout counts 0, 1, …; idle (SetState(0, no anim)) once frame > 31.
        let idle = states.iter().position(|s| s.0 == 0).unwrap();
        assert!(states[idle - 1].1 <= 31.0 && states[idle].1 > 31.0, "{:?}", &states[idle - 2..idle + 1]);
        // A press after frame 33 (chain-before) starts swing 1 again, not swing 2.
        tick(&mut r, &coll, &mut a, sq());
        assert_eq!((r.hero.state, r.hero.melee.combo), (0x13, 0));
    }

    #[test]
    fn third_swing_wraps_to_the_first() {
        let (mut r, coll, mut a) = runner();
        tick(&mut r, &coll, &mut a, sq());
        let mut seen = vec![0];
        for _ in 0..200 {
            let from = COMBO[r.hero.melee.combo as usize][C_CHAIN_FROM] as f32;
            let inp = if a.v.seq_a == a.v.seq_b && a.v.frame >= from && r.hero.state == 0x13 { sq() } else { PadInput::neutral() };
            tick(&mut r, &coll, &mut a, inp);
            if r.hero.state == 0x13 && *seen.last().unwrap() != r.hero.melee.combo { seen.push(r.hero.melee.combo); }
            if seen.len() >= 3 { break; }
        }
        // Swing 2 (row 1) chains at frame ≥ 12 (< 25); swing 3 (row 2) has chain-before 0: the next press
        // restarts at swing 1.
        assert_eq!(seen, vec![0, 1, 2]);
        // Swing 3's idle-after (21) comes before its chain-from (23): it always ends in idle, and □ there
        // starts swing 1 (previous group 0).
        let mut st = Vec::new();
        for _ in 0..40 { tick(&mut r, &coll, &mut a, PadInput::neutral()); st.push(r.hero.state); }
        assert_eq!(*st.last().unwrap(), 0);
        tick(&mut r, &coll, &mut a, sq());
        seen.push(r.hero.melee.combo);
        assert_eq!(seen, vec![0, 1, 2, 0]);
    }

    #[test]
    fn hit_frames() {
        let w = |c: i32| (0..40).filter(|&f| in_hit_window(c, Pf::from_i32(f), false)).collect::<Vec<_>>();
        assert_eq!(w(0), (17..=23).collect::<Vec<_>>());
        assert_eq!(w(1), (8..=13).collect::<Vec<_>>());
        assert_eq!(w(2), (12..=16).collect::<Vec<_>>());
        assert_eq!(w(4), (1..=28).collect::<Vec<_>>());
        assert!(!in_hit_window(0, Pf::from_i32(20), true), "no hits while the anim blends");
    }

    #[test]
    fn square_in_the_air_is_the_jump_attack() {
        let (mut r, coll, mut a) = runner();
        tick(&mut r, &coll, &mut a, PadInput::neutral().press(button::CROSS));
        for _ in 0..11 { tick(&mut r, &coll, &mut a, PadInput::neutral()); }
        assert_eq!(r.hero.state, 7);
        assert!(r.hero.height > Pf::b(0x3dcc_cccd));
        tick(&mut r, &coll, &mut a, sq());
        assert_eq!((r.hero.state, r.hero.melee.combo, r.hero.items.slot.swap_timer), (0x14, 4, 10));
        assert_eq!(a.calls.last(), Some(&(Pf::from_i32(11), 0x2b, 4)));
        // It lands and ends in idle (frame > 30) without freezing.
        let mut last = HeroTick::Ran;
        for _ in 0..120 { last = tick(&mut r, &coll, &mut a, PadInput::neutral()); }
        assert_eq!(last, HeroTick::Ran);
        assert_eq!(r.hero.state, 0);
        assert!(r.hero.grounded());
    }

    #[test]
    fn swap_to_the_glove_and_back() {
        let (mut r, _coll, _a) = runner();
        let empty = MobyAnimClass { joint_count: 1, skeleton: vec![], rest: vec![[0.0; 3]], parent_word: vec![0], sequences: vec![] };
        let cls = |o| ItemClass { o_class: o, anim: empty.clone(), scale: 1.0, chains: vec![] };
        let mut defs = vec![ItemDef::default(); 37];
        defs[8] = ItemDef { slot: 0, attach: 0, o_class: 0x47, b18: 0 };
        defs[10] = ItemDef { slot: 0, attach: 6, o_class: 0xc0, b18: 0 };
        let data = ItemData { defs, hero_chains: vec![], classes: vec![cls(0x47), cls(0xc0)] };
        let mut g = ItemGlobals { request: 10, saved: 10, previous: 0, wrench_flag: 1, drone: false };
        let mut rng = Rng::new();
        let mut table = crate::moby_runtime::MobyTable::new(vec![crate::moby_runtime::Moby::zeroed()], 4);
        let pad = crate::pad::PadState::default();
        let anim = FrameAnim::default();
        let mut frame = 100;
        let mut hand = Vec::new();
        for _ in 0..8 {
            let env = ItemEnv { data: &data, pad: &pad, frame, hero_moby: 0, coll: None, camera: None, camera_up: None, targets: &[] };
            items::items_update(&mut r.hero, &mut g, &mut table, &anim, &mut rng, &env, &mut items::NoHits);
            hand.push((r.hero.items.slot.id, r.hero.items.slot.state, r.hero.items.slot.item.as_ref().map(|m| m.o_class)));
            frame += 1;
        }
        // Tick 0: the swap starts (put away, state 3, request consumed, the wrench flag cleared, one RNG draw);
        // tick 1: deleted, the gate at frame + 2; ticks 2–3 empty; tick 4: the glove (class 0xc0, fire ○).
        assert_eq!(hand[0], (8, 3, Some(0x47)));
        assert_eq!(hand[1], (0, 0, None));
        assert_eq!(hand[2], (0, 0, None));
        assert_eq!(hand[3], (0, 0, None));
        assert_eq!(hand[4], (10, 2, Some(0xc0)));
        assert_eq!((g.request, g.wrench_flag, g.saved), (0, 0, 10));
        assert_eq!(r.hero.items.slot.fire_mask, button::CIRCLE);
        // □ with the glove in hand swaps back to the wrench.
        let mut p = crate::pad::PadState::default();
        p.update(Some(&PadInput::neutral().press(button::SQUARE).bytes()), false);
        let env = ItemEnv { data: &data, pad: &p, frame, hero_moby: 0, coll: None, camera: None, camera_up: None, targets: &[] };
        items::items_update(&mut r.hero, &mut g, &mut table, &anim, &mut rng, &env, &mut items::NoHits);
        assert_eq!((r.hero.items.target, r.hero.items.slot.state, g.wrench_flag), (8, 3, 1));
    }
}
