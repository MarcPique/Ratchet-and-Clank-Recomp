//! **Blarg's story NPCs** (level 06; read from the level06 decomp and disassembly):
//!
//! * **1105 "Scrawny Scientist"** (`0x301070`, census U233, one instance): behind the petal door +0x164, he waits
//!   until the creatures (class 827) of his group +0x170 are dead or asleep (state 0x10) and a short delay has
//!   passed, then talks. Every talk below node 2 sets his mission and opens the door +0x164; node 2 gives the
//!   Grindboots (item 29), saves, asks help 6005, and plays a camera glide over the grind rail under a fade (the door
//!   +0x160 opens on the way); he is gone once Ratchet grinds (group 0xf), opening +0x164. Ratchet grinding at any
//!   time opens +0x160.
//!
//! **System or not.** Per-class code on the shared systems: the talk system, the look-at layout, the petal doors'
//! open / close (`petal_door::open` / `close`: the calls `0x2f5360` / `0x2f53e8` that other classes make), the
//! cinematic calls, `story`.
//!
//! ## 1105 coverage (level06 `0x301070`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x300f98`: the big-head cheat's manipulator on his scene actors | `manip::scene_big_head` |
//! | top | drawn and within 24 of the camera: the shadow probe, +0x7f = 0x13 | [`scientist_update`] (`shadows::probe_down`) |
//! | state 0 | Grindboots acquired (0x13d505) → `DeleteMoby`; talk slot 47 not talked to (0x13d8ac): seq 1 (blend `ticks(10)`), +0x176 = 1, door +0x164 closed; else 2 along row 0, z = `GroundHeight(0.5)`, +0x176 = 0; the s16 delay +0x174 = `ticks(45)`; door +0x160 closed; the name, → 1, `NpcTalkRegister` | [`scientist_update`] |
//! | state 1 | radius 16 (auto in cuboid +0x168) else 3.7; group +0x170 clear of live class-827 mobys not in state 0x10 (`0x2e9ea0`), the delay out, `NpcTalkUpdate` → 2 (+0x176 ≠ 0: the move 2 along row 0, grounded), `PlaceAfterScene(2.2)`; Ratchet grinding (0x1413dc = 0xf) → door +0x160 opened | [`scientist_update`] |
//! | state 2 | game mode ≠ 2: seq ≠ 0 → blend 0; node played < 2: `SetMissionDone(+0xb0)`, door +0x164 opened, → 1; node 2: `GiveItem(29, 1)`, `memcard_Save`, `Help_Request(6005, 0x2c)`, fade 1, +0x178 = 0, `CameraScript(A, eA, 1, 0, 0)`, `HeroTeleport(here, 0x72, 1)`, door +0x164 closed, → 3, hidden (not drawn, mode \| 1, collision off) | [`scientist_update`] |
//! | state 3 | `0x26a780(1, 0.1·dt², 0.1·dt², 0.1·dt)` on +0x178 / +0x17c; past 0.1 door +0x160 opened; the camera from A (178.47, 319.76, 146.31) / (0.07, 0.18, −1.77) to B (172.55, 316.01, 145.27) / (0.07, 0.13, −1.21) (`CameraScript` unless the script camera is up, then the targets); the fade toward 1 when t ≥ 1 or (help idle and t > 0.2), else toward 0 (4·dt); at 1: `HeroTeleport(here, 0, 0)`, `CameraScript2(0)`, → 4 | [`scientist_update`] |
//! | state 4 | the fade toward 0; at 0 → 5 | [`scientist_update`] |
//! | state 5 | Ratchet grinding → door +0x164 opened, `DeleteMoby` | [`scientist_update`] |
//! | tail | the look-at (records +0x40 list 0 / +0xc0 list 2, glance +0x140, s32 +0x158 / +0x15c, eye 1, pitch × 1, yaw 0.6 / 0.4); the big-head cheat | [`talking_npc::look_at_layout`] (the cheat in `look_springs`) |

use super::petal_door;
use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::talking_npc::{look_at_layout, LookLayout};
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::interact::{self, talk};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 6;
pub const SCIENTIST_FN: u32 = 0x30_1070;
pub const SCIENTIST_CLASSES: [i16; 1] = [1105];

/// The creatures the scientist waits for (class 827).
const CREATURE: i16 = 0x33b;
const SCIENTIST_LOOK: LookLayout = LookLayout { pitch: (0x40, 0), yaw: (0xc0, 2), glance: 0x140, seen: 0x158, glance_timer: 0x15c, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.6, yaw_b: 0.4, short_timers: false, gate_main: 0, gate_alt: 0xff, k_seen: 0.04 };
const A: ([f32; 3], [f32; 3]) = ([178.47, 319.76, 146.31], [0.07, 0.18, -1.77]);
const B: ([f32; 3], [f32; 3]) = ([172.55, 316.01, 145.27], [0.07, 0.13, -1.21]);

/// `0x2e9ea0(g)`: no live member of group `g` of class 827 outside state 0x10 (an empty or missing group: true).
pub fn group_asleep(w: &World, g: i32) -> bool {
    let Ok(g) = i8::try_from(g) else { return true };
    crate::moby_update::scheduler::group_ids(w, g).into_iter().all(|m| {
        let mo = w.m(m);
        mo.o_class != CREATURE || mo.state == 0xfe || mo.state == 0xfd || mo.state == 0x10
    })
}

fn door(w: &World, id: MobyId, o: usize) -> Option<MobyId> { story::link(w, p::i32(&w.m(id).pvars, o)) }
fn open(w: &mut World, id: MobyId, o: usize) { if let Some(d) = door(w, id, o) { petal_door::open(w, d); } }
fn close(w: &mut World, id: MobyId, o: usize) { if let Some(d) = door(w, id, o) { petal_door::close(w, d); } }

/// The move 2 along row 0, onto the ground.
fn step_out(w: &mut World, id: MobyId) {
    let r = c::set_len3(w.m(id).rows[0], 2.0);
    let m = w.mm(id);
    for (q, d) in m.position.iter_mut().zip(r).take(3) { *q += d; }
    let g = w.ground_height(Pf::f(0.5), w.m(id).position.map(Pf::f), 0).to_f32();
    w.mm(id).position[2] = g;
}

/// Level06 `0x301070` (1105; module doc).
pub fn scientist_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x180);
    interact::poll_scene_end(w, id);
    // 0x300f98: the actors' big-head cheat on the scene actors of [1105] (list 1, 2.3).
    crate::moby_update::manip::scene_big_head(w, &[1105], 1, 2.3);
    if w.m(id).visible != 0 && c::len3(c::sub(c::pos(w, id), w.camera.map(|x| f32::from_bits(x.0)))) < 24.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x13;
    }
    match w.m(id).state {
        0 => {
            if w.svc.interact.game.acquired.get(29).is_some_and(|&b| b != 0) {
                w.delete_moby(id);
                return;
            }
            if !w.svc.interact.game.talked.get(47).is_some_and(|&t| t != 0) {
                if w.m(id).anim.seq_b != 1 {
                    let n = w.ticks(10);
                    w.anim_blend(id, 1, 0, n);
                }
                c::set_pi16(w, id, 0x176, 1);
                close(w, id, 0x164);
            } else {
                step_out(w, id);
                c::set_pi16(w, id, 0x176, 0);
            }
            let t = w.ticks(0x2d);
            c::set_pi16(w, id, 0x174, t as i16);
            close(w, id, 0x160);
            w.mm(id).state = 1;
            interact::talk_register(w, id);
        }
        1 => {
            super::story_npc::talk_radius(w, id, 0, 0x168, f32::from_bits(0x406c_cccd));
            if group_asleep(w, p::i32(&w.m(id).pvars, 0x170)) && c::dec_timer_pvar_s16(w, id, 0x174) != 0 && interact::talk_update(w, id) {
                w.mm(id).state = 2;
                if c::pi16(w, id, 0x176) != 0 {
                    step_out(w, id);
                    c::set_pi16(w, id, 0x176, 0);
                }
                interact::place_after_scene(w, id, f32::from_bits(0x400c_cccd));
            }
            if w.hero.group == 0xf { open(w, id, 0x160); }
        }
        2 => {
            if w.svc.game_mode != 2 && w.svc.interact.talker != Some(id) {
                if w.m(id).anim.seq_b != 0 {
                    let n = w.ticks(10);
                    w.anim_blend(id, 0, 0, n);
                }
                let last = p::i16(&w.m(id).pvars, talk::LAST);
                if last < 2 {
                    let mission = w.m(id).mission;
                    crate::cinematic::set_mission_done(w, mission);
                    open(w, id, 0x164);
                    w.mm(id).state = 1;
                }
                if p::i16(&w.m(id).pvars, talk::LAST) == 2 {
                    interact::give_item(w, 29, true);
                    crate::cinematic::save(w);
                    w.svc.help.request(0x1775, 0x2c);
                    crate::cinematic::set_fade(w, 1.0);
                    c::set_pf(w, id, 0x178, 0.0);
                    crate::cinematic::camera_script(w, A.0, A.1, 1, 0, false);
                    let (pos, yaw) = (crate::hero::physics::to_f32x3(w.hero.pos), w.hero.yaw().to_f32());
                    crate::cinematic::hero_teleport(w, pos, [0.0, 0.0, yaw], 0x72, true);
                    close(w, id, 0x164);
                    let m = w.mm(id);
                    m.state = 3;
                    m.visible = 0;
                    m.mode |= mode::HIDDEN;
                    m.has_collision = false;
                }
            }
        }
        3 => {
            let (mut t, mut v) = (c::pf(w, id, 0x178), c::pf(w, id, 0x17c));
            turn::spring(1.0, DT2 * 0.1, DT2 * 0.1, DT * 0.1, &mut t, &mut v);
            c::set_pf(w, id, 0x178, t);
            c::set_pf(w, id, 0x17c, v);
            if 0.1 < t { open(w, id, 0x160); }
            let pos: [f32; 3] = std::array::from_fn(|l| A.0[l] + (B.0[l] - A.0[l]) * t);
            let e: [f32; 3] = std::array::from_fn(|l| c::lerp_rot(A.1[l], B.1[l], t));
            crate::cinematic::camera_script_unless_script(w, A.0, A.1, 1, 0, false);
            crate::cinematic::camera_targets(w, Some(pos), Some(e));
            let mut f = w.svc.cinematic.fade;
            let up = 1.0 <= t || (w.svc.help.idle() && 0.2 < t);
            turn::approach(if up { 1.0 } else { 0.0 }, DT * 4.0, &mut f);
            crate::cinematic::set_fade(w, f);
            if f == 1.0 {
                let (pos, yaw) = (crate::hero::physics::to_f32x3(w.hero.pos), w.hero.yaw().to_f32());
                crate::cinematic::hero_teleport(w, pos, [0.0, 0.0, yaw], 0, false);
                crate::cinematic::camera_script2(w, 0);
                w.mm(id).state = 4;
            }
        }
        4 => {
            let mut f = w.svc.cinematic.fade;
            turn::approach(0.0, DT * 4.0, &mut f);
            crate::cinematic::set_fade(w, f);
            if f == 0.0 { w.mm(id).state = 5; }
        }
        5 if w.hero.group == 0xf => {
            open(w, id, 0x164);
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    look_at_layout(w, id, &SCIENTIST_LOOK);
}
