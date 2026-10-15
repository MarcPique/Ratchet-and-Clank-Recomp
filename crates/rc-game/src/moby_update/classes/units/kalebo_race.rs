//! **Kalebo III's hoverboard-race host, class 1455** (level16 `0x2e6808`, census U538, one instance): the twin of Rilgar's
//! race girl 918 (`rilgar_story`; her talk update and teleport are the same code, level05 `0x2903a8` / `0x244090` =
//! level16 `0x261888` / `0x213dc0`). Her talk's node 3 starts the race: Ratchet is teleported to the start, the finish
//! becomes his entry pose (0x141050 / 0x141060), the race's moby groups are switched on and the others off
//! (`0x2e70c0`), the death height drops to 74, the sea (1111) is told to sink to 76 and the race music plays; the race
//! (Ratchet's group-0x16 code, **G-LVL-007**) reports through her +0xbc: 1 won (back to the finish; the Hologuise not
//! yet acquired → her talk jumps to node 4), 2 lost, 3 a restart; Ratchet back in group 0 ends it (`0x2e7168`). Node 4
//! played → item movie 0 → the Hologuise (item 31) and a save. A race score past 4499 earns skill point 0x13d422.
//! Read from the level16 decomp. Native `f32`.
//!
//! **System or not.** Per-class code on the talking-NPC machinery (`interact::{talk_register_at, talk_update_at,
//! poll_scene_end_at, place_after_scene, give_item}`, the talk block at +0x20), the scene big-head cheat
//! (`manip::scene_big_head`), the look-at manipulators (`manip::look`), the story layer (`cinematic::{hero_teleport,
//! item_movie, save}`, `story::award_skill_point`), the moby groups ([`story::group_set`] = `0x254a30`, the loader's group
//! lists `Services::groups`).
//!
//! **Pvar block** (0x1e0): +0x20 the talk block (+0x24 last node, +0x28 auto, +0x2c radius 3.7, +0x56 node, +0x58 since),
//! +0x60 s32[16] the race's groups, +0xa0 s32[8] the groups off during the race, +0xc4 her yaw, +0xc8 the look-at-
//! Ratchet timer, +0xcc the glance timer, +0xd0 the glance point, +0xe0 / +0x160 the head / chest look-at records.
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | top | drawn and within 28 of the camera: the shadow probe, +0x7f = 0x16 | [`update`] (`shadows::probe_down`) |
//! | top | `0x2e6720`: in a scene with the actors' cheat (0x15edb0), the big head (2.75) on her actors | [`update`] (`manip::scene_big_head`) |
//! | state 0 | → 1; update distance 0xff; `NpcTalkRegister(+0x20)`; radius 3.7; +0xc4 = yaw; landmark word 0x13dc1c = 0 → yaw += π; the race off | [`update`] (0x13dc1c is chunk 15's landmark 102 flags word, not kept by the port: read as 0 [L]) |
//! | state 1 | death height 0x15f638 = 100 (115 when Ratchet's group ≠ 0xf) | [`update`] (`HeroFields::death_z`) |
//! | state 1 | `NpcTalkUpdate` → yaw = +0xc4, `PlaceAfterScene(3.5)`, → 2 | [`update`] |
//! | state 1 | Hoverboard (item 30) not owned, Ratchet within 3.5 (xy): help record 0x87's time ×600 + `ticks(18)`·60 < `ticks(play time)` or never shown → `Help_Request(16003, 0x87)`; else the record's time = `ticks(play time)`/600 when that is larger | [`update`] |
//! | state 1 | node 3: last = 3, node 2, since = tick, +0xbc = 0, `HeroTeleport(start, 0, 1)`, entry pose = the finish, → 3, the race on | [`update`], [`race_on`] |
//! | state 1 | Ratchet's group 0x16 (racing): → 3, the race on | [`update`] |
//! | state 2 | game mode ≠ 2: → 1; last node 4: the item movie 0 (`0x2923d0`), → 4 | [`update`] (`cinematic::item_movie`) |
//! | state 3+ | the race score 0x13fbf8 > 4499: skill point 0x13d422 (`PlayLevelSoundAtMoby(1)`, banner 0x53d6); best 0x15ee6c < 4500 → the score | [`update`] (the score and the best are the race's words: read / written as unit words until G-LVL-007) |
//! | state 3+ | Ratchet's group 0 → 1, the race off | [`update`] |
//! | state 3+ | +0xbc = 1: → 1, `HeroTeleport(finish)`, +0xbc = 0; the Hologuise not acquired (0x13d507): auto, last = node, node 4, since = tick; the race off | [`update`] |
//! | state 3+ | +0xbc = 2: → 1, `HeroTeleport(finish)`, +0xbc = 0, the race off; +0xbc = 3: +0xbc = 0, `HeroTeleport(start)`, entry pose = the finish | [`update`] |
//! | state 4 | game mode 0: → 1, `GiveItem(31, 1)` (the Hologuise), `memcard_Save` | [`update`] |
//! | tail | seq B = 0: look at Ratchet's body point within 8 in front (±90°) for `ticks(120)` after he moved (0x13f450 > 0.01), else at a glance point (every `trunc(scale(randf(180, 300)))`: 6 ahead at yaw + `randf(±90°)`, `randf(0, 30°)` up); targets: pitch −atan(\|d.xy\|, d.z) clamped ±30°, yaw (clamped ±90°) ·0.7 / ·0.3 on the two records | [`look`] |
//! | tail | the actors' cheat: the head record's scale 2.75; `0x25ddd0(k·0.02 (0.04 on Ratchet), 0.3, self, +0xe0, 0)` and `(…, +0x160, 1)` | [`look`] (`manip::look`) |
//! | 0x2e70c0 | the race groups +0x60 on (update, drawn, collision), +0xa0 off; death height 74; the sea word gp−0x4f8c = 76; `MusicRequestTrack(2, 6)` | [`race_on`] (the sea class 1111 (`Sea(6)`) does not read the word yet: noted in G-CLS-032) |
//! | 0x2e7168 | +0x60 off; +0xa0 on, drawn, collision off; the sea word = 100; `MusicRequestTrack(0, 8)` | [`race_off`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::interact::{self, talk};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2e_6808;
pub const CLASSES: [i16; 1] = [1455];

/// The talk block's base.
const TB: usize = 0x20;
const LEN: usize = 0x1e0;

mod pvo {
    pub const GROUPS_ON: usize = 0x60;
    pub const GROUPS_OFF: usize = 0xa0;
    pub const YAW: usize = 0xc4;
    pub const LOOK_T: usize = 0xc8;
    pub const GLANCE_T: usize = 0xcc;
    pub const GLANCE: usize = 0xd0;
    pub const HEAD: usize = 0xe0;
    pub const CHEST: usize = 0x160;
}

/// 0x1d9840 / 0x1d9850: the start (position, Euler); 0x1d9860 / 0x1d9870: the finish.
const START: ([f32; 3], [f32; 3]) = ([142.0, f32::from_bits(0x42b4_b333), 83.0], [0.0, 0.0, f32::from_bits(0x4016_6666)]);
const FINISH: ([f32; 3], [f32; 3]) = ([f32::from_bits(0x42f0_e666), f32::from_bits(0x438f_799a), 119.5], [0.0, 0.0, f32::from_bits(0xc048_f5c3)]);

/// The race's words (written by the race code, G-LVL-007): the score 0x13fbf8 and the best score 0x15ee6c.
pub const SCORE_WORD: u32 = 0x13_fbf8;
pub const BEST_WORD: u32 = 0x15_ee6c;
/// gp−0x4f8c: the sea 1111's target height.
pub const SEA_WORD: u32 = 0x16_1c74;
/// The skill point 0x13d422.
const SKILL: u32 = 0x13_d422;
/// The Hologuise (item 31) and the Hoverboard (item 30).
const HOLOGUISE: usize = 31;
const HOVERBOARD: usize = 30;

fn music(w: &mut World, track: i16, stinger: i16) { if let Some(s) = w.sound.as_deref_mut() { s.music_request(track, stinger); } }
fn to(w: &mut World, at: ([f32; 3], [f32; 3])) { crate::cinematic::hero_teleport(w, at.0, at.1, 0, true); }

fn set_death_z(w: &mut World, z: f32) {
    w.svc.death_z = z;
    w.hero_fields_mut().death_z = Some(z);
}

/// `0x2e70c0` (module doc).
pub fn race_on(w: &mut World, id: MobyId) {
    for k in 0..16 { let g = c::pi32(w, id, pvo::GROUPS_ON + 4 * k); story::group_set(w, g, 1, 1, 1); }
    for k in 0..8 { let g = c::pi32(w, id, pvo::GROUPS_OFF + 4 * k); story::group_set(w, g, 0, 0, 0); }
    set_death_z(w, 74.0);
    w.svc.units.set_word(SEA_WORD, 76.0f32.to_bits());
    music(w, 2, 6);
}

/// `0x2e7168` (module doc).
pub fn race_off(w: &mut World, id: MobyId) {
    for k in 0..16 { let g = c::pi32(w, id, pvo::GROUPS_ON + 4 * k); story::group_set(w, g, 0, 0, 0); }
    for k in 0..8 { let g = c::pi32(w, id, pvo::GROUPS_OFF + 4 * k); story::group_set(w, g, 1, 1, 0); }
    w.svc.units.set_word(SEA_WORD, 100.0f32.to_bits());
    music(w, 0, 8);
}

/// The talk jump: last = the current node, node = `node`, since = this tick.
fn jump(w: &mut World, id: MobyId, node: i16, since: u32) {
    let pv = &mut w.mm(id).pvars;
    let cur = p::i16(pv, TB + talk::NODE);
    p::set_i16(pv, TB + talk::LAST, cur);
    p::set_i16(pv, TB + talk::NODE, node);
    p::set_u32(pv, TB + talk::SINCE, since);
}

/// Level16 `0x2e6808` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, LEN);
    interact::poll_scene_end_at(w, id, TB);
    let cam = w.camera_point();
    if w.m(id).visible != 0 && c::dist3(w.m(id).position, [cam[0], cam[1], cam[2], 0.0]) < 28.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x16;
    }
    crate::moby_update::manip::scene_big_head(w, &CLASSES, 0, 2.75);
    match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            w.mm(id).update_dist = 0xff;
            interact::talk_register_at(w, id, TB);
            p::set_ff(&mut w.mm(id).pvars, TB + talk::RADIUS, f32::from_bits(0x406c_cccd));
            let y = w.m(id).rotation[2];
            c::set_pf(w, id, pvo::YAW, y);
            // 0x13dc1c (landmark 102's flags) reads 0 in the port.
            w.mm(id).rotation[2] = c::add_rot(y, std::f32::consts::PI);
            race_off(w, id);
        }
        1 => {
            set_death_z(w, if w.hero.group != 0xf { 115.0 } else { 100.0 });
            if interact::talk_update_at(w, id, TB) {
                let y = c::pf(w, id, pvo::YAW);
                w.mm(id).rotation[2] = y;
                interact::place_after_scene(w, id, 3.5);
                w.mm(id).state = 2;
            }
            if !super::hints::owned(w, HOVERBOARD) && c::dist2(story::hero4(w), w.m(id).position) < 3.5 {
                let play = w.ticks(w.svc.help.play_time);
                let n = w.svc.help.records.help[0x87].time as i32;
                if ((w.ticks(0x12) as f32 * 60.0) as i32) < play - n * 600 || n * 600 == 0 {
                    w.svc.help.request(0x3e83, 0x87);
                } else if n < play / 600 {
                    w.svc.help.records.help[0x87].time = (play / 600) as u16;
                }
            }
            if p::i16(&w.m(id).pvars, TB + talk::NODE) == 3 {
                jump(w, id, 2, w.counter as u32);
                w.mm(id).cmd = 0;
                to(w, START);
                w.hero_fields_mut().set_entry_pose = Some(FINISH);
                w.mm(id).state = 3;
                race_on(w, id);
            }
            if w.hero.group == 0x16 {
                w.mm(id).state = 3;
                race_on(w, id);
            }
        }
        2 => {
            if w.svc.game_mode != 2 {
                w.mm(id).state = 1;
                if p::i16(&w.m(id).pvars, TB + talk::LAST) == 4 {
                    crate::cinematic::item_movie(w, 0);
                    w.mm(id).state = 4;
                }
            }
        }
        4 => {
            if w.svc.game_mode == 0 {
                w.mm(id).state = 1;
                interact::give_item(w, HOLOGUISE, true);
                crate::cinematic::save(w);
            }
        }
        _ => {
            let score = w.svc.units.word(SCORE_WORD) as i32;
            if 0x1193 < score {
                story::award_skill_point(w, story::skill_index(SKILL));
                if (w.svc.units.word(BEST_WORD) as i32) < 0x1194 { w.svc.units.set_word(BEST_WORD, score as u32); }
            }
            if w.hero.group == 0 {
                w.mm(id).state = 1;
                race_off(w, id);
            }
            match w.m(id).cmd {
                1 => {
                    w.mm(id).state = 1;
                    to(w, FINISH);
                    w.mm(id).cmd = 0;
                    if w.svc.interact.game.acquired.get(HOLOGUISE).copied().unwrap_or(0) == 0 {
                        p::set_u8(&mut w.mm(id).pvars, TB + talk::AUTO, 1);
                        jump(w, id, 4, w.counter as u32);
                    }
                    race_off(w, id);
                }
                2 => {
                    w.mm(id).state = 1;
                    to(w, FINISH);
                    w.mm(id).cmd = 0;
                    race_off(w, id);
                }
                3 => {
                    w.mm(id).cmd = 0;
                    to(w, START);
                    w.hero_fields_mut().set_entry_pose = Some(FINISH);
                }
                _ => {}
            }
        }
    }
    look(w, id);
}

/// The tail: her head and chest look-at (module doc).
fn look(w: &mut World, id: MobyId) {
    let mut d = 0.3;
    let mut k = 0.02;
    if w.m(id).anim.seq_b == 0 {
        let pos = w.m(id).position;
        let body = w.hero_body_point();
        let body4 = [body[0], body[1], body[2], crate::moby_update::services::fl(w.hero.body_point[3])];
        let near = c::dist2(pos, story::hero4(w)) < 8.0;
        if near && c::diff_rots(w.m(id).rotation[2], c::atan(body[0] - pos[0], body[1] - pos[1])) < std::f32::consts::FRAC_PI_2 {
            let v = w.hero.disp.map(crate::moby_update::services::fl);
            if 0.01 < c::len3([v[0], v[1], v[2], 0.0]) {
                let t = w.ticks(0x78);
                c::set_pi32(w, id, pvo::LOOK_T, t);
            } else {
                c::dec_timer_pvar_i32(w, id, pvo::LOOK_T);
            }
        } else if c::pi32(w, id, pvo::LOOK_T) != 0 {
            c::set_pi32(w, id, pvo::LOOK_T, 0);
            c::set_pv4(w, id, pvo::GLANCE, body4);
        }
        if c::dec_timer_pvar_i32(w, id, pvo::GLANCE_T) != 0 {
            let r = w.rng.randf(180.0, 300.0);
            let t = w.svc.timing.scale(crate::moby_update::services::pf(r)).to_f32() as i32;
            c::set_pi32(w, id, pvo::GLANCE_T, t);
            let a = w.rng.randf(-90.0, 90.0);
            let yaw = c::add_rot(w.m(id).rotation[2], a * 0.017_453_292);
            let b = w.rng.randf(0.0, 30.0);
            let q = c::add(crate::moby_update::creature::fx::polar(6.0, yaw, b * 0.017_453_292), pos);
            c::set_pv4(w, id, pvo::GLANCE, q);
        }
        let target = if c::pi32(w, id, pvo::LOOK_T) != 0 {
            d = 0.3;
            k = 0.04;
            body4
        } else {
            k = 0.02;
            c::pv4(w, id, pvo::GLANCE)
        };
        let eye = [pos[0], pos[1], pos[2] + 1.0, pos[3]];
        let dv = c::sub(target, eye);
        let mut yaw = c::sub_rot(c::atan(dv[0], dv[1]), w.m(id).rotation[2]);
        let half = std::f32::consts::FRAC_PI_2;
        yaw = yaw.clamp(-half, half);
        let pitch = (-c::atan(c::len2(dv), dv[2])).clamp(-f32::from_bits(0x3f06_0a92), f32::from_bits(0x3f06_0a92));
        story::pvars(w, id, LEN);
        c::set_pf(w, id, pvo::HEAD + 0x64, pitch);
        c::set_pf(w, id, pvo::HEAD + 0x68, yaw * 0.7);
        c::set_pf(w, id, pvo::CHEST + 0x68, yaw * 0.3);
    }
    // 0x15ed64 (the springs' rate) is 1.0.
    if w.svc.cheats.on(crate::cheats::slot::ACTORS) { c::set_pf(w, id, pvo::HEAD + 0x70, 2.75); }
    crate::moby_update::manip::look(w, id, id, pvo::HEAD, 0, k, d);
    crate::moby_update::manip::look(w, id, id, pvo::CHEST, 1, k, d);
}
