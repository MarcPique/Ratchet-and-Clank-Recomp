//! **Gemlik's tower fields, classes 404 and 405** (level13 `0x2ed4a8`, the touch `0x2ed158`; census U467; 2 + 3
//! placed, each linked to a tower 170 by pvar +0x00). A flat field (its model, alpha 0x14, its class texture
//! scrolling 0xc0 a tick over 0..0x1000) +0x08 wide and +0x0a high (s16) in the moby's frame. Every tick each live moby
//! within half its larger side of its middle is pressed to the plane (its offset clamped to the field's half sizes
//! when beyond them by 0.7 across / 1.5 up), and a 0.7 sphere there touching it hits it for 1 (flags 0x10001). Its
//! tower destroyed (deleted, or dead in the save): the field fades out (alpha 20 × a value falling 0.05 a tick) and
//! is deleted (freeing a point light it never took, +0x0e).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2ed4a8` | the field (module doc) | [`update`] |
//! | `0x2ed158` | the touch | [`touch`] |
//!
//! Read from the level13 decomp. [L] The texture scroll (`0x25ac38` = L01 `0x263d90`) is kept in +0x0c only: the
//! renderer has no moby texture scroll; the hit's direction register is not set by the caller (zero here). Native
//! `f32`.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::{euler_rows, pv, World};
use crate::moby_update::story;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 13;
pub const UPDATE_FN: u32 = 0x2e_d4a8;
pub const CLASSES: [i16; 2] = [404, 405];

mod pv_ {
    pub const LINK: usize = 0x00;
    pub const FADE: usize = 0x04;
    pub const W: usize = 0x08;
    pub const H: usize = 0x0a;
    pub const SCROLL: usize = 0x0c;
    pub const LIGHT: usize = 0x0e;
    pub const SIZE: usize = 0x10;
}
use pv_ as o;

/// `0x2ed158(m, P)`: the touch (module doc).
fn touch(w: &mut World, id: MobyId) {
    let r = euler_rows(pv(w.m(id).rotation)).map(|row| row.map(|x| f32::from_bits(x.0)));
    let (wd, ht) = (c::pi16(w, id, o::W) as f32, c::pi16(w, id, o::H) as f32);
    let big = (c::pi16(w, id, o::H)).max(c::pi16(w, id, o::W)) as f32 * 0.5;
    let pos = c::pos(w, id);
    let to_world = |l: [f32; 3]| -> c::V { [r[0][0] * l[0] + r[1][0] * l[1] + r[2][0] * l[2] + pos[0], r[0][1] * l[0] + r[1][1] * l[1] + r[2][1] * l[2] + pos[1], r[0][2] * l[0] + r[1][2] * l[1] + r[2][2] * l[2] + pos[2], pos[3]] };
    let mid = to_world([0.0, 0.0, big]);
    for m in w.sphere_mobys_list(Pf::f(big), pv(mid), 0x10, Some(id), None) {
        let s = w.m(m).state;
        if s == 0xfe || s == 0xfd { continue; }
        let d = c::sub(w.m(m).position, pos);
        let mut l = [0.0f32; 3];
        for (i, x) in l.iter_mut().enumerate() { *x = r[i][0] * d[0] + r[i][1] * d[1] + r[i][2] * d[2]; }
        let (hw, hh) = (wd * 0.5, ht * 0.5);
        if l[0] < -(hw + 0.7) { l[0] = -wd * 0.5; } else if hw + 0.7 < l[0] { l[0] = hw; }
        if l[2] < -(hh + 1.5) { l[2] = -ht * 0.5; } else if hh + 1.5 < l[2] { l[2] = hh; }
        l[1] = 0.0;
        let p = to_world(l);
        if w.sphere_mobys_list(Pf::f(f32::from_bits(0x3f33_3333)), pv(p), 0x10, Some(id), None).contains(&m) {
            c::attack::hit_moby(w, m, id, 1.0, 0x1_0001, p, [0.0; 4]);
        }
    }
}

/// Level13 `0x2ed4a8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.alpha = 0x14;
            m.mode |= 0xa08;
            m.ambient = [0x80, 0x80, 0x80, 0];
            c::set_pi16(w, id, o::LIGHT, -1);
            c::set_pf(w, id, o::FADE, 1.0);
        }
        1 => {
            let link = c::pi32(w, id, o::LINK);
            let gone = match usize::try_from(link).ok().filter(|&l| l < w.table.mobys.len()) {
                None => true,
                Some(l) => {
                    let t = w.m(l);
                    t.state == 0xfe || t.state == 0xfd || w.svc.save.death.contains(&(w.svc.level, t.spawn_id))
                }
            };
            if gone {
                c::set_pf(w, id, o::FADE, 1.0);
                w.mm(id).state = 2;
            }
            touch(w, id);
        }
        2 => {
            let f = c::pf(w, id, o::FADE) - c::SPEED * 0.05;
            c::set_pf(w, id, o::FADE, f);
            if f < 0.0 { w.mm(id).state = 3; } else { w.mm(id).alpha = (f * 20.0) as i32 as u8; }
        }
        3 => {
            let l = c::pi16(w, id, o::LIGHT);
            if l != -1 {
                w.svc.point_lights.free(l as u16 as usize);
                c::set_pi16(w, id, o::LIGHT, -1);
            }
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    let mut s = c::pi16(w, id, o::SCROLL) as i32 + 0xc0;
    if 0x1000 < s { s -= 0x1000; } else if s < 0 { s += 0x1000; }
    c::set_pi16(w, id, o::SCROLL, s as i16);
}
