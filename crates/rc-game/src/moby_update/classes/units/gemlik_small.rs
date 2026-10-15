//! **Gemlik's small classes** (level 13; read from the level13 decomp; native `f32`):
//!
//! * **1403 the tracker** (`0x30bb70`, census U480, one placed): it sits on the moby +0x00 while that moby's state is
//!   below 7; after, along the path +0x04 (its chords into the points' w at the start): for the first 30 ticks, or
//!   while the moby's nearest point on the path (`SplineSample(100, 1, 0)`) moves on, it takes that point; then it
//!   rides the path at 0.25 a tick (`0x281e40`) to its last point. The moby deleted: deleted.
//! * **1558 the scene thrusters** (`0x30bdf0`, U481, one placed): updated always; in scene 2 (game mode 2) the
//!   thrusters on the scene's third actor.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x30bb70` | 1403 | [`tracker_update`] |
//! | `0x281e40` | a step of `d` along the path from the segment start `seg` (`dir` the way), the point and its segment | [`path_step`] |
//! | `0x30bdf0` | 1558 | [`thrusters_update`] |
//!
//! [L] 1558's actor is the word 0x16cce0 of the scene's actor table, taken as its third entry.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::World;
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 13;
pub const TRACKER_FN: u32 = 0x30_bb70;
pub const TRACKER_CLASSES: [i16; 1] = [1403];
pub const THRUSTERS_FN: u32 = 0x30_bdf0;
pub const THRUSTERS_CLASSES: [i16; 1] = [1558];

fn pt(w: &World, p: usize, i: i32) -> c::V { w.svc.splines[p].get(i.max(0) as usize).map_or([0.0; 4], |q| q.map(f32::from_bits)) }

/// The neighbour of point `i` the way `dir` (`0x281e40`'s end rules).
fn next(i: i32, n: i32, dir: i32) -> i32 {
    if i == n - 1 { if dir < 1 { n - 2 } else { 0 } } else if i == 0 { if dir < 1 { n - 1 } else { 1 } } else { i + dir }
}

/// `0x281e40(d, m, path, &out, seg, dir)` (module doc): (the point, its segment).
pub fn path_step(w: &World, d: f32, at: c::V, p: usize, seg: i32, dir: i32) -> (c::V, i32) {
    let n = w.svc.splines[p].len() as i32;
    let mut seg = seg;
    let a = next(seg, n, dir);
    let v = c::sub(pt(w, p, a), pt(w, p, seg));
    let l = c::len3(v);
    let mut u = c::set_len3(v, 1.0);
    let mut s = c::dot3(c::sub(at, pt(w, p, seg)), u) + d;
    if l < s {
        let b = next(a, n, dir);
        u = c::set_len3(c::sub(pt(w, p, b), pt(w, p, a)), 1.0);
        s -= l;
        seg = a;
    }
    (c::add(c::set_len3(u, s), pt(w, p, seg)), seg)
}

/// Level13 `0x30bb70`: 1403 (module doc).
pub fn tracker_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x14);
    let path = usize::try_from(c::pi32(w, id, 4)).ok().filter(|&p| p < w.svc.splines.len());
    if w.m(id).state == 0 {
        w.mm(id).update_dist = 0xff;
        if let Some(p) = path {
            let n = w.svc.splines[p].len();
            for i in 0..n {
                let d = c::dist3(pt(w, p, i as i32), pt(w, p, ((i + 1) % n) as i32));
                w.svc.splines[p][i][3] = d.to_bits();
            }
        }
        c::set_pi32(w, id, 8, 0);
        c::set_pf(w, id, 0xc, 0.0);
        w.mm(id).state = 1;
        c::set_pi32(w, id, 0x10, 0);
    }
    let link = c::pi32(w, id, 0);
    if link == -1 { return; }
    let Some(m) = usize::try_from(link).ok().filter(|&m| m < w.table.mobys.len()) else { return };
    let ms = w.m(m).state;
    if ms == 0xfe || ms == 0xfd {
        w.delete_moby(id);
        return;
    }
    if ms < 7 {
        w.mm(id).position = w.m(m).position;
        return;
    }
    let k = c::pi32(w, id, 0x10) + 1;
    c::set_pi32(w, id, 0x10, k);
    let Some(p) = path else { return };
    if w.m(id).state == 1 {
        let pts: Vec<[f32; 4]> = w.svc.splines[p].iter().map(|q| q.map(f32::from_bits)).collect();
        let mp = w.m(m).position;
        if let Some((q, cur)) = crate::spline::nearest(&pts, false, 100.0, 1.0, 0.0, [mp[0], mp[1], mp[2]]) {
            if c::pi32(w, id, 8) < cur.seg || c::pf(w, id, 0xc) < cur.t || k < 0x1e {
                c::set_pi32(w, id, 8, cur.seg);
                c::set_pf(w, id, 0xc, cur.t);
                let m4 = w.m(id).position[3];
                w.mm(id).position = [q[0], q[1], q[2], m4];
                return;
            }
        }
        w.mm(id).state = 2;
    }
    if w.m(id).state == 2 {
        let n = w.svc.splines[p].len() as i32;
        let seg = c::pi32(w, id, 8);
        if seg < n - 1 {
            let (q, s) = path_step(w, c::SPEED * 0.25, c::pos(w, id), p, seg, 1);
            w.mm(id).position = q;
            c::set_pi32(w, id, 8, s);
            let d = c::dist3(q, pt(w, p, s));
            c::set_pf(w, id, 0xc, d);
        } else {
            w.mm(id).state = 3;
        }
    }
}

/// Level13 `0x30bdf0`: 1558 (module doc).
pub fn thrusters_update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
        }
        1 => {
            if w.svc.game_mode != 2 { return; }
            let Some(scene) = w.svc.cinematic.scene.clone() else { return };
            if scene.id != 2 { return; }
            if let Some(a) = scene.actors.get(2) { crate::moby_update::classes::cutscene_fx::infobot_thrusters(w, a); }
        }
        _ => {}
    }
}
