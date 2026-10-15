//! **The fleet's turrets, class 347** (level17 `0x2cb310`, census U563, eight instances) **and their shot 1368**
//! (`0x2e8e08`, made by `0x2e94d8`): the targets of the fleet's ship mission ([`super::fleet_ship`] counts them on its
//! HUD). A turret turns (rot.z) and raises its barrel (the look-at node) toward its target and fires a bolt along it:
//! at the fleet's ship (class 1379) once Ratchet flies it within 254, when it has the ship in the line of fire, every
//! 45 ticks; while Ratchet walks a magnetic floor (0x13f658) at random points of its target cuboid, when that cuboid is
//! in sight, on its phase of a 90-tick cycle. Now and then (1 in 500 a tick) it launches a fighter of its group
//! ([`super::ship_fighter::launch`]); it places its escort fighters along their path at the start. Only the ship's
//! lasers 1009 and missiles 295 (and hits without an attacker) wear its 15 health; at 0 it breaks in two pieces. The
//! bolt flies straight (homing a little on Ratchet's body when close), leaves type-80 glows, and hurts what it hits
//! (6, a sphere of 0.75) before it bursts into type-60 sparks. Read from the level17 decomp; the level data from the
//! overlay (gp 0x166c00). Native `f32`.
//!
//! **System or not.** Per-class code (a cluster of one), on the shared pieces: the hit resolver and flash
//! (`creature::{damage, flash}`), the NPC look-at node (`manip::look`), the target search (`creature::target`), the
//! sphere hit (`creature::attack::sphere_hit`), the break pieces, the particles 27 / 60 / 80, the fighters.
//!
//! **Turret pvars** (0x110; the instance's words): +0x20 the damage record (health 15, +0x24 = 15), +0x60 the flash,
//! +0x70 the target cuboid, +0x74 the fire phase (state 2) / timer (state 3), +0x78 the yaw rate, +0x7c the fighters'
//! group it launches, +0x80 (s16) the escorts' group, +0x84 their path, +0x90 the look-at record. **Shot pvars** (0x44):
//! +0x00 the velocity, +0x10 the target, +0x38 the timer, +0x3c the turret (index + 1), +0x40 the speed.
//!
//! ## Coverage (the turret, `0x2cb310`)
//! | address | what | port |
//! |---|---|---|
//! | entry | the hits (`0x2cbc10`) | [`hits`] |
//! | 0 | → 1; +0x24 = 15, health 15; the escorts placed (`0x2cbd38`); the mission done → `DeleteMoby` | [`update`], [`place_escorts`] |
//! | 1 | not on a magnetic floor: Ratchet flying the fleet's ship (0x32, the record's class 0x563) within 254 (xy) → 3; on one: the line from 10 above it to 0.9 of the way to the target cuboid (+0.5 z) clear (`CollLine_Fix(·, ·, 2, m, 0)`) → 2 | [`update`] |
//! | 2 | off the floor → 1 (this tick goes on); joint lists 0 / 1 (`MobyGetBoneMatrix(m, 2, gp−0x5280)`); a random point of the cuboid (its rows · `randf(0, 1)` each + its centre); d = it − joint 0 | [`update`], [`aim`] |
//! | 2, 3 | rot.z `0x270cc0(atan(d), 30°·dt², 30°·dt², 60°·dt, +0x78)`; the look-at's pitch target (+0xf4) = −atan(\|d\|xy, d.z); `0x2777d8(0.02, 0.3, m, +0x90, 0)` | [`aim`] (`turn::turn_toward`, `manip::look`) |
//! | 2 | the tick ≡ +0x74 (mod `ticks(90)`), the barrel within 2° of its pitch target and the yaw within 2° of the cuboid's centre: the muzzle glint (`PartType27Spawn(1264096, joint 1 + unit(d)·60·dt, v, 0x80ff8f6f, 3)`, byte 9 = 0x74; v an unset stack vector: 0 [L]), `PlayClassSound(0, 0)`, a bolt (`0x2e94d8(60·dt, m, joint 1, the cuboid's centre)`) | [`update`], [`glint`], [`shoot`] |
//! | 3 | Ratchet off the ship → 1; the target (`0x274b78(512)`); d = its position − joint 0; the aim; `FastDecTimer(+0x74)` out, within 2° both ways, the line joint 1 → the target clear but for the ship (`CollLine_Fix(·, ·, 2, ship, 0)`): the glint at joint 1, +0x74 = `ticks(90)` / 2, `PlayClassSound(0, 0)`, a bolt along the barrel (joint 1 + unit(joint 1 − joint 0)·250) | [`update`] (`target::acquire`) |
//! | 2, 3 | the fighters' group (+0x7c) and `randi(500)` = 0 → a launch (`0x2f4fb8(+0x7c)`) (state 3 only) | [`update`] (`ship_fighter::launch`) |
//! | 4 | `BreakFxB(0, m, 0x63a / 0x63b, position, Euler, 0, 0, 0, 0)`; `0x273f50(10, 13, m, position, −1)`; `DeleteMoby` | [`update`] (`fx::break_piece`, `fx::death_explosion`) |
//! | 0x2cbc10 | not in state 0: `MobyGetHitMessage(m, 0x330000, 0)` kept only without an attacker or from the classes 0x3f1 / 0x127; `0x26f378(m, hit, +0x20, 0, &out, &damage, 0, 4)`; out ≠ 1 and not in 4: health −= damage, ≤ 0 → 4, not targetable; the flash red 0xf0 (`0x272318`); +0xa4 = 0xff; the flash (`0x2723f8`) | [`hits`] |
//! | 0x2cbd38 | the escorts (class 1843) of group +0x80 spread along path +0x84: step = its count / their number; each at point k·step, +0x64 = k·step, +0x60 = the path, +0x68 = 0 | [`place_escorts`] |
//!
//! ## Coverage (the shot 1368, `0x2e8e08`; the spawner `0x2e94d8`)
//! | address | what | port |
//! |---|---|---|
//! | 0x2e94d8 | `CreateMoby(0x558)`: distances 0xff, drawn, state 0, the turret's light word / ambient, position; +0x40 speed, +0x10 target, +0x38 = `ticks(600)`, +0x3c the turret; velocity = unit(target − position)·speed; rot.z / rot.y toward it | [`shoot`] |
//! | 0 | draw distance 0, not drawn; Ratchet's body point (0x13f420, 0.5 lower) within 8 (xy) of the target → the target = it + (target − it)·0.99; more than 4 (xy) from it and within 45° of rot.z → the velocity re-aimed at it | [`shot_update`] |
//! | 0 | back = velocity·−20; ahead = position + velocity; `0x26fdd0` type-80 glows at ahead (kind 1), at the middle of ahead and the position (kind 1), at ahead (kind 0), k = (60 − timer)/4 below 60; the template (`0x26e808(6, tmpl, m, 1, velocity)`, its class); position = ahead | [`shot_update`] (`World::part80`) |
//! | 0 | `CollLine_Fix(position + back (+0.25 z), ahead, 0, m, tmpl)` or `coll_sphere(0.5, position, 0, m)`: the world more than 10 (xy) from the turret → the hit point, +0x38 = `ticks(30)`, → 1; a moby but the turret: Ratchet hit on a magnetic floor → help record 0x8a armed (count 0 → 1, the time / mask); the hit point; `0x26e830(0.75, 6, 0, m, position, 1, 0, 1, 0)`; +0x38 = `ticks(30)`, → 1 | [`shot_update`] (`attack::sphere_hit`, `hints::touch`) |
//! | 0 | past `ticks(70)` outside [80, 890]³ → +0x38 = `ticks(70)`; `FastDecTimer(+0x38)` out or outside [10, 968]³ → `DeleteMoby` | [`shot_update`] |
//! | 1 | `PartType60Spawn(3, position, polar(randf(2·dt, 10·dt), randf(−π, π), randf(−π, π)), 0x60ff3020, 15, randi(255), 0)`; `FastDecTimer(+0x38)` out → `DeleteMoby` | [`shot_update`] (`World::part60`) |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::fx::{self, polar};
use crate::moby_update::creature::turn::turn_toward;
use crate::moby_update::creature::{self as c, add, atan, diff_rots, dist2, flash, set_len3, sub, DT, DT2};
use crate::moby_update::services::{pf as to_pf, pv, HitTemplate, World};
use crate::moby_update::{scheduler, story};

pub const REFERENCE_LEVEL: u32 = 17;
pub const UPDATE_FN: u32 = 0x2c_b310;
pub const CLASS: i16 = 0x15b;
pub const CLASSES: [i16; 1] = [CLASS];
pub const SHOT_FN: u32 = 0x2e_8e08;
pub const SHOT: i16 = 0x558;
pub const SHOT_CLASSES: [i16; 1] = [SHOT];

pub mod pvo {
    pub const RECORD: usize = 0x20;
    pub const FLASH: usize = 0x60;
    pub const CUBOID: usize = 0x70;
    pub const PHASE: usize = 0x74;
    pub const YAW_V: usize = 0x78;
    pub const FIGHTERS: usize = 0x7c;
    pub const ESCORTS: usize = 0x80;
    pub const ESCORT_PATH: usize = 0x84;
    pub const LOOK: usize = 0x90;
    pub const LEN: usize = 0x110;
}

pub mod shot_pvo {
    pub const VEL: usize = 0x00;
    pub const TARGET: usize = 0x10;
    pub const TIMER: usize = 0x38;
    pub const OWNER: usize = 0x3c;
    pub const SPEED: usize = 0x40;
    pub const LEN: usize = 0x44;
}

/// The fleet's ship, the record's class while it flies.
const SHIP: i16 = 0x563;
/// The ship's laser and missile: the only attackers that hurt a turret.
const HURT_BY: [i16; 2] = [0x3f1, 0x127];
const HEALTH: f32 = 15.0;
const DEG2: f32 = 0.034_906_585;
/// The muzzle glint: `PartType27Spawn(0x4999cf00, …, 0x80ff8f6f, 3)`, byte 9 `trunc(4) + 0x70`.
const GLINT_SIZE: f32 = f32::from_bits(0x4999_cf00);
const GLINT_RGBA: u32 = 0x80ff_8f6f;
/// Help record 0x8a (0x141dc0): Ratchet hit by a turret on the hull.
const HELP_HULL: usize = 0x8a;

fn pi(w: &World, id: MobyId, o: usize) -> i32 { c::pi32(w, id, o) }
fn seti(w: &mut World, id: MobyId, o: usize, v: i32) { c::set_pi32(w, id, o, v) }
fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> { usize::try_from(pi(w, id, o) - 1).ok().filter(|&m| m < w.table.mobys.len()) }

/// Ratchet flying the fleet's ship (hero state 0x32, the record's moby of class 0x563).
fn flying(w: &World) -> Option<MobyId> {
    w.svc.vehicle.moby.filter(|&s| w.hero.state == 0x32 && s < w.table.mobys.len() && w.m(s).o_class == SHIP)
}

fn mag_floor(w: &World) -> bool { w.hero.f658 != 0 }

/// Level17 `0x2cb310` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, pvo::LEN);
    hits(w, id);
    match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            c::set_pi16(w, id, pvo::RECORD + 4, 0xf);
            c::set_pf(w, id, pvo::RECORD, HEALTH);
            place_escorts(w, id);
            if story::mission_done(w, w.m(id).mission as i32) { w.delete_moby(id); }
        }
        1 => {
            if !mag_floor(w) {
                if flying(w).is_some() && dist2(w.m(id).position, super::hero_pos(w)) < 254.0 { w.mm(id).state = 3; }
            } else if let Some((centre, _)) = story::cuboid(w, pi(w, id, pvo::CUBOID)) {
                let p = w.m(id).position;
                let from = [p[0], p[1], p[2] + 10.0, p[3]];
                let mut to: [f32; 4] = std::array::from_fn(|k| if k < 3 { (centre[k] - from[k]) * 0.9 + from[k] } else { from[3] });
                to[2] += 0.5;
                if w.coll_line(pv(from), pv(to), 2, Some(id)).is_none() { w.mm(id).state = 2; }
            }
        }
        2 => {
            if !mag_floor(w) { w.mm(id).state = 1; }
            let (j0, j1) = (w.joint_point(id, 0), w.joint_point(id, 1));
            let Some(s) = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, pi(w, id, pvo::CUBOID)) else { return };
            let (m, centre) = (s.matrix, s.centre());
            let mut pt = [centre[0], centre[1], centre[2], 0.0];
            for row in m.iter().take(3) {
                let r = w.rng.randf(0.0, 1.0);
                for k in 0..3 { pt[k] += row[k] * r; }
            }
            let d = sub(pt, j0);
            let err = aim(w, id, d);
            let n = w.ticks(0x5a).max(1);
            if (w.counter % n as u64) as i32 == pi(w, id, pvo::PHASE) && err < DEG2 {
                let yaw_to = atan(centre[0] - w.m(id).position[0], centre[1] - w.m(id).position[1]);
                if diff_rots(w.m(id).rotation[2], yaw_to) < DEG2 {
                    glint(w, add(j1, set_len3(d, DT * 60.0)));
                    w.play_sound(0, 0, id);
                    shoot(w, DT * 60.0, id, j1, [centre[0], centre[1], centre[2], 0.0]);
                }
            }
        }
        3 => {
            let Some(ship) = flying(w) else {
                w.mm(id).state = 1;
                return;
            };
            let (j0, j1) = (w.joint_point(id, 0), w.joint_point(id, 1));
            let t = crate::moby_update::creature::target::acquire(w, id, 512.0);
            let d = sub(t.pos, j0);
            let err = aim(w, id, d);
            if c::dec_timer_pvar_i32(w, id, pvo::PHASE) != 0 && err < DEG2 {
                let p = w.m(id).position;
                let yaw_to = atan(t.pos[0] - p[0], t.pos[1] - p[1]);
                if diff_rots(w.m(id).rotation[2], yaw_to) < DEG2 && w.coll_line(pv(j1), pv(t.pos), 2, Some(ship)).is_none() {
                    let along = add(set_len3(sub(j1, j0), 250.0), j1);
                    glint(w, j1);
                    let n = w.ticks(0x5a) / 2;
                    seti(w, id, pvo::PHASE, n);
                    w.play_sound(0, 0, id);
                    shoot(w, DT * 60.0, id, j1, along);
                }
            }
            let g = pi(w, id, pvo::FIGHTERS);
            if g != 0 && w.rng.randi(500) == 0 { super::ship_fighter::launch(w, g, super::ship_fighter::FLEET_CLASS); }
        }
        4 => {
            let (p, rot) = (w.m(id).position, w.m(id).rotation);
            for k in [0x63a, 0x63b] { fx::break_piece(w, id, k, p, rot, 0, 0); }
            fx::death_explosion(w, 10.0, 13.0, Some(id), p, -1);
            w.delete_moby(id);
        }
        _ => {}
    }
}

/// The turn and the barrel toward `d` (states 2 / 3); the barrel's error from its pitch target.
fn aim(w: &mut World, id: MobyId, d: [f32; 4]) -> f32 {
    let (mut a, mut v) = (w.m(id).rotation[2], c::pf(w, id, pvo::YAW_V));
    turn_toward(atan(d[0], d[1]), DT2 * std::f32::consts::FRAC_PI_6, DT2 * std::f32::consts::FRAC_PI_6, DT * std::f32::consts::FRAC_PI_3, &mut a, &mut v);
    w.mm(id).rotation[2] = a;
    c::set_pf(w, id, pvo::YAW_V, v);
    let xy = (d[0] * d[0] + d[1] * d[1]).sqrt();
    let pitch = atan(xy, d[2]);
    use crate::moby_update::manip::rec;
    c::set_pf(w, id, pvo::LOOK + rec::TARGET + 4, -pitch);
    crate::moby_update::manip::look(w, id, id, pvo::LOOK, 0, 0.02, 0.3);
    diff_rots(c::pf(w, id, pvo::LOOK + rec::ANGLES + 4), -pitch)
}

/// The muzzle glint (module doc).
fn glint(w: &mut World, at: [f32; 4]) {
    *w.svc.fx.part_spawns.entry(27).or_default() += 1;
    let Some(sys) = w.particles.as_deref_mut() else {
        w.rng.rand();
        return;
    };
    match crate::particles::type27::spawn_rng(sys, w.rng, GLINT_SIZE, at, [0.0; 4], GLINT_RGBA, 3) {
        Some(i) => sys.pool.recs[i][9] = 4 + 0x70,
        None => w.svc.fx.part_failed += 1,
    }
}

/// Level17 `0x2cbc10` (module doc).
fn hits(w: &mut World, id: MobyId) {
    if w.m(id).state == 0 { return; }
    let hit = w.get_hit(id, 0x33_0000, false);
    let kept = hit.filter(|h| h.attacker.is_none_or(|a| a >= w.table.mobys.len() || HURT_BY.contains(&w.m(a).o_class)));
    let r = c::damage::resolve(w, id, kept, pvo::RECORD, 0, 4);
    if r.out5 != 1 && w.m(id).state != 4 {
        let hp = c::pf(w, id, pvo::RECORD) - r.damage;
        c::set_pf(w, id, pvo::RECORD, hp);
        if hp <= 0.0 {
            w.mm(id).state = 4;
            w.mm(id).mode &= !mode::TARGETABLE;
        }
        c::set_pu8(w, id, pvo::FLASH + 7, 0xf0);
        flash::start(w, id, pvo::FLASH);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pvo::FLASH);
}

/// Level17 `0x2cbd38` (module doc).
fn place_escorts(w: &mut World, id: MobyId) {
    let g = c::pi16(w, id, pvo::ESCORTS);
    let Ok(g) = i8::try_from(g) else { return };
    if g < 0 { return; }
    let list: Vec<MobyId> = scheduler::group_ids(w, g).into_iter().filter(|&m| m < w.table.mobys.len() && w.m(m).o_class == super::ship_fighter::FLEET_CLASS).collect();
    if list.is_empty() { return; }
    let path_i = pi(w, id, pvo::ESCORT_PATH);
    let pts: Vec<[f32; 4]> = usize::try_from(path_i).ok().and_then(|i| w.svc.splines.get(i)).map_or(Vec::new(), |p| p.iter().map(|q| q.map(f32::from_bits)).collect());
    let step = (pts.len() as i32 / list.len() as i32) as i16 as i32;
    use super::ship_fighter::pvo as fp;
    for (j, m) in list.into_iter().enumerate() {
        let i = j as i32 * step;
        let q = if pts.is_empty() { [0.0; 4] } else { pts[i.rem_euclid(pts.len() as i32) as usize] };
        story::pvars(w, m, fp::LEN);
        w.mm(m).position = q;
        seti(w, m, fp::POINT, i);
        seti(w, m, fp::PATH, path_i);
        c::set_pf(w, m, fp::T, 0.0);
    }
}

/// Level17 `0x2e94d8(speed, turret, pos, target)` (module doc).
fn shoot(w: &mut World, speed: f32, owner: MobyId, pos: [f32; 4], target: [f32; 4]) -> Option<MobyId> {
    let id = w.create_moby(SHOT)?;
    story::pvars(w, id, shot_pvo::LEN);
    let (light, ambient) = (w.m(owner).light, w.m(owner).ambient);
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
        m.visible = 1;
        m.state = 0;
        m.light = light;
        m.ambient = ambient;
        m.position = pos;
    }
    c::set_pf(w, id, shot_pvo::SPEED, speed);
    c::set_pv4(w, id, shot_pvo::TARGET, target);
    let t = w.ticks(600);
    seti(w, id, shot_pvo::TIMER, t);
    seti(w, id, shot_pvo::OWNER, owner as i32 + 1);
    c::set_pv4(w, id, shot_pvo::VEL, set_len3(sub(target, pos), speed));
    let m = w.mm(id);
    m.rotation[2] = atan(target[0] - pos[0], target[1] - pos[1]);
    m.rotation[1] = -atan(dist2(pos, target), target[2] - pos[2]);
    Some(id)
}

fn out_of(p: [f32; 4], lo: f32, hi: f32) -> bool { p.iter().take(3).any(|&x| x < lo || hi < x) }

/// Level17 `0x2e8e08`: the bolt (module doc).
pub fn shot_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, shot_pvo::LEN);
    match w.m(id).state {
        0 => {}
        1 => {
            let s = w.rng.randf(DT + DT, DT * 10.0);
            let a = w.rng.randf(-std::f32::consts::PI, std::f32::consts::PI);
            let b = w.rng.randf(-std::f32::consts::PI, std::f32::consts::PI);
            let v = polar(s, a, b);
            let rot = w.rng.randi(0xff) as u8;
            let p = w.m(id).position;
            w.part60(3.0, p, v, 0x60ff_3020, 0xf, rot, 0);
            if c::dec_timer_pvar_i32(w, id, shot_pvo::TIMER) != 0 { w.delete_moby(id); }
            return;
        }
        _ => return,
    }
    {
        let m = w.mm(id);
        m.draw_dist = 0;
        m.visible = 0;
    }
    let body = crate::moby_update::services::fv(w.hero.body_point);
    let body = [body[0], body[1], body[2] - 0.5, 0.0];
    let mut target = c::pv4(w, id, shot_pvo::TARGET);
    if dist2(body, target) < 8.0 {
        target = std::array::from_fn(|k| body[k] + (target[k] - body[k]) * f32::from_bits(0x3f7d_70a4));
        c::set_pv4(w, id, shot_pvo::TARGET, target);
    }
    let pos = w.m(id).position;
    if 4.0 < dist2(pos, target) && diff_rots(w.m(id).rotation[2], atan(target[0] - pos[0], target[1] - pos[1])) < std::f32::consts::FRAC_PI_4 {
        let sp = c::pf(w, id, shot_pvo::SPEED);
        c::set_pv4(w, id, shot_pvo::VEL, set_len3(sub(target, pos), sp));
    }
    let vel = c::pv4(w, id, shot_pvo::VEL);
    let back = vel.map(|x| x * -20.0);
    let ahead = add(pos, vel);
    let mut tail = add(pos, back);
    let mid: [f32; 4] = std::array::from_fn(|k| ahead[k] + (pos[k] - ahead[k]) * 0.5);
    let timer = c::pi32(w, id, shot_pvo::TIMER);
    let k = if timer < 0x3c { (0x3c - timer) >> 2 } else { 0 };
    w.part80(ahead, 1, k);
    w.part80(mid, 1, k);
    w.part80(ahead, 0, k);
    tail[2] += 0.25;
    let tmpl = HitTemplate { dir: pv(vel), attacker: Some(id), flags: 1, b18: 0, b19: 0, h1a: SHOT as u16, damage: to_pf(6.0), w20: 0 };
    w.mm(id).position = ahead;
    let owner = link(w, id, shot_pvo::OWNER);
    let hit = crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, pv(tail), pv(ahead), 0, Some(id), &tmpl)
        .or_else(|| w.coll_sphere(pv(ahead), to_pf(0.5), 0, Some(id)));
    if let Some(h) = hit {
        let at = [h.point[0], h.point[1], h.point[2], ahead[3]];
        match h.moby {
            None => {
                let far = owner.is_none_or(|o| 10.0 < dist2(ahead, w.m(o).position));
                if far {
                    w.mm(id).position = at;
                    boom(w, id);
                    return;
                }
            }
            Some(m) if Some(m) != owner => {
                let hull = mag_floor(w) && Some(m) == w.hero_moby;
                if hull && w.svc.help.records.help[HELP_HULL].count == 0 {
                    w.svc.help.records.help[HELP_HULL].count = 1;
                    super::hints::touch(w, HELP_HULL);
                }
                w.mm(id).position = at;
                crate::moby_update::creature::attack::sphere_hit(w, 0.75, 6.0, 0.0, id, at, 1, 0, 1, 0);
                boom(w, id);
                return;
            }
            Some(_) => {}
        }
    }
    let p = w.m(id).position;
    if w.ticks(0x46) < timer && out_of(p, 80.0, 890.0) {
        let t = w.ticks(0x46);
        seti(w, id, shot_pvo::TIMER, t);
    }
    if c::dec_timer_pvar_i32(w, id, shot_pvo::TIMER) != 0 || out_of(p, 10.0, 968.0) { w.delete_moby(id); }
}

/// The burst: `ticks(30)`, → 1.
fn boom(w: &mut World, id: MobyId) {
    let t = w.ticks(0x1e);
    seti(w, id, shot_pvo::TIMER, t);
    w.mm(id).state = 1;
}
