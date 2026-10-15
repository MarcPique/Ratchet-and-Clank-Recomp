//! **Giant Clank's missile** (class 0x100; spawn level15 `0x29d408` / level18 `0x2aa0f8`, update level15 `0x29d6c0` /
//! level18 `0x2aa3b0`, the same code (`clusters.tsv`: one hash for the two); level00's `0x2a96f8` is the test level's
//! copy). Giant Clank fires one from each arm in his state 0x5f (`crate::hero::bodies::giant`, key 4 of his missile
//! sequence: `0x2a96f8(yaw, 0, 0.5, yaw, 0, hero, point, 0)`). Read from the level18 decompiler output and checked
//! against its disassembly.
//!
//! **System or not.** The code is a compiled copy of the Devastator missile (class 153, `0x2c5b70`,
//! [`crate::moby_update::classes::devastator_missile`]) with its own constants and branches (explosions.md §E): no
//! gold, no frame-load throttle, a speed of 100 u/s from rest, a life of `ticks(150)`, the range a flat 260 from the
//! hero (2D), the launcher motion only the platform's, its own target search ([`search`], `0x2658d8`), a brighter and
//! longer trail, unclamped streaks, the blast's flags 0x810000. So it is per-class code, as in the game; the shared
//! pieces are the port's: the intercept (`classes::missile`), `SpringTurn`, the blast's area hit
//! (`creature::attack::area_hit` = `0x26f8f8`), the fireball (`bomb::fireball`, `0x2c4c20`), the flashes
//! (`debris::flash_spawn`, `0x2c20e0`), the explosion light (`fx::light_spawn`, the template = `fx::LIGHT_BOMB`, checked
//! byte for byte: level18 0x1f1ee0 = level01 0x20a9b0) and the colour tables (`fx::SPARK_A` / `SPARK_B` = level18
//! 0x1f1f30, checked).
//!
//! **Pvars** (as the Devastator's): +0x00 the target's aim point last tick, +0x10 the owner (moby + 1), +0x14 the
//! life, +0x18 the target (moby + 1), +0x1c the aim height, +0x20 the speed, +0x24 / +0x28 the yaw / pitch turn
//! rates, +0x2c / +0x30 the yaw / pitch it was fired along, +0x34 the top speed, +0x38 leading, +0x3c / +0x3e the
//! launcher-motion timer and its start. Moby +0xbc: explode now (1 a face, 2 a moby).
//!
//! **Coverage** (level18 addresses):
//!
//! | address | what | status |
//! |---|---|---|
//! | 0x2aa0f8 +0 | `CreateMoby(0x100)`; +0x30 = 0xff, +0x31 = 1, state 0, +0x32 = 0xff; scale ×= the level's 3.0 (gp−0x57c8; level15 gp−0x5780, both 3.0 in the image); +0x48 / +0x44 = the moby yaw / pitch | ported ([`spawn`]) |
//! | +1 | no target given: the aim `0x265460(20·dt, yaw, pitch)` and the search `0x2658d8(20°, owner, muzzle, aim, &yaw, &pitch, &aim height)` | ported ([`search`]) |
//! | +2 | owner, position = muzzle, lead 1, turn rates 0, speed 0, top speed 100·dt; a target: its record +0x1e \|= 0x80, record +0x0b → lead 0, +0x0c → top = +0x0c·dt | ported (`targeting::mark_missile`) |
//! | +3 | yaw / pitch, life `ticks(150)`; a target: aim point = its position + the aim height, +0x1c = the aim height | ported |
//! | +4 | the motion timer: `ticks(60)` when \|0x13f490\| (the applied platform motion) > 0.1·dt, else `ticks(1)` | ported |
//! | +5 | `CollLine_Fix((owner.x, owner.y, muzzle.z), muzzle, 0, 0x1413d0, 0)` hit: +0xbc = 1, position = the hit | ported |
//! | +6 | `MobyBuildRotation`; the missile list 0x1b1170 (+ 0x1f); scale doubled | ported (rows); the list is the moby table in the port (`hero::devastator::already_targeted` reads class 0x100 too) |
//! | 0x2aa3b0 +0 | speed += (top − speed)/10 | ported ([`update`]) |
//! | +1 | `FastDecTimer(+0x3c)`; motion = 0x13f490 × timer / start | ported (`Hero::plat_applied`) |
//! | +2 | the missile view 0x15f30c on and a target: the target bracket `0x1f6c48(\|0.5 − d/50·0.35\|, 0, 90, m, ~0xf00000, target point, 0xd, −1, 4)` (the table 0x169a40) | NOT ported (G-HERO-037; unreachable [L]: the view needs Ratchet's Visibomb while a 150-tick missile flies) |
//! | +3 | a live target (state < 0x80) after the first tick (life < `ticks(150)`): its aim point; leading: the motion of the aim point less the launcher's, the intercept (`ticks(150)` cap), else straight at it | ported |
//! | +4 | no live target: the fired yaw / pitch after the first tick, else its own rotation | ported |
//! | +5 | `SpringTurn` yaw and pitch (2π·dt², π·dt²), at most 270°/s without a target word, 360°/s with one | ported |
//! | +6 | step = `0x265460(speed, yaw, −pitch)` | ported |
//! | +7 | the trail every tick (types 44 ×2, 21) | ported ([`trail`]) |
//! | +8 | step += motion; position += step; out of the positive octant → list removal, `DeleteMoby` | ported |
//! | +9 | `CollLine_Fix(old, new, 0, 0x1413d0, tmpl)` (push = the step's direction, z 1, 5627.97; flags 0x830000; damage 3; type 3 / 1; class) | ported (`services::line_hit_in`) |
//! | +10 | nothing: `FastDecTimer(life)` out or \|xy − Ratchet\| > 260 → list removal, `DeleteMoby` (no blast) | ported |
//! | +11 | a face of kind ≥ 1 → +0xbc = 1; the hero moby → through; the owner → ignored; another moby → +0xbc = 2 (the blast's ignore = itself); then the edge, the drift (the step reflected, 2·dt), the normal | ported |
//! | +12 | the blast: `coll_sphere_mobys(2, pos, 0x10, m)` + `0x25cd38(3, 1, 1, m, pos, list, ignore, 0x810000, 3, 1)` | ported (`attack::area_hit`) |
//! | +13 | 10 streaks (type 15, 40000, 0x4f007fff → 0x1f00007f, 60..120 ticks, split): on a face in its plane, else random; `randf(8.5, 16.5)·dt`; z += 5·dt; + drift (not clamped) | ported ([`explode`]) |
//! | +14 | the fireball toward the camera (`0x2b1cb8`, life 60..90) | ported (`bomb::fireball`) |
//! | +15 | the rings (type 11, 400000; `d < 6`: d/2 pairs, else 3; slower within 7) | ported |
//! | +16 | 10 puffs (type 8, 200000, 20..35 ticks) | ported |
//! | +17 | the flashes (`0x2af178`: two white beyond 9 with the frame load < 0.95; yellow; white 2.0) | ported (`debris::flash_spawn`) |
//! | +18 | +0xbc = 0; the shake record 0x1677e0 / 0x1677e8 = (0.4 − 0.0175·d, or 0.05 beyond 20), `ticks(25)` | ported (`World::shake_camera`, up) |
//! | +19 | `PlayClassSound(0, 0, m)`; the light `0x2de830(template, pos)`; list removal; `DeleteMoby` | ported |
//! | spawn +0xbc = 1 with no face this tick | the blast with an uninitialised edge (stack) | ported with the edge (1, 0, 0) [L] |
//!
//! Native `f32`, the game's draws in its order.

use crate::hero::guns::{add3, len3, reflect, scale3, sub3, with_len};
use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx;
use crate::moby_update::creature::turn::spring_turn;
use crate::moby_update::creature::{atan, attack};
use crate::moby_update::services::{self as sv, pvar as p, HitTemplate, World};
use crate::ps2v::Pf;
use crate::targeting::polar;
use std::f32::consts::{PI, TAU};

/// The update in the level15 class table (level18's 0x2aa3b0 is the same code).
pub const UPDATE_FN: u32 = 0x29_d6c0;
pub const REFERENCE_LEVEL: u32 = 15;
pub const CLASS: i16 = 0x100;
pub const CLASSES: [i16; 1] = [CLASS];
const DT: f32 = 1.0 / 60.0;
/// The level's scale factor (level15 gp−0x5780 / level18 gp−0x57c8: 3.0 in both images, no writer).
const LEVEL_SCALE: f32 = 3.0;
/// The search's cone (`0x3eb2b8c2`, 20°).
const CONE: f32 = f32::from_bits(0x3eb2_b8c2);

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
}

fn v3(a: [f32; 4]) -> [f32; 3] { [a[0], a[1], a[2]] }
fn v4(a: [f32; 3]) -> [f32; 4] { [a[0], a[1], a[2], 0.0] }
fn f4(a: [f32; 3]) -> [Pf; 4] { [Pf::f(a[0]), Pf::f(a[1]), Pf::f(a[2]), Pf::ZERO] }
fn len2(a: [f32; 3]) -> f32 { (a[0] * a[0] + a[1] * a[1]).sqrt() }

/// The hero moby 0x1413d0 (the body moby while Giant Clank is the hero).
fn hero_moby(w: &World) -> Option<MobyId> { w.hero_moby.map(|r| w.hero.hero_moby(r)) }

/// The search's result: the aim and the target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Found {
    pub yaw: f32,
    pub pitch: f32,
    pub aim_h: f32,
    pub target: Option<MobyId>,
}

/// Level18 `0x2658d8(cone, owner, from, dir, &yaw, &pitch, &aim height)` (level15 `0x2527c0`, the same code on 00, 04, 07,
/// 09, 10, 13): over the target list (targetable, live, class type 5), the aim points (the record's height, 0.5
/// without one). Within 7.5 (3D) of `from`, a creature within 60° of the hero moby's yaw (seen from the hero's position)
/// and 60° of elevation is taken outright (the search ends; the aim height stays the caller's). Else the nearest so far
/// within the cone around the current aim (pitch + 0x13f634·0.5, narrowed by the record's radius byte:
/// `targeting::cone_miss`) whose camera line (`CollLine_Fix(camera, point, 6, owner)`) is clear: the aim turns to it and
/// the aim height becomes its record's (0.5 without one). A blocked line skips the target.
pub fn search(w: &World, cone: f32, owner: Option<MobyId>, from: [f32; 3], dir: [f32; 3], aim_h: f32) -> Found {
    let mut f = Found { yaw: atan(dir[0], dir[1]), pitch: -atan(len2(dir), dir[2]), aim_h, target: None };
    let mut best = 10000.0f32;
    let hm = hero_moby(w);
    let hero_yaw = hm.map_or(w.hero.rot[2].to_f32(), |h| w.m(h).rotation[2]);
    let hp = crate::hero::physics::to_f32x3(w.hero.pos);
    let ground = w.hero.pitch.to_f32();
    let cam = w.camera;
    for &id in &w.svc.targets {
        let Some(m) = w.table.mobys.get(id) else { continue };
        if m.state >= 0x80 { continue; }
        let p = crate::targeting::aim_point(m);
        if m.mode & crate::moby_runtime::mode::TARGETABLE == 0 || !m.has_class { continue; }
        if w.classes.info(m.o_class).map(|i| i.ty) != Some(5) { continue; }
        let ty = atan(p[0] - from[0], p[1] - from[1]);
        let elev = atan(len2(sub3(p, from)), p[2] - from[2]);
        let d = len3(sub3(p, from));
        if d < 7.5 {
            let to = atan(m.position[0] - hp[0], m.position[1] - hp[1]);
            if crate::moby_update::creature::diff_rots(hero_yaw, to) < std::f32::consts::FRAC_PI_3 && elev.abs() < std::f32::consts::FRAC_PI_3 {
                f.pitch = -elev;
                f.yaw = ty;
                f.target = Some(id);
                return f;
            }
        }
        if d <= best {
            let rec = crate::targeting::record(m);
            let miss = crate::targeting::cone_miss(d, f.yaw, f.pitch + ground * 0.5, from, p, rec.and(crate::targeting::record_radius(m)), cone);
            if miss < cone {
                if w.line([cam[0], cam[1], cam[2], Pf::ZERO], f4(p), 6, owner).is_some() { continue; }
                f.pitch = -elev;
                f.yaw = ty;
                best = d;
                f.target = Some(id);
                f.aim_h = crate::targeting::aim_height(m).unwrap_or(0.5);
            }
        }
    }
    f
}

/// `0x2a96f8(yaw, pitch, aim height, moby yaw, moby pitch, owner, muzzle, target)` (level18 0x2aa0f8): see the module
/// doc. None: no moby could be made.
#[allow(clippy::too_many_arguments)]
pub fn spawn(w: &mut World, yaw: f32, pitch: f32, aim_h: f32, moby_yaw: f32, moby_pitch: f32, owner: MobyId, muzzle: [f32; 3], target: Option<MobyId>) -> Option<MobyId> {
    let id = w.create_moby(CLASS)?;
    {
        let m = w.mm(id);
        if m.pvars.len() < 0x80 { m.pvars.resize(0x80, 0); }
        m.update_dist = 0xff;
        m.visible = 1;
        m.state = 0;
        m.draw_dist = 0xff;
        m.scale *= LEVEL_SCALE;
        m.rotation[2] = moby_yaw;
        m.rotation[1] = moby_pitch;
    }
    let (mut yaw, mut pitch, mut aim_h) = (yaw, pitch, aim_h);
    let mut target = target;
    if target.is_none() {
        let dir = polar(DT * 20.0, moby_yaw, moby_pitch);
        let f = search(w, CONE, Some(owner), muzzle, dir, aim_h);
        (yaw, pitch, aim_h, target) = (f.yaw, f.pitch, f.aim_h, f.target);
    }
    let mut top = DT * 100.0;
    let mut lead = 1;
    if let Some(t) = target {
        if let Some(r) = crate::targeting::record(w.m(t)) {
            crate::targeting::mark_missile(w.mm(t));
            let pv_ = &w.m(t).pvars;
            if pv_[r + 0xb] != 0 { lead = 0; }
            if pv_[r + 0xc] != 0 { top = pv_[r + 0xc] as f32 * DT; }
        }
    }
    let life = w.ticks(150);
    let plat = crate::hero::physics::to_f32x3(w.hero.plat_applied);
    let motion_t = if DT * 0.1 < len3(plat) { w.ticks(60) } else { w.ticks(1) };
    let tpos = target.map(|t| v3(w.m(t).position));
    {
        let m = w.mm(id);
        let pv_ = &mut m.pvars;
        p::set_i32(pv_, pv::OWNER, owner as i32 + 1);
        p::set_i32(pv_, pv::LEAD, lead);
        p::set_ff(pv_, pv::YAW_VEL, 0.0);
        p::set_ff(pv_, pv::PITCH_VEL, 0.0);
        p::set_ff(pv_, pv::SPEED, 0.0);
        p::set_ff(pv_, pv::TOP, top);
        p::set_i32(pv_, pv::TARGET, target.map_or(0, |t| t as i32 + 1));
        p::set_ff(pv_, pv::YAW, yaw);
        p::set_ff(pv_, pv::PITCH, pitch);
        p::set_i32(pv_, pv::LIFE, life);
        if let Some(tp) = tpos {
            p::set_v4f(pv_, pv::AIM, [tp[0], tp[1], tp[2] + aim_h, 0.0]);
            p::set_ff(pv_, pv::AIM_H, aim_h);
        }
        p::set_i16(pv_, pv::MOTION, motion_t as i16);
        p::set_i16(pv_, pv::MOTION0, motion_t as i16);
        m.position = [muzzle[0], muzzle[1], muzzle[2], m.position[3]];
    }
    // The line from the owner (at the muzzle's height) to the muzzle: a hit starts the blast there.
    let from = { let o = w.m(owner).position; [o[0], o[1], muzzle[2]] };
    if let Some(h) = w.line(f4(from), f4(muzzle), 0, hero_moby(w)) {
        let m = w.mm(id);
        m.cmd = 1;
        m.position = [h.point[0].to_f32(), h.point[1].to_f32(), h.point[2].to_f32(), m.position[3]];
    }
    // MobyBuildRotation; scale doubled.
    let m = w.mm(id);
    let r = rc_formats::moby_light::rotation_rows([0.0, moby_pitch, moby_yaw]);
    for (row, src) in m.rows.iter_mut().zip(r) { *row = [f32::from_bits(src[0]), f32::from_bits(src[1]), f32::from_bits(src[2]), 0.0]; }
    m.scale += m.scale;
    Some(id)
}

/// Level18 `0x2aa3b0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x80 { w.mm(id).pvars.resize(0x80, 0); }
    // The speed toward the top speed.
    let speed = {
        let m = w.mm(id);
        let s = p::ff(&m.pvars, pv::SPEED);
        let s = s + (p::ff(&m.pvars, pv::TOP) - s) / 10.0;
        p::set_ff(&mut m.pvars, pv::SPEED, s);
        s
    };
    // The launcher's (platform) motion, fading out.
    let motion = {
        let m = w.mm(id);
        let mut t = p::i16(&m.pvars, pv::MOTION);
        sv::fast_dec_timer_s16(&mut t);
        p::set_i16(&mut m.pvars, pv::MOTION, t);
        let t0 = p::i16(&m.pvars, pv::MOTION0).max(1);
        scale3(crate::hero::physics::to_f32x3(w.hero.plat_applied), t as f32 / t0 as f32)
    };
    // (0x15f30c, the missile view's target bracket: G-HERO-037.)
    let life = p::i32(&w.m(id).pvars, pv::LIFE);
    let tword = p::i32(&w.m(id).pvars, pv::TARGET);
    let t = (tword as usize).checked_sub(1);
    let first = w.ticks(150) - w.ticks(0) <= life;
    let live = t.filter(|&t| !first && w.table.mobys.get(t).is_some_and(|m| m.state < 0x80));
    let pos0 = v3(w.m(id).position);
    let (ty, tp) = match live {
        None => {
            let pv_ = &w.m(id).pvars;
            if life < w.ticks(150) - w.ticks(0) { (p::ff(pv_, pv::YAW), p::ff(pv_, pv::PITCH)) } else { (w.m(id).rotation[2], w.m(id).rotation[1]) }
        }
        Some(t) => {
            let tm = w.m(t).position;
            let pvs = &w.m(id).pvars;
            let aim = [tm[0], tm[1], tm[2] + p::ff(pvs, pv::AIM_H)];
            let rel = sub3(aim, pos0);
            let mut tt = 0.0f32;
            let mut vt = [0.0; 3];
            if p::i32(pvs, pv::LEAD) != 0 {
                vt = sub3(sub3(aim, v3(p::v4f(pvs, pv::AIM))), motion);
                p::set_v4f(&mut w.mm(id).pvars, pv::AIM, v4(aim));
                tt = crate::moby_update::classes::missile::intercept_time(speed, vt, rel);
            }
            let a = if 0.0 < tt && tt < w.ticks(150) as f32 { crate::moby_update::classes::missile::lead_point(vt, rel, tt) } else { rel };
            (atan(a[0], a[1]), -atan(len2(a), a[2]))
        }
    };
    let max = if tword == 0 { DT * 4.712_389 } else { DT * TAU };
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
    trail(w, id, step, speed);
    let dir = add3(step, motion);
    let pos = add3(pos0, dir);
    { let m = w.mm(id); m.position = [pos[0], pos[1], pos[2], m.position[3]]; }
    if pos[0] < 0.0 || pos[1] < 0.0 || pos[2] < 0.0 {
        w.delete_moby(id);
        return;
    }
    let n = with_len(dir, 1.0);
    let tmpl = HitTemplate { dir: [Pf::f(n[0]), Pf::f(n[1]), Pf::ONE, Pf::b(0x45af_df66)], attacker: Some(id), flags: 0x83_0000, b18: 3, b19: 1, h1a: CLASS as u16, damage: Pf::f(3.0), w20: 1 };
    let owner = (p::i32(&w.m(id).pvars, pv::OWNER) as usize).checked_sub(1);
    let hm = hero_moby(w);
    let mut edge = [1.0f32, 0.0, 0.0];
    let mut normal = [0.0f32, 0.0, 1.0];
    let mut drift = [0.0f32, 0.0, DT * 8.0];
    let mut ignore = None;
    match sv::line_hit_in(w.table, w.svc, w.classes, w.coll, f4(pos0), f4(pos), 0, hm, &tmpl) {
        None => {
            let mut l = life;
            let out = crate::hero::guns::dec(&mut l);
            p::set_i32(&mut w.mm(id).pvars, pv::LIFE, l);
            let hp = crate::hero::physics::to_f32x3(w.hero.pos);
            if out || 260.0 < len2(sub3(pos, hp)) {
                w.delete_moby(id);
                return;
            }
        }
        Some(h) => {
            let hit = match h.moby {
                None if h.kind < 1 => None,
                None => Some(1u8),
                Some(m) if Some(m) == hm => None,
                Some(m) if Some(m) == owner => None,
                Some(_) => Some(2u8),
            };
            if let Some(k) = hit {
                if k == 2 { ignore = Some(id); }
                let m = w.mm(id);
                m.cmd = k;
                m.position = [h.point[0], h.point[1], h.point[2], m.position[3]];
                edge = with_len(sub3(h.tri[0], h.tri[1]), 1.0);
                drift = with_len(reflect(dir, h.normal), DT + DT);
                normal = with_len(h.normal, 1.0);
            }
        }
    }
    let kind = w.m(id).cmd;
    if kind == 0 { return; }
    explode(w, id, kind, edge, normal, drift, ignore);
    w.delete_moby(id);
}

/// The trail: a smoke puff (type 44: size 40000 growing 1000, falling 0.0002, alpha 0x7f grey 0x606060, `ticks(60)`,
/// ALPHA 0x44 and texture `def[23][0]`) at a random point of this step drifting `randf(0.1, 0.2)·dt` less
/// `randf(0.1, 1)·dt` along the step; a brighter one (falling 0.0004, alpha 0x7f 0xb0b0b0, `ticks(6)`) at the missile;
/// a spark (type 21, size 20000, 0.02 along the missile's z axis turned a random angle about its x axis, 0x4f007fff →
/// 0x1fffffff, `ticks(5)`, splitting). Every tick.
fn trail(w: &mut World, id: MobyId, step: [f32; 3], speed: f32) {
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
    let at = add3(with_len(step, k * speed), pos);
    let (t60, t6, t5) = (w.ticks(60), w.ticks(6), w.ticks(5));
    let a = type44::Spawn { size: 40000.0, growth: 1000.0, damp: 1.0, fall: f32::from_bits(0xb951_b717), w: 0.0, pos: at, vel: v, life: t60, alpha: 0x7f, rgb: 0x60_6060, spin: 3 };
    let b = type44::Spawn { size: 40000.0, growth: 1000.0, damp: 1.0, fall: f32::from_bits(0xb9d1_b717), w: 0.0, pos, vel: v, life: t6, alpha: 0x7f, rgb: 0xb0_b0b0, spin: 3 };
    let rows = w.m(id).rows;
    let zax = [rows[2][0] * 0.02, rows[2][1] * 0.02, rows[2][2] * 0.02];
    let xax = [rows[0][0], rows[0][1], rows[0][2]];
    let Some(sys) = w.particles.as_deref_mut() else {
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
    let sv_ = crate::moby_update::classes::blaster_shot::rotate(zax, ang, xax);
    type21::spawn_rng(sys, w.rng, 20000.0, [at[0], at[1], at[2], 0.0], [sv_[0], sv_[1], sv_[2], 0.0], 0x4f00_7fff, 0x1fff_ffff, t5, 1);
}

/// The blast (module doc rows +12..+19).
fn explode(w: &mut World, id: MobyId, kind: u8, edge: [f32; 3], normal: [f32; 3], drift: [f32; 3], ignore: Option<MobyId>) {
    let pos = w.m(id).position;
    let p3 = v3(pos);
    attack::area_hit(w, 2.0, pos, id, 3.0, 1.0, 1.0, ignore, 0x81_0000, 3, 1);
    for _ in 0..10 {
        let mut v = if kind == 1 {
            let a = w.rng.rand_angle();
            let r = crate::moby_update::classes::blaster_shot::rotate(edge, a, normal);
            let s = w.rng.randf(8.5, 16.5);
            with_len(r, s * DT)
        } else {
            let s = w.rng.randf(8.5, 16.5) * DT;
            let a = w.rng.rand_angle();
            let b = w.rng.rand_angle();
            polar(s, a, b)
        };
        v[2] += DT * 5.0;
        v = add3(v, drift);
        let (t60, t120) = (w.ticks(60), w.ticks(120));
        let life = w.rng.rand_range(t60, t120);
        let a = crate::particles::type15::Spawn { size: 40000.0, pos, vel: v4(v), c1: 0x4f00_7fff, c2: 0x1f00_007f, life, split: 1, def: -1, blend: -1 };
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
    let mut v = add3(with_len([x, y, z], (d / 5.0) * DT), with_len(to, (d + d) * DT));
    if DT * 10.0 < len3(v) { v = with_len(v, DT * 10.0); }
    let (t60, t90) = (w.ticks(60), w.ticks(90));
    let life = w.rng.rand_range(t60, t90);
    crate::moby_update::classes::bomb::fireball(w, pos, v4(v), life, 0);
    // The rings (type 11), two per round.
    let rings = if d < 6.0 { (d as i32) / 2 } else { 3 };
    let slow = if d < 7.0 { 7.0 - d } else { 0.0 };
    let base = v4(drift).map(Pf::f);
    for _ in 0..rings.max(0) {
        let s = w.rng.randf(8.0, 10.0);
        let speed = s * DT - slow * DT;
        let c1 = fx::SPARK_A[w.rng.randi(6) as usize];
        let c2 = fx::SPARK_B[w.rng.randi(6) as usize];
        let (t15, t20) = (w.ticks(15), w.ticks(20));
        let life = w.rng.rand_range(t15, t20);
        let (t25, t30) = (w.ticks(25), w.ticks(30));
        let t1 = w.rng.rand_range(t25, t30);
        w.part11(Pf::f(400_000.0), Pf::f(speed), pos.map(Pf::f), base, c1, c2, life, t1, 0, 0);
        let (t5, t10) = (w.ticks(5), w.ticks(10));
        let life = w.rng.rand_range(t5, t10);
        let (t15, t20) = (w.ticks(15), w.ticks(20));
        let t2 = w.rng.rand_range(t15, t20);
        w.part11(Pf::f(400_000.0), Pf::f(speed * 0.5), pos.map(Pf::f), base, 0x7fff_ffff, 0xff_ffff, life, t2, 0, 0);
    }
    // The smoke puffs (type 8).
    for _ in 0..10 {
        let x = w.rng.randf(-1.0, 1.0);
        let y = w.rng.randf(-1.0, 1.0);
        let z = w.rng.randf(-1.0, 1.0);
        let s = w.rng.randf(0.0, 3.0);
        let vel = with_len([x, y, z], s * DT);
        let c1 = fx::SPARK_A[w.rng.randi(6) as usize];
        let c2 = fx::SPARK_B[w.rng.randi(6) as usize];
        let (t20, t35) = (w.ticks(20), w.ticks(35));
        let life = w.rng.rand_range(t20, t35);
        fx::part08(w, 200_000.0, pos, v4(vel), c1, c2, life);
    }
    // The flashes.
    let dv = v4(drift);
    if fx::frame_load(w).0 < 0.95 && 9.0 < d {
        let t = w.ticks(15);
        crate::moby_update::classes::debris::flash_spawn(w, 4.0, id, pos, dv, t, 0x7f, 0x7f, 0x7f, 0x20);
        let t = w.ticks(24);
        crate::moby_update::classes::debris::flash_spawn(w, 4.0, id, pos, dv, t, 0x7f, 0x7f, 0x7f, 0x20);
    }
    let t = w.ticks(20);
    crate::moby_update::classes::debris::flash_spawn(w, 4.0, id, pos, dv, t, 0x7f, 0x7f, 0, 0x30);
    let t = w.ticks(19);
    crate::moby_update::classes::debris::flash_spawn(w, 2.0, id, pos, dv, t, 0xff, 0xff, 0xff, 0x20);
    w.mm(id).cmd = 0;
    let amp = if d < 20.0 { 0.4 - d * 0.0175 } else { f32::from_bits(0x3d4c_ccd0) };
    let t = w.ticks(25);
    w.shake_camera(crate::follow_camera::ShakeRequest { axis: crate::follow_camera::ShakeAxis::Up, amp, ticks: t });
    w.play_sound(0, 0, id);
    fx::light_spawn(w, &fx::LIGHT_BOMB, pos);
}
