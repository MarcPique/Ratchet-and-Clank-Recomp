//! `compare-novalis-spawn`: everything one savestate taken at the Novalis spawn can check against the port
//! (docs/plan/trace_results_novalis.md): the persistent game state, the hero block, the follow camera,
//! the fog globals, the `rand` stream and the tick counter, the moby table, and the tie/shrub lit colours.
//!
//! The port side: `rc_game::game_state` (new game → Veldin start + Clank init → transition → Novalis start),
//! and [`crate::port_sim::PortSim`] run for as many ticks as the game's tick counter says (0x15f5cc − 1: the
//! level load's own increment), with the pad scripted from what the RAM says the player did (the last ✕
//! press, from the idle counter; see [`Timeline`]).

use crate::ee::EeImage;
use crate::port_sim::{lcg_distance, LevelData, PortSim};
use crate::tie_shrub_cmp as tsc;
use anyhow::{Context, Result};
use rc_formats::save_game::{ChunkTables, ItemTables, SaveGameLump};
use rc_game::game_state::{GameState, Scope, SessionState};
use rc_game::hero::physics::V4;
use rc_game::moby_runtime::Moby;
use rc_game::pad::{button, PadInput};
use rc_game::ps2v::Pf;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

// Level01 globals (docs/plan/game_state.md, player_controller.md, menus.md, particles.md).
pub const LEVEL: u32 = 0x15ed84;
pub const TICK: u32 = 0x15f5cc;
pub const MODE: u32 = 0x15f5c4;
pub const MODE_FRAMES: u32 = 0x15f5c8;
pub const VSYNC: u32 = 0x15f3f8;
/// `IncrementTickCounter` 0x2ab960: `iGpffffa3f0` = gp − 0x5c10, ticks with a neutral pad.
pub const IDLE: u32 = 0x160ff0;
pub const PLAY_TIME: u32 = 0x15eea4;
pub const RAND_STATE: u32 = 0x12f4d8;
pub const HERO_MOBY_PTR: u32 = 0x1413d0;
pub const CAM_POS: u32 = 0x167240;
pub const CAM_EULER: u32 = 0x167250;
pub const CAM_ROWS: u32 = 0x167450;
pub const TAN_HALF_FOV: u32 = 0x16cf70;
pub const FOG_GLOBALS: u32 = 0x15f444;
pub const UNDERWATER_LOOK: u32 = 0x161200;

/// What the savestate says about the session, for scripting the port.
#[derive(Clone, Copy, Debug)]
pub struct Timeline {
    pub tick_counter: u32,
    pub mode: i32,
    pub mode_frames: i32,
    pub idle: i32,
    /// Port ticks to run: tick counter − 1 (the level load's `0x15f5cc++` after `MobyLoadTimeUpdatePass`).
    pub ticks: u64,
    /// Port tick index of the last non-neutral pad input (a ✕ tap: the hero's previous state is 7), if any.
    pub press_at: Option<u64>,
}

impl Timeline {
    pub fn read(ee: &EeImage) -> Result<Timeline> {
        Ok(Timeline::from_counters(ee.u32(TICK)?, ee.u32(MODE)? as i32, ee.u32(MODE_FRAMES)? as i32, ee.u32(IDLE)? as i32))
    }

    /// The timeline from the four RAM counters (also what `crate::spawn_facts` stores).
    pub fn from_counters(tick_counter: u32, mode: i32, mode_frames: i32, idle: i32) -> Timeline {
        let ticks = tick_counter.saturating_sub(1) as u64;
        // The idle counter is 0 on the tick a button is held and +1 per neutral tick after it.
        let press_at = (idle >= 0 && (idle as u64) < ticks).then(|| ticks - 1 - idle as u64);
        Timeline { tick_counter, mode, mode_frames, idle, ticks, press_at }
    }
}

/// A report being built: text plus per-check tallies.
#[derive(Default)]
pub struct Out {
    pub text: String,
    /// (check, matching, total)
    pub tallies: Vec<(String, u64, u64)>,
}

impl Out {
    pub fn line(&mut self, s: impl AsRef<str>) {
        println!("{}", s.as_ref());
        self.text.push_str(s.as_ref());
        self.text.push('\n');
    }
    pub fn tally(&mut self, name: &str, ok: u64, total: u64) { self.tallies.push((name.to_string(), ok, total)); }
}

fn f(ee: &EeImage, a: u32) -> f32 { f32::from_le_bytes(ee.bytes(a, 4).unwrap().try_into().unwrap()) }
fn v4(ee: &EeImage, a: u32) -> [f32; 4] { std::array::from_fn(|k| f(ee, a + 4 * k as u32)) }
fn i16at(ee: &EeImage, a: u32) -> i16 { ee.u16(a).unwrap() as i16 }

/// One compared scalar.
struct Field { name: String, addr: u32, ram: String, port: String, equal: bool, note: String }

fn fld_f(ee: &EeImage, name: &str, addr: u32, port: Pf) -> Field {
    let r = f(ee, addr);
    let p = port.to_f32();
    let d = (r - p).abs();
    Field { name: name.into(), addr, ram: format!("{r:?}"), port: format!("{p:?}"), equal: r.to_bits() == p.to_bits(), note: if r.to_bits() != p.to_bits() { format!("d={d:e}") } else { String::new() } }
}
fn fld_v(ee: &EeImage, name: &str, addr: u32, port: V4, lanes: usize) -> Vec<Field> {
    (0..lanes).map(|k| fld_f(ee, &format!("{name}.{}", ["x", "y", "z", "w"][k]), addr + 4 * k as u32, port[k])).collect()
}
fn fld_i(name: &str, addr: u32, ram: i64, port: i64) -> Field {
    Field { name: name.into(), addr, ram: ram.to_string(), port: port.to_string(), equal: ram == port, note: String::new() }
}

fn print_fields(out: &mut Out, check: &str, fields: &[Field]) {
    let ok = fields.iter().filter(|f| f.equal).count();
    for x in fields {
        out.line(format!("    {} {:<22} {:#08x}  RAM {:<24} port {:<24} {}", if x.equal { " " } else { "X" }, x.name, x.addr, x.ram, x.port, x.note));
    }
    out.line(format!("  {check}: {ok}/{} fields bit-equal", fields.len()));
    out.tally(check, ok as u64, fields.len() as u64);
}

// ------------------------------------------------------------------------------------------------
// a. Game state

fn chunk_name(scope: Scope, id: i32) -> &'static str {
    match (scope, id) {
        (Scope::Global, 0) => "level", (Scope::Global, 1) => "bolts", (Scope::Global, 2) => "times completed", (Scope::Global, 3) => "elapsed (ticks)",
        (Scope::Global, 4) => "last-save clock", (Scope::Global, 5) => "global flags", (Scope::Global, 7) => "cheats active", (Scope::Global, 8) => "skill points",
        (Scope::Global, 9) => "ammo", (Scope::Global, 10) => "item owned", (Scope::Global, 11) => "item acquired", (Scope::Global, 12) => "vendor stock",
        (Scope::Global, 13) => "quick-select", (Scope::Global, 19) => "max HP", (Scope::Global, 14) => "planet unlocked", (Scope::Global, 20) => "galaxy map order",
        (Scope::Global, 15) => "map landmarks", (Scope::Global, 16) => "help-message records", (Scope::Global, 17) => "misc records", (Scope::Global, 18) => "gadget-help records",
        (Scope::Global, 21) => "last hand item", (Scope::Global, 22) => "wrench held", (Scope::Global, 23) => "thruster last", (Scope::Global, 24) => "unused",
        (Scope::Global, 25) => "camera L/R", (Scope::Global, 26) => "camera U/D", (Scope::Global, 27) => "camera speed", (Scope::Global, 28) => "helpdesk voice",
        (Scope::Global, 29) => "helpdesk text", (Scope::Global, 30) => "gold weapons", (Scope::Global, 31) => "game beaten", (Scope::Global, 32) => "equipped",
        (Scope::Global, 33) => "subtitles", (Scope::Global, 34) => "stereo", (Scope::Global, 35) => "music volume", (Scope::Global, 36) => "effects volume",
        (Scope::Global, 37) => "cheats ever", (Scope::Global, 1000) => "ammo bought", (Scope::Global, 1001) => "ammo picked up", (Scope::Global, 1002) => "ammo used",
        (Scope::Global, 1003) => "play time (ticks)", (Scope::Global, 1004) => "hits taken", (Scope::Global, 1005) => "deaths", (Scope::Global, 1008) => "bolt-rate window",
        (Scope::Global, 1009) => "bolts in window", (Scope::Global, 1010) => "help log", (Scope::Global, 1011) => "help log pos",
        (Scope::Level(_), 3001) => "visited", (Scope::Level(_), 3002) => "packed moby state", (Scope::Level(_), 3003) => "gold bolts",
        (Scope::Level(_), 3004) => "mission bytes", (Scope::Level(_), 3005) => "killed bitset", (Scope::Level(_), 3006) => "bolt drops",
        (Scope::Level(_), 3007) => "entry record", (Scope::Level(_), 3008) => "metal detector", (Scope::Level(_), 4000) => "bolts collected",
        (Scope::Level(_), 4001) => "hits", (Scope::Level(_), 4002) => "deaths",
        _ => "?",
    }
}

/// Chunks whose value depends on how (and how long) Veldin and the Novalis intro were played, not on a
/// port rule: reported with their values, not counted as port mismatches.
pub fn play_dependent(scope: Scope, id: i32) -> bool {
    match scope {
        Scope::Global => matches!(id, 1 | 3 | 4 | 9 | 15 | 16 | 17 | 18 | 1000 | 1001 | 1002 | 1003 | 1004 | 1005 | 1008 | 1009 | 1010 | 1011),
        Scope::Level(l) => l <= 1 && matches!(id, 3002 | 3003 | 3004 | 3005 | 3006 | 3008 | 4000 | 4001 | 4002),
    }
}

fn words(b: &[u8]) -> Vec<i32> { b.chunks(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect() }
fn nonzero(b: &[u8]) -> String {
    let v: Vec<String> = b.iter().enumerate().filter(|(_, &x)| x != 0).map(|(i, x)| format!("[{i}]={x:#x}")).collect();
    if v.is_empty() { "all 0".into() } else { v.join(" ") }
}
fn nonzero_words(b: &[u8]) -> String {
    let v: Vec<String> = words(b).iter().enumerate().filter(|(_, &x)| x != 0).map(|(i, x)| format!("[{i}]={x}")).collect();
    if v.is_empty() { "all 0".into() } else { v.join(" ") }
}
pub fn describe(scope: Scope, id: i32, b: &[u8]) -> String {
    match (scope, id, b.len()) {
        (_, _, 4) => format!("{}", i32::from_le_bytes(b.try_into().unwrap())),
        (Scope::Global, 9 | 13 | 32 | 1000 | 1001 | 1002, _) => nonzero_words(b),
        (Scope::Global, 12, _) => format!("{:02x?}", b),
        (_, _, n) if n <= 40 => nonzero(b),
        _ => {
            let nz = b.iter().filter(|&&x| x != 0).count();
            format!("{nz} non-zero bytes")
        }
    }
}

pub struct GameStateInputs { pub state: GameState, pub session: SessionState }

pub fn port_game_state(extracted: &Path) -> Result<GameStateInputs> {
    let elf = std::fs::read(extracted.join("boot/SCUS_971.99")).context("boot ELF")?;
    let lump = SaveGameLump::parse(&std::fs::read(extracted.join("global/save_game.bin")).context("save_game lump")?)?;
    let ov = |l: u32| std::fs::read(extracted.join(format!("levels/{l:02}/overlay.bin"))).with_context(|| format!("level {l:02} overlay"));
    let tables = ChunkTables::from_boot_elf(&elf)?;
    let veldin = ItemTables::load(&elf, &ov(0)?)?;
    let novalis = ItemTables::load(&elf, &ov(1)?)?;
    let mut s = GameState::new_game(tables, &lump.template)?;
    let mut session = SessionState::default();
    s.apply_level_start(0, &veldin, &mut session);
    s.on_veldin_clank_init(&mut session);
    s.apply_transition(1);
    s.apply_level_start(1, &novalis, &mut session);
    Ok(GameStateInputs { state: s, session })
}

pub fn check_game_state(ee: &EeImage, gs: &GameStateInputs, out: &mut Out) -> Result<()> {
    out.line("\n== a. Game state (every save chunk at its RAM address vs new game -> Veldin -> transition -> Novalis start)");
    let s = &gs.state;
    let (mut eq, mut total, mut play) = (0u64, 0u64, 0u64);
    let mut unexpected = Vec::new();
    let mut scopes: Vec<(Scope, u32)> = vec![(Scope::Global, 0)];
    for l in 0..20 { scopes.push((Scope::Level(l), l as u32)); }
    for (scope, l) in scopes {
        let descs = match scope { Scope::Global => &s.tables.global, Scope::Level(_) => &s.tables.level };
        for d in descs {
            let addr = d.addr + l * d.size;
            let ram = ee.bytes(addr, d.size as usize)?;
            let port = s.chunk_bytes(scope, d);
            let same = ram == port.as_slice();
            let pd = play_dependent(scope, d.id);
            let label = match scope { Scope::Global => format!("global {:>4}", d.id), Scope::Level(l) => format!("L{l:02} {:>4}", d.id) };
            if same {
                eq += 1;
                total += 1;
                if matches!(scope, Scope::Global) || l <= 1 {
                    out.line(format!("     {label} {:<20} {:#08x} equal: {}", chunk_name(scope, d.id), addr, describe(scope, d.id, ram)));
                }
            } else if pd {
                play += 1;
                out.line(format!("   ~ {label} {:<20} {:#08x} play-dependent: RAM {} | port {}", chunk_name(scope, d.id), addr, describe(scope, d.id, ram), describe(scope, d.id, &port)));
            } else {
                total += 1;
                let offs: Vec<usize> = (0..ram.len()).filter(|&k| ram[k] != port[k]).collect();
                out.line(format!("   X {label} {:<20} {:#08x} DIFFERS ({} bytes, first at +{:#x}): RAM {} | port {}", chunk_name(scope, d.id), addr, offs.len(), offs[0],
                    describe(scope, d.id, ram), describe(scope, d.id, &port)));
                unexpected.push(label);
            }
        }
    }
    let hp = ee.u32(0x1415f8)? as i32;
    out.line(format!("     session HP 0x1415f8: RAM {hp} | port {}", gs.session.hp));
    total += 1;
    if hp == gs.session.hp { eq += 1 } else { unexpected.push("HP".into()) }
    out.line(format!("  game state: {eq}/{total} chunks equal ({play} play-dependent chunks listed separately); unexpected: {unexpected:?}"));
    out.tally("a. game state (chunks, excl. play-dependent)", eq, total);
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// b. Hero

pub fn check_hero(ee: &EeImage, sim: &PortSim, out: &mut Out) -> Result<()> {
    out.line("\n== b. Hero block (0x13f350..) vs the port after the same ticks");
    let h = &sim.game.hero;
    let mut v = Vec::new();
    v.extend(fld_v(ee, "pos", 0x13f3d0, h.pos, 4));
    v.extend(fld_v(ee, "rot", 0x13f3e0, h.rot, 3));
    for k in 0..3 { v.extend(fld_v(ee, &format!("rows[{k}]"), 0x13f350 + 16 * k as u32, h.rows[k], 3)); }
    v.extend(fld_v(ee, "vel", 0x13f430, h.vel, 4));
    v.extend(fld_v(ee, "disp", 0x13f450, h.disp, 3));
    v.extend(fld_v(ee, "eff", 0x13f460, h.eff, 3));
    v.extend(fld_v(ee, "momentum", 0x13f4a0, h.momentum, 3));
    for (n, a, p) in [("eff_len", 0x13f4b0, h.eff_len), ("eff_len_xy", 0x13f4b4, h.eff_len_xy), ("fwd_speed", 0x13f4b8, h.fwd_speed), ("slope_ratio", 0x13f4bc, h.slope_ratio),
        ("target_yaw", 0x13f4d0, h.target_yaw), ("yaw_vel", 0x13f4d4, h.yaw_vel), ("yaw_residual", 0x13f4d8, h.yaw_residual), ("target_speed", 0x13f4e0, h.target_speed),
        ("speed", 0x13f4e4, h.speed), ("cap_top", 0x13f570, h.cap_top), ("cap_bottom", 0x13f574, h.cap_bottom), ("cap_top_target", 0x13f578, h.cap_top_target),
        ("cap_bottom_target", 0x13f57c, h.cap_bottom_target), ("cap_radius_target", 0x13f580, h.cap_radius_target), ("cap_radius", 0x13f584, h.cap_radius),
        ("ground_z", 0x13f628, h.ground_z), ("height", 0x13f62c, h.height), ("slope", 0x13f630, h.slope), ("pitch", 0x13f634, h.pitch), ("roll", 0x13f638, h.roll),
        ("slope_yaw", 0x13f63c, h.slope_yaw), ("anim_speed", 0x13fde0, h.anim_speed), ("stick_mag", 0x1415ec, h.stick_mag)] {
        v.push(fld_f(ee, n, a, p));
    }
    v.extend(fld_v(ee, "ground_normal", 0x13f5c0, h.ground_normal, 3));
    v.extend(fld_v(ee, "gravity_dir", 0x13f5e0, h.gravity_dir, 3));
    v.extend(fld_v(ee, "ground_point", 0x13f5f0, h.ground_point, 3));
    v.extend(fld_v(ee, "contact_normal", 0x13f550, h.contact_normal, 3));
    // 0x13f640 is the water level (f32, the ground probe's water branches), not a surface id.
    v.push(fld_f(ee, "water_level", 0x13f640, h.water_level));
    for (n, a, r, p) in [
        ("state", 0x1413d4, ee.u32(0x1413d4)? as i32 as i64, h.state as i64), ("substate", 0x1413d8, ee.u32(0x1413d8)? as i32 as i64, h.substate as i64),
        ("group", 0x1413dc, ee.u32(0x1413dc)? as i32 as i64, h.group as i64), ("prev_state", 0x1413e0, ee.u32(0x1413e0)? as i32 as i64, h.prev_state as i64),
        ("prev_group", 0x1413e4, ee.u32(0x1413e4)? as i32 as i64, h.prev_group as i64), ("timer", 0x13f4e8, ee.u32(0x13f4e8)? as i32 as i64, h.timer as i64),
        ("substate_timer", 0x13f4f0, ee.u32(0x13f4f0)? as i32 as i64, h.substate_timer as i64), ("seq_timer", 0x13f4f4, ee.u32(0x13f4f4)? as i32 as i64, h.seq_timer as i64),
        ("lockout", 0x13f514, ee.u32(0x13f514)? as i32 as i64, h.lockout as i64), ("air_ticks", 0x13f65e, i16at(ee, 0x13f65e) as i64, h.air_ticks as i64),
        ("grounded_ticks", 0x13f650, ee.u32(0x13f650)? as i32 as i64, h.grounded_ticks as i64),
        ("surface_id", 0x140630, i16at(ee, 0x140630) as i64, h.surface_id as i64), ("footstep", 0x14063d, ee.bytes(0x14063d, 1)?[0] as i64, h.footstep as i64),
        ("f532 stuck", 0x13f532, i16at(ee, 0x13f532) as i64, h.f532 as i64), ("look_timer 0x140360", 0x140360, ee.u32(0x140360)? as i32 as i64, h.fidget_timer as i64),
        ("pos_hist_idx", 0x141500, ee.u32(0x141500)? as i32 as i64, h.pos_hist_idx as i64), ("pos_hist_count", 0x141504, ee.u32(0x141504)? as i32 as i64, h.pos_hist_count as i64),
        ("yaw_hist_idx", 0x1414f8, ee.u32(0x1414f8)? as i32 as i64, h.yaw_hist_idx as i64), ("yaw_hist_count", 0x1414fc, ee.u32(0x1414fc)? as i32 as i64, h.yaw_hist_count as i64),
        ("health", 0x1415f8, ee.u32(0x1415f8)? as i32 as i64, h.health as i64), ("gravity_mode", 0x141403, ee.bytes(0x141403, 1)?[0] as i64, h.gravity_mode as i64),
    ] {
        v.push(fld_i(n, a, r, p));
    }
    // Ratchet's moby (+0x10 position, +0x40 Euler).
    let hm = ee.u32(HERO_MOBY_PTR)?;
    let pm = &sim.game.mobys.mobys[sim.hero_id];
    for k in 0..3 {
        let (r, p) = (f(ee, hm + 0x10 + 4 * k), pm.position[k as usize]);
        v.push(Field { name: format!("moby+0x10.{}", ["x", "y", "z"][k as usize]), addr: hm + 0x10 + 4 * k, ram: format!("{r:?}"), port: format!("{p:?}"), equal: r.to_bits() == p.to_bits(), note: String::new() });
    }
    // The idle fields (rc_game::hero::idle): fidget cooldowns and record, head look, secondaries, blinks,
    // Clank's blink and fidget timers, the head joint's angles; Ratchet's and the back mobys' sequence (+0x53).
    let id = &h.idle;
    v.push(fld_i("idle.cooldown", 0x13f538, i16at(ee, 0x13f538) as i64, id.cooldown as i64));
    v.push(fld_i("idle.fidget", 0x1415c0, ee.u32(0x1415c0)? as i32 as i64, id.fidget as i64));
    for k in 0..5 {
        let a = 0x179d70 + 0x70 * k as u32;
        v.push(fld_i(&format!("idle.fidget_cool[{k}]"), a, ee.u32(a)? as i32 as i64, id.fidget_cool[k] as i64));
    }
    for (n, a, p) in [("idle.look_pitch", 0x140354, id.look_pitch), ("idle.look_yaw", 0x140358, id.look_yaw), ("idle.look_k", 0x140364, id.look_k), ("idle.look_d", 0x140368, id.look_d)] {
        v.push(fld_f(ee, n, a, Pf::f(p)));
    }
    for k in 0..4 {
        let a = 0x1403b0 + 4 * k as u32;
        v.push(fld_i(&format!("idle.sec_timer[{k}]"), a, ee.u32(a)? as i32 as i64, id.sec_timer[k] as i64));
    }
    for (n, a, p) in [("idle.blink", 0x140340, id.blink), ("idle.blink_next", 0x140344, id.blink_next), ("idle.blink_period", 0x140348, id.blink_period)] {
        v.push(fld_i(n, a, ee.u32(a)? as i32 as i64, p as i64));
    }
    v.push(fld_i("idle.clank_blink_timer", 0x14034c, i16at(ee, 0x14034c) as i64, id.clank_blink_timer as i64));
    v.push(fld_i("idle.clank_blink", 0x14034e, i16at(ee, 0x14034e) as i64, id.clank_blink as i64));
    v.push(fld_i("idle.clank_fidget_timer", 0x141614, ee.u32(0x141614)? as i32 as i64, id.clank_fidget_timer as i64));
    let (hy, hz) = id.head();
    v.push(fld_f(ee, "idle.head().y", 0x17ad54, Pf::f(hy)));
    v.push(fld_f(ee, "idle.head().z", 0x17ad58, Pf::f(hz)));
    let seq_at = |m: u32| -> Result<i64> { Ok(ee.bytes(m + 0x53, 1)?[0] as i64) };
    v.push(fld_i("Ratchet seq +0x53", hm + 0x53, seq_at(hm)?, sim.anim.state.seq_b as i64));
    for (n, slot, port) in [
        ("pack seq +0x53", 0x1404d0u32, h.back.as_ref().map(|b| b.pack.anim.seq_b)),
        ("Clank seq +0x53", 0x1404d4u32, h.back.as_ref().map(|b| b.clank.anim.seq_b)),
    ] {
        let m = ee.u32(slot)?;
        let ram = if m == 0 { -1 } else { seq_at(m)? };
        v.push(fld_i(n, if m == 0 { slot } else { m + 0x53 }, ram, port.map_or(-1, |s| s as i64)));
    }
    print_fields(out, "b. hero", &v);
    let ram_look = ee.u32(0x140360)? as i32;
    out.line(format!(
        "  look timer 0x140360: HeroInit drew {} (rand_range(180, 300), the first draw after srand(1234)); RAM {ram_look}, port {} after the run \
         (the head look 0x22b928 counts it down and re-arms it)",
        sim.fidget_at_init, h.fidget_timer
    ));
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// c. Camera

pub fn check_camera(ee: &EeImage, sim: &PortSim, out: &mut Out) -> Result<()> {
    out.line("\n== c. Follow camera (0x167240 pos, 0x167250 Euler, 0x167450.. rows, 0x16cf70 tan(hfov/2))");
    let c = &sim.game.camera.out;
    let mut v = Vec::new();
    v.extend(fld_v(ee, "pos", CAM_POS, c.pos, 3));
    v.extend(fld_v(ee, "euler", CAM_EULER, c.euler, 3));
    for (k, n) in ["forward", "left", "up"].iter().enumerate() { v.extend(fld_v(ee, n, CAM_ROWS + 16 * k as u32, c.rows[k], 3)); }
    v.push(fld_f(ee, "tan(hfov/2)", TAN_HALF_FOV, Pf::f(crate::port_sim::TAN_HALF_FOV_X)));
    print_fields(out, "c. camera", &v);
    let (r, p) = (v4(ee, CAM_POS), c.pos_f32());
    let d = ((r[0] - p[0]).powi(2) + (r[1] - p[1]).powi(2) + (r[2] - p[2]).powi(2)).sqrt();
    let h = sim.game.hero.position();
    let dist = |q: [f32; 3]| ((q[0] - h[0]).powi(2) + (q[1] - h[1]).powi(2)).sqrt();
    out.line(format!("  camera position distance RAM-port {d:.5}; horizontal distance to the hero: RAM {:.5}, port {:.5}; height above the feet: RAM {:.5}, port {:.5}",
        dist([r[0], r[1], r[2]]), dist(p), r[2] - h[2], p[2] - h[2]));
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// d. Fog

pub fn check_fog(ee: &EeImage, lv: &LevelData, sim: &PortSim, out: &mut Out) -> Result<()> {
    use rc_game::fog_zones::{self, FogGlobals};
    out.line("\n== d. Fog globals (0x15f444..0x15f457) vs the level settings and the fog zone at the camera");
    let s = &lv.level_settings;
    let w = |o: usize| u32::from_le_bytes(s[o..o + 4].try_into().unwrap());
    let settings = FogGlobals { color: [w(0x0c) as u8, w(0x10) as u8, w(0x14) as u8], near_dist: f32::from_bits(w(0x18)), far_dist: f32::from_bits(w(0x1c)), near_intensity: f32::from_bits(w(0x20)), far_intensity: f32::from_bits(w(0x24)) };
    let c = ee.bytes(FOG_GLOBALS, 3)?;
    let ram = FogGlobals { color: [c[0], c[1], c[2]], near_dist: f(ee, 0x15f448), far_dist: f(ee, 0x15f44c), near_intensity: f(ee, 0x15f450), far_intensity: f(ee, 0x15f454) };
    let cam_ram = v4(ee, CAM_POS);
    let zone_ram = fog_zones::lookup(&lv.fog_zones, [cam_ram[0], cam_ram[1], cam_ram[2]]);
    let zone_port = fog_zones::lookup(&lv.fog_zones, sim.game.camera.out.pos_f32());
    let mut expect = settings;
    if let Some((i, t)) = zone_ram { if lv.fog_zones.zones[i].lerps_fog() { fog_zones::apply(&mut expect, &lv.fog_zones.zones[i], t); } }
    out.line(format!("  RAM globals        {ram:?}"));
    out.line(format!("  level settings     {settings:?}"));
    out.line(format!("  zone at RAM camera {zone_ram:?}; at port camera {zone_port:?}"));
    let eqf = |a: &FogGlobals, b: &FogGlobals| a.color == b.color && [a.near_dist, a.far_dist, a.near_intensity, a.far_intensity].map(f32::to_bits) == [b.near_dist, b.far_dist, b.near_intensity, b.far_intensity].map(f32::to_bits);
    let ok = eqf(&ram, &expect);
    out.line(format!("  {} RAM == settings{}", if ok { "MATCH" } else { "MISMATCH" }, if zone_ram.is_some() { " + zone lerp" } else { " (no zone at the camera)" }));
    let tail = ee.bytes(0x15f440, 4)?;
    out.line(format!("  word before the colour 0x15f440 = {:02x?} (not read by the port)", tail));
    // Underwater alternate set (UpdateFog while 0x167494): 751 rewrites its colour/tint per tick.
    let u = ee.bytes(UNDERWATER_LOOK, 0x18)?;
    // The port's look: the overlay default, then 751's zone colours of every tick (PortSim::look).
    let d = sim.look;
    let ram_u = ([u[0], u[1], u[2], u[3]], [u[4], u[5], u[6]], [f(ee, 0x161208), f(ee, 0x16120c), f(ee, 0x161210), f(ee, 0x161214)]);
    let ok_u = ram_u.0 == d.tint && ram_u.1 == d.fog.color && ram_u.2.map(f32::to_bits) == [d.fog.near_dist, d.fog.far_dist, d.fog.near_intensity, d.fog.far_intensity].map(f32::to_bits);
    out.line(format!("  underwater look 0x161200: RAM tint {:02x?} fog {:02x?} {:?} | port (751 zone {:?}) tint {:02x?} fog {:02x?} {:?} -> {}",
        ram_u.0, ram_u.1, ram_u.2, sim.svc.water.sim.as_ref().map(|r| r.last.zone), d.tint, d.fog.color, [d.fog.near_dist, d.fog.far_dist, d.fog.near_intensity, d.fog.far_intensity], if ok_u { "equal" } else { "DIFFERS" }));
    let underwater = ee.bytes(0x167494, 4)?;
    out.line(format!("  underwater flag 0x167494 = {:02x?}", underwater));
    out.tally("d. fog globals", ok as u64, 1);
    out.tally("d. underwater look", ok_u as u64, 1);
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// e. RNG and tick counter

pub fn check_rng(ee: &EeImage, sim: &PortSim, tl: &Timeline, out: &mut Out) -> Result<()> {
    out.line("\n== e. rand state (0x12f4d8) and tick counter (0x15f5cc)");
    let ram = ee.u32(RAND_STATE)?;
    let port = sim.game.rng.state;
    let n_ram = lcg_distance(rc_game::rng::LEVEL_SEED, ram);
    let n_port = lcg_distance(rc_game::rng::LEVEL_SEED, port);
    let tpt: u64 = sim.draws_per_tick.iter().sum();
    out.line(format!("  RAM state {ram:#010x} = {n_ram} draws after srand(1234) (from 1: {}, from 12345: {})",
        lcg_distance(1, ram), lcg_distance(12345, ram)));
    out.line(format!("  port state {port:#010x} = {n_port} draws: load {} (HeroInit 1 + load pass) + {} ticks x avg {:.2} (min {}, max {})",
        sim.load_draws, sim.draws_per_tick.len(), tpt as f64 / sim.draws_per_tick.len().max(1) as f64,
        sim.draws_per_tick.iter().min().copied().unwrap_or(0), sim.draws_per_tick.iter().max().copied().unwrap_or(0)));
    let diff = n_ram as i64 - n_port as i64;
    out.line(format!("  {}: the game drew {diff:+} more than the port ({:.2} per tick on average over {} ticks, vs the port's {:.2})",
        if ram == port { "MATCH" } else { "MISMATCH" }, n_ram as f64 / tl.ticks.max(1) as f64, tl.ticks, tpt as f64 / tl.ticks.max(1) as f64));
    // Per-tick draw histogram of the port.
    let mut h: BTreeMap<u64, u64> = BTreeMap::new();
    for &d in &sim.draws_per_tick { *h.entry(d).or_default() += 1; }
    let top: Vec<String> = h.iter().map(|(k, v)| format!("{k}:{v}")).collect();
    out.line(format!("  port draws per tick (draws:ticks): {}", top.join(" ")));
    let snd: u64 = sim.sound_draws.iter().sum();
    let a = sim.audio.as_ref().map(|a| a.stats);
    out.line(format!(
        "  of which the sound step (sound_update; Ratchet's trigger sounds draw inside the hero update): {snd} ({:.2} per tick){}",
        snd as f64 / sim.sound_draws.len().max(1) as f64,
        a.map_or(" [no sound layer]".to_string(), |s| format!("; sound: {} plays, {} class sounds requested ({} got a slot)", s.plays, s.class_sounds, s.class_slots))
    ));
    out.tally("e. rand state", (ram == port) as u64, 1);
    let tc_port = sim.game.counter;
    out.line(format!("  tick counter: RAM {} | port Game::counter {tc_port} (load pass at 0, then the load's 0x15f5cc++, then {} ticks) -> {}",
        tl.tick_counter, sim.draws_per_tick.len(), if tc_port == tl.tick_counter as u64 { "equal" } else { "DIFFERS" }));
    out.tally("e. tick counter", (tc_port == tl.tick_counter as u64) as u64, 1);
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// f. Moby table

#[derive(Default, Clone)]
struct ClassTally { n: u64, field_ok: BTreeMap<&'static str, u64> }

pub fn check_mobys(ee: &EeImage, sim: &PortSim, out: &mut Out, csv: &Path) -> Result<()> {
    out.line("\n== f. Moby table: static mobys vs the port's load pass + the same ticks");
    let hero_ptr = ee.u32(HERO_MOBY_PTR)?;
    let hero_index = ee.u32(hero_ptr + 0xac)?;
    let base = hero_ptr - 0x100 * hero_index;
    let port = &sim.game.mobys.mobys;
    let n_static = sim.n_static;
    let mut ram_end = 0usize;
    while ee.bytes(base + 0x100 * ram_end as u32 + 0x20, 1)?[0] != 0xff { ram_end += 1; }
    out.line(format!("  RAM moby array at {base:#010x} (Ratchet = index {hero_index}), {ram_end} slots before the 0xff end; port: {n_static} static + {} slots",
        port.len() - n_static));
    // The loader skips instances whose spawn test fails, so RAM slot j holds the j-th created instance; the
    // port applies the same test, so slot j must hold the same (class +0xa6, spawn id +0xb2) in both.
    let key = |j: usize| -> Result<(i16, i16)> { let r = ee.bytes(base + 0x100 * j as u32, 0x100)?; Ok((i16::from_le_bytes([r[0xa6], r[0xa7]]), i16::from_le_bytes([r[0xb2], r[0xb3]]))) };
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    let mut shifted: Vec<usize> = Vec::new();
    for (i, p) in port.iter().enumerate().take(n_static) {
        if i < ram_end && key(i)? == (p.o_class, p.spawn_id) { pairs.push((i, i)); } else { shifted.push(i); }
    }
    // RAM statics end where the slot no longer holds a record of the level (the first dynamic slot).
    let ram_static_end = n_static.min(ram_end);
    let mut pend_by: BTreeMap<(i32, i32), Vec<usize>> = BTreeMap::new();
    for &ii in &sim.pending { pend_by.entry((sim.lv.instances[ii].o_class, sim.lv.instances[ii].spawn_flags)).or_default().push(ii); }
    out.line(format!("  {} of {n_static} port static slots hold the RAM slot's (class, spawn id) at the same index{}; the spawn test did not create {} instances (class, flags: instances): {}",
        pairs.len(), if shifted.is_empty() { String::new() } else { format!(" (first mismatch at slot {})", shifted[0]) }, sim.pending.len(),
        pend_by.iter().map(|((c, f), v)| format!("({c}, {f:#x}): {} [{}..{}]", v.len(), v[0], v[v.len() - 1])).collect::<Vec<_>>().join("; ")));
    // No RAM slot may hold a record the port rejected.
    let rejected: std::collections::BTreeSet<(i16, i16)> = sim.pending.iter().map(|&ii| (sim.lv.instances[ii].o_class as i16, sim.lv.instances[ii].spawn_id as i16)).collect();
    let mut in_ram = 0;
    for j in 0..ram_end { if rejected.contains(&key(j)?) { in_ram += 1; } }
    out.line(format!("  rejected instances found in RAM anyway: {in_ram}"));
    out.tally("f. moby slots paired by index", pairs.len() as u64, n_static as u64);
    out.tally("f. rejected instances absent from RAM", (sim.pending.len() - in_ram) as u64, sim.pending.len() as u64);
    const FIELDS: [&str; 16] = ["class", "state", "pos", "pos~1e-3", "rot", "mode", "seq_a", "seq_b", "frame_a", "frame_b", "t", "speed", "visible", "update_dist", "draw_dist", "group"];
    let mut by_class: BTreeMap<i16, ClassTally> = BTreeMap::new();
    let mut totals: BTreeMap<&'static str, (u64, u64)> = BTreeMap::new();
    let mut rows = String::from("instance,ram_index,o_class,field,ram,port\n");
    for &(i, jr) in &pairs {
        let p = &port[i];
        let a = base + 0x100 * jr as u32;
        let r = ee.bytes(a, 0x100)?;
        let rf = |o: usize| f32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let r_class = i16::from_le_bytes([r[0xa6], r[0xa7]]);
        let rpos = [rf(0x10), rf(0x14), rf(0x18)];
        let rrot = [rf(0x40), rf(0x44), rf(0x48)];
        let cmp: [(&'static str, bool, String, String); 16] = {
            let pm: &Moby = p;
            let an = &pm.anim;
            [
                ("class", r_class == pm.o_class, r_class.to_string(), pm.o_class.to_string()),
                ("state", r[0x20] == pm.state, format!("{:#x}", r[0x20]), format!("{:#x}", pm.state)),
                ("pos", (0..3).all(|k| rpos[k].to_bits() == pm.position[k].to_bits()), format!("{rpos:?}"), format!("{:?}", &pm.position[..3])),
                ("pos~1e-3", (0..3).all(|k| (rpos[k] - pm.position[k]).abs() < 1e-3), String::new(), String::new()),
                ("rot", (0..3).all(|k| rrot[k].to_bits() == pm.rotation[k].to_bits()), format!("{rrot:?}"), format!("{:?}", &pm.rotation[..3])),
                ("mode", u16::from_le_bytes([r[0x34], r[0x35]]) == pm.mode, format!("{:#06x}", u16::from_le_bytes([r[0x34], r[0x35]])), format!("{:#06x}", pm.mode)),
                ("seq_a", r[0x52] == an.seq_a, r[0x52].to_string(), an.seq_a.to_string()),
                ("seq_b", r[0x53] == an.seq_b, r[0x53].to_string(), an.seq_b.to_string()),
                ("frame_a", r[0x50] == an.frame_a, r[0x50].to_string(), an.frame_a.to_string()),
                ("frame_b", r[0x51] == an.frame_b, r[0x51].to_string(), an.frame_b.to_string()),
                ("t", rf(0x54).to_bits() == an.t.to_bits(), format!("{:?}", rf(0x54)), format!("{:?}", an.t)),
                ("speed", rf(0x58).to_bits() == an.speed.to_bits(), format!("{:?}", rf(0x58)), format!("{:?}", an.speed)),
                ("visible", r[0x31] == pm.visible, r[0x31].to_string(), pm.visible.to_string()),
                ("update_dist", r[0x30] == pm.update_dist, r[0x30].to_string(), pm.update_dist.to_string()),
                ("draw_dist", i16::from_le_bytes([r[0x32], r[0x33]]) == pm.draw_dist, i16::from_le_bytes([r[0x32], r[0x33]]).to_string(), pm.draw_dist.to_string()),
                ("group", r[0x21] as i8 == pm.group, (r[0x21] as i8).to_string(), pm.group.to_string()),
            ]
        };
        let t = by_class.entry(p.o_class).or_default();
        t.n += 1;
        for (name, ok, rv, pv) in cmp {
            *t.field_ok.entry(name).or_default() += ok as u64;
            let e = totals.entry(name).or_default();
            e.0 += ok as u64;
            e.1 += 1;
            if !ok && name != "pos~1e-3" { let _ = writeln!(rows, "{i},{jr},{},{name},\"{rv}\",\"{pv}\"", p.o_class); }
        }
    }
    // The load pass pinned on the stream: every bolt's spin-direction bit (pvar +0x55, its first randi(2)) and
    // every class-500 crate's π/2 turn (randi(2)) come from load-time draws.
    let (mut spin_ok, mut spin_n, mut turn_ok, mut turn_n) = (0u64, 0u64, 0u64, 0u64);
    for &(i, jr) in &pairs {
        let p = &port[i];
        let a = base + 0x100 * jr as u32;
        match p.o_class {
            13 => {
                let pv = ee.u32(a + 0x78)?;
                let r = ee.bytes(pv + 0x55, 1)?[0];
                spin_n += 1;
                spin_ok += (Some(&r) == p.pvars.get(0x55)) as u64;
            }
            500 => {
                let r = f(ee, a + 0x48);
                turn_n += 1;
                turn_ok += (r.to_bits() == p.rotation[2].to_bits()) as u64;
            }
            _ => {}
        }
    }
    out.line(format!("  load-pass draws: bolt spin bits (pvar+0x55) {spin_ok}/{spin_n}, class-500 crate turns (rot z) {turn_ok}/{turn_n}"));
    out.tally("e. load pass: bolt spin bits", spin_ok, spin_n);
    out.tally("e. load pass: crate 500 turns", turn_ok, turn_n);
    crate::write_output(csv, rows)?;
    let line: Vec<String> = FIELDS.iter().map(|f| { let (a, b) = totals.get(f).copied().unwrap_or_default(); format!("{f} {a}/{b}") }).collect();
    out.line(format!("  totals: {}", line.join(", ")));
    for fname in FIELDS { let (a, b) = totals.get(fname).copied().unwrap_or_default(); out.tally(&format!("f. moby {fname}"), a, b); }
    out.line("  per class (n; fields that do not all match: matching/n):");
    for (c, t) in &by_class {
        let bad: Vec<String> = FIELDS.iter().filter_map(|f| { let ok = t.field_ok.get(f).copied().unwrap_or(0); (ok != t.n).then(|| format!("{f} {ok}/{}", t.n)) }).collect();
        if !bad.is_empty() {
            let ported = sim.game.mobys.mobys.iter().find(|m| m.o_class == *c).and_then(|m| m.update_fn).and_then(rc_game::moby_update::scheduler::ported).is_some();
            out.line(format!("    class {c:>5} n {:>3}{}: {}", t.n, if ported { " (ported update)" } else { "" }, bad.join(", ")));
        }
    }
    let all_ok: Vec<String> = by_class.iter().filter(|(_, t)| FIELDS.iter().all(|f| t.field_ok.get(f).copied().unwrap_or(0) == t.n)).map(|(c, t)| format!("{c}x{}", t.n)).collect();
    out.line(format!("  classes with every field equal: {}", all_ok.join(" ")));
    out.line(format!("  every mismatching field written to {}", csv.display()));
    // Dynamic slots.
    let dyn_ram: Vec<String> = (ram_static_end..ram_end).map(|i| {
        let r = ee.bytes(base + 0x100 * i as u32, 0x100).unwrap();
        format!("{i}:{}/{:#x}{}", i16::from_le_bytes([r[0xa6], r[0xa7]]), r[0x20], if r[0x34] & 1 != 0 { "h" } else { "" })
    }).collect();
    let dyn_port: Vec<String> = port.iter().enumerate().skip(n_static).take_while(|(_, m)| m.state != 0xff).map(|(i, m)| format!("{i}:{}/{:#x}{}", m.o_class, m.state, if m.mode & 1 != 0 { "h" } else { "" })).collect();
    out.line(format!("  after the statics (index:class/state, h = hidden): RAM {} | port {}", dyn_ram.join(" "), dyn_port.join(" ")));
    check_flyers_pads(ee, sim, base, out)?;
    Ok(())
}

/// The Blarg flyers (660, `rc_game::moby_update::classes::flyer`), their exhaust emitters (27) and the
/// challenge-gated classes (1135 pads, 304/1456–1465 offers). The flyers' positions depend on the tick count
/// the port cannot follow through scene 5, so besides the positions the check compares what does not: each
/// flyer's processed spline (count after the closing-point drop, the z rounding, the per-point arc lengths)
/// with RAM's copy, the RAM segment's tangents with the port's at RAM's point index, and the port's Hermite
/// point at RAM's t with RAM's curve point (+0xd0).
fn check_flyers_pads(ee: &EeImage, sim: &PortSim, base: u32, out: &mut Out) -> Result<()> {
    use rc_game::moby_update::classes::flyer;
    out.line("\n== f2. Blarg flyers (660), their emitters (27), teleporter pads (1135), gold-weapon offers");
    let port = &sim.game.mobys.mobys;
    let rv = |a: u32| -> Result<[f32; 4]> { Ok(v4(ee, a)) };
    let maxd = |a: [f32; 4], b: [f32; 4], n: usize| (0..n).map(|k| (a[k] - b[k]).abs()).fold(0.0f32, f32::max);
    let d3 = |a: [f32; 4], b: [f32; 4]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    let (mut n, mut st1, mut spl_ok, mut seg_ok, mut curve_ok, mut em_ok, mut em_dir_ok) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for (i, pm) in port.iter().enumerate().take(sim.n_static) {
        if pm.o_class != 660 { continue; }
        n += 1;
        let a = base + 0x100 * i as u32;
        let pv = ee.u32(a + 0x78)?;
        let (r_state, r_mode) = (ee.bytes(a + 0x20, 1)?[0], ee.u16(a + 0x34)?);
        let r_pos = rv(a + 0x10)?;
        let r_idx = ee.u32(pv + 0x60)? as i32;
        let r_spl = ee.u32(pv + 0x74)? as i32;
        let r_t = f(ee, pv + 0xf0);
        let [r_m0, r_m1, r_p0, r_p1, r_d0] = [0x90, 0xa0, 0xb0, 0xc0, 0xd0].map(|o| v4(ee, pv + o));
        let looped = ee.bytes(pv + 0x64, 1)?[0] as i8 >= 0;
        let (tension, bias) = (f(ee, pv + 0xf4), f(ee, pv + 0xf8));
        st1 += (r_state == 1 && pm.state == 1) as u64;
        // The processed spline: RAM's copy (pvar+0x70 → count, points at +0x10) vs the port's.
        let sp = ee.u32(pv + 0x70)?;
        let r_count = ee.u32(sp)? as usize;
        let pts: &[[u32; 4]] = usize::try_from(r_spl).ok().and_then(|s| sim.svc.splines.get(s)).map(|v| v.as_slice()).unwrap_or(&[]);
        let mut spl_d = if r_count == pts.len() { 0.0f32 } else { f32::INFINITY };
        if r_count == pts.len() {
            for (k, q) in pts.iter().enumerate() {
                spl_d = spl_d.max(maxd(rv(sp + 0x10 + 0x10 * k as u32)?, q.map(f32::from_bits), 4));
            }
        }
        spl_ok += (spl_d < 1e-3) as u64;
        let seg = flyer::segment(pts, looped, r_idx, tension, bias);
        // RAM's segment ends at point idx + 1 (p1 = +0xc0); p0 is point idx (or, on the first segment, p1 too).
        let seg_d = maxd(seg[1], r_p1, 4).max(maxd(seg[3], r_m1, 3));
        seg_ok += (seg_d < 1e-3) as u64;
        let c = flyer::curve_point(r_t, &[r_p0, r_p1, r_m0, r_m1]);
        let curve_d = maxd(c, r_d0, 3);
        curve_ok += (curve_d < 1e-3) as u64;
        out.line(format!(
            "  flyer slot {i}: RAM state {r_state} mode {r_mode:#06x} pos {:.2?} seg {r_idx} t {r_t:.3} | port state {} mode {:#06x} pos {:.2?} seg {} t {:.3} | |d| {:.1}; spline {r_spl} ({} pts, RAM {r_count}) max diff {spl_d:.1e}, segment at RAM's point {seg_d:.1e}, curve point at RAM's t {curve_d:.1e}",
            &r_pos[..3], pm.state, pm.mode, &pm.position[..3], rc_game::moby_update::services::pvar::i32(&pm.pvars, 0x60),
            rc_game::moby_update::services::pvar::ff(&pm.pvars, 0xf0), d3(r_pos, pm.position), pts.len()
        ));
        // The linked emitter (bit 1: link +0x140).
        let link = ee.u32(pv + 0x140)? as i32;
        if let Ok(e) = usize::try_from(link) {
            let ea = base + 0x100 * e as u32;
            let (r_e, p_e) = (rv(ea + 0x10)?, port.get(e).map(|m| m.position).unwrap_or([0.0; 4]));
            let (r_off, p_off) = (d3(r_e, r_pos), d3(p_e, pm.position));
            let epv = ee.u32(ea + 0x78)?;
            let r_dir = v4(ee, epv + 0x80);
            let p_dir = port.get(e).map(|m| rc_game::moby_update::services::pvar::v4f(&m.pvars, 0x80)).unwrap_or([0.0; 4]);
            let r_flags = ee.u32(epv + 0x70)?;
            let p_flags = port.get(e).map(|m| rc_game::moby_update::services::pvar::u32(&m.pvars, 0x70)).unwrap_or(0);
            em_ok += ((r_off - p_off).abs() < 0.25) as u64;
            em_dir_ok += (r_flags == p_flags && (r_dir[0] * r_dir[0] + r_dir[1] * r_dir[1] + r_dir[2] * r_dir[2] - 1.0).abs() < 1e-3
                && (p_dir[0] * p_dir[0] + p_dir[1] * p_dir[1] + p_dir[2] * p_dir[2] - 1.0).abs() < 1e-3) as u64;
            out.line(format!(
                "    emitter slot {e}: RAM pos {:.2?} (|e − flyer| {r_off:.3}) P+0x70 {r_flags:#x} P+0x80 {:.3?} | port pos {:.2?} (|e − flyer| {p_off:.3}) P+0x70 {p_flags:#x} P+0x80 {:.3?} | |d| {:.1}",
                &r_e[..3], &r_dir[..3], &p_e[..3], &p_dir[..3], d3(r_e, p_e)
            ));
        }
    }
    out.tally("f2. flyers in state 1", st1, n);
    out.tally("f2. flyer splines (count, z, arc lengths) < 1e-3", spl_ok, n);
    out.tally("f2. flyer segment (p1, m1) at RAM's point < 1e-3", seg_ok, n);
    out.tally("f2. flyer Hermite point at RAM's t < 1e-3", curve_ok, n);
    out.tally("f2. emitter at the flyer's joint (|e − flyer| ± 0.25)", em_ok, n);
    out.tally("f2. emitter P+0x70 equal, P+0x80 unit", em_dir_ok, n);
    // The challenge-gated classes.
    let mut gated = Vec::new();
    for (i, pm) in port.iter().enumerate().take(sim.n_static) {
        if !matches!(pm.o_class, 1135 | 304 | 1456..=1465) { continue; }
        let a = base + 0x100 * i as u32;
        let (r_state, r_mode) = (ee.bytes(a + 0x20, 1)?[0], ee.u16(a + 0x34)?);
        let r_coll = ee.u32(a + 0x94)? != 0;
        gated.push((r_state == pm.state && r_mode == pm.mode && (pm.o_class != 1135 || r_coll == pm.has_collision)) as u64);
        if pm.o_class == 1135 {
            let arms: Vec<String> = (0..3).map(|k| rc_game::moby_update::services::pvar::i32(&pm.pvars, 0x14 + 4 * k)).filter(|&v| v > 0)
                .map(|v| { let m = &port[(v - 1) as usize]; format!("{}:{} {:.2?} rot {:.3?}", v - 1, m.o_class, &m.position[..3], &m.rotation[..3]) }).collect();
            out.line(format!("  pad slot {i} (instance {}): RAM state {r_state} mode {r_mode:#06x} collision {r_coll} | port state {} mode {:#06x} collision {} arms [{}]",
                sim.moby_to_instance[i], pm.state, pm.mode, pm.has_collision, arms.join("; ")));
        }
    }
    out.tally("f2. challenge-gated classes (state, mode, pad collision)", gated.iter().sum(), gated.len() as u64);
    let unported: Vec<String> = sim.svc.fx.unported.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    out.line(format!("  unported branches reached by the ported classes (counts): {}", if unported.is_empty() { "none".into() } else { unported.join("; ") }));
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// g. Tie / shrub palettes

pub fn check_tie_shrub(ee: &EeImage, extracted: &Path, out: &mut Out, max: usize) -> Result<()> {
    out.line("\n== g. Tie (LightTies, record +0x40) and shrub (LightShrubs, *0x1604a0 + i*0x60) lit colours");
    let inp = tsc::Inputs::load(extracted, 1)?;
    let (bank, points) = tsc::ram_banks(ee, inp.disc_bank.count)?;
    let diff_sets: Vec<usize> = (0..rc_formats::tfrag_light::BANK_SETS).filter(|&s| {
        let a = &bank.sets[s];
        let b = &inp.disc_bank.sets[s];
        [a.color_a, a.dir_a, a.color_b, a.dir_b].concat().iter().map(|x| x.to_bits()).ne([b.color_a, b.dir_a, b.color_b, b.dir_b].concat().iter().map(|x| x.to_bits()))
    }).collect();
    out.line(format!("  light bank sets that differ from the disc (gameplay count {}): {diff_sets:?}; point lights active: {}", inp.disc_bank.count,
        points.iter().filter(|p| p.pos[3] != 0.0).count()));
    let t = tsc::compare_ties(ee, &inp, &bank, &points)?;
    t.print("ties", max);
    out.text.push_str(&format!("  ties: {}/{} instances equal, {}/{} entries\n", t.instances_equal, t.located, t.entries_equal, t.entries));
    out.tally("g. tie instances", t.instances_equal as u64, t.located as u64);
    out.tally("g. tie entries", t.entries_equal, t.entries);
    let s = tsc::compare_shrubs(ee, &inp, &bank, &points)?;
    s.print("shrubs", max);
    out.text.push_str(&format!("  shrubs: {}/{} instances equal, {}/{} entries\n", s.instances_equal, s.located, s.entries_equal, s.entries));
    out.tally("g. shrub instances", s.instances_equal as u64, s.located as u64);
    out.tally("g. shrub entries", s.entries_equal, s.entries);
    Ok(())
}

// ------------------------------------------------------------------------------------------------

/// Runs the port for the savestate's timeline: `tl.ticks` ticks, neutral pad except a one-tick ✕ at
/// `press_at` (when the RAM says the hero's previous state was a jump).
pub fn run_port<'l>(lv: &'l LevelData, tl: &Timeline, jump: bool, opt: &crate::port_sim::SimOptions) -> Result<PortSim<'l>> { run_port_hold(lv, tl, if jump { 1 } else { 0 }, opt) }

/// [`run_port`] with ✕ held for `hold` ticks ending at `press_at` (0 = no press).
pub fn run_port_hold<'l>(lv: &'l LevelData, tl: &Timeline, hold: u64, opt: &crate::port_sim::SimOptions) -> Result<PortSim<'l>> {
    let mut sim = PortSim::with_options(lv, opt)?;
    for t in 0..tl.ticks {
        sim.svc.game_mode = if t < opt.cutscene_ticks { 2 } else { 0 };
        // The scene starts inside the first tick's moby loop, whose CameraUpdate still runs; the mode-2 frames
        // after it never update the follow camera.
        sim.game.camera_paused = t >= 1 && t < opt.cutscene_ticks;
        let held = tl.press_at.is_some_and(|p| hold > 0 && t <= p && t + hold > p);
        let pad = if held { PadInput::neutral().press(button::CROSS) } else { PadInput::neutral() };
        sim.tick(pad);
    }
    Ok(sim)
}
