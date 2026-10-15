//! **The Blaster** (item 15, class 168; level01 weapon-check case 0xf of `HeroPdaGadget` 0x240ed8, item update
//! `0x2ca610`, its search `0x2ca310`, the shot's spawn `0x2e1cc8`; read from the decompiler output). A persistent-arm
//! weapon (item def +0x30 = 1): the stance / arm layer rules are [`super::weapons`]'s (`0x22ee08` standing sequence 56,
//! moving 57, crouched 62).
//!
//! **The weapon check** ([`fire`], case 0xf): in groups 0, 1, 2, 4, 5, 0xc, 0xf or the look stance 0x1e, with the item
//! moby: ○ held while the item has been ready for more than `ticks(5)`, or ○ pressed, with ammo → the weapon drawn
//! (`0x22ee08`) unless it is out; ○ held without ammo → the empty click (class sound 0, kept in pvar +0x0c while it
//! plays: [`super::fx::LOOP_CLICK`]).
//!
//! **The update** ([`update`], every tick with the slot ready). Pvars ([`Blaster`]): +0x00 the fire timer, +0x04 a
//! timer nothing sets (the target marker waits for it), +0x10 the point light, +0x14 its timer.
//! 1. Item state 0 → 1 (light −1, light timer 0); both timers step (`FastDecTimer`); the look stance 1 becomes 0x1e
//!    (`SetState(0x1e, 1)`, made right after the slot loop: `super::weapons::after_items`); key B on sequence 0
//!    before frame 3 (the draw) → the fire timer `ticks(7)`.
//! 2. **The aim**. In first person ([`super::guns::first_person`]): 40·dt along the camera, the muzzle 0.15 right,
//!    0.15 down, 0.75 ahead of the eye, and (not 0x1413fc) the red crosshair (marker FX 0x26, size 1, colour
//!    0xff0f0fff, screen centre) and a ray from the eye 20 ahead (`CollLine_Fix(.., 0, item)`): a targetable moby it
//!    hits is the target. Otherwise 40·dt along Ratchet's facing (his moby rows' x), the muzzle the item's joint list
//!    0, and — outside 0x1e — **the search** [`search`] (`0x2ca310`). Yaw `FastArcTan(dir)` (0x140600 = 0 [L]),
//!    pitch `−FastArcTan(|dir.xy|, dir.z)`.
//! 3. With a target, the +0x04 timer out and ammo: the green marker (FX 0x26, colour 0xff0fff0f) at its aim point,
//!    size `|0.9 − d/50·0.65|` (d = its distance from the camera).
//! 4. **Firing**: the arm out (0x1413f8), not in 0x72, `ticks(5 − gold) < 0x13f50c` (the arm's timer) and the fire
//!    timer out → one ammo (`0x249450`); with it (not 0x1413fc): the fire timer `ticks(6)`; the shot's aim
//!    ([`shot_aim`]); a hit sphere of 0.42 at Ratchet's (0.7, −0.15, 0.5) (flags 5; template: push (cos, sin) of his
//!    facing, 1, 5627.97; flags 0x10000; damage 0.25; type 1 / 1; class 168); the shot (class 305,
//!    [`crate::moby_update::classes::blaster_shot::spawn`]); the light timer `ticks(15)`; five muzzle sparks (type 27:
//!    size 10000, 0x5f2f4f6f, a random direction 0.65 of the aim's length added to it, `randf(3, 6)·dt` long,
//!    `rand_range(ticks(5), ticks(10))` ticks) and a flash (size 200000, 0x2f4f7f7f, 0.5·dt along the aim,
//!    `rand_range(ticks(4), ticks(7))`). Ammo or not, the item then blends to its firing sequence 4 over `ticks(7)`
//!    (whose loop sound, class sound 1, is the shot's sound: `super::items`' sequence loop). The shot statistics
//!    (0x1416f8..) are not ported.
//! 5. **Held / released**: ○ held with ammo, or the arm out within `ticks(9)` of the draw → nothing; else the item
//!    blends back to sequence 1 over `ticks(7)` (not while it plays its draw, sequence 0), the weapon is put away
//!    (`0x22efd8`) and the fire timer cleared. In Ratchet's hold 0x72 the item's sequence is cut to 1 [M: the game's
//!    `fun_00212ed8` / `fun_0020d580` pair; the second is not ported].
//! 6. **The light**: while the item's key A is on sequence 2 (put away) it is freed; else while the light timer runs,
//!    a grey point light `k·0.7/ticks(9)` (k = min(timer, ticks(9))) of radius 6 two units ahead of Ratchet and one up
//!    (`WritePointLight_B`, then rewritten), the timer stepping; at 0 it is freed.
//!
//! **Native.** Standard `f32`; RNG draws in the game's order (the spawners' own draws made at the call, the records
//! created by the particle hook). The item is not a table moby in the port (its hits' attacker is Ratchet's moby;
//! the shot's owner is Ratchet's moby, as the game's `0x2e1cc8(.., Ratchet, ..)`). Gold (0x13e52f) is not mirrored: 0.

use super::fx::PartSpawn;
use super::guns::{self, add3, len3, scale3, sub3, with_len};
use super::items::{HitSink, ItemEnv};
use super::packs::SoundCmd;
use super::physics::{from_f32x3, to_f32x3, ticks};
use super::Hero;
use crate::moby_runtime::{mode, MobyId, MobyTable};
use crate::moby_update::creature::{add_rot, atan, diff_rots};
use crate::moby_update::services::HitTemplate;
use crate::point_lights::PointLight;
use crate::ps2v::Pf;
use crate::rng::Rng;
use crate::targeting::{self, Marker};
use rc_formats::moby_anim;

/// The Blaster (item 15) and its class.
pub const BLASTER: i32 = 15;
pub const CLASS: i16 = 168;
/// The marker's FX texture (`FUN_0020fb60(.., 0x26, ..)`).
pub const MARKER_FX: usize = 0x26;
/// The markers' colours: the first-person crosshair and the target.
pub const MARKER_RED: u32 = 0xff0f_0fff;
pub const MARKER_GREEN: u32 = 0xff0f_ff0f;
/// The search's range (`0x2ca310`: 20, 3D) and cones (9° for the aim, 10° to take the target's yaw).
pub const RANGE: f32 = 20.0;
pub const CONE: f32 = 0.157_079_64;
pub const YAW_CONE: f32 = 0.174_532_92;
/// The shot's speed (40 u/s: `0x15ed6c · 40`).
pub const SPEED: f32 = 40.0;
const DT: f32 = 1.0 / 60.0;
/// The template's push word (5627.97).
const PUSH: u32 = 0x45af_df66;

/// The Blaster's pvars (item moby +0x78).
#[derive(Clone, Debug, PartialEq)]
pub struct Blaster {
    /// +0x00: the fire timer.
    pub fire_timer: i32,
    /// +0x04 (s16): a timer the target marker waits for (nothing in the level code sets it).
    pub t04: i16,
    /// +0x10: the point light (−1: none); +0x14 its timer.
    pub light: i32,
    pub light_timer: i32,
    /// The last target (this tick's; kept for the tests and the engine's reads).
    pub target: Option<MobyId>,
}

impl Default for Blaster {
    fn default() -> Self { Blaster { fire_timer: 0, t04: 0, light: -1, light_timer: 0, target: None } }
}

/// `HeroPdaGadget` 0x240ed8 case 0xf.
pub fn fire(h: &mut Hero, c: &mut super::states::Ctx) {
    let g = h.group;
    if !(g <= 1 || g == 0xc || g == 2 || g == 4 || g == 5 || g == 0xf || h.state == 0x1e) { return; }
    if h.items.slot.item.is_none() { return; }
    let pad = c.env.pad;
    let mask = h.items.slot.fire_mask;
    let held = pad.held & mask != 0;
    let pressed = pad.pressed & mask != 0;
    let id = h.items.slot.id;
    if ((held && ticks(5) < h.items.slot.ticks_ready) || pressed) && h.weapons.has_ammo(id) != 0 {
        if h.f13f8 == 0 { super::weapons::draw_weapon(h, c); }
        return;
    }
    if held && h.weapons.has_ammo(id) == 0 {
        // `if !SoundIsAlive(item, +0x0c) { +0x0c = PlayClassSound(0, 0, item) }`.
        h.fx.item_voices.push(SoundCmd::ItemLoop { n: super::fx::LOOP_CLICK, index: 0, flags: 0 });
    }
}

/// The template of the Blaster's hits (the muzzle sphere): push `(cos, sin)` of Ratchet's facing, 1, 5627.97; flags
/// 0x10000; damage 0.25; type bytes 1 / 1; the item's class.
fn template(env: &ItemEnv, yaw: f32) -> HitTemplate {
    HitTemplate { dir: [Pf::f(yaw.cos()), Pf::f(yaw.sin()), Pf::ONE, Pf::b(PUSH)], attacker: Some(env.hero_moby), flags: 0x1_0000, b18: 1, b19: 1, h1a: CLASS as u16, damage: Pf::f(0.25), w20: 1 }
}

/// `0x2ca310(item, &from, &yaw, &pitch)`: the Blaster's aim search over the run list's targetable mobys (the target list
/// in its order: the game walks the run list 0x15ffe4 and skips the mobys without 0x1000). For each live one (state <
/// 0x80) within 20 (3D) of `from` and not farther than the best so far, its aim point (the record's height, 0.5 without
/// one) is tested against the aim (`crate::targeting::cone_miss` with the aim's pitch + half the ground pitch
/// 0x13f634, 9°): inside, the camera's and the muzzle's lines to it (flags 6, world only) must be clear — a blocked one
/// **ends the search** with the best so far; a clear one becomes the best: the pitch becomes the elevation to it and,
/// when it is within 10° of the aim's yaw, the yaw too.
#[allow(clippy::too_many_arguments)]
pub fn search(table: &MobyTable, list: &[MobyId], from: [f32; 3], yaw: &mut f32, pitch: &mut f32, ground_pitch: f32, mut clear: impl FnMut([f32; 3], [f32; 3]) -> bool, camera: [f32; 3]) -> Option<MobyId> {
    let mut best = RANGE;
    let mut out = None;
    for &id in list {
        let Some(m) = table.mobys.get(id) else { continue };
        if m.state >= 0x80 || m.mode & mode::TARGETABLE == 0 { continue; }
        let p = targeting::aim_point(m);
        let ty = atan(p[0] - from[0], p[1] - from[1]);
        let d2 = ((p[0] - from[0]).powi(2) + (p[1] - from[1]).powi(2)).sqrt();
        let elev = atan(d2, p[2] - from[2]);
        let diff = diff_rots(*yaw, ty);
        let d = len3(sub3(p, from));
        if !(d <= best) { continue; }
        let miss = targeting::cone_miss(d, *yaw, *pitch + ground_pitch * 0.5, from, p, targeting::record(m).and(targeting::record_radius(m)), CONE);
        if miss < CONE {
            if !clear(camera, p) || !clear(from, p) { return out; }
            *pitch = -elev;
            best = d;
            out = Some(id);
            if diff < YAW_CONE { *yaw = ty; }
        }
    }
    out
}

/// The shot's aim at the moment of firing (`0x2ca610` before `0x2e1cc8`): from the aim `(yaw0, pitch0)` (before the
/// search) and the search's `(yaw, pitch)`: without a target (outside 0x1e) both pitches lose half the ground pitch;
/// with one, the pitch toward its aim point is clamped to ±10° of `pitch0 − ground/2`, one 40·dt step along
/// `(yaw0, that pitch)` from the muzzle is taken, and the shot's direction is from there to the aim point. Returns
/// `((pvar yaw, pvar pitch), (moby yaw, moby pitch))` of the shot.
#[allow(clippy::too_many_arguments)]
pub fn shot_aim(muzzle: [f32; 3], target: Option<[f32; 3]>, look: bool, yaw0: f32, pitch0: f32, yaw: f32, pitch: f32, ground: f32) -> ((f32, f32), (f32, f32)) {
    let Some(tp) = target else {
        return if look { ((yaw, pitch), (yaw0, pitch0)) } else { ((yaw, pitch - ground * 0.5), (yaw0, pitch0 - ground * 0.5)) };
    };
    let d2 = ((tp[0] - muzzle[0]).powi(2) + (tp[1] - muzzle[1]).powi(2)).sqrt();
    let to = -atan(d2, tp[2] - muzzle[2]);
    let base = pitch0 - ground * 0.5;
    let mut fp = to;
    if YAW_CONE <= diff_rots(base, to) {
        fp = add_rot(base, YAW_CONE);
        if to <= fp {
            fp = add_rot(base, -YAW_CONE);
            if fp <= to { fp = base; }
        }
    }
    let step = add3(targeting::polar(SPEED * DT, yaw0, -fp), muzzle);
    let sy = atan(tp[0] - step[0], tp[1] - step[1]);
    let sd2 = ((tp[0] - step[0]).powi(2) + (tp[1] - step[1]).powi(2)).sqrt();
    let sp = -atan(sd2, tp[2] - step[2]);
    ((sy, sp), (yaw0, fp))
}

/// `0x2ca610`, the Blaster's update (from the slot loop with the slot ready).
pub fn update(hero: &mut Hero, table: &mut MobyTable, _anim: &dyn super::anim::AnimCtl, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng) {
    let Some(class) = hero.items.slot.item.as_ref().and_then(|m| env.data.class(m.o_class)).cloned() else { return };
    let id = hero.items.slot.id;
    let gold = hero.weapons.gold[BLASTER as usize] as i32;
    let tick = env.frame as u64;
    if hero.items.slot.item.as_ref().is_some_and(|it| it.mstate == 0) {
        if let Some(it) = hero.items.slot.item.as_mut() { it.mstate = 1; }
        hero.weapons.blaster.light = -1;
        hero.weapons.blaster.light_timer = 0;
    }
    {
        let b = &mut hero.weapons.blaster;
        guns::dec16(&mut b.t04);
        guns::dec(&mut b.fire_timer);
    }
    if hero.state == 1 { hero.weapons.deferred = Some(0x1e); }
    if hero.items.slot.item.as_ref().is_some_and(|it| it.anim.seq_b == 0 && it.anim.frame_b < 3) { hero.weapons.blaster.fire_timer = ticks(7); }
    // The aim.
    let look = guns::first_person(hero);
    let mut target: Option<MobyId> = None;
    let (dir, muzzle) = match (look, env.camera) {
        (true, Some((eye, fwd))) => {
            let dir = scale3(fwd, SPEED * DT);
            let muzzle = guns::camera_point(env, [0.15, 0.15, 0.75]).unwrap_or(eye);
            if hero.items.f13fc == 0 {
                hero.weapons.markers.register(tick, Marker { size: 1.0, angle: 0.0, rgba: MARKER_RED, at: None, fx: MARKER_FX });
                let end = add3(eye, with_len(dir, 20.0));
                if let Some(Some(super::items::Probe { moby: Some(m), .. })) = hits.probe_moby(table, from_f32x3(eye), from_f32x3(end), 0, None) {
                    if table.mobys.get(m).is_some_and(|mo| mo.mode & mode::TARGETABLE != 0) { target = Some(m); }
                }
            }
            (dir, muzzle)
        }
        _ => (scale3(to_f32x3(hero.moby_rows[0]), SPEED * DT), guns::item_point(hero, env, 0)),
    };
    let (mut yaw0, mut pitch0) = guns::aim_angles(dir);
    // Outside 0x1e with the arm raised at a wall (0x141618 = 1): the gun's own yaw (its +0x48) and the raised arm's
    // pitch (joint record 9, 0x17b174).
    if hero.state != 0x1e && guns::arm_raised(hero) {
        if let Some(e) = guns::item_euler(hero) { yaw0 = e[2]; }
        pitch0 = guns::arm_pitch(hero);
    }
    let (mut yaw, mut pitch) = (yaw0, pitch0);
    let ground = hero.pitch.to_f32();
    if hero.state != 0x1e {
        let cam = env.camera.map_or(muzzle, |c| c.0);
        let coll = env.coll;
        let clear = |a: [f32; 3], b: [f32; 3]| coll.is_none_or(|c| super::physics::line_world(c, from_f32x3(a), from_f32x3(b), 6).is_none());
        target = search(table, env.targets, muzzle, &mut yaw, &mut pitch, ground, clear, cam);
    }
    hero.weapons.blaster.target = target;
    let target_point = target.and_then(|t| table.mobys.get(t)).map(targeting::aim_point);
    if let (Some(t), Some(tp)) = (target, target_point) {
        if hero.weapons.blaster.t04 == 0 && hero.weapons.has_ammo(id) != 0 {
            let cam = env.camera.map_or(muzzle, |c| c.0);
            let m = &table.mobys[t];
            let d = len3(sub3([m.position[0], m.position[1], m.position[2]], cam));
            let size = (0.9 - (d / 50.0) * 0.65).abs();
            hero.weapons.markers.register(tick, Marker { size, angle: 0.0, rgba: MARKER_GREEN, at: Some(tp), fx: MARKER_FX });
        }
    }
    // Firing.
    if hero.f13f8 != 0 && hero.state != 0x72 && ticks(5 - gold) < hero.f50c && hero.weapons.blaster.fire_timer == 0 {
        if hero.weapons.use_ammo(id, 1) && hero.items.f13fc == 0 {
            hero.weapons.blaster.fire_timer = ticks(6);
            let ((sy, sp), (my, mp)) = shot_aim(muzzle, target_point, look, yaw0, pitch0, yaw, pitch, ground);
            let facing = to_f32x3(hero.moby_rows[0]);
            let fy = atan(facing[0], facing[1]);
            let tmpl = template(env, fy);
            let c = guns::hero_point(hero, [0.7, -0.15, 0.5]);
            hits.sphere(table, Pf::b(0x3ed7_0a3d), from_f32x3(c), 5, Some(env.hero_moby), &tmpl);
            let hero_pos = table.mobys.get(env.hero_moby).map_or(to_f32x3(hero.pos), |m| [m.position[0], m.position[1], m.position[2]]);
            crate::moby_update::classes::blaster_shot::spawn(hero, table, env, hits, rng, (sy, sp), (my, mp), muzzle, hero_pos, target, gold);
            hero.weapons.blaster.light_timer = ticks(15);
            // The muzzle sparks and the flash (type 27).
            let len = len3(dir);
            for _ in 0..5 {
                let x = rng.randf(-1.0, 1.0);
                let y = rng.randf(-1.0, 1.0);
                let z = rng.randf(-1.0, 1.0);
                let v = add3(with_len([x, y, z], len * 0.65), dir);
                let s = rng.randf(DT * 3.0, DT * 6.0);
                let v = with_len(v, s);
                let life = rng.rand_range(ticks(5), ticks(10));
                let rot = rng.rand() as u8;
                hero.fx.parts.push(PartSpawn::Spark27 { size: 10000.0, pos: [muzzle[0], muzzle[1], muzzle[2], 0.0], vel: [v[0], v[1], v[2], 0.0], rgba: 0x5f2f_4f6f, life, rot });
            }
            let v = with_len(dir, DT * 0.5);
            let life = rng.rand_range(ticks(4), ticks(7));
            let rot = rng.rand() as u8;
            hero.fx.parts.push(PartSpawn::Spark27 { size: 200000.0, pos: [muzzle[0], muzzle[1], muzzle[2], 0.0], vel: [v[0], v[1], v[2], 0.0], rgba: 0x2f4f_7f7f, life, rot });
        }
        if let Some(it) = hero.items.slot.item.as_mut() {
            if it.anim.seq_b != 4 { moby_anim::set_sequence(&mut it.anim, &class.anim, 4, 0, ticks(7), &mut it.snapshot); }
        }
    }
    // Held / released.
    let held = env.pad.held & hero.items.slot.fire_mask != 0 && hero.weapons.has_ammo(id) != 0;
    let keep = held || (hero.f50c <= ticks(9) && hero.f13f8 != 0);
    if !(keep && hero.state != 0x72) {
        if let Some(it) = hero.items.slot.item.as_mut() {
            let drawing = it.anim.seq_b == 0 && it.anim.flags & 2 == 0;
            if hero.state == 0x72 {
                moby_anim::hard_cut(&mut it.anim, &class.anim, 1, 0);
            } else if !drawing && it.anim.seq_b != 1 {
                moby_anim::set_sequence(&mut it.anim, &class.anim, 1, 0, ticks(7), &mut it.snapshot);
            }
        }
        super::weapons::put_away(hero);
        hero.weapons.blaster.fire_timer = 0;
    }
    light(hero, env, hits);
}

/// Step 6 of the update: the muzzle flash's light.
fn light(hero: &mut Hero, _env: &ItemEnv, hits: &mut dyn HitSink) {
    let put_away = hero.items.slot.item.as_ref().is_some_and(|it| it.anim.seq_a == 2);
    let b = &mut hero.weapons.blaster;
    if !put_away && 0 < b.light_timer {
        let k = b.light_timer.min(ticks(9)) as f32;
        let c = k * 0.7 / ticks(9) as f32;
        let f = to_f32x3(hero.moby_rows[0]);
        let yaw = atan(f[0], f[1]);
        let p = to_f32x3(hero.pos);
        let pos = [p[0] + yaw.cos() * 2.0, p[1] + yaw.sin() * 2.0, p[2] + 1.0];
        let l = PointLight { color: [c, c, c], intensity: 0.0, pos, radius: 6.0 };
        if b.light == -1 { b.light = hits.light_alloc(l); } else { hits.light_set(b.light, l); }
        guns::dec(&mut b.light_timer);
        return;
    }
    if b.light != -1 {
        hits.light_free(b.light);
        b.light = -1;
        b.light_timer = 0;
    }
}

/// The item is gone (swapped away, the slot emptied) with the Blaster's light still out.
pub fn item_gone(hero: &mut Hero, hits: &mut dyn HitSink) {
    let b = &mut hero.weapons.blaster;
    if b.light != -1 {
        hits.light_free(b.light);
        b.light = -1;
    }
    b.light_timer = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::Moby;

    fn target(pos: [f32; 3]) -> Moby {
        let mut m = Moby { position: [pos[0], pos[1], pos[2], 1.0], mode: mode::TARGETABLE | 0x20, pvars: vec![0; 0x80], ..Moby::default() };
        m.pvars[0..4].copy_from_slice(&0x20u32.to_le_bytes());
        m.pvars[0x30..0x34].copy_from_slice(&0.5f32.to_le_bytes());
        m
    }

    /// 0x2ca310: 20 units, 9° off the aim (less the record's radius), the camera's and the muzzle's lines; the yaw only
    /// within 10°; the pitch = the elevation; a blocked line ends the search.
    #[test]
    fn search_rules() {
        let from = [0.0, 0.0, 1.0];
        let one = |p: [f32; 3], radius: u8| {
            let mut m = target(p);
            m.pvars[0x2a] = radius;
            let t = MobyTable::new(vec![m], 0);
            let (mut y, mut pi) = (0.0f32, 0.0f32);
            let r = search(&t, &[0], from, &mut y, &mut pi, 0.0, |_, _| true, [0.0, -5.0, 3.0]);
            (r, y, pi)
        };
        // Straight ahead at the muzzle's height (aim point 0.5 above the moby).
        let (r, y, p) = one([10.0, 0.0, 0.5], 0);
        assert_eq!(r, Some(0));
        assert!(y.abs() < 1e-6 && p.abs() < 1e-6);
        assert_eq!(one([21.0, 0.0, 0.5], 0).0, None, "range 20");
        // 8° to the side: taken, the yaw follows; 12°: refused, unless the radius byte covers it.
        let a = 8f32.to_radians();
        let (r, y, _) = one([10.0 * a.cos(), 10.0 * a.sin(), 0.5], 0);
        assert_eq!(r, Some(0));
        assert!((y - a).abs() < 1e-5);
        let a = 12f32.to_radians();
        assert_eq!(one([10.0 * a.cos(), 10.0 * a.sin(), 0.5], 0).0, None);
        assert_eq!(one([10.0 * a.cos(), 10.0 * a.sin(), 0.5], 8).0, Some(0), "radius 1 at 10: asin(0.1) off");
        // Blocked line of sight: nothing.
        let t = MobyTable::new(vec![target([10.0, 0.0, 0.5])], 0);
        let (mut y, mut pi) = (0.0, 0.0);
        assert_eq!(search(&t, &[0], from, &mut y, &mut pi, 0.0, |_, _| false, [0.0; 3]), None);
    }

    /// The shot's direction: straight at a target when within the ±10° clamp; the moby keeps the facing's yaw.
    #[test]
    fn shot_aims_at_the_target() {
        let muzzle = [0.0, 0.0, 1.0];
        let ((sy, sp), (my, mp)) = shot_aim(muzzle, Some([10.0, 0.0, 1.0]), false, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert!(sy.abs() < 1e-5 && sp.abs() < 1e-5 && my == 0.0 && mp == 0.0);
        // A target 30° up: the moby's pitch is clamped to 10° up (negative down-positive pitch).
        let ((_, sp), (_, mp)) = shot_aim(muzzle, Some([10.0, 0.0, 1.0 + 10.0 * 30f32.to_radians().tan()]), false, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert!((mp + YAW_CONE).abs() < 1e-5, "{mp}");
        assert!(sp < -0.5, "the pvar pitch still aims up at it: {sp}");
        // No target: the ground pitch's half comes off.
        let ((_, sp), (_, mp)) = shot_aim(muzzle, None, false, 0.0, 0.1, 0.0, 0.1, 0.2);
        assert!((sp - 0.0).abs() < 1e-6 && (mp - 0.0).abs() < 1e-6);
    }
}
