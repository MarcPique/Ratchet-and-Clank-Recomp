//! Orxon's trip blocks, classes 1015 and 1282: level10 0x2d90a8 (census U329; 9 created instances). A block that,
//! once put in state 1 (by other code), sinks 0.2 a tick to its home height and rises 4 above it when a live member
//! of its group (+0x10), the camera or Ratchet stands in its box (|x| < 1.75, |y| < 0.5 in its own frame, any z);
//! at the top it sets its global flag (`0x13d388 + 0x4c + i`). Read from the level10 decomp and disassembly
//! (0x2d90a8, the box test 0x2d9000). Native `f32`.
//!
//! **Pvar block**: +0x00 home, +0x10 the group (−1 none), +0x14 the flag index (−1 none).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2d90a8 | no pvar block → a debug `printf`, `DeleteMoby` | [`update`] |
//! | state 0 | home = position, → 2 | [`update`] |
//! | state 1 | z −= 4 / `multiply_global_scale(20)`; at home z → home z, → 2; the group's live members (`0x26e150` / `0x26e238`), then the camera (0x167240), then Ratchet (0x13f3d0) in the box (0x2d9000) → 3 | [`update`], [`in_box`] |
//! | 0x2d9000 | l = (p − position)·transpose(rows) (`fun_001fa2d8`, `fun_001f9d20`); \|l.x\| < 1.75 (gp−0x4ff0), \|l.y\| < 0.5 (gp−0x4ff4) | [`in_box`] |
//! | state 3 | z += 0.2; at home z + 4 → there, → 4, flag `0x13d3d4[+0x14]` = 1 | [`update`] (`interact::set_global_flag`) |
//! | | no sound, particle, hit, other moby written | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{pi32, pv4, set_pv4};
use crate::moby_update::services::World;

/// The update in the level10 class table.
pub const UPDATE_FN: u32 = 0x2d_90a8;
pub const REFERENCE_LEVEL: u32 = 10;
pub const CLASSES: [i16; 2] = [1015, 1282];
/// gp−0x4ff0 / −0x4ff4: the box half-sizes.
pub const HALF_X: f32 = 1.75;
pub const HALF_Y: f32 = 0.5;
/// The rise height.
pub const RISE: f32 = 4.0;
/// `0x13d3d4 − 0x13d388`: the global flags' base for +0x14.
pub const FLAG_BASE: usize = 0x4c;

/// `0x2d9000(m, p)` (module doc).
pub fn in_box(w: &World, id: MobyId, p: [f32; 3]) -> bool {
    let m = w.m(id);
    let d = [p[0] - m.position[0], p[1] - m.position[1], p[2] - m.position[2]];
    let l = |r: [f32; 4]| d[0] * r[0] + d[1] * r[1] + d[2] * r[2];
    l(m.rows[0]).abs() < HALF_X && l(m.rows[1]).abs() < HALF_Y
}

fn step(w: &World) -> f32 { 4.0 / crate::moby_update::services::fl(w.svc.timing.scale(crate::ps2v::Pf::f(20.0))) }

/// Level10 0x2d90a8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.is_empty() {
        w.delete_moby(id);
        return;
    }
    if w.m(id).pvars.len() < 0x18 { return; }
    let home = pv4(w, id, 0);
    match w.m(id).state {
        0 => {
            let p = w.m(id).position;
            set_pv4(w, id, 0, p);
            w.mm(id).state = 2;
        }
        1 => {
            let z = w.m(id).position[2] - step(w);
            w.mm(id).position[2] = z;
            if z <= home[2] {
                let m = w.mm(id);
                m.position[2] = home[2];
                m.state = 2;
            }
            let g = pi32(w, id, 0x10);
            let mut hit = false;
            if g != -1 {
                for k in crate::moby_update::scheduler::group_ids(w, g as i8) {
                    let m = w.m(k);
                    if (m.state as i8) < 0 { continue; }
                    if in_box(w, id, [m.position[0], m.position[1], m.position[2]]) { hit = true; break; }
                }
            }
            if !hit {
                let h = super::hero_pos(w);
                hit = in_box(w, id, w.camera_point()) || in_box(w, id, [h[0], h[1], h[2]]);
            }
            if hit { w.mm(id).state = 3; }
        }
        3 => {
            let z = w.m(id).position[2] + step(w);
            w.mm(id).position[2] = z;
            if home[2] + RISE <= z {
                let m = w.mm(id);
                m.position[2] = home[2] + RISE;
                m.state = 4;
                let f = pi32(w, id, 0x14);
                if f != -1 { crate::moby_update::interact::set_global_flag(w, FLAG_BASE + f as usize, 1); }
            }
        }
        _ => {}
    }
}
