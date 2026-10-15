//! **The Walloper** (item 18, class 180; level01 item update `0x2d11c0`, its draw callback `0x2d1768` → the arcs'
//! strips `0x2d1830` and the fist glow `0x2d1e08`) and **the gadget lunge, hero state 0x20** (the weapon check's case
//! 0x12 in `HeroPdaGadget` 0x240ed8, SetState `0x23cf98` group 6, the physics case in `HeroStatePhysics` 0x2370b8 with
//! its hit helper `0x236cb8`, the transitions case in `0x242930`). Read from the decompiler output and checked against
//! the disassembly (the spheres' ignored mobys, the weapon check's pad window, the item's blend call). The coverage
//! table (every call, branch and side effect) is docs/plan/hero_gameplay.md §15.
//!
//! **Fire** ([`fire`], case 0x12): in movement group 0 / 1 (or 4 once landed, 0x13f768), not in the look stance 1, the
//! fire button (○) pressed within `min(ticks(10), the slot's ready time 0x140400)` → `SetState(0x20, 1)` and the item's
//! class sound 0 (the swing).
//!
//! **The lunge 0x20**: the entry (the group-6 part is `super::melee`'s) plays Ratchet's sequence 0x67 from frame 4 on
//! the eased curve 1, blends the item to its sequence 3 frame 4 (curve), and aims (`0x2351d0(11, 50°, −1)`: the stick,
//! or the best target of the melee search `0x22e238`, [`Hero::aim_assist`]). The physics (0x239d84): aimed or with a
//! target it turns to the aim (TurnTo 0.05 / 0.2, 15.0098 rad/s), else it re-aims during the first 4 ticks; the
//! target speed is `18·dt` for ticks 10 < T < 18 (0 otherwise), SpeedStep(190·dt², 90·dt²) after tick 10, the velocity
//! along the aim (or the facing); the after-images (`0x277400` + ghosts 0x30 / 0x17 / 0x0c at 2 / 4 / 6 ticks back at
//! tick 8, `0x277508` every tick, fading by 5 a tick after tick 18); for 10 < T < 22 with the item out, three hit
//! spheres of 0.8 at 0.8 ahead of Ratchet (0.55 up) at his yaw, yaw − 50° and yaw + 50° (the second ignores
//! Ratchet's moby, the other two the item), template: push `(2 cos yaw, 2 sin yaw, 1.3)` with the exact-push marker,
//! Ratchet the attacker, flags 0x30000, type 0 / 3, class = the item's, damage 3; each sphere's list through `0x236cb8`:
//! the first sphere of the lunge that lists anything plays the item's class sound 1 (0x13fdb4), and every listed moby
//! of class type 5 (creatures) or 9 gets four sparkle pairs (`0x2bdb18` = `super::fx::sparkle_burst`) at its position
//! 0.5 up; the edge brake (3.7) after tick 10, the wall check, gravity (24·dt² from the effective velocity in the air
//! with the steep-wall stop, 54·dt² on the ground). The transitions: with the item out, not blending and past frame 20
//! → `SetState(0, 0)` (without the item the lunge never ends by itself, as in the game).
//!
//! **The item update 0x2d11c0** ([`update`]): its state +0x20 0 → 2 (the ten arcs cleared, the fade timers
//! `ticks(5)` / `ticks(20)`); in state 2, while Ratchet lunges and the item's key time is 6 or more the arcs are
//! "active" and the glow intensity 0x161704 runs 0.2 → 1 over the fade-in, 1 until key time 12, then down over the
//! fade-out to 0; out of the lunge the timers are reset and the item blends to its sequence 1 frame 0 over 12 ticks
//! (every tick, as the game does). The arcs (0x1dc020.., ten records of five points, their timers 0x1dc340 and alphas
//! 0x1dc358): each tick a timer counts down; at 0 an active arc is re-made (timer 2, alpha 0x40): from the fist (the
//! item's position − 0.5·row 1) four 0.2-long segments, each turned about the view ray by ±10°..45° alternately (the
//! jagged bolt); otherwise it follows the fist, each segment jittered by `randf_sym(0, 0.1)` per axis (alpha 0x20). With
//! an arc alive or active, the draw callback is registered (list 2).
//!
//! **The draw** ([`draw_quads`], `rc-engine` walloper_render): per live arc the two strips of `0x2d1830` (the same
//! code as the Tesla Claw's `0x2d0748`: `super::tesla::strip_colored`; core FX 14 white 0.05 across, glow FX 16
//! 0x7f2020 0.5 across lengthened 0.2), alpha × the intensity; the fist glow: one quad of 0.75 (FX 8) facing the eye
//! 0.2 toward it from the fist, colour 0x307f4040 (or 0x7f7f4040 one tick in four: the draw's `randi(4)`, run where
//! the game draws it, `crate::moby_update::classes::draw_callbacks`), its alpha × the intensity. Additive (ALPHA 0x48).
//!
//! **Native.** Standard `f32` for the new code (the item update, the draw, the hit spheres); the state's physics calls
//! the hero block's existing PS2-float helpers (SpeedStep, TurnTo, …) as the other states do. No ammo (the Walloper
//! has none), no holding layer (def +0x18 = 0), no first-person case (the lunge refuses state 1; 0x1e is the look
//! stance moved by mobys only), no loop sound, no gold version.

use super::anim::{AnimCtl, AnimView};
use super::guns::{add3, cross3, scale3, sub3, with_len};
use super::items::{HitSink, ItemEnv};
use super::physics::*;
use super::states::Ctx;
use super::tesla::BeamQuad;
use super::Hero;
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::services::HitTemplate;
use crate::ps2v::Pf;
use crate::rng::Rng;

/// Item id (level01 item table 0x179f40: name id 18) and its class.
pub const WALLOPER: i32 = 18;
pub const CLASS: i16 = 180;
/// Hero state: the gadget lunge.
pub const LUNGE: i32 = 0x20;
/// Ratchet's lunge sequence and the item's (the entry).
pub const HERO_SEQ: u8 = 0x67;
pub const ITEM_SEQ: u8 = 3;
/// The item's class sounds: the swing (the weapon check) and the hit (`0x236cb8`).
pub const SWING_SOUND: i32 = 0;
pub const HIT_SOUND: i32 = 1;
/// The aim: range 11, cone 50° (`0x3f5f66f3`), no elevation limit.
pub const AIM_RANGE: f32 = 11.0;
pub const SIDE: f32 = f32::from_bits(0x3f5f_66f3);
/// The hit spheres: radius 0.8 at 0.8 ahead, 0.55 up; the template.
pub const HIT_R: f32 = 0.8;
pub const HIT_AHEAD: f32 = 0.8;
pub const HIT_UP: f32 = 0.55;
pub const HIT_FLAGS: u32 = 0x3_0000;
pub const HIT_DAMAGE: f32 = 3.0;
/// The after-images: (alpha, ticks back) at tick 8, faded by 5 a tick after tick 18.
pub const GHOSTS: [(u8, i32); 3] = [(0x30, 2), (0x17, 4), (0x0c, 6)];
pub const GHOST_FADE: u8 = 5;
/// The arcs.
pub const ARCS: usize = 10;
pub const ARC_POINTS: usize = 5;
/// gp−0x5528 / −0x5524: the glow's fade-in and fade-out (ticks).
pub const FADE_IN: i32 = 5;
pub const FADE_OUT: i32 = 20;
/// The strips (`0x2d1768` → `0x2d1830(gp−0x5540, gp−0x553c, 0.2, …)`) and their colours gp−0x5550 / −0x554c.
pub const CORE_W: f32 = 0.05;
pub const GLOW_W: f32 = 0.5;
pub const GLOW_EXT: f32 = 0.2;
pub const CORE_RGB: u32 = 0x00ff_ffff;
pub const GLOW_RGB: u32 = 0x007f_2020;
/// The fist glow (`0x2d1e08`): FX 8, 0.75 across, 0.5 back along row 1, 0.2 toward the eye; dim / bright colours.
pub const FIST_FX: usize = 8;
pub const FIST_SIZE: f32 = 0.75;
pub const FIST_BACK: f32 = -0.5;
pub const FIST_DIM: u32 = 0x307f_4040;
pub const FIST_BRIGHT: u32 = 0x7f7f_4040;

/// One arc (0x1dc020 + 0x50·i: five points; its timer 0x1dc340 and alpha 0x1dc358 [s16]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Arc {
    pub points: [[f32; 3]; ARC_POINTS],
    pub timer: i16,
    pub alpha: i16,
}

/// One hit sphere of the lunge's physics, delivered by [`deliver_hits`] later in the tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    pub centre: [f32; 3],
    /// The ignored moby: Ratchet's (the second sphere, yaw − 50°) or the item (the others; the port's item is not a table
    /// moby: none).
    pub ignore_hero: bool,
    /// The template's push `(2 cos yaw, 2 sin yaw)`.
    pub push: [f32; 2],
}

/// The Walloper's state outside the item moby (the game's globals 0x1dc020.. and 0x1616fc..0x161704) and the lunge's
/// queued hits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Walloper {
    pub arcs: [Arc; ARCS],
    /// gp−0x5504 / −0x5500: the fade timers; gp−0x54fc: the glow intensity (0..1).
    pub fade_in: i32,
    pub fade_out: i32,
    pub glow: f32,
    /// The tick the item update registered its draw callback, and the fist's frame then (the item's position and row 1).
    pub drawn: Option<u64>,
    pub fist: [[f32; 3]; 2],
    /// This tick's hit spheres (the physics → [`deliver_hits`]).
    pub hits: Vec<Sphere>,
    /// Counters for reports and tests: spheres delivered, mobys listed, sparkle bursts, hit sounds.
    pub stats: [u32; 4],
    /// The mobys the last tick's spheres listed (id, class), in order.
    pub listed: Vec<(MobyId, i16)>,
}

// ------------------------------------------------------------------------------------------------
// The weapon check's case 0x12

/// `HeroPdaGadget` 0x240ed8, case 0x12 (the checks before the switch are the caller's).
pub fn fire(h: &mut Hero, c: &mut Ctx) {
    let g = h.group;
    if (1 < g && (g != 4 || h.jump.landed == 0)) || h.state == 1 { return; }
    let n = ticks(10).min(h.items.slot.ticks_ready);
    if c.env.pad.pressed_within(h.items.slot.fire_mask, n).is_none() { return; }
    if h.items.slot.item.is_none() { return; }
    h.set_state(c, LUNGE, true);
    // `PlayClassSound(0, 0, item)` right after the SetState (the queue plays it after the item's update).
    if h.items.slot.item.is_some() { h.fx.item_sounds.push(SWING_SOUND); }
}

// ------------------------------------------------------------------------------------------------
// The state 0x20

/// SetState's case 0x20 after the group-6 part (`super::melee`): with the item out, the animations (when `play`) and
/// the aim.
pub(super) fn entry(h: &mut Hero, c: &mut Ctx, play: bool) {
    if h.items.slot.item.is_none() { return; }
    if play {
        h.set_anim(c.anim, c.rng, Pf::b(0xbf80_0000), HERO_SEQ, 4);
        // MobyAnimBlend(item, 3, 4, −1) (applied by the items' update of this tick, as the wrench's blends).
        h.items.pending_blend = Some((ITEM_SEQ, 4, -1));
    }
    h.aim_assist(c.env, AIM_RANGE, SIDE, -1.0);
}

/// The physics of 0x20 (0x239d84). Always ported (true).
pub(super) fn physics(h: &mut Hero, env: &Env, anim: &mut dyn AnimCtl) -> bool {
    let t = h.timer;
    if h.melee.aimed == 0 && h.melee.target.is_none() {
        if t < ticks(4) { h.aim_assist(env, AIM_RANGE, SIDE, -1.0); }
    } else {
        h.target_yaw = h.melee.aim_yaw;
        h.turn_to(SCALE64 * Pf::b(0x3d4c_cccd), SCALE64 * Pf::b(0x3e4c_cccd), DT * Pf::b(0x4170_2845));
    }
    let t10 = ticks(10);
    let end = t10 + ticks(8);
    h.target_speed = Pf::ZERO;
    if t10 < t && t < end { h.target_speed = DT * Pf::b(0x4190_0000); }
    if t == ticks(8) { super::packs::thruster_trail_start(h, &GHOSTS); }
    let fade = if end < t { GHOST_FADE } else { 0 };
    super::packs::hero_trail_update(h, &anim.view(), fade);
    if t10 < t { h.speed_step(DT2 * Pf::b(0x433e_0000), DT2 * Pf::b(0x42b4_0000)); }
    let y = if h.melee.aimed != 0 { h.melee.aim_yaw } else { Pf::b(0x47c3_4f80) };
    h.set_planar_vel(y);
    if h.state == LUNGE && t10 < t && t < end + ticks(4) && h.items.slot.item.is_some() {
        let pos = to_f32x3(h.pos);
        let yaw = h.rot[2].to_f32();
        let push = [2.0 * yaw.cos(), 2.0 * yaw.sin()];
        for (a, ignore_hero) in [(yaw, false), (sub_yaw(yaw, SIDE), true), (sub_yaw(yaw, -SIDE), false)] {
            let centre = [pos[0] + HIT_AHEAD * a.cos(), pos[1] + HIT_AHEAD * a.sin(), pos[2] + HIT_UP];
            h.walloper.hits.push(Sphere { centre, ignore_hero, push });
        }
    }
    if t10 < t { h.edge_brake(env, Pf::b(0x406c_cccd), Pf::ZERO); }
    h.wall_check(env, 1);
    if h.air_ticks != 0 {
        h.vel[2] = h.eff[2] - DT2 * Pf::b(0x41c0_0000);
        h.climb_check(env);
    } else {
        h.vel[2] = h.vel[2] - DT2 * Pf::b(0x4258_0000);
    }
    true
}

/// `fast_add_rotations(a, −b)` in the port's radians.
fn sub_yaw(a: f32, b: f32) -> f32 { crate::moby_update::creature::add_rot(a, -b) }

/// The transitions of 0x20 (`0x242930` case 0x20).
pub(super) fn transitions(h: &mut Hero, c: &mut Ctx) {
    let v = c.anim.view();
    if h.items.slot.item.is_none() || v.blending() || v.frame <= 20.0 { return; }
    h.set_state(c, 0, false);
}

/// One moby a sphere listed: its id, class type (header +0x46) and position.
type Listed = (MobyId, Option<u8>, [f32; 3]);

/// The lunge's hit spheres queued by this tick's physics (`coll_sphere_mobys` + `0x236cb8`), in the items' update
/// (before the hand item's own update, as the jump attack's shockwave): the hits, the item's hit sound once per lunge
/// (0x13fdb4), and the sparkle bursts on the listed creatures (class type 5) and type-9 mobys.
pub fn deliver_hits(hero: &mut Hero, table: &mut MobyTable, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng) {
    if hero.walloper.hits.is_empty() { return; }
    let spheres = std::mem::take(&mut hero.walloper.hits);
    let Some(o_class) = hero.items.slot.item.as_ref().map(|it| it.o_class) else { return };
    let hm = env.hero_moby;
    let mut listed: Vec<Vec<Listed>> = Vec::new();
    hero.walloper.listed.clear();
    let tmpl = |s: &Sphere| HitTemplate {
        dir: [Pf::f(s.push[0]), Pf::f(s.push[1]), Pf::f(1.3), Pf::b(0x45af_df66)],
        attacker: Some(hm),
        flags: HIT_FLAGS,
        b18: 0,
        b19: 3,
        h1a: o_class as u16,
        damage: Pf::f(HIT_DAMAGE),
        w20: 1,
    };
    let ran = hits.world(table, hero, rng, env.frame as u64, &mut |w| {
        for s in &spheres {
            let t = tmpl(s);
            let ignore = if s.ignore_hero { Some(hm) } else { None };
            let c = [Pf::f(s.centre[0]), Pf::f(s.centre[1]), Pf::f(s.centre[2]), Pf::ONE];
            let ids = crate::moby_update::services::sphere_mobys_in(w.table, w.svc, w.classes, Pf::f(HIT_R), c, 0, ignore, Some(&t));
            listed.push(
                ids.into_iter()
                    .map(|id| {
                        let m = &w.table.mobys[id];
                        (id, w.classes.info(m.o_class).map(|i| i.ty), [m.position[0], m.position[1], m.position[2]])
                    })
                    .collect(),
            );
        }
    });
    if !ran {
        // A hit sink without the moby world (unit tests): the first listed moby only.
        for s in &spheres {
            let t = tmpl(s);
            let ignore = if s.ignore_hero { Some(hm) } else { None };
            let c = [Pf::f(s.centre[0]), Pf::f(s.centre[1]), Pf::f(s.centre[2]), Pf::ONE];
            let first = hits.sphere(table, Pf::f(HIT_R), c, 0, ignore, &t);
            listed.push(first.map(|id| {
                let m = &table.mobys[id];
                (id, hits.class_type(m.o_class), [m.position[0], m.position[1], m.position[2]])
            }).into_iter().collect());
        }
    }
    for l in listed {
        hero.walloper.stats[0] += 1;
        if l.is_empty() { continue; }
        hero.walloper.stats[1] += l.len() as u32;
        if hero.melee.hit == 0 {
            hero.fx.item_sounds.push(HIT_SOUND);
            hero.walloper.stats[3] += 1;
            hero.melee.hit = 1;
        }
        for (id, ty, p) in l {
            let class = table.mobys.get(id).map_or(-1, |m| m.o_class);
            hero.walloper.listed.push((id, class));
            if ty != Some(5) && ty != Some(9) { continue; }
            // FUN_00248d80(0.5, p, p): the gravity step backwards (0.5 up in gravity mode 0), then 0x2bdb18(p, 4).
            let mut q = [p[0], p[1], p[2] + 0.5];
            super::fx::sparkle_burst(hero, rng, &mut q, 4);
            hero.walloper.stats[2] += 1;
        }
    }
}

// ------------------------------------------------------------------------------------------------
// The item update 0x2d11c0

/// `FastDecTimer__FRi` 0x220e78: 1 when it is 0, else `t = max(t, 1) − 1` and 2 when that is ≤ 0, 0 while it runs.
fn dec(t: &mut i32) -> i32 {
    if *t == 0 { return 1; }
    *t = (*t).max(1) - 1;
    if *t < 1 { 2 } else { 0 }
}

/// `FUN_0026cae0(out, lo, hi)`: two random angles, then a length `randf(lo, hi)`, as `polar(len, a, b)`.
fn rand_vec_ab(rng: &mut Rng, lo: f32, hi: f32) -> [f32; 3] {
    let a = rng.rand_angle();
    let b = rng.rand_angle();
    let l = rng.randf(lo, hi);
    crate::targeting::polar(l, a, b)
}

/// The Walloper's item update `0x2d11c0` (module doc). The write of 0.1 to 0x17b33c while Ratchet's sequence B is not
/// 0x82 has no reader in the level code (the Tesla Claw writes it too): not kept.
pub fn update(hero: &mut Hero, table: &mut MobyTable, _anim: &dyn AnimCtl, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng) {
    let lunging = hero.state == LUNGE;
    let Some(it) = hero.items.slot.item.as_mut() else { return };
    let w = &mut hero.walloper;
    let mut active = false;
    match it.mstate {
        0 => {
            it.mstate = 2;
            for a in &mut w.arcs {
                a.timer = 0;
            }
            w.fade_in = ticks(FADE_IN);
            w.fade_out = ticks(FADE_OUT);
        }
        2 if lunging => {
            let key = env.data.class(it.o_class).map_or(0.0, |c| crate::moby_update::creature::ground::key_time_of(&c.anim, &it.anim));
            if 6.0 <= key {
                active = true;
                if dec(&mut w.fade_in) == 0 {
                    w.glow = 1.0 - w.fade_in as f32 / ticks(FADE_IN) as f32;
                } else if 12.0 <= key {
                    w.glow = if dec(&mut w.fade_out) == 0 { w.fade_out as f32 / ticks(FADE_OUT) as f32 } else { 0.0 };
                } else {
                    w.glow = 1.0;
                }
            }
        }
        2 => {
            w.fade_in = ticks(FADE_IN);
            w.fade_out = ticks(FADE_OUT);
            super::items::blend_item(it, env.data, 1, 0, ticks(12));
        }
        _ => {}
    }
    let row1 = [f32::from_bits(it.rows[1][0]), f32::from_bits(it.rows[1][1]), f32::from_bits(it.rows[1][2])];
    let fist = add3(it.position, scale3(row1, FIST_BACK));
    let eye = env.camera.map_or([0.0; 3], |c| c.0);
    if !active && w.arcs.iter().all(|a| a.timer == 0) { return; }
    for a in w.arcs.iter_mut() {
        a.timer -= 1;
        if a.timer < 0 { a.timer = 0; }
        if a.timer == 0 {
            if !active { continue; }
            a.timer = 2;
            a.alpha = 0x40;
            let mut q = rand_vec_ab(rng, 0.2, 0.2);
            a.points[0] = fist;
            let mut sign = 1.0f32;
            for k in 0..ARC_POINTS - 1 {
                let axis = sub3(a.points[k], eye);
                let ang = rng.randf(f32::from_bits(0x3e32_b8c2), f32::from_bits(0x3f49_0fdb)) * sign;
                sign = -sign;
                q = crate::moby_update::classes::blaster_shot::rotate(q, ang, axis);
                a.points[k + 1] = add3(a.points[k], q);
            }
        } else {
            let mut d = [[0.0f32; 3]; ARC_POINTS - 1];
            for (k, dk) in d.iter_mut().enumerate() {
                *dk = sub3(a.points[k + 1], a.points[k]);
                for x in dk.iter_mut() { *x += rng.randf_sym(0.0, 0.1); }
            }
            a.points[0] = fist;
            for (k, dk) in d.iter().enumerate() { a.points[k + 1] = add3(a.points[k], *dk); }
            a.alpha = if a.timer < 1 { 0 } else { 0x20 };
        }
    }
    // RegisterDrawCallback2(0x2d1768, item): the draw (and its flicker draw at the frame's end) on Ratchet's moby.
    w.drawn = Some(env.frame as u64);
    w.fist = [it.position, row1];
    let hm = env.hero_moby;
    hits.world(table, hero, rng, env.frame as u64, &mut |wo| wo.svc.draw_callbacks.register2(crate::moby_update::classes::draw_callbacks::Callback::Walloper, hm));
}

// ------------------------------------------------------------------------------------------------
// The draw callback 0x2d1768

/// The quads of the draw callback `0x2d1768` for the state the last update left, seen from `eye` (module doc): the
/// live arcs' strips, then the fist glow (`dim`: the draw's `randi(4) ≠ 0`; `gravity` = 0x13f5e0 for its up).
pub fn draw_quads(w: &Walloper, eye: [f32; 3], gravity: [f32; 3], dim: bool) -> Vec<BeamQuad> {
    let mut out = Vec::new();
    for a in &w.arcs {
        if a.timer == 0 { continue; }
        let al = (a.alpha as f32 * w.glow) as i32 as u32 & 0xff;
        super::tesla::strip_colored(&mut out, &a.points, ARC_POINTS, [CORE_W, GLOW_W, GLOW_EXT], [al, al], [CORE_RGB, GLOW_RGB], 0.0, eye);
    }
    let [pos, row1] = w.fist;
    let mut c = add3(pos, scale3(row1, FIST_BACK));
    let col = if dim { FIST_DIM } else { FIST_BRIGHT };
    let al = ((col >> 24) as f32 * w.glow) as i32 as u32 & 0xff;
    let col = (col & 0x00ff_ffff) | al << 24;
    let f = with_len(sub3(eye, c), 1.0);
    let r = with_len(cross3(f, gravity), -1.0);
    let u = cross3(r, f);
    c = add3(c, scale3(f, 0.2));
    // The unit quad 0x1dbfe0: (0, −1, 1), (0, −1, −1), (0, 1, 1), (0, 1, −1), × 0.75 through the rows (f, r, u).
    let q = |y: f32, z: f32| add3(c, add3(scale3(r, y * FIST_SIZE), scale3(u, z * FIST_SIZE)));
    out.push(BeamQuad { fx: FIST_FX, corners: [q(-1.0, 1.0), q(-1.0, -1.0), q(1.0, 1.0), q(1.0, -1.0)], st: [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]], rgba: [col; 4] });
    out
}

/// For the tests: the key time the item update compares (`MobyAnimKeyTime`).
pub fn item_key_time(hero: &Hero, env: &ItemEnv) -> Option<f32> {
    let it = hero.items.slot.item.as_ref()?;
    env.data.class(it.o_class).map(|c| crate::moby_update::creature::ground::key_time_of(&c.anim, &it.anim))
}

/// For the tests: the view fields the transitions read.
pub fn ends(v: &AnimView, item: bool) -> bool { item && !v.blending() && 20.0 < v.frame }

#[cfg(test)]
mod tests {
    use super::*;

    /// `0x22e238` / `0x22dff0`: the lowest `d + yaw_off·d` (+7 for a crate) among the live, targetable targets within
    /// the range and the cone.
    #[test]
    fn melee_aim_search_scores() {
        use super::super::melee::{aim_search, MeleeTarget};
        let t = |id, x: f32, y: f32, health: f32, targetable: bool, is_crate: bool| MeleeTarget { id, pos: [x, y, 0.0], health, targetable, is_crate, look: [0; 3] };
        let list = [
            t(1, 5.0, 0.0, 1.0, true, false),
            t(2, 3.0, 3.0, 1.0, true, false),
            t(3, 12.0, 0.0, 1.0, true, false),
            t(4, 2.0, 0.0, 0.0, true, false),
            t(5, 1.0, 0.0, 1.0, false, false),
            t(6, 4.0, 0.0, 1.0, true, true),
        ];
        let pick = |aim: f32, cone: f32| aim_search(&list, [0.0; 3], aim, AIM_RANGE, cone, -1.0).map(|x| x.id);
        assert_eq!(pick(0.0, SIDE), Some(1), "5 beats 4.24 + 45°·4.24, the crate's 4 + 7, the dead and the untargetable");
        assert_eq!(pick(std::f32::consts::FRAC_PI_2, SIDE), Some(2), "at 90° only the one 45° off is in the 50° cone");
        assert_eq!(pick(std::f32::consts::PI, SIDE), None);
        assert_eq!(pick(std::f32::consts::PI, -1.0), Some(2), "no cone: 4.24 + 135°·4.24 beats 5 + 180°·5");
    }

    #[test]
    fn dec_timer_semantics() {
        let mut t = 2;
        assert_eq!((dec(&mut t), t), (0, 1));
        assert_eq!((dec(&mut t), t), (2, 0));
        assert_eq!((dec(&mut t), t), (1, 0));
    }

    /// The arcs: re-made from the fist (four 0.2 segments) while active, jittered in between, all out 2 ticks after the
    /// lunge's glow stops; the draw: two strips per live arc (3 core + 4 glow quads) and the fist glow.
    #[test]
    fn arc_geometry_and_draw() {
        let mut w = Walloper::default();
        let mut rng = Rng::new();
        let eye = [0.0, -10.0, 1.0];
        for a in w.arcs.iter_mut() {
            a.timer = 2;
            a.alpha = 0x40;
            let mut q = rand_vec_ab(&mut rng, 0.2, 0.2);
            a.points[0] = [0.0, 0.0, 1.0];
            let mut sign = 1.0f32;
            for k in 0..4 {
                let ang = rng.randf(f32::from_bits(0x3e32_b8c2), f32::from_bits(0x3f49_0fdb)) * sign;
                sign = -sign;
                q = crate::moby_update::classes::blaster_shot::rotate(q, ang, sub3(a.points[k], eye));
                a.points[k + 1] = add3(a.points[k], q);
            }
            for k in 0..4 {
                let d = sub3(a.points[k + 1], a.points[k]);
                assert!((super::super::guns::len3(d) - 0.2).abs() < 1e-4, "segment length {d:?}");
            }
        }
        w.glow = 1.0;
        w.fist = [[0.0, 0.0, 1.5], [0.0, 1.0, 0.0]];
        let q = draw_quads(&w, eye, [0.0, 0.0, -1.0], true);
        assert_eq!(q.len(), ARCS * 7 + 1);
        let fist = q.last().unwrap();
        assert_eq!((fist.fx, fist.rgba[0]), (FIST_FX, FIST_DIM));
        // Half the intensity halves every alpha.
        w.glow = 0.5;
        let q = draw_quads(&w, eye, [0.0, 0.0, -1.0], false);
        assert_eq!(q.last().unwrap().rgba[0], 0x3f7f_4040);
        assert!(q.iter().filter(|x| x.fx == super::super::tesla::FX_CORE).all(|x| x.rgba.iter().all(|c| c >> 24 <= 0x20)));
    }
}
