//! **The hoverboard course's roaming boost pickups, class 133** (level05 `0x2dac80`, census U171, 20 placed on Rilgar's
//! course). Each walks the ground (the walker of `creature::walker`) toward a point of its path, a new one
//! when it gets there or every ten seconds; Ratchet hitting it on his board (the board's hit sphere, flags 0x10000:
//! `crate::hero::hoverboard`) bursts it and gives him ticks(60) of boost; it comes back once he is 48 away. Read from
//! the level05 decomp. Native `f32`.
//!
//! **Pvars** (0xc0): +0x40 the step's move, +0x60 the walker's record, +0xb0 its path, +0xb4 the point it walks to,
//! +0xb8 the point's timer, +0xbc the turn's velocity.
//!
//! ## Coverage (`0x2dac80`)
//! | address | what | port |
//! |---|---|---|
//! | `0x2daf58` (every tick) | `MobyGetHitMessage(m, 0x330000)`: → 2, not drawn, no collision, hidden; the burst `0x2db238` along Ratchet's displacement (the amoeboids' goo burst: 20 clumps, size 0.5, `dir`·randf(0, 1)); `PlayClassSound(0, 0, m)`; hit by Ratchet (the attacker's class 0): 0x13fc14 += ticks(60); +0xa4 = 0xff | [`update`] (`fx::goo_burst_k`, `HeroFields::board_boost`) |
//! | state 0 | → 1, update distance 0x40, a sequence other than 0 blended to 0; a path with points: the walker seeded (`SeedJumpPattern(+0x60)`, J2 / J3 = 1, J0 = 409, J9 = 4·dt); else `DeleteMoby` | [`update`] |
//! | state 1 | the yaw turns to the point (`0x270cc0(atan, 60°·dt², 60°·dt², 180°·dt)`); one walker step toward (cos yaw, sin yaw, 0) (`0x26d9a8(1, m, +0x60, dir, +0x40)`); the point's timer out, or within 1 (xy) of it: the point + `randi(count)` (mod count), timer ticks(600); the goo drips `0x2db018` (size 0.5) | [`update`] (`walker::step`, `fx::goo_drips`) |
//! | state 2 | more than 48 (xy) from Ratchet: drawn, → 1, shown, its class collision back | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, fx, turn, walker};
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 5;
pub const UPDATE_FN: u32 = 0x2d_ac80;
pub const CLASSES: [i16; 1] = [133];

mod pv {
    pub const MOVE: usize = 0x40;
    pub const J: usize = 0x60;
    pub const PATH: usize = 0xb0;
    pub const POINT: usize = 0xb4;
    pub const TIMER: usize = 0xb8;
    pub const TURN_VEL: usize = 0xbc;
    pub const LEN: usize = 0xc0;
}

const DT: f32 = 1.0 / 60.0;
const DT2: f32 = 1.0 / 3600.0;

fn path(w: &World, id: MobyId) -> Vec<[f32; 4]> {
    usize::try_from(c::pi32(w, id, pv::PATH)).ok().and_then(|i| w.svc.splines.get(i)).map(|p| p.iter().map(|q| q.map(f32::from_bits)).collect()).unwrap_or_default()
}

/// Level05 `0x2dac80` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, pv::LEN);
    hit(w, id);
    match w.m(id).state {
        0 => {
            {
                let m = w.mm(id);
                m.state = 1;
                m.update_dist = 0x40;
            }
            if w.m(id).anim.seq_b != 0 { w.anim_blend(id, 0, 0, 0); }
            if path(w, id).is_empty() {
                w.delete_moby(id);
                return;
            }
            walker::seed(&mut w.mm(id).pvars, pv::J);
            c::set_pf(w, id, pv::J + 0x8, 1.0);
            c::set_pf(w, id, pv::J + 0xc, 1.0);
            c::set_pi32(w, id, pv::J, 409.6f32 as i32);
            c::set_pf(w, id, pv::J + 0x24, DT * 4.0);
        }
        1 => walk(w, id),
        2 => {
            let h = super::hero_pos(w);
            let p = c::pos(w, id);
            if ((p[0] - h[0]).powi(2) + (p[1] - h[1]).powi(2)).sqrt() <= 48.0 { return; }
            let coll = super::class_collision(w, w.m(id).o_class);
            let m = w.mm(id);
            m.visible = 1;
            m.state = 1;
            m.mode &= !1;
            m.has_collision = coll;
        }
        _ => {}
    }
}

/// `0x2daf58`: hit → burst, boost.
fn hit(w: &mut World, id: MobyId) {
    let Some(h) = w.get_hit(id, 0x33_0000, false) else { return };
    {
        let m = w.mm(id);
        m.state = 2;
        m.visible = 0;
        m.has_collision = false;
        m.mode |= 1;
    }
    let d = w.hero.disp.map(|x| x.to_f32());
    fx::goo_burst_k(w, id, d, 0.5, 1.0);
    w.play_sound(0, 0, id);
    if h.attacker.is_some_and(|a| a < w.table.mobys.len() && w.m(a).o_class == 0) {
        let t = w.ticks(0x3c);
        w.hero_fields_mut().board_boost += t;
    }
    w.mm(id).hit_slot = 0xff;
}

/// State 1: walk toward the path's point.
fn walk(w: &mut World, id: MobyId) {
    let pts = path(w, id);
    let n = pts.len() as i32;
    let k = c::pi32(w, id, pv::POINT).rem_euclid(n.max(1));
    let q = pts[k as usize];
    let p = c::pos(w, id);
    let to = c::atan(q[0] - p[0], q[1] - p[1]);
    let (mut yaw, mut v) = (c::yaw(w, id), c::pf(w, id, pv::TURN_VEL));
    turn::turn_toward(to, DT2 * std::f32::consts::FRAC_PI_3, DT2 * std::f32::consts::FRAC_PI_3, DT * std::f32::consts::PI, &mut yaw, &mut v);
    c::set_yaw(w, id, yaw);
    c::set_pf(w, id, pv::TURN_VEL, v);
    let dir = [yaw.cos(), yaw.sin(), 0.0, 0.0];
    let mut mv = [0, 1, 2, 3].map(|i| c::pf(w, id, pv::MOVE + 4 * i));
    walker::step(w, id, pv::J, 1.0, dir, &mut mv);
    for (i, &x) in mv.iter().enumerate() { c::set_pf(w, id, pv::MOVE + 4 * i, x); }
    let out = c::dec_timer_pvar_i32(w, id, pv::TIMER) != 0;
    let p = c::pos(w, id);
    if out || ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt() < 1.0 {
        let r = w.rng.randi(n);
        c::set_pi32(w, id, pv::POINT, (k + r) % n);
        let t = w.ticks(600);
        c::set_pi32(w, id, pv::TIMER, t);
    }
    fx::goo_drips(w, id, 0.5);
}
