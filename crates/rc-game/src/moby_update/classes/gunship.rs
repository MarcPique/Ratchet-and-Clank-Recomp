//! The Blarg gunship 688 (`GunshipUpdate` 0x2f7728), its shells 686 (spawn `0x2f68a0`, update `0x2f6a30`), the fires
//! they light, 700 (`ImpactSmokeEmitterUpdate` 0x2f8c58), and the burning bits the fires drop, 696–698 (spawn
//! `SpawnDebrisMoby` 0x2f8530, update `0x2f8718`). Level01 only (cluster hashes: one program each). Spec
//! `docs/plan/creatures.md` "Gunship 688".
//!
//! **The gunship** flies its path with the flyer driver ([`flyer::driver`], `FlyerPathDriver` 0x2f5168; path P+0x74,
//! speed doubled at the init) and bombards: its fire nodes P+0x180 are the path points with w = 1 (at most four). On
//! reaching a fire node other than the last one it fired from (P+0x16c) it takes that node's target cuboids
//! (P+0x190 + 16·k, up to four), does nothing while the camera is inside cuboid P+0x188, drops (at most) one target
//! whose centre the camera cannot see (`CollLine_Fix(camera, centre, 0x86)`, from the last), picks one with
//! `randi(n)`, and fires a shell from joint list 0 or 1 (alternating, P+0x184) at the centre at 160 units/s. The
//! fires P+0x1d0 (eight 700s) that are idle, and whose cuboid holds both Ratchet and the camera, are handed to the
//! shell (a `randi(100)` each). Scale and speed shrink with the camera distance as the flyer's (0.5 at 200), and the
//! alpha fades above z 125.
//!
//! **The scripted fly-by** (P+0x170 ≠ −1: gunship 695 over the bridge). Hidden and still until Ratchet enters cuboid
//! P+0x170; then, unless the fly-by was seen before (`0x1bbb04[id]` or its spawn id's persistent death bit `0x14c190`:
//! the troopers P+0x1f0 are woken at once and the gunship deleted), it shows up, and with a camera cuboid P+0x174 it starts the sequence: Ratchet's state
//! 0x72, the camera script on that cuboid, `0x15f404` (`creature::Globals::cutscene`) set, no collision, not
//! targetable, timer P+0x186 = 510 ticks. The first 120 ticks of the timer run in the first tick (the update loops
//! until the timer is at 390: the ship jumps ahead along its path). It fires as above (the cutscene shells hit
//! mobys: the bridge pieces 709–711); with 60 ticks left it wakes the troopers P+0x1f0 (+0x1e8 = 1, +0x1ec = 2
//! ticks: they drop in); at the end the camera goes back to Ratchet (`CameraScript2(0)`, `SetState(0, 1)`), the flag
//! clears, the spawn id's persistent death bit and this visit's are set and the gunship is deleted. The camera and hero calls are
//! queued in `creature::Globals::scripts` ([`ScriptRequest`]) for the cutscene system and the hero.
//!
//! **A hit** (mask 0x800000; no Novalis weapon has it): `SpawnBeamExplosion(0, 0, 4, 2, 100000, 3, 15, …, 20 streaks,
//! 3 sparks, 4 puffs, sound 1, shake, 1 debris)`, the links P+0x140.. and the ship deleted; skill point 0x13d408 with level sound 1 and banner 0x53d6
//! (`story::award_skill_point`). The wreck 1510 of `0x30be70`: `classes::burning_wreck`.

use super::flyer;
use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::debris::flash_spawn;
use crate::moby_update::creature::projectile;
use crate::moby_update::creature::{self as c, fx, ScriptRequest};
use crate::moby_update::services::{pf as to_pf, pv, pvar, reflect, World};
use crate::particles::{type04, type15};
use std::f32::consts::PI;

pub const UPDATE_FN: u32 = 0x2f7728;
pub const CLASSES: [i16; 1] = [688];
pub const SHELL_FN: u32 = 0x2f6a30;
pub const SHELL_CLASS: i16 = 0x2ae;
pub const SHELL_CLASSES: [i16; 1] = [SHELL_CLASS];
pub const FIRE_FN: u32 = 0x2f8c58;
pub const FIRE_CLASS: i16 = 700;
pub const FIRE_CLASSES: [i16; 1] = [FIRE_CLASS];
pub const EMBER_FN: u32 = 0x2f8718;
/// `0x161c08`: the burning bits a fire drops (`randi(3)`).
pub const EMBER_CLASSES: [i16; 3] = [696, 697, 698];
/// 0x161c18: the fire smoke's texture pick (`randi(3)`).
const SMOKE_TEX: [i32; 3] = [0, 1, 2];
/// The trooper class the gunship wakes (459).
const TROOPER: i16 = 0x1cb;

/// gp−0x5030 / −0x502c / −0x5028 (0x161bd0..): the distance factor's minimum, far and near distances.
const SCALE_MIN: f32 = 0.5;
const SCALE_FAR: f32 = 200.0;
const SCALE_NEAR: f32 = 20.0;

const P_NODE: usize = 0x60;
const P_SPLINE: usize = 0x74;
const P_SPEED: usize = 0xfc;
const P_LINKS: usize = 0x140;
const P_FIRED: usize = 0x16c;
const P_TRIGGER: usize = 0x170;
const P_CAMERA: usize = 0x174;
const P_SCALE0: usize = 0x178;
const P_SPEED0: usize = 0x17c;
const P_NODES: usize = 0x180;
const P_JOINT: usize = 0x184;
const P_TIMER: usize = 0x186;
const P_NO_FIRE: usize = 0x188;
const P_HERO_CUB: usize = 0x18c;
const P_TARGETS: usize = 0x190;
const P_FIRES: usize = 0x1d0;
const P_TROOPERS: usize = 0x1f0;

fn cam(w: &World) -> c::V { w.camera.map(|x| f32::from_bits(x.0)) }
fn hero_pos(w: &World) -> c::V { w.hero.pos.map(|x| f32::from_bits(x.0)) }
fn in_cuboid(w: &World, p: c::V, i: i32) -> bool { w.in_cuboid([p[0], p[1], p[2]], i) }
fn cuboid(w: &World<'_>, i: i32) -> Option<rc_formats::volumes::Shape> { w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, i).copied() }
fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    usize::try_from(c::pi32(w, id, o)).ok().filter(|&m| m < w.table.mobys.len())
}

/// `GunshipUpdate` 0x2f7728 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x200 { return; }
    let d = c::dist3(c::pos(w, id), cam(w));
    let k = -((1.0 - SCALE_MIN) / (SCALE_FAR - SCALE_NEAR));
    let f = (k * d + (1.0 - k * SCALE_NEAR)).clamp(SCALE_MIN, 1.0);
    let hit = w.get_hit(id, 0x80_0000, false);
    w.mm(id).hit_slot = 0xff;
    if hit.is_some() {
        kill(w, id, f);
        return;
    }
    let loop_end = w.ticks(0x1fe) - w.ticks(0x78);
    loop {
        if w.m(id).state == 0 { init(w, id); }
        {
            let m = w.mm(id);
            m.scale = pvar::ff(&m.pvars, P_SCALE0) * f;
            let v = pvar::ff(&m.pvars, P_SPEED0) * f;
            pvar::set_ff(&mut m.pvars, P_SPEED, v);
            let z = m.position[2];
            m.alpha = if 175.0 <= z { 0 } else if 125.0 < z { (((175.0 - z) * 128.0) / 50.0) as i32 as u8 } else { 0x80 };
        }
        if c::pi32(w, id, P_TRIGGER) == -1 {
            flyer::driver(w, id);
        } else {
            if w.m(id).state == 0 { flyer::driver(w, id); }
            let mut started = false;
            if c::pi16(w, id, P_TIMER) == -1 {
                if in_cuboid(w, hero_pos(w), c::pi32(w, id, P_TRIGGER)) {
                    if seen(w, id) {
                        wake_troopers(w, id, true);
                        w.delete_moby(id);
                        return;
                    }
                    w.mm(id).mode &= !0x41;
                    let camc = c::pi32(w, id, P_CAMERA);
                    if camc != -1 {
                        let t = w.ticks(0x1fe);
                        if let Some(s) = cuboid(w, camc) {
                            let (cc, e) = (s.centre(), s.euler);
                            w.svc.creatures.scripts.push(ScriptRequest::Start { moby: id, hero_state: 0x72, cuboid: camc, centre: cc, euler: e, ticks: t });
                        }
                        w.svc.creatures.cutscene = true;
                        let m = w.mm(id);
                        m.has_collision = false;
                        m.mode &= !mode::TARGETABLE;
                        c::set_pi16(w, id, P_TIMER, t as i16);
                    }
                    started = true;
                } else {
                    w.mm(id).mode |= 0x41;
                    return;
                }
            }
            if !started {
                if c::dec_timer_pvar_s16(w, id, P_TIMER) != 0 {
                    w.svc.creatures.scripts.push(ScriptRequest::End { moby: id });
                    w.svc.creatures.cutscene = false;
                    set_seen(w, id);
                    w.delete_moby(id);
                    return;
                }
                if c::pi16(w, id, P_TIMER) as i32 == w.ticks(0x3c) { wake_troopers(w, id, false); }
                flyer::driver(w, id);
            }
        }
        w.mm(id).mode |= mode::TARGETABLE;
        c::set_pu8(w, id, 0x2b, 1);
        c::set_pu8(w, id, 0x2c, 0x2f);
        if !volley(w, id) { return; }
        if (c::pi16(w, id, P_TIMER) as i32) <= loop_end { return; }
    }
}

/// The state-0 block: the speed doubled, a closed path's last point dropped (the shared spline), the fire nodes, the
/// bases of the distance factor.
fn init(w: &mut World, id: MobyId) {
    let sp = c::pi32(w, id, P_SPLINE);
    if let Some(s) = usize::try_from(sp).ok().filter(|&s| s < w.svc.splines.len()) {
        let v = c::pf(w, id, P_SPEED);
        c::set_pf(w, id, P_SPEED, v + v);
        let pts = &w.svc.splines[s];
        if let (Some(a), Some(b)) = (pts.first(), pts.last()) {
            if c::dist3(a.map(f32::from_bits), b.map(f32::from_bits)) < 0.5 { w.svc.splines[s].pop(); }
        }
        let wk = |w: &World, i: usize| f32::from_bits(w.svc.splines[s][i][3]);
        let n = w.svc.splines[s].len();
        let mut k = 0usize;
        for i in 0..n {
            if wk(w, i) == 1.0 {
                c::set_pu8(w, id, P_NODES + k, i as u8);
                k += 1;
                if 3 < k { break; }
            }
        }
        c::set_pi16(w, id, P_JOINT, 0);
        c::set_pi32(w, id, P_FIRED, -1);
        c::set_pi16(w, id, P_TIMER, -1);
        let sc = w.m(id).scale;
        let v = c::pf(w, id, P_SPEED);
        c::set_pf(w, id, P_SPEED0, v);
        c::set_pf(w, id, P_SCALE0, sc);
    } else {
        w.svc.unported("gunship 688: no path");
    }
    let m = w.mm(id);
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
}

/// The fly-by was seen before: `g_spawn_id_flags` `0x1bbb04[id]` ≠ 0 or the spawn id's persistent death bit
/// (`0x14c190 + L·0x100`), which the end of the fly-by sets: a later visit wakes the troopers and deletes the ship.
fn seen(w: &World, id: MobyId) -> bool {
    let sid = w.m(id).spawn_id;
    sid >= 0 && (w.svc.save.collected.get(&sid).is_some_and(|&f| f != 0) || w.svc.save.death.contains(&(w.svc.level, sid)))
}
/// The end of the fly-by (0x2f7728): the spawn id's persistent death bit (`0x14c190`) and this visit's (`0x1ba950`).
fn set_seen(w: &mut World, id: MobyId) {
    let sid = w.m(id).spawn_id;
    if sid < 0 { return; }
    let lvl = w.svc.level;
    w.svc.save.death.insert((lvl, sid));
    w.svc.save.death_level.insert(sid);
}

/// The troopers P+0x1f0 (class 459) drop in: `+0x1e8 = 1`, `+0x1ec = ticks(2)`.
fn wake_troopers(w: &mut World, id: MobyId, alive_only: bool) {
    for k in 0..4 {
        let Some(t) = link(w, id, P_TROOPERS + 4 * k) else { continue };
        let m = w.m(t);
        if m.o_class != TROOPER || (alive_only && m.is_deleted()) || m.pvars.len() < 0x1f0 { continue; }
        c::set_pi32(w, t, 0x1e8, 1);
        let h = w.ticks(2);
        c::set_pi16(w, t, 0x1ec, h as i16);
    }
}

/// The fire logic at a fire node (module doc). False when the update ends here.
fn volley(w: &mut World, id: MobyId) -> bool {
    let node = c::pi32(w, id, P_NODE);
    if c::pi32(w, id, P_FIRED) == node { return true; }
    let mut slot = None;
    for k in 0..4 {
        if c::pu8(w, id, P_NODES + k) as i32 == node { slot = Some(k); }
    }
    let Some(k) = slot else { return true };
    let mut a: Vec<i32> = (0..4).map(|i| c::pi32(w, id, P_TARGETS + 16 * k + 4 * i)).filter(|&t| t != -1).collect();
    if a.is_empty() { return false; }
    if in_cuboid(w, cam(w), c::pi32(w, id, P_NO_FIRE)) { return false; }
    let mut b: Vec<i32> = a.clone();
    let mut left = a.len();
    for i in (0..a.len()).rev() {
        let Some(cc) = cuboid(w, a[i]).map(|s| s.centre()) else { continue };
        if let Some(h) = w.line(w.camera, pv([cc[0], cc[1], cc[2], 0.0]), 0x86, Some(id)) {
            if h.moby == Some(id) { continue; }
            b[i] = -1;
            left -= 1;
            break;
        }
    }
    if left != 0 { a = b.into_iter().filter(|&t| t != -1).collect(); }
    if a.is_empty() { return true; }
    let r = w.rng.randi(a.len() as i32) as usize;
    let tgt = a[r];
    let j = (c::pi16(w, id, P_JOINT) as u16 ^ 1) as i16;
    c::set_pi16(w, id, P_JOINT, j);
    let from = w.joint_point(id, j as usize);
    let Some(tc) = cuboid(w, tgt).map(|s| s.centre()) else { return true };
    let tc = [tc[0], tc[1], tc[2], 0.0];
    let v = c::set_len3(c::sub(tc, from), c::DT * 160.0);
    let hero_in = in_cuboid(w, hero_pos(w), c::pi32(w, id, P_HERO_CUB));
    if let Some(s) = spawn_shell(w, id, from, v, tc, hero_in) {
        c::set_pi32(w, id, P_FIRED, c::pu8(w, id, P_NODES + k) as i32);
        let mut n = 0usize;
        for e in 0..8 {
            let Some(f) = link(w, id, P_FIRES + 4 * e) else { continue };
            if w.m(f).o_class != FIRE_CLASS || w.m(f).pvars.len() < 0x10 { continue; }
            if c::pi16(w, f, 4) == 0 && c::pi32(w, f, 0) == 0 {
                let cub = c::pi32(w, f, 8);
                if w.rng.randi(100) < 100 && in_cuboid(w, hero_pos(w), cub) && in_cuboid(w, cam(w), cub) {
                    c::set_pi32(w, s, 0x20 + 4 * n, f as i32);
                    n += 1;
                }
            }
            if 4 <= n { break; }
        }
    }
    true
}

/// A hit (mask 0x800000): the explosion, the links and the ship deleted (module doc).
fn kill(w: &mut World, id: MobyId, f: f32) {
    crate::moby_update::story::award_skill_point(w, 0);
    fx::beam_explosion(w, &flyer::KILL_BEAM, Some(id), c::pos(w, id));
    // `0x30be70(2f, m, P+0xd0 − P+0xe0)`: the wreck 1510 (`classes::burning_wreck`).
    let dir = c::sub(c::pv4(w, id, 0xd0), c::pv4(w, id, 0xe0));
    super::burning_wreck::spawn(w, f + f, id, dir);
    for k in 0..4 {
        if let Some(l) = link(w, id, P_LINKS + 4 * k) { w.delete_moby(l); }
    }
    w.delete_moby(id);
}

// -------------------------------------------------------------------------------------------------
// The shell 686

/// `0x2f68a0(gunship, from, vel, target, hero_in, scripted)`: the shell moby (quarter scale, pitched and turned along
/// `vel`), pvars +0x00 velocity, +0x10 target, +0x20 four fires (the zeroed block's 0 until the gunship fills them), +0x30 the gunship, +0x34 the Ratchet-in-cuboid
/// flag, +0x38 speed; two glows (type 26: 420000 / 210000, 0x2f4f7f7f / 0x4f6f7f7f, `ticks(120)`); class sound 0.
pub fn spawn_shell(w: &mut World, ship: MobyId, from: c::V, v: c::V, target: c::V, hero_in: bool) -> Option<MobyId> {
    let m = w.create_moby(SHELL_CLASS)?;
    {
        let mo = w.mm(m);
        if mo.pvars.len() < 0x40 { mo.pvars.resize(0x80, 0); }
        mo.rotation[0] = 0.0;
        mo.update_dist = 0xff;
        mo.draw_dist = 0xff;
        mo.visible = 1;
        mo.scale *= 0.25;
        mo.rotation[1] = -c::atan(c::len2(v), v[2]);
        mo.rotation[2] = c::atan(v[0], v[1]);
        mo.position = from;
    }
    c::set_pv4(w, m, 0, v);
    c::set_pv4(w, m, 0x10, target);
    c::set_pi32(w, m, 0x30, ship as i32);
    c::set_pi32(w, m, 0x34, hero_in as i32);
    let l = c::len3(v);
    c::set_pf(w, m, 0x38, l);
    let t = w.ticks(0x78);
    projectile::part26(w, 420000.0, m, 0x2f4f_7f7f, t);
    let t = w.ticks(0x78);
    projectile::part26(w, 210000.0, m, 0x4f6f_7f7f, t);
    w.play_sound(0, 0, m);
    w.build_matrix(m);
    Some(m)
}

/// `0x2f6a30`: the shell's flight and impact. Each tick it moves by its velocity and trails 4 (2 in a scripted
/// sequence) pairs of type-4 smoke (11 draws a pair); at its target (within one step) it bursts: velocity ·= −0.55,
/// in a scripted sequence a radius-1 hit on the mobys around (flags 9, Ratchet ignored; template flags 0x810001) and
/// the sound 1 (2 for a bridge piece 709–711); up to 5 (2) type-11 spark pairs, 5 type-8 puffs, the flashes (two
/// more far from the camera), 40 (10) type-15 streaks; then the fires it carries start (`ticks(rand_range(60,
/// 180))`, outside a scripted sequence, when Ratchet and the camera are in their cuboid) and it is deleted. Below 0 on
/// any axis it vanishes.
pub fn shell_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x3c { return; }
    let scene = w.svc.creatures.cutscene;
    let div = if scene { 4 } else { 2 };
    let vel = c::pv4(w, id, 0);
    let p = c::add(c::pos(w, id), vel);
    c::set_pos(w, id, p);
    let spd = c::len3(vel);
    let pairs = if scene { 2 } else { 4 };
    for _ in 0..pairs {
        let x = w.rng.randf(-1.0, 1.0);
        let y = w.rng.randf(-1.0, 1.0);
        let z = w.rng.randf(-1.0, 1.0);
        let l = w.rng.randf(0.5, 1.0);
        let mut dv = c::set_len3([x, y, z, 0.0], l * c::DT);
        let b = w.rng.randf(c::DT * 0.1, c::DT);
        dv = c::add(dv, c::set_len3(vel, -b));
        let p1 = c::add(c::set_len3(vel, w.rng.randf(0.0, 1.0) * spd), p);
        let n = w.rng.rand_range(0x1e, 0x2d);
        let life = w.ticks(n) / div;
        let g = w.rng.rand_range(0x50, 0x8c) / div;
        let a = type04::Spawn { pos: p1, vel: dv, c1: 0x6f00_afff, c2: 0xff, life, base: 0x50, growth: g as i16, additive: true };
        fx::part04(w, &a);
        let p2 = c::add(c::set_len3(vel, w.rng.randf(0.0, 1.0) * spd), p);
        let n = w.rng.rand_range(0x3c, 0x78);
        let life = w.ticks(n) / div;
        let g = w.rng.rand_range(200, 300) / div;
        let a = type04::Spawn { pos: p2, vel: dv, c1: 0x1fff_ffff, c2: 0x004f_4f4f, life, base: 0x50, growth: g as i16, additive: false };
        fx::part04(w, &a);
    }
    if p[0] < 0.0 || p[1] < 0.0 || p[2] < 0.0 {
        w.delete_moby(id);
        return;
    }
    let target = c::pv4(w, id, 0x10);
    if c::dist3(target, p) < c::pf(w, id, 0x38) {
        c::set_pos(w, id, target);
        w.mm(id).state = 1;
    }
    if w.m(id).state == 0 { return; }
    impact(w, id, scene, div);
    w.delete_moby(id);
}

fn impact(w: &mut World, id: MobyId, scene: bool, div: i32) {
    let size = if scene { 4.0 } else { 8.0 };
    let pos = c::pos(w, id);
    let camd = c::dist3(cam(w), pos);
    let v = c::scale(c::pv4(w, id, 0), -0.55);
    c::set_pv4(w, id, 0, v);
    if scene {
        let t = projectile::template(w, id, [0.0; 4], 0x81_0001, (0, 0), 0.0, 0);
        let hero = w.hero_moby;
        let got = crate::moby_update::services::sphere_mobys_in(w.table, w.svc, w.classes, to_pf(1.0), pv(pos), 9, hero, Some(&t));
        let bridge = got.last().is_some_and(|&m| (0x2c5..=0x2c7).contains(&w.m(m).o_class));
        w.play_sound(if bridge { 2 } else { 1 }, 0, id);
    }
    let d2 = c::dist3(pos, cam(w));
    let mut n = if scene { 2 } else { 5 };
    if d2 < (n as f32) * 2.0 { n = (d2 as i32) / 2; }
    let near = if d2 < 7.0 { 7.0 - d2 } else { 0.0 };
    for _ in 0..n.max(0) {
        let sp = w.rng.randf(8.0, 10.0) * c::DT;
        let speed = (sp - near * c::DT) * size;
        let a = w.rng.randi(6) as usize;
        let b = w.rng.randi(6) as usize;
        let (t30, t40) = (w.ticks(0x1e), w.ticks(0x28));
        let life = w.rng.rand_range(t30, t40) / div;
        let (t60, t90) = (w.ticks(0x3c), w.ticks(0x5a));
        let t1 = w.rng.rand_range(t60, t90) / div;
        let sz = to_pf(size * 400000.0);
        w.part11(sz, to_pf(speed), pv(pos), pv(v), fx::SPARK_A[a], fx::SPARK_B[b], life, t1, 0, 8);
        let (t10, t20) = (w.ticks(10), w.ticks(0x14));
        let life = w.rng.rand_range(t10, t20) / div;
        let (t30, t40) = (w.ticks(0x1e), w.ticks(0x28));
        let t1 = w.rng.rand_range(t30, t40) / div;
        w.part11(sz, to_pf(speed * 0.5), pv(pos), pv(v), 0x7fff_ffff, 0x00ff_ffff, life, t1, 0, 8);
    }
    for _ in 0..5 {
        let x = w.rng.randf(-1.0, 1.0);
        let y = w.rng.randf(-1.0, 1.0);
        let z = w.rng.randf(-1.0, 1.0);
        let s = w.rng.randf(0.0, 3.0);
        let pv_ = c::set_len3([x, y, z, 0.0], size * s * c::DT);
        let a = w.rng.randi(6) as usize;
        let b = w.rng.randi(6) as usize;
        let (t15, t20) = (w.ticks(0xf), w.ticks(0x14));
        let life = w.rng.rand_range(t15, t20) / div;
        fx::part08(w, 200_000.0, pos, pv_, fx::SPARK_A[a], fx::SPARK_B[b], life);
    }
    let load = f32::from_bits(w.svc.frame_load[0].0);
    let flashes = |w: &mut World, big: f32, small: f32| {
        if load < 0.95 && 9.0 < d2 {
            let t = w.ticks(0xf);
            flash_spawn(w, big, id, pos, v, t, 0x7f, 0x7f, 0x7f, 0x20);
            let t = w.ticks(0x18);
            flash_spawn(w, big, id, pos, v, t, 0x7f, 0x7f, 0x7f, 0x20);
        }
        let t = w.ticks(0x14);
        flash_spawn(w, big, id, pos, v, t, 0x7f, 0x7f, 0, 0x30);
        let t = w.ticks(0x13);
        flash_spawn(w, small, id, pos, v, t, 0xff, 0xff, 0xff, 0x20);
    };
    flashes(w, size * 4.0, size + size);
    if 100.0 < camd { flashes(w, 30.0, 15.0); }
    let streaks = if scene { 10 } else { 40 };
    for _ in 0..streaks {
        let x = w.rng.randf(-1.0, 1.0);
        let y = w.rng.randf(-1.0, 1.0);
        let z = w.rng.randf(-1.0, 1.0);
        let mut s = w.rng.randf(10.0, 40.0) * c::DT;
        if scene { s *= 0.2; }
        let sv = c::add(c::set_len3([x, y, z, 0.0], s), v);
        let (t90, t150) = (w.ticks(0x5a), w.ticks(0x96));
        let life = w.rng.rand_range(t90, t150) / div;
        let a = type15::Spawn { size: if scene { 21000.0 } else { 105_000.0 }, pos, vel: sv, c1: 0x4f00_7fff, c2: 0x1f00_007f, life, split: 1, def: -1, blend: -1 };
        fx::part15(w, &a);
    }
    if !scene {
        for k in 0..4 {
            let Some(f) = usize::try_from(c::pi32(w, id, 0x20 + 4 * k)).ok().filter(|&f| f < w.table.mobys.len()) else { continue };
            if w.m(f).o_class != FIRE_CLASS || w.m(f).pvars.len() < 0x10 { continue; }
            if c::pi16(w, f, 4) != 0 || c::pi32(w, f, 0) != 0 { continue; }
            let cub = c::pi32(w, f, 8);
            if in_cuboid(w, hero_pos(w), cub) && in_cuboid(w, cam(w), cub) {
                let n = w.rng.rand_range(0x3c, 0xb4);
                let t = w.ticks(n);
                c::set_pi32(w, f, 0, t);
            }
        }
    }
}

// -------------------------------------------------------------------------------------------------
// The fire 700 and its embers 696–698

/// `ImpactSmokeEmitterUpdate` 0x2f8c58: pvars +0x00 burn timer (armed by a shell), +0x04 s16 re-arm lock (120 ticks
/// after a burn), +0x06 s16 ember lock, +0x08 cuboid, +0x0c smoke height. While burning: an ember (5 % a tick: a
/// burning bit 696–698 from the smoke height, `SpawnDebrisMoby`) and two type-16 smoke puffs a tick.
pub fn fire_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x10 { return; }
    c::dec_timer_pvar_s16(w, id, 4);
    c::dec_timer_pvar_s16(w, id, 6);
    if c::pi32(w, id, 0) == 0 { return; }
    if c::dec_timer_pvar_i32(w, id, 0) != 0 {
        let t = w.ticks(0x78);
        c::set_pi16(w, id, 4, t as i16);
    }
    let base = c::pos(w, id);
    let hz = c::pf(w, id, 0xc);
    if c::pi16(w, id, 6) == 0 && w.rng.randi(100) < 5 {
        let x = w.rng.randf(c::DT * -0.5, c::DT * 0.5);
        let y = w.rng.randf(c::DT * -0.5, c::DT * 0.5);
        let r = w.rng.randf(0.0, 0.25);
        let a = w.rng.rand_angle();
        let mut p = c::add([a.cos() * r, a.sin() * r, 0.0, 0.0], base);
        p[2] += hz;
        let cl = EMBER_CLASSES[w.rng.randi(3) as usize];
        let life = w.rng.rand_range(0xb4, 300);
        spawn_ember(w, 0.05, 1.0, 1.0, 0.75, p, [x, y, 0.0, 0.0], cl, life, 0);
    }
    for _ in 0..2 {
        let x = w.rng.randf(c::DT * -0.5, c::DT * 0.5);
        let y = w.rng.randf(c::DT * -0.5, c::DT * 0.5);
        let z = w.rng.randf(0.0, c::DT * 3.0);
        let r = w.rng.randf(0.0, 0.25);
        let a = w.rng.rand_angle();
        let mut pos = c::add([a.cos() * r, a.sin() * r, 0.0, 0.0], base);
        pos[2] += hz;
        let vel = [x, y, z + r * c::DT * 8.0, 0.0];
        let size = w.rng.randf(70000.0, 140000.0);
        let life = w.ticks(0xb4);
        let kind = SMOKE_TEX[w.rng.randi(3) as usize] as i16;
        let a = crate::particles::type16::Spawn { size, pos, vel, c1: 0x0f08_1020, c2: 0x08_1020, life, kind };
        if let Some(i) = projectile::part16(w, &a) {
            // The caller's floor: the fire's height − 0.5.
            let z = c::pos(w, id)[2] - 0.5;
            if let Some(r) = w.particles.as_deref_mut().and_then(|s| s.pool.recs.get_mut(i)) { crate::particles::rec::set_ff(r, 0x2c, z); }
        }
    }
}

/// `SpawnDebrisMoby(scale, g, spin, t, pos, vel, class, life, keep)` 0x2f8530: a burning bit (`class`) at `pos`
/// with Ratchet's light, `scale`× the class scale, a random Euler (3 × `randf(−π, π)`), pvars +0x00 velocity, +0x10
/// life, +0x14 radius (= `scale`), +0x18 spin `±randf(π/2·dt, 2π·dt)·spin` (the sign from `rand()`), +0x1c no-bounce
/// ticks `trunc(t·60)`, +0x20 `keep`, +0x24 gravity `g·9.8·dt²`, +0x28 life.
#[allow(clippy::too_many_arguments)]
pub fn spawn_ember(w: &mut World, scale: f32, g: f32, spin: f32, t: f32, p: c::V, v: c::V, class: i16, life: i32, keep: i32) -> Option<MobyId> {
    let m = w.create_moby(class)?;
    let rx = w.rng.randf(-PI, PI);
    let ry = w.rng.randf(-PI, PI);
    let rz = w.rng.randf(-PI, PI);
    let mut s = w.rng.randf(c::DT * std::f32::consts::FRAC_PI_2, c::DT * 2.0 * PI);
    if w.rng.rand() & 1 != 0 { s = -s; }
    let hl = w.hero_moby.map(|h| (w.m(h).light, w.m(h).ambient));
    let nb = w.svc.timing.scale(to_pf(t * 60.0)).to_f32() as i32;
    {
        let mo = w.mm(m);
        if let Some((l, a)) = hl {
            mo.light = l;
            mo.ambient = a;
        }
        if mo.pvars.len() < 0x2c { mo.pvars.resize(0x80, 0); }
        mo.draw_dist = 0xff;
        mo.update_dist = 0xff;
        mo.visible = 1;
        mo.scale *= scale;
        mo.rotation = [rx, ry, rz, 0.0];
        mo.position = p;
    }
    c::set_pv4(w, m, 0, v);
    c::set_pi32(w, m, 0x28, life);
    c::set_pf(w, m, 0x14, scale);
    c::set_pi32(w, m, 0x10, life);
    c::set_pf(w, m, 0x18, s * spin);
    c::set_pi32(w, m, 0x1c, nb);
    c::set_pi32(w, m, 0x20, keep);
    c::set_pf(w, m, 0x24, g * 9.8 * c::DT2);
    w.build_matrix(m);
    Some(m)
}

/// `0x2f8718`: a burning bit falls (gravity +0x24), spins about z (+0x18) and, once its no-bounce time +0x1c is out,
/// bounces off what the step or its sphere (+0x14) touches (reflected, ·`randf(0.07, 0.13)`, plus a random xy kick
/// up to 0.35 of its fall speed, a new spin and Euler; 13 draws); a kept bit (+0x20) fades its alpha over its life;
/// otherwise when it has come to rest (speed < 0.5·dt) or its life +0x10 is out it bursts into 40 type-22 smoke puffs
/// (10 draws each) and is deleted. Out of the world box it is deleted.
pub fn ember_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x2c { return; }
    let old = c::pos(w, id);
    let mut v = c::pv4(w, id, 0);
    v[2] -= c::pf(w, id, 0x24);
    c::set_pv4(w, id, 0, v);
    let p = c::add(old, v);
    c::set_pos(w, id, p);
    let y = c::add_rot(c::yaw(w, id), c::pf(w, id, 0x18));
    c::set_yaw(w, id, y);
    if !projectile::in_world(p) {
        w.delete_moby(id);
        return;
    }
    if c::dec_timer_pvar_i32(w, id, 0x1c) != 0 {
        let h = w.coll_line(pv(old), pv(p), 0, Some(id)).or_else(|| w.coll_sphere(pv(p), to_pf(c::pf(w, id, 0x14)), 0, Some(id)));
        if let Some(h) = h { bounce(w, id, h.point, h.normal); }
    }
    let keep = c::pi32(w, id, 0x20);
    if keep == 0 {
        if c::dec_timer_pvar_i32(w, id, 0x10) == 0 && (c::DT * 0.5 <= c::len3(c::pv4(w, id, 0)) || c::pi32(w, id, 0x1c) != 0) { return; }
        let pos = c::pos(w, id);
        let vel = c::pv4(w, id, 0);
        for _ in 0..40 {
            let d = [w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), 0.0];
            let o = [w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), 0.0];
            let r = w.rng.randf(0.0, 0.5);
            let at = c::add(to_len(o, r), pos);
            let s = w.rng.randf(0.0, 0.5);
            let v = c::add(to_len(d, s * c::DT), vel);
            let size = w.rng.randf(70000.0, 140000.0);
            let n = w.rng.rand_range(0x14, 0x3c);
            let life = w.ticks(n);
            projectile::part22(w, &crate::particles::type22::Spawn { size, pos: at, vel: v, c1: 0x1f0c_1820, c2: 0x08_1020, life });
        }
    } else if c::dec_timer_pvar_i32(w, id, 0x10) == 0 {
        let a = (c::pi32(w, id, 0x10) * 0x7f) / c::pi32(w, id, 0x28).max(1);
        w.mm(id).alpha = a as u8;
        return;
    }
    w.delete_moby(id);
}

/// `FastVecNormalize(len, v, v)`: `v` scaled to length `len` (zero stays zero; w kept).
fn to_len(v: c::V, len: f32) -> c::V {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l == 0.0 { return v; }
    let k = len / l;
    [v[0] * k, v[1] * k, v[2] * k, v[3]]
}

/// The ember's bounce (`LAB_002f8864`).
fn bounce(w: &mut World, id: MobyId, at: [f32; 3], n: [f32; 3]) {
    let v = c::pv4(w, id, 0);
    let a = c::atan(v[0], v[1]);
    let r1 = w.rng.randf(-PI, PI);
    let cx = c::add_rot(a, r1).cos();
    let x = w.rng.randf(0.0, v[2] * 0.35) * cx;
    let r2 = w.rng.randf(-PI, PI);
    let sy = c::add_rot(a, r2).sin();
    let y = w.rng.randf(0.0, v[2] * 0.35) * sy;
    let e = [w.rng.randf(-PI, PI), w.rng.randf(-PI, PI), w.rng.randf(-PI, PI)];
    w.mm(id).rotation = [e[0], e[1], e[2], 0.0];
    let nn = [n[0], n[1], n[2], 0.0];
    let r = crate::moby_update::services::fv(reflect(pv(v), pv(nn)));
    let k = w.rng.randf(0.07, 0.13);
    let nv = c::add(c::scale(r, k), [x, y, 0.0, 0.0]);
    c::set_pv4(w, id, 0, nv);
    let off = c::set_len3(nn, 0.1);
    c::set_pos(w, id, [at[0] + off[0], at[1] + off[1], at[2] + off[2], c::pos(w, id)[3]]);
    let mut s = w.rng.randf(c::DT * std::f32::consts::FRAC_PI_2, c::DT * 2.0 * PI);
    if 0.0 < c::pf(w, id, 0x18) { s = -s; }
    c::set_pf(w, id, 0x18, s);
}

