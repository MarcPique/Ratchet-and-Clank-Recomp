//! **Pokitaru's story pickups and NPCs** (level 11; read from the level11 decomp and disassembly):
//!
//! * **23 the O2 Mask prop** (`0x2cb668`, census U360, one instance): hidden 0.75 above its spot until another class
//!   puts it in state 2; then, within 2 (XY), the O2 Mask (item 6) is owned and acquired (direct stores), a save, the
//!   help 11000; gone. Gone at once when the mask is owned.
//! * **90 the Thruster-Pack giver** (`0x2d0710`, U363, one instance): talks only inside his cuboid +0x40; node 2 gives
//!   the Thruster-Pack (item 3), the checkpoint at Ratchet, a save and the help 11003 (the help box first closed).
//! * **298 the Persuader giver** (`0x2f0e40`, U365, one instance): near him (4, 3-D) without the Persuader and with
//!   global flag 1 clear, the help 11011 reminder (record 0x86); within 4 (and 4 in z) his mission and the
//!   checkpoint at cuboid +0x48; node 2 gives the Persuader (item 35, owned and acquired by direct stores), clears
//!   flag 1, shows banner 11014 for `ticks(300)`, saves and asks help 11001.
//!
//! **System or not.** Per-class code from the NPC template (the look-at layout rows), the talk system, `story` (the
//! direct item stores), the help records (`hints::remind`).
//!
//! ## 23 coverage (level11 `0x2cb668`)
//!
//! | address | what | status |
//! |---|---|---|
//! | state 0 | O2 Mask owned (0x13d4c6) → `DeleteMoby`; else → 1, mode \|= 1, z += 0.75 | [`mask_update`] |
//! | state 1 | nothing (another class sets state 2) | [`mask_update`] [L: no ported writer] |
//! | state 2 | `add_rot(yaw, 3π/2·dt)` computed and dropped (no store: 0x2cb710); within 2 (XY): 0x13d4c6 = 1, 0x13d4ee = 1, `memcard_Save`, `Help_Request(11000, 0x3a)`, `DeleteMoby` | [`mask_update`] (`story::set_owned` / `set_acquired`) |
//!
//! ## 90 coverage (level11 `0x2d0710`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x2d0638` (the big-head scene manipulator) | `manip::scene_big_head` |
//! | state 0 | Thruster-Pack owned (0x13d4c3) → 3, else 1; `NpcTalkRegister`; radius 3.7 | [`thruster_update`] |
//! | state 1 | in cuboid +0x40: `NpcTalkUpdate` → `PlaceAfterScene(2.7)`, → 2 | [`thruster_update`] |
//! | state 2 | game mode ≠ 2: → 1; node played 2: `GiveItem(3, 1)`, the checkpoint at Ratchet, `memcard_Save`, the help box killed, `Help_Request(11003, 0x3d)` | [`thruster_update`] |
//! | tail | the look-at (records +0x70 / +0xf0, glance +0x170, s32 +0x184 / +0x188, pitch × 1, yaw 0.5 / 0.5) | [`talking_npc::look_at_layout`] |
//!
//! ## 298 coverage (level11 `0x2f0e40`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x2f0d58` (the big-head scene manipulator); drawn within 30: the shadow probe, +0x7f = 0x18 | the probe: [`persuader_update`]; the cheat: `manip::scene_big_head` |
//! | state 0 | → 1; seq ≠ 1 → blend 1 (`ticks(20)`); Persuader acquired (0x13d50b) → `DeleteMoby`; `NpcTalkRegister`; radius 4 | [`persuader_update`] |
//! | state 1 | `NpcTalkUpdate` → `PlaceAfterScene(3)`, → 2; flag 1 clear, Persuader not owned, within 4 (3-D): the reminder on help record 0x86 → `Help_Request(11011, 0x86)`; mission not done, within 4 and \|Δz\| < 4: `SetMissionDone(+0xb0)`, the checkpoint at cuboid +0x48 | [`persuader_update`] (`hints::remind`) |
//! | state 2 | game mode ≠ 2: → 1; node played 2: 0x13d4e3 = 1, 0x13d50b = 1, flag 1 = 0, `ShowBanner(11014, ticks(300))`, `memcard_Save`, `Help_Request(11001, 0x3b)` | [`persuader_update`] |
//! | tail | the look-at (records +0x50 / +0xd0, glance +0x150, s32 +0x160 / +0x164, pitch × 1, yaw 0.5 / 0.5, gate seq 1) | [`talking_npc::look_at_layout`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::talking_npc::{look_at_layout, LookLayout};
use crate::moby_update::creature as c;
use crate::moby_update::interact::{self, talk};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 11;
pub const MASK_FN: u32 = 0x2c_b668;
pub const MASK_CLASSES: [i16; 1] = [23];
pub const THRUSTER_FN: u32 = 0x2d_0710;
pub const THRUSTER_CLASSES: [i16; 1] = [90];
pub const PERSUADER_FN: u32 = 0x2f_0e40;
pub const PERSUADER_CLASSES: [i16; 1] = [298];

const THRUSTER_LOOK: LookLayout = LookLayout { pitch: (0x70, 0), yaw: (0xf0, 1), glance: 0x170, seen: 0x184, glance_timer: 0x188, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.5, yaw_b: 0.5, short_timers: false, gate_main: 0, gate_alt: 0xff, k_seen: 0.04 };
const PERSUADER_LOOK: LookLayout = LookLayout { pitch: (0x50, 0), yaw: (0xd0, 1), glance: 0x150, seen: 0x160, glance_timer: 0x164, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.5, yaw_b: 0.5, short_timers: false, gate_main: 1, gate_alt: 0xff, k_seen: 0.04 };

fn checkpoint_here(w: &mut World) {
    let (pos, yaw) = (crate::hero::physics::to_f32x3(w.hero.pos), w.hero.yaw().to_f32());
    crate::moby_update::classes::checkpoint::record(w, crate::moby_update::classes::checkpoint::Record { pos, rot: [0.0, 0.0, yaw] });
}

/// Level11 `0x2cb668` (23; module doc).
pub fn mask_update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            if w.inventory.owned(6) {
                w.delete_moby(id);
                return;
            }
            let m = w.mm(id);
            m.state = 1;
            m.mode |= mode::HIDDEN;
            m.position[2] += 0.75;
        }
        2 if c::dist2(c::pos(w, id), story::hero4(w)) < 2.0 => {
            story::set_owned(w, 6, 1);
            story::set_acquired(w, 6, 1);
            crate::cinematic::save(w);
            w.svc.help.request(11000, 0x3a);
            w.delete_moby(id);
        }
        _ => {}
    }
}

/// Level11 `0x2d0710` (90; module doc).
pub fn thruster_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x190);
    interact::poll_scene_end(w, id);
    // 0x2d0638: the actors' big-head cheat on the scene actors of [90] (list 0, 2.1).
    crate::moby_update::manip::scene_big_head(w, &[90], 0, 2.1);
    match w.m(id).state {
        0 => {
            w.mm(id).state = if w.inventory.owned(3) { 3 } else { 1 };
            interact::talk_register(w, id);
            p::set_ff(&mut w.mm(id).pvars, talk::RADIUS, f32::from_bits(0x406c_cccd));
        }
        1 => {
            if story::hero_in(w, p::i32(&w.m(id).pvars, 0x40)) && interact::talk_update(w, id) {
                interact::place_after_scene(w, id, f32::from_bits(0x402c_cccd));
                w.mm(id).state = 2;
            }
        }
        2 if w.svc.game_mode != 2 && w.svc.interact.talker != Some(id) => {
            w.mm(id).state = 1;
            if p::i16(&w.m(id).pvars, talk::LAST) == 2 {
                interact::give_item(w, 3, true);
                checkpoint_here(w);
                crate::cinematic::save(w);
                w.svc.help.kill();
                w.svc.help.request(0x2afb, 0x3d);
            }
        }
        _ => {}
    }
    look_at_layout(w, id, &THRUSTER_LOOK);
}

/// Level11 `0x2f0e40` (298; module doc).
pub fn persuader_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x170);
    interact::poll_scene_end(w, id);
    // 0x2f0d58: the actors' big-head cheat on the scene actors of [298] (list 0, 2.75).
    crate::moby_update::manip::scene_big_head(w, &[298], 0, 2.75);
    if w.m(id).visible != 0 && c::len3(c::sub(c::pos(w, id), w.camera.map(|x| f32::from_bits(x.0)))) < 30.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x18;
    }
    match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            if w.m(id).anim.seq_b != 1 {
                let n = w.ticks(0x14);
                w.anim_blend(id, 1, 0, n);
            }
            if w.svc.interact.game.acquired.get(35).is_some_and(|&b| b != 0) {
                w.delete_moby(id);
                return;
            }
            interact::talk_register(w, id);
            p::set_ff(&mut w.mm(id).pvars, talk::RADIUS, 4.0);
        }
        1 => {
            if interact::talk_update(w, id) {
                interact::place_after_scene(w, id, 3.0);
                w.mm(id).state = 2;
            }
            let (pos, h) = (c::pos(w, id), story::hero4(w));
            if story::flag(w, 1) == 0 && !w.inventory.owned(35) && c::len3(c::sub(pos, h)) < 4.0 {
                super::hints::remind(w, 0x86, 0x2b03, 0x86);
            }
            if !story::mission_done(w, w.m(id).mission as i32) && c::len3(c::sub(h, pos)) < 4.0 && (h[2] - pos[2]).abs() < 4.0 {
                let mission = w.m(id).mission;
                crate::cinematic::set_mission_done(w, mission);
                let cb = p::i32(&w.m(id).pvars, 0x48);
                story::checkpoint_at(w, cb);
            }
        }
        2 if w.svc.game_mode != 2 && w.svc.interact.talker != Some(id) => {
            w.mm(id).state = 1;
            if p::i16(&w.m(id).pvars, talk::LAST) == 2 {
                story::set_owned(w, 35, 1);
                story::set_acquired(w, 35, 1);
                story::set_flag(w, 1, 0);
                let t = w.ticks(300);
                crate::cinematic::show_banner(w, 0x2b06, t);
                crate::cinematic::save(w);
                w.svc.help.request(0x2af9, 0x3b);
            }
        }
        _ => {}
    }
    look_at_layout(w, id, &PERSUADER_LOOK);
}
