//! Reads one level's disc-derived lumps from the game data folder and parses what the renderer needs.
//!
//! Inputs (the Tier 0 archive `ratchet_clank-extract` writes, never shipped with the repo):
//! `levels/NN/core_index.bin`, `levels/NN/core_data.bin` (WAD-compressed), `levels/NN/gs_ram.bin`.
//! The call chain is the one `crates/rc-formats/tests/formats/golden.rs` checks against the committed snapshot hashes.

use anyhow::{Context, Result};
use rc_formats::{level, texture, tfrag};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Novalis. Level 01 on the NTSC-U disc (docs/plan/roadmap.md "Novalis (level01)",
/// docs/formats/tfrag_rac1.md 9b: 1004 tfrags, 61,919 LOD-0 triangles).
pub const DEFAULT_LEVEL: u32 = 1;

pub struct LoadedLevel {
    pub tfrags: Vec<tfrag::Tfrag>,
    pub textures: Vec<texture::LevelTexture>,
    /// Level-settings fog (gameplay file, NTSC variant), docs/plan/game_camera_fog.md.
    pub fog: crate::game_camera::LevelFog,
    /// Level-settings background colour = the GS frame clear colour (`game_camera::level_background`).
    pub background: [u8; 3],
    /// Moby classes, instances and their lit vertex colours (crate::moby_render).
    pub mobys: crate::moby_render::LevelMobys,
    /// Tie classes, instances and their LightTies colour tables (crate::tie_render).
    pub ties: crate::tie_render::LevelTies,
    /// Shrub classes, instances and their LightShrubs palettes (crate::shrub_render).
    pub shrubs: crate::shrub_render::LevelShrubs,
    /// The sky block and its textures (crate::sky_render).
    pub sky: Option<crate::sky_render::LevelSky>,
    /// Occlusion grid and per-object occlusion words (crate::occlusion).
    pub occlusion: crate::occlusion::LevelOcclusionData,
    /// Tfrag LOD distances and mip chains (crate::tfrag_lod).
    pub tfrag_lod: crate::tfrag_lod::TfragLodData,
    /// Particle textures and the class-27 emitters (crate::particle_render).
    pub particles: crate::particle_render::LevelParticles,
    /// HUD graphics, glyph tables and level text (crate::hud_render); None if they do not load.
    pub hud: Option<crate::hud_render::LevelHud>,
    /// Level-code water tables (crate::water_render; empty when `RC_WATER=0` or the level has none).
    pub water: crate::water_render::LevelWater,
    /// The liquid grids' raw animation frames (crate::sea_render; the sea ports' data is in `water.data`).
    pub sea: crate::sea_render::LevelSea,
    /// The level collision mesh (the hero and the follow camera query it, crate::gameplay); None when it
    /// does not parse (warned; the game tick is then off).
    pub collision: Option<rc_formats::collision::Collision>,
    /// Level settings +0x28: the death height the game copies to `0x15f638` (player_controller.md §10).
    pub death_z: f32,
    /// Sound bank, defs, sound instances and music (crate::audio_out); None with `RC_AUDIO=0`.
    pub audio: Option<rc_game::audio::LevelAudio>,
    /// The decompressed gameplay file (NTSC), for the moby scheduler's loader (pvars, groups, splines, the
    /// spawnable count; crate::gameplay).
    pub gameplay: Vec<u8>,
    /// The level core's moby class list in order (o_class per entry): the class slot of `MobyClassesRelocate`
    /// (0x2549d0) is the index here (crate::gameplay).
    pub moby_class_order: Vec<i32>,
    pub timings: LoadTimings,
}

#[derive(Debug, Default)]
pub struct LoadTimings {
    pub read: Duration,
    pub decompress: Duration,
    pub parse_core: Duration,
    pub parse_tfrags: Duration,
    pub parse_textures: Duration,
}

impl LoadTimings {
    pub fn total(&self) -> Duration { self.read + self.decompress + self.parse_core + self.parse_tfrags + self.parse_textures }
}

/// The game data folder (`--data-dir`, `RC_DATA_DIR`, else the development `extracted/`; crate::disc_source).
pub fn extracted_root() -> PathBuf { crate::disc_source::data_root() }

/// `RC_LEVEL` (decimal level index), else Novalis: the level the boot loads.
pub fn boot_level_index() -> u32 {
    match std::env::var("RC_LEVEL") {
        Ok(s) => s.trim().parse().unwrap_or_else(|_| panic!("RC_LEVEL must be a level index, got {s:?}")),
        Err(_) => DEFAULT_LEVEL,
    }
}

/// The loaded level (0x15ed84 as the engine's loader has it): the boot's [`boot_level_index`] until a runtime level
/// change ([`set_level_index`], crate::level_switch).
pub fn level_index() -> u32 {
    let v = CURRENT_LEVEL.load(std::sync::atomic::Ordering::Relaxed);
    if v == u32::MAX { boot_level_index() } else { v }
}

/// The level loaded by a runtime level change (crate::level_switch).
pub fn set_level_index(index: u32) { CURRENT_LEVEL.store(index, std::sync::atomic::Ordering::Relaxed); }

static CURRENT_LEVEL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);

/// 0x13e056, the ship index `DoSpaceTransition` sets before the load (`rc_game::travel::ship_for`); None at the boot
/// (crate::moby_spawn then takes `RC_SHIP` or the level's first-visit ship).
pub fn ship() -> Option<usize> {
    let v = SHIP.load(std::sync::atomic::Ordering::Relaxed);
    (v >= 0).then_some(v as usize)
}

/// Sets 0x13e056 for the next load (crate::travel_render).
pub fn set_ship(ship: i16) { SHIP.store(ship.clamp(0, 2) as i32, std::sync::atomic::Ordering::Relaxed); }

static SHIP: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

/// 0x160540: a placed ship was adopted (`rc_game::travel::ship::adopt`, Gemlik's 533 after the escape). The game never
/// clears it: it holds for the session (not in the save).
/// `RC_SHIP_ADOPTED=1` starts with it set (debug: a level-13 load with `RC_SHIP=2` adopts the placed 533 at once).
pub fn ship_adopted() -> bool {
    SHIP_ADOPTED.load(std::sync::atomic::Ordering::Relaxed) || std::env::var("RC_SHIP_ADOPTED").is_ok_and(|v| v.trim() == "1")
}
pub fn set_ship_adopted() { SHIP_ADOPTED.store(true, std::sync::atomic::Ordering::Relaxed); }

static SHIP_ADOPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A per-level cache (one `OnceLock` per level index: the level-keyed tables built from the loaded level's overlay).
pub type PerLevel<T> = [std::sync::OnceLock<T>; 20];

/// The cell of the loaded level in a [`PerLevel`] cache.
pub fn per_level<T>(cells: &'static PerLevel<T>) -> &'static std::sync::OnceLock<T> { &cells[(level_index() as usize).min(19)] }

pub fn load_level(root: &Path, index: u32) -> Result<LoadedLevel> {
    // The game data folder (crate::disc_source).
    let read = |name: &str| crate::disc_source::level_file(root, index, name);
    let mut t = LoadTimings::default();

    let t0 = Instant::now();
    let core_index = read("core_index.bin")?;
    let gs_ram = read("gs_ram.bin")?;
    t.read = t0.elapsed();

    let t0 = Instant::now();
    // Decompressed once per process, from the Tier 1 engine cache when it is there (rc_data).
    let core_data = rc_data::level_core_data(root, index).context("decompressing core_data")?;
    t.decompress = t0.elapsed();

    let t0 = Instant::now();
    let core = level::parse_level_core(&core_index, core_data.len()).context("parsing core index")?;
    t.parse_core = t0.elapsed();

    let t0 = Instant::now();
    let mut tfrags = tfrag::parse_level_tfrags(&core, &core_data).context("parsing tfrags")?;
    // Game's LightTfrags pass overwrites the stored RGBA (docs/plan/tfrag_lighting.md); RC_NO_LIGHT=1 skips it.
    crate::tfrag_light::light_level_tfrags(root, index, &mut tfrags).context("lighting tfrags")?;
    t.parse_tfrags = t0.elapsed();

    let t0 = Instant::now();
    let textures = texture::parse_textures(&core, &core_data, &gs_ram).context("decoding textures")?;
    t.parse_textures = t0.elapsed();
    let tfrag_lod = crate::tfrag_lod::load(&core, &core_data, &gs_ram)?;

    let gameplay = rc_data::level_gameplay(root, index).context("decompressing gameplay_ntsc")?.to_vec();
    let fog = crate::game_camera::LevelFog::parse(&gameplay).context("parsing level settings")?;
    let background = crate::game_camera::level_background(&gameplay).context("parsing level settings")?;
    let collision = rc_formats::collision::parse_collision(&core, &core_data)
        .map_err(|e| eprintln!("level {index:02}: collision not parsed ({e}); no game tick"))
        .ok();
    let mobys = crate::moby_render::load_mobys(root, &core, &core_data, &gameplay, collision.as_ref()).context("loading mobys")?;
    let ties = crate::tie_render::load_ties(&core, &core_data, &gs_ram, &gameplay, &textures).context("loading ties")?;
    let shrubs = crate::shrub_render::load_shrubs(&core, &core_data, &gs_ram, &gameplay, &textures).context("loading shrubs")?;

    let sky = crate::sky_render::load(&core, &core_data, index).context("loading sky")?;
    let occlusion = crate::occlusion::load(&core, &core_data, &gameplay, &tfrags, &ties.instances, &mobys.instances).context("loading occlusion")?;

    let particles = crate::particle_render::load(&core, &core_index, &core_data, &gameplay, index).context("loading particles")?;
    let hud = crate::hud_render::load(root, index, &core, &core_index, &core_data, &gameplay).map_err(|e| eprintln!("hud: {e:#}")).ok();
    let water = crate::water_render::load(root, index, &gameplay).context("loading water")?;
    let sea = crate::sea_render::load(&core, &core_index, &core_data, &water);
    let death_z = level_settings_f32(&gameplay, 0x28).context("reading the death height")?;
    let audio = crate::audio_out::load(root, index, &core_index, &core, &core_data, &gameplay, collision.clone());
    let moby_class_order = core.moby_classes.iter().map(|e| e.o_class).collect();
    Ok(LoadedLevel {
        tfrags, textures, fog, background, mobys, ties, shrubs, sky, occlusion, tfrag_lod, particles, hud, water, sea, collision, death_z, audio,
        gameplay, moby_class_order, timings: t,
    })
}

/// The f32 at byte `off` of the level-settings section (gameplay file pointer 0).
fn level_settings_f32(gameplay: &[u8], off: usize) -> Result<f32> {
    let word = |o: usize| gameplay.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).context("gameplay file too short for level settings");
    let base = word(0)? as usize;
    anyhow::ensure!(base != 0, "gameplay file has no level-settings section");
    Ok(f32::from_bits(word(base + off)?))
}
