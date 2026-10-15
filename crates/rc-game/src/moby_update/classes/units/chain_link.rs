//! Gaspar's chain links, class 1181 (124 instances): level09 0x304360 with its private search 0x3041d0 (census unit
//! U303). A link hangs between its two neighbours in a chain whose ends are the classes 1172 and 1184; when one link is
//! told to break (+0xbc = 1) it sags for 10 ticks, tells both neighbours to break and explodes, so the break runs down
//! the chain. Read from the level09 decomp and disassembly. Native `f32`.
//!
//! **Pvar block** (P, bytes): +0x60 i32 the previous link (moby index, −1 none), +0x64 i32 the next link, +0x68 u8
//! "settled", +0x69 s8 a global flag (0x13d388 + 0x39 + it; −1 none), +0x6a s16 the break timer, +0x6c i32 a moby to
//! tell to break too.
//!
//! * **0** init: when the flag byte is set (`0x13d3c1[b]`) the link is deleted. Without a previous link: the nearest
//!   chain moby (1181, 1172, 1184, self excluded) to this link's joint-0 point (`FUN_002645a8(m, 0)`); without a next
//!   link: the chain moby whose joint-0 point is nearest to this link's position (the search 0x3041d0: nearest within
//!   0.5, or a 1184 within 10; none: the game's debug print). → **1**.
//! * **1** hanging: undrawn or over 32 from the camera, the link runs only on the ticks with `counter % 8 == spawn id
//!   % 8`. The previous / next link (a deleted one is none). A missing next link, or a next link not settled, unsettles
//!   this one. Unsettled with a previous link and a next 1181: when the next link is unsettled too, it is pulled
//!   toward both (each pull `(|d| − 1.3)·0.7` along `d`) and falls `dt²·9.8`; then it turns toward the previous link
//!   (pitch +0x44 toward `−atan(|d|xy, dz)`, yaw +0x48 toward `atan(dx, dy)`, 10 % a tick). Otherwise settled. Then,
//!   told to break (+0xbc = 1): the +0x6c moby, when in state 1, is told too; timer `ticks(10)`, update distance 0xff,
//!   → **2**.
//! * **2** breaking: update distance 0xff; toward the previous link `(|d| − 1.3)` along `d`, falling `2·dt²·9.8 ·
//!   (ticks(10) − (timer − 3))`, turning 50 % a tick. When the timer ends: the previous and next link (1172, 1181 or
//!   1184) are told to break (+0xbc = 1, update distance 0xff); → **3**.
//! * **3**: `SpawnBeamExplosion(0, 0, 2, 1, 10, 1, 10, link, pos, streaks 10, sparks 5, puffs 8, class sound 0, no
//!   shake, debris 1, −1, 0)`, then `DeleteMoby`.
//!
//! **Inferred [L]**: the moby list the search walks (0x15ffe4) is the table in index order (as for the drones' and the
//! R.C. range's searches); the camera 0x166f40 (this level's camera position) is `World::camera`.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, fx, DT2};
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x30_4360;
pub const REFERENCE_LEVEL: u32 = 9;
pub const LINK: i16 = 1181;
pub const CLASSES: [i16; 1] = [LINK];
/// The chain's other members (the search's and the break's class filter).
pub const END_A: i16 = 1172;
pub const END_B: i16 = 1184;
/// The classes whose joint-0 points the search reads.
pub const JOINTS: [i16; 3] = [LINK, END_A, END_B];

pub mod pv_ {
    pub const PREV: usize = 0x60;
    pub const NEXT: usize = 0x64;
    pub const SETTLED: usize = 0x68;
    pub const FLAG: usize = 0x69;
    pub const TIMER: usize = 0x6a;
    pub const ALSO: usize = 0x6c;
    pub const LEN: usize = 0x70;
}
use pv_ as o;

/// The rest length `1.3` (0x3fa66666) and the hang stiffness `0.7` (0x3f333333).
const REST: f32 = 1.3;
const PULL: f32 = 0.7;
const G: f32 = 9.8;

fn chain(oc: i16) -> bool { oc == LINK || oc == END_A || oc == END_B }

/// A pvar moby index as a live moby (−1, out of the table or deleted: none).
fn link(w: &World, i: i32) -> Option<MobyId> {
    let k = usize::try_from(i).ok()?;
    let m = w.table.mobys.get(k)?;
    (m.state != 0xfe && m.state != 0xfd).then_some(k)
}

/// Level09 0x3041d0: the chain moby nearest to `q` (by its position, or by its joint `joint`'s point), self excluded:
/// within 0.5, or a 1184 within 10.
pub fn search(w: &World, id: MobyId, q: c::V, joint: Option<usize>) -> Option<MobyId> {
    let mut best = (1e9f32, None);
    for (k, m) in w.table.mobys.iter().enumerate() {
        if k == id || m.state == 0xfe || m.state == 0xfd || m.state == 0xff || !chain(m.o_class) { continue; }
        let p = match joint { Some(j) => w.joint_point(k, j), None => m.position };
        let d = c::dist3(q, p);
        if d < best.0 { best = (d, Some(k)); }
    }
    let (d, k) = best;
    let k = k?;
    if d < 0.5 || (w.m(k).o_class == END_B && d < 10.0) { Some(k) } else { None }
}

/// Eases pitch (+0x44) and yaw (+0x48) toward `to` by `f` of the difference.
fn turn_toward(w: &mut World, id: MobyId, to: c::V, f: f32) {
    let p = c::pos(w, id);
    let d2 = c::dist2(p, to);
    let a = c::atan(d2, to[2] - p[2]);
    let m = w.mm(id);
    m.rotation[1] = c::add_rot(m.rotation[1], c::sub_rot(-a, m.rotation[1]) * f);
    let a = c::atan(to[0] - p[0], to[1] - p[1]);
    m.rotation[2] = c::add_rot(m.rotation[2], c::sub_rot(a, m.rotation[2]) * f);
}

/// `(|d| − 1.3)·k` along `d = to − p` (`FastVecNormalize`).
fn pull(p: c::V, to: c::V, k: f32) -> c::V {
    let d = c::sub(to, p);
    c::set_len3(d, (c::len3(d) - REST) * k)
}

/// Tells a chain moby to break (+0xbc = 1, update distance 0xff).
fn tell(w: &mut World, k: Option<MobyId>) {
    if let Some(k) = k {
        if chain(w.m(k).o_class) {
            let m = w.mm(k);
            m.cmd = 1;
            m.update_dist = 0xff;
        }
    }
}

/// Level09 0x304360 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < o::LEN { return; }
    match w.m(id).state {
        0 => {
            let b = c::pu8(w, id, o::FLAG) as i8;
            if b != -1 {
                let set = w.svc.interact.game.flags.get((b as i32 + 0x39) as usize).is_some_and(|&f| f != 0);
                if set {
                    w.delete_moby(id);
                    return;
                }
            }
            if c::pi32(w, id, o::PREV) == -1 {
                let q = w.joint_point(id, 0);
                match search(w, id, q, None) {
                    Some(k) => c::set_pi32(w, id, o::PREV, k as i32),
                    None => w.svc.unported("1181 chain link: no previous link (the game's debug print)"),
                }
            }
            if c::pi32(w, id, o::NEXT) == -1 {
                let q = c::pos(w, id);
                match search(w, id, q, Some(0)) {
                    Some(k) => c::set_pi32(w, id, o::NEXT, k as i32),
                    None => w.svc.unported("1181 chain link: no next link (the game's debug print)"),
                }
            }
            w.mm(id).state = 1;
        }
        1 => hang(w, id),
        2 => breaking(w, id),
        3 => {
            let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 10.0, scale: 1.0, light: 10.0, streaks: 10, sparks: 5, puffs: 8, debris: 1, sound: 0, shake: false };
            let p = c::pos(w, id);
            fx::beam_explosion(w, &b, Some(id), p);
            w.delete_moby(id);
        }
        _ => {}
    }
}

/// State 1.
fn hang(w: &mut World, id: MobyId) {
    let far = w.m(id).visible == 0 || 32.0 < c::dist3(c::pos(w, id), w.camera.map(|x| f32::from_bits(x.0)));
    if far {
        let sid = w.m(id).spawn_id as i32;
        if (w.counter as i32) % 8 != sid % 8 { return; }
    }
    let prev = link(w, c::pi32(w, id, o::PREV));
    let mut next = None;
    let ni = c::pi32(w, id, o::NEXT);
    if ni != -1 {
        next = link(w, ni);
        match next {
            None => c::set_pu8(w, id, o::SETTLED, 0),
            Some(n) => {
                if w.m(n).pvars.get(o::SETTLED).copied().unwrap_or(0) == 0 { c::set_pu8(w, id, o::SETTLED, 0); }
            }
        }
    }
    let mut settled = true;
    if c::pu8(w, id, o::SETTLED) == 0 {
        if let (Some(p), Some(n)) = (prev, next) {
            if w.m(n).o_class == LINK {
                settled = false;
                let to = c::pos(w, p);
                if w.m(n).pvars.get(o::SETTLED).copied().unwrap_or(0) == 0 {
                    let me = c::pos(w, id);
                    let mut v = pull(me, to, PULL);
                    let n_alive = w.m(n).o_class == LINK && w.m(n).state != 0xfe && w.m(n).state != 0xfd;
                    if n_alive { v = c::add(v, pull(me, c::pos(w, n), PULL)); }
                    v[2] -= DT2 * G;
                    w.mm(id).position = c::add(me, v);
                }
                turn_toward(w, id, to, 0.1);
            }
        }
    }
    if settled { c::set_pu8(w, id, o::SETTLED, 1); }
    if w.m(id).cmd != 1 { return; }
    if let Some(k) = link(w, c::pi32(w, id, o::ALSO)) {
        if w.m(k).state == 1 { w.mm(k).cmd = 1; }
    }
    let t = w.ticks(10);
    c::set_pi16(w, id, o::TIMER, t as i16);
    let m = w.mm(id);
    m.state = 2;
    m.update_dist = 0xff;
}

/// State 2.
fn breaking(w: &mut World, id: MobyId) {
    w.mm(id).update_dist = 0xff;
    if let Some(p) = link(w, c::pi32(w, id, o::PREV)) {
        let to = c::pos(w, p);
        let me = c::pos(w, id);
        let mut v = pull(me, to, 1.0);
        let k = w.ticks(10) - (c::pi16(w, id, o::TIMER) as i32 - 3);
        v[2] -= (DT2 * G + DT2 * G) * k as f32;
        w.mm(id).position = c::add(me, v);
        turn_toward(w, id, to, 0.5);
    }
    if c::dec_timer_pvar_s16(w, id, o::TIMER) == 0 { return; }
    let prev = link(w, c::pi32(w, id, o::PREV));
    tell(w, prev);
    let next = link(w, c::pi32(w, id, o::NEXT));
    tell(w, next);
    w.mm(id).state = 3;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};

    fn at(oc: i16, x: f32, prev: i32, next: i32) -> Moby {
        let mut m = Moby { o_class: oc, state: 0, position: [x, 0.0, 10.0, 1.0], visible: 1, pvars: vec![0; o::LEN], ..Moby::default() };
        crate::moby_update::services::pvar::set_i32(&mut m.pvars, o::PREV, prev);
        crate::moby_update::services::pvar::set_i32(&mut m.pvars, o::NEXT, next);
        m.pvars[o::FLAG] = 0xff;
        crate::moby_update::services::pvar::set_i32(&mut m.pvars, o::ALSO, -1);
        m
    }

    #[test]
    fn break_runs_down_the_chain() {
        // 1172 — link — link — 1184, spaced 1.3 (at rest).
        let ms = vec![at(END_A, 0.0, -1, -1), at(LINK, 1.3, 0, 2), at(LINK, 2.6, 1, 3), at(END_B, 3.9, -1, -1)];
        let mut t = MobyTable::new(ms, 8);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        for id in [1, 2] { update(&mut w, id); }
        assert_eq!((w.m(1).state, w.m(2).state), (1, 1));
        w.mm(1).cmd = 1;
        update(&mut w, 1);
        assert_eq!((w.m(1).state, w.m(1).update_dist), (2, 0xff));
        for _ in 0..10 { update(&mut w, 1); }
        assert_eq!(w.m(1).state, 3);
        assert_eq!((w.m(0).cmd, w.m(2).cmd), (1, 1), "both neighbours told");
        assert!(w.m(1).position[2] < 10.0, "sagged");
        update(&mut w, 1);
        assert!(w.m(1).state >= 0x80, "exploded and deleted");
        // The next link breaks in turn; its previous link is gone, so it only waits.
        update(&mut w, 2);
        assert_eq!(w.m(2).state, 2);
    }

    #[test]
    fn search_takes_the_nearest_within_half_a_unit() {
        let ms = vec![at(LINK, 0.0, -1, -1), at(LINK, 0.4, -1, -1), at(LINK, 0.3, -1, -1), at(END_B, 9.0, -1, -1)];
        let mut t = MobyTable::new(ms, 8);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        assert_eq!(search(&w, 0, [0.0, 0.0, 10.0, 1.0], None), Some(2));
        assert_eq!(search(&w, 3, [5.0, 0.0, 10.0, 1.0], None), None);
        // A 1184 within 10 is taken even when nothing is within 0.5.
        assert_eq!(search(&w, 0, [9.5 - 0.0, 0.0, 10.0, 1.0], None).map(|k| w.m(k).o_class), Some(END_B));
    }
}
