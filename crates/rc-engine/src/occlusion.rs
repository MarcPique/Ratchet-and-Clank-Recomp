//! Precomputed occlusion at run time: the load-time resolution of every tfrag, tie and moby to its
//! occlusion word (`FUN_00255958`) and the per-frame mask `UpdateOcclusion` (level01 0x2193f8) builds from
//! the camera's grid cell. Spec: docs/plan/occlusion_culling.md (the rule and the renderer plan, "In the
//! port"), data: `rc_formats::occlusion`.
//!
//! Every frame `update_occlusion` runs `OcclusionState::update` once, in `PostUpdate` inside
//! [`OcclusionSet`], and publishes the mask (bit 1023 forced on) as [`OcclusionFrame::mask`]. The tfrag
//! (`tfrag_lod::update_tfrag_modes`), tie (`tie_lod::update_tie_lods`) and moby
//! (`moby_render::update_moby_occlusion`) systems run after the set and test each object's word
//! (`OcclBits::visible`) before any frustum or distance test, as `TfragProc`, `TieProc` and `MobyProc` do.
//! Shrubs never consult it.
//!
//! Environment:
//! - `RC_OCCL=0`: mode off (every mask bit set, everything passes).
//! - `RC_OCCL=1`: freeze. The mask is built once, from the starting camera, then kept while the camera
//!   moves (the debug menu's "freeze", switched on after the first frame).
//! - `RC_OCCL_STATS=1`: print the cell and the occluded / frustum-culled / drawn counts, at most once a second.

use crate::level_load::LoadedLevel;
use bevy::prelude::*;
use rc_formats::gameplay::MobyInstance;
use rc_formats::level::LevelCore;
use rc_formats::occlusion::{
    self, cell_coords, LevelOcclusion, Occlusion, OcclusionFallback, OcclusionMode, OcclusionState, PreviousMask, VisMask, MASK_BYTES,
};
use rc_formats::tfrag::Tfrag;
use rc_formats::tie::TieInstance;

/// The level's occlusion data after load.
pub struct LevelOcclusionData {
    /// The core occlusion grid (`0x15f600`) with its octant override; `None` when the level has none.
    pub grid: Option<Occlusion>,
    /// Per-object words (`LevelOcclusion`), all `ALWAYS` when the grid or the mappings are missing.
    pub objects: LevelOcclusion,
    /// `0x16c4f4` as the loader sets it: `Active` with a grid and mappings, else `Off`.
    pub loader_mode: OcclusionMode,
}

/// Grid (core header 0x0c / 0xa8), gameplay mappings (pointer 0x8c) and the load-time resolution, in the
/// order of `FUN_00255958`. `tfrags` in header-table order, `ties` / `mobys` in gameplay order.
pub fn load(core: &LevelCore, core_data: &[u8], gameplay: &[u8], tfrags: &[Tfrag], ties: &[TieInstance], mobys: &[MobyInstance]) -> anyhow::Result<LevelOcclusionData> {
    let grid = if core.blocks.iter().any(|b| b.name == "occlusion") { Some(occlusion::parse_occlusion(core, core_data)?) } else { None };
    let maps = occlusion::parse_gameplay_occlusion_mappings(gameplay)?;
    let maps = maps.filter(|_| grid.is_some());
    let headers: Vec<_> = tfrags.iter().map(|t| t.header).collect();
    let tie_keys: Vec<i32> = ties.iter().map(|t| t.occlusion_index).collect();
    let objects = occlusion::resolve_level_occlusion(maps.as_ref(), &headers, &tie_keys, mobys);
    let loader_mode = if maps.is_some() { OcclusionMode::Active } else { OcclusionMode::Off };
    Ok(LevelOcclusionData { grid, objects, loader_mode })
}

/// Runs before every system that tests the mask.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OcclusionSet;

/// Counts one renderer reports for the stats line: occlusion-culled, then culled by its own tests, then drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CullCounts {
    pub occluded: usize,
    pub culled: usize,
    pub drawn: usize,
}

/// This frame's mask and what the renderers did with it.
#[derive(Resource)]
pub struct OcclusionFrame {
    /// `0x174180`: the mask every proc tests this frame.
    pub mask: VisMask,
    /// `0x16c4f4` in effect (loader value, or the `RC_OCCL` override).
    pub mode: OcclusionMode,
    pub state: OcclusionState,
    /// Camera cell (`cvt.w.s(cam · 0.25)`) and its grid mask index, for the stats line.
    pub cell: [i32; 3],
    pub cell_mask: Option<u16>,
    pub camera: Vec3,
    pub tfrags: CullCounts,
    pub ties: CullCounts,
    pub mobys: CullCounts,
    /// `RC_OCCL=1`: freeze after the first built mask.
    freeze_after_first: bool,
    stats: bool,
}

fn env(name: &str) -> Option<String> { std::env::var(name).ok().map(|v| v.trim().to_string()) }

pub struct OcclusionPlugin;

impl Plugin for OcclusionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(crate::level_switch::LevelStartup, init)
            .add_systems(crate::level_switch::LevelUnload, crate::level_switch::remove::<OcclusionFrame>)
            .add_systems(PostUpdate, update_occlusion.in_set(OcclusionSet))
            .add_systems(Last, print_stats);
    }
}

fn init(mut commands: Commands, level: Res<crate::Level>) {
    let d = &level.0.occlusion;
    let (mode, freeze_after_first) = match (d.loader_mode, env("RC_OCCL").as_deref()) {
        (_, Some("0")) => (OcclusionMode::Off, false),
        (OcclusionMode::Active, Some("1")) => (OcclusionMode::Active, true),
        (m, _) => (m, false),
    };
    report(&level.0, mode, freeze_after_first);
    commands.insert_resource(OcclusionFrame {
        mask: [0xff; MASK_BYTES],
        mode,
        // The port starts with no previous mask and an all-visible mask (docs/plan/occlusion_culling.md §5).
        state: OcclusionState::default(),
        cell: [0; 3],
        cell_mask: None,
        camera: Vec3::ZERO,
        tfrags: CullCounts::default(),
        ties: CullCounts::default(),
        mobys: CullCounts::default(),
        freeze_after_first,
        stats: env("RC_OCCL_STATS").as_deref() == Some("1"),
    });
}

fn report(level: &LoadedLevel, mode: OcclusionMode, freeze: bool) {
    let d = &level.occlusion;
    let o = &d.objects;
    let mapped = |v: &[occlusion::OcclBits]| v.iter().filter(|b| **b != occlusion::OcclBits::ALWAYS).count();
    match &d.grid {
        Some(g) => println!(
            "occlusion: {} cells, {} masks{}; mapped tfrags {}/{}, ties {}/{}{}, mobys {}/{}{}; mode {:?}{}",
            g.cells.len(), g.masks.len(), if g.octants.is_some() { ", octant override" } else { "" },
            mapped(&o.tfrag), o.tfrag.len(), mapped(&o.tie), o.tie.len(), if o.ties_positional { " (positional)" } else { "" },
            mapped(&o.moby), o.moby.len(), if o.tfrag_out_of_date { "; tfrag mappings out of date" } else { "" },
            mode, if freeze { " (RC_OCCL=1: frozen after the first frame)" } else { "" }
        ),
        None => println!("occlusion: none in this level; everything visible"),
    }
}

/// `UpdateOcclusion` once per frame, at the camera `tfrag_lod` scales by 1024 (the game's 0x167240).
/// Fallback 0 (`0x15f608` is cleared at the end of every frame render), else the value a tick set for its frame (1 the
/// Visibomb's missile, 2 `UpdateModeFreeze` and Gemlik's exploding ship: crate::visibomb_view::occlusion_fallback),
/// debug camera off (`0x16c4ec` = 0 in play).
fn update_occlusion(frame: Option<ResMut<OcclusionFrame>>, level: Res<crate::Level>, cams: Query<&Transform, With<Camera3d>>, play: Option<Res<crate::gameplay::Play>>) {
    let (Some(mut f), Some(cam)) = (frame, cams.iter().next()) else { return };
    let eye = crate::game_camera::game_eye(cam);
    let grid = level.0.occlusion.grid.as_ref();
    let mode = f.mode;
    let fallback = match crate::visibomb_view::occlusion_fallback(play.as_deref()) {
        1 => OcclusionFallback::AllVisible,
        2 => OcclusionFallback::Octants,
        _ => OcclusionFallback::Neighbours,
    };
    f.mask = *f.state.update(grid, mode, fallback, false, eye.to_array());
    if f.freeze_after_first && f.mode == OcclusionMode::Active { f.mode = OcclusionMode::Freeze; }
    f.camera = eye;
    f.cell = cell_coords(eye.to_array());
    f.cell_mask = grid.and_then(|g| g.lookup(f.cell[0], f.cell[1], f.cell[2]));
}

fn print_stats(frame: Option<Res<OcclusionFrame>>, time: Res<Time<Real>>, mut last: Local<f32>) {
    let Some(f) = frame else { return };
    if !f.stats || time.elapsed_secs() - *last < 1.0 { return; }
    *last = time.elapsed_secs();
    let src = match (f.mode, f.cell_mask, f.state.previous) {
        (OcclusionMode::Off, ..) => "off".to_string(),
        (OcclusionMode::Freeze, ..) => "frozen".to_string(),
        (_, Some(m), _) => format!("mask {m}"),
        (_, None, PreviousMask::Union) => "outside grid, neighbour union".to_string(),
        (_, None, PreviousMask::Grid(m)) => format!("outside grid, previous mask {m}"),
        (_, None, PreviousMask::None) => "outside grid, all visible".to_string(),
    };
    let c = |n: &CullCounts| format!("{} occluded / {} culled / {} drawn", n.occluded, n.culled, n.drawn);
    let bits = f.mask.iter().map(|b| b.count_ones()).sum::<u32>();
    println!(
        "occl: cam ({:.1}, {:.1}, {:.1}) cell ({}, {}, {}) {src}, {bits} bits | tfrags {} | ties {} | mobys {}",
        f.camera.x, f.camera.y, f.camera.z, f.cell[0], f.cell[1], f.cell[2], c(&f.tfrags), c(&f.ties), c(&f.mobys)
    );
}
