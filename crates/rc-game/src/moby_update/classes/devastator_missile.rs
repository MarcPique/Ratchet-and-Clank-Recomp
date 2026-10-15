//! **The Devastator's missile** (class 153; level01 spawn `0x2c5440` from the Devastator's update, update `0x2c5b70`;
//! read from the decompiler output). The launcher is `crate::hero::devastator`.
//!
//! **Pvars** (the game's layout): +0x00 the target's aim point last tick, +0x10 the owner (the item; the port keeps
//! Ratchet's moby + 1), +0x14 its life (`ticks(300)`), +0x18 the target (moby + 1), +0x1c the target's aim height, +0x20
//! the speed, +0x24 / +0x28 the yaw / pitch turn rates, +0x2c / +0x30 the yaw / pitch it was fired along, +0x34 the top
//! speed (20 u/s, or the target record's +0x0c u/s), +0x38 leading the target (1; 0 when the record's +0x0b is set),
//! +0x3c / +0x3e the launcher-motion timer and its start (`ticks(60)` when Ratchet's platform moves, else
//! `ticks(15)`), +0x48 its range from Ratchet (70; 140 for a faster target). Moby +0xbc: explode now (1 a face, 2 a
//! moby).
//!
//! **Spawn** ([`spawn`], `0x2c5440(yaw, pitch, aim height, moby yaw, moby pitch, owner, muzzle, target)`):
//! `CreateMoby(0x99)`, distances 0xff, visible, rotation (0, moby pitch, moby yaw), at the muzzle, speed 20·dt/3, the
//! target's record +0x1e |= 0x80, life `ticks(300)`, twice the class scale; a line from the owner at the muzzle's
//! height to the muzzle that hits (flags 0, Ratchet ignored) starts it exploding there. (Its entry in the missile
//! list 0x1b0bf0 is the table itself in the port: `crate::hero::devastator::already_targeted`.)
//!
//! **Update** ([`update`]): the speed approaches the top speed by a tenth a tick; the launcher's motion (Ratchet's
//! displacement and platform motion, fading over +0x3c) is added to each step. After 5 ticks, with a live target:
//! toward its aim point, led by `super::missile::intercept_time` (when +0x38; the lead is dropped past `ticks(300)`),
//! else toward the pvar aim; spring turns `0x26cef0(2π·dt², π·dt²)` at most 360°/s with a target, 270°/s without. The
//! path is tested (`CollLine_Fix(old, new, 0, Ratchet, tmpl)`: push along the step, 1, 5627.97; flags 0x830000;
//! damage 3; type 3 / 1; class 153): nothing — its life out or past +0x48 from Ratchet (2D) → deleted without
//! exploding (the gold missile explodes instead); a face → explode (1); Ratchet → through; its owner →
//! ignored; another moby → explode (2). Out of the positive octant → deleted.
//!
//! **The explosion**: the mobys in a sphere of 2 (×(gold + 1)) are hit (`0x26f8f8`: damage 3, flags 0x830000, type
//! 3 / 1); 10 streaks (type 15, size 40000, 0x4f007fff → 0x1f00007f, 60..120 ticks: on a face spread in its plane
//! (a triangle edge turned a random angle about the normal), else random, `randf(8.5, 16.5)·dt` fast, rising 5·dt, plus
//! the face's reflection drift, at most 8·dt when longer than 1); a fireball toward the camera
//! (`super::bomb::fireball`, type 0); 3 pairs of rings (type 11; fewer and slower within 6 / 7 of the camera);
//! 10 smoke puffs (type 8, size 200000, 20..35 ticks); the flashes (`FlashSpawn` 0x2c20e0: two more beyond 9 from the
//! camera); the camera shake along up (`0.4 − 0.0175·d`, 0.05 beyond 20, 25 ticks); class sound 0; the explosion light
//! (template 0x20a9b0, the Bomb Glove's values); deleted. Heavy frame loads (above 0.9) thin it (a third of the
//! streaks and puffs, half the rings, shorter lives, no last flash, no light) and the trail (every other tick).
//!
//! The trail: [`trail`] (types 44 / 21). **The gold missile** (0x13e52b, item 11): its blast ×2 with the colours shifted
//! (`0x270fa8`), green flashes and the gold light (0x20aa00); at its life's end it explodes; after a blast it bounces off
//! and seeks a new target ([`retarget`], `0x2c5778`), up to three blasts while its timer +0x44 runs. [L] A blast without
//! a face bounces off (0, 0, 1) (the game reads the last collision's normal 0x174300). Native `f32`; the game's draws
//! in its order.

use crate::hero::guns::{add3, len3, reflect, scale3, sub3, with_len};
use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx;
use crate::moby_update::creature::turn::spring_turn;
use crate::moby_update::creature::atan;
use crate::moby_update::services::{self as sv, pvar as p, HitTemplate, World};
use crate::ps2v::Pf;
use crate::targeting::polar;
use std::f32::consts::{PI, TAU};

pub const UPDATE_FN: u32 = 0x2c5b70;
pub const CLASSES: [i16; 1] = [153];
pub const CLASS: i16 = 153;
const DT: f32 = 1.0 / 60.0;

pub mod pv {
    pub const AIM: usize = 0x00;
    pub const OWNER: usize = 0x10;
    pub const LIFE: usize = 0x14;
    pub const TARGET: usize = 0x18;
    pub const AIM_H: usize = 0x1c;
    pub const SPEED: usize = 0x20;
    pub const YAW_VEL: usize = 0x24;
    pub const PITCH_VEL: usize = 0x28;
    pub const YAW: usize = 0x2c;
    pub const PITCH: usize = 0x30;
    pub const TOP: usize = 0x34;
    pub const LEAD: usize = 0x38;
    pub const MOTION: usize = 0x3c;
    pub const MOTION0: usize = 0x3e;
    /// The gold missile's blasts so far and the timer that clears them (`0x2c5b70`'s tail).
    pub const BLASTS: usize = 0x40;
    pub const BLAST_T: usize = 0x44;
    pub const RANGE: usize = 0x48;
}

/// `0x20aa50` / `0x20aa68`: the rings' and puffs' colours (`randi(6)` each).
const COL_A: [u32; 6] = fx::SPARK_A;
const COL_B: [u32; 6] = fx::SPARK_B;

/// `0x2c5440` from the Devastator's update (module doc).
#[allow(clippy::too_many_arguments)]
pub fn spawn(table: &mut crate::moby_runtime::MobyTable, env: &crate::hero::items::ItemEnv, hits: &mut dyn crate::hero::items::HitSink, aim: (f32, f32), aim_h: f32, rot: (f32, f32), muzzle: [f32; 3], target: Option<MobyId>, hero: &crate::hero::Hero) -> Option<MobyId> {
    let id = hits.create_moby(table, CLASS, env.frame as u64)?;
    let dt = DT;
    let mut top = dt * 20.0;
    let mut lead = 1;
    let mut range = 70.0f32;
    let tinfo = target.and_then(|t| table.mobys.get(t)).map(|m| ([m.position[0], m.position[1], m.position[2]], crate::targeting::record(m)));
    if let (Some(t), Some((_, Some(r)))) = (target, tinfo) {
        crate::targeting::mark_missile(&mut table.mobys[t]);
        let pv = &table.mobys[t].pvars;
        if pv[r + 0xb] != 0 { lead = 0; }
        if pv[r + 0xc] != 0 { top = pv[r + 0xc] as f32 * dt; }
        if dt * 20.0 < top { range = 140.0; }
    }
    let plat = crate::hero::physics::to_f32x3(hero.plat_applied);
    let motion_t = if dt * 0.1 < len3(plat) { crate::hero::physics::ticks(60) } else { crate::hero::physics::ticks(15) };
    let m = &mut table.mobys[id];
    if m.pvars.len() < 0x80 { m.pvars.resize(0x80, 0); }
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
    m.visible = 1;
    m.rotation = [0.0, rot.1, rot.0, 0.0];
    m.state = 0;
    p::set_i32(&mut m.pvars, pv::OWNER, env.hero_moby as i32 + 1);
    m.position = [muzzle[0], muzzle[1], muzzle[2], m.position[3]];
    p::set_ff(&mut m.pvars, pv::TOP, top);
    p::set_i32(&mut m.pvars, pv::LEAD, lead);
    p::set_ff(&mut m.pvars, pv::RANGE, range);
    p::set_ff(&mut m.pvars, pv::YAW_VEL, 0.0);
    p::set_ff(&mut m.pvars, pv::PITCH_VEL, 0.0);
    p::set_i32(&mut m.pvars, pv::TARGET, target.map_or(0, |t| t as i32 + 1));
    p::set_ff(&mut m.pvars, pv::SPEED, dt * 20.0 / 3.0);
    p::set_ff(&mut m.pvars, pv::YAW, aim.0);
    p::set_ff(&mut m.pvars, pv::PITCH, aim.1);
    p::set_i32(&mut m.pvars, pv::LIFE, crate::hero::physics::ticks(300));
    if let Some((tp, _)) = tinfo {
        p::set_v4f(&mut m.pvars, pv::AIM, [tp[0], tp[1], tp[2] + aim_h, 0.0]);
        p::set_ff(&mut m.pvars, pv::AIM_H, aim_h);
    }
    p::set_i16(&mut m.pvars, pv::MOTION, motion_t as i16);
    p::set_i16(&mut m.pvars, pv::MOTION0, motion_t as i16);
    m.scale += m.scale;
    let r = rc_formats::moby_light::rotation_rows([0.0, rot.1, rot.0]);
    for (row, src) in m.rows.iter_mut().zip(r) { *row = [f32::from_bits(src[0]), f32::from_bits(src[1]), f32::from_bits(src[2]), 0.0]; }
    let from = table.mobys.get(env.hero_moby).map_or(muzzle, |h| [h.position[0], h.position[1], muzzle[2]]);
    let f4 = |a: [f32; 3]| [Pf::f(a[0]), Pf::f(a[1]), Pf::f(a[2]), Pf::ZERO];
    let blocked = hits.probe_moby(table, f4(from), f4(muzzle), 0, Some(env.hero_moby)).flatten().map(|h| h.point).or_else(|| env.coll.and_then(|c| crate::hero::physics::line_world(c, f4(from), f4(muzzle), 0)).map(|o| o.point));
    if let Some(h) = blocked {
        let m = &mut table.mobys[id];
        m.cmd = 1;
        m.position = [h[0], h[1], h[2], m.position[3]];
    }
    Some(id)
}

fn v3(a: [f32; 4]) -> [f32; 3] { [a[0], a[1], a[2]] }
fn v4(a: [f32; 3]) -> [f32; 4] { [a[0], a[1], a[2], 0.0] }

/// `0x2c5b70`.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x80 { w.mm(id).pvars.resize(0x80, 0); }
    let (l0, l1) = fx::frame_load(w);
    let throttle = 0.9 < l0 || 0.9 < l1;
    // The gold Devastator 0x13e52b (item 11).
    let gold = w.hero.weapons.gold.get(11).copied().unwrap_or(0);
    // The speed toward the top speed.
    let speed = {
        let m = w.mm(id);
        let s = p::ff(&m.pvars, pv::SPEED);
        let s = s + ((p::ff(&m.pvars, pv::TOP) - s) / 10.0) * 1.0;
        p::set_ff(&mut m.pvars, pv::SPEED, s);
        s
    };
    // The launcher's motion, fading out.
    let motion = {
        let m = w.mm(id);
        let mut t = p::i16(&m.pvars, pv::MOTION);
        sv::fast_dec_timer_s16(&mut t);
        p::set_i16(&mut m.pvars, pv::MOTION, t);
        let t0 = p::i16(&m.pvars, pv::MOTION0).max(1);
        let h = w.hero;
        let d = add3(crate::hero::physics::to_f32x3(h.plat_applied), crate::hero::physics::to_f32x3(h.disp));
        scale3(d, t as f32 / t0 as f32)
    };
    let life = p::i32(&w.m(id).pvars, pv::LIFE);
    let t = (p::i32(&w.m(id).pvars, pv::TARGET) as usize).checked_sub(1);
    let early = w.ticks(300) - w.ticks(5) <= life;
    let live = t.filter(|&t| !early && w.table.mobys.get(t).is_some_and(|m| m.state < 0x80));
    let pos0 = v3(w.m(id).position);
    let (ty, tp) = match live {
        None => {
            let pv = &w.m(id).pvars;
            if life < w.ticks(300) - w.ticks(5) { (p::ff(pv, pv::YAW), p::ff(pv, pv::PITCH)) } else { (w.m(id).rotation[2], w.m(id).rotation[1]) }
        }
        Some(t) => {
            let tm = w.m(t).position;
            let pvs = &w.m(id).pvars;
            let aim = [tm[0], tm[1], tm[2] + p::ff(pvs, pv::AIM_H)];
            let rel = sub3(aim, pos0);
            if p::i32(pvs, pv::LEAD) != 0 {
                let vt = sub3(sub3(aim, v3(p::v4f(pvs, pv::AIM))), motion);
                p::set_v4f(&mut w.mm(id).pvars, pv::AIM, v4(aim));
                let tt = super::missile::intercept_time(speed, vt, rel);
                if 0.0 < tt && (tt as i32) < w.ticks(300) {
                    let a = super::missile::lead_point(vt, rel, tt);
                    (atan(a[0], a[1]), -atan((a[0] * a[0] + a[1] * a[1]).sqrt(), a[2]))
                } else {
                    (atan(rel[0], rel[1]), -atan((rel[0] * rel[0] + rel[1] * rel[1]).sqrt(), rel[2]))
                }
            } else {
                (atan(rel[0], rel[1]), -atan((rel[0] * rel[0] + rel[1] * rel[1]).sqrt(), rel[2]))
            }
        }
    };
    let max = if live.is_none() { DT * 4.712_389 } else { DT * TAU };
    {
        let m = w.mm(id);
        let (mut vy, mut vp) = (p::ff(&m.pvars, pv::YAW_VEL), p::ff(&m.pvars, pv::PITCH_VEL));
        m.rotation[2] = spring_turn(m.rotation[2], ty, DT * DT * TAU, DT * DT * PI, max, &mut vy);
        m.rotation[1] = spring_turn(m.rotation[1], tp, DT * DT * TAU, DT * DT * PI, max, &mut vp);
        p::set_ff(&mut m.pvars, pv::YAW_VEL, vy);
        p::set_ff(&mut m.pvars, pv::PITCH_VEL, vp);
    }
    let (yaw, pitch) = (w.m(id).rotation[2], w.m(id).rotation[1]);
    let step = polar(speed, yaw, -pitch);
    // The trail every tick (every other one under load).
    if !throttle || w.counter & 1 == 0 { trail(w, id, step, speed, &TRAIL); }
    let dir = add3(step, motion);
    let pos = add3(pos0, dir);
    { let m = w.mm(id); m.position = [pos[0], pos[1], pos[2], m.position[3]]; }
    if pos[0] < 0.0 || pos[1] < 0.0 || pos[2] < 0.0 {
        w.delete_moby(id);
        return;
    }
    let n = with_len(dir, 1.0);
    let tmpl = HitTemplate { dir: [Pf::f(n[0]), Pf::f(n[1]), Pf::ONE, Pf::b(0x45af_df66)], attacker: Some(id), flags: 0x83_0000, b18: 3, b19: 1, h1a: CLASS as u16, damage: Pf::f(3.0), w20: 1 };
    let f4 = |a: [f32; 3]| [Pf::f(a[0]), Pf::f(a[1]), Pf::f(a[2]), Pf::ZERO];
    let owner = (p::i32(&w.m(id).pvars, pv::OWNER) as usize).checked_sub(1);
    let mut face: Option<([f32; 3], [f32; 3], [f32; 3])> = None;
    let mut last_moby: Option<MobyId> = None;
    let mut kind = w.m(id).cmd;
    match sv::line_hit_in(w.table, w.svc, w.classes, w.coll, f4(pos0), f4(pos), 0, w.hero_moby, &tmpl) {
        None => {
            if kind == 0 {
                let mut l = life;
                let out = crate::hero::guns::dec(&mut l);
                p::set_i32(&mut w.mm(id).pvars, pv::LIFE, l);
                let hero = w.hero_moby.map_or(pos0, |h| v3(w.m(h).position));
                let d = ((pos[0] - hero[0]).powi(2) + (pos[1] - hero[1]).powi(2)).sqrt();
                if out || p::ff(&w.m(id).pvars, pv::RANGE) < d {
                    // Not gold: deleted without a blast; the gold missile explodes where it is.
                    if gold == 0 {
                        w.delete_moby(id);
                        return;
                    }
                    kind = 1;
                }
            }
        }
        Some(h) => {
            let edge = sub3(h.tri[0], h.tri[1]);
            match h.moby {
                None if h.kind < 1 => {}
                None => {
                    kind = 1;
                    face = Some((h.point, edge, h.normal));
                }
                Some(m) if Some(m) == w.hero_moby => {}
                Some(m) if Some(m) == owner => {}
                Some(m) => {
                    last_moby = Some(m);
                    kind = 2;
                    face = Some((h.point, edge, h.normal));
                }
            }
            if let Some((pt, _, _)) = face {
                let m = w.mm(id);
                m.position = [pt[0], pt[1], pt[2], m.position[3]];
            }
        }
    }
    if kind == 0 {
        blast_timer(w, id);
        return;
    }
    let (edge, normal) = face.map_or(([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]), |f| (f.1, f.2));
    let drift = with_len(reflect(dir, normal), DT + DT);
    explode(w, id, kind, with_len(edge, 1.0), with_len(normal, 1.0), drift, throttle, gold);
    let blasts = p::i32(&w.m(id).pvars, pv::BLASTS) + 1;
    p::set_i32(&mut w.mm(id).pvars, pv::BLASTS, blasts);
    if gold == 0 || blasts == 3 {
        w.delete_moby(id);
        return;
    }
    // The gold missile goes on (`0x2c5b70`'s tail): off the face it hit, the hit moby ignored from now on (as its
    // owner), a new target (`0x2c5778`), class sound 1; three blasts at most while the timer +0x44 runs.
    let hit_moby = if kind == 2 { last_moby } else { None };
    let n = with_len(normal, 1.0);
    let r = reflect(dir, n);
    {
        let m = w.mm(id);
        m.cmd = 0;
        p::set_i32(&mut m.pvars, pv::TARGET, 0);
        m.rotation[2] = atan(r[0], r[1]);
        m.rotation[1] = -atan((r[0] * r[0] + r[1] * r[1]).sqrt(), r[2]);
        p::set_ff(&mut m.pvars, pv::YAW_VEL, 0.0);
        p::set_ff(&mut m.pvars, pv::PITCH_VEL, 0.0);
        for (q, nk) in m.position.iter_mut().zip(n) { *q += nk * 0.05; }
        if let Some(h) = hit_moby { p::set_i32(&mut m.pvars, pv::OWNER, h as i32 + 1); }
    }
    if p::i32(&w.m(id).pvars, pv::BLAST_T) == 0 {
        let t = w.ticks(90);
        p::set_i32(&mut w.mm(id).pvars, pv::BLAST_T, t);
    }
    retarget(w, id, hit_moby);
    w.play_sound(1, 0, id);
    blast_timer(w, id);
}

/// The tail of `0x2c5b70` every tick: the blast timer +0x44 run out clears the gold missile's blast count.
fn blast_timer(w: &mut World, id: MobyId) {
    let mut t = p::i32(&w.m(id).pvars, pv::BLAST_T);
    if crate::moby_update::creature::dec_timer_i32(&mut t) != 0 { p::set_i32(&mut w.mm(id).pvars, pv::BLASTS, 0); }
    p::set_i32(&mut w.mm(id).pvars, pv::BLAST_T, t);
}

/// `0x2c5778(missile, pvars, skip)`: the gold missile's next target: over the target list, the live mobys but `skip`
/// within 40 (3-D) of it, aimed at the record's height (+0x10; 0.5 without a record); a creature (class type 5) closer
/// than 2.5 in front of Ratchet (60° of his facing, its pitch under 45°) ends the search; else the nearest one so far
/// seen from the camera (`CollLine_Fix(camera, it, 6)` clear) and held by no other missile becomes the target: yaw /
/// pitch toward it (and the comparison's), the lead on, the turn rates and the speed 0, the aim point and height.
/// [L] The gold cone test (`2·asin(…) > gold·40° + 90°`) only feeds a value the game drops.
fn retarget(w: &mut World, id: MobyId, skip: Option<MobyId>) {
    let me = v3(w.m(id).position);
    let mut best = 10_000.0f32;
    let list = w.svc.targets.clone();
    let hero = w.hero_moby.map(|h| (v3(w.m(h).position), w.m(h).rotation[2]));
    for t in list {
        if Some(t) == skip || w.table.mobys.get(t).is_none_or(|m| 0x80 <= m.state) { continue; }
        let tm = w.m(t);
        let rec = crate::targeting::record(tm);
        let aim_h = rec.map_or(0.5, |r| p::ff(&tm.pvars, r + 0x10));
        let tp = [tm.position[0], tm.position[1], tm.position[2] + aim_h];
        let d = len3(sub3(tp, me));
        if 40.0 < d { continue; }
        let creature = w.classes.info(tm.o_class).is_some_and(|i| i.ty == 5);
        if !creature { continue; }
        let rel = sub3(tp, me);
        let yaw = atan(rel[0], rel[1]);
        let pitch = atan((rel[0] * rel[0] + rel[1] * rel[1]).sqrt(), rel[2]);
        if d < 2.5 {
            if let Some((hp, hy)) = hero {
                let a = atan(tm.position[0] - hp[0], tm.position[1] - hp[1]);
                if crate::moby_update::creature::diff_rots(hy, a) < std::f32::consts::FRAC_PI_3 && pitch.abs() < std::f32::consts::FRAC_PI_4 { return; }
            }
        }
        if best < d { continue; }
        let cam = [w.camera[0], w.camera[1], w.camera[2], Pf::ZERO];
        let to = [Pf::f(tp[0]), Pf::f(tp[1]), Pf::f(tp[2]), Pf::ZERO];
        if sv::line_hit_in(w.table, w.svc, w.classes, w.coll, cam, to, 6, Some(id), &HitTemplate::default()).is_some() { continue; }
        if crate::hero::devastator::already_targeted(w.table, t) { continue; }
        best = d;
        let m = w.mm(id);
        p::set_i32(&mut m.pvars, pv::TARGET, t as i32 + 1);
        p::set_ff(&mut m.pvars, pv::YAW, yaw);
        p::set_i32(&mut m.pvars, pv::LEAD, 1);
        p::set_ff(&mut m.pvars, pv::PITCH, -pitch);
        p::set_ff(&mut m.pvars, pv::YAW_VEL, 0.0);
        p::set_ff(&mut m.pvars, pv::PITCH_VEL, 0.0);
        p::set_ff(&mut m.pvars, pv::SPEED, 0.0);
        p::set_v4f(&mut m.pvars, pv::AIM, [tp[0], tp[1], tp[2] - aim_h, 0.0]);
        p::set_ff(&mut m.pvars, pv::AIM_H, aim_h);
    }
}

/// The trail's constants: the first puff's alpha, grey and life (ticks), the second puff's life, the spark's size and
/// life. Kalebo III's board missile 1475 runs the same trail code with its own (`units::board_missile`).
pub struct Trail {
    pub alpha: u8,
    pub rgb: u32,
    pub life: i32,
    pub life2: i32,
    pub spark: f32,
    pub spark_life: i32,
}

/// The Devastator missile's trail constants.
const TRAIL: Trail = Trail { alpha: 0x32, rgb: 0x50_5050, life: 40, life2: 7, spark: 25000.0, spark_life: 15 };

/// The trail (`0x2c5b70`): a smoke puff (type 44: size 40000 growing 1000, falling 0.0002, alpha 0x32 grey 0x505050,
/// `ticks(40)`, ALPHA 0x44 and texture `def[23][0]`) at a random point of this step drifting `randf(0.1, 0.2)·dt` less
/// `randf(0.1, 1)·dt` along the path; a brighter one (falling 0.0004, alpha 0x7f 0xb0b0b0, `ticks(7)`) at the missile
/// with the same drift; a spark (type 21, size 25000, 0.02 along the missile's z axis turned a random angle about its x
/// axis, 0x4f007fff → 0x1fffffff, `ticks(15)`, splitting). [`Trail`] holds the constants the copies differ in.
pub fn trail(w: &mut World, id: MobyId, step: [f32; 3], speed: f32, look: &Trail) {
    use crate::particles::{type21, type44};
    let x = w.rng.randf(-1.0, 1.0);
    let y = w.rng.randf(-1.0, 1.0);
    let z = w.rng.randf(-1.0, 1.0);
    let s = w.rng.randf(0.1, 0.2);
    let mut v = with_len([x, y, z], s * DT);
    let back = w.rng.randf(DT * 0.1, DT);
    v = add3(v, with_len(step, -back));
    let k = w.rng.randf(0.0, 1.0);
    let pos = v3(w.m(id).position);
    let p = add3(with_len(step, k * speed), pos);
    let t40 = w.ticks(look.life);
    let t7 = w.ticks(look.life2);
    let t15 = w.ticks(look.spark_life);
    let a = type44::Spawn { size: 40000.0, growth: 1000.0, damp: 1.0, fall: f32::from_bits(0xb951_b717), w: 0.0, pos: p, vel: v, life: t40, alpha: look.alpha, rgb: look.rgb, spin: 3 };
    let b = type44::Spawn { size: 40000.0, growth: 1000.0, damp: 1.0, fall: f32::from_bits(0xb9d1_b717), w: 0.0, pos, vel: v, life: t7, alpha: 0x7f, rgb: 0xb0_b0b0, spin: 3 };
    let rows = w.m(id).rows;
    let zax = [rows[2][0] * 0.02, rows[2][1] * 0.02, rows[2][2] * 0.02];
    let xax = [rows[0][0], rows[0][1], rows[0][2]];
    let Some(sys) = w.particles.as_deref_mut() else {
        // No particle system: the draws alone.
        let _ = w.rng.rand_angle();
        return;
    };
    if let Some(i) = type44::spawn_rng(sys, w.rng, &a) {
        let def = sys.def_first(23);
        let r = &mut sys.pool.recs[i];
        r[3] = 0x44;
        r[2] = def;
    }
    type44::spawn_rng(sys, w.rng, &b);
    let ang = w.rng.rand_angle();
    let sv = super::blaster_shot::rotate(zax, ang, xax);
    type21::spawn_rng(sys, w.rng, look.spark, [p[0], p[1], p[2], 0.0], [sv[0], sv[1], sv[2], 0.0], 0x4f00_7fff, 0x1fff_ffff, t15, 1);
}

/// The explosion (module doc).
#[allow(clippy::too_many_arguments)]
fn explode(w: &mut World, id: MobyId, kind: u8, edge: [f32; 3], normal: [f32; 3], drift: [f32; 3], throttle: bool, gold: u8) {
    // The gold missile: sizes and speeds ×(gold + 1), every colour through `0x270fa8(c, gold)`.
    let g1 = gold as f32 + 1.0;
    let sh = |c: u32| fx::colour_shift(c, gold);
    let pos = w.m(id).position;
    let p3 = v3(pos);
    let tmpl = HitTemplate { dir: [Pf::ZERO, Pf::ZERO, Pf::ONE, Pf::b(0x45af_df66)], attacker: Some(id), flags: 0x83_0000, b18: 3, b19: 1, h1a: CLASS as u16, damage: Pf::f(3.0), w20: 1 };
    w.sphere_mobys(Pf::f(g1 + g1), pos.map(Pf::f), 0x10, Some(id), Some(&tmpl));
    let (mut n, lower) = (10i32, if throttle { 1 } else { 0 });
    if throttle { n /= 3; }
    for _ in 0..n {
        let mut v = if kind == 1 {
            let a = w.rng.rand_angle();
            let r = super::blaster_shot::rotate(edge, a, normal);
            let s = w.rng.randf(8.5, 16.5);
            with_len(r, s * DT * g1)
        } else {
            let s = w.rng.randf(8.5, 16.5) * DT;
            let a = w.rng.rand_angle();
            let b = w.rng.rand_angle();
            polar(s * g1, a, b)
        };
        v[2] += DT * 5.0;
        v = add3(v, drift);
        if 1.0 < len3(v) { v = with_len(v, DT * 8.0); }
        let (t60, t120) = (w.ticks(60), w.ticks(120));
        let life = w.rng.rand_range(t60, t120) - lower * 35;
        let a = crate::particles::type15::Spawn { size: g1 * 40000.0, pos, vel: v4(v), c1: sh(0x4f00_7fff), c2: sh(0x1f00_007f), life, split: 1, def: -1, blend: -1 };
        fx::part15(w, &a);
    }
    // The fireball toward the camera.
    let cam = [w.camera[0].to_f32(), w.camera[1].to_f32(), w.camera[2].to_f32()];
    let x = w.rng.randf(-1.0, 1.0);
    let y = w.rng.randf(-1.0, 1.0);
    let z = w.rng.randf(-1.0, 1.0);
    let mut to = sub3(cam, p3);
    let d = len3(to);
    to[2] += d * 0.5;
    let mut v = add3(with_len([x, y, z], (d / 5.0) * DT * g1), with_len(to, (d + d) * DT));
    if DT * 10.0 < len3(v) { v = with_len(v, DT * 10.0); }
    let (t60, t90) = (w.ticks(60), w.ticks(90));
    let life = w.rng.rand_range(t60, t90);
    super::bomb::fireball(w, pos, v4(v), life, 0);
    // The rings (type 11), two per round.
    let mut rings = if d < 6.0 { (d as i32) / 2 } else { 3 };
    let slow = if d < 7.0 { 7.0 - d } else { 0.0 };
    if throttle { rings /= 2; }
    for _ in 0..rings.max(0) {
        let s = w.rng.randf(8.0, 10.0);
        let speed = s * g1 * DT - slow * DT;
        let c1 = sh(COL_A[w.rng.randi(6) as usize]);
        let c2 = sh(COL_B[w.rng.randi(6) as usize]);
        let (t15, t20) = (w.ticks(15), w.ticks(20));
        let life = w.rng.rand_range(t15, t20) - lower * 7;
        let (t25, t30) = (w.ticks(25), w.ticks(30));
        let t1 = w.rng.rand_range(t25, t30) - lower * 10;
        w.part11(Pf::f(400_000.0), Pf::f(speed), pos.map(Pf::f), v4(drift).map(Pf::f), c1, c2, life, t1, 0, 0);
        let (t5, t10) = (w.ticks(5), w.ticks(10));
        let life = w.rng.rand_range(t5, t10) - lower * 5;
        let (t15, t20) = (w.ticks(15), w.ticks(20));
        let t2 = w.rng.rand_range(t15, t20) - lower * 7;
        w.part11(Pf::f(400_000.0), Pf::f(speed * 0.5), pos.map(Pf::f), v4(drift).map(Pf::f), 0x7fff_ffff, 0xff_ffff, life, t2, 0, 0);
    }
    // The smoke puffs (type 8).
    for _ in 0..n {
        let x = w.rng.randf(-1.0, 1.0);
        let y = w.rng.randf(-1.0, 1.0);
        let z = w.rng.randf(-1.0, 1.0);
        let s = w.rng.randf(0.0, 3.0);
        let vel = with_len([x, y, z], s * DT * g1);
        let c1 = sh(COL_A[w.rng.randi(6) as usize]);
        let c2 = sh(COL_B[w.rng.randi(6) as usize]);
        let (t20, t35) = (w.ticks(20), w.ticks(35));
        let life = w.rng.rand_range(t20, t35) - lower * 10;
        fx::part08(w, g1 * 200_000.0, pos, v4(vel), c1, c2, life);
    }
    // The flashes (FlashSpawn 0x2c20e0).
    let dv = v4(drift);
    // The gold flashes: (0x3c, 0x7f, 0x3c), then red 0x20 for the third.
    let rb = if gold == 0 { 0x7f } else { 0x3c };
    if l0(w) < 0.95 && 9.0 < d {
        let t = w.ticks(15);
        super::debris::flash_spawn(w, g1 * 4.0, id, pos, dv, t, rb, 0x7f, rb, 0x20);
        let t = w.ticks(24);
        super::debris::flash_spawn(w, g1 * 4.0, id, pos, dv, t, rb, 0x7f, rb, 0x20);
    }
    let t = w.ticks(20);
    let r3 = if gold == 0 { 0x7f } else { 0x20 };
    super::debris::flash_spawn(w, g1 * 4.0, id, pos, dv, t, r3, 0x7f, 0, 0x30);
    if !throttle {
        let t = w.ticks(19);
        super::debris::flash_spawn(w, g1 + g1, id, pos, dv, t, 0xff, 0xff, 0xff, 0x20);
    }
    let amp = if d < 20.0 { 0.4 - d * 0.0175 } else { f32::from_bits(0x3d4c_ccd0) };
    let t = w.ticks(25);
    w.shake_camera(crate::follow_camera::ShakeRequest { axis: crate::follow_camera::ShakeAxis::Up, amp, ticks: t });
    w.play_sound(0, 0, id);
    if !throttle { fx::light_spawn(w, if gold == 0 { &fx::LIGHT_BOMB } else { &fx::LIGHT_GOLD }, pos); }
}

fn l0(w: &World) -> f32 { fx::frame_load(w).0 }
