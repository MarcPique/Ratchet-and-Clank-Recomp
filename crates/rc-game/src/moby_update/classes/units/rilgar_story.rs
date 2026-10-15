//! **Rilgar's story NPCs** (level 05; read from the level05 decomp and disassembly):
//!
//! * **918 "Race Girl"** (`0x316ab8`, census U200, one instance): the hoverboard race's host. Her talk's node 3
//!   sends Ratchet to the start (A = (277.4, 337, 73), yaw −2.1) and stores the finish (B = (294.51, 223.43, 100.08),
//!   Euler (0, 0.11, 1.55)) as the pose the mode freeze returns to (0x141050 / 0x141060), with music track (2, 6).
//!   The race's classes command her through +0xbc: 1 the finish (back to B, music (0, 8); the first win jumps her talk
//!   to node 4, auto), 2 a lost race (back to B), 3 a restart (to A again). Node 4 played: global flag 0 (the race won),
//!   banner 5013 for `ticks(300)`, a save. She grows infatuated when Ratchet shows off in front of her (a counter
//!   that swells two of her joints).
//! * **919 "Bouncer"** (`0x317470`, U201, one instance): his talk's node 5 unlocks Umbris (planet 7) and saves; gone
//!   once planet 7 is unlocked.
//! * **925 "Shifty Salesman"** (`0x3180a0`, U203, one instance): his talk's nodes 3 / 4 sell the R.Y.N.O. (item 23,
//!   banner 5014), save; gone once it is acquired.
//!
//! **System or not.** Per-class code from the NPC template (`story_npc`, `talking_npc::look_at_layout` rows), the
//! talk system, `story`, cinematic.
//!
//! ## 918 coverage (level05 `0x316ab8`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x316990`: in a scene, the big-head cheat's manipulator, and with the counter gp−0x4d30 ≠ 0 the swelling `0x316810` on her scene actors | [`race_girl_update`] (`manip::scene_big_head`, [`swell`] on the actors) |
//! | state 0 | seq B ≠ 0 → blend to 0 over `ticks(20)`; z = `GroundHeight(0.5)`; talk slots 16 or 17 not talked to (0x13d6bc / 0x13d6cc) → her talk table's node 0 scene := 13 (0x1b1af8[slot]+4); the name, → 1, `NpcTalkRegister`, the shadow slab | [`race_girl_update`] |
//! | state 1 | `NpcTalkUpdate` → `PlaceAfterScene(2)`, → 3; talk node 3 → last = 3, node 2, since = tick, +0xbc = 0, `HeroTeleport(A, 0, 1)`, 0x141050 / 0x141060 = B, `MusicRequestTrack(2, 6)` | [`race_girl_update`] (`HeroFields::set_entry_pose`) |
//! | | +0xbc = 1: `HeroTeleport(B, 0, 1)`, `MusicRequestTrack(0, 8)`; flag 0 clear → last = node, node 4, since, auto 1; +0xbc = 0 | [`race_girl_update`] |
//! | | +0xbc = 2: `MusicRequestTrack(0, 8)`, `HeroTeleport(B, 0, 1)`, +0xbc = 0; +0xbc = 3: +0xbc = 0, `HeroTeleport(A, 0, 1)`, the pose = B | [`race_girl_update`] |
//! | | the idle: seq 0 / 2 with the s32 timer +0x244 out → `ticks(randf(1200, 2400))`, seq 1; at a sequence's end: `randi(2) = 0` → 2, else 0 | `story_npc::idle_anim` |
//! | state 3 | game mode ≠ 2: → 1; node 4 played: global flag 0 = 1, `ShowBanner(5013, ticks(300))`, `memcard_Save` | [`race_girl_update`] |
//! | states 2, ≥ 4 | +0xbc ≠ 0: `HeroTeleport(B, 0, 1)`, +0xbc = 0 | [`race_girl_update`] |
//! | tail | the look-at targets (records +0x40 list 0 / +0xc0 list 1, glance +0x250, s32 timers +0x248 / +0x24c, eye 1, pitch × 1, yaw 0.6 / 0.4, under seq 0 or 2, k 0.03 when seen) | [`talking_npc::look_targets`] |
//! | tail | within 15 (XY) and her facing within 70° of Ratchet: in state 0xb with 0x13f4e8 = 0xf the counter +0x260 + 1 (≤ 20), in state 4 past `ticks(20)` every 10th tick − 1 (≥ 0); gp−0x4d30 = the counter | [`race_girl_update`] (`Services::level_words` 0x161ed0) |
//! | tail | outside game mode 2: `0x316810` (records +0x140 / +0x1c0 on lists 2 / 3: with the counter, scale `min(1 + 0.15·n, 3.7)` and target y `sin((tick & 0x7f)/128·π)·35°`; springs 0.05 / 0.3), then the look springs; the big-head cheat | [`race_girl_update`]; the cheat: `manip::scene_big_head` |
//!
//! ## 919 coverage (level05 `0x317470`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x317398` (the big-head scene manipulator) | `manip::scene_big_head` |
//! | state 0 | planet 7 unlocked (0x13dd47) → `DeleteMoby`; the name, → 1, `NpcTalkRegister`, radius 3.7, the shadow slab | [`bouncer_update`] |
//! | state 1 | `NpcTalkUpdate` → `PlaceAfterScene(2.5)`; node played 5 → `UnlockPlanet(7)`, `memcard_Save`, `DeleteMoby`; the idle (s32 timer +0x144: `randi(2) ≠ 0` → 1 else 2; back to 0 at the end) | [`bouncer_update`] |
//! | tail | the look-at (records +0x40 / +0xc0, glance +0x150, s32 +0x148 / +0x14c, eye 1, pitch × 1, yaw 0.5 / 0.5); the big-head cheat | [`talking_npc::look_at_layout`] (the cheat in `look_springs`) |
//!
//! ## 925 coverage (level05 `0x3180a0`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x317fb8` (the big-head scene manipulator); drawn within 40 of the camera: the shadow probe, +0x7f = 0x20 | the probe: [`salesman_update`]; the cheat: `manip::scene_big_head` |
//! | state 0 | R.Y.N.O. acquired (0x13d4ff) → `DeleteMoby`; z += 0.5, z = `GroundHeight(0.5)`; the name, → 1, `NpcTalkRegister` | [`salesman_update`] |
//! | state 1 | `NpcTalkUpdate` → `PlaceAfterScene(2)`; node played 3 or 4 → `GiveItem(23, 1)`, `ShowBanner(5014, −1)`, `memcard_Save`, `DeleteMoby`; the idle (s32 timer +0x154 → seq 1; seq 1 at its end → 0) | [`salesman_update`] |
//! | tail | the look-at (records +0x40 / +0xc0, glance +0x140, s32 +0x158 / +0x15c, eye 1, pitch × 1, yaw 0.6 / 0.4); the big-head cheat | [`talking_npc::look_at_layout`] (the cheat in `look_springs`) |

use super::story_npc::{self, Idle};
use crate::moby_update::classes::talking_npc::{look_at_layout, look_springs, look_targets, LookLayout};
use crate::moby_update::creature as c;
use crate::moby_update::interact::{self, talk};
use crate::moby_update::manip;
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;
use crate::moby_runtime::MobyId;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 5;
pub const GIRL_FN: u32 = 0x31_6ab8;
pub const GIRL_CLASSES: [i16; 1] = [918];
pub const BOUNCER_FN: u32 = 0x31_7470;
pub const BOUNCER_CLASSES: [i16; 1] = [919];
pub const SALESMAN_FN: u32 = 0x31_80a0;
pub const SALESMAN_CLASSES: [i16; 1] = [925];

/// 0x215bc0 / 0x215bd0: the race's start (position, Euler).
const START: ([f32; 3], [f32; 3]) = ([277.4, 337.0, 73.0], [0.0, 0.0, -2.1]);
/// 0x215be0 / 0x215bf0: the race's finish.
const FINISH: ([f32; 3], [f32; 3]) = ([294.51, 223.43, 100.08], [0.0, 0.11, 1.55]);
/// gp−0x4d30 (0x161ed0): the race girl's infatuation counter as the scene-actor code reads it.
pub const SWOON_WORD: u32 = 0x16_1ed0;

const GIRL_LOOK: LookLayout = LookLayout { pitch: (0x40, 0), yaw: (0xc0, 1), glance: 0x250, seen: 0x248, glance_timer: 0x24c, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.6, yaw_b: 0.4, short_timers: false, gate_main: 0, gate_alt: 2, k_seen: 0.03 };
const GIRL_IDLE: Idle = Idle { timer: 0x244, short: false, rest: |s| s == 0 || s == 2, pick: |_| 1, at_end: |w, _| Some(if w.rng.randi(2) == 0 { 2 } else { 0 }) };
const BOUNCER_LOOK: LookLayout = LookLayout { pitch: (0x40, 0), yaw: (0xc0, 1), glance: 0x150, seen: 0x148, glance_timer: 0x14c, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.5, yaw_b: 0.5, short_timers: false, gate_main: 0, gate_alt: 0xff, k_seen: 0.04 };
const BOUNCER_IDLE: Idle = Idle { timer: 0x144, short: false, rest: story_npc::rest0, pick: |w| if w.rng.randi(2) != 0 { 1 } else { 2 }, at_end: story_npc::back0 };
const SALESMAN_LOOK: LookLayout = LookLayout { pitch: (0x40, 0), yaw: (0xc0, 1), glance: 0x140, seen: 0x158, glance_timer: 0x15c, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.6, yaw_b: 0.4, short_timers: false, gate_main: 0, gate_alt: 0xff, k_seen: 0.04 };
const SALESMAN_IDLE: Idle = Idle { timer: 0x154, short: false, rest: story_npc::rest0, pick: |_| 1, at_end: |_, s| (s == 1).then_some(0) };

fn to(w: &mut World, at: ([f32; 3], [f32; 3])) { crate::cinematic::hero_teleport(w, at.0, at.1, 0, true); }

fn music(w: &mut World, track: i16, stinger: i16) { if let Some(s) = w.sound.as_deref_mut() { s.music_request(track, stinger); } }

/// The talk block's jump to `node` (last = the current node, since = this tick).
fn jump(w: &mut World, id: MobyId, node: i16) {
    let t = w.counter as u32;
    let pv = &mut w.mm(id).pvars;
    let cur = p::i16(pv, talk::NODE);
    p::set_i16(pv, talk::LAST, cur);
    p::set_u32(pv, talk::SINCE, t);
    p::set_i16(pv, talk::NODE, node);
}

/// Level05 `0x316ab8` (918; module doc).
pub fn race_girl_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x264);
    interact::poll_scene_end(w, id);
    // 0x316990: in a scene, the actors' big-head cheat on her scene actors (list 0, 2.75), and with the swoon counter
    // gp−0x4d30 ≠ 0 the swelling `0x316810` on them (her records, their joints).
    crate::moby_update::manip::scene_big_head(w, &GIRL_CLASSES, 0, 2.75);
    if w.svc.game_mode == 2 && w.svc.level_words.get(&SWOON_WORD).copied().unwrap_or(0) != 0 {
        let actors: Vec<MobyId> = w.svc.cinematic.scene.as_ref().map(|s| s.actors.iter().filter(|a| a.o_class == GIRL_CLASSES[0]).filter_map(|a| a.moby).collect()).unwrap_or_default();
        for a in actors { swell(w, id, a); }
    }
    match w.m(id).state {
        0 => {
            if w.m(id).anim.seq_b != 0 {
                let n = w.ticks(0x14);
                w.anim_blend(id, 0, 0, n);
            }
            let g = w.ground_height(Pf::f(0.5), w.m(id).position.map(Pf::f), 0).to_f32();
            w.mm(id).position[2] = g;
            let talked = |k: usize| w.svc.interact.game.talked.get(k).is_some_and(|&t| t != 0);
            if !talked(16) || !talked(17) {
                let slot = interact::talk_slot(w, id);
                if let Ok(s) = usize::try_from(slot) {
                    if let Some(n) = std::sync::Arc::make_mut(&mut w.svc.interact.tables).tables.get_mut(s).and_then(|t| t.first_mut()) { n.scene = 0xd; }
                }
            }
            w.mm(id).state = 1;
            interact::talk_register(w, id);
            story::shadow_slab(w, id);
        }
        1 => {
            if interact::talk_update(w, id) {
                interact::place_after_scene(w, id, 2.0);
                w.mm(id).state = 3;
            }
            if p::i16(&w.m(id).pvars, talk::NODE) == 3 {
                jump(w, id, 2);
                w.mm(id).cmd = 0;
                to(w, START);
                w.hero_fields_mut().set_entry_pose = Some(FINISH);
                music(w, 2, 6);
            }
            if w.m(id).cmd == 1 {
                to(w, FINISH);
                music(w, 0, 8);
                if story::flag(w, 0) == 0 {
                    jump(w, id, 4);
                    p::set_u8(&mut w.mm(id).pvars, talk::AUTO, 1);
                }
                w.mm(id).cmd = 0;
            }
            if w.m(id).cmd == 2 {
                music(w, 0, 8);
                to(w, FINISH);
                w.mm(id).cmd = 0;
            }
            if w.m(id).cmd == 3 {
                w.mm(id).cmd = 0;
                to(w, START);
                w.hero_fields_mut().set_entry_pose = Some(FINISH);
            }
            story_npc::idle_anim(w, id, &GIRL_IDLE);
        }
        3 => {
            if w.svc.game_mode != 2 && w.svc.interact.talker != Some(id) {
                w.mm(id).state = 1;
                if p::i16(&w.m(id).pvars, talk::LAST) == 4 {
                    story::set_flag(w, 0, 1);
                    let t = w.ticks(300);
                    crate::cinematic::show_banner(w, 0x1395, t);
                    crate::cinematic::save(w);
                }
            }
        }
        _ => {
            if w.m(id).cmd != 0 {
                to(w, FINISH);
                w.mm(id).cmd = 0;
            }
        }
    }
    let (k, d) = look_targets(w, id, &GIRL_LOOK);
    // The swoon counter (module doc).
    let (pos, h) = (c::pos(w, id), story::hero4(w));
    if c::dist2(pos, h) < 15.0 && c::diff_rots(c::yaw(w, id), c::atan(h[0] - pos[0], h[1] - pos[1])) < 1.221_730_5 {
        if w.hero.state == 0xb && w.hero.timer == 0xf {
            let n = (p::i32(&w.m(id).pvars, 0x260) + 1).min(20);
            p::set_i32(&mut w.mm(id).pvars, 0x260, n);
            w.svc.level_words.insert(SWOON_WORD, n as u32);
        }
        if w.hero.state == 4 && w.ticks(0x14) < w.hero.timer && w.counter.is_multiple_of(10) {
            let n = (p::i32(&w.m(id).pvars, 0x260) - 1).max(0);
            p::set_i32(&mut w.mm(id).pvars, 0x260, n);
            w.svc.level_words.insert(SWOON_WORD, n as u32);
        }
    }
    if w.svc.game_mode != 2 {
        swell(w, id, id);
        look_springs(w, id, &GIRL_LOOK, k, d);
    }
}

/// `0x316810(m, pvars)`: her records +0x140 / +0x1c0 on moby `target`'s joint lists 2 / 3 (herself, or one of her scene
/// actors), swollen by the counter gp−0x4d30.
fn swell(w: &mut World, id: MobyId, target: MobyId) {
    let n = w.svc.level_words.get(&SWOON_WORD).copied().unwrap_or(0) as i32;
    for i in 0..2usize {
        let rec = 0x140 + 0x80 * i;
        story::pvars(w, id, rec + manip::rec::SIZE);
        if n != 0 {
            let s = (n as f32 * 0.15 + 1.0).min(3.7);
            c::set_pf(w, id, rec + manip::rec::REC_SCALE, s);
            let a = (((w.counter as u32 & 0x7f) as f32) * 0.007_812_5 * std::f32::consts::PI).sin();
            c::set_pf(w, id, rec + manip::rec::TARGET + 4, a * 0.610_865_24);
        }
        manip::look(w, target, id, rec, (i + 2) as u8, 0.05, 0.3);
    }
}

/// Level05 `0x317470` (919; module doc).
pub fn bouncer_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x160);
    interact::poll_scene_end(w, id);
    // 0x317398: the actors' big-head cheat on the scene actors of [919] (list 0, 2.3).
    crate::moby_update::manip::scene_big_head(w, &[919], 0, 2.3);
    match w.m(id).state {
        0 => {
            if story::planet_unlocked(w, 7) {
                w.delete_moby(id);
                return;
            }
            w.mm(id).state = 1;
            interact::talk_register(w, id);
            p::set_ff(&mut w.mm(id).pvars, talk::RADIUS, f32::from_bits(0x406c_cccd));
            story::shadow_slab(w, id);
        }
        1 => {
            if interact::talk_update(w, id) { interact::place_after_scene(w, id, 2.5); }
            if p::i16(&w.m(id).pvars, talk::LAST) == 5 {
                crate::cinematic::unlock_planet(w, 7);
                crate::cinematic::save(w);
                w.delete_moby(id);
                return;
            }
            story_npc::idle_anim(w, id, &BOUNCER_IDLE);
        }
        _ => {}
    }
    look_at_layout(w, id, &BOUNCER_LOOK);
}

/// Level05 `0x3180a0` (925; module doc).
pub fn salesman_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x160);
    interact::poll_scene_end(w, id);
    // 0x317fb8: the actors' big-head cheat on the scene actors of [925] (list 0, 2.75).
    crate::moby_update::manip::scene_big_head(w, &[925], 0, 2.75);
    if w.m(id).visible != 0 && c::len3(c::sub(c::pos(w, id), w.camera.map(|x| f32::from_bits(x.0)))) < 40.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x20;
    }
    match w.m(id).state {
        0 => {
            if w.svc.interact.game.acquired.get(23).is_some_and(|&b| b != 0) {
                w.delete_moby(id);
                return;
            }
            w.mm(id).position[2] += 0.5;
            let g = w.ground_height(Pf::f(0.5), w.m(id).position.map(Pf::f), 0).to_f32();
            w.mm(id).position[2] = g;
            w.mm(id).state = 1;
            interact::talk_register(w, id);
        }
        1 => {
            if interact::talk_update(w, id) { interact::place_after_scene(w, id, 2.0); }
            let last = p::i16(&w.m(id).pvars, talk::LAST);
            if last == 3 || last == 4 {
                interact::give_item(w, 23, true);
                let t = w.ticks(180);
                crate::cinematic::show_banner(w, 0x1396, t);
                crate::cinematic::save(w);
                w.delete_moby(id);
                return;
            }
            story_npc::idle_anim(w, id, &SALESMAN_IDLE);
        }
        _ => {}
    }
    look_at_layout(w, id, &SALESMAN_LOOK);
}
