//! **The Suck Cannon** (item 9, class 849; level01 item update `0x303000`, no weapon-check case; read from the
//! decompiler output of 0x303000, 0x302ed0, 0x302bd0, 0x3028c8, 0x302ac0, 0x3027e8). It pulls creatures in, holds
//! them, and fires them back out as homing projectiles. Everything it does to a creature goes through the class
//! reaction table ([`crate::moby_update::creature::react`], the one per-class dispatch any weapon uses); the per-class
//! side is the creatures' held state (577: 7, 572 / 866: 0xe, the chicken 270: 5).
//!
//! **States** (item +0x20; pvars [`SuckCannon`]):
//! * 0 → the held mobys' collision off and untargetable, the swap lock 0x1403fc clear; none held → 1, else the loaded
//!   sequence 6 (4 ticks) → 4.
//! * 1 (empty): the item back to sequence 1 when a sequence wraps; its timer +0x08 counts, putting the arm away
//!   (`0x22efd8`) when it runs out; something held → 4. ○ held (the fire mask) with the timer out, not 0x1413f7 /
//!   0x1413fc, ready for more than `ticks(15)`, in the groups `0x3027e8` allows (0, 1, 5, 0xc; 2 / 4 outside 0x3c) →
//!   the arm out (`0x22ee08`), the suction loop (class sound 2, looped), timer `ticks(10)` → 2.
//! * 2 (starting): sequence 3 after a wrap (5 ticks); a blocked group / 0x1413fc / 0x1413f7 stops (the loop released,
//!   the arm away, the swap lock clear; sequence 1 / 6 → 1 / 4); else after the timer, `ticks(20)` → 3.
//! * 3 (sucking): sequence 3 (5 ticks; 2 after a wrap); with nothing coming, the limit held (5; 10 gold), a blocked
//!   group, ○ let go with the timer out, 0x1413fc or 0x1413f7 stop it (as 2, plus `0x3078b8`'s vortex collapse);
//!   otherwise **the pull** ([`pull`], `0x302bd0`).
//! * 4 (loaded): the arm away when its timer runs out or a sequence wraps (sequence 6 again); ○ released clears the
//!   lock +0x0c; the look stance 1 becomes 0x1e; in first person (not 0x1413fc) a ray 50 along the view marks the
//!   targetable moby it hits (+0x10 for `ticks(20)`, +0x18 its hit height) and the crosshair (FX 0x21: red, green
//!   with a target); ○ held (not in the groups 3 / 7 / 0x11 / 0x12, ready for more than `ticks(22)`, the lock clear,
//!   something held, the timer out) → the arm out, the firing sequence 5 (2 ticks), the swap lock 2, timer
//!   `ticks(25)` → 5.
//! * 5 (firing): at the firing key (`MobyAnimKeyTime ≥ 0`) the first held moby leaves its slot at the muzzle (joint
//!   list 0; in first person 0.15 right, 0.3 down, 1 ahead of the eye) with `0.4462·dt`-per-tick… (0.4462 a tick)
//!   along the aim (Ratchet's aim row 0x13f990, or the view), plus Ratchet's velocity 0x13f450; outside first person
//!   it homes on the search's creature ([`fire_search`]); slot +0x08 fires it; → 4 (none left: 1, timer `ticks(20)`).
//!
//! **Native / inferred.** Standard `f32`. [L] The hand item is not a table moby: its position, mouth and rows are
//! published each tick for the creatures' carried update (`react::Cannon`), its sequence-4 request and the swallow's
//! sound 5 come back through the reaction globals and are made by the next update, and the swap lock the carried
//! update sets reaches the hand slot at the cannon's next update.
//!
//! **The vortex** (the tube and its smoke strands, [`super::suck_vortex`]): while sucking the pull runs `0x3067d0`
//! first; in the other states it fades (`0x307850`), state 0 resets it, the stop from state 3 collapses it. **The
//! vacuum** (`0x307a50`, [`vacuum`]): the pull's walk of the run list gives every live moby it does not pull a
//! `randi(10)`; on 0 a bolt or an ammo pickup within 30 of Ratchet and near a strong node is taken. **Sounds**: the
//! suction loop (class sound 2) on its own channel ([`super::fx::LOOP_ITEM`]: channel 0 is released every tick by the
//! Pyrocitor's `item_gone` when another item is in the hand — the cause of the "sucks for a moment" report).
//! **What it takes** (docs/plan/hero_gameplay.md §10.2, the full coverage table): the pull only mobys whose class has a
//! reaction table (never a crate: every crate class keeps the default table on all 19 levels), the vacuum only bolts
//! and the 8 ammo pickups. Its calls into class code run on the moby world the hit sink builds, with the moby loop's
//! item tables and sound layer (the vacuumed bolts' pickup sound `bolt::start_fly`, `pickup::collect`'s `AddAmmo`,
//! whose hero-block write goes to Ratchet in the same tick).
//! The held-count HUD element (`queue_animation_update(4, 0x753f, …)`, every tick of the update) is `crate::hud`'s,
//! derived each tick from the cannon's state (`hud::Inputs::suck_active`, `update_suck` / `draw_suck`).
//! Not ported: the stats 0x1416c8..
//! (G-SAV-009), the gold cannon 0x13e529 (not mirrored: 5 slots; G-WPN-009). The pad's released mask 0x13cae8 |= 5 as it goes away with L1 / L2 held is ported (2026-10-01,
//! G-HERO-025: `Gadgets::released_or`, applied to the pad by the tick right after the item updates).

use super::guns::{self, add3, len3, scale3, sub3};
use super::items::{HitSink, ItemEnv};
use super::packs::SoundCmd;
use super::physics::{from_f32x3, ticks, to_f32x3};
use super::Hero;
use crate::moby_runtime::{mode, MobyId, MobyTable};
use crate::moby_update::creature::{atan, diff_rots, react};
use crate::rng::Rng;
use crate::targeting::Marker;
use rc_formats::moby_anim;

/// The Suck Cannon (item 9) and its class.
pub const SUCK_CANNON: i32 = 9;
pub const CLASS: i16 = 849;
/// The crosshair (`FUN_0020fb60(1, 0, 90, …, 0x21, …)`).
pub const MARKER_FX: usize = 0x21;
pub const MARKER_RED: u32 = 0xff0f_0fff;
pub const MARKER_GREEN: u32 = 0xff0f_ff0f;
/// The pull's reach (`0x302bd0`: 15, 3-D), its cones (squared radians: (12°)², or (110°)² within 3).
pub const REACH: f32 = 15.0;
pub const CONE2: f32 = 0.043_864_91;
pub const CONE2_NEAR: f32 = 3.685_870_6;
/// The fired moby's speed a tick (0.4462).
pub const SHOT: f32 = 0.4462;
/// The fire search's cone (26°).
pub const FIRE_CONE: f32 = 0.453_785_6;
fn len2(a: [f32; 3]) -> f32 { (a[0] * a[0] + a[1] * a[1]).sqrt() }

/// The Suck Cannon's pvars (item moby +0x78).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SuckCannon {
    /// +0x08: the timer; +0x0c: the fire lock (○ must be let go).
    pub timer: i32,
    pub lock: i32,
    /// +0x10 / +0x14 / +0x18: the first-person target, its timer, its hit height above it.
    pub target: Option<MobyId>,
    pub target_timer: i32,
    pub target_dz: f32,
    /// Fired (the port's count).
    pub fired: u32,
    /// The mobys the last pull took (the port's record, for tests).
    pub pulled: Vec<MobyId>,
    /// The vortex's globals ([`super::suck_vortex`]).
    pub vortex: super::suck_vortex::Vortex,
    /// Bolts / ammo pickups the vortex's vacuum took (the port's count).
    pub vacuumed: u32,
}

/// `0x3027e8`: the groups the cannon works in (0 blocked).
pub fn group_ok(h: &Hero) -> bool {
    match h.group {
        0 | 1 | 5 | 0xc => true,
        2 | 4 => h.state != 0x3c,
        _ => false,
    }
}

fn blend(hero: &mut Hero, env: &ItemEnv, seq: u8, n: i32) {
    let Some(it) = hero.items.slot.item.as_mut() else { return };
    let Some(class) = env.data.class(it.o_class) else { return };
    if it.anim.seq_b != seq { moby_anim::set_sequence(&mut it.anim, &class.anim, seq, 0, ticks(n), &mut it.snapshot); }
}

fn wrapped(hero: &Hero) -> bool { hero.items.slot.item.as_ref().is_some_and(|it| it.anim.flags & 2 != 0) }

/// The aim of the pull (`0x302ac0`): the mouth (joint list 0; in the look stance 1.2 along the view from Ratchet at the
/// item's height), the yaw and pitch (the view's, else Ratchet's aim 0x13f9d8 / 0x13f9d4; the gun's pitch +0x44 with
/// the arm raised at a wall, 0x141618).
fn aim(hero: &Hero, env: &ItemEnv) -> ([f32; 4], f32, f32) {
    let m = guns::item_point(hero, env, 0);
    let mut mouth = [m[0], m[1], m[2], 1.0];
    if hero.state == 1 || hero.state == 0x1e {
        if let Some((_, f)) = env.camera {
            let hp = to_f32x3(hero.pos);
            let z = hero.items.slot.item.as_ref().map_or(m[2], |it| it.position[2]);
            mouth = [hp[0] + 1.2 * f[0], hp[1] + 1.2 * f[1], z + 1.2 * f[2], 1.0];
            return (mouth, atan(f[0], f[1]), atan(len2(f), f[2]));
        }
    }
    // The arm raised at a wall (0x141618 = 1): the pitch is the gun's own (its +0x44).
    let pitch = if guns::arm_raised(hero) { guns::item_euler(hero).map_or(hero.rot[1].to_f32(), |e| e[1]) } else { hero.rot[1].to_f32() };
    (mouth, hero.rot[2].to_f32(), pitch)
}

/// The vortex's frame (`0x3067d0`): the view's rows in the look stance under the first-person camera (forward,
/// `up × forward` [L], up), with the arm raised at a wall the gun's (−row 1, row 0, row 2), else Ratchet's aim rows
/// 0x13f990 (his moby rows).
fn aim_rows(hero: &Hero, env: &ItemEnv) -> [[f32; 3]; 3] {
    if (hero.state == 1 || hero.state == 0x1e) && hero.f13f5 != 0 {
        if let (Some((_, f)), Some(u)) = (env.camera, env.camera_up) {
            return [f, guns::cross3(u, f), u];
        }
    }
    // The arm raised at a wall (0x141618 = 1; also the start `0x306528`): the gun's −row 1, row 0, row 2.
    if let (true, Some(r)) = (guns::arm_raised(hero), guns::item_rows(hero)) {
        return [[-r[1][0], -r[1][1], -r[1][2]], r[0], r[2]];
    }
    hero.moby_rows.map(to_f32x3)[..3].try_into().unwrap()
}

/// The cannon as the creatures read it (`react::Cannon`).
fn publish(hero: &Hero, env: &ItemEnv) -> Option<react::Cannon> {
    let it = hero.items.slot.item.as_ref()?;
    Some(react::Cannon { pos: it.position, mouth: guns::item_point(hero, env, 0), rows: it.rows.map(|r| [f32::from_bits(r[0]), f32::from_bits(r[1]), f32::from_bits(r[2])]) })
}

/// `0x303000`, the Suck Cannon's update (from the slot loop with the slot ready).
pub fn update(hero: &mut Hero, table: &mut MobyTable, _anim: &dyn super::anim::AnimCtl, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng) {
    if hero.items.slot.item.is_none() { return; }
    let tick = env.frame as u64;
    let away = hero.items.slot.state == 3;
    let cannon = publish(hero, env);
    let hero_moby = env.hero_moby;
    // The reaction globals: publish the cannon, take the moby side's requests, recount the slots (0x302ed0).
    let mut g = None;
    hits.world(table, &*hero, rng, tick, &mut |w| {
        let r = &mut w.svc.creatures.react;
        r.cannon = cannon;
        r.hero = Some(hero_moby);
        let req = (r.swap_lock.take(), std::mem::take(&mut r.cannon_seq4), std::mem::take(&mut r.cannon_sound5));
        recount(w);
        g = Some((req, w.svc.creatures.react.held, w.svc.creatures.react.coming, w.svc.creatures.react.limit() as i32));
    });
    let Some(((lock, seq4, sound5), mut held, coming, limit)) = g else { return };
    if let Some(l) = lock { hero.items.slot.swap = l; }
    if seq4 {
        let n = 2;
        if let Some(it) = hero.items.slot.item.as_mut() {
            if let Some(class) = env.data.class(it.o_class) { moby_anim::set_sequence(&mut it.anim, &class.anim, 4, 0, ticks(n), &mut it.snapshot); }
        }
    }
    for _ in 0..sound5 { hero.fx.item_sounds.push(5); }
    let mask = hero.items.slot.fire_mask;
    let held_btn = env.pad.held & mask != 0;
    let busy = hero.items.f13f7 != 0 || hero.items.f13fc != 0;
    let st = hero.items.slot.item.as_ref().map_or(0, |it| it.mstate);
    // 0x307850: the vortex fades while not sucking.
    if st != 0 && st != 3 { hero.weapons.reactive.suck.vortex.fade(tick); }
    let set_st = |hero: &mut Hero, s: u8| { if let Some(it) = hero.items.slot.item.as_mut() { it.mstate = s; } };
    match st {
        0 => {
            hits.world(table, &*hero, rng, tick, &mut |w| {
                let limit = w.svc.creatures.react.limit();
                for i in 0..limit {
                    let s = w.svc.creatures.react.slots[i];
                    if s == 0 { continue; }
                    let m = (s - 1) as usize;
                    if m < w.table.mobys.len() {
                        w.mm(m).has_collision = false;
                        w.mm(m).mode &= !mode::TARGETABLE;
                    }
                }
            });
            hero.items.slot.swap = 0;
            hero.weapons.reactive.suck.vortex.reset();
            if held < 1 {
                hero.weapons.reactive.suck.timer = 0;
                set_st(hero, 1);
            } else {
                hero.weapons.reactive.suck.lock = 1;
                blend(hero, env, 6, 4);
                set_st(hero, 4);
            }
        }
        1 => {
            if wrapped(hero) { hard_blend(hero, env, 1); }
            let s = &mut hero.weapons.reactive.suck;
            guns::dec(&mut s.timer);
            if s.timer == 0 { super::weapons::put_away(hero); }
            if 0 < held {
                blend(hero, env, 6, 4);
                set_st(hero, 4);
            } else if held_btn && hero.weapons.reactive.suck.timer == 0 && !busy && ticks(0xf) < hero.items.slot.ticks_ready && group_ok(hero) {
                hero.weapons.pending_draw = true;
                hero.fx.item_voices.push(SoundCmd::ItemLoop { n: super::fx::LOOP_ITEM, index: 2, flags: 4 });
                hero.weapons.reactive.suck.timer = ticks(10);
                set_st(hero, 2);
            }
        }
        2 => {
            if wrapped(hero) { blend(hero, env, 3, 5); }
            if !group_ok(hero) || busy {
                stop(hero, env, held, false);
            } else if guns::dec(&mut hero.weapons.reactive.suck.timer) {
                hero.weapons.reactive.suck.timer = ticks(20);
                set_st(hero, 3);
            }
        }
        3 => {
            let seq_b = hero.items.slot.item.as_ref().map_or(0, |it| it.anim.seq_b);
            if seq_b != 3 && (seq_b != 0 || wrapped(hero)) { blend(hero, env, 3, 5); }
            guns::dec(&mut hero.weapons.reactive.suck.timer);
            let stop_now = coming == 0 && (limit <= held || !group_ok(hero) || (!held_btn && hero.weapons.reactive.suck.timer == 0) || busy);
            if stop_now {
                stop(hero, env, held, true);
                // 0x3078b8: the vortex collapses.
                let mut v = std::mem::take(&mut hero.weapons.reactive.suck.vortex);
                hits.world(table, &*hero, rng, tick, &mut |w| v.collapse(w, tick));
                hero.weapons.reactive.suck.vortex = v;
            } else {
                if wrapped(hero) { blend(hero, env, 3, 2); }
                let (mouth, yaw, pitch) = aim(hero, env);
                let group_blocked = !group_ok(hero);
                let pos = cannon.map_or([0.0; 3], |c| c.pos);
                let rows = aim_rows(hero, env);
                let cam = env.camera.map_or([0.0; 3], |c| c.0);
                let mut v = std::mem::take(&mut hero.weapons.reactive.suck.vortex);
                let mut taken = Vec::new();
                let mut vac = 0;
                let mut writes = None;
                hits.world(table, &*hero, rng, tick, &mut |w| {
                    // 0x302bd0: the vortex (0x3067d0), then the run list: the pull, and the vacuum for the others.
                    v.update(w, rows, [mouth[0], mouth[1], mouth[2]], Some(hero_moby), tick);
                    w.camera = [cam[0], cam[1], cam[2], 1.0].map(crate::ps2v::Pf::f);
                    let list = crate::moby_update::scheduler::build_active_list(w.table, w.camera, &w.svc.groups).0;
                    (taken, vac) = pull(w, &list, pos, mouth, yaw, pitch, group_blocked, Some(&v));
                    // The vacuum's `AddAmmo` (0x2db850) stores into 0x13d428 at once: its hero-block writes of this
                    // tick go to Ratchet now, not at the next moby loop.
                    let now = w.counter;
                    writes = w.svc.hero_writes.take_if(|(c, _)| *c == now).map(|(_, f)| f);
                });
                if let Some(f) = writes { f.apply(hero); }
                hero.weapons.reactive.suck.vortex = v;
                hero.weapons.reactive.suck.pulled = taken;
                hero.weapons.reactive.suck.vacuumed += vac;
            }
        }
        4 => {
            let s = &mut hero.weapons.reactive.suck;
            if guns::dec(&mut s.timer) { super::weapons::put_away(hero); }
            if wrapped(hero) {
                hard_blend(hero, env, 6);
                super::weapons::put_away(hero);
            }
            if !held_btn { hero.weapons.reactive.suck.lock = 0; }
            let s = &mut hero.weapons.reactive.suck;
            if !guns::dec(&mut s.target_timer) {
                if s.target.is_some_and(|t| table.mobys.get(t).is_none_or(|m| m.state & 0x80 != 0)) { s.target = None; }
            } else {
                s.target = None;
            }
            if hero.state == 1 { hero.weapons.deferred = Some(0x1e); }
            if hero.state == 0x1e && hero.f13f5 != 0 && hero.items.f13fc == 0 {
                if let Some((eye, f)) = env.camera {
                    let end = add3(eye, scale3(f, 50.0));
                    if let Some(Some(p)) = hits.probe_moby(table, from_f32x3(eye), from_f32x3(end), 0, None) {
                        if let Some(m) = p.moby.filter(|&m| table.mobys.get(m).is_some_and(|mo| mo.mode & mode::TARGETABLE != 0)) {
                            let s = &mut hero.weapons.reactive.suck;
                            s.target = Some(m);
                            s.target_timer = ticks(0x14);
                            s.target_dz = p.point[2] - table.mobys[m].position[2];
                        }
                    }
                }
                let s = &hero.weapons.reactive.suck;
                let rgba = if s.target.is_none() || s.timer != 0 { MARKER_RED } else { MARKER_GREEN };
                hero.weapons.markers.register(tick, Marker { size: 1.0, angle: 0.0, rgba, at: None, fx: MARKER_FX });
            }
            let s = &hero.weapons.reactive.suck;
            if s.lock == 0 && 0 < held && s.timer == 0 && held_btn && !matches!(hero.group, 7 | 3 | 0x12) && hero.group != 0x11 && !busy && ticks(0x16) < hero.items.slot.ticks_ready {
                hero.weapons.pending_draw = true;
                blend(hero, env, 5, 2);
                hero.items.slot.swap = 2;
                hero.weapons.reactive.suck.timer = ticks(0x19);
                set_st(hero, 5);
            }
        }
        5 => {
            let key = hero.items.slot.item.as_ref().and_then(|it| env.data.class(it.o_class).map(|c| crate::moby_update::creature::ground::key_time_of(&c.anim, &it.anim))).unwrap_or(-1.0);
            if 0.0 <= key {
                let look = guns::first_person(hero);
                let (vel, at) = match (look, env.camera) {
                    (true, Some((_, f))) => (scale3(f, SHOT), guns::camera_point(env, [0.15, 0.3, 1.0]).unwrap_or(guns::item_point(hero, env, 0))),
                    _ => (scale3(to_f32x3(hero.moby_rows[0]), SHOT), guns::item_point(hero, env, 0)),
                };
                let vel = add3(vel, to_f32x3(hero.vel));
                let mut target = hero.weapons.reactive.suck.target;
                let dz = hero.weapons.reactive.suck.target_dz;
                let targets = env.targets.to_vec();
                let cam = env.camera.map_or(at, |c| c.0);
                let hero_pos = to_f32x3(hero.pos);
                let hero_yaw = hero.rot[2].to_f32();
                let mut fired = false;
                hits.world(table, &*hero, rng, tick, &mut |w| {
                    let r = &mut w.svc.creatures.react;
                    let limit = r.limit();
                    let Some(i) = (0..limit).find(|&i| r.slots[i] != 0) else { return };
                    let obj = (r.slots[i] - 1) as usize;
                    r.slots[i] = 0;
                    w.mm(obj).position = [at[0], at[1], at[2], w.m(obj).position[3]];
                    react::sphere_lerp(w, obj);
                    if !look {
                        if let Some(t) = fire_search(w, &targets, obj, at, vel, cam, hero_pos, hero_yaw) { target = Some(t); }
                    }
                    // FUN_00302840(cannon, moby, gp−0x4d50): the cannon's rows turned by (π/2, 0, π/2).
                    react::cannon_frame(w, obj, react::FIRE_EULER);
                    react::slot_fire(w, obj, dz, [vel[0], vel[1], vel[2], 0.0], target);
                    fired = true;
                });
                hero.weapons.reactive.suck.target = target;
                if fired { hero.weapons.reactive.suck.fired += 1; }
                held -= 1;
                hits.world(table, &*hero, rng, tick, &mut |w| { w.svc.creatures.react.held = held; });
                hero.items.slot.swap = 0;
                if held < 1 {
                    hero.weapons.reactive.suck.timer = ticks(0x14);
                    set_st(hero, 1);
                } else {
                    set_st(hero, 4);
                }
            }
        }
        _ => {}
    }
    if away {
        hero.fx.item_voices.push(SoundCmd::ItemRelease { n: super::fx::LOOP_ITEM });
        // L1 / L2 held as it goes away: the pad's released mask 0x13cae8 |= 5 (applied by the tick right after the
        // item updates: the hand items get the pad read-only; `super::gadgets::Gadgets::released_or`).
        if env.pad.held & 5 != 0 { hero.gadgets.released_or |= 5; }
    }
}

fn hard_blend(hero: &mut Hero, env: &ItemEnv, seq: u8) {
    let Some(it) = hero.items.slot.item.as_mut() else { return };
    let Some(class) = env.data.class(it.o_class) else { return };
    moby_anim::set_sequence(&mut it.anim, &class.anim, seq, 0, 0, &mut it.snapshot);
}

/// States 2 / 3's stop: the loop released, the arm away, the swap lock clear; nothing held → 1 (sequence 1), else →
/// 4 (sequence 6, locked).
fn stop(hero: &mut Hero, env: &ItemEnv, held: i32, from_sucking: bool) {
    if from_sucking { super::weapons::put_away(hero); }
    hero.fx.item_voices.push(SoundCmd::ItemRelease { n: super::fx::LOOP_ITEM });
    if !from_sucking { super::weapons::put_away(hero); }
    hero.items.slot.swap = 0;
    let s = &mut hero.weapons.reactive.suck;
    if held < 1 {
        s.timer = 0;
        blend(hero, env, 1, 4);
        if let Some(it) = hero.items.slot.item.as_mut() { it.mstate = 1; }
    } else {
        blend(hero, env, 6, 4);
        if from_sucking {
            hero.weapons.reactive.suck.timer = 0;
            hero.weapons.reactive.suck.lock = 1;
        }
        if let Some(it) = hero.items.slot.item.as_mut() { it.mstate = 4; }
    }
}

/// `0x302ed0`: held / coming recounted from the slots (a slot whose moby lost its record, or is not pulled / held,
/// or has no slot index, is emptied).
fn recount(w: &mut crate::moby_update::services::World) {
    let limit = w.svc.creatures.react.limit();
    w.svc.creatures.react.held = 0;
    w.svc.creatures.react.coming = 0;
    for i in 0..limit {
        let s = w.svc.creatures.react.slots[i];
        if s == 0 { continue; }
        let m = (s - 1) as usize;
        let rec = (m < w.table.mobys.len()).then(|| react::record(w, m)).flatten();
        let ok = rec.map(|r| (crate::moby_update::creature::pi16(w, m, r + react::rec::STATE), crate::moby_update::creature::pi16(w, m, r + react::rec::SLOT)));
        match ok {
            Some((3, sl)) if sl != -1 => w.svc.creatures.react.coming += 1,
            Some((4, sl)) if sl != -1 => w.svc.creatures.react.held += 1,
            _ => w.svc.creatures.react.slots[i] = 0,
        }
    }
}

/// `0x302bd0(cannon, pvars)`: over the moby loop's run list: the targetable mobys with a reaction table are pulled
/// (module doc); Ratchet, the untargetable and the default-table ones go to the vortex's vacuum `0x307a50`
/// ([`vacuum`], with `vortex`). Returns the mobys taken or refreshed this tick and the vacuum's takes.
#[allow(clippy::too_many_arguments)]
pub fn pull(w: &mut crate::moby_update::services::World, list: &[MobyId], cannon: [f32; 3], mouth: [f32; 4], yaw: f32, pitch: f32, group_blocked: bool, vortex: Option<&super::suck_vortex::Vortex>) -> (Vec<MobyId>, u32) {
    let mut taken = Vec::new();
    let mut vac = 0;
    for &m in list {
        if m >= w.table.mobys.len() { continue; }
        let mo = w.m(m);
        if mo.o_class == 0 || mo.mode & mode::TARGETABLE == 0 || react::table(w, m).is_none() {
            if let Some(v) = vortex { vac += vacuum(w, m, v) as u32; }
            continue;
        }
        let Some(r) = react::record(w, m) else { continue };
        let st = crate::moby_update::creature::pi16(w, m, r + react::rec::STATE);
        if 5 < st { continue; }
        let p = w.m(m).position;
        let d = len3(sub3([p[0], p[1], p[2]], cannon));
        let r0 = &w.svc.creatures.react;
        let full = r0.limit() as i32 <= r0.held;
        if REACH <= d || group_blocked || full {
            if st != 0 { react::slot_let_go(w, m); }
            continue;
        }
        if 3 < st { continue; }
        if st == 3 {
            react::take(w, m, mouth);
            taken.push(m);
            continue;
        }
        let lim = if d < 3.0 { CONE2_NEAR } else { CONE2 };
        crate::moby_update::creature::set_pi16(w, m, r + react::rec::SLOT, -1);
        let pt = [p[0], p[1], p[2] + 0.4];
        let dy = diff_rots(yaw, atan(pt[0] - cannon[0], pt[1] - cannon[1]));
        if dy * dy < lim {
            let dp = diff_rots(pitch, atan(d, pt[2] - cannon[2]));
            if dp * dp < lim {
                let blocked = w.line(mouth.map(crate::ps2v::Pf::f), [pt[0], pt[1], pt[2], 0.0].map(crate::ps2v::Pf::f), 2, Some(m)).is_some();
                if !blocked {
                    react::take(w, m, mouth);
                    taken.push(m);
                    continue;
                }
            }
            if 0 < st { react::slot_let_go(w, m); }
        } else if 1 <= st {
            react::slot_let_go(w, m);
        }
    }
    (taken, vac)
}

/// `0x2732b8`: the ammo pickup classes the vacuum takes.
pub const PICKUP_CLASSES: [i16; 8] = [0xe2, 0xcc, 0xde, 0x3ee, 0xd6, 0xe1, 0xd5, 0xdf];

/// `0x307a50(moby)`: a live moby draws `randi(10)`; on 0 a bolt (class type 0x13) or an ammo pickup
/// ([`PICKUP_CLASSES`]) within 30 of Ratchet and inside the vortex's reach (`Vortex::in_vacuum`) is taken: the bolt flies
/// to Ratchet (`0x2bcb90`), the pickup is collected (`0x2db850`). True when taken.
pub fn vacuum(w: &mut crate::moby_update::services::World, m: MobyId, v: &super::suck_vortex::Vortex) -> bool {
    let st = w.m(m).state;
    if st == crate::moby_runtime::state::DELETED || st == crate::moby_runtime::state::DELETED_STATIC { return false; }
    if w.rng.randi(10) != 0 { return false; }
    let o = w.m(m).o_class;
    let bolt = w.m(m).has_class && w.classes.info(o).map(|i| i.ty) == Some(0x13);
    if !bolt && !PICKUP_CLASSES.contains(&o) { return false; }
    let p = w.m(m).position;
    let h = w.hero.pos.map(|x| f32::from_bits(x.0));
    if 30.0 <= len3(sub3([p[0], p[1], p[2]], [h[0], h[1], h[2]])) { return false; }
    if !v.in_vacuum([p[0], p[1], p[2]]) { return false; }
    if bolt {
        crate::moby_update::classes::bolt::start_fly(w, m, crate::ps2v::Pf::ZERO, crate::ps2v::Pf::ZERO)
    } else {
        crate::moby_update::classes::pickup::collect(w, m)
    }
}

/// State 5's search (0x303000, outside first person): over the run list's targetable creatures (class type 5), a
/// creature within 2.5 that Ratchet faces (60°, below 45° of elevation) at once; else inside the 26° cone around the
/// shot (the bounding radius +0x0c / 1024 widening it), nearer than the best so far, with a clear camera line (flags 6;
/// a blocked one ends the search): its distance and elevation become the search's best and pitch, and it is taken —
/// the cone's yaw turning to it — only when its yaw is within 10° of the cone's (`FastDiffRots`).
#[allow(clippy::too_many_arguments)]
pub fn fire_search(w: &crate::moby_update::services::World, list: &[MobyId], obj: MobyId, from: [f32; 3], vel: [f32; 3], cam: [f32; 3], hero_pos: [f32; 3], hero_yaw: f32) -> Option<MobyId> {
    let mut yaw = atan(vel[0], vel[1]);
    let mut pitch = -atan(len2(vel), vel[2]);
    let mut best = 10000.0f32;
    let mut found = None;
    for &m in list {
        if m >= w.table.mobys.len() || m == obj { continue; }
        let mo = w.m(m);
        if mo.state & 0x80 != 0 { continue; }
        let h = crate::targeting::aim_height(mo).unwrap_or(0.5);
        let t = [mo.position[0], mo.position[1], mo.position[2] + h];
        if mo.mode & mode::TARGETABLE == 0 || !mo.has_class || w.classes.info(mo.o_class).map(|i| i.ty) != Some(5) { continue; }
        let ty = atan(t[0] - from[0], t[1] - from[1]);
        let d = len3(sub3(t, from));
        let el = atan(d, t[2] - from[2]);
        let dy = diff_rots(yaw, ty);
        if d < 2.5 {
            let hy = atan(mo.position[0] - hero_pos[0], mo.position[1] - hero_pos[1]);
            if diff_rots(hero_yaw, hy) < std::f32::consts::FRAC_PI_3 && el.abs() < std::f32::consts::FRAC_PI_4 { return Some(m); }
        }
        if best < d { continue; }
        let a = add3(crate::targeting::polar(d, yaw, pitch), from);
        let c = len3(sub3(a, t));
        let mut ang = std::f32::consts::FRAC_PI_2 - (1.0 - (c * c) / ((d + d) * d)).clamp(-1.0, 1.0).asin();
        if FIRE_CONE < ang {
            let r = mo.bsphere[3] / 1024.0;
            let hh = (d * d + r * r).sqrt();
            if r < d { ang -= std::f32::consts::FRAC_PI_2 - (d / hh).clamp(-1.0, 1.0).asin(); }
        }
        if ang < FIRE_CONE {
            let clear = w.line([cam[0], cam[1], cam[2], 0.0].map(crate::ps2v::Pf::f), [t[0], t[1], t[2], 0.0].map(crate::ps2v::Pf::f), 6, Some(obj)).is_none();
            if !clear { return found; }
            best = d;
            pitch = -el;
            if dy < TURN_MAX {
                yaw = ty;
                found = Some(m);
            }
        }
    }
    found
}

/// The fire search's turn limit (0.17453292: 10°).
pub const TURN_MAX: f32 = 0.174_532_92;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups() {
        let mut h = Hero::default();
        for (g, s, ok) in [(0, 0, true), (1, 3, true), (2, 0x3c, false), (2, 7, true), (4, 7, true), (6, 0x23, false), (0xc, 0, true), (0xf, 0, false)] {
            h.group = g;
            h.state = s;
            assert_eq!(group_ok(&h), ok, "group {g:#x} state {s:#x}");
        }
    }
}
