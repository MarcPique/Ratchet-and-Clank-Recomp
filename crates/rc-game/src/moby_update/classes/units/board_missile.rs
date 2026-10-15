//! **Kalebo III's board missile, class 1475** (level16 spawn `0x2a3e30` from the board weapon's fire, update
//! `0x2a3f38`; created only, so not in the census). Fired from Ratchet's board ([`crate::hero::hoverboard`]'s weapon,
//! item 0x24) at the racer in front of him, it speeds up, homes on the racer after a few ticks, trails smoke like the
//! Devastator's missile (the same trail code, [`super::super::devastator_missile::trail`], with its own constants) and
//! blows up on the first face or moby it meets. Read from the level16 decomp and disassembly. Native `f32`.
//!
//! **Pvars** (0x1c): +0x00 the owner (the board weapon's hand item; the port keeps Ratchet's moby + 1, as the
//! Devastator's missile does), +0x04 (s16) its life, +0x06 (s16) its age, +0x08 the target (moby + 1, 0 none), +0x10
//! the speed, +0x14 / +0x18 the yaw / pitch spring velocities.
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | `0x2a3e30` | `CreateMoby(0x5c3)`; distances 0xff, visible; rotation (0, pitch, yaw) = Ratchet's (0x13f3e4 / 0x13f3e8); the owner, the target, at the hand item's joint list 0; speed 21·dt, the spring velocities and age 0; scale × 2.3; life `ticks(300)` | [`spawn`] |
//! | state 0 | age + 1; the speed approaches 30·dt by 70·dt²; with a target, past `ticks(7)`: `SpringAngle` toward its position + 0.7 up (yaw; pitch = −atan(xy distance, dz)) by 0.03 / 0.3, at most 550°/s; the step `0x25e148(speed, yaw, −pitch)`; the trail (puffs alpha 0x7f grey 0x606060 for `ticks(60)` and `ticks(6)`, the spark 20000 for `ticks(5)`); the move; out of x, y, z ≥ 2 → deleted | [`update`] |
//! | state 0, the path | `CollLine_Fix(old, new, 0, Ratchet, tmpl)` (push along the step, 1, 5627.97; flags 0x830000; type 3 / 1; class 1475; damage 1): a face (+0x1c > 0) or a moby other than Ratchet and the owner → at the hit, state 1; Ratchet, the owner, a kind ≤ 0 face → on; nothing → its life out or more than 60 from Ratchet (xy) → deleted | [`update`] |
//! | state 1 | `SpawnBeamExplosion(0, 0, 1, 0.5, 4, 0.7, 7, m, …, pos, 3, 3, 5, 0, 0, 0, −1, 0)` (class sound 0); deleted | [`update`] |

use crate::hero::guns::{add3, with_len};
use crate::moby_runtime::MobyId;
use crate::moby_update::classes::devastator_missile::{trail, Trail};
use crate::moby_update::creature::{self as c, fx, turn};
use crate::moby_update::services::{self as sv, HitTemplate, World};
use crate::ps2v::Pf;
use crate::targeting::polar;

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2a_3f38;
pub const CLASS: i16 = 1475;
pub const CLASSES: [i16; 1] = [CLASS];

mod pv {
    pub const OWNER: usize = 0x00;
    pub const LIFE: usize = 0x04;
    pub const AGE: usize = 0x06;
    pub const TARGET: usize = 0x08;
    pub const SPEED: usize = 0x10;
    pub const YAW_VEL: usize = 0x14;
    pub const PITCH_VEL: usize = 0x18;
    pub const LEN: usize = 0x1c;
}

const DT: f32 = 1.0 / 60.0;
const TRAIL: Trail = Trail { alpha: 0x7f, rgb: 0x60_6060, life: 60, life2: 6, spark: 20000.0, spark_life: 5 };
const BLAST: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 1.0, flash2: 0.5, flash_dist: 4.0, scale: 0.7, light: 7.0, streaks: 3, sparks: 3, puffs: 5, debris: 0, sound: 0, shake: false };

/// Level16 `0x2a3e30(yaw, pitch, owner, &pos, target)` (module table), from the hand item's update with the table.
#[allow(clippy::too_many_arguments)]
pub fn spawn(table: &mut crate::moby_runtime::MobyTable, hits: &mut dyn crate::hero::items::HitSink, frame: u64, rot: (f32, f32), owner: MobyId, pos: [f32; 3], target: Option<MobyId>) -> Option<MobyId> {
    let id = hits.create_moby(table, CLASS, frame)?;
    let life = crate::hero::physics::ticks(300);
    let m = &mut table.mobys[id];
    if m.pvars.len() < pv::LEN { m.pvars.resize(pv::LEN, 0); }
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
    m.visible = 1;
    m.rotation = [0.0, rot.1, rot.0, 0.0];
    m.state = 0;
    let p = &mut m.pvars;
    sv::pvar::set_i32(p, pv::OWNER, owner as i32 + 1);
    m.position = [pos[0], pos[1], pos[2], m.position[3]];
    sv::pvar::set_i32(&mut m.pvars, pv::TARGET, target.map_or(0, |t| t as i32 + 1));
    sv::pvar::set_ff(&mut m.pvars, pv::YAW_VEL, 0.0);
    sv::pvar::set_ff(&mut m.pvars, pv::PITCH_VEL, 0.0);
    sv::pvar::set_i16(&mut m.pvars, pv::AGE, 0);
    sv::pvar::set_ff(&mut m.pvars, pv::SPEED, DT * 30.0 * 0.7);
    m.scale *= 2.3;
    sv::pvar::set_i16(&mut m.pvars, pv::LIFE, life as i16);
    let r = rc_formats::moby_light::rotation_rows([0.0, rot.1, rot.0]);
    for (row, src) in m.rows.iter_mut().zip(r) { *row = [f32::from_bits(src[0]), f32::from_bits(src[1]), f32::from_bits(src[2]), 0.0]; }
    Some(id)
}

/// Level16 `0x2a3f38` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, pv::LEN);
    match w.m(id).state {
        0 => fly(w, id),
        1 => {
            let p = c::pos(w, id);
            fx::beam_explosion(w, &BLAST, Some(id), p);
            w.delete_moby(id);
        }
        _ => {}
    }
}

fn fly(w: &mut World, id: MobyId) {
    let age = c::pi16(w, id, pv::AGE).wrapping_add(1);
    c::set_pi16(w, id, pv::AGE, age);
    let mut speed = c::pf(w, id, pv::SPEED);
    turn::approach(DT * 30.0, DT * DT * 70.0, &mut speed);
    c::set_pf(w, id, pv::SPEED, speed);
    let target = usize::try_from(c::pi32(w, id, pv::TARGET) - 1).ok().filter(|&t| t < w.table.mobys.len());
    if let Some(t) = target {
        if w.ticks(7) < age as i32 {
            let tp = w.m(t).position;
            let aim = [tp[0], tp[1], tp[2] + 0.7];
            let p = c::pos(w, id);
            let yaw_to = c::atan(aim[0] - p[0], aim[1] - p[1]);
            let d = ((p[0] - aim[0]).powi(2) + (p[1] - aim[1]).powi(2)).sqrt();
            let pitch_to = -c::atan(d, aim[2] - p[2]);
            let max = Pf::f(DT * 9.599_311);
            let (k, dd) = (Pf::b(0x3cf5_c28f), Pf::b(0x3e99_999a));
            let (mut a, mut v) = (Pf::f(w.m(id).rotation[2]), Pf::f(c::pf(w, id, pv::YAW_VEL)));
            crate::hero::physics::turn_spring(Pf::f(yaw_to), k, dd, max, &mut a, &mut v, 0);
            w.mm(id).rotation[2] = a.to_f32();
            c::set_pf(w, id, pv::YAW_VEL, v.to_f32());
            let (mut a, mut v) = (Pf::f(w.m(id).rotation[1]), Pf::f(c::pf(w, id, pv::PITCH_VEL)));
            crate::hero::physics::turn_spring(Pf::f(pitch_to), k, dd, max, &mut a, &mut v, 0);
            w.mm(id).rotation[1] = a.to_f32();
            c::set_pf(w, id, pv::PITCH_VEL, v.to_f32());
        }
    }
    let (yaw, pitch) = (w.m(id).rotation[2], w.m(id).rotation[1]);
    let step = polar(speed, yaw, -pitch);
    trail(w, id, step, speed, &TRAIL);
    let p = c::pos(w, id);
    let old = [p[0], p[1], p[2]];
    let pos = add3(old, step);
    { let m = w.mm(id); m.position = [pos[0], pos[1], pos[2], m.position[3]]; }
    if pos[0] < 2.0 || pos[1] < 2.0 || pos[2] < 2.0 {
        w.delete_moby(id);
        return;
    }
    let n = with_len(step, 1.0);
    let tmpl = HitTemplate { dir: [Pf::f(n[0]), Pf::f(n[1]), Pf::ONE, Pf::b(0x45af_df66)], attacker: Some(id), flags: 0x83_0000, b18: 3, b19: 1, h1a: CLASS as u16, damage: Pf::ONE, w20: 1 };
    let f4 = |a: [f32; 3]| [Pf::f(a[0]), Pf::f(a[1]), Pf::f(a[2]), Pf::ZERO];
    let owner = usize::try_from(c::pi32(w, id, pv::OWNER) - 1).ok();
    match sv::line_hit_in(w.table, w.svc, w.classes, w.coll, f4(old), f4(pos), 0, w.hero_moby, &tmpl) {
        None => {
            let mut life = c::pi16(w, id, pv::LIFE);
            let out = sv::fast_dec_timer_s16(&mut life) != 0;
            c::set_pi16(w, id, pv::LIFE, life);
            let hero = w.hero.pos.map(|x| f32::from_bits(x.0));
            let d = ((pos[0] - hero[0]).powi(2) + (pos[1] - hero[1]).powi(2)).sqrt();
            if out || 60.0 < d { w.delete_moby(id); }
        }
        Some(h) => {
            let explode = match h.moby {
                None => 0 < h.kind,
                Some(m) => Some(m) != w.hero_moby && Some(m) != owner,
            };
            if explode {
                let m = w.mm(id);
                m.position = [h.point[0], h.point[1], h.point[2], m.position[3]];
                m.state = 1;
            }
        }
    }
}
