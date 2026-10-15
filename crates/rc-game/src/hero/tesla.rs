//! **The Tesla Claw** (item 19, class 177; level01 item update `0x2ce448`, the chain's reset `0x2cef78` and build
//! `0x2cf138`, its draw callback `0x2d05d8` with the strip `0x2d0748` and the claw glow `0x2d0d18`; read from the
//! decompiler output and the disassembly). No weapon-check case: its update fires. A persistent-arm weapon (def +0x30
//! = 1; standing sequence 51, no moving arm layer).
//!
//! **Pvars** ([`Tesla`]): +0x00 / +0x10 the beam's start and end, +0x20 / +0x38 the two targets (the second only with
//! the gold claw), +0x24 the beam's length, +0x28 / +0x2a how long each target has been held, +0x2c the warm-up,
//! +0x30 the point light, +0x34 its level, +0x3c the ammo timer, +0x3e the chain points drawn, +0x40 / +0x42 "no
//! target" flags, +0x44 / +0x46 the targets' hit cooldowns, +0x48 the hum's sound slot. The chain itself lives in
//! globals in the game (0x1dba40: 20 points, 0x1dbb80: the second chain, 0x1dbcc0 / 0x1dbd20 / 0x1dbe50 their offsets,
//! 0x1dbea0: four five-point arcs; the arcs' timers 0x161688, periods 0x161698 / 0x1616a0, alphas 0x161690; the wave
//! phases gp−0x5624 / −0x561c / −0x5608): here in [`Chain`].
//!
//! **The update**: ○ pressed (not 0x1413fc) sets the length to 0.8. ○ held with the item ready (state 1 / 3, not 4,
//! not 0x1413fc, not in 0x72) after its warm-up (`ticks(16)`), with ammo: one ammo every `ticks(10)` (the targets'
//! cooldowns step with it), the light's level rises to `ticks(10)`, **firing**: a target dropped when it is more
//! than 120° off Ratchet's yaw, not drawn (+0x31), farther than 20, or held longer than `ticks(50)` (`ticks(5)` for a
//! record with +0x04 = 1); the length grows toward 12 (+6 gold) by 7 % a tick; the beam from the claw (the item's joint
//! list 0; in the look stance 1: 1.2 ahead of Ratchet at the item's height, along the view) `length` along the facing
//! (or the view); [`reset`] (once), [`build`] (the chain, its hits, the arcs, the sparks) and the draw callback; the
//! hum (class sound 4, flags 4) kept playing. Not firing: the chain is reset next time, the light's level falls, the
//! hum is released. **States**: 0 → 1 (warm-up `ticks(16)`, the light (radius 0 at the start), 20 points); 1: the
//! warm-up steps; firing → the weapon drawn (`0x22ee08`), 3; ○ pressed without firing → class sound 0, put away;
//! 3: ○ released (or not firing, or 0x1413fc) → the targets cleared, length 0, put away (`0x22efd8`), 1, with the shot statistics
//! (move record 26, the short-shot counter 0x141405) and, after three short shots, the "keep ○ held" help 20016. **The light**: 2 ahead of Ratchet and 2 up
//! (his rows), radius 7.5, colour `(0.7k, 0.7k, k)`, `k = level/ticks(10)`; freed when the item is put away (then
//! state 4).
//!
//! **Native.** Standard `f32`; the chain's globals are this struct's (one Tesla Claw at a time, as the game). The item
//! is not a table moby: the chain's first line ignores Ratchet, the others too [L: the game ignores the item there].
//! Gold (0x13e533) is not mirrored: one target, the blue colour 0x7f2020.

use super::guns::{self, add3, cross3, len3, scale3, sub3, with_len};
use super::items::{HitSink, ItemEnv};
use super::packs::SoundCmd;
use super::physics::{from_f32x3, to_f32x3, ticks};
use super::Hero;
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::blaster_shot::rotate;
use crate::moby_update::creature::{add_rot, atan, diff_rots, sub_rot};
use crate::moby_update::services::HitTemplate;
use crate::point_lights::PointLight;
use crate::ps2v::Pf;
use crate::rng::Rng;
use crate::targeting;

pub const TESLA: i32 = 19;
pub const CLASS: i16 = 177;
/// Chain points (0x1dba40, 20 × 16 bytes).
pub const POINTS: usize = 20;
/// The gp block 0x1615c4..0x16167c.
pub const RANGE: f32 = 15.0;
const HERO_CONE: f32 = f32::from_bits(0x3f49_0fdb);
const CAM_CONE: f32 = 0.558_505_4;
const WAVE_STEP: f32 = 0.017_453_292;
const WAVE_AMP: [f32; 2] = [0.03, 0.03];
const WAVE_RATE: f32 = 0.349_065_84;
const WAVE2_RATE: f32 = 0.802_851_4;
const WAVE2_STEP: f32 = 0.994_837_6;
const WAVE2_AMP: f32 = 0.8;
const WAVE2_SIDE: f32 = 0.5;
const WAVE3_RATE: f32 = 0.052_36;
const WAVE3_STEP: f32 = 0.209_439_52;
const JITTER: f32 = 0.1;
/// 0x3f060a92 / 0x3f860a92: the arcs' and sparks' angles.
const DEG30: f32 = f32::from_bits(0x3f06_0a92);
const DEG60: f32 = f32::from_bits(0x3f86_0a92);
const SCORE_CAM: f32 = 1.5;
/// The strips' colours (gp−0x55dc white, gp−0x55d8 the blue: R 0x20 G 0x20 B 0x7f; gold 0x207f20) and FX textures.
pub const WHITE: u32 = 0x7f_7f7f;
pub const BLUE: u32 = 0x7f_2020;
pub const FX_CORE: usize = 14;
pub const FX_GLOW: usize = 16;
pub const FX_CLAW: usize = 0xb;
/// The core strips' half width (gp−0x55d4): the strip helper `0x2d0748` reads it for every strip, whatever its
/// caller passes.
pub const CORE_W: f32 = 0.05;
/// The hum and the lock / dud sounds.
pub const HUM: i32 = 4;
pub const LOCK_SOUND: i32 = 1;
pub const DUD_SOUND: i32 = 0;

/// The chain's globals (module doc).
#[derive(Clone, Debug, PartialEq)]
pub struct Chain {
    /// gp−0x5640 (a9c0): reset the chain before the next build.
    pub reset: bool,
    pub main: [[f32; 3]; POINTS],
    pub second: [[f32; 3]; POINTS],
    /// 0x1dbcc0 + 4i: the main chain's last z offsets; 0x1dbd20 + 16k: the second's side offsets; 0x1dbe50 + 4i: its
    /// z offsets (indexed as the game: see [`build`]).
    pub zoff: [f32; POINTS + 1],
    pub side: [[f32; 3]; POINTS],
    pub zoff2: [f32; 22],
    /// The wave phases (gp−0x5624, −0x561c, −0x5608).
    pub phase: [f32; 3],
    /// 0x1dbea0: four arcs of five points; 0x161688 timers, 0x161690 alphas.
    pub arcs: [[[f32; 3]; 5]; 4],
    pub arc_timer: [i16; 4],
    pub arc_alpha: [i16; 4],
    /// gp−0x55e4: the core strip's texture scroll (the draw callback steps it).
    pub scroll: f32,
    /// The strips' second colour (gp−0x55d8).
    pub color: u32,
}

impl Default for Chain {
    fn default() -> Self {
        Chain { reset: true, main: [[0.0; 3]; POINTS], second: [[0.0; 3]; POINTS], zoff: [0.0; POINTS + 1], side: [[0.0; 3]; POINTS], zoff2: [0.0; 22], phase: [0.0; 3], arcs: [[[0.0; 3]; 5]; 4], arc_timer: [-1; 4], arc_alpha: [0; 4], scroll: 0.0, color: BLUE }
    }
}

/// The arcs' periods (0x161698) and their alpha peaks (0x1616a0), in ticks.
const ARC_PERIOD: [i32; 4] = [8, 4, 8, 4];
const ARC_PEAK: [i32; 4] = [4, 2, 4, 2];

/// The Tesla Claw's pvars (item moby +0x78) and the chain.
#[derive(Clone, Debug, PartialEq)]
pub struct Tesla {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub targets: [Option<MobyId>; 2],
    pub length: f32,
    pub held: [i16; 2],
    pub warmup: i32,
    pub light: i32,
    pub level: i32,
    pub ammo_timer: i16,
    pub count: i16,
    pub none: [i16; 2],
    pub cooldown: [i16; 2],
    pub chain: Chain,
    /// The tick the draw callback was registered for (`FUN_0021b198(0x2d05d8, item)`: drawn after that tick).
    pub drawn: Option<u64>,
    /// Firing this tick (the port's record for the tests).
    pub firing: bool,
}

impl Default for Tesla {
    fn default() -> Self { Tesla { start: [0.0; 3], end: [0.0; 3], targets: [None; 2], length: 0.0, held: [0; 2], warmup: 0, light: -1, level: 0, ammo_timer: 0, count: 0, none: [0; 2], cooldown: [0; 2], chain: Chain::default(), drawn: None, firing: false } }
}

fn alive(table: &MobyTable, t: MobyId) -> bool { table.mobys.get(t).is_some_and(|m| m.state != 0xfe && m.state != 0xfd && m.mode & crate::moby_runtime::mode::TARGETABLE != 0) }

/// `0x2ce448`, the Tesla Claw's update (from the slot loop with the slot ready).
pub fn update(hero: &mut Hero, table: &mut MobyTable, _anim: &dyn super::anim::AnimCtl, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng) {
    let Some(item) = hero.items.slot.item.as_ref() else { return };
    let (mstate, item_pos) = (item.mstate, item.position);
    let putting_away = hero.items.slot.state == 3;
    let mask = hero.items.slot.fire_mask;
    let pad = env.pad;
    let id = hero.items.slot.id;
    let tick = env.frame as u64;
    if pad.pressed & mask != 0 && hero.items.f13fc == 0 { hero.weapons.tesla.length = 0.8; }
    // The "keep ○ held" help 20016 (record 0x81) after three short shots, while move record 26 is unused.
    {
        let r = &hero.help.records;
        let group_ok = (hero.group as u32) < 2 || hero.group == 9;
        if r.help[0x81].count == 0 && r.moves[26].count == 0 && 2 < hero.help.tesla_taps && group_ok {
            hero.help.out.push(crate::help::HeroHelpOut::Request { msg: 0x4e30, rec: 0x81 });
        }
    }
    let ammo = hero.weapons.has_ammo(id);
    let mut firing = false;
    {
        let hero_pos = to_f32x3(hero.pos);
        let hero_yaw = hero.rot[2].to_f32();
        let t = &mut hero.weapons.tesla;
        let stop = pad.held & mask == 0 || mstate == 4 || mstate == 0 || hero.items.f13fc != 0 || hero.state == 0x72;
        if stop {
            t.targets = [None; 2];
            t.none = [0; 2];
        } else if t.warmup == 0 {
            if ammo < 1 {
                t.targets = [None; 2];
                t.none = [0; 2];
                t.cooldown = [0; 2];
            } else {
                if guns::dec16(&mut t.ammo_timer) {
                    guns::dec16(&mut t.cooldown[0]);
                    guns::dec16(&mut t.cooldown[1]);
                    hero.weapons.use_ammo(id, 1);
                    hero.weapons.tesla.ammo_timer = ticks(10) as i16;
                }
                let t = &mut hero.weapons.tesla;
                if t.light == -1 {
                    t.light = hits.light_alloc(PointLight { color: [0.0; 3], intensity: 0.0, pos: item_pos, radius: 7.5 });
                } else if t.level < ticks(10) {
                    t.level += 1;
                }
                firing = true;
                for k in 0..2 {
                    if let Some(m) = t.targets[k] {
                        let mm = &table.mobys[m];
                        let off = diff_rots(hero_yaw, atan(mm.position[0] - hero_pos[0], mm.position[1] - hero_pos[1]));
                        let far = len3(sub3([mm.position[0], mm.position[1], mm.position[2]], hero_pos)) > 20.0;
                        if 2.0943 < off || mm.visible == 0 || far {
                            t.targets[k] = None;
                            t.none[k] = 0;
                        }
                    }
                }
            }
        }
    }
    if firing {
        let look = hero.state == 1;
        let t = &mut hero.weapons.tesla;
        t.start = add3(item_pos, [0.0, 0.0, 0.7]);
        t.end = t.start;
        for k in 0..2 {
            match t.targets[k] {
                Some(m) if t.none[k] != 0 => {
                    t.held[k] += 1;
                    let lim = if targeting::record(&table.mobys[m]).is_some_and(|r| i16::from_le_bytes([table.mobys[m].pvars[r + 4], table.mobys[m].pvars[r + 5]]) == 1) { 5 } else { 50 };
                    if ticks(lim) as i16 <= t.held[k] {
                        t.targets[k] = None;
                        t.none[k] = 0;
                    }
                }
                _ => t.held[k] = 0,
            }
        }
        let gold = hero.weapons.gold[TESLA as usize] as f32;
        let t = &mut hero.weapons.tesla;
        t.length += ((gold * 6.0 + 12.0) - t.length) * 1.0 * 0.07;
        if look {
            if let Some((_, f)) = env.camera {
                let a = scale3(f, 1.2);
                let hp = to_f32x3(hero.pos);
                t.start = [a[0] + hp[0], a[1] + hp[1], a[2] + item_pos[2]];
                t.end = add3(t.start, scale3(f, t.length));
            }
        } else {
            let muzzle = guns::item_point(hero, env, 0);
            // The arm raised at a wall (0x141618 = 1): the beam along the claw's −row 1 (its +0xd0), not the facing.
            let raised = guns::arm_raised(hero).then(|| guns::item_rows(hero)).flatten();
            let t = &mut hero.weapons.tesla;
            t.start = muzzle;
            t.end = match raised {
                Some(r) => add3(muzzle, scale3(r[1], -t.length)),
                None => add3(muzzle, with_len(to_f32x3(hero.moby_rows[0]), t.length)),
            };
        }
        reset(&mut hero.weapons.tesla);
        build(hero, table, env, hits, rng);
        hero.weapons.tesla.drawn = Some(tick);
        // The draw callback's scroll, stepped once per drawn tick (the game steps it per drawn frame) [L].
        step_scroll(&mut hero.weapons.tesla.chain);
        hero.fx.item_voices.push(SoundCmd::ItemLoop { n: super::fx::LOOP_HUM, index: HUM, flags: 4 });
    } else {
        let t = &mut hero.weapons.tesla;
        t.chain.reset = true;
        if 0 < t.level { t.level -= 1; }
        if hero.fx.item_loops[super::fx::LOOP_HUM].is_some() { hero.fx.item_voices.push(SoundCmd::ItemRelease { n: super::fx::LOOP_HUM }); }
    }
    hero.weapons.tesla.firing = firing;
    {
        let t = &mut hero.weapons.tesla;
        for k in 0..2 {
            if !t.targets[k].is_some_and(|m| alive(table, m)) { t.none[k] = 1; }
        }
    }
    let set_state = |hero: &mut Hero, s: u8| if let Some(it) = hero.items.slot.item.as_mut() { it.mstate = s; };
    match mstate {
        0 => {
            let t = &mut hero.weapons.tesla;
            t.warmup = ticks(16);
            t.light = hits.light_alloc(PointLight { color: [0.0; 3], intensity: 0.0, pos: t.start, radius: 0.0 });
            t.chain.reset = true;
            t.count = 20;
            t.targets = [None; 2];
            t.none = [0; 2];
            t.cooldown = [0; 2];
            set_state(hero, 1);
        }
        1 => {
            guns::dec(&mut hero.weapons.tesla.warmup);
            if firing {
                hero.weapons.pending_draw = true;
                set_state(hero, 3);
            }
            if pad.pressed & mask != 0 && hero.items.f13fc == 0 {
                if !firing {
                    let t = &mut hero.weapons.tesla;
                    t.targets = [None; 2];
                    t.none = [0; 2];
                    set_state(hero, 1);
                    hero.fx.item_sounds.push(DUD_SOUND);
                    super::weapons::put_away(hero);
                } else {
                    hero.weapons.pending_draw = true;
                    set_state(hero, 3);
                }
            }
        }
        3 if !(pad.released & mask == 0 && firing && hero.items.f13fc == 0) => {
            // The shot's statistics (move record 26: counted for a shot longer than ticks(70), its time and mask on
            // every shot) and the short-shot counter 0x141405 (below ticks(30), else reset).
            let out = if ticks(0x46) < hero.f50c { crate::help::HeroHelpOut::BumpMove(26) } else { crate::help::HeroHelpOut::TouchMove(26) };
            hero.help.out.push(out);
            hero.help.tesla_taps = if hero.f50c < ticks(0x1e) { hero.help.tesla_taps.wrapping_add(1) } else { 0 };
            let t = &mut hero.weapons.tesla;
            t.targets = [None; 2];
            t.none = [0; 2];
            t.length = 0.0;
            super::weapons::put_away(hero);
            set_state(hero, 1);
        }
        _ => {}
    }
    // The light.
    let t = &mut hero.weapons.tesla;
    if t.light != -1 {
        let r = hero.moby_rows.map(to_f32x3);
        let p = to_f32x3(hero.pos);
        let pos = add3(p, add3(scale3(r[0], 2.0), scale3(r[2], 2.0)));
        let k = t.level as f32 / ticks(10) as f32;
        hits.light_set(t.light, PointLight { color: [k * 0.7, k * 0.7, k], intensity: 0.0, pos, radius: 7.5 });
        if putting_away {
            hits.light_free(t.light);
            t.light = -1;
        }
    }
    if putting_away && hero.weapons.tesla.light == -1 { set_state(hero, 4); }
}

/// `0x2cef78`: once after a pause in the firing, the chain laid straight from the start toward the end (20 points
/// `range/20` apart), the offsets zeroed, the arcs' timers reset, the phases 0.
pub fn reset(t: &mut Tesla) {
    if !t.chain.reset { return; }
    let c = &mut t.chain;
    c.reset = false;
    let step = with_len(sub3(t.end, t.start), RANGE / 20.0);
    c.main[0] = t.start;
    for i in 0..POINTS - 1 { c.main[i + 1] = add3(c.main[i], step); }
    c.zoff = [0.0; POINTS + 1];
    c.side = [[0.0; 3]; POINTS];
    c.zoff2 = [0.0; 22];
    c.arc_timer = [-1; 4];
    c.phase = [0.0; 3];
}

/// `0x2cf138`: the targets, the chain, its hits, the arcs and the sparks (module doc of [`Chain`] for the layout).
#[allow(clippy::too_many_lines)]
pub fn build(hero: &mut Hero, table: &mut MobyTable, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng) {
    let gold = hero.weapons.gold[TESLA as usize];
    let mut seg = RANGE / 20.0;
    let hero_pos = to_f32x3(hero.pos);
    let hero_yaw = hero.rot[2].to_f32();
    let (cam, cam_fwd) = env.camera.unwrap_or((hero_pos, [1.0, 0.0, 0.0]));
    let cam_yaw = atan(cam_fwd[0], cam_fwd[1]);
    {
        let p = &mut hero.weapons.tesla.chain.phase;
        p[0] = add_rot(p[0], WAVE_RATE);
        p[1] = sub_rot(p[1], WAVE2_RATE);
        p[2] = add_rot(p[2], WAVE3_RATE);
    }
    // The targets.
    let (mut best, mut second): ((Option<MobyId>, f32), (Option<MobyId>, f32)) = ((None, 0.0), (None, 0.0));
    let t = &mut hero.weapons.tesla;
    let search = t.targets[0].is_none() || (gold != 0 && t.targets[1].is_none());
    if search {
        if t.targets[1].is_some() && t.targets[0].is_none() {
            t.targets[0] = t.targets[1];
            t.none[0] = t.none[1];
            t.targets[1] = None;
            t.none[1] = 0;
        } else {
            for &id in env.targets {
                if Some(id) == t.targets[0] || Some(id) == t.targets[1] { continue; }
                let Some(m) = table.mobys.get(id) else { continue };
                if !m.has_class || hits.class_type(m.o_class) != Some(5) || m.state == 0xfe || m.state == 0xfd || targeting::record(m).is_none() { continue; }
                let rel = sub3([m.position[0], m.position[1], m.position[2]], t.start);
                let d = (rel[0] * rel[0] + rel[1] * rel[1]).sqrt();
                if RANGE < d { continue; }
                let cd = diff_rots(cam_yaw, atan(m.position[0] - cam[0], m.position[1] - cam[1]));
                if CAM_CONE < cd { continue; }
                let hd = diff_rots(atan(m.position[0] - hero_pos[0], m.position[1] - hero_pos[1]), hero_yaw);
                if HERO_CONE < hd { continue; }
                let score = ((CAM_CONE - cd) / CAM_CONE) * SCORE_CAM + (RANGE - d) / RANGE;
                if best.1 < score {
                    second = best;
                    best = (Some(id), score);
                } else if second.1 < score {
                    second = (Some(id), score);
                }
            }
        }
    }
    let t = &mut hero.weapons.tesla;
    let mut other = second.0;
    if let Some(b) = best.0 {
        other = Some(b);
        if t.targets[0].is_none() {
            t.targets[0] = Some(b);
            t.none[0] = 0;
            hero.fx.item_sounds.push(LOCK_SOUND);
            other = second.0;
        }
    }
    let t = &mut hero.weapons.tesla;
    if let (Some(o), true, Some(b)) = (other, gold != 0, best.0) {
        let bp = table.mobys[b].position;
        let d1 = len3(sub3(t.start, [bp[0], bp[1], bp[2]]));
        if d1 < RANGE {
            let op = table.mobys[o].position;
            if d1 + len3(sub3([bp[0], bp[1], bp[2]], [op[0], op[1], op[2]])) < RANGE {
                t.targets[1] = Some(o);
                t.none[1] = 0;
            }
        }
    }
    let aim = |table: &MobyTable, m: MobyId| targeting::aim_point(&table.mobys[m]);
    let aims = [t.targets[0].map(|m| aim(table, m)), t.targets[1].map(|m| aim(table, m))];
    let mut d0 = 0.0;
    if let Some(a) = aims[0] {
        d0 = len3(sub3(t.start, a));
        seg = d0 * 0.05;
    }
    if let (Some(a), Some(b)) = (aims[0], aims[1]) { seg = (len3(sub3(a, b)) + d0) * 0.05; }
    t.chain.color = if gold != 0 { 0x20_7f20 } else { BLUE };
    // The claw's own hit: a sphere of 0.3 at the start.
    let tmpl = HitTemplate { dir: [Pf::ZERO; 4], attacker: Some(env.hero_moby), flags: 0x21_0000, b18: 5, b19: 1, h1a: CLASS as u16, damage: Pf::f(gold as f32 + 2.0), w20: 1 };
    t.count = 20;
    hits.sphere(table, Pf::f(0.3), from_f32x3(t.start), 0x10, Some(env.hero_moby), &tmpl);
    let t = &mut hero.weapons.tesla;
    let mut dirn = with_len(sub3(t.end, t.start), 1.0);
    if t.targets[0].is_some() || t.targets[1].is_some() {
        let side = cross3(dirn, to_f32x3(hero.gravity_dir));
        dirn = rotate(dirn, 0.174_532_92, side);
    }
    let mut step = scale3(dirn, seg);
    t.chain.main[0] = t.start;
    t.chain.second[0] = t.start;
    let mut blocked = false;
    let mut passed: Vec<MobyId> = Vec::new();
    let mut i = 1usize;
    while i < POINTS {
        let fi = i as f32;
        let amp = WAVE_AMP[0] + (WAVE_AMP[1] - WAVE_AMP[0]) * fi * 0.05;
        let wave = amp * crate::moby_update::creature::add_rot(t.chain.phase[0] + fi * WAVE_STEP, 0.0).sin() + rng.randf_sym(0.0, JITTER);
        let old = t.chain.zoff[i];
        t.chain.zoff[i] = wave;
        t.chain.main[i][2] -= old;
        let rel = sub3(t.chain.main[i], t.chain.main[i - 1]);
        let blend = add3(rel, scale3(sub3(step, rel), 0.85));
        let l = len3(blend);
        if l == 0.0 {
            t.chain.main[i] = t.chain.main[i - 1];
        } else {
            let s = scale3(blend, seg / l);
            t.chain.main[i] = add3(t.chain.main[i - 1], s);
            step = s;
        }
        t.chain.main[i][2] += wave;
        let rel2 = sub3(t.chain.main[i], t.chain.main[i - 1]);
        if let Some(a) = aims[0] {
            if gold == 0 || aims[1].is_none() {
                let to = sub3(a, t.chain.main[i]);
                let l = len3(to);
                if l != 0.0 {
                    let k = if i == 19 { seg = l; 0.5 } else if i < 16 { 0.1 } else { seg = l / (19 - i) as f32; 0.5 };
                    let to = scale3(to, seg / l);
                    step = add3(step, scale3(sub3(to, step), k));
                }
            }
        }
        if i < 10 {
            let p2 = crate::moby_update::creature::add_rot(t.chain.phase[1] + fi * WAVE2_STEP, 0.0);
            let p3 = crate::moby_update::creature::add_rot(t.chain.phase[2] + fi * WAVE3_STEP, 0.0);
            let s3 = WAVE2_AMP * p3.sin();
            t.chain.second[i] = t.chain.main[i];
            let z2 = s3 * p2.sin() + rng.randf_sym(0.0, JITTER);
            t.chain.zoff2[i] = z2;
            t.chain.second[i][2] += z2;
            let ang = add_rot(atan(rel2[0], rel2[1]), std::f32::consts::FRAC_PI_2);
            let r = WAVE2_SIDE * p2.cos();
            t.chain.side[i - 1] = [ang.cos() * r, ang.sin() * r, 0.0];
            t.chain.second[i] = add3(t.chain.second[i], t.chain.side[i - 1]);
        } else {
            t.chain.second[i] = t.chain.main[i];
            t.chain.second[i][2] += t.chain.zoff2[21 - i];
            t.chain.second[i] = add3(t.chain.second[i], t.chain.side[20 - i]);
        }
        // The hit tests: the body to the start, then every other point (and the last) from two points back.
        let test = if i == 1 {
            let up = to_f32x3(hero.moby_rows[2]);
            Some((add3(hero_pos, scale3(up, 0.5)), t.start))
        } else if i == 19 || i & 1 == 0 {
            Some((t.chain.main[i - 2], t.chain.main[i]))
        } else {
            None
        };
        if let Some((a, b)) = test {
            let hit = hits.probe_moby(table, from_f32x3(a), from_f32x3(b), 0x10, Some(env.hero_moby)).flatten().or_else(|| {
                env.coll.and_then(|c| super::physics::line_world(c, from_f32x3(a), from_f32x3(b), 0x10)).map(|o| super::items::Probe { moby: None, point: o.point, normal: o.normal, surface: o.surface_id() })
            });
            let d = sub3(t.chain.main[i], cam);
            let n = len3([d[0], d[1], 0.0]);
            let dir = if n == 0.0 { [0.0, 0.0, 1.0] } else { [d[0] / n, d[1] / n, 0.0] };
            let tm = HitTemplate { dir: [Pf::f(dir[0]), Pf::f(dir[1]), Pf::f(dir[2]), Pf::b(0x45af_df66)], attacker: Some(env.hero_moby), flags: 0x21_0000, b18: 5, b19: 1, h1a: CLASS as u16, damage: Pf::f(gold as f32 + 2.0), w20: 1 };
            if let Some(h) = hit {
                let mut stop = true;
                if let Some(m) = h.moby {
                    let mm = &table.mobys[m];
                    if targeting::record(mm).is_some() && mm.has_class && hits.class_type(mm.o_class) == Some(5) {
                        // A creature: the chain passes through it (its collision off until the chain is built).
                        stop = false;
                        passed.push(m);
                        table.mobys[m].has_collision = false;
                    } else {
                        hits.deliver(table, m, &tm);
                    }
                }
                if stop {
                    let t = &mut hero.weapons.tesla;
                    t.chain.main[i] = h.point;
                    t.count = i as i16;
                    t.end = h.point;
                    // The world, or a moby with collision, drops the target (`0x26e7b0`); a moby without keeps it.
                    let drop = match h.moby { None => true, Some(m) => table.mobys[m].has_collision };
                    if drop {
                        t.targets[0] = None;
                        t.none[0] = 0;
                    }
                    blocked = true;
                    break;
                }
            }
        }
        i += 1;
    }
    for m in passed { table.mobys[m].has_collision = true; }
    let t = &mut hero.weapons.tesla;
    if !blocked { t.end = t.chain.main[POINTS - 1]; }
    // The targets' hits (their cooldowns).
    for k in 0..2 {
        let Some(m) = t.targets[k] else { continue };
        if t.none[k] != 0 || t.cooldown[k] != 0 { continue; }
        let p = table.mobys[m].position;
        let d = sub3([p[0], p[1], p[2]], cam);
        let n = len3([d[0], d[1], 0.0]);
        let dir = if n == 0.0 { [0.0, 0.0, 1.0] } else { [d[0] / n, d[1] / n, 0.0] };
        let tm = HitTemplate { dir: [Pf::f(dir[0]), Pf::f(dir[1]), Pf::f(dir[2]), Pf::b(0x45af_df66)], attacker: Some(env.hero_moby), flags: 0x21_0000, b18: 5, b19: 3, h1a: CLASS as u16, damage: Pf::f(gold as f32 + 2.0), w20: 1 };
        hits.deliver(table, m, &tm);
        let quick = targeting::record(&table.mobys[m]).is_some_and(|r| i16::from_le_bytes([table.mobys[m].pvars[r + 4], table.mobys[m].pvars[r + 5]]) == 1);
        t.cooldown[k] = ticks(if quick { 1 } else { 5 }) as i16;
    }
    // The arcs at the claw (0, 1) and the end (2, 3).
    let rows = hero.moby_rows.map(to_f32x3);
    for k in 0..4 {
        let (base, len) = if k < 2 { (t.start, 0.4) } else { (t.end, 0.5) };
        if !guns::dec16(&mut t.chain.arc_timer[k]) {
            let mut d = [[0.0f32; 3]; 4];
            for (j, dj) in d.iter_mut().enumerate() {
                let r = sub3(t.chain.arcs[k][j + 1], t.chain.arcs[k][j]);
                *dj = [r[0] + rng.randf_sym(0.0, JITTER), r[1] + rng.randf_sym(0.0, JITTER), r[2] + rng.randf_sym(0.0, JITTER)];
            }
            t.chain.arcs[k][0] = base;
            for (j, dj) in d.iter().enumerate() { t.chain.arcs[k][j + 1] = add3(t.chain.arcs[k][j], *dj); }
            let tv = t.chain.arc_timer[k] as i32;
            let dd = (tv - ARC_PEAK[k]).abs() as f32;
            t.chain.arc_alpha[k] = ((1.0 - dd / ticks(15) as f32) * 32.0) as i32 as i16;
        } else {
            t.chain.arc_timer[k] = ticks(ARC_PERIOD[k]) as i16;
            t.chain.arc_alpha[k] = 0;
            let a = rng.randf_sym(DEG30, DEG60);
            let b = rng.randf(-DEG60, 0.174_532_92);
            let mut v = scale3(dirn, len);
            v = rotate(v, b, rows[1]);
            v = rotate(v, a, rows[2]);
            t.chain.arcs[k][0] = base;
            t.chain.arcs[k][1] = add3(base, v);
            let mut sign = 1.0f32;
            for j in 0..3 {
                let c = sub3(t.chain.arcs[k][j + 1], cam);
                let ang = rng.randf(0.174_532_92, 0.959_931_1) * sign;
                sign = -sign;
                v = rotate(v, ang, c);
                t.chain.arcs[k][j + 2] = add3(t.chain.arcs[k][j + 1], v);
            }
        }
    }
    // The sparks (type 53) at the claw and the end.
    let a = rng.randf_sym(DEG30, DEG60);
    let b = rng.randf(-DEG30, DEG60);
    let mut v = scale3(dirn, 0.09 * 1.0);
    v = rotate(v, b, rows[1]);
    v = rotate(v, a, rows[2]);
    let mut spin = rng.rand_range(1, 8);
    if rng.randi(2) != 0 { spin = -spin; }
    let f = rng.randf(0.2, 0.6);
    let col = t.chain.color | 0x7f00_0000;
    let (start, end) = (t.start, t.end);
    let life = ticks(15);
    hero.fx.parts.push(super::fx::PartSpawn::Sparkle { s12: f * 0.3, s13: f, s14: 0.0, pos: [start[0], start[1], start[2], 0.0], life, rgba: col, b8: 0, t0: spin as i8, vel: v });
    hero.fx.parts.push(super::fx::PartSpawn::Sparkle { s12: f * 0.15, s13: f * 0.5, s14: 0.0, pos: [start[0], start[1], start[2], 0.0], life, rgba: 0x307f_7f7f, b8: 0, t0: -spin as i8, vel: v });
    let mut spin = rng.rand_range(1, 8);
    if rng.randi(2) != 0 { spin = -spin; }
    let f = rng.randf(0.15, 3.0);
    hero.fx.parts.push(super::fx::PartSpawn::Sparkle { s12: f * 0.1, s13: f, s14: 0.0, pos: [end[0], end[1], end[2], 0.0], life: ticks(12), rgba: col, b8: 0, t0: spin as i8, vel: [0.0; 3] });
}

/// The item is gone with the Tesla Claw's light or hum still out.
pub fn item_gone(hero: &mut Hero, hits: &mut dyn HitSink) {
    let t = &mut hero.weapons.tesla;
    if t.light != -1 {
        hits.light_free(t.light);
        t.light = -1;
    }
    t.drawn = None;
    t.firing = false;
    if hero.fx.item_loops[super::fx::LOOP_HUM].is_some() { hero.fx.item_voices.push(SoundCmd::ItemRelease { n: super::fx::LOOP_HUM }); }
}

// ------------------------------------------------------------------------------------------------------------------
// The draw callback 0x2d05d8

/// One quad of the beam: FX texture, corners (GS strip order), texture coordinates, vertex colours (R low, 0x80 = 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamQuad {
    pub fx: usize,
    pub corners: [[f32; 3]; 4],
    pub st: [[f32; 2]; 4],
    pub rgba: [u32; 4],
}

/// `0x2d0748(w1, w2, ext, points, a1, a2, n)`: the two strips along the first `n` points: the core (FX 14, white at
/// alpha `a1`, `w1` across the view, the texture scrolled by `scroll`, faded in on the first quad and out on the last)
/// and the glow (FX 16, `color` at alpha `a2`: each segment put at the same distance from the eye, lengthened by `ext`
/// at both ends, `w2` across, faded at both ends). Additive (ALPHA 0x48).
#[allow(clippy::too_many_arguments)]
pub fn strip(out: &mut Vec<BeamQuad>, pts: &[[f32; 3]], n: usize, w1: f32, w2: f32, ext: f32, a1: u32, a2: u32, color: u32, scroll: f32, eye: [f32; 3]) {
    strip_colored(out, pts, n, [w1, w2, ext], [a1, a2], [WHITE, color], scroll, eye)
}

/// [`strip`] with the core's colour too (`core`, alpha 0 in the low 24 bits): the same draw code the Walloper's arcs use
/// (`0x2d1830`, its core 0xffffff and glow 0x7f2020 from gp−0x5550 / −0x554c; `super::walloper`). `w` = (core width,
/// glow width, glow lengthening), `a` = (core alpha, glow alpha), `rgb` = (core colour, glow colour).
#[allow(clippy::too_many_arguments)]
pub fn strip_colored(out: &mut Vec<BeamQuad>, pts: &[[f32; 3]], n: usize, w: [f32; 3], a: [u32; 2], rgb: [u32; 2], scroll: f32, eye: [f32; 3]) {
    let ([w1, w2, ext], [a1, a2], [core, color]) = (w, a, rgb);
    let n = n.min(pts.len());
    if n < 2 { return; }
    let full1 = core | a1 << 24;
    let side = |a: [f32; 3], b: [f32; 3], w: f32| {
        let c = cross3(sub3(b, a), sub3(a, eye));
        let l = len3(c);
        scale3(c, if l != 0.0 { w / l } else { 0.0 })
    };
    let s0 = side(pts[0], pts[1], w1);
    let (mut pa, mut pb) = (add3(pts[0], s0), sub3(pts[0], s0));
    let st = [[scroll, 0.0], [scroll, 1.0], [scroll + 1.0, 0.0], [scroll + 1.0, 1.0]];
    let mut c = [full1; 4];
    for i in 1..n - 1 {
        let s = side(pts[i], pts[i + 1], w1);
        let (qa, qb) = (add3(pts[i], s), sub3(pts[i], s));
        if i == 1 { c[0] = core; c[1] = core; } else if i == n - 2 { c[2] = core; c[3] = core; } else if i == 2 { c[0] = full1; c[1] = full1; }
        out.push(BeamQuad { fx: FX_CORE, corners: [pa, pb, qa, qb], st, rgba: c });
        pa = qa;
        pb = qb;
    }
    let full2 = color | a2 << 24;
    let mut c = [full2; 4];
    let st2 = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];
    for i in 0..n - 1 {
        let (mut a, mut b) = (sub3(pts[i], eye), sub3(pts[i + 1], eye));
        let (la, lb) = (len3(a), len3(b));
        if lb < la { if lb != 0.0 { b = scale3(b, la / lb); } } else if la != 0.0 { a = scale3(a, lb / la); }
        let (mut pa, mut pb) = (add3(eye, a), add3(eye, b));
        let d = sub3(pb, pa);
        let e = scale3(d, ext);
        pa = sub3(pa, e);
        pb = add3(pb, e);
        let cr = cross3(sub3(eye, pa), d);
        let l = len3(cr);
        let s = scale3(cr, if l != 0.0 { w2 / l } else { 0.0 });
        if i == 0 { c[0] = color; c[1] = color; } else if i == n - 2 { c[2] = color; c[3] = color; } else if i == 1 { c[0] = full2; c[1] = full2; }
        out.push(BeamQuad { fx: FX_GLOW, corners: [add3(pa, s), sub3(pa, s), add3(pb, s), sub3(pb, s)], st: st2, rgba: c });
    }
}

/// The draw callback `0x2d05d8` for the chain as the last update left it, seen from `eye`: the scroll step (−0.3 a
/// tick, wrapped at −8), the main chain (0x80 / 0x40, widths 0.05 / 0.4, lengthened 0.9), the second (0x10 / 0x20,
/// 0.05 / 0.3, 0.1), the four arcs (their alphas, 0.05 / 0.3, 0.2; the core's width is the helper's [`CORE_W`]) and the claw's glow (`0x2d0d18`: FX 0xb, two quads facing
/// the eye 0.3 toward it from the start, 0.1 and 0.35 across (each + `randf_sym(0, 0.1)` / `(0, 0.05)` in the game:
/// the draw's own jitter is not reproduced [L]), 0x307f7f7f and the strips' colour).
pub fn beam_quads(t: &Tesla, eye: [f32; 3], gravity: [f32; 3]) -> Vec<BeamQuad> {
    let c = &t.chain;
    let mut out = Vec::new();
    let n = (t.count.max(0) as usize).min(POINTS);
    strip(&mut out, &c.main, n, CORE_W, 0.4, 0.9, 0x80, 0x40, c.color, c.scroll, eye);
    strip(&mut out, &c.second, n, CORE_W, 0.3, 0.1, 0x10, 0x20, c.color, c.scroll, eye);
    for k in 0..4 {
        let a = c.arc_alpha[k].max(0) as u32;
        strip(&mut out, &c.arcs[k], 5, CORE_W, 0.3, 0.2, a, a, c.color, c.scroll, eye);
    }
    // The claw's glow: a quad facing the eye.
    let toward = add3(t.start, with_len(sub3(eye, t.start), 0.3));
    let f = with_len(sub3(eye, toward), 1.0);
    let r = with_len(cross3(f, gravity), -1.0);
    let u = cross3(r, f);
    for (size, col) in [(0.1f32, 0x307f_7f7f), (0.35, c.color | 0x7f00_0000)] {
        let q = |x: f32, y: f32| add3(toward, add3(scale3(r, x * size), scale3(u, y * size)));
        out.push(BeamQuad { fx: FX_CLAW, corners: [q(-1.0, 1.0), q(1.0, 1.0), q(-1.0, -1.0), q(1.0, -1.0)], st: [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]], rgba: [col; 4] });
    }
    out
}

/// The draw callback's scroll step (once per drawn tick).
pub fn step_scroll(c: &mut Chain) {
    c.scroll -= 0.3;
    if c.scroll <= -8.0 { c.scroll += 8.0; }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The core strip: one quad per inner point, the first faded in, the last out; the glow strip one per segment,
    /// all across the view (their corners' spread perpendicular to the eye's line and the segment).
    #[test]
    fn strips_face_the_eye() {
        let pts: Vec<[f32; 3]> = (0..6).map(|i| [i as f32, 0.0, 0.0]).collect();
        let mut out = Vec::new();
        strip(&mut out, &pts, 6, 0.05, 0.4, 0.9, 0x80, 0x40, BLUE, 0.0, [2.5, -10.0, 0.0]);
        let core: Vec<_> = out.iter().filter(|q| q.fx == FX_CORE).collect();
        let glow: Vec<_> = out.iter().filter(|q| q.fx == FX_GLOW).collect();
        assert_eq!((core.len(), glow.len()), (4, 5));
        assert_eq!(core[0].rgba[0] >> 24, 0);
        assert_eq!(core[1].rgba[0] >> 24, 0x80);
        assert_eq!(core[3].rgba[3] >> 24, 0);
        // Seen along −y, the strips spread in z.
        for q in &out { assert!((q.corners[0][2] - q.corners[1][2]).abs() > 0.05, "{q:?}"); }
    }

    #[test]
    fn reset_lays_the_chain_straight() {
        let mut t = Tesla { start: [0.0, 0.0, 1.0], end: [5.0, 0.0, 1.0], ..Default::default() };
        reset(&mut t);
        assert!(!t.chain.reset);
        assert_eq!(t.chain.main[0], [0.0, 0.0, 1.0]);
        assert!((t.chain.main[19][0] - 19.0 * 0.75).abs() < 1e-4);
    }
}
