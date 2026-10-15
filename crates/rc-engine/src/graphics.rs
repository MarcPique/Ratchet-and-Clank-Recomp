//! **Port-only: the graphics options** (docs/plan/graphics_options.md). Every option controls one thing, and its first
//! value is always `Original`: what the PS2 showed on a TV. The others are modern alternatives. A preset sets them all
//! at once; it is not stored, it is read back from the options (Custom when they match no preset).
//!
//! | Option | Values | Consumer |
//! |---|---|---|
//! | [`AntiAliasing`] | Original (the game's own softening passes: the AA blit and the display copy), Off, 2x / 4x / 8x multisampling | crate::aa_blit, crate::render_settings (the samples) |
//! | [`Hud`] | Original (the 512×416 HUD scaled up smoothly, as the TV showed it), Sharp pixels | crate::hud_render composite |
//! | [`Textures`] | Original (the GS mip rule, one level per pixel), Smooth (blended levels), Sharp (the GPU's own choice, anisotropic) | the world shaders, through crate::game_camera's shared fog buffer |
//! | [`Detail`] | Original (the game's LOD distances), Far (×2), Farther (×4), Maximum (the full model whenever it is drawn) | crate::tfrag_lod, crate::tie_lod, crate::moby_render, crate::shrub_render (drawing only: the game logic keeps its own decisions) |
//!
//! Persisted in the port settings file (`crate::render_settings::save_key`); `RC_HUD`-style switches are not used here
//! (`RC_HUD=0` already hides the HUD): `RC_GFX_ANTI_ALIASING`, `RC_GFX_HUD`, `RC_GFX_TEXTURES`, `RC_GFX_DETAIL` (the
//! keys below), `RC_MSAA=0|2|4|8` (Anti-aliasing Off or that multisampling) and `RC_GFX_PRESET=original|enhanced`
//! override the file at start.

use bevy::prelude::*;
use bevy::anti_alias::{fxaa::Fxaa, smaa::Smaa, contrast_adaptive_sharpening::ContrastAdaptiveSharpening};
use bevy::image::ImageSampler;
use std::collections::HashSet;

/// One option: its values in the Advanced Options row's order (the first is `Original`) and their settings-file keys.
pub trait GfxOption: Copy + PartialEq + Sized + 'static {
    const ALL: &'static [Self];
    /// The settings-file key of the option.
    const KEY: &'static str;
    fn key(self) -> &'static str;
    fn index(self) -> u8 { Self::ALL.iter().position(|&v| v == self).unwrap_or(0) as u8 }
    fn from_index(i: u8) -> Self { Self::ALL.get(i as usize).copied().unwrap_or(Self::ALL[0]) }
    fn parse(v: &str) -> Option<Self> { Self::ALL.iter().copied().find(|a| a.key() == v.trim()) }
}

/// How the world's edges are smoothed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AntiAliasing {
    /// The game's two full-screen bilinear copies, scaled to the frame (crate::aa_blit).
    #[default]
    Original,
    /// Neither: one sample per pixel, unfiltered.
    Off,
    X2,
    X4,
    X8,
}

impl AntiAliasing {
    /// The world cameras' samples per pixel.
    pub fn samples(self) -> u32 {
        match self {
            AntiAliasing::Original | AntiAliasing::Off => 1,
            AntiAliasing::X2 => 2,
            AntiAliasing::X4 => 4,
            AntiAliasing::X8 => 8,
        }
    }

    /// Multisampling at `n` samples (1 or less: Off).
    pub fn from_samples(n: u32) -> Self {
        match n {
            2 => AntiAliasing::X2,
            4 => AntiAliasing::X4,
            8 => AntiAliasing::X8,
            _ => AntiAliasing::Off,
        }
    }
}

impl GfxOption for AntiAliasing {
    const ALL: &'static [Self] = &[AntiAliasing::Original, AntiAliasing::Off, AntiAliasing::X2, AntiAliasing::X4, AntiAliasing::X8];
    const KEY: &'static str = "anti_aliasing";
    fn key(self) -> &'static str {
        match self {
            AntiAliasing::Original => "original",
            AntiAliasing::Off => "off",
            AntiAliasing::X2 => "2x",
            AntiAliasing::X4 => "4x",
            AntiAliasing::X8 => "8x",
        }
    }
}

/// How the game's 2D screen (HUD, menus, text) reaches the frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Hud {
    /// The 512×416 image scaled to the frame with bilinear filtering, as the TV (and an emulator's display) showed it.
    #[default]
    Original,
    /// Each game pixel a hard square (integer-looking pixel doubling).
    SharpPixels,
    /// Render text and menu quads at 2–4 times the game's internal resolution,
    /// adapting to the output screen while keeping the PS2 layout.
    HighResolution,
}

impl GfxOption for Hud {
    const ALL: &'static [Self] = &[Hud::Original, Hud::SharpPixels, Hud::HighResolution];
    const KEY: &'static str = "hud";
    fn key(self) -> &'static str {
        match self {
            Hud::Original => "original",
            Hud::SharpPixels => "sharp",
            Hud::HighResolution => "high",
        }
    }
}

/// How world textures pick their detail level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Textures {
    /// The GS rule: one whole level from the depth (`round(log2(z / 32) + K)`), bilinear inside it.
    #[default]
    Original,
    /// The same rule without rounding: the two nearest levels blended (no visible switch line).
    Smooth,
    /// The GPU's own level from the screen footprint, with anisotropic filtering.
    Sharp,
}

impl GfxOption for Textures {
    const ALL: &'static [Self] = &[Textures::Original, Textures::Smooth, Textures::Sharp];
    const KEY: &'static str = "textures";
    fn key(self) -> &'static str {
        match self {
            Textures::Original => "original",
            Textures::Smooth => "smooth",
            Textures::Sharp => "sharp",
        }
    }
}

/// How far away models keep their full detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Detail {
    #[default]
    Original,
    Far,
    Farther,
    Maximum,
}

impl Detail {
    /// The factor on the game's LOD distances. Maximum: far past every draw distance (the largest is 720 units), finite
    /// so the morph ramps stay defined.
    pub fn lod_scale(self) -> f32 {
        match self {
            Detail::Original => 1.0,
            Detail::Far => 2.0,
            Detail::Farther => 4.0,
            Detail::Maximum => 1000.0,
        }
    }
}

impl GfxOption for Detail {
    const ALL: &'static [Self] = &[Detail::Original, Detail::Far, Detail::Farther, Detail::Maximum];
    const KEY: &'static str = "detail";
    fn key(self) -> &'static str {
        match self {
            Detail::Original => "original",
            Detail::Far => "far",
            Detail::Farther => "farther",
            Detail::Maximum => "maximum",
        }
    }
}

/// Post-processing after the game frame is presented. Exactly one edge filter is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PostAa { #[default] Off, Fxaa, Smaa }

impl GfxOption for PostAa {
    const ALL: &'static [Self] = &[Self::Off, Self::Fxaa, Self::Smaa];
    const KEY: &'static str = "post_aa";
    fn key(self) -> &'static str { match self { Self::Off => "off", Self::Fxaa => "fxaa", Self::Smaa => "smaa" } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sharpness { #[default] Off, Low, Medium, High }

impl Sharpness {
    pub fn strength(self) -> f32 { match self { Self::Off => 0.0, Self::Low => 0.25, Self::Medium => 0.5, Self::High => 0.75 } }
}

impl GfxOption for Sharpness {
    const ALL: &'static [Self] = &[Self::Off, Self::Low, Self::Medium, Self::High];
    const KEY: &'static str = "sharpness";
    fn key(self) -> &'static str { match self { Self::Off => "off", Self::Low => "low", Self::Medium => "medium", Self::High => "high" } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Anisotropy { X1, X2, X4, X8, #[default] X16 }

impl Anisotropy {
    pub fn samples(self) -> u16 { match self { Self::X1 => 1, Self::X2 => 2, Self::X4 => 4, Self::X8 => 8, Self::X16 => 16 } }
}

impl GfxOption for Anisotropy {
    const ALL: &'static [Self] = &[Self::X1, Self::X2, Self::X4, Self::X8, Self::X16];
    const KEY: &'static str = "anisotropy";
    fn key(self) -> &'static str { match self { Self::X1 => "1x", Self::X2 => "2x", Self::X4 => "4x", Self::X8 => "8x", Self::X16 => "16x" } }
}

/// A preset: a value for every option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Original,
    Enhanced,
    /// The options match no preset (shown, never chosen).
    Custom,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Original, Preset::Enhanced, Preset::Custom];
    pub fn index(self) -> u8 { Self::ALL.iter().position(|&p| p == self).unwrap_or(0) as u8 }
    pub fn from_index(i: u8) -> Self { Self::ALL.get(i as usize).copied().unwrap_or(Preset::Custom) }
}

/// The graphics options (module docs).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct GraphicsSettings {
    pub aa: AntiAliasing,
    pub hud: Hud,
    pub textures: Textures,
    pub detail: Detail,
    pub post_aa: PostAa,
    pub sharpness: Sharpness,
    pub anisotropy: Anisotropy,
}

impl GraphicsSettings {
    /// The options of a preset (`Custom` keeps `self`).
    pub fn with_preset(self, p: Preset) -> Self {
        match p {
            Preset::Original => GraphicsSettings::default(),
            // SMAA avoids rebuilding every world pipeline when this preset is chosen in a live menu.
            // The separate Anti-aliasing row still offers multisampling for players who want it.
            Preset::Enhanced => GraphicsSettings { aa: AntiAliasing::Off, hud: Hud::HighResolution, textures: Textures::Smooth, detail: Detail::Far, post_aa: PostAa::Smaa, sharpness: Sharpness::Off, anisotropy: Anisotropy::X16 },
            Preset::Custom => self,
        }
    }

    /// The preset these options are.
    pub fn preset(&self) -> Preset {
        [Preset::Original, Preset::Enhanced].into_iter().find(|&p| self.with_preset(p) == *self).unwrap_or(Preset::Custom)
    }

    /// The settings file's values, then the `RC_GFX_*` switches over them.
    pub fn startup() -> Self {
        let mut s = GraphicsSettings::default();
        if let Ok(p) = std::env::var("RC_GFX_PRESET") {
            match p.trim() {
                "enhanced" => s = s.with_preset(Preset::Enhanced),
                _ => s = s.with_preset(Preset::Original),
            }
        } else {
            s.aa = file_value(legacy_msaa());
            s.hud = file_value(s.hud);
            s.textures = file_value(s.textures);
            s.detail = file_value(s.detail);
            s.post_aa = file_value(s.post_aa);
            s.sharpness = file_value(s.sharpness);
            s.anisotropy = file_value(s.anisotropy);
        }
        s.aa = env_value(s.aa);
        // The older switch: `RC_MSAA=0|2|4|8` (0 = Off).
        if let Ok(v) = std::env::var("RC_MSAA") { s.aa = AntiAliasing::from_samples(v.trim().parse().unwrap_or(0)); }
        s.hud = env_value(s.hud);
        s.textures = env_value(s.textures);
        s.detail = env_value(s.detail);
        s.post_aa = env_value(s.post_aa);
        s.sharpness = env_value(s.sharpness);
        s.anisotropy = env_value(s.anisotropy);
        s
    }

    /// Writes every option into the port settings file.
    pub fn save(&self) {
        save(self.aa);
        save(self.hud);
        save(self.textures);
        save(self.detail);
        save(self.post_aa);
        save(self.sharpness);
        save(self.anisotropy);
    }
}

/// Before the Anti-aliasing row had Original, the file kept the sample count (`msaa = 0|2|4|8`): a count above 1 stays
/// that multisampling; off (the old default) becomes Original.
fn legacy_msaa() -> AntiAliasing {
    match crate::render_settings::load_key("msaa").and_then(|v| v.trim().parse::<u32>().ok()) {
        Some(n) if n > 1 => AntiAliasing::from_samples(n),
        _ => AntiAliasing::Original,
    }
}

fn file_value<T: GfxOption>(d: T) -> T { crate::render_settings::load_key(T::KEY).as_deref().and_then(T::parse).unwrap_or(d) }

fn env_value<T: GfxOption>(d: T) -> T {
    std::env::var(format!("RC_GFX_{}", T::KEY.to_ascii_uppercase())).ok().as_deref().and_then(T::parse).unwrap_or(d)
}

fn save<T: GfxOption>(v: T) { crate::render_settings::save_key(T::KEY, v.key()); }

pub struct GraphicsPlugin;

impl Plugin for GraphicsPlugin {
    fn build(&self, app: &mut App) {
        let s = GraphicsSettings::startup();
        if s != GraphicsSettings::default() { println!("graphics: {s:?} ({:?})", s.preset()); }
        app.insert_resource(s).init_resource::<WorldSamplers>()
            .add_systems(PreUpdate, (apply_detail, apply_samples, apply_post))
            .add_systems(Update, apply_anisotropy);
    }
}

#[derive(Resource, Default)]
struct WorldSamplers(HashSet<AssetId<Image>>);

/// The world texture builders request 16x samplers. Track those image assets and change only their
/// samplers when the player selects a different value; HUD, UI, films and render targets are untouched.
fn apply_anisotropy(
    g: Res<GraphicsSettings>,
    mut world: ResMut<WorldSamplers>,
    mut events: MessageReader<AssetEvent<Image>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut added = Vec::new();
    for event in events.read() {
        match event {
            AssetEvent::Added { id } => added.push(*id),
            AssetEvent::Removed { id } => { world.0.remove(id); },
            _ => {},
        }
    }
    for id in &added {
        if images.get(*id).is_some_and(|image| matches!(&image.sampler, ImageSampler::Descriptor(d) if d.anisotropy_clamp == 16)) {
            world.0.insert(*id);
        }
    }
    if !g.is_changed() && added.is_empty() { return; }
    let wanted = g.anisotropy.samples();
    for id in &world.0 {
        if !images.get(*id).is_some_and(|image| matches!(&image.sampler, ImageSampler::Descriptor(d) if d.anisotropy_clamp != wanted)) { continue; }
        if let Some(mut image) = images.get_mut(*id) {
            if let ImageSampler::Descriptor(d) = &mut image.sampler { d.anisotropy_clamp = wanted; }
        }
    }
}

fn apply_post(
    g: Res<GraphicsSettings>,
    camera: Query<Entity, With<crate::display::PresentCamera>>,
    mut commands: Commands,
) {
    if !g.is_changed() { return; }
    let Ok(entity) = camera.single() else { return };
    let mut e = commands.entity(entity);
    e.remove::<(Fxaa, Smaa, ContrastAdaptiveSharpening)>();
    match g.post_aa { PostAa::Off => {}, PostAa::Fxaa => { e.insert(Fxaa::default()); }, PostAa::Smaa => { e.insert(Smaa::default()); } }
    if g.sharpness != Sharpness::Off {
        e.insert(ContrastAdaptiveSharpening { sharpening_strength: g.sharpness.strength(), ..default() });
    }
}

/// The Anti-aliasing option's sample count into crate::render_settings (which puts it on the world cameras), clamped to
/// what the GPU offers.
fn apply_samples(g: Res<GraphicsSettings>, render: Option<ResMut<crate::render_settings::RenderSettings>>, supported: Option<Res<crate::render_settings::SupportedMsaa>>) {
    let Some(mut render) = render else { return };
    if !g.is_changed() { return; }
    let want = crate::render_settings::msaa_from_samples(g.aa.samples());
    let want = supported.map_or(want, |s| s.clamp(want));
    if render.msaa != want { render.msaa = want; }
}

/// The Detail distance factor into the moby LOD pick (crate::moby_lod reads it from a static; tfrag, tie and shrub
/// systems and the shaders read the resource or the shared fog buffer).
fn apply_detail(g: Res<GraphicsSettings>) {
    if g.is_changed() { crate::moby_lod::set_lod_scale(g.detail.lod_scale()); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_round_trip() {
        let o = GraphicsSettings::default();
        assert_eq!(o.preset(), Preset::Original);
        let e = o.with_preset(Preset::Enhanced);
        assert_eq!(e.preset(), Preset::Enhanced);
        let c = GraphicsSettings { hud: Hud::SharpPixels, ..o };
        assert_eq!(c.preset(), Preset::Custom);
        assert_eq!(c.with_preset(Preset::Custom), c);
    }

    #[test]
    fn keys_parse_back() {
        for &v in AntiAliasing::ALL { assert_eq!(AntiAliasing::parse(v.key()), Some(v)); }
        for &v in Hud::ALL { assert_eq!(Hud::parse(v.key()), Some(v)); }
        for &v in Textures::ALL { assert_eq!(Textures::parse(v.key()), Some(v)); }
        for &v in Detail::ALL { assert_eq!(Detail::parse(v.key()), Some(v)); }
        for &v in PostAa::ALL { assert_eq!(PostAa::parse(v.key()), Some(v)); }
        for &v in Sharpness::ALL { assert_eq!(Sharpness::parse(v.key()), Some(v)); }
        for &v in Anisotropy::ALL { assert_eq!(Anisotropy::parse(v.key()), Some(v)); }
        assert_eq!(Detail::from_index(9), Detail::Original);
        assert_eq!(AntiAliasing::from_samples(0), AntiAliasing::Off);
        assert_eq!(AntiAliasing::from_samples(4).samples(), 4);
    }
}
