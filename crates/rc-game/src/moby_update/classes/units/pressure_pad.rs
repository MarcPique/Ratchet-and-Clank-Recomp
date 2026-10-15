//! Quartu's pressure pads, class 1209: level15 0x2e5958 (census U487; 15 created instances on 15 / 17, the level17
//! copy 0x2e5f50 is the same code). A group of pads: the first to initialise leads (its reach is the distance to its
//! joint 0, handed to the others). Ratchet standing on a pad (within the reach, and local y < 0.2) lights it green and
//! starts the leader's clock (ticks(+0x0c), 900 without one) with a ticking sound that speeds up as it runs out; all
//! pads pressed in time → the group goes to 4 (solved: blinking, death bits kept), else the group resets (3 → 1).
//! Read from the level15 decomp and disassembly (0x2e5958; the pressed test at 0x2e5990). Native `f32`.
//!
//! **Pvar block**: +0x00 s16 the clock, +0x02 s16 the tick timer, +0x04 s16 pressed, +0x06 s16 the blink timer,
//! +0x08 u8 the blink phase, +0x09 u8 a follower, +0x0a u8 pressed count, +0x0b u8 the group's count, +0x0c s32 the
//! time (seconds·60 ticks), +0x10 the leader (moby index + 1 here; a pointer in the game), +0x14 the reach, +0x18 s16
//! the solved clock value, +0x1c s32 keep solved.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2e5958 | states 1 / 2: pressed = \|pos − Ratchet\| < reach and (pos − Ratchet)·row 1 < 0.2 (0x1f8a20, 0x1f8b50, the transpose 0x1f9470, 0x1f8e70) | [`pressed`] |
//! | state 0 | not a follower: reach = distance to joint 0 (`0x23ee30` = 0x2645a8), count 0, leader = itself; each live group member (0x248b38 / 0x248c20): another 1209 gets leader = this, follower 1, the reach; count += 1 for every member; blink timer 0 | [`update`] (`scheduler::group_ids`, `World::joint_point`) |
//! | | clock, tick timer, pressed count, pressed = 0; ambient (0, 0, 0) (`0x23f958` = 0x2650d0); → 1 | [`update`] |
//! | states 1 / 2 | the collected byte (`0x1bbc84[id]`, level01 `0x1bbb04`) or the death bit set and +0x1c ≠ 0 → group state 4 (`0x248ac8` = 0x26e0e0), clock = +0x18, → 4 | [`update`] (`scheduler::group_state`) |
//! | | not pressed yet and pressed now: the leader in state 1 → this pad's pressed count 0 (sic), the leader → 2, its clock = ticks(its +0x0c) (+0x0c < 1: 900); the leader's pressed count += 1; `PlayClassSound(3, 0x31, m)`; ambient (0, 0xff, 0); pressed | [`update`] |
//! | state 2 | pressed count < count: the clock (`FastDecTimer_s16`) runs → the tick timer done → sound 0 (0x31), tick timer = min(ticks(45), ticks(5) + clock / ticks(40)); the clock out → pressed 0, count 0, sound 2 (0x31), group state 3 | [`update`] |
//! | | all pressed → group state 4, sound 1 (0x31), clock = +0x18, → 4 | [`update`] |
//! | state 3 | ambient (0, 0, 0), pressed 0, count 0, → 1 | [`update`] |
//! | state 4 | the blink timer done → ticks(20), ambient white / green by turns; the death bits (`0x14c190[level]`, `0x1baad0` = level01 `0x1ba950`) every tick | [`update`] |
//! | | no particle, hit, light | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, pi16, pi32, pu8, set_pf, set_pi16, set_pi32, set_pu8};
use crate::moby_update::scheduler::{group_ids, group_state};
use crate::moby_update::services::{fast_dec_timer_s16, World};

/// The update in the level15 class table.
pub const UPDATE_FN: u32 = 0x2e_5958;
pub const REFERENCE_LEVEL: u32 = 15;
pub const CLASSES: [i16; 1] = [1209];
/// The sound flags of every call.
const SND: u32 = 0x31;

fn ambient(w: &mut World, id: MobyId, r: u8, g: u8, b: u8) { w.mm(id).ambient = [r, g, b, 0]; }

/// The pressed test (module doc).
pub fn pressed(w: &World, id: MobyId) -> bool {
    let m = w.m(id);
    let h = super::hero_pos(w);
    let d = c::sub(m.position, h);
    if c::len3(d) >= c::pf(w, id, 0x14) { return false; }
    let r = m.rows[1];
    d[0] * r[0] + d[1] * r[1] + d[2] * r[2] < 0.2
}

fn leader(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(pi32(w, id, 0x10) - 1).ok().filter(|&l| l < w.table.mobys.len()) }

fn solved_before(w: &World, id: MobyId) -> bool {
    let b2 = w.m(id).spawn_id;
    b2 >= 0 && (w.svc.save.collected.get(&b2).is_some_and(|&v| v != 0) || w.svc.save.death.contains(&(w.svc.level, b2)))
}

fn dec16(w: &mut World, id: MobyId, o: usize) -> i32 {
    let mut t = pi16(w, id, o);
    let r = fast_dec_timer_s16(&mut t);
    set_pi16(w, id, o, t);
    r
}

/// Level15 0x2e5958 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x20 { return; }
    let st = w.m(id).state;
    let on = matches!(st, 1 | 2) && pressed(w, id);
    let g = w.m(id).group;
    match st {
        0 => {
            if pu8(w, id, 9) == 0 {
                let j = w.joint_point(id, 0);
                let reach = c::dist3(w.m(id).position, j);
                set_pf(w, id, 0x14, reach);
                set_pu8(w, id, 0xb, 0);
                set_pi32(w, id, 0x10, id as i32 + 1);
                for k in group_ids(w, g) {
                    if (w.m(k).state as i8) < 0 { continue; }
                    if k != id && w.m(k).o_class == CLASSES[0] && w.m(k).pvars.len() >= 0x18 {
                        set_pi32(w, k, 0x10, id as i32 + 1);
                        set_pu8(w, k, 9, 1);
                        set_pf(w, k, 0x14, reach);
                    }
                    let n = pu8(w, id, 0xb).wrapping_add(1);
                    set_pu8(w, id, 0xb, n);
                }
            }
            set_pi16(w, id, 6, 0);
            set_pi16(w, id, 0, 0);
            set_pi16(w, id, 2, 0);
            set_pu8(w, id, 0xa, 0);
            set_pi16(w, id, 4, 0);
            ambient(w, id, 0, 0, 0);
            w.mm(id).state = 1;
        }
        1 | 2 => {
            if solved_before(w, id) && pi32(w, id, 0x1c) != 0 {
                group_state(w, g, 4);
                let v = pi16(w, id, 0x18);
                set_pi16(w, id, 0, v);
                w.mm(id).state = 4;
                return;
            }
            if pi16(w, id, 4) == 0 && on {
                if let Some(l) = leader(w, id) {
                    if w.m(l).state == 1 && w.m(l).pvars.len() >= 0x20 {
                        set_pu8(w, id, 0xa, 0);
                        w.mm(l).state = 2;
                        let t = pi32(w, l, 0xc);
                        let clock = if t < 1 { 900 } else { w.ticks(t) };
                        set_pi16(w, l, 0, clock as i16);
                    }
                    if w.m(l).pvars.len() >= 0x20 {
                        let n = pu8(w, l, 0xa).wrapping_add(1);
                        set_pu8(w, l, 0xa, n);
                    }
                }
                w.play_sound(3, SND, id);
                ambient(w, id, 0, 0xff, 0);
                set_pi16(w, id, 4, 1);
            }
            if w.m(id).state != 2 { return; }
            if pu8(w, id, 0xa) < pu8(w, id, 0xb) {
                if dec16(w, id, 0) == 0 {
                    if dec16(w, id, 2) != 0 {
                        w.play_sound(0, SND, id);
                        let (a, b, d) = (w.ticks(45), w.ticks(5), w.ticks(40).max(1));
                        let clock = pi16(w, id, 0) as i32;
                        let t = if a < b + clock / d { a } else { b + clock / d };
                        set_pi16(w, id, 2, t as i16);
                    }
                } else {
                    set_pi16(w, id, 4, 0);
                    set_pu8(w, id, 0xa, 0);
                    w.play_sound(2, SND, id);
                    group_state(w, g, 3);
                }
            } else {
                group_state(w, g, 4);
                w.play_sound(1, SND, id);
                let v = pi16(w, id, 0x18);
                set_pi16(w, id, 0, v);
                w.mm(id).state = 4;
            }
        }
        3 => {
            ambient(w, id, 0, 0, 0);
            set_pi16(w, id, 4, 0);
            set_pu8(w, id, 0xa, 0);
            w.mm(id).state = 1;
        }
        4 => {
            if dec16(w, id, 6) != 0 {
                let t = w.ticks(20);
                set_pi16(w, id, 6, t as i16);
                if pu8(w, id, 8) == 0 {
                    ambient(w, id, 0xff, 0xff, 0xff);
                    set_pu8(w, id, 8, 1);
                } else {
                    ambient(w, id, 0, 0xff, 0);
                    set_pu8(w, id, 8, 0);
                }
            }
            let (b2, lvl) = (w.m(id).spawn_id, w.svc.level);
            if b2 >= 0 {
                w.svc.save.death.insert((lvl, b2));
                w.svc.save.death_level.insert(b2);
            }
        }
        _ => {}
    }
}
