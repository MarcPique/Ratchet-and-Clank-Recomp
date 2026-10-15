//! **Hoven's helicopters, class 336** (level12 `0x2ebea8`, its hits `0x2ebca8`; census U430; 11 placed in groups
//! 62–66). Flyers circling their path (+0x74) at 30 a second from their start node (+0xa0), banking into the turns
//! (the turn between the last and the next leg over the leg's length · 20, eased by a twentieth a tick), nosed down
//! 30°, yaw eased toward the travel; at the path's last node they jump back to its first. A hit (0xa30000) blows one
//! up (a beam explosion, 31..74 bolts) and hides it for `ticks(500)`; it comes back only out of view, and stays hidden
//! and untouchable out of view while the carrier's battle (0x1619b4, `hoven_carrier`) is on. The fifth hit outside
//! state 0x32 earns skill point 0x15 (0x13d41d).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2ebea8` | 0: with a path: the chords (`FUN_0028b410`), at node +0xa0 (≤ the last), the damage record's bytes, → 1, draw distance 0xaa, targetable; 1: the flight (module doc) | [`update`] (`flyer::chord_lengths`) |
//! | `0x2ebca8` | the hits: the skill-point count (0x15ee04), `BoltBurst(m, 0x1f, 0x4a, 2, −1)`, +0xa4 `ticks(500)`, `SpawnBeamExplosion(0, 0, 4, 2, 100000, 3, 15, m, 0, pos, 20, 3, 4, −1, t3, 1, −1, 0)`; the battle and out of view → hidden; the timer or hidden: out of view with the timer out and no battle → shown, targetable, collision; else hidden | [`hits`] |
//!
//! Read from the level12 decomp and disassembly (the explosion's stack arguments). [L] The explosion's shake flag is
//! whatever `t3` holds after `BoltBurst` (no write before the call): no shake. Native `f32`.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::bolt_burst;
use crate::moby_update::classes::flyer;
use crate::moby_update::creature::{self as c, fx};
use crate::moby_update::services::World;
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 12;
pub const UPDATE_FN: u32 = 0x2e_bea8;
pub const CLASSES: [i16; 1] = [336];

const DT: f32 = c::DT;
/// The carrier's battle word (gp−0x524c).
const BATTLE: u32 = 0x16_19b4;
/// The hit count toward the skill point (gp−0x7dfc).
const HITS: u32 = 0x15_ee04;
/// Skill point 0x13d41d.
const SKILL: usize = 0x15;
/// gp−0x5258 / −0x5254 / −0x5250: the speed, the bank gain, the easing.
const SPEED: f32 = 30.0;
const BANK: f32 = 20.0;
const EASE: f32 = 20.0;

mod pv {
    pub const NODE: usize = 0x60;
    pub const STEP: usize = 0x64;
    pub const INIT: usize = 0x65;
    pub const PATH: usize = 0x74;
    pub const START: usize = 0xa0;
    pub const HIDE_T: usize = 0xa4;
    pub const SIZE: usize = 0xa8;
}

const BOOM: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 100_000.0, scale: 3.0, light: 15.0, streaks: 0x14, sparks: 3, puffs: 4, debris: 1, sound: -1, shake: false };

fn path(w: &World, id: MobyId) -> Option<usize> { usize::try_from(c::pi32(w, id, pv::PATH)).ok().filter(|&p| p < w.svc.splines.len()) }
fn point(w: &World, p: usize, i: usize) -> c::V { w.svc.splines[p].get(i).map_or([0.0; 4], |q| q.map(f32::from_bits)) }

/// `0x2ebca8(m, off)`: the hits and the hiding (module doc).
fn hits(w: &mut World, id: MobyId, off: bool) {
    if w.get_hit(id, 0xa3_0000, false).is_some() {
        if w.hero.state != 0x32 {
            let n = w.svc.units.word(HITS) + 1;
            w.svc.units.set_word(HITS, n);
            if 4 < n as i32 && !story::skill_point(w, SKILL) {
                story::set_skill_point(w, SKILL);
                w.play_level_sound(story::SKILL_SOUND, 0, None);
                crate::cinematic::show_banner(w, story::SKILL_BANNER, -1);
            }
        }
        bolt_burst(w, id, 0x1f, 0x4a, 2, -1);
        let t = w.ticks(500);
        c::set_pi32(w, id, pv::HIDE_T, t);
        let p = c::pos(w, id);
        fx::beam_explosion(w, &BOOM, Some(id), p);
    }
    w.mm(id).hit_slot = 0xff;
    let battle = w.svc.units.word(BATTLE) != 0;
    if battle && off {
        let m = w.mm(id);
        m.has_collision = false;
        m.mode = (m.mode & !mode::TARGETABLE) | 0x41;
    }
    if c::pi32(w, id, pv::HIDE_T) == 0 && w.m(id).mode & 1 == 0 { return; }
    if c::dec_timer_pvar_i32(w, id, pv::HIDE_T) != 0 && off && !battle {
        let coll = super::class_collision(w, w.m(id).o_class);
        let m = w.mm(id);
        m.mode = (m.mode & !0x41) | mode::TARGETABLE;
        m.has_collision = coll;
        return;
    }
    let m = w.mm(id);
    m.has_collision = false;
    m.mode = (m.mode & !mode::TARGETABLE) | 0x41;
}

/// Level12 `0x2ebea8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    let off = flyer::culled(w, id);
    hits(w, id, off);
    match w.m(id).state {
        0 => {
            let Some(p) = path(w, id) else { return };
            {
                let pv_ = &mut w.mm(id).pvars;
                pv_[pv::INIT] = 0;
                crate::moby_update::services::pvar::set_i32(pv_, 0x68, 0);
            }
            let n = w.svc.splines[p].len() as i32;
            flyer::chord_lengths(w, &flyer::Path { spline: p, looped: true, count: n });
            let i = c::pi32(w, id, pv::START).min(n - 1).max(0);
            w.mm(id).position = point(w, p, i as usize);
            c::set_pi32(w, id, pv::NODE, i);
            c::set_pu8(w, id, 0x2a, 0x10);
            c::set_pu8(w, id, 0x28, 2);
            c::set_pu8(w, id, 0x2c, 0x2c);
            c::set_pf(w, id, 0x30, f32::from_bits(0x3fb3_3333));
            c::set_pu8(w, id, 0x2b, 1);
            let m = w.mm(id);
            m.state = 1;
            m.draw_dist = 0xaa;
            m.mode |= mode::TARGETABLE;
        }
        1 => {
            let Some(p) = path(w, id) else { return };
            let n = w.svc.splines[p].len() as i32;
            if n == 0 { return; }
            let i = c::pi32(w, id, pv::NODE);
            let s = c::pu8(w, id, pv::STEP) as i8 as i32;
            let prev = ((i + n) - s).rem_euclid(n) as usize;
            let next = (i + n + s).rem_euclid(n) as usize;
            let (a, b, q) = (point(w, p, prev), point(w, p, i.max(0) as usize), point(w, p, next));
            let e = c::sub(b, a);
            let d = c::sub(q, b);
            let turn = c::sub_rot(c::atan(e[0], e[1]), c::atan(d[0], d[1]));
            let bank = c::sub_rot((turn / c::len3(e)) * BANK, w.m(id).rotation[0]);
            let rx = c::add_rot(w.m(id).rotation[0], bank / EASE);
            w.mm(id).rotation[0] = rx;
            if c::dist3(c::pos(w, id), q) < SPEED * DT {
                c::set_pi32(w, id, pv::NODE, next as i32);
                if next as i32 == n - 1 { w.mm(id).position = point(w, p, 0); }
            }
            let v = c::set_len3(c::sub(q, c::pos(w, id)), SPEED * DT);
            let pos = c::add(c::pos(w, id), v);
            let yaw = c::yaw(w, id);
            let dy = c::sub_rot(c::atan(v[0], v[1]), yaw);
            let m = w.mm(id);
            m.position = pos;
            m.rotation[2] = c::add_rot(yaw, dy / EASE);
            m.rotation[1] = f32::from_bits(0x3f06_0a92);
        }
        _ => {}
    }
}
