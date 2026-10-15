//! The Visibomb's missile view in the renderer: what the game state `rc_game::moby_update::classes::visibomb`
//! (`Services::visibomb`) asks the frame render for while the missile flies (docs/plan/hero_gameplay.md §17).
//!
//! * **The look** (`0x2cb338`, undone by `0x2cb458`): the level fog globals 0x15f444..0x15f454 get the green fog
//!   (0x40 / 0x60 / 0x40, near 0, far 128 units, F 255 → 0) and are restored from the saved copy afterwards
//!   ([`apply_fog`], called by `crate::fog_state` before its `UpdateFog`, so the fog zones keep overwriting them as in
//!   the game; the end of the flight also clears the underwater flag 0x167494); 0x16a478 = 0: `DrawWorld` draws no sky
//!   and clears the frame with `SetBackgroundColor(0x40, 0x60, 0x40)` ([`sky`]: the sky camera draws no layer and
//!   clears to that colour); 0x15f30c: the scanline overlay (`0x21b9f8`, [`MissileViewOverlay`] and
//!   `assets/shaders/missile_view.wgsl`), after the frame and the static.
//! * **The HUD** (`HudDraw` 0x24fb50): skipped in every frame whose tick set 0x17e988 ([`hud_off`], read by
//!   `crate::hud_render`).
//! * **The occlusion** (`UpdateOcclusion`): 0x15f608 = 1, everything visible that frame ([`all_visible`], read by
//!   `crate::occlusion`).
//! * **The range static** (`0x302438`, the range limiter 832's draw on list 2): its quads into the HUD's static layer
//!   ([`statics`]).
//!
//! * **The far distances** (`0x2cb338`, back at `0x2cb458(m, 1)`): the tie cap 0x160fe0 and the moby cap 0x15fff0 at 144
//!   units ([`SHORT_FAR`], read by `crate::tie_lod` and `crate::moby_lod`; they matter in the orbit after the blast).
//!
//! Not modelled [L]: the shrub cap 0x1604a4 (the shrubs cull on the GPU from a distance baked at the load) and the
//! liquid / environment meshes' 0x160f80 (no far cap in the port), both fully fogged beyond 128 during the flight; the
//! particle far 80 (`UpdateFog` rewrites 500 at the end of the same frame: no effect).

use crate::gameplay::Play;
use crate::hud_render::{Hud2dHook, HudBuild};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::ClearColorConfig;
use bevy::core_pipeline::fullscreen_material::FullscreenMaterial;
use bevy::ecs::schedule::ScheduleConfigs;
use bevy::ecs::system::BoxedSystem;
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;
use rc_game::fog_zones::FogGlobals;
use rc_game::moby_update::classes::visibomb::{self, FogWrite, Globals};

/// The fog globals `0x2cb338` saved (the missile's pvars +0x30..+0x42) and the last fog write applied.
#[derive(Clone, Copy, Debug, Default)]
pub struct FogSwap {
    pub seen: u32,
    pub saved: Option<FogGlobals>,
}

/// `0x2cb338`'s fog.
pub fn missile_fog() -> FogGlobals {
    FogGlobals { color: visibomb::FOG_COLOUR, near_dist: 0.0, far_dist: visibomb::FOG_FAR, near_intensity: visibomb::FOG_NEAR_F, far_intensity: 0.0 }
}

/// The missile's writes into the level fog globals since the last call, in order.
pub fn apply_fog(g: &Globals, level: &mut FogGlobals, underwater: &mut bool, swap: &mut FogSwap) {
    let new: Vec<FogWrite> = g.fog_since(swap.seen).map(|x| x.1).collect();
    swap.seen = g.view.fog_seq;
    for f in new {
        match f {
            FogWrite::None => {}
            FogWrite::Swap => {
                swap.saved = Some(*level);
                *level = missile_fog();
            }
            FogWrite::Restore { underwater_off, .. } => {
                if let Some(s) = swap.saved { *level = s; }
                if underwater_off { *underwater = false; }
            }
        }
    }
}

/// The frame of the tick that just ran (`Game::counter` − 1).
fn last_tick(p: &Play) -> u64 { p.game.counter.wrapping_sub(1) }

/// `HudDraw` skips the HUD this frame (0x17e988).
pub fn hud_off(play: Option<&Play>) -> bool { play.is_some_and(|p| p.svc.visibomb.hud_off_at == Some(last_tick(p))) }

/// 0x15f458 = 1 this frame (the missile in flight): the liquid grids keep the level fog (crate::sea_render).
pub fn missile_view(play: Option<&Play>) -> bool { play.is_some_and(|p| p.svc.visibomb.missile_view_at == Some(last_tick(p))) }

/// The occlusion fallback 0x15f608 the last tick set for this frame (`Services::occlusion_fallback`; 0: none).
pub fn occlusion_fallback(play: Option<&Play>) -> u8 {
    play.and_then(|p| p.svc.occlusion_fallback.filter(|&(t, _)| t == last_tick(p))).map_or(0, |(_, v)| v)
}

/// 0x15f608 ≠ 0 this frame (the grid seas' "drawn anyway" test).
pub fn all_visible(play: Option<&Play>) -> bool { occlusion_fallback(play) != 0 }

/// The record `0x21b9f8` draws with: 0x16cc30's bands (alpha 0x0f) on levels 2, 5–8 and 10, else 0x16cc00's (0x17).
pub fn band_alpha(level: u32) -> u32 { if level == 2 || level == 10 || (5..9).contains(&level) { 0x0f } else { 0x17 } }

/// The overlay's full-screen pass (module docs; `assets/shaders/missile_view.wgsl`).
#[derive(Component, ExtractComponent, Clone, Copy, Default, PartialEq, ShaderType)]
pub struct MissileViewOverlay {
    /// The game frame in target pixels (x, y, w, h).
    pub vp: Vec4,
    /// The bands' alpha, the frame's rows, the period and the band rows.
    pub band: UVec4,
}

impl FullscreenMaterial for MissileViewOverlay {
    fn fragment_shader() -> ShaderRef { "shaders/missile_view.wgsl".into() }

    /// `DrawWorld`'s end: after the world and the UI layer (where the port keeps the static, which the game draws
    /// before the overlay in list 2; the HUD is off in the frames with the overlay), before the underwater tint
    /// (`DrawDebugProfiler`, after the HUD) and upscaling. Registered with crate::gs_post.
    fn schedule_configs(system: ScheduleConfigs<BoxedSystem>) -> ScheduleConfigs<BoxedSystem> {
        system
            .after(bevy::core_pipeline::Core3dSystems::PostProcess)
            .after(bevy::ui_render::ui_pass)
            .before(crate::gs_post::pass::<crate::fog_state::UnderwaterTint>)
            .before(bevy::core_pipeline::upscaling::upscaling)
    }
}

pub struct VisibombViewPlugin;

/// The missile view's far distances are on (`visibomb::View::short_far`), set at the start of every frame.
pub static SHORT_FAR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The caps of [`SHORT_FAR`]: 144.0 (0x43100000) and 0x90.
pub const SHORT_TIE_CAP: f32 = 144.0;
pub const SHORT_MOBY_CAP: i32 = 0x90;

fn short_far(play: Option<Res<Play>>) {
    let on = play.as_deref().is_some_and(|p| p.svc.visibomb.view.short_far);
    SHORT_FAR.store(on, std::sync::atomic::Ordering::Relaxed);
}

impl Plugin for VisibombViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::gs_post::GsPostPlugin::<MissileViewOverlay>::default())
            .add_systems(First, short_far)
            .add_systems(Update, (sky, statics.after(crate::menu_render::MenuPrims).before(HudBuild)))
            .add_systems(PostUpdate, overlay);
    }
}

type MainCam<'w, 's> = Query<'w, 's, (Entity, &'static Camera, Option<&'static MissileViewOverlay>), With<crate::fly_cam::FlyCam>>;

/// 0x15f30c: the overlay on the main camera.
fn overlay(mut commands: Commands, play: Option<Res<Play>>, cams: MainCam) {
    let on = play.as_deref().is_some_and(|p| p.svc.visibomb.view.overlay);
    let level = play.as_deref().map_or(1, |p| p.svc.level);
    for (e, cam, have) in &cams {
        let want = on.then(|| {
            let (pos, size) = match &cam.viewport {
                Some(v) => (v.physical_position.as_vec2(), v.physical_size.as_vec2()),
                None => (Vec2::ZERO, cam.physical_target_size().map_or(Vec2::new(1024.0, 832.0), |s| s.as_vec2())),
            };
            MissileViewOverlay { vp: Vec4::new(pos.x, pos.y, size.x, size.y), band: UVec4::new(band_alpha(level), rc_game::hud::SCREEN_H as u32, 34, 31) }
        });
        match (want, have) {
            (Some(w), Some(h)) if *h == w => {}
            (Some(w), _) => { commands.entity(e).insert(w); }
            (None, Some(_)) => { commands.entity(e).remove::<MissileViewOverlay>(); }
            (None, None) => {}
        }
    }
}

/// 0x16a478 = 0 and `SetBackgroundColor`: the sky camera renders no layer and clears to the missile view's colour;
/// its layers and clear come back when the view ends.
#[allow(clippy::type_complexity)]
fn sky(play: Option<Res<Play>>, mut cams: Query<(&mut Camera, &mut RenderLayers), With<crate::sky_render::SkyCamera>>, mut saved: Local<Option<(ClearColorConfig, RenderLayers)>>) {
    let view = play.as_deref().map(|p| p.svc.visibomb.view);
    let off = view.is_some_and(|v| v.no_sky);
    for (mut cam, mut layers) in &mut cams {
        if off {
            if saved.is_none() { *saved = Some((cam.clear_color, layers.clone())); }
            let c = view.and_then(|v| v.background).unwrap_or(visibomb::FOG_COLOUR);
            let want = ClearColorConfig::Custom(Color::srgb_u8(c[0], c[1], c[2]));
            if !matches!((&cam.clear_color, &want), (ClearColorConfig::Custom(a), ClearColorConfig::Custom(b)) if a == b) { cam.clear_color = want; }
            if *layers != RenderLayers::none() { *layers = RenderLayers::none(); }
        } else if let Some((c, l)) = saved.take() {
            cam.clear_color = c;
            *layers = l;
        }
    }
}

/// The range static's quads (the frame of the tick that just ran) into the static layer.
fn statics(play: Option<Res<Play>>, mut hook: ResMut<Hud2dHook>) {
    let Some(p) = play.as_deref() else { return };
    let g = &p.svc.visibomb;
    if g.static_at != Some(last_tick(p)) { return; }
    for s in &g.static_quads { hook.statics[(s.pass as usize).min(2)].push(crate::menu_render::static_prim(s, 0, 0)); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fog_swap_and_restore_in_order() {
        let mut g = Globals::default();
        let level0 = FogGlobals { color: [1, 2, 3], near_dist: 10.0, far_dist: 20.0, near_intensity: 200.0, far_intensity: 50.0 };
        let (mut level, mut uw, mut swap) = (level0, true, FogSwap::default());
        // The game side's writes (two in one frame are both applied, in order).
        let push = |g: &mut Globals, f: FogWrite| {
            g.view.fog_seq += 1;
            g.view.fog = f;
            g.fog_log.push((g.view.fog_seq, f));
        };
        push(&mut g, FogWrite::Swap);
        apply_fog(&g, &mut level, &mut uw, &mut swap);
        assert_eq!(level, missile_fog());
        // A zone overwrites the globals in flight; the restore brings the saved ones back.
        level.near_dist = 99.0;
        push(&mut g, FogWrite::Restore { full: false, underwater_off: false });
        push(&mut g, FogWrite::Restore { full: true, underwater_off: true });
        apply_fog(&g, &mut level, &mut uw, &mut swap);
        assert_eq!((level, uw), (level0, false));
        // Nothing new: nothing changes.
        level.near_dist = 5.0;
        apply_fog(&g, &mut level, &mut uw, &mut swap);
        assert_eq!(level.near_dist, 5.0);
    }

    #[test]
    fn band_records_by_level() {
        assert_eq!((band_alpha(1), band_alpha(2), band_alpha(5), band_alpha(8), band_alpha(9), band_alpha(10), band_alpha(13)), (0x17, 0x0f, 0x0f, 0x0f, 0x17, 0x0f, 0x17));
    }
}
