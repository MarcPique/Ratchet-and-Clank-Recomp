//! The port's hero, headless, driven by a recorded trace's pad bytes (`rc-trace replay-hero`). Doc:
//! docs/plan/hero_feel_pass.md.
//!
//! The run is the hero-only harness of rc-game's pack tests (`tests/hero/hero_packs_novalis.rs`) on the recorded level:
//! the level's collision, Ratchet (his class and his own sequence table), the back items (packs 607 / 608 / 609 and
//! Clank 601 when the level has them), the follow camera, one `rc_game::tick::Game` ticked with the recorded pad
//! bytes through `Game::tick` (the port's normal input path: `PadState::update` → the hero → the camera). **No
//! mobys** (no moving platforms, crates or enemies) and no hand items: record the feel pass on open ground.
//!
//! **Initial state** from the start sample (a standing, grounded one): position, yaw, target yaw, velocity,
//! momentum, health, the item-owned table, the back item; the pad is primed with the start sample's bytes (so a
//! button already held is not a new press); the tick counter = the start tick. The camera is set up behind the hero
//! at the recorded camera's yaw (`Camera::new` on a copy of the hero turned to it), not at the hero's yaw, and the
//! recorded camera record (position, Euler, rows) is published for the first tick: the stick is read relative to
//! the camera, so its yaw decides where Ratchet runs. The follow camera's springs start at rest (not recorded).

use crate::hero_trace::{Hex, Sample, Trace};
use anyhow::{bail, Context, Result};
use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gameplay, level, wad};
use rc_game::follow_camera::{CamInput, Camera};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::physics;
use rc_game::hero::{Hero, HeroTick};
use rc_game::moby_runtime::{ClassInfo, Moby, MobyTable};
use rc_game::ps2v::Pf;
use rc_game::tick::{Game, GameOptions, TickHooks};
use std::path::Path;

/// A level's data for the hero-only run.
pub struct HeroLevel {
    pub n: u32,
    pub mesh: collision::Collision,
    pub ratchet: MobyAnimClass,
    pub death_z: f32,
    /// Ratchet's instance on the level (position, yaw).
    pub spawn: ([f32; 3], f32),
    /// (back item id, o_class, class) of the packs the level has, and Clank.
    pub packs: Vec<(i32, i16, MobyAnimClass)>,
    pub clank: Option<MobyAnimClass>,
}

impl HeroLevel {
    pub fn load(extracted: &Path, n: u32) -> Result<HeroLevel> {
        let dir = extracted.join(format!("levels/{n:02}"));
        let rd = |f: &str| std::fs::read(dir.join(f)).with_context(|| format!("reading {}/{f}", dir.display()));
        let data = wad::decompress(&rd("core_data.bin")?)?;
        let idx = rd("core_index.bin")?;
        let gp = wad::decompress(&rd("gameplay_ntsc.bin")?)?;
        let settings = gameplay::section(&gp, "level_settings")?.context("no level settings section")?;
        let core = level::parse_level_core(&idx, data.len())?;
        let mesh = collision::parse_collision(&core, &data)?;
        let instances = gameplay::parse_moby_instances(&gp)?;
        let r = instances.iter().find(|m| m.o_class == 0).context("no Ratchet instance on the level")?;
        let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
        let blob = core.block(&data, "moby_class/0000").context("Ratchet's class")?;
        let class = rc_formats::moby::parse_moby_class(blob)?;
        let seqs: Vec<Option<MobySequence>> = (0..256)
            .map(|i| core.block(&data, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(b, 0).ok()))
            .collect();
        let class_of = |o: u32| -> Option<MobyAnimClass> {
            let b = core.block(&data, &format!("moby_class/{o:04}"))?;
            let c = rc_formats::moby::parse_moby_class(b).ok()?;
            Some(MobyAnimClass::new(&c, parse_sequences(b, &c).ok()?))
        };
        let packs = [(2, 607u32), (3, 608), (4, 609)].into_iter().filter_map(|(i, o)| Some((i, o as i16, class_of(o)?))).collect();
        Ok(HeroLevel { n, mesh, ratchet: MobyAnimClass::new(&class, seqs), death_z, spawn: (r.position, r.rotation[2]), packs, clank: class_of(601) })
    }
}

/// Options of a replay.
#[derive(Clone, Debug, Default)]
pub struct ReplayOptions {
    /// Index of the start sample (None: the first standing, grounded, gameplay-mode sample).
    pub start: Option<usize>,
    /// Stop after this many ticks (None: the whole trace).
    pub ticks: Option<u32>,
    /// Set the camera up behind the hero at the hero's yaw instead of the recorded camera's.
    pub camera_at_hero: bool,
}

/// The replay's result: the port's samples (one per recorded sample from the start on) and notes.
pub struct Replay {
    pub start: usize,
    pub samples: Vec<Sample>,
    /// Ticks the recording missed that the replay ran with the next recorded pad bytes.
    pub filled: u32,
    pub notes: Vec<String>,
}

/// The first standing, grounded, gameplay sample.
pub fn auto_start(s: &[Sample]) -> Option<usize> { s.iter().position(|x| x.state == 0 && x.air_ticks == 0 && x.mode == 0 && x.group == 0) }

fn pad_of(s: &Sample) -> Result<[u8; 18]> {
    s.pad.0.as_slice().try_into().with_context(|| format!("tick {}: pad column has {} bytes, not 18", s.tick, s.pad.0.len()))
}

/// The port's hero and camera as a [`Sample`] (the fields the port has; `jump_raw` and `lib_pad` empty).
pub fn port_sample(g: &Game, anim: &RatchetAnim, pad: &[u8; 18], mirror: bool, level: i32) -> Sample {
    let h: &Hero = &g.hero;
    let f = |p: Pf| p.to_f32();
    let j = &h.jump;
    let c = &g.camera.out;
    let p = &g.pad;
    Sample {
        t_us: 0,
        tick: g.counter as u32,
        mode: 0,
        level,
        mirror: mirror as u8,
        pad: Hex(pad.to_vec()),
        lib_pad: Hex(Vec::new()),
        pad_held: p.held,
        pad_pressed: p.pressed,
        pad_held_u: p.held_unmirrored,
        state: h.state,
        substate: h.substate,
        group: h.group,
        prev_state: h.prev_state,
        st_timer: h.timer,
        pos_x: f(h.pos[0]),
        pos_y: f(h.pos[1]),
        pos_z: f(h.pos[2]),
        yaw: f(h.rot[2]),
        vel_x: f(h.vel[0]),
        vel_y: f(h.vel[1]),
        vel_z: f(h.vel[2]),
        disp_x: f(h.disp[0]),
        disp_y: f(h.disp[1]),
        disp_z: f(h.disp[2]),
        mom_x: f(h.momentum[0]),
        mom_y: f(h.momentum[1]),
        mom_z: f(h.momentum[2]),
        eff_len_xy: f(h.eff_len_xy),
        fwd_speed: f(h.fwd_speed),
        target_yaw: f(h.target_yaw),
        target_speed: f(h.target_speed),
        speed: f(h.speed),
        ground_z: f(h.ground_z),
        height: f(h.height),
        air_ticks: h.air_ticks,
        t_508: h.f508,
        t_510: h.f510,
        t_lockout: h.lockout,
        t_524: h.f524,
        t_542: h.jump_lockout,
        t_70c: h.land_run_blend,
        j_f744: f(j.f744),
        j_g_down: f(j.g_down),
        j_takeoff_x: f(j.takeoff_pos[0]),
        j_takeoff_y: f(j.takeoff_pos[1]),
        j_takeoff_z: f(j.takeoff_pos[2]),
        j_ref_z: f(j.ref_z),
        j_landed: j.landed,
        j_curve_on: j.curve_on,
        j_descending: j.descending,
        j_takeoff: j.takeoff,
        j_air_speed: f(j.air_speed),
        j_pending: f(j.pending),
        j_applied: f(j.applied),
        j_h: f(j.h),
        j_land_eta: j.land_eta,
        j_acc: f(j.acc),
        j_dec: f(j.dec),
        j_hmin: f(j.hmin),
        j_hmax: f(j.hmax),
        j_ramp: j.ramp,
        j_max_air: j.max_air,
        j_g: f(j.g),
        jump_raw: Hex(Vec::new()),
        anim_seq: anim.state.seq_b,
        anim_frame: anim.state.frame_b,
        anim_t: anim.state.t,
        anim_key: f(anim.frame),
        anim_speed: f(h.anim_speed),
        stick_x: f(h.stick[0]),
        stick_y: f(h.stick[1]),
        stick_mag: f(h.stick_mag),
        health: h.health,
        owned: Hex(h.owned.0.to_vec()),
        back_state: h.back_slot.slot.state,
        back_id: h.back_slot.slot.id,
        cam_x: f(c.pos[0]),
        cam_y: f(c.pos[1]),
        cam_z: f(c.pos[2]),
        cam_roll: f(c.euler[0]),
        cam_pitch: f(c.euler[1]),
        cam_yaw: f(c.euler[2]),
        cam_fx: f(c.rows[0][0]),
        cam_fy: f(c.rows[0][1]),
        cam_fz: f(c.rows[0][2]),
        cam_lx: f(c.rows[1][0]),
        cam_ly: f(c.rows[1][1]),
        cam_lz: f(c.rows[1][2]),
        cam_ux: f(c.rows[2][0]),
        cam_uy: f(c.rows[2][1]),
        cam_uz: f(c.rows[2][2]),
    }
}

/// A game on `lv` placed as sample `s0` (see the module doc).
pub fn place(lv: &HeroLevel, s0: &Sample, camera_at_hero: bool) -> Result<(Game, RatchetAnim, Vec<String>)> {
    let mut notes = Vec::new();
    let class = ClassInfo { scale: 1.0, ..Default::default() };
    let mut m = Moby::init_instance(0, 0, Some(&class));
    m.position = [s0.pos_x, s0.pos_y, s0.pos_z, 1.0];
    m.rotation = [0.0, 0.0, s0.yaw, 0.0];
    let mirror = s0.mirror != 0;
    let mut g = Game::new(&lv.mesh, MobyTable::new(vec![m], 16), 0, GameOptions { mirror, ..Default::default() }, lv.death_z);
    g.finish_load();
    let snapped = g.hero.position();
    if (snapped[2] - s0.pos_z).abs() > 0.05 {
        notes.push(format!("the hero init's ground snap moved z {} -> {} (start not standing on the level's collision?)", s0.pos_z, snapped[2]));
    }
    let h = &mut g.hero;
    h.idle.level = lv.n as i32;
    h.pos = physics::from_f32x3([s0.pos_x, s0.pos_y, s0.pos_z]);
    h.rot[2] = Pf::f(s0.yaw);
    h.rows = physics::euler_rows(h.rot);
    h.moby_rows = h.rows;
    h.moby_rot = h.rot;
    h.target_yaw = Pf::f(s0.target_yaw);
    h.vel = physics::from_f32x3([s0.vel_x, s0.vel_y, s0.vel_z]);
    h.momentum = physics::from_f32x3([s0.mom_x, s0.mom_y, s0.mom_z]);
    h.health = s0.health;
    if s0.owned.0.len() == h.owned.0.len() {
        h.owned.0.copy_from_slice(&s0.owned.0);
    } else {
        notes.push("no item-owned table in the start sample: the Heli-Pack (2) and the Thruster-Pack (3) granted".into());
        h.grant_items(&[2, 3]);
    }
    if s0.back_state == 2 && (2..=4).contains(&s0.back_id) {
        h.equip_back(s0.back_id);
    } else {
        notes.push(format!("back slot state {} id {}: no pack equipped", s0.back_state, s0.back_id));
    }
    if let Some(clank) = lv.clank.clone() { h.set_back_packs(lv.packs.clone(), clank); } else { notes.push("no Clank class on the level: back items not modelled".into()); }
    {
        let m = &mut g.mobys.mobys[0];
        m.position = [s0.pos_x, s0.pos_y, s0.pos_z, 1.0];
        m.rotation = [0.0, 0.0, s0.yaw, 0.0];
    }
    g.counter = s0.tick as u64;
    g.hero.idle.counter = s0.tick as i32;
    // The pad as the start sample's tick left it (its held mask is the next tick's "previous").
    g.pad.update(Some(&pad_of(s0)?), mirror);
    // The camera behind the hero at the recorded camera's yaw, and the recorded camera record published (the
    // hero's first tick reads its yaw and rows; `Camera::new` publishes only the position).
    let recorded = !camera_at_hero && (s0.cam_fx != 0.0 || s0.cam_fy != 0.0);
    let mut look = g.hero.clone();
    if recorded {
        look.rot[2] = Pf::f(s0.cam_yaw);
        look.rows = physics::euler_rows(look.rot);
        look.moby_rows = look.rows;
        look.moby_rot = look.rot;
        look.target_yaw = look.rot[2];
    }
    g.camera = Camera::new(&CamInput { hero: &look, pad: &g.pad, coll: &lv.mesh, mobys: None, hero_moby: None }, g.options.camera);
    if recorded {
        let v = |x: f32, y: f32, z: f32| physics::from_f32x3([x, y, z]);
        let out = &mut g.camera.out;
        out.pos = v(s0.cam_x, s0.cam_y, s0.cam_z);
        out.euler = v(s0.cam_roll, s0.cam_pitch, s0.cam_yaw);
        out.rows = [v(s0.cam_fx, s0.cam_fy, s0.cam_fz), v(s0.cam_lx, s0.cam_ly, s0.cam_lz), v(s0.cam_ux, s0.cam_uy, s0.cam_uz)];
    } else if !camera_at_hero {
        notes.push("no camera rows in the start sample: camera set up behind the hero's yaw".into());
    }
    Ok((g, RatchetAnim::new(&lv.ratchet), notes))
}

/// Replays `trace` on `lv` (the samples from the start one on; ticks the recording missed run with the next
/// recorded pad bytes).
pub fn replay(lv: &HeroLevel, trace: &Trace, opt: &ReplayOptions) -> Result<Replay> {
    let s = &trace.samples;
    if s.is_empty() { bail!("the trace has no samples"); }
    let start = match opt.start { Some(i) => i, None => auto_start(s).context("no standing, grounded gameplay sample to start from (record starting while Ratchet stands still)")? };
    let s0 = &s[start];
    let (mut g, mut anim, mut notes) = place(lv, s0, opt.camera_at_hero)?;
    if s0.level != lv.n as i32 { notes.push(format!("trace level {} replayed on level {}", s0.level, lv.n)); }
    let mirror = s0.mirror != 0;
    let mut out = vec![port_sample(&g, &anim, &pad_of(s0)?, mirror, lv.n as i32)];
    let mut filled = 0;
    let mut mobys = |_: &mut MobyTable, _: &Hero, _: &mut rc_game::rng::Rng, _: &rc_game::follow_camera::CameraView, _: &collision::Collision, _: u64| {};
    let mut parts = |_: &Hero, _: &rc_game::follow_camera::CameraView, _: &mut rc_game::rng::Rng, _: u64| {};
    let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: None };
    'outer: for x in &s[start + 1..] {
        let target = x.tick as u64;
        if target <= g.counter {
            notes.push(format!("tick counter went back ({} -> {}): replay stopped", g.counter, target));
            break;
        }
        let pad = pad_of(x)?;
        if target - g.counter > 600 {
            notes.push(format!("gap of {} ticks at {}: replay stopped", target - g.counter, g.counter));
            break;
        }
        while g.counter < target {
            if g.counter + 1 < target { filled += 1; }
            g.hero.idle.counter = g.counter as i32;
            let r = g.tick(Some(&pad), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks);
            if r.hero == HeroTick::OutOfBounds {
                notes.push(format!("the port's hero left the level at tick {}", g.counter));
                break 'outer;
            }
        }
        out.push(port_sample(&g, &anim, &pad, mirror, lv.n as i32));
        if opt.ticks.is_some_and(|n| (g.counter - s0.tick as u64) >= n as u64) { break; }
    }
    if !g.hero.unimplemented_seen.is_empty() { notes.push(format!("states the port does not implement were reached: {:x?}", g.hero.unimplemented_seen)); }
    Ok(Replay { start, samples: out, filled, notes })
}
