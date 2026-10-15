//! **Oltanis' story NPCs and pickup** (level 14; read from the level14 decomp):
//!
//! * **851 "Captain Qwark"** (`0x2fba20`, census U464, one instance): the fitness course. With his talk auto inside his
//!   cuboid +0x40 he sets his mission and the checkpoint at cuboid +0x48 and starts the course moby +0x44
//!   (`0x2ef578`: its +0xbc = 2 when its +0xac < 0, else 1; its +0x124 = 1); node 3 or 4 played gives the PDA (item 32,
//!   banner 21469 or 13995), stops the course moby for good (`0x2ef5a8`) and himself (the death bits), saves; gone.
//! * **924 "Scrap Merchant"** (`0x2fefe0`, U469, one instance): the same mission / checkpoint rule (cuboids +0x40 /
//!   +0x44); his talk puts Ratchet on cuboid +0x48 when it ends; node 4 unlocks Quartu (planet 15) with its banner,
//!   sets his death bits, saves; gone (also once planet 15 is unlocked).
//! * **1354 the Morph-o-Ray** (`0x305758`, U474, one instance): a turning, bobbing, glowing item; checked every tenth
//!   tick within 1 (XY) and 2 (z): the Morph-o-Ray (item 21) given, scene 6; after the scene (hidden while it runs) its
//!   mission and a save, banner 13994 (or 21472 when it was owned), help 14000; gone.
//!
//! **System or not.** Per-class code from the NPC template and the item glow (`gold_bolt`), the talk system, `story`.
//!
//! ## 851 coverage (level14 `0x2fba20`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x2fb938` (the big-head scene manipulator); drawn within 28: the shadow probe, +0x7f = 0x16 | the probe: [`qwark_update`]; the cheat: `manip::scene_big_head` |
//! | state 0 | collected byte (0x1bbd84[spawn]) or death bit (0x14c190) set, or +0x44 / +0x48 = −1 → `DeleteMoby`; else the name, `NpcTalkRegister`, auto = 1, → 1 | [`qwark_update`] |
//! | state 1 | auto and in cuboid +0x40: mission not done → `SetMissionDone(+0xb0)`, the checkpoint at cuboid +0x48; `0x2ef578(+0x44)`; radius 255; else radius 1.7; `NpcTalkUpdate` → `PlaceAfterScene(3)`, → 2 | [`qwark_update`] |
//! | state 2 | game mode ≠ 2: → 1; node played 3 / 4: `ShowBanner(13995 / 21469, −1)`, `GiveItem(32, 1)`, `0x2ef5a8(+0x44)` (its +0xbc = 0, +0x124 = 0, its death bits), his death bits (0x14c190 / 0x1babd0), `memcard_Save`, `DeleteMoby` | [`qwark_update`] (`story::death_bits`) |
//! | state 3 | `DeleteMoby` | [`qwark_update`] |
//! | tail | the look-at (records +0x80 / +0x100, glance +0x180, s32 +0x194 / +0x198, pitch × 1, yaw 0.6 / 0.4) | [`talking_npc::look_at_layout`] |
//!
//! ## 924 coverage (level14 `0x2fefe0`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x2feef8` (the big-head scene manipulator) | `manip::scene_big_head` |
//! | state 0 | planet 15 unlocked (0x13dd4f), collected / death bit, or +0x44 / +0x48 = −1 → `DeleteMoby`; the name, `NpcTalkRegister`, → 1, auto = 1 | [`merchant_update`] |
//! | state 1 | auto in cuboid +0x40: mission not done → `SetMissionDone`, the checkpoint at cuboid +0x44; radius 255; else 3; `NpcTalkUpdate` → the scene-end place = cuboid +0x48's centre, yaw its Euler z (0x16cfa6 = 1), → 2 | [`merchant_update`] |
//! | state 2 | game mode ≠ 2: → 1; node played 4: `UnlockPlanet(15)`, `ShowPlanetBanner(15)`, his death bits, `memcard_Save`, `DeleteMoby` | [`merchant_update`] |
//! | tail | the look-at (records +0x50 / +0xd0, glance +0x150, s32 +0x160 / +0x164, yaw 0.6 / 0.4) | [`talking_npc::look_at_layout`] |
//!
//! ## 1354 coverage (level14 `0x305758`)
//!
//! | address | what | status |
//! |---|---|---|
//! | state 0 | mission not done: the glow init (`0x305a38`, base +0x20), z = `GroundHeight(0.5)` + 0.8 (gp−0x4a18), rot x 0, rot y −π/2, home P[0] = position, → 1; done: `DeleteMoby` | [`morph_update`] |
//! | state 1 | yaw += 0.02618 (gp−0x4a14); position = home + row 1 · 0.4 (gp−0x4a10); the glow (`0x305b18`: home + 0.1 z); every 10th tick within 1 (XY), \|Δz\| < 2: P+0x18 = owned (0x13d4d5); `GiveItem(21, 1)`; `DialogStreamStart(6)`; → 2 | [`morph_update`] |
//! | state 2 | game mode 2: mode \|= 0x41; else mission not done → `SetMissionDone`, `memcard_Save`; `ShowBanner(13994 / 21472 when owned, −1)`, `Help_Request(14000, 0x7a)`, `DeleteMoby` | [`morph_update`] |
//! | state 3 | scale → 0 by 0.02 × the class scale per tick (`Approach`), then `DeleteMoby` | [`morph_update`] (no code sets it) |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::gold_bolt;
use crate::moby_update::classes::talking_npc::{look_at_layout, LookLayout};
use crate::moby_update::creature::{self as c, turn};
use crate::moby_update::interact::{self, talk};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 14;
pub const QWARK_FN: u32 = 0x2f_ba20;
pub const QWARK_CLASSES: [i16; 1] = [851];
pub const MERCHANT_FN: u32 = 0x2f_efe0;
pub const MERCHANT_CLASSES: [i16; 1] = [924];
pub const MORPH_FN: u32 = 0x30_5758;
pub const MORPH_CLASSES: [i16; 1] = [1354];

const QWARK_LOOK: LookLayout = LookLayout { pitch: (0x80, 0), yaw: (0x100, 1), glance: 0x180, seen: 0x194, glance_timer: 0x198, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.6, yaw_b: 0.4, short_timers: false, gate_main: 0, gate_alt: 0xff, k_seen: 0.04 };
const MERCHANT_LOOK: LookLayout = LookLayout { pitch: (0x50, 0), yaw: (0xd0, 1), glance: 0x150, seen: 0x160, glance_timer: 0x164, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.6, yaw_b: 0.4, short_timers: false, gate_main: 0, gate_alt: 0xff, k_seen: 0.04 };

/// The NPC already gone for good: its collected byte or its death bit, or a missing link / cuboid.
fn gone_for_good(w: &World, id: MobyId) -> bool {
    let sid = w.m(id).spawn_id;
    w.svc.save.collected.get(&sid).is_some_and(|&b| b != 0) || w.svc.save.death.contains(&(w.svc.level, sid))
}

/// The mission and checkpoint rule of 851 / 924: auto and in cuboid `cb`: the mission (when open) and the checkpoint at
/// cuboid `chk`; returns whether the rule held (radius 255).
fn mission_rule(w: &mut World, id: MobyId, cb: usize, chk: usize) -> bool {
    if p::u8(&w.m(id).pvars, talk::AUTO) != 1 || !story::hero_in(w, p::i32(&w.m(id).pvars, cb)) { return false; }
    if !story::mission_done(w, w.m(id).mission as i32) {
        let mission = w.m(id).mission;
        crate::cinematic::set_mission_done(w, mission);
        let k = p::i32(&w.m(id).pvars, chk);
        story::checkpoint_at(w, k);
    }
    true
}

/// Level14 `0x2fba20` (851; module doc).
pub fn qwark_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x1a0);
    interact::poll_scene_end(w, id);
    // 0x2fb938: the actors' big-head cheat on the scene actors of [851] (list 0, 2.75).
    crate::moby_update::manip::scene_big_head(w, &[851], 0, 2.75);
    if w.m(id).visible != 0 && c::len3(c::sub(c::pos(w, id), w.camera.map(|x| f32::from_bits(x.0)))) < 28.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x16;
    }
    match w.m(id).state {
        0 => {
            if gone_for_good(w, id) || p::i32(&w.m(id).pvars, 0x44) == -1 || p::i32(&w.m(id).pvars, 0x48) == -1 {
                w.delete_moby(id);
                return;
            }
            interact::talk_register(w, id);
            p::set_u8(&mut w.mm(id).pvars, talk::AUTO, 1);
            w.mm(id).state = 1;
        }
        1 => {
            if mission_rule(w, id, 0x40, 0x48) {
                if let Some(m) = story::link(w, p::i32(&w.m(id).pvars, 0x44)) {
                    story::pvars(w, m, 0x128);
                    let v = if p::ff(&w.m(m).pvars, 0xac) < 0.0 { 2 } else { 1 };
                    w.mm(m).cmd = v;
                    p::set_i16(&mut w.mm(m).pvars, 0x124, 1);
                }
                p::set_ff(&mut w.mm(id).pvars, talk::RADIUS, 255.0);
            } else {
                p::set_ff(&mut w.mm(id).pvars, talk::RADIUS, f32::from_bits(0x3fd9_999a));
            }
            if interact::talk_update(w, id) {
                interact::place_after_scene(w, id, 3.0);
                w.mm(id).state = 2;
            }
        }
        2 => {
            if w.svc.game_mode != 2 && w.svc.interact.talker != Some(id) {
                w.mm(id).state = 1;
                let last = p::i16(&w.m(id).pvars, talk::LAST);
                if last == 3 || last == 4 {
                    let t = w.ticks(180);
                    crate::cinematic::show_banner(w, if last == 4 { 0x53dd } else { 0x36b3 }, t);
                    interact::give_item(w, 32, true);
                    if let Some(m) = story::link(w, p::i32(&w.m(id).pvars, 0x44)) {
                        story::pvars(w, m, 0x128);
                        w.mm(m).cmd = 0;
                        p::set_i16(&mut w.mm(m).pvars, 0x124, 0);
                        story::death_bits(w, m);
                    }
                    story::death_bits(w, id);
                    crate::cinematic::save(w);
                    w.delete_moby(id);
                    return;
                }
            }
        }
        3 => {
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    look_at_layout(w, id, &QWARK_LOOK);
}

/// Level14 `0x2fefe0` (924; module doc).
pub fn merchant_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x170);
    interact::poll_scene_end(w, id);
    // 0x2feef8: the actors' big-head cheat on the scene actors of [924] (list 0, 2.75).
    crate::moby_update::manip::scene_big_head(w, &[924], 0, 2.75);
    match w.m(id).state {
        0 => {
            if story::planet_unlocked(w, 15) || gone_for_good(w, id) || p::i32(&w.m(id).pvars, 0x44) == -1 || p::i32(&w.m(id).pvars, 0x48) == -1 {
                w.delete_moby(id);
                return;
            }
            interact::talk_register(w, id);
            w.mm(id).state = 1;
            p::set_u8(&mut w.mm(id).pvars, talk::AUTO, 1);
        }
        1 => {
            let r = if mission_rule(w, id, 0x40, 0x44) { 255.0 } else { 3.0 };
            p::set_ff(&mut w.mm(id).pvars, talk::RADIUS, r);
            if interact::talk_update(w, id) {
                if let Some((centre, rot)) = story::cuboid(w, p::i32(&w.m(id).pvars, 0x48)) {
                    w.svc.interact.scene_end_place = Some((centre, rot[2]));
                }
                w.mm(id).state = 2;
            }
        }
        2 => {
            if w.svc.game_mode != 2 && w.svc.interact.talker != Some(id) {
                w.mm(id).state = 1;
                if p::i16(&w.m(id).pvars, talk::LAST) == 4 {
                    crate::cinematic::unlock_planet(w, 15);
                    crate::cinematic::show_planet_banner(w, 15);
                    story::death_bits(w, id);
                    crate::cinematic::save(w);
                    w.delete_moby(id);
                    return;
                }
            }
        }
        3 => {
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    look_at_layout(w, id, &MERCHANT_LOOK);
}

/// gp−0x4a18 / −0x4a14 / −0x4a10: the Morph-o-Ray's height over the ground, its turn rate, its bob.
const MORPH_HEIGHT: f32 = 0.8;
const MORPH_TURN: f32 = 0.026_179_94;
const MORPH_BOB: f32 = 0.4;

/// Level14 `0x305758` (1354; module doc).
pub fn morph_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x60);
    match w.m(id).state {
        0 => {
            if story::mission_done(w, w.m(id).mission as i32) {
                w.delete_moby(id);
                return;
            }
            gold_bolt::glow_init(w, id, 0x20);
            let g = w.ground_height(Pf::f(0.5), w.m(id).position.map(Pf::f), 0).to_f32();
            let m = w.mm(id);
            m.rotation[0] = 0.0;
            m.rotation[1] = -std::f32::consts::FRAC_PI_2;
            m.position[2] = g + MORPH_HEIGHT;
            let home = m.position;
            m.state = 1;
            c::set_pv4(w, id, 0, home);
        }
        1 => {
            let y = c::add_rot(c::yaw(w, id), MORPH_TURN);
            c::set_yaw(w, id, y);
            let home = c::pv4(w, id, 0);
            let r1 = w.m(id).rows[1];
            w.mm(id).position = [home[0] + r1[0] * MORPH_BOB, home[1] + r1[1] * MORPH_BOB, home[2] + r1[2] * MORPH_BOB, home[3] + r1[3] * MORPH_BOB];
            gold_bolt::item_glow(w, id, 0x20, [home[0], home[1], home[2] + 0.1], 1.0, true);
            if !w.counter.is_multiple_of(10) { return; }
            let (pos, h) = (w.m(id).position, story::hero4(w));
            if 1.0 <= c::dist2(pos, h) || 2.0 <= (pos[2] - h[2]).abs() { return; }
            let had = w.inventory.owned(21) as i32;
            p::set_i32(&mut w.mm(id).pvars, 0x18, had);
            interact::give_item(w, 21, true);
            crate::cinematic::start_scene(w, 6, false);
            w.mm(id).state = 2;
        }
        2 => {
            if w.svc.game_mode == 2 {
                w.mm(id).mode |= mode::HIDDEN | mode::NO_ANIM;
                return;
            }
            if !story::mission_done(w, w.m(id).mission as i32) {
                let mission = w.m(id).mission;
                crate::cinematic::set_mission_done(w, mission);
                crate::cinematic::save(w);
            }
            let t = w.ticks(180);
            let msg = if p::i32(&w.m(id).pvars, 0x18) == 0 { 0x36b2 } else { 0x53e0 };
            crate::cinematic::show_banner(w, msg, t);
            w.svc.help.request(14000, 0x7a);
            w.delete_moby(id);
        }
        3 => {
            let k = w.classes.info(w.m(id).o_class).map_or(1.0, |i| i.scale);
            let mut s = w.m(id).scale;
            turn::approach(0.0, k * 0.02, &mut s);
            w.mm(id).scale = s;
            if s == 0.0 { w.delete_moby(id); }
        }
        _ => {}
    }
}
