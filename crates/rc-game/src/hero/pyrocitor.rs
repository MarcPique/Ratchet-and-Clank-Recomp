//! **The Pyrocitor** (item 16, class 176; level01 update `0x2cd458` with `0x2cde98` / `0x2ce0a0`, read from the
//! disassembly), the flamethrower sold by the Novalis vendor: its row in `super::gadgets::HAND_ITEMS`. It has no case
//! in the weapon check `HeroPdaGadget` 0x240ed8: the item's own update fires it.
//!
//! **Pvars** ([`Pyro`]): +0x00 the pilot-flame moby (class 179, [`crate::moby_update::classes::pyro_glow`]), +0x04..+0x3c
//! the flames it hits with (15), +0x44 the point light, +0x4a the loop sound's slot, +0x4c the fire timer, +0x4e the
//! smoke timer, +0x50 the hold counter.
//!
//! **The update**, every tick in the slot loop (slot ready):
//! 1. `0x2cde98`: the item put away (key B on sequence 2) → state 4, the pilot flame deleted, the light freed, the loop
//!    released. Otherwise its hold statistics and the "hold ○" help after three short taps ([`hold_stats`]).
//! 2. The nozzle `P` = the item's joint list 0 point (`FUN_002645a8`).
//! 3. By the item's state (+0x20): **0** → 1; **1** (drawn): key A on sequence 1 → 2; on sequence 0 past key time 10 with
//!    ○ held → blend to sequence 1 over 5 ticks; **2** (ready): the pilot flame is created (`CreateMoby(0xb3)`,
//!    `0x2d0fc8`), a light left from firing fades (its colour ×0.9 a tick, freed below 0.001); ○ held (not 0x1413fc) with
//!    ammo → **3**, the weapon drawn (`0x22ee08`: the standing stance, [`super::weapons::draw_weapon`]), the fire timer
//!    `ticks(10)`, a point light (radius 7.5, `WritePointLight_A`); ○ pressed without ammo → the empty click (class sound
//!    0); **3** (firing): the loop (class sound 1, flags 4) while it is not alive; the light 2 ahead and 2 above Ratchet
//!    (his rows), its colour 10 % a tick toward a flicker between (1, 0.45, 0, 0.25) and (1, 0.55, 0, 0.25) on a 120-tick
//!    sine; **the flame**: from `P` along Ratchet's facing (row 0 of his moby rows; in first person, look stance 1 with
//!    the item hidden: from 1 below the eye along the view), range 12 (+4 gold), cut 0.75 short of a wall (the line from
//!    the body point to `P`, then `P` to the end, `CollLine_Fix` flags 2), the launch speed
//!    `min(sqrt(2·48·dt²·len), 16·dt)` (`0x270830`); 2 + 2 type-12 particles a tick (1 + 2 when the flame is shorter
//!    than 4 or in first person), the first ones glow puffs, each at a random point of the first tick's segment with a
//!    random spread `rand_vec(0, (3 − 2·len/range)·dt)` ([`crate::particles::type12`]); every 4th tick the third is
//!    kept for the hits; two hit spheres of 0.42 at `P − 0.3·fwd` and `P − 0.8·fwd` (template: damage 1 + gold, flags
//!    0x10000, type 5); the smoke: every `trunc(randf(10, 30))` ticks one type-25 spark along the flame; one ammo every
//!    10 ticks; when the fire timer has run out and ○ is released (or no ammo / 0x1413fc): state 2, the weapon put away
//!    (`0x22efd8`), the loop released, the empty click without ammo.
//! 4. The pilot flame follows the item: its rows = its own spin (Euler +0x40) through the item's rows, at `P`.
//! 5. `0x2ce0a0`: each kept flame alive hits a sphere of half its size at its position (template: damage 1 + gold,
//!    flags 0x10000, type 5 / 1, class 176); at life 10 it throws two type-2 embers; below life 2 it is dropped.
//!
//! **Native.** Standard `f32`; the kept flames are [`Flame`] values stepped by the same [`Flame::step`] as their
//! particle records (the port creates the records after the hero update, `super::fx`), so the hits follow the flames
//! drawn. The item is not a moby of the table in the port: the hits' attacker is Ratchet's moby and the pilot flame
//! reads its owner's state from its pvars (written here every tick). RNG draws are the game's, in its order.

use super::fx::PartSpawn;
use super::items::{HitSink, ItemEnv};
use super::packs::SoundCmd;
use super::physics::{from_f32x3, to_f32x3, ticks};
use super::Hero;
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::services::{pvar, HitTemplate};
use crate::particles::type12::{self, Flame};
use crate::point_lights::PointLight;
use crate::ps2v::Pf;
use crate::rng::Rng;
use rc_formats::moby_anim;

/// The Pyrocitor (item 16).
pub const PYROCITOR: i32 = 16;
/// Its class.
pub const CLASS: i16 = 176;
/// The pilot flame's class (`CreateMoby(0xb3)`).
pub const GLOW_CLASS: i16 = 179;
/// The loop's class sound; the empty click.
pub const LOOP_SOUND: i32 = 1;
pub const EMPTY_SOUND: i32 = 0;
/// `0x161550`: the range; `0x161544`: the top speed (u/s); `0x161548` / `0x16154c`: glow puffs / flames a tick;
/// `0x161554` / `0x161558`: the spread at full / no length (×dt); `0x161560`: ticks per ammo.
pub const RANGE: f32 = 12.0;
pub const TOP_SPEED: f32 = 16.0;
pub const GLOWS: i32 = 2;
pub const FLAMES: i32 = 2;
pub const SPREAD: [f32; 2] = [1.0, 3.0];
pub const AMMO_TICKS: i32 = 10;
/// `0x161520` / `0x161530`: the light's flicker colours (r, g, b, intensity).
pub const LIGHT_A: [f32; 4] = [1.0, 0.45, 0.0, 0.25];
pub const LIGHT_B: [f32; 4] = [1.0, 0.55, 0.0, 0.25];
/// The hit template's push word (+0x0c: 5627.97) and type byte (+0x18).
const PUSH: u32 = 0x45af_df66;
const HIT_TYPE: u8 = 5;
const DT: f32 = 1.0 / 60.0;

/// The pilot flame's pvars as the port uses them: +0x04 the texture scroll, +0x08 the owner's state byte, +0x09 the
/// owner hidden (mode bit 0), +0x0a the owner's slot ticks ready (s16), +0x0c the scale factor 0.5. The game keeps
/// the owner's pointer at +0x08 and reads its fields; the port writes them there every tick.
pub mod glow_pv {
    pub const SCROLL: usize = 0x04;
    pub const OWNER_STATE: usize = 0x08;
    pub const OWNER_HIDDEN: usize = 0x09;
    pub const READY: usize = 0x0a;
    pub const SCALE: usize = 0x0c;
}

/// The Pyrocitor's pvars (item moby +0x78).
#[derive(Clone, Debug, PartialEq)]
pub struct Pyro {
    /// +0x00: the pilot flame.
    pub glow: Option<MobyId>,
    /// +0x04..: the kept flames, each with "created this tick" (not yet stepped by `UpdateParts`).
    pub hits: [Option<(Flame, bool)>; 15],
    /// +0x44: the point light (−1: none). (+0x4a, the loop's sound slot, is `super::fx::HeroFx::item_loops[0]`.)
    pub light: i32,
    /// +0x4c / +0x4e: the fire and smoke timers.
    pub fire_timer: i16,
    pub smoke_timer: i16,
    /// +0x50: ticks ○ has been held in state 3 (the hold statistics, [`hold_stats`]).
    pub hold: i16,
}

impl Default for Pyro {
    fn default() -> Self { Pyro { glow: None, hits: [None; 15], light: -1, fire_timer: 0, smoke_timer: 0, hold: 0 } }
}

/// `0x2cde98`'s other half (the item not being put away; from the disassembly): while the "hold ○" help (record 0x4e)
/// and move record 10 are unused, the ticks of each fire (state 3) are counted in +0x50; when a fire ends, one longer
/// than `ScaleTicks(120)` bumps move record 10 (the player learnt it), one shorter than `ScaleTicks(30)` counts a tap
/// (0x141404); after three taps the help 20004 "To use the Pyrocitor, hold down ○" is requested (record 0x4e) on each
/// fire's end.
fn hold_stats(hero: &mut Hero) {
    let r = &hero.help.records;
    if r.help[0x4e].count != 0 || r.moves[10].count != 0 { return; }
    let firing = hero.items.slot.item.as_ref().is_some_and(|m| m.mstate == 3);
    let p = &mut hero.weapons.pyro;
    if firing {
        p.hold = p.hold.wrapping_add(1);
        return;
    }
    let held = p.hold as i32;
    if held == 0 { return; }
    if crate::hud::scale_ticks(0x78) < held {
        hero.help.out.push(crate::help::HeroHelpOut::BumpMove(10));
    } else if held < crate::hud::scale_ticks(0x1e) {
        hero.help.pyro_taps = hero.help.pyro_taps.wrapping_add(1);
    }
    hero.weapons.pyro.hold = 0;
    if 2 < hero.help.pyro_taps { hero.help.out.push(crate::help::HeroHelpOut::Request { msg: 0x4e24, rec: 0x4e }); }
}

/// `FastDecTimer` (s16): non-zero when the timer was 0 or reaches 0 now.
fn dec(t: &mut i16) -> bool { super::idle::dec_timer_s16(t) != 0 }

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn scale3(a: [f32; 3], k: f32) -> [f32; 3] { a.map(|x| x * k) }
fn len3(a: [f32; 3]) -> f32 { (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt() }
/// `FastVecNormalize(out, v, len)`: `v` scaled to length `len` (0 stays 0).
fn with_len(a: [f32; 3], l: f32) -> [f32; 3] {
    let n = len3(a);
    if n == 0.0 { [0.0; 3] } else { scale3(a, l / n) }
}

/// `0x270830(len, 1, 48·dt², 16·dt, &0, &0)` from rest: the flame's launch speed (u/tick) for `len`.
pub fn launch_speed(len: f32) -> f32 {
    if len <= 0.0 { return 0.0; }
    let b = type12::DECEL * DT * DT;
    let v = (2.0 * b * len).sqrt().min(TOP_SPEED * DT);
    // The approach step 1.0 reaches it at once; a flame shorter than one step lands on its end.
    v.min(len)
}

/// The template of the Pyrocitor's hits (`0x26e808` and the spheres' own): push `(dir.xy, 1, 5627.97)`, flags
/// 0x10000, damage 1 + gold, type 5.
fn template(env: &ItemEnv, dir: [f32; 3], gold: u8, kept: bool) -> HitTemplate {
    let d = with_len(dir, 1.0);
    HitTemplate {
        dir: [Pf::f(d[0]), Pf::f(d[1]), Pf::ONE, Pf::b(PUSH)],
        attacker: Some(env.hero_moby),
        flags: 0x1_0000,
        b18: HIT_TYPE,
        b19: kept as u8,
        h1a: if kept { CLASS as u16 } else { 0 },
        damage: Pf::f(1.0 + gold as f32),
        w20: 1,
    }
}

/// `0x2cd458`, the Pyrocitor's update (from the slot loop with the slot ready).
pub fn update(hero: &mut Hero, table: &mut MobyTable, _anim: &dyn super::anim::AnimCtl, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng) {
    let Some(class) = hero.items.slot.item.as_ref().and_then(|m| env.data.class(m.o_class)).cloned() else { return };
    let gold = hero.weapons.gold[PYROCITOR as usize];
    // The kept flames' records moved in last tick's UpdateParts.
    for e in hero.weapons.pyro.hits.iter_mut() {
        if let Some((f, fresh)) = e {
            if *fresh { *fresh = false } else if !f.step(gold) { *e = None; }
        }
    }
    // 0x2cde98: put away (key B on sequence 2).
    if hero.items.slot.item.as_ref().is_some_and(|it| it.anim.seq_b == 2) {
        if let Some(it) = hero.items.slot.item.as_mut() { it.mstate = 4; }
        shut_down(hero, table, env, hits);
    } else {
        hold_stats(hero);
    }
    // The nozzle: joint list 0 of the item.
    let nozzle = super::guns::item_point(hero, env, 0);
    let pad = env.pad;
    let mask = hero.items.slot.fire_mask;
    let held = pad.held & mask != 0;
    let pressed = pad.pressed & mask != 0;
    let id = hero.items.slot.id;
    let mstate = hero.items.slot.item.as_ref().map_or(0, |m| m.mstate);
    match mstate {
        0 => {
            hero.weapons.pyro.light = -1;
            hero.fx.item_loops[0] = None;
            if let Some(it) = hero.items.slot.item.as_mut() { it.mstate = 1; }
        }
        1 => {
            let it = hero.items.slot.item.as_mut().unwrap();
            if it.anim.seq_a == 1 { it.mstate = 2; }
            if it.anim.seq_a == 0 && 10.0 < key_time(&class.anim, &it.anim) && held && it.anim.seq_b != 1 {
                moby_anim::set_sequence(&mut it.anim, &class.anim, 1, 0, ticks(5), &mut it.snapshot);
            }
        }
        2 => ready(hero, table, env, hits, nozzle, held, pressed, id, gold),
        3 => firing(hero, table, env, hits, rng, nozzle, held, id, gold),
        _ => {}
    }
    // The pilot flame follows the item.
    if let (Some(g), Some(it)) = (hero.weapons.pyro.glow, hero.items.slot.item.as_ref()) {
        if let Some(m) = table.mobys.get_mut(g) {
            let e = [m.rotation[0], m.rotation[1], m.rotation[2]];
            let local = euler_rows(e);
            let item: [[f32; 3]; 3] = [0, 1, 2].map(|i| [0, 1, 2].map(|k| f32::from_bits(it.rows[i][k])));
            for (i, l) in local.iter().enumerate() {
                let r: [f32; 3] = std::array::from_fn(|k| l[0] * item[0][k] + l[1] * item[1][k] + l[2] * item[2][k]);
                m.rows[i] = [r[0], r[1], r[2], 0.0];
            }
            m.position = [nozzle[0], nozzle[1], nozzle[2], m.position[3]];
            // Its owner's fields, read by its update in the next moby loop.
            if m.pvars.len() < 0x10 { m.pvars.resize(0x10, 0); }
            m.pvars[glow_pv::OWNER_STATE] = it.mstate;
            m.pvars[glow_pv::OWNER_HIDDEN] = (hero.f13f5 != 0) as u8;
            let r = hero.items.slot.ticks_ready.clamp(0, i16::MAX as i32) as i16;
            m.pvars[glow_pv::READY..glow_pv::READY + 2].copy_from_slice(&r.to_le_bytes());
        }
    }
    kept_hits(hero, table, env, hits, rng, gold);
}

/// `MobyAnimKeyTime` 0x263920 for the item.
fn key_time(class: &moby_anim::MobyAnimClass, s: &moby_anim::AnimState) -> f32 {
    let time = |seq: u8, f: u8| class.frame(seq, f).map(|fr| fr.header.time as i32 as f32).unwrap_or(0.0);
    let ta = if s.seq_a == moby_anim::SNAPSHOT_SEQ { time(s.seq_b, s.frame_b) } else { time(s.seq_a, s.frame_a) };
    if s.t == 0.0 { return ta * 0.0625; }
    if s.seq_a == s.seq_b && s.frame_a <= s.frame_b { return (ta * (1.0 - s.t) + time(s.seq_b, s.frame_b) * s.t) * 0.0625; }
    ta * 0.0625 + s.t
}

/// `fun_001fa030`: rows from Euler angles (x, then y, then z), standard `f32`.
fn euler_rows(e: [f32; 3]) -> [[f32; 3]; 3] {
    let r = super::physics::euler_rows([Pf::f(e[0]), Pf::f(e[1]), Pf::f(e[2]), Pf::ZERO]);
    [0, 1, 2].map(|i| to_f32x3(r[i]))
}

/// State 2.
#[allow(clippy::too_many_arguments)]
fn ready(hero: &mut Hero, table: &mut MobyTable, env: &ItemEnv, hits: &mut dyn HitSink, nozzle: [f32; 3], held: bool, pressed: bool, id: i32, _gold: u8) {
    if hero.weapons.pyro.glow.is_none() {
        // 0x2d0fc8: CreateMoby(0xb3) with the item's rows, drawn with alpha 0x50, rows kept (mode 0x204 | 0x100).
        if let Some(g) = hits.create_moby(table, GLOW_CLASS, env.frame as u64) {
            let rows = hero.items.slot.item.as_ref().map(|it| it.rows);
            let m = &mut table.mobys[g];
            let keep = [0, 1, 2].map(|i| [m.rows[i][0], m.rows[i][1], m.rows[i][2]]);
            let r = rows.map_or(keep, |r| r.map(|src| [f32::from_bits(src[0]), f32::from_bits(src[1]), f32::from_bits(src[2])]));
            crate::moby_update::classes::pyro_glow::init(m, r, Some(nozzle));
            pvar::set_ff(&mut m.pvars, glow_pv::SCALE, 0.5);
            hero.weapons.pyro.glow = Some(g);
        }
    }
    let p = &mut hero.weapons.pyro;
    if p.light != -1 {
        if let Some(mut l) = hits.light_get(p.light) {
            let c = [l.color[0], l.color[1], l.color[2], l.intensity].map(|x| x + (0.0 - x) * 0.1);
            (l.color, l.intensity) = ([c[0], c[1], c[2]], c[3]);
            hits.light_set(p.light, l);
            if len3([c[0], c[1], c[2]]) < 0.001 {
                hits.light_free(p.light);
                p.light = -1;
            }
        } else {
            p.light = -1;
        }
    }
    let ammo = hero.weapons.has_ammo(id) != 0;
    if held && hero.items.f13fc == 0 && ammo {
        if let Some(it) = hero.items.slot.item.as_mut() { it.mstate = 3; }
        hero.weapons.pending_draw = true;
        hero.weapons.pyro.fire_timer = ticks(10) as i16;
        if hero.weapons.pyro.light == -1 {
            let pos = hero.items.slot.item.as_ref().map_or(nozzle, |it| it.position);
            hero.weapons.pyro.light = hits.light_alloc(PointLight { color: [0.0; 3], intensity: 0.0, pos, radius: 7.5 });
        }
    } else if pressed && !ammo {
        hero.fx.item_sounds.push(EMPTY_SOUND);
    }
}

/// State 3.
#[allow(clippy::too_many_arguments)]
fn firing(hero: &mut Hero, table: &mut MobyTable, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng, nozzle: [f32; 3], held: bool, id: i32, gold: u8) {
    hero.fx.item_voices.push(SoundCmd::ItemLoop { n: 0, index: LOOP_SOUND, flags: 4 });
    let rows: [[f32; 3]; 3] = [0, 1, 2].map(|i| to_f32x3(hero.moby_rows[i]));
    let hpos = to_f32x3(hero.pos);
    // The light: 2 ahead, 2 up, flickering.
    let pl = hero.weapons.pyro.light;
    if pl != -1 {
        if let Some(mut l) = hits.light_get(pl) {
            l.pos = add3(hpos, add3(scale3(rows[0], 2.0), scale3(rows[2], 2.0)));
            l.radius = 7.5;
            // The game's own literals (6.28318, 3.14159), not τ and π.
            #[allow(clippy::approx_constant)]
            let ph = (env.frame % 120) as f32 / 120.0 * 6.28318 - 3.14159;
            let t = ph.sin() * 0.5 + 0.5;
            let target: [f32; 4] = std::array::from_fn(|k| LIGHT_A[k] + (LIGHT_B[k] - LIGHT_A[k]) * t);
            let cur = [l.color[0], l.color[1], l.color[2], l.intensity];
            let c: [f32; 4] = std::array::from_fn(|k| cur[k] + (target[k] - cur[k]) * 0.1);
            (l.color, l.intensity) = ([c[0], c[1], c[2]], c[3]);
            hits.light_set(pl, l);
        }
    }
    // The flame's start, direction and length.
    let bonus = gold as f32 * 4.0;
    let range = RANGE + bonus;
    let first_person = hero.state == 1 && hero.f13f5 != 0;
    let (start, dir) = match (first_person, env.camera, env.camera_up) {
        (true, Some((eye, fwd)), Some(up)) => (sub3(eye, with_len(up, 1.0)), with_len(fwd, range + 0.75)),
        _ => (nozzle, with_len(rows[0], range + 0.75)),
    };
    let end = add3(start, dir);
    let mut probe = |a: [f32; 3], b: [f32; 3]| -> Option<[f32; 3]> {
        match hits.probe(table, from_f32x3(a), from_f32x3(b), 2, None) {
            Some(r) => r,
            None => env.coll.and_then(|c| super::physics::line_world(c, from_f32x3(a), from_f32x3(b), 2)).map(|o| o.point),
        }
    };
    let len = if probe(to_f32x3(hero.body_point), start).is_some() {
        0.0
    } else if let Some(hp) = probe(start, end) {
        (len3(sub3(start, hp)) - 0.75).max(0.0)
    } else {
        range
    };
    let v = launch_speed(len);
    let step = with_len(dir, v);
    let glows = if len < 4.0 || first_person { 1 } else { GLOWS };
    let spread = ((SPREAD[0] - SPREAD[1]) * (len / range) + SPREAD[1]) * DT;
    let keep = env.frame & 3 == 0;
    for i in 0..glows + FLAMES {
        let r = rng.rand_vec(0.0, spread);
        let vel = add3(r, step);
        let k = rng.randf(0.0, 1.0);
        let pos = add3(scale3(step, k), start);
        let flags = if i < glows { 1 | 4 } else { 4 };
        let draws = type12::Draws::draw(rng, flags);
        let (pos4, vel4) = ([pos[0], pos[1], pos[2], 0.0], [vel[0], vel[1], vel[2], 0.0]);
        hero.fx.parts.push(PartSpawn::Flame { len, pos: pos4, vel: vel4, flags, draws });
        if i == 2 && keep {
            let mut life = draws.life as i32 as i16;
            if flags & 4 != 0 { life = ((life as i32 * (gold as i32 + 2)) / 2) as i16; }
            let size = if flags & 1 != 0 { 1.0 } else { 0.25 } * 210000.0;
            let f = Flame { pos, vel, len, life, size, flags, life0: life as u8 };
            if let Some(e) = hero.weapons.pyro.hits.iter_mut().find(|e| e.is_none()) { *e = Some((f, true)); }
        }
    }
    // The two hit spheres at the nozzle.
    let tmpl = template(env, step, gold, false);
    for back in [-0.3f32, -0.8] {
        let c = add3(scale3(rows[0], back), nozzle);
        hits.sphere(table, Pf::b(0x3ed7_0a3d), from_f32x3(c), 5, Some(env.hero_moby), &tmpl);
    }
    // The smoke: a type-25 spark along the flame now and then.
    if dec(&mut hero.weapons.pyro.smoke_timer) {
        hero.weapons.pyro.smoke_timer = rng.randf(10.0, 30.0) as i32 as i16;
        let r = rng.rand_vec(0.0, DT + DT);
        let k = rng.randf(0.5, 0.1);
        let vel = add3(scale3(step, k), r);
        super::fx::spark(hero, rng, [start[0], start[1], start[2], 0.0], [vel[0], vel[1], vel[2], 0.0], gold != 0);
    }
    if env.frame % ticks(AMMO_TICKS) == 0 { hero.weapons.use_ammo(id, 1); }
    let ammo = hero.weapons.has_ammo(id) != 0;
    if dec(&mut hero.weapons.pyro.fire_timer) {
        if held && hero.items.f13fc == 0 && ammo { return; }
        if let Some(it) = hero.items.slot.item.as_mut() { it.mstate = 2; }
        super::weapons::put_away(hero);
        hero.fx.item_voices.push(SoundCmd::ItemRelease { n: 0 });
        if !ammo { hero.fx.item_sounds.push(EMPTY_SOUND); }
    }
}

/// `0x2ce0a0`: the kept flames' hits and embers.
fn kept_hits(hero: &mut Hero, table: &mut MobyTable, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng, gold: u8) {
    for k in 0..hero.weapons.pyro.hits.len() {
        let Some((f, _)) = hero.weapons.pyro.hits[k] else { continue };
        let r = f.size / 210000.0 * 0.5;
        let tmpl = template(env, f.vel, gold, true);
        hits.sphere(table, Pf::f(r), from_f32x3(f.pos), 0, Some(env.hero_moby), &tmpl);
        if f.life == 10 {
            for _ in 0..2 { embers(hero, rng, &f); }
        } else if f.life < 2 {
            hero.weapons.pyro.hits[k] = None;
        }
    }
}

/// One of `0x2ce0a0`'s embers: `PartType02Spawn` (def 12, additive) at the flame, moving on with it and settling.
fn embers(hero: &mut Hero, rng: &mut Rng, f: &Flame) {
    let r = f.size / 210000.0 * 0.5;
    let mut v2 = rng.rand_vec(0.0, DT);
    v2[2] = DT * 4.0;
    let c1 = crate::hud::tween_color(rng.randf(0.0, 1.0), 0x3000_2040, 0x3000_4040);
    let c2 = crate::hud::tween_color(rng.randf(0.0, 1.0), 0x1000_2040, 0x1000_4040);
    let b = rng.randf(10.0, 20.0) as i32;
    let c = rng.randf(10.0, 20.0) as i32;
    let p = add3(rng.rand_vec(0.0, r + r), f.pos);
    let spawn = crate::particles::type02::Spawn {
        pos: [p[0], p[1], p[2], 0.0],
        v1: [f.vel[0], f.vel[1], f.vel[2], 0.2],
        v2: [v2[0], v2[1], v2[2], 0.1],
        c1,
        c2,
        t: [10, b, c],
        def: 0x1_000c,
    };
    // PartType02Spawn's own rotation draw, made here (the record is created before UpdateParts).
    let rot = *rng;
    let _ = rng.randf(0.0, 255.0);
    hero.fx.parts.push(PartSpawn::Blob { spawn, rng: rot });
}

/// The Pyrocitor's shutdown (`0x2cde98` on the put-away, or the item gone): the pilot flame deleted, the light freed,
/// the loop released; the kept flames dropped.
pub fn shut_down(hero: &mut Hero, table: &mut MobyTable, env: &ItemEnv, hits: &mut dyn HitSink) {
    let p = &mut hero.weapons.pyro;
    if let Some(g) = p.glow.take() {
        if table.mobys.get(g).is_some_and(|m| !m.is_deleted() && m.o_class == GLOW_CLASS) { hits.delete_moby(table, g, env.frame as u64); }
    }
    if p.light != -1 {
        hits.light_free(p.light);
        p.light = -1;
    }
    if hero.fx.item_loops[0].is_some() { hero.fx.item_voices.push(SoundCmd::ItemRelease { n: 0 }); }
    p.hits = [None; 15];
}

/// The item is gone (swapped away, the slot emptied) with the Pyrocitor's resources still out.
pub fn item_gone(hero: &mut Hero, table: &mut MobyTable, env: &ItemEnv, hits: &mut dyn HitSink) {
    let p = &hero.weapons.pyro;
    if p.glow.is_some() || p.light != -1 || hero.fx.item_loops[0].is_some() { shut_down(hero, table, env, hits); }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `0x270830` from rest: `sqrt(2·48·dt²·len)` up to 16 u/s.
    #[test]
    fn launch_speed_rule() {
        assert!((launch_speed(12.0) - 16.0 * DT).abs() < 1e-7);
        let v = launch_speed(1.0);
        assert!((v - (2.0f32 * 48.0 * DT * DT).sqrt()).abs() < 1e-7);
        assert_eq!(launch_speed(0.0), 0.0);
    }
}
