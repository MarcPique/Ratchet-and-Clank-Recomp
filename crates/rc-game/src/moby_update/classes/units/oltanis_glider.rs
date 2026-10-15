//! **Oltanis's pull-target gliders, class 557** (level14 `0x2e7bd8`; census U497, 4 placed). Flyers on the shared
//! path driver (`FlyerPathDriver`, [`crate::moby_update::classes::flyer::driver`]) that carry a Swingshot pull target
//! (the moby +0x150, at +0x160 turned by the glider, from joint list +0x154 or its own position when that is −1, with
//! +0x158 = 12345), spin a part (joint list 0, 10° a tick about x) and carry riders (+0x180). Read from the level14
//! decomp; native `f32`.
//!
//! * Hidden (with its target, not targetable) while Ratchet is in its hiding cuboid (+0x21c, if any); shown again
//!   (the target targetable) when he leaves it.
//! * 0: at its path's start, the manipulator on list 0 (+0x1c0), its base speed (+0x214 = +0xfc); no target: deleted.
//! * Its hum (sound 0, flags 4) once.
//! * Ratchet pulling himself to its target (group 0xd): it slows from its base speed to 0.005 over `ticks(60)`;
//!   still in the air and less than 3 below it after that, he is drawn under the target (his platform push toward it,
//!   at most 3·dt a tick) and it keeps slowing; once he lands or drops 3 below, it speeds back up to its base over
//!   `ticks(90)`.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2e7bd8` | 557 | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::flyer;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::{euler_rows, pv, World};
use crate::moby_update::{manip, story, triggers};

pub const REFERENCE_LEVEL: u32 = 14;
pub const UPDATE_FN: u32 = 0x2e_7bd8;
pub const CLASSES: [i16; 1] = [557];

mod o {
    pub const PATH: usize = 0x74;
    pub const SPEED: usize = 0xfc;
    pub const TARGET: usize = 0x150;
    pub const LIST: usize = 0x154;
    pub const CARRY: usize = 0x158;
    pub const VOICE: usize = 0x15c;
    pub const OFFSET: usize = 0x160;
    pub const RIDERS: usize = 0x180;
    pub const NODE: usize = 0x1c0;
    pub const SPIN: usize = 0x200;
    pub const HOLD: usize = 0x204;
    pub const SPARE: usize = 0x206;
    pub const TIMER: usize = 0x20c;
    pub const INV: usize = 0x210;
    pub const BASE: usize = 0x214;
    pub const RELEASED: usize = 0x218;
    pub const CUBOID: usize = 0x21c;
    pub const SIZE: usize = 0x220;
}
/// Level14 gp words (0x161b88..0x161b90).
const DRAW_IN: f32 = 3.0;
const SLOW_TICKS: i32 = 60;
const FAST_TICKS: i32 = 90;

fn target(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, o::TARGET)).ok().filter(|&m| m < w.table.mobys.len()) }

/// Level14 `0x2e7bd8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    let cub = c::pi32(w, id, o::CUBOID);
    if 0 <= cub && w.in_cuboid(w.hero_point(), cub) {
        let m = w.mm(id);
        m.has_collision = false;
        m.mode |= 0x41;
        if let Some(t) = target(w, id) {
            let tm = w.mm(t);
            tm.has_collision = false;
            tm.mode = tm.mode & 0xefff | 0x41;
        }
        return;
    }
    if !w.m(id).has_collision {
        let coll = super::class_collision(w, w.m(id).o_class);
        let m = w.mm(id);
        m.mode &= 0xffbe;
        m.has_collision = coll;
        if let Some(t) = target(w, id) {
            let tc = super::class_collision(w, w.m(t).o_class);
            let tm = w.mm(t);
            tm.mode = tm.mode & 0xffbe | 0x1000;
            tm.has_collision = tc;
        }
    }
    let (old, old_rot) = (c::pos(w, id), w.m(id).rotation);
    if w.m(id).state == 0 {
        if let Some(p0) = usize::try_from(c::pi32(w, id, o::PATH)).ok().and_then(|i| w.svc.splines.get(i)).and_then(|s| s.first()) {
            w.mm(id).position = p0.map(f32::from_bits);
        }
        c::set_pf(w, id, o::SPIN, 0.0);
        c::set_pi32(w, id, o::VOICE, -1);
        c::set_pi16(w, id, o::SPARE, 0);
        manip::attach(w, id, 0, id, o::NODE);
        if c::pi32(w, id, o::TARGET) == -1 {
            w.delete_moby(id);
            return;
        }
        c::set_pi16(w, id, o::HOLD, 0);
        c::set_pf(w, id, o::BASE, c::pf(w, id, o::SPEED));
    }
    flyer::driver(w, id);
    if !w.table.mobys.get(id).is_some_and(|m| m.state < 0xfd) { return; }
    let tgt = target(w, id);
    if c::pi32(w, id, o::CARRY) == 0x3039 {
        let list = c::pi32(w, id, o::LIST);
        let base = if list == -1 { c::pos(w, id) } else { w.joint_point(id, list as usize) };
        let r = euler_rows(pv(w.m(id).rotation)).map(|row| row.map(|x| f32::from_bits(x.0)));
        let v = c::pv4(w, id, o::OFFSET);
        let off: [f32; 4] = std::array::from_fn(|k| if k == 3 { 0.0 } else { r[0][k] * v[0] + r[1][k] * v[1] + r[2][k] * v[2] });
        if let Some(t) = tgt { w.mm(t).position = c::add(base, off); }
    }
    if c::pi32(w, id, o::VOICE) == -1 {
        let v = w.play_sound(0, 4, id);
        c::set_pi32(w, id, o::VOICE, v);
    }
    let a = c::add_rot(c::pf(w, id, o::SPIN), f32::from_bits(0x3e32_b8c2));
    c::set_pf(w, id, o::SPIN, a);
    manip::set_axis(w, id, id, o::NODE, a, 0);
    let pos = c::pos(w, id);
    let delta = [pos[0] - old[0], pos[1] - old[1], pos[2] - old[2], pos[3]];
    let rot = w.m(id).rotation;
    triggers::carry_riders(&mut w.mm(id).pvars, o::RIDERS, delta, old_rot, rot);
    let pulling = w.hero.group == 0xd && tgt.is_some() && w.hero.swing.pull == tgt;
    if pulling {
        if c::pi16(w, id, o::HOLD) == 0 {
            c::set_pi16(w, id, o::HOLD, 1);
            let t = w.ticks(SLOW_TICKS);
            c::set_pi16(w, id, o::TIMER, t as i16);
            c::set_pf(w, id, o::INV, 1.0 / t as f32);
        }
    } else if c::pi16(w, id, o::HOLD) == 1 {
        let h = super::hero_pos(w);
        if w.hero.air_ticks == 0 || h[2] <= pos[2] - 3.0 {
            c::set_pi16(w, id, o::HOLD, 0);
            let t = w.ticks(FAST_TICKS);
            c::set_pi16(w, id, o::TIMER, t as i16);
            c::set_pf(w, id, o::RELEASED, c::pf(w, id, o::SPEED));
            c::set_pf(w, id, o::INV, 1.0 / t as f32);
        } else if let Some(t) = tgt {
            let tp = c::pos(w, t);
            let d = [tp[0] - h[0], tp[1] - h[1], 0.0, 0.0];
            let l = c::len3(d).min(DRAW_IN * DT);
            let v = c::set_len3(d, l);
            let f = w.hero_fields_mut();
            f.platform = [v[0], v[1], v[2], f.platform[3]];
        }
    }
    c::dec_timer_pvar_s16(w, id, o::TIMER);
    let k = c::pi16(w, id, o::TIMER) as f32 * c::pf(w, id, o::INV);
    let s = if c::pi16(w, id, o::HOLD) == 0 {
        c::pf(w, id, o::BASE) + (c::pf(w, id, o::RELEASED) - c::pf(w, id, o::BASE)) * k
    } else {
        (c::pf(w, id, o::BASE) - 0.005) * k + 0.005
    };
    c::set_pf(w, id, o::SPEED, s);
}
