//! Rilgar's hinged hatches, class 893 (level 05, 4 created instances): level05 0x316258 (census U194). A hatch whose
//! lid is turned by a manipulator on its joint list 0 (the angle about x, `FUN_00221e38`). Mode 0 hatches start
//! ajar (34°) and, once their linked moby's command byte is set, fall shut (class sound 0) with one bounce and then
//! slide 1.9 along −row 1 (sound 1); the other mode starts shut, displaced 2 along row 1 (−2 when flipped), slides
//! home when the link's command byte is set to anything but 2, then swings open to 68° with one bounce. Flipped ones
//! (pvar +0x50) are upside down and 4 higher. The light word and ambient are Ratchet's moby's. Read from the level05
//! decomp and disassembly of 0x316258 (state 4 slides to the whole home point). Native `f32`.
//!
//! **Pvar block**: +0x00 home, +0x10 the manipulator record (`manip`, node + quaternion at +0x20), +0x50 s32 flipped,
//! +0x54 s32 the mode, +0x58 s32 the linked moby, +0x5c the slide speed, +0x60 the lid angle, +0x64 its velocity.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | flipped → rot.x = π, z += 4; home = position; `AttachManipulator(m, 0, +0x10)`; mode 0 → 1, angle = 34° (0x3f17e9d8), `FUN_00221e38(angle, +0x20, 0)` (0x22ec10); else angle 0, → 2, position += row 1 (+0xd0) · (flipped ? 2 : −2); light word / ambient (+0x38) = Ratchet's moby's (0x1413d0) | [`update`] (`manip::attach`, `manip::set_axis`) |
//! | states 1 / 2 | the link's +0xbc = 0 → nothing; state 2 and +0xbc ≠ 2 → 4; state 1 → `PlayClassSound(0, 0, m)`, → 6 | [`update`] |
//! | states 3 / 4 | target = home − 1.9·row 1 (3) or home (4); `0x285be8(\|target − position\|, 6·dt², 12·dt², 10·dt, &0, &+0x5c)` (L01 0x270830, from 0); position += unit(target − position)·+0x5c; within 0.0001 → 3: `PlayClassSound(1, 0, m)`, → 7; 4: → 5 | [`update`] (`turn::spring`) |
//! | states 5 / 6 | target 68° (5) / −34° (6); `0x286078(target, 4π/3·dt², 4π/3·dt², 4π·dt, &angle, &+0x64)` (L01 0x270cc0); past 34° or below 0: clamped; the command byte +0xbc = 0 → +0xbc = 1, velocity ·= −0.33 (the bounce); else at the bound (34° for 5, 0 for 6) → 7 (5) / 3 (6); `FUN_00221e38(angle, +0x20, 0)` | [`update`] (`turn::turn_toward`, `manip::set_axis`) |
//! | state 7 | done | n/a |
//! | | no particle, hit, flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::manip;
use crate::moby_update::services::World;

/// The update in the level05 class table.
pub const UPDATE_FN: u32 = 0x31_6258;
pub const REFERENCE_LEVEL: u32 = 5;
pub const CLASSES: [i16; 1] = [893];
pub const REC: usize = 0x10;
/// 34° (0x3f17e9d8) and 68° (0x3f97e9d8).
pub const AJAR: f32 = f32::from_bits(0x3f17_e9d8);
pub const OPEN: f32 = f32::from_bits(0x3f97_e9d8);

fn set_angle(w: &mut World, id: MobyId, a: f32) {
    c::set_pf(w, id, 0x60, a);
    manip::set_axis(w, id, id, REC, a, 0);
}

/// Level05 0x316258 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x68 { return; }
    let st = w.m(id).state;
    match st {
        0 => {
            let flipped = c::pi32(w, id, 0x50) != 0;
            if flipped {
                let m = w.mm(id);
                m.rotation[0] = std::f32::consts::PI;
                m.position[2] += 4.0;
            }
            let pos = w.m(id).position;
            c::set_pv4(w, id, 0, pos);
            manip::attach(w, id, 0, id, REC);
            if c::pi32(w, id, 0x54) == 0 {
                w.mm(id).state = 1;
                set_angle(w, id, AJAR);
            } else {
                c::set_pf(w, id, 0x60, 0.0);
                w.mm(id).state = 2;
                let r1 = w.m(id).rows[1];
                let k = if flipped { 2.0 } else { -2.0 };
                let m = w.mm(id);
                for (q, r) in m.position.iter_mut().zip(r1) { *q += r * k; }
            }
            super::take_hero_light(w, id);
        }
        1 | 2 => {
            let link = usize::try_from(c::pi32(w, id, 0x58)).ok().and_then(|l| w.table.mobys.get(l)).map_or(0, |m| m.cmd);
            if link == 0 { return; }
            if st == 2 && link != 2 { w.mm(id).state = 4; return; }
            if st != 1 { return; }
            w.play_sound(0, 0, id);
            w.mm(id).state = 6;
        }
        3 | 4 => {
            let home = c::pv4(w, id, 0);
            let target = if st == 3 { c::add(home, c::scale(w.m(id).rows[1], -1.9)) } else { home };
            let pos = w.m(id).position;
            let l = c::len3(c::sub(target, pos));
            let (mut x, mut v) = (0.0, c::pf(w, id, 0x5c));
            turn::spring(l, DT2 * 6.0, DT2 * 12.0, DT * 10.0, &mut x, &mut v);
            c::set_pf(w, id, 0x5c, v);
            let step = c::set_len3(c::sub(target, pos), v);
            let p = c::add(pos, step);
            w.mm(id).position = p;
            if 0.0001 <= c::dist3(p, target) { return; }
            if st == 3 {
                w.play_sound(1, 0, id);
                w.mm(id).state = 7;
            } else {
                w.mm(id).state = 5;
            }
        }
        5 | 6 => {
            let (bound, target) = if st == 5 { (AJAR, OPEN) } else { (0.0, -AJAR) };
            let mut a = c::pf(w, id, 0x60);
            let mut v = c::pf(w, id, 0x64);
            let k = 4.188_790_3 * DT2;
            turn::turn_toward(target, k, k, DT * 12.566_371, &mut a, &mut v);
            if AJAR < a || a < 0.0 {
                a = if AJAR < a { AJAR } else { 0.0 };
                if w.m(id).cmd == 0 {
                    w.mm(id).cmd = 1;
                    v *= -0.33;
                } else if a == bound {
                    w.mm(id).state = if st == 5 { 7 } else { 3 };
                }
            }
            c::set_pf(w, id, 0x64, v);
            set_angle(w, id, a);
        }
        _ => {}
    }
}
