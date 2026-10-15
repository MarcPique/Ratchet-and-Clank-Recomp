//! **The R.Y.N.O.** (item 23, class 454; level01 item update `0x2e4e60`, its search `0x2e4bb8`, the missile's spawn
//! `0x2e5738`; read from the decompiler output and the disassembly). No weapon-check case: its update fires. A
//! persistent-arm weapon (def +0x30 = 1; standing sequence 81, no moving arm layer). The missiles are class 457
//! (`crate::moby_update::classes::ryno_missile`).
//!
//! **Pvars** ([`Ryno`]): +0x00 the target, +0x04 the salvo's shot count, +0x05 the barrel (the class's joint lists
//! 0..8), +0x06 the shot timer, +0x08 the lock timer, +0x0c the target's aim height, +0x10 the marker's wait, +0x12
//! the next slot of +0x18.. (the salvo's seven past targets), +0x14 the target's health left for this salvo.
//!
//! **Every tick in the item's states 2 / 3** (ready / aiming): the lock timer steps; a target it outlived, or a dead
//! one, is dropped. **The search** [`search`] (`0x2e4bb8(97.3°, 97.3°, 80, barrel point, camera angles, none)`) from
//! the barrel's point with the camera's yaw / elevation; a live, drawn (+0x31) current target is kept, else the found
//! one is taken: the lock timer `ticks(20)`, the health left = its record's `+0x00` (1 without a record), the aim
//! height its record's `+0x10`. With a target, the marker's wait out and ammo: the green marker (FX 0x23, colour
//! 0xff0fff0f) at its aim point, size `|0.9 − d/150·0.65|`.
//!
//! **States** (+0x20): 0 → 1 (target none, the swap lock 0x1403fc clear, the marker's wait 0); 1 → 2 when the draw
//! wraps; **2**: the look stance 1 becomes 0x1e; ○ pressed (not 0x1413fc) with one ammo (`0x249450`) → **4**, the swap
//! lock 2 (no item swap during the salvo), the shot timer and counters 0, the weapon drawn (`0x22ee08`), the past
//! targets cleared; **4** (the salvo): every `ticks(9)` one missile from the next barrel (0..8) — along Ratchet's facing
//! (in 0x1e the camera's) plus 3× his displacement and platform motion, the weapon drawn again, `0x2e5738(aim height,
//! yaw, pitch, item, barrel point, target)` — seven in all (one ammo for the salvo), then the swap lock clear, the
//! weapon put away, state **5** with the shot timer and the marker's wait `ticks(60)`. Each missile takes 1 off the
//! target's health left; at 0 (or with the target gone) the target joins the past targets and a new one is searched
//! (`0x2e4bb8(π, π, 100, the item's position, the camera angles, the past targets)`), else a past one is picked at
//! random (`rand() & (n + 1)` must be 0, then `rand() % n`); **5**: after the shot timer, target none, the count 0,
//! back to 2. **State 3** (0x2e5264, ported 2026-09-28): the red crosshair (FX 0x23, 0xff0f0fff, size 1) at the screen
//! centre every tick; with L1 or L2 held ○ fires the salvo as state 2 does ([`start_salvo`], the same code in the game),
//! released → state 2. No code in the level overlays writes 3 to the item's +0x20 (every writer of the R.Y.N.O.'s state
//! stores 0 / 1 / 2 / 4 / 5): the state is unreachable in play, kept for completeness and tested by setting it. The
//! prologue's lock / marker upkeep also runs in state 3 (states 2 / 3). The shot statistics (0x141738..) are not ported
//! (G-SAV-009).
//!
//! **Native.** Standard `f32`; the camera angles are the camera's yaw and its elevation (0x167250's `(x, −y, z)`: y
//! is the pitch, positive down) from its forward row [M]. The item is not a table moby in the port: the missiles'
//! owner is Ratchet's moby, and the second search starts from the item's position (moby +0x10, `HeroItemsAttach`'s).

use super::guns::{self, add3, len3, scale3, sub3};
use super::items::{HitSink, ItemEnv};
use super::physics::{to_f32x3, ticks};
use super::Hero;
use crate::moby_runtime::{mode, MobyId, MobyTable};
use crate::moby_update::creature::{atan, diff_rots};
use crate::rng::Rng;
use crate::targeting::{self, Marker};

pub const RYNO: i32 = 23;
pub const CLASS: i16 = 454;
pub const MARKER_FX: usize = 0x23;
/// The first search's cones and range (`0x3fd95d40` = 1.6982 rad, 80) and the salvo's (π, 100).
pub const CONE: f32 = 1.698_161_6;
pub const RANGE: f32 = 80.0;
pub const SALVO_RANGE: f32 = 100.0;
/// Missiles per salvo and the barrels.
pub const SALVO: u8 = 7;
pub const BARRELS: u8 = 9;

/// The R.Y.N.O.'s pvars (item moby +0x78).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ryno {
    /// +0x00: the target.
    pub target: Option<MobyId>,
    /// +0x04 / +0x05: the salvo's shot count, the barrel.
    pub count: u8,
    pub barrel: u8,
    /// +0x06: the shot timer (s16); +0x08 the lock timer.
    pub shot_timer: i16,
    pub lock_timer: i32,
    /// +0x0c: the target's aim height; +0x14 its health left for this salvo.
    pub aim_height: f32,
    pub left: f32,
    /// +0x10: the marker's wait (s16).
    pub marker_wait: i16,
    /// +0x12 / +0x18..: the past targets of the salvo (seven slots, the next one to write).
    pub next: i16,
    pub past: [Option<MobyId>; 7],
}

fn dead(table: &MobyTable, id: MobyId) -> bool { table.mobys.get(id).is_none_or(|m| m.state == 0xfe || m.state == 0xfd) }

/// `0x2e4bb8(cone_yaw, cone_pitch, range, from, angles, exclude)`: the R.Y.N.O.'s target (module doc). Walks the run
/// list (here: the target list, the same mobys in its order, plus the ones the draw distance keeps): a moby with a draw
/// distance (+0x32), drawn (+0x31, only without an exclusion list) or not excluded, with a record whose health is not
/// negative, targetable, a creature (class type 5), within `range` (2D); its aim point 0.1 above the record's height;
/// inside `cone_yaw` of `angles.yaw` and `cone_pitch` of `angles.elevation`; score `d/5` within 5, else
/// `yaw²·pitch²·d + d`; the lowest score with a clear line (flags 2) wins.
#[allow(clippy::too_many_arguments)]
pub fn search(table: &MobyTable, list: &[MobyId], cone_yaw: f32, cone_pitch: f32, range: f32, from: [f32; 3], yaw: f32, elevation: f32, exclude: Option<&[Option<MobyId>; 7]>, class_type: &dyn Fn(i16) -> Option<u8>, mut clear: impl FnMut([f32; 3], [f32; 3], MobyId) -> bool) -> Option<MobyId> {
    let mut best = 1e9f32;
    let mut out = None;
    for &id in list {
        let Some(m) = table.mobys.get(id) else { continue };
        if m.draw_dist == 0 { continue; }
        match exclude {
            None => { if m.visible == 0 { continue; } }
            Some(x) => { if x.contains(&Some(id)) { continue; } }
        }
        let Some(health) = targeting::record_health(m) else { continue };
        if health < 0.0 || m.mode & mode::TARGETABLE == 0 || !m.has_class || class_type(m.o_class) != Some(5) { continue; }
        let d = ((m.position[0] - from[0]).powi(2) + (m.position[1] - from[1]).powi(2)).sqrt();
        if !(d < range) { continue; }
        let p = [m.position[0], m.position[1], m.position[2] + targeting::aim_height(m).unwrap_or(0.0) + 0.1];
        let dy = diff_rots(yaw, atan(p[0] - from[0], p[1] - from[1]));
        if !(dy * dy < cone_yaw * cone_yaw) { continue; }
        let dp = diff_rots(elevation, atan(d, p[2] - from[2]));
        if cone_pitch * cone_pitch <= dp * dp { continue; }
        let score = if 5.0 < d { dy * dy * dp * dp * d + d } else { d / 5.0 };
        if score < best && clear(from, p, id) {
            best = score;
            out = Some(id);
        }
    }
    out
}

/// The camera's yaw and elevation (0x167258 and −0x167254).
pub fn camera_angles(env: &ItemEnv) -> (f32, f32) {
    match env.camera {
        Some((_, f)) => (atan(f[0], f[1]), atan((f[0] * f[0] + f[1] * f[1]).sqrt(), f[2])),
        None => (0.0, 0.0),
    }
}

/// The new target's lock: the lock timer, the health left and the aim height from its record.
fn lock(r: &mut Ryno, table: &MobyTable, t: Option<MobyId>) {
    r.target = t;
    r.lock_timer = ticks(20);
    r.left = 1.0;
    r.aim_height = 0.0;
    if let Some(m) = t.and_then(|t| table.mobys.get(t)) {
        if let Some(rec) = targeting::record(m) {
            r.aim_height = f32::from_le_bytes(m.pvars[rec + 0x10..rec + 0x14].try_into().unwrap());
            r.left = f32::from_le_bytes(m.pvars[rec..rec + 4].try_into().unwrap());
        }
    }
}

/// States 2 and 3 on ○ (the same code twice in `0x2e4e60`): with one ammo (`0x249450`), the shot statistics
/// 0x141738.. (not kept: G-SAV-009), the swap lock 2, state 4, the shot timer, barrel and count 0, the weapon drawn
/// (`0x22ee08`), the past targets cleared.
fn start_salvo(hero: &mut Hero, id: i32) {
    if !hero.weapons.use_ammo(id, 1) { return; }
    hero.items.slot.swap = 2;
    if let Some(it) = hero.items.slot.item.as_mut() { it.mstate = 4; }
    let r = &mut hero.weapons.ryno;
    r.shot_timer = 0;
    r.barrel = 0;
    r.count = 0;
    r.past = [None; 7];
    r.next = 0;
    hero.weapons.pending_draw = true;
}

/// `0x2e4e60`, the R.Y.N.O.'s update (from the slot loop with the slot ready).
pub fn update(hero: &mut Hero, table: &mut MobyTable, _anim: &dyn super::anim::AnimCtl, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng) {
    let Some(item) = hero.items.slot.item.as_ref() else { return };
    let mstate = item.mstate;
    let wrapped = item.anim.flags & 2 != 0;
    let item_pos = item.position;
    let id = hero.items.slot.id;
    let tick = env.frame as u64;
    let coll = env.coll;
    // The class types of the listed mobys (class header +0x46), read once.
    let types: Vec<(i16, Option<u8>)> = env.targets.iter().filter_map(|&t| table.mobys.get(t)).map(|m| (m.o_class, hits.class_type(m.o_class))).collect();
    let class_type = |c: i16| types.iter().find(|x| x.0 == c).and_then(|x| x.1);
    let clear = |a: [f32; 3], b: [f32; 3], _m: MobyId| coll.is_none_or(|c| super::physics::line_world(c, super::physics::from_f32x3(a), super::physics::from_f32x3(b), 2).is_none());
    if mstate.wrapping_sub(2) < 2 {
        let r = &mut hero.weapons.ryno;
        if !guns::dec(&mut r.lock_timer) {
            if r.target.is_some_and(|t| dead(table, t)) { r.target = None; }
            if r.target.is_none() { r.aim_height = 0.0; }
        } else {
            r.target = None;
            r.aim_height = 0.0;
        }
        let (yaw, elev) = camera_angles(env);
        let muzzle = guns::item_point(hero, env, hero.weapons.ryno.barrel as usize);
        let found = search(table, env.targets, CONE, CONE, RANGE, muzzle, yaw, elev, None, &class_type, clear);
        let r = &mut hero.weapons.ryno;
        let keep = r.target.is_some_and(|t| !dead(table, t) && table.mobys[t].visible != 0);
        if !keep { lock(r, table, found); }
        if let Some(t) = r.target {
            if guns::dec16(&mut r.marker_wait) && hero.weapons.has_ammo(id) != 0 {
                let r = &mut hero.weapons.ryno;
                if dead(table, t) {
                    r.target = None;
                    r.aim_height = 0.0;
                } else {
                    let cam = env.camera.map_or(muzzle, |c| c.0);
                    let m = &table.mobys[t];
                    let d = len3(sub3([m.position[0], m.position[1], m.position[2]], cam));
                    let size = (0.9 - (d / 150.0) * 0.65).abs();
                    let at = [m.position[0], m.position[1], m.position[2] + r.aim_height];
                    hero.weapons.markers.register(tick, Marker { size, angle: 0.0, rgba: super::blaster::MARKER_GREEN, at: Some(at), fx: MARKER_FX });
                }
            }
        }
    }
    let set_state = |hero: &mut Hero, s: u8| if let Some(it) = hero.items.slot.item.as_mut() { it.mstate = s; };
    match mstate {
        0 => {
            let r = &mut hero.weapons.ryno;
            r.target = None;
            hero.items.slot.swap = 0;
            r.marker_wait = 0;
            set_state(hero, 1);
        }
        1 => { if wrapped { set_state(hero, 2); } }
        2 => {
            if hero.state == 1 { hero.weapons.deferred = Some(0x1e); }
            if env.pad.pressed & hero.items.slot.fire_mask == 0 || hero.items.f13fc != 0 { return; }
            start_salvo(hero, id);
        }
        3 => {
            // The first-person variant (0x2e5264): the red crosshair at the screen centre (`FUN_0020fb60(1.0, 0, 90.0,
            // item, 0xff0f0fff, none, 0x23, −1, 4)`) every tick; while L1 or L2 is held (0x13cae0 & 5) ○ fires the
            // salvo as state 2 does; without them, back to state 2.
            hero.weapons.markers.register(tick, Marker { size: 1.0, angle: 0.0, rgba: super::blaster::MARKER_RED, at: None, fx: MARKER_FX });
            if env.pad.held & (crate::pad::button::L1 | crate::pad::button::L2) == 0 {
                set_state(hero, 2);
                return;
            }
            if env.pad.pressed & hero.items.slot.fire_mask == 0 || hero.items.f13fc != 0 { return; }
            start_salvo(hero, id);
        }
        4 => {
            if !guns::dec16(&mut hero.weapons.ryno.shot_timer) { return; }
            {
                let r = &mut hero.weapons.ryno;
                r.shot_timer = ticks(9) as i16;
                r.barrel = (r.barrel + 1) % BARRELS;
                r.count += 1;
            }
            let fwd = match (hero.state == 0x1e, env.camera) {
                (true, Some((_, f))) => f,
                _ => to_f32x3(hero.moby_rows[0]),
            };
            let motion = scale3(add3(to_f32x3(hero.disp), to_f32x3(hero.plat_applied)), 3.0);
            let dir = add3(fwd, motion);
            let (yaw, pitch) = guns::aim_angles(dir);
            let muzzle = guns::item_point(hero, env, hero.weapons.ryno.barrel as usize);
            hero.weapons.pending_draw = true;
            let (aimh, target) = (hero.weapons.ryno.aim_height, hero.weapons.ryno.target);
            crate::moby_update::classes::ryno_missile::spawn(hero, table, env, hits, rng, aimh, yaw, pitch, muzzle, target);
            if SALVO <= hero.weapons.ryno.count {
                hero.items.slot.swap = 0;
                super::weapons::put_away(hero);
                set_state(hero, 5);
                let r = &mut hero.weapons.ryno;
                r.shot_timer = ticks(60) as i16;
                r.marker_wait = ticks(60) as i16;
            }
            let r = &mut hero.weapons.ryno;
            r.left -= 1.0;
            if 0.0 < r.left && r.target.is_some_and(|t| !dead(table, t)) { return; }
            let k = r.next.clamp(0, 6) as usize;
            r.past[k] = r.target;
            r.next = (r.next + 1) % 7;
            let (yaw, elev) = camera_angles(env);
            let past = r.past;
            let found = search(table, env.targets, std::f32::consts::PI, std::f32::consts::PI, SALVO_RANGE, item_pos, yaw, elev, Some(&past), &class_type, clear);
            let r = &mut hero.weapons.ryno;
            let pick = match found {
                Some(t) => Some(t),
                None if r.next < 1 => None,
                None => {
                    if rng.rand() & (r.next as i32 + 1) != 0 {
                        None
                    } else {
                        let n = r.next as i32;
                        past[(rng.rand() % n) as usize]
                    }
                }
            };
            lock(r, table, pick);
        }
        5 => {
            if !guns::dec16(&mut hero.weapons.ryno.shot_timer) { return; }
            hero.weapons.ryno.target = None;
            hero.weapons.ryno.count = 0;
            set_state(hero, 2);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::Moby;

    fn creature(pos: [f32; 3], health: f32) -> Moby {
        let mut m = Moby { position: [pos[0], pos[1], pos[2], 1.0], mode: mode::TARGETABLE | 0x20, pvars: vec![0; 0x80], has_class: true, draw_dist: 0x40, visible: 1, o_class: 577, ..Moby::default() };
        m.pvars[0..4].copy_from_slice(&0x20u32.to_le_bytes());
        m.pvars[0x20..0x24].copy_from_slice(&health.to_le_bytes());
        m.pvars[0x30..0x34].copy_from_slice(&0.4f32.to_le_bytes());
        m
    }

    /// 0x2e4bb8: creatures only, alive, in the cones and range; the lowest score wins (close ones by distance / 5);
    /// the exclusion list replaces the drawn test.
    #[test]
    fn search_scores_and_filters() {
        let ty = |c: i16| if c == 577 { Some(5) } else { Some(0) };
        let from = [0.0, 0.0, 0.0];
        let t = MobyTable::new(vec![creature([10.0, 1.0, 0.0], 3.0), creature([4.0, 0.0, 0.0], 3.0), creature([90.0, 0.0, 0.0], 3.0)], 0);
        assert_eq!(search(&t, &[0, 1, 2], CONE, CONE, RANGE, from, 0.0, 0.0, None, &ty, |_, _, _| true), Some(1));
        let x = [Some(1), None, None, None, None, None, None];
        assert_eq!(search(&t, &[0, 1, 2], CONE, CONE, RANGE, from, 0.0, 0.0, Some(&x), &ty, |_, _, _| true), Some(0));
        let mut dead = creature([4.0, 0.0, 0.0], -1.0);
        dead.visible = 1;
        let t2 = MobyTable::new(vec![dead], 0);
        assert_eq!(search(&t2, &[0], CONE, CONE, RANGE, from, 0.0, 0.0, None, &ty, |_, _, _| true), None);
        // Behind (180°) is outside 97°; the salvo's π cone takes it.
        let t3 = MobyTable::new(vec![creature([-10.0, 0.0, 0.0], 3.0)], 0);
        assert_eq!(search(&t3, &[0], CONE, CONE, RANGE, from, 0.0, 0.0, None, &ty, |_, _, _| true), None);
        assert_eq!(search(&t3, &[0], std::f32::consts::PI + 0.01, std::f32::consts::PI + 0.01, SALVO_RANGE, from, 0.0, 0.0, None, &ty, |_, _, _| true), Some(0));
    }
}
