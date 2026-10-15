//! Blarg's split doors, class 1054 (level 06, 9 created instances) with the half 1055 it creates: level06 0x2fbfb0
//! (census U222). The moby is one half of a door; at the init it creates the other half (class 1055, which has no
//! update of its own) at the same place. Mode 2 doors stay shut; doors without a cuboid start open; the others open
//! when Ratchet steps into their cuboid (the halves spring apart along the door's z row to 2 and 2.75 from home, class
//! sound 0 on the other half; with a group, the group is set to state 3 instead), and mode 1 doors only jam: they
//! rattle half open, swinging their halves ±20 (sic: radians, the data's value) by turns until they settle 1.5 apart.
//! Read from the level06 decomp of 0x2fbfb0 and the overlay's data (gp−0x4ce4 .. −0x4ca4). Native `f32`; the `rand`
//! draws in the game's order.
//!
//! **Pvar block**: +0x00 s32 the cuboid (−1 none), +0x04 s32 the mode, +0x08 / +0x0c this half's slide / turn
//! velocity, +0x10 home, +0x20 home rotation, +0x30 the axis (row 2 at the init), +0x40 the other half, +0x44 / +0x48
//! its slide / turn velocity, +0x4c s32 the rattle timer.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | home = position, home rotation = rotation, axis = row 2 (+0xe0); `CreateMoby(0x41f)`: draw distance 0x40, drawn (+0x31 = 1), the light word and ambient (+0x38) and mode (+0x34) copied, position and rotation = this one's | [`update`] (`World::create_moby`) |
//! | | cuboid ≥ 0 → 1; mode 2 → 2 (shut for good); else position = home + unit(axis)·2, the other = home − unit(axis)·2.75 (gp−0x4ce4 / −0x4ce0), its bounding sphere (0x20def8), → 5 | [`update`] |
//! | state 1 | Ratchet (0x13f3d0) in the cuboid (0x26e770 = L01 0x274820): mode 1 → 4, timer = `ticks(20)`; else group ≠ −1 → `0x267f70(group, 3)` (L01 0x26e0e0: every member's state 3, this one's too); else → 3; `PlayClassSound(0, 0, other half)` | [`update`] (`scheduler::group_state`) |
//! | state 3 | a = \|position − home\|, b = \|other − home\|; `0x26a780(2, 10·dt², 20·dt², 5·dt, &a, &+0x08)`, `(2.75, …, &b, &+0x44)` (L01 0x270830); position = home + unit(axis)·a, other = home − unit(axis)·b, its sphere; \|a − 2\| < 0.01 and \|b − 2.75\| < 0.01 → 5 | [`update`] (`turn::spring`) |
//! | state 4 | by the command byte +0xbc: 1 → slide targets a, b (stay), turn targets their own rotation y (stay), next 2, timer `trunc(randf(5, 15))`; 2 → slide 1.5 / 1.5 (gp−0x4cdc / −0x4cd8), turn +20 / −20 (gp−0x4cd4 / −0x4cd0), next 1, timer `trunc(randf(10, 30))` (both with slide rates 10·dt², 50·dt², 1·dt and turn rates 100°·dt², 300°·dt², 45°·dt: gp−0x4ccc .. −0x4cb8); else → slide 1.5 / 1.5, turn targets this half's rotation y for both, next 1, timer 30, rates 10·dt², 20·dt², 5·dt and 10°·dt², 10°·dt², 5°·dt | [`update`] |
//! | | the four springs (slides, then rotation y of each half with +0x0c / +0x48); positions as in state 3, its sphere; `FastDecTimer(+0x4c)` out → +0xbc = next, timer = `ticks(that)`, and \|a − 1.5\| < 0.01 and \|b − 1.5\| < 0.01 → 5 | [`update`] |
//! | state 5 | gp−0x4ca4 (0.0) ≠ 0 → reset to home and state 1: never (n/a) | [`update`] |
//! | | no particle, hit, flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::scheduler::group_state;
use crate::moby_update::services::World;

/// The update in the level06 class table.
pub const UPDATE_FN: u32 = 0x2f_bfb0;
pub const REFERENCE_LEVEL: u32 = 6;
pub const CLASSES: [i16; 1] = [1054];
/// The other half (no update of its own).
pub const HALF: i16 = 0x41f;
pub const OPEN: [f32; 2] = [2.0, 2.75];
pub const JAM: [f32; 2] = [1.5, 1.5];
pub const SWING: [f32; 2] = [20.0, -20.0];
const DEG: f32 = 0.017_453_292;

fn other(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, 0x40)).ok().filter(|&o| o < w.table.mobys.len()) }

/// position = home + unit(axis)·a, the other = home − unit(axis)·b, its sphere.
fn place(w: &mut World, id: MobyId, o: Option<MobyId>, a: f32, b: f32) {
    let (home, axis) = (c::pv4(w, id, 0x10), c::pv4(w, id, 0x30));
    w.mm(id).position = c::add(home, c::set_len3(axis, a));
    if let Some(o) = o {
        w.mm(o).position = c::add(home, c::set_len3(axis, -b));
        w.build_matrix(o);
    }
}

fn dists(w: &World, id: MobyId, o: Option<MobyId>) -> (f32, f32) {
    let home = c::pv4(w, id, 0x10);
    (c::dist3(w.m(id).position, home), o.map_or(0.0, |o| c::dist3(w.m(o).position, home)))
}

/// Level06 0x2fbfb0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x50 { return; }
    match w.m(id).state {
        0 => {
            let m = w.m(id);
            let (p, r, ax, light, amb, mode) = (m.position, m.rotation, m.rows[2], m.light, m.ambient, m.mode);
            c::set_pv4(w, id, 0x10, p);
            c::set_pv4(w, id, 0x20, r);
            c::set_pv4(w, id, 0x30, ax);
            let o = w.create_moby(HALF);
            c::set_pi32(w, id, 0x40, o.map_or(-1, |o| o as i32));
            if let Some(o) = o {
                let h = w.mm(o);
                h.draw_dist = 0x40;
                h.visible = 1;
                h.light = light;
                h.ambient = amb;
                h.mode = mode;
                h.position = p;
                h.rotation = r;
            }
            if c::pi32(w, id, 0) >= 0 { w.mm(id).state = 1; return; }
            if c::pi32(w, id, 4) == 2 { w.mm(id).state = 2; return; }
            place(w, id, o, OPEN[0], OPEN[1]);
            w.mm(id).state = 5;
        }
        1 => {
            if !w.in_cuboid(w.hero_point(), c::pi32(w, id, 0)) { return; }
            if c::pi32(w, id, 4) == 1 {
                w.mm(id).state = 4;
                let t = w.ticks(0x14);
                c::set_pi32(w, id, 0x4c, t);
            } else if w.m(id).group != -1 {
                let g = w.m(id).group;
                group_state(w, g, 3);
            } else {
                w.mm(id).state = 3;
            }
            if let Some(o) = other(w, id) { w.play_sound(0, 0, o); }
        }
        3 => {
            let o = other(w, id);
            let (mut a, mut b) = dists(w, id, o);
            let (mut va, mut vb) = (c::pf(w, id, 0x08), c::pf(w, id, 0x44));
            turn::spring(OPEN[0], DT2 * 10.0, DT2 * 20.0, DT * 5.0, &mut a, &mut va);
            turn::spring(OPEN[1], DT2 * 10.0, DT2 * 20.0, DT * 5.0, &mut b, &mut vb);
            c::set_pf(w, id, 0x08, va);
            c::set_pf(w, id, 0x44, vb);
            place(w, id, o, a, b);
            if (a - OPEN[0]).abs() < 0.01 && (b - OPEN[1]).abs() < 0.01 { w.mm(id).state = 5; }
        }
        4 => {
            let o = other(w, id);
            let (a, b) = dists(w, id, o);
            let my = w.m(id).rotation[1];
            let their = o.map_or(0.0, |o| w.m(o).rotation[1]);
            let (ta, tb, ra, rb, next, timer, s, t) = match w.m(id).cmd {
                1 => {
                    let n = w.rng.randf(5.0, 15.0) as i32;
                    (a, b, my, their, 2u8, n, [10.0, 50.0, 1.0], [100.0, 300.0, 45.0])
                }
                2 => {
                    let n = w.rng.randf(10.0, 30.0) as i32;
                    (JAM[0], JAM[1], SWING[0], SWING[1], 1, n, [10.0, 50.0, 1.0], [100.0, 300.0, 45.0])
                }
                _ => (JAM[0], JAM[1], my, my, 1, 0x1e, [10.0, 20.0, 5.0], [10.0, 10.0, 5.0]),
            };
            let (mut a, mut b) = (a, b);
            let (mut va, mut vb) = (c::pf(w, id, 0x08), c::pf(w, id, 0x44));
            turn::spring(ta, s[0] * DT2, s[1] * DT2, s[2] * DT, &mut a, &mut va);
            turn::spring(tb, s[0] * DT2, s[1] * DT2, s[2] * DT, &mut b, &mut vb);
            c::set_pf(w, id, 0x08, va);
            c::set_pf(w, id, 0x44, vb);
            let (ka, kd, km) = (t[0] * DEG * DT2, t[1] * DEG * DT2, t[2] * DEG * DT);
            let mut x = my;
            let mut v = c::pf(w, id, 0x0c);
            turn::spring(ra, ka, kd, km, &mut x, &mut v);
            w.mm(id).rotation[1] = x;
            c::set_pf(w, id, 0x0c, v);
            if let Some(o) = o {
                let mut x = w.m(o).rotation[1];
                let mut v = c::pf(w, id, 0x48);
                turn::spring(rb, ka, kd, km, &mut x, &mut v);
                w.mm(o).rotation[1] = x;
                c::set_pf(w, id, 0x48, v);
            }
            place(w, id, o, a, b);
            let mut tm = c::pi32(w, id, 0x4c);
            let fired = crate::moby_update::classes::gold_bolt::fast_dec_timer(&mut tm);
            c::set_pi32(w, id, 0x4c, tm);
            if fired == 0 { return; }
            w.mm(id).cmd = next;
            let t = w.ticks(timer);
            c::set_pi32(w, id, 0x4c, t);
            if (a - JAM[0]).abs() < 0.01 && (b - JAM[1]).abs() < 0.01 { w.mm(id).state = 5; }
        }
        _ => {}
    }
}
