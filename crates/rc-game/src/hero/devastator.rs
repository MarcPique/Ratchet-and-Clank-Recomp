//! **The Devastator** (item 11, class 157; level01 item update `0x2c7d68` with its muzzle smoke `0x2c7758` / `0x2c7a48`,
//! the missile's spawn `0x2c5440`; read from the decompiler output). No weapon-check case: its update fires. Not a
//! persistent-arm weapon (def +0x30 = 0; standing shot sequence 54, the arm layers 73 on lists 12 and 13: def +0x18 =
//! 2). The missiles are class 153 (`crate::moby_update::classes::devastator_missile`).
//!
//! **Pvars** ([`Devastator`]): +0x00 the fire timer, +0x10 the lock, +0x14 the lock timer, +0x18 the lock's aim height,
//! +0x1c the point light, +0x1e its timer; the item's +0xbc the muzzle smoke's timer.
//!
//! **The update**: the muzzle smoke while +0xbc runs ([`muzzle_smoke`], `0x2c7758`: types 44 and 21); item state 0 →
//! 1 (light none); the fire timer steps; the lock is dropped when its timer runs out or it is being deleted (+0x20 bit
//! 7); the look stance 1 becomes 0x1e. **First person** (not 0x1413fc): a ray 70 ahead along the view
//! (`CollLine_Fix(eye, .., 0, item)`): a targetable moby hit is locked for `ticks(20)`, its aim height the hit's height
//! above it; the lock-on crosshair (marker FX 0x27, colour 0xff180a65, size 1, turning once every `ticks(240)`) at the
//! screen centre, and with a lock, the fire timer out and ammo the green marker over it (size `|0.9 − d/100·0.57|`,
//! turning the other way). **Firing** (not game mode 2): the fire timer out, not 0x1413fc, the item's key A before
//! sequence 2, ○ held and the item ready for more than `ticks(22)`: the item's sequence 0 blends to 1 (`ticks(4)`); the
//! aim 10·dt along the view (first person) or Ratchet's facing; no ammo → the empty click (class sound 1); else:
//! * the muzzle: first person 0.15 right, 0.3 down, 0.75 ahead of the eye (a lock's aim height kept), else the item's
//!   joint list 0 and **the search** [`search`] over the target list;
//! * class sound 0; the light's timer `ticks(5) + ticks(10)`; outside 0x1e the muzzle burst ([`muzzle_burst`],
//!   `0x2c7a48`: types 44 / 21) and +0xbc = `ticks(5)`;
//! * **the missile** `0x2c5440(yaw, pitch, aim height, moby yaw, moby pitch, item, muzzle, lock)` ([`fire_missile`]);
//! * Ratchet: standing, or walking slowly (state 2, stick < 0.7) → `SetAnim(ticks(10), 54, 1)`; else the weapon drawn
//!   (`0x22ee08`: the arm layers 73);
//! * the fire timer `ticks(35)`. The shot statistics (0x1416d8..) are not ported.
//!
//! **The light**: freed when the item's key A is on sequence 2 (put away) or its key B is; else while its timer runs a
//! point light of radius 7, one unit ahead of the muzzle (along Ratchet's facing) and 0.3 up: rising over `ticks(5)`
//! then fading over `ticks(10)`, colour `(k, 0.8k, 0.5k)`.
//!
//! **Native.** Standard `f32`; the item is not a table moby (the missiles' owner is Ratchet's moby). 0x140600 (a forced
//! target) is 0 [L]; with the arm raised at a wall (0x141618) the missile flies along the gun; gold (0x13e52b) is not mirrored.

use super::guns::{self, add3, len3, scale3, sub3};
use super::items::{HitSink, ItemEnv};
use super::physics::{from_f32x3, to_f32x3, ticks};
use super::Hero;
use crate::moby_runtime::{mode, MobyId, MobyTable};
use crate::moby_update::creature::{atan, diff_rots};
use crate::point_lights::PointLight;
use crate::rng::Rng;
use crate::targeting::{self, Marker};
use rc_formats::moby_anim;

pub const DEVASTATOR: i32 = 11;
pub const CLASS: i16 = 157;
pub const MARKER_FX: usize = 0x27;
pub const MARKER_LOCK: u32 = 0xff18_0a65;
/// The class sounds: the shot, the empty click.
pub const FIRE_SOUND: i32 = 0;
pub const EMPTY_SOUND: i32 = 1;
const DT: f32 = 1.0 / 60.0;

/// The Devastator's pvars (item moby +0x78) and its +0xbc.
#[derive(Clone, Debug, PartialEq)]
pub struct Devastator {
    pub fire_timer: i32,
    pub lock: Option<MobyId>,
    pub lock_timer: i32,
    pub aim_height: f32,
    pub light: i32,
    pub light_timer: i16,
    pub smoke: u8,
}

impl Default for Devastator {
    fn default() -> Self { Devastator { fire_timer: 0, lock: None, lock_timer: 0, aim_height: 0.0, light: -1, light_timer: 0, smoke: 0 } }
}

/// A live missile already holds `t` as its target (`FUN_00274738` over the missile list 0x1b0bf0): the Devastator's
/// (class 153) and, on levels 15 / 18, Giant Clank's (class 0x100, which joins the level's list 0x1b0d70 / 0x1b1170 too:
/// `classes::units::giant_missile`; the same pvar +0x18).
pub fn already_targeted(table: &MobyTable, t: MobyId) -> bool {
    use crate::moby_update::classes::devastator_missile as dm;
    use crate::moby_update::classes::units::giant_missile as gm;
    table.mobys.iter().any(|m| (m.o_class == dm::CLASS || m.o_class == gm::CLASS) && m.state < 0xfd && m.pvars.len() >= 0x20 && (i32::from_le_bytes(m.pvars[dm::pv::TARGET..dm::pv::TARGET + 4].try_into().unwrap()) as usize).checked_sub(1) == Some(t))
}

/// The search's result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Found {
    pub yaw: f32,
    pub pitch: f32,
    /// The best distance (10000 when none).
    pub best: f32,
    /// The moby that passed the cone (the last accepted), and the lock the search wrote (one no missile holds yet).
    pub found: Option<MobyId>,
    pub lock: Option<MobyId>,
    pub aim_height: Option<f32>,
}

/// The Devastator's search in `0x2c7d68` over the target list, from the muzzle with the aim `(yaw, pitch)`: live
/// targetable creatures (class type 5), their aim points (record height, 0.5 without one). Within 2.5 of the muzzle, a
/// creature within 60° of Ratchet's yaw and 45° of elevation is taken outright (the search ends; the aim turns to it).
/// Otherwise the nearest so far passing `crate::targeting::cone_miss` (the aim's pitch + half the ground pitch;
/// 10° + 40° per gold level) with a clear camera line (flags 6): the aim turns to it; when no missile holds it yet it
/// becomes the lock with its aim height.
#[allow(clippy::too_many_arguments)]
pub fn search(table: &MobyTable, list: &[MobyId], from: [f32; 3], yaw: f32, pitch: f32, ground: f32, hero_yaw: f32, hero_pos: [f32; 3], gold: u8, class_type: &dyn Fn(i16) -> Option<u8>, mut clear: impl FnMut([f32; 3]) -> bool) -> Found {
    let mut f = Found { yaw, pitch, best: 10000.0, found: None, lock: None, aim_height: None };
    let thr = gold as f32 * 0.698_131_7 + 0.174_532_92;
    for &id in list {
        let Some(m) = table.mobys.get(id) else { continue };
        if m.state >= 0x80 { continue; }
        let p = targeting::aim_point(m);
        if m.mode & mode::TARGETABLE == 0 || !m.has_class || class_type(m.o_class) != Some(5) { continue; }
        let ty = atan(p[0] - from[0], p[1] - from[1]);
        let d2 = ((p[0] - from[0]).powi(2) + (p[1] - from[1]).powi(2)).sqrt();
        let elev = atan(d2, p[2] - from[2]);
        let d = len3(sub3(p, from));
        if d < 2.5 {
            let to = atan(m.position[0] - hero_pos[0], m.position[1] - hero_pos[1]);
            if diff_rots(hero_yaw, to) < f32::from_bits(0x3f86_0a92) && elev.abs() < f32::from_bits(0x3f49_0fdb) {
                f.pitch = -elev;
                f.yaw = ty;
                f.best = d;
                f.found = Some(id);
                return f;
            }
        }
        if d <= f.best {
            let rec = targeting::record(m);
            let miss = targeting::cone_miss(d, f.yaw, f.pitch + ground * 0.5, from, p, rec.and(targeting::record_radius(m)), thr);
            if miss < thr && clear(p) {
                f.pitch = -elev;
                f.yaw = ty;
                f.best = d;
                f.found = Some(id);
                if !already_targeted(table, id) {
                    f.lock = Some(id);
                    f.aim_height = Some(targeting::aim_height(m).unwrap_or(0.5));
                }
            }
        }
    }
    f
}

/// `FUN_002c5720(n)`: the distance a missile covers in its first `n` ticks (its speed from 0 toward 20·dt by a tenth a
/// tick).
pub fn early_distance(n: i32) -> f32 {
    let (mut v, mut d) = (0.0f32, 0.0f32);
    for _ in 0..n.max(0) {
        v += (DT * 20.0 - v) / 10.0;
        d += v;
    }
    d
}

/// `0x2c7d68`, the Devastator's update (from the slot loop with the slot ready).
pub fn update(hero: &mut Hero, table: &mut MobyTable, _anim: &dyn super::anim::AnimCtl, env: &ItemEnv, hits: &mut dyn HitSink, _rng: &mut Rng) {
    let Some(class) = hero.items.slot.item.as_ref().and_then(|m| env.data.class(m.o_class)).cloned() else { return };
    let id = hero.items.slot.id;
    let tick = env.frame as u64;
    let gold = hero.weapons.gold[DEVASTATOR as usize];
    // 0x2c7758: the muzzle smoke while +0xbc runs.
    if hero.weapons.devastator.smoke != 0 { muzzle_smoke(hero, _rng); }
    if hero.items.slot.item.as_ref().is_some_and(|it| it.mstate == 0) {
        if let Some(it) = hero.items.slot.item.as_mut() { it.mstate = 1; }
        hero.weapons.devastator.light = -1;
        hero.weapons.devastator.light_timer = 0;
    }
    {
        let d = &mut hero.weapons.devastator;
        guns::dec(&mut d.fire_timer);
        if !guns::dec(&mut d.lock_timer) {
            if d.lock.is_some_and(|t| table.mobys.get(t).is_none_or(|m| m.state & 0x80 != 0)) { d.lock = None; }
        } else {
            d.lock = None;
        }
    }
    if hero.state == 1 { hero.weapons.deferred = Some(0x1e); }
    let look = guns::first_person(hero);
    if look && hero.items.f13fc == 0 {
        if let Some((eye, fwd)) = env.camera {
            let end = add3(eye, scale3(fwd, 70.0));
            if let Some(Some(super::items::Probe { moby: Some(m), point, .. })) = hits.probe_moby(table, from_f32x3(eye), from_f32x3(end), 0, None) {
                if table.mobys.get(m).is_some_and(|mo| mo.mode & mode::TARGETABLE != 0) {
                    let d = &mut hero.weapons.devastator;
                    d.lock = Some(m);
                    d.lock_timer = ticks(20);
                    d.aim_height = point[2] - table.mobys[m].position[2];
                }
            }
            let t240 = ticks(240).max(1);
            let spin = crate::moby_update::services::normalize_angle(crate::ps2v::Pf::f(((tick % t240 as u64) as f32 * 360.0 / 240.0).to_radians())).to_f32();
            hero.weapons.markers.register(tick, Marker { size: 1.0, angle: spin, rgba: MARKER_LOCK, at: None, fx: MARKER_FX });
            let d = &hero.weapons.devastator;
            if let (Some(t), 0) = (d.lock, d.fire_timer) {
                if hero.weapons.has_ammo(id) != 0 {
                    let m = &table.mobys[t];
                    let dist = len3(sub3([m.position[0], m.position[1], m.position[2]], eye));
                    let size = (0.9 - (dist / 100.0) * 0.57).abs();
                    let at = [m.position[0], m.position[1], m.position[2] + d.aim_height];
                    let back = crate::moby_update::services::normalize_angle(crate::ps2v::Pf::f(((tick % t240 as u64) as f32 * -360.0 / 240.0).to_radians())).to_f32();
                    hero.weapons.markers.register(tick, Marker { size, angle: back, rgba: super::blaster::MARKER_GREEN, at: Some(at), fx: MARKER_FX });
                }
            }
        }
    }
    let seq_a = hero.items.slot.item.as_ref().map_or(0, |it| it.anim.seq_a);
    let held = env.pad.held & hero.items.slot.fire_mask != 0;
    if hero.weapons.devastator.fire_timer == 0 && hero.items.f13fc == 0 && seq_a < 2 && held && ticks(0x16) < hero.items.slot.ticks_ready {
        if let Some(it) = hero.items.slot.item.as_mut() {
            if it.anim.seq_b == 0 { moby_anim::set_sequence(&mut it.anim, &class.anim, 1, 0, ticks(4), &mut it.snapshot); }
        }
        let dir = match (look, env.camera) {
            (true, Some((_, f))) => scale3(f, DT * 10.0),
            _ => scale3(to_f32x3(hero.moby_rows[0]), DT * 10.0),
        };
        if !hero.weapons.use_ammo(id, 1) {
            hero.fx.item_sounds.push(EMPTY_SOUND);
        } else {
            fire_missile(hero, table, env, hits, _rng, dir, look, gold);
        }
        hero.weapons.devastator.fire_timer = ticks(0x23);
    }
    light(hero, env, hits);
}

/// The firing part of `0x2c7d68` (with ammo used): the aim, the search, the sound, the light, the missile, Ratchet's
/// shot animation or the arm.
#[allow(clippy::too_many_arguments)]
fn fire_missile(hero: &mut Hero, table: &mut MobyTable, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng, dir: [f32; 3], look: bool, gold: u8) {
    let (yaw0, pitch0) = guns::aim_angles(dir);
    let ground = hero.pitch.to_f32();
    let mut aim_h = 0.75f32;
    let muzzle;
    let mut f = Found { yaw: yaw0, pitch: pitch0, best: 10000.0, found: None, lock: None, aim_height: None };
    if look {
        muzzle = guns::camera_point(env, [0.15, 0.3, 0.75]).unwrap_or(to_f32x3(hero.pos));
        if hero.weapons.devastator.lock.is_some() { aim_h = hero.weapons.devastator.aim_height; }
    } else {
        muzzle = guns::item_point(hero, env, 0);
        let types: Vec<(i16, Option<u8>)> = env.targets.iter().filter_map(|&t| table.mobys.get(t)).map(|m| (m.o_class, hits.class_type(m.o_class))).collect();
        let class_type = |c: i16| types.iter().find(|x| x.0 == c).and_then(|x| x.1);
        let cam = env.camera.map(|c| c.0);
        let coll = env.coll;
        let clear = |p: [f32; 3]| match (cam, coll) {
            (Some(c), Some(coll)) => super::physics::line_world(coll, from_f32x3(c), from_f32x3(p), 6).is_none(),
            _ => true,
        };
        let hm = table.mobys.get(env.hero_moby);
        let hero_yaw = hm.map_or(hero.rot[2].to_f32(), |m| m.rotation[2]);
        let hero_pos = hm.map_or(to_f32x3(hero.pos), |m| [m.position[0], m.position[1], m.position[2]]);
        f = search(table, env.targets, muzzle, yaw0, pitch0, ground, hero_yaw, hero_pos, gold, &class_type, clear);
        if let Some(l) = f.lock {
            hero.weapons.devastator.lock = Some(l);
            aim_h = f.aim_height.unwrap_or(0.5);
        }
    }
    hero.fx.item_sounds.push(FIRE_SOUND);
    hero.weapons.devastator.light_timer = (ticks(5) + ticks(10)) as i16;
    if !look { muzzle_burst(hero, rng); }
    let lock = hero.weapons.devastator.lock;
    let aim_of = |table: &MobyTable, t: Option<MobyId>, h: f32| t.and_then(|t| table.mobys.get(t)).and_then(targeting::aim_height).unwrap_or(h);
    let (yaw, pitch, myaw, mpitch, target);
    if guns::arm_raised(hero) && hero.state != 0x1e {
        // The arm raised at a wall (0x141618 = 1): along the gun's own yaw (its +0x48) and the raised arm's pitch
        // (joint record 9, 0x17b174), at the lock (0x2c87a4's branch).
        let gy = guns::item_euler(hero).map_or(yaw0, |e| e[2]);
        let gp = guns::arm_pitch(hero);
        aim_h = aim_of(table, lock, aim_h);
        (yaw, pitch, myaw, mpitch, target) = (gy, gp, gy, gp, lock);
    } else if f.best < 2.0 {
        aim_h = aim_of(table, lock, aim_h);
        (yaw, pitch, myaw, mpitch, target) = (f.yaw, f.pitch, f.yaw, f.pitch, lock);
    } else if lock.is_none() {
        match f.found {
            None if look => { (yaw, pitch, myaw, mpitch, target) = (f.yaw, f.pitch, yaw0, pitch0, None); }
            None => { (yaw, pitch, myaw, mpitch, target) = (f.yaw, f.pitch - ground * 0.5, yaw0, pitch0 - ground * 0.5, None); }
            Some(t) => {
                // A creature another missile holds: aimed from where the missile will be after its first 6 ticks.
                let tp = targeting::aim_point(&table.mobys[t]);
                let mp = pitch0 - ground * 0.5;
                let p = add3(targeting::polar(early_distance(6), yaw0, mp), muzzle);
                let d = sub3(tp, p);
                (yaw, pitch, myaw, mpitch, target) = (atan(d[0], d[1]), -atan((d[0] * d[0] + d[1] * d[1]).sqrt(), d[2]), yaw0, mp, None);
            }
        }
    } else {
        (yaw, pitch, myaw, mpitch, target) = (f.yaw, f.pitch, yaw0, pitch0, lock);
    }
    crate::moby_update::classes::devastator_missile::spawn(table, env, hits, (yaw, pitch), aim_h, (myaw, mpitch), muzzle, target, hero);
    // Ratchet: the standing shot, or the arm.
    if hero.group == 0 || (hero.state == 2 && hero.stick_mag.to_f32() < 0.7) {
        hero.weapons.pending_anim = Some((ticks(10), 0x36, 1));
    } else {
        hero.weapons.pending_draw = true;
    }
}

/// The item's frame: its rows (moby +0xc0, as `HeroItemsAttach` left them) and position.
fn item_frame(hero: &Hero) -> Option<([[f32; 3]; 3], [f32; 3])> {
    let it = hero.items.slot.item.as_ref()?;
    Some(([0, 1, 2].map(|i| [0, 1, 2].map(|k| f32::from_bits(it.rows[i][k]))), it.position))
}

fn in_frame(r: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] { std::array::from_fn(|k| v[0] * r[0][k] + v[1] * r[1][k] + v[2] * r[2][k]) }

/// `0x2c7758` (every tick while +0xbc runs, stepping it): at the item's (0.23, 0.18, 0) two smoke puffs (type 44: size
/// 25000 growing 300 a tick, damping 0.98, alpha 0x7f white, `ticks(30)`, spin `trunc(randf_sym(0, 6))`) — the first
/// along the item's y axis `randf(0.01, 0.06)` fast, the second along the direction the first overwrote with 0.7 of
/// Ratchet's platform motion (zero on still ground: the game's reuse of the vector) — each plus 0.7 of his displacement
/// and platform motion (flat); then one spark (type 21: size 10000, 0x4f007fff → 0x1fffffff, `rand_range(ticks(20),
/// ticks(40))`, splitting) with 0.55 of them.
fn muzzle_smoke(hero: &mut Hero, rng: &mut Rng) {
    hero.weapons.devastator.smoke -= 1;
    let Some((r, p)) = item_frame(hero) else { return };
    let pos = add3(p, in_frame(&r, [f32::from_bits(0x3e6b_851f), f32::from_bits(0x3e38_51ec), 0.0]));
    let flat = |v: [f32; 3], k: f32| [v[0] * k, v[1] * k, 0.0];
    let disp = to_f32x3(hero.disp);
    let plat = to_f32x3(hero.plat_applied);
    let mut c = in_frame(&r, [0.0, 1.0, 0.0]);
    let pos4 = [pos[0], pos[1], pos[2], 0.0];
    for _ in 0..2 {
        let s = rng.randf(f32::from_bits(0x3c23_d70a), f32::from_bits(0x3d75_c28f));
        let mut v = guns::with_len(c, s);
        c = flat(disp, 0.7);
        v = add3(v, c);
        c = flat(plat, 0.7);
        v = add3(v, c);
        let spin = rng.randf_sym(0.0, 6.0) as i32;
        let spawn = crate::particles::type44::Spawn { size: 25000.0, growth: 300.0, damp: f32::from_bits(0x3f7a_e148), fall: 0.0, w: f32::from_bits(0x3f59_999a), pos, vel: v, life: ticks(30), alpha: 0x7f, rgb: 0xff_ffff, spin };
        let rot = rng.randi(0xff) as u8;
        hero.fx.parts.push(super::fx::PartSpawn::Smoke44 { spawn, rot });
    }
    let s = rng.randf(f32::from_bits(0x3c23_d70a), f32::from_bits(0x3d75_c28f));
    let mut v = guns::with_len(c, s);
    v = add3(v, flat(disp, f32::from_bits(0x3f0c_cccd)));
    v = add3(v, flat(plat, f32::from_bits(0x3f0c_cccd)));
    let life = rng.rand_range(ticks(20), ticks(40));
    let rot = rng.rand() as u8;
    hero.fx.parts.push(super::fx::PartSpawn::Spark21 { size: 10000.0, pos: pos4, vel: [v[0], v[1], v[2], 0.0], c1: 0x4f00_7fff, c2: 0x1fff_ffff, life, split: 1, rot });
}

/// `0x2c7a48` (a third-person shot): at the item's (0.3, −1.2, 0) a ring of 24 smoke puffs (type 44: size 60000
/// growing 3000, damping 0.85, falling 0.001, alpha 0x1e, `rand_range(ticks(30), ticks(90))`, spin ±`rand_range(0, 3)`)
/// around Ratchet's facing (his up row turned `i·15° + randf_sym(0, 7.5°)` about it, `randf(0.05, 0.1)` fast, plus
/// 1.7 of his displacement on the ground) and 6 sparks (type 21, 60° apart + `randf(0, 45°)`, `rand_range(ticks(20),
/// ticks(60))`); +0xbc = `ticks(5)`.
fn muzzle_burst(hero: &mut Hero, rng: &mut Rng) {
    let Some((r, p)) = item_frame(hero) else { return };
    let pos = add3(p, in_frame(&r, [f32::from_bits(0x3e99_999a), f32::from_bits(0xbf99_999a), 0.0]));
    let pos4 = [pos[0], pos[1], pos[2], 0.0];
    let rows = hero.moby_rows.map(to_f32x3);
    let disp = to_f32x3(hero.disp);
    for i in 0..24 {
        let a = rng.randf_sym(0.0, f32::from_bits(0x3e06_0a92));
        let mut v = crate::moby_update::classes::blaster_shot::rotate(rows[2], i as f32 * 0.261_799_4 + a, rows[0]);
        let s = rng.randf(0.05, 0.1);
        v = guns::with_len(v, s);
        let mut spin = rng.rand_range(0, 3);
        if rng.randi(2) != 0 { spin = -spin; }
        if hero.grounded_ticks != 0 { v = add3(v, scale3(disp, f32::from_bits(0x3fd9_999a))); }
        let life = rng.rand_range(ticks(30), ticks(90));
        let spawn = crate::particles::type44::Spawn { size: 60000.0, growth: 3000.0, damp: f32::from_bits(0x3f59_999a), fall: f32::from_bits(0xba83_126f), w: 0.0, pos, vel: v, life, alpha: 0x1e, rgb: 0xff_ffff, spin };
        let rot = rng.randi(0xff) as u8;
        hero.fx.parts.push(super::fx::PartSpawn::Smoke44 { spawn, rot });
    }
    for i in 0..6 {
        let a = rng.randf(0.0, std::f32::consts::FRAC_PI_4);
        let v = crate::moby_update::classes::blaster_shot::rotate(rows[2], i as f32 * f32::from_bits(0x3f86_0a92) + a, rows[0]);
        let s = rng.randf(0.05, 0.1);
        let v = guns::with_len(v, s);
        let life = rng.rand_range(ticks(20), ticks(60));
        let rot = rng.rand() as u8;
        hero.fx.parts.push(super::fx::PartSpawn::Spark21 { size: 10000.0, pos: pos4, vel: [v[0], v[1], v[2], 0.0], c1: 0x4f00_7fff, c2: 0x1fff_ffff, life, split: 1, rot });
    }
    hero.weapons.devastator.smoke = ticks(5) as u8;
}

/// The light (the tail of `0x2c7d68`).
fn light(hero: &mut Hero, env: &ItemEnv, hits: &mut dyn HitSink) {
    let (seq_a, seq_b) = hero.items.slot.item.as_ref().map_or((0, 0), |it| (it.anim.seq_a, it.anim.seq_b));
    if seq_a != 2 && 0 < hero.weapons.devastator.light_timer {
        let t = hero.weapons.devastator.light_timer as i32;
        let (t5, t10) = (ticks(5), ticks(10));
        let (r, g, b) = if t10 < t {
            let k = (t5 + t10 - t) as f32;
            (k / t5 as f32, k * 0.8 / t5 as f32, k * 0.5 / t5 as f32)
        } else {
            let k = t as f32;
            (k / t10 as f32, k * 0.8 / t10 as f32, k * 0.5 / t10 as f32)
        };
        let f = to_f32x3(hero.moby_rows[0]);
        let yaw = atan(f[0], f[1]);
        let m = guns::item_point(hero, env, 0);
        let pos = [m[0] + yaw.cos(), m[1] + yaw.sin(), m[2] + 0.3];
        let l = PointLight { color: [r, g, b], intensity: 0.0, pos, radius: 7.0 };
        let d = &mut hero.weapons.devastator;
        if d.light == -1 { d.light = hits.light_alloc(l); } else { hits.light_set(d.light, l); }
        guns::dec16(&mut d.light_timer);
        if seq_b != 2 { return; }
    }
    let d = &mut hero.weapons.devastator;
    if d.light != -1 {
        hits.light_free(d.light);
        d.light = -1;
    }
    d.light_timer = 0;
}

/// The item is gone with the Devastator's light still out.
pub fn item_gone(hero: &mut Hero, hits: &mut dyn HitSink) {
    let d = &mut hero.weapons.devastator;
    if d.light != -1 {
        hits.light_free(d.light);
        d.light = -1;
    }
    d.light_timer = 0;
    d.smoke = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_distance_accumulates() {
        let d = early_distance(6);
        let mut v = 0.0f32;
        let mut s = 0.0f32;
        for _ in 0..6 { v += (DT * 20.0 - v) / 10.0; s += v; }
        assert_eq!(d, s);
        assert!(d > 0.0 && d < 6.0 * DT * 20.0);
    }
}
