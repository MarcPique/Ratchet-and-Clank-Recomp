//! **Port-only: the game frame and its presentation.** The PS2 drew one 512×416 buffer; the port draws the whole
//! game (world, sky, HUD, menus, canvases) into one offscreen image of that shape, [`GameFrame`], and a last camera
//! ([`PresentCamera`]) draws it into the window, scaled to fit and centred on black. Every camera that would render
//! to the primary window is pointed at the frame as it is spawned ([`redirect`]), so nothing else needs to know
//! the window's size: overlays laid out on the frame (the HUD composite, the menu layer and its snapshot, the item
//! canvases) match it at any window size, the menu snapshot copies the frame without bars, and a screenshot is the
//! frame itself.
//!
//! **Aspect.** The PS2 drew 512×416 and the TV showed it at 4:3; the frame is 4:3 ([`Aspect::Original`], the
//! default), 16:10 ([`Aspect::Wide16x10`]), 16:9 ([`Aspect::Wide16x9`]), 21:9 or 32:9. The game's 512×416
//! screen (the HUD, the menus, the item canvases) always maps onto the frame's centred 4:3 box ([`ui_box`]: the whole
//! frame at 4:3) and the 3D view widens around it: the world cameras' horizontal tangent is the game's ×6/5 at 16:10,
//! ×4/3 at 16:9 ([`hor_scale`], Hor+: the vertical view is the game's).
//!
//! **Settings** ([`DisplaySettings`], the Advanced Options page; persisted in the port settings file,
//! `crate::render_settings::save_key`): the aspect, output resolution, internal render scale, window mode,
//! monitor, refresh rate, V-Sync, FPS cap and performance overlay. A fixed-height frame follows the chosen aspect;
//! `Window` fits the selected monitor. F11 toggles windowed and borderless modes.
//!
//! The frame's format is the window's swap-chain format (`Bgra8UnormSrgb`), so every pipeline is specialised as it
//! was for the window; it can be copied (screenshots) and sampled (the present sprite, linear filtering).

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy::render::view::Msaa;
use bevy::window::{Monitor, MonitorSelection, OnMonitor, PresentMode, PrimaryMonitor, PrimaryWindow, VideoModeSelection, WindowMode, WindowPosition, WindowRef};

use crate::game_camera::SCREEN_W;

/// The present camera's own layer (nothing else draws in it).
pub const PRESENT_LAYER: usize = 63;

/// The offscreen image every game camera renders into, and its current size (physical pixels).
#[derive(Resource, Clone, Debug)]
pub struct GameFrame {
    pub image: Handle<Image>,
    pub size: UVec2,
}

/// The frame's shape: the TV's 4:3, or a wide screen's 16:10 / 16:9 / 21:9 / 32:9 (the menu row's order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aspect {
    Original,
    Wide16x10,
    Wide16x9,
    Wide21x9,
    Wide32x9,
}

impl Aspect {
    /// The Port Options row's values, in order.
    pub const ALL: [Aspect; 5] = [Aspect::Original, Aspect::Wide16x10, Aspect::Wide16x9, Aspect::Wide21x9, Aspect::Wide32x9];
    pub fn index(self) -> u8 { Self::ALL.iter().position(|&a| a == self).unwrap_or(0) as u8 }
    pub fn from_index(i: u8) -> Self { Self::ALL.get(i as usize).copied().unwrap_or(Aspect::Original) }
    /// Width / height.
    pub fn ratio(self) -> f32 {
        match self {
            Aspect::Original => 4.0 / 3.0,
            Aspect::Wide16x10 => 16.0 / 10.0,
            Aspect::Wide16x9 => 16.0 / 9.0,
            Aspect::Wide21x9 => 21.0 / 9.0,
            Aspect::Wide32x9 => 32.0 / 9.0,
        }
    }
    fn parse(v: &str) -> Option<Self> { Self::ALL.into_iter().find(|a| a.key() == v.trim()) }
    fn key(self) -> &'static str {
        match self {
            Aspect::Original => "4:3",
            Aspect::Wide16x10 => "16:10",
            Aspect::Wide16x9 => "16:9",
            Aspect::Wide21x9 => "21:9",
            Aspect::Wide32x9 => "32:9",
        }
    }
}

/// The factor on the game camera's horizontal tangent (0.63) for `aspect`: 1 at 4:3, 6/5 at 16:10, 4/3 at 16:9 (the
/// frame is that much wider at the same height, and the view keeps the TV's proportions).
pub fn hor_scale(aspect: Aspect) -> f32 { aspect.ratio() / Aspect::Original.ratio() }

/// The game pixels the frame extends past the 512-wide game screen on each side (0 at 4:3, 51 at 16:10, 85 at 16:9): the HUD's
/// layer is that much wider (crate::hud_render), its left / right slots move out by it (`rc_game::hud`).
pub fn side_extra(aspect: Aspect) -> i32 { ((SCREEN_W * hor_scale(aspect) - SCREEN_W) * 0.5).round() as i32 }

/// The render resolution: fit to the window, or a fixed frame height (the width follows the aspect): the PS2's 416
/// lines or a common screen height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    Window,
    Height(u16),
}

/// Internal frame size relative to the selected output resolution. The window, UI layout and
/// simulation keep their own sizes; only the shared game render target changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderScale { Half, TwoThirds, ThreeQuarters, Native, OneAndQuarter, OneAndHalf, Double }

impl RenderScale {
    pub const ALL: [Self; 7] = [Self::Half, Self::TwoThirds, Self::ThreeQuarters, Self::Native, Self::OneAndQuarter, Self::OneAndHalf, Self::Double];
    pub fn index(self) -> u8 { Self::ALL.iter().position(|&v| v == self).unwrap_or(3) as u8 }
    pub fn from_index(i: u8) -> Self { Self::ALL.get(i as usize).copied().unwrap_or(Self::Native) }
    pub fn percent(self) -> u32 { match self { Self::Half => 50, Self::TwoThirds => 67, Self::ThreeQuarters => 75, Self::Native => 100, Self::OneAndQuarter => 125, Self::OneAndHalf => 150, Self::Double => 200 } }
    pub fn parse(s: &str) -> Option<Self> { let n = s.trim().trim_end_matches('%').parse::<u32>().ok()?; Self::ALL.into_iter().find(|v| v.percent() == n) }
    pub fn key(self) -> String { self.percent().to_string() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowKind { Windowed, Borderless, Exclusive }

impl WindowKind {
    pub const ALL: [Self; 3] = [Self::Windowed, Self::Borderless, Self::Exclusive];
    pub fn index(self) -> u8 { Self::ALL.iter().position(|&v| v == self).unwrap_or(0) as u8 }
    pub fn from_index(i: u8) -> Self { Self::ALL.get(i as usize).copied().unwrap_or(Self::Windowed) }
    pub fn parse(s: &str) -> Option<Self> { match s.trim() { "windowed" => Some(Self::Windowed), "borderless" => Some(Self::Borderless), "exclusive" => Some(Self::Exclusive), _ => None } }
    pub fn key(self) -> &'static str { match self { Self::Windowed => "windowed", Self::Borderless => "borderless", Self::Exclusive => "exclusive" } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefreshRate { Auto, Hz60, Hz75, Hz90, Hz120, Hz144, Hz165, Hz240 }

impl RefreshRate {
    pub const ALL: [Self; 8] = [Self::Auto, Self::Hz60, Self::Hz75, Self::Hz90, Self::Hz120, Self::Hz144, Self::Hz165, Self::Hz240];
    pub fn index(self) -> u8 { Self::ALL.iter().position(|&v| v == self).unwrap_or(0) as u8 }
    pub fn from_index(i: u8) -> Self { Self::ALL.get(i as usize).copied().unwrap_or(Self::Auto) }
    pub fn hz(self) -> Option<u32> { match self { Self::Auto => None, Self::Hz60 => Some(60), Self::Hz75 => Some(75), Self::Hz90 => Some(90), Self::Hz120 => Some(120), Self::Hz144 => Some(144), Self::Hz165 => Some(165), Self::Hz240 => Some(240) } }
    pub fn parse(s: &str) -> Option<Self> { if s.trim() == "auto" { return Some(Self::Auto); } let hz = s.trim().parse::<u32>().ok()?; Self::ALL.into_iter().find(|v| v.hz() == Some(hz)) }
    pub fn key(self) -> String { self.hz().map_or_else(|| "auto".into(), |v| v.to_string()) }
}

/// Presentation cap. The game simulation remains at 60 Hz at every value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameLimit { F30, F60, F120, F144, F165, F240, Unlimited }

impl FrameLimit {
    pub const ALL: [Self; 7] = [Self::F30, Self::F60, Self::F120, Self::F144, Self::F165, Self::F240, Self::Unlimited];
    pub fn index(self) -> u8 { Self::ALL.iter().position(|&v| v == self).unwrap_or(1) as u8 }
    pub fn from_index(i: u8) -> Self { Self::ALL.get(i as usize).copied().unwrap_or(Self::F60) }
    pub fn fps(self) -> Option<u32> {
        match self { Self::F30 => Some(30), Self::F60 => Some(60), Self::F120 => Some(120), Self::F144 => Some(144), Self::F165 => Some(165), Self::F240 => Some(240), Self::Unlimited => None }
    }
    pub fn parse(v: &str) -> Option<Self> {
        match v.trim() { "30" => Some(Self::F30), "60" | "1" => Some(Self::F60), "120" => Some(Self::F120), "144" => Some(Self::F144), "165" => Some(Self::F165), "240" => Some(Self::F240), "0" | "unlimited" => Some(Self::Unlimited), _ => None }
    }
    pub fn key(self) -> String { self.fps().map_or_else(|| "unlimited".into(), |n| n.to_string()) }
}

impl Resolution {
    /// The Port Options row's values, in order.
    pub const ALL: [Resolution; 6] = [
        Resolution::Window,
        Resolution::Height(416),
        Resolution::Height(720),
        Resolution::Height(1080),
        Resolution::Height(1440),
        Resolution::Height(2160),
    ];

    pub fn index(self) -> u8 { Self::ALL.iter().position(|&r| r == self).unwrap_or(6) as u8 }
    pub fn from_index(i: u8) -> Self { Self::ALL.get(i as usize).copied().unwrap_or(Resolution::Window) }

    /// `window`, or a height of the list (`720p` or `720`).
    fn parse(v: &str) -> Option<Self> {
        let v = v.trim();
        if v == "window" { return Some(Resolution::Window); }
        let h = v.strip_suffix('p').unwrap_or(v).parse::<u16>().ok()?;
        (240..=2160).contains(&h).then_some(Resolution::Height(h))
    }

    fn key(self) -> String {
        match self {
            Resolution::Window => "window".into(),
            Resolution::Height(h) => format!("{h}p"),
        }
    }
}

/// The display settings a menu can change at run time.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplaySettings {
    pub aspect: Aspect,
    pub resolution: Resolution,
    pub render_scale: RenderScale,
    pub window_kind: WindowKind,
    /// 0 = current screen, 1..8 = numbered connected monitors.
    pub monitor: u8,
    pub refresh_rate: RefreshRate,
    pub vsync: bool,
    pub frame_limit: FrameLimit,
    pub show_stats: bool,
}

/// A display change awaits confirmation; reverting uses the exact previous values.
#[derive(Resource, Default)]
pub struct DisplayPending(pub Option<(DisplaySettings, std::time::Instant)>);

impl DisplayPending {
    pub fn start(&mut self, previous: DisplaySettings) { self.0 = Some((previous, std::time::Instant::now() + std::time::Duration::from_secs(15))); }
    pub fn seconds_left(&self) -> Option<u8> {
        self.0.map(|(_, deadline)| deadline.saturating_duration_since(std::time::Instant::now()).as_secs().min(15) as u8)
    }
    pub fn confirm(&mut self, settings: &DisplaySettings) {
        if let Some((previous, _)) = self.0.take() { settings.save_changed(&previous); }
    }
    pub fn cancel(&mut self, settings: &mut DisplaySettings) {
        if let Some((previous, _)) = self.0.take() { *settings = previous; }
    }
}

impl Default for DisplaySettings {
    fn default() -> Self { DisplaySettings { aspect: Aspect::Original, resolution: Resolution::Window, render_scale: RenderScale::Native, window_kind: WindowKind::Windowed, monitor: 0, refresh_rate: RefreshRate::Auto, vsync: true, frame_limit: FrameLimit::F60, show_stats: false } }
}

impl DisplaySettings {
    /// The settings file's values, then `RC_ASPECT` / `RC_RESOLUTION` / `RC_FULLSCREEN` over them.
    pub fn startup() -> Self {
        let mut s = DisplaySettings::default();
        if let Some(a) = crate::render_settings::load_key("aspect").as_deref().and_then(Aspect::parse) { s.aspect = a; }
        if let Some(a) = std::env::var("RC_ASPECT").ok().as_deref().and_then(Aspect::parse) { s.aspect = a; }
        if let Some(r) = crate::render_settings::load_key("resolution").as_deref().and_then(Resolution::parse) { s.resolution = r; }
        if let Some(r) = crate::render_settings::load_key("render_scale").as_deref().and_then(RenderScale::parse) { s.render_scale = r; }
        if let Some(f) = crate::render_settings::load_key("fullscreen") { s.window_kind = if f.trim() == "1" { WindowKind::Borderless } else { WindowKind::Windowed }; }
        if let Some(v) = crate::render_settings::load_key("window_mode").as_deref().and_then(WindowKind::parse) { s.window_kind = v; }
        if let Some(v) = crate::render_settings::load_key("monitor").and_then(|v| v.trim().parse::<u8>().ok()).filter(|&v| v <= 8) { s.monitor = v; }
        if let Some(v) = crate::render_settings::load_key("refresh_hz").as_deref().and_then(RefreshRate::parse) { s.refresh_rate = v; }
        if let Some(r) = std::env::var("RC_RESOLUTION").ok().as_deref().and_then(Resolution::parse) { s.resolution = r; }
        if let Some(r) = std::env::var("RC_RENDER_SCALE").ok().as_deref().and_then(RenderScale::parse) { s.render_scale = r; }
        if let Ok(f) = std::env::var("RC_FULLSCREEN") { s.window_kind = if f.trim() == "1" { WindowKind::Borderless } else { WindowKind::Windowed }; }
        if let Some(v) = std::env::var("RC_WINDOW_MODE").ok().as_deref().and_then(WindowKind::parse) { s.window_kind = v; }
        if let Some(v) = std::env::var("RC_MONITOR").ok().and_then(|v| v.trim().parse::<u8>().ok()).filter(|&v| v <= 8) { s.monitor = v; }
        if let Some(v) = std::env::var("RC_REFRESH_HZ").ok().as_deref().and_then(RefreshRate::parse) { s.refresh_rate = v; }
        if let Some(v) = crate::render_settings::load_key("vsync").as_deref().and_then(parse_bool) { s.vsync = v; }
        if let Some(v) = crate::render_settings::load_key("fps_limit").as_deref().and_then(FrameLimit::parse) { s.frame_limit = v; }
        if let Some(v) = crate::render_settings::load_key("show_stats").as_deref().and_then(parse_bool) { s.show_stats = v; }
        if let Some(v) = std::env::var("RC_VSYNC").ok().as_deref().and_then(parse_bool) { s.vsync = v; }
        if std::env::var("RC_NOVSYNC").is_ok_and(|v| v.trim() == "1") { s.vsync = false; s.frame_limit = FrameLimit::Unlimited; }
        if let Some(v) = std::env::var("RC_FRAME_CAP").ok().as_deref().and_then(FrameLimit::parse) { s.frame_limit = v; }
        if let Some(v) = std::env::var("RC_SHOW_STATS").ok().as_deref().and_then(parse_bool) { s.show_stats = v; }
        s
    }

    /// Saves only settings changed by this menu action, preserving other launcher-managed keys.
    pub fn save_changed(&self, old: &Self) {
        if self.aspect != old.aspect { crate::render_settings::save_key("aspect", self.aspect.key()); }
        if self.resolution != old.resolution { crate::render_settings::save_key("resolution", &self.resolution.key()); }
        if self.render_scale != old.render_scale { crate::render_settings::save_key("render_scale", &self.render_scale.key()); }
        if self.window_kind != old.window_kind { crate::render_settings::save_key("window_mode", self.window_kind.key()); }
        if self.monitor != old.monitor { crate::render_settings::save_key("monitor", &self.monitor.to_string()); }
        if self.refresh_rate != old.refresh_rate { crate::render_settings::save_key("refresh_hz", &self.refresh_rate.key()); }
        if self.vsync != old.vsync { crate::render_settings::save_key("vsync", if self.vsync { "1" } else { "0" }); }
        if self.frame_limit != old.frame_limit { crate::render_settings::save_key("fps_limit", &self.frame_limit.key()); }
        if self.show_stats != old.show_stats { crate::render_settings::save_key("show_stats", if self.show_stats { "1" } else { "0" }); }
    }
}

fn parse_bool(v: &str) -> Option<bool> { match v.trim() { "1" | "true" | "on" => Some(true), "0" | "false" | "off" => Some(false), _ => None } }

/// The frame's size for `res` and `aspect` in a window of `window` physical pixels: the largest rectangle of the
/// aspect that fits, or one of the chosen height.
pub fn frame_size(res: Resolution, aspect: Aspect, window: UVec2) -> UVec2 {
    let a = aspect.ratio();
    match res {
        Resolution::Height(h) => {
            let h = h as f32;
            UVec2::new((h * a).round() as u32, h as u32)
        }
        Resolution::Window => {
            let h = (window.y as f32).min(window.x as f32 / a);
            UVec2::new((h * a).round() as u32, h.round() as u32).min(window).max(UVec2::ONE)
        }
    }
}

pub fn scaled_frame_size(res: Resolution, aspect: Aspect, window: UVec2, scale: RenderScale) -> UVec2 {
    let base = frame_size(res, aspect, window);
    let percent = scale.percent();
    UVec2::new((base.x.saturating_mul(percent) / 100).clamp(1, 8192), (base.y.saturating_mul(percent) / 100).clamp(1, 8192))
}

/// The frame's centred 4:3 box (origin and size, frame pixels): where the game's 512×416 screen lands.
pub fn ui_box(frame: UVec2) -> (Vec2, Vec2) {
    let h = frame.y as f32;
    let w = (h * 4.0 / 3.0).round().min(frame.x as f32);
    (Vec2::new(((frame.x as f32 - w) * 0.5).round(), 0.0), Vec2::new(w, h))
}

/// The largest rectangle of the frame's aspect that fits `window` (logical units): the present sprite's size.
pub fn fit(frame: UVec2, window: Vec2) -> Vec2 {
    let (fw, fh) = (frame.x.max(1) as f32, frame.y.max(1) as f32);
    // Cross-multiplied (no aspect quotient): a window of the frame's own aspect gets exactly its size.
    if window.x * fh > window.y * fw { Vec2::new(window.y * fw / fh, window.y) } else { Vec2::new(window.x, window.x * fh / fw) }
}

/// A UI node kept on the frame's 4:3 box ([`ui_box`]; percent of its parent, which must span the frame): the HUD
/// composite (and the canvases composed with it, its children), the movie picture.
#[derive(Component)]
pub struct UiBoxed;

/// A 3D camera that draws the game's 512×416 screen (the menu's frame mobys): its viewport is the frame's 4:3 box.
#[derive(Component)]
pub struct BoxedView;

/// The camera that draws the frame into the window.
#[derive(Component)]
pub struct PresentCamera;

/// The frame's sprite on the present camera.
#[derive(Component)]
struct PresentSprite;

pub struct DisplayPlugin;

/// Create the native window in its saved mode. Winit can panic while resolving
/// an exclusive monitor during Windows startup (before our recovery systems run),
/// so use borderless fullscreen there.
pub fn initial_window_mode() -> WindowMode {
    if graphics_guard_path().is_some_and(|path| path.is_file()) { return WindowMode::Windowed; }
    let settings = DisplaySettings::startup();
    // `Current` is unavailable until a native window exists. Winit can select
    // `Primary` or an index while creating that window.
    let monitor = if settings.monitor == 0 { MonitorSelection::Primary } else { MonitorSelection::Index((settings.monitor - 1) as usize) };
    match settings.window_kind {
        WindowKind::Windowed => WindowMode::Windowed,
        WindowKind::Borderless => WindowMode::BorderlessFullscreen(monitor),
        WindowKind::Exclusive if cfg!(target_os = "windows") => WindowMode::BorderlessFullscreen(monitor),
        WindowKind::Exclusive => WindowMode::Fullscreen(monitor, VideoModeSelection::Current),
    }
}

impl Plugin for DisplayPlugin {
    /// The frame exists from `build` on, so plugins built later (crate::determinism's capture) can use it.
    fn build(&self, app: &mut App) {
        let size = UVec2::new(1024, 768);
        let mut img = Image::new_target_texture(size.x, size.y, TextureFormat::Bgra8UnormSrgb, None);
        img.asset_usage = RenderAssetUsages::RENDER_WORLD;
        img.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        img.sampler = ImageSampler::linear();
        let image = app.world_mut().resource_mut::<Assets<Image>>().add(img);
        let mut settings = DisplaySettings::startup();
        if let Some(path) = graphics_guard_path() {
            if path.is_file() {
                let unsafe_settings = settings;
                settings = DisplaySettings::default();
                settings.save_changed(&unsafe_settings);
                eprintln!("display: previous graphics startup did not complete; restored safe windowed settings");
            }
            if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
            let _ = std::fs::write(&path, "Graphics startup or a display change is pending.\n");
        }
        println!("display: aspect {:?}, render resolution {:?}, mode {:?}, monitor {}, refresh {:?}, vsync {}, fps limit {:?}", settings.aspect, settings.resolution, settings.window_kind, settings.monitor, settings.refresh_rate, settings.vsync, settings.frame_limit);
        app.insert_resource(GameFrame { image, size })
            .insert_resource(settings)
            .init_resource::<DisplayPending>()
            .add_observer(redirect)
            .add_systems(Startup, spawn_present)
            .add_systems(First, sync_frame)
            .add_systems(Update, (toggle_fullscreen, rollback_display, apply_mode, fit_present, graphics_guard).chain())
            .add_systems(Last, clear_graphics_guard_on_exit)
            .add_systems(PostUpdate, (place_boxed, box_views, fly_view, refresh_resized_targets
                .after(ResizeTargets)
                .before(bevy::camera::CameraUpdateSystems)));
    }
}

/// A camera spawned for the primary window renders into the frame instead (all but the present camera).
fn redirect(add: On<Add, Camera>, frame: Res<GameFrame>, targets: Query<&RenderTarget>, present: Query<(), With<PresentCamera>>, mut commands: Commands) {
    if present.contains(add.entity) { return; }
    if matches!(targets.get(add.entity), Ok(RenderTarget::Window(WindowRef::Primary))) {
        commands.entity(add.entity).insert(RenderTarget::Image(frame.image.clone().into()));
    }
}

/// The present camera and sprite live for the whole run: a level change keeps them (crate::level_switch despawns
/// every other camera), else the window shows nothing after the first change.
fn spawn_present(mut commands: Commands, frame: Res<GameFrame>) {
    commands.spawn((
        Camera2d,
        crate::level_switch::KeepAcrossLevels,
        Camera { order: 1000, clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        RenderTarget::Window(WindowRef::Primary),
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        RenderLayers::layer(PRESENT_LAYER),
        PresentCamera,
        Name::new("present camera (game frame → window)"),
    ));
    commands.spawn((
        Sprite { image: frame.image.clone(), custom_size: Some(Vec2::new(1024.0, 768.0)), ..default() },
        crate::level_switch::KeepAcrossLevels,
        RenderLayers::layer(PRESENT_LAYER),
        PresentSprite,
        Name::new("game frame"),
    ));
}

/// Keeps the frame at the size the settings and the window ask for.
fn sync_frame(mut frame: ResMut<GameFrame>, settings: Res<DisplaySettings>, window: Option<Single<&Window, With<PrimaryWindow>>>, mut images: ResMut<Assets<Image>>) {
    let Some(w) = window else { return };
    let ws = w.physical_size();
    if ws.x == 0 || ws.y == 0 { return; }
    let want = scaled_frame_size(settings.resolution, settings.aspect, ws, settings.render_scale);
    if want == frame.size { return; }
    if let Some(mut img) = images.get_mut(&frame.image) { img.resize(Extent3d { width: want.x, height: want.y, depth_or_array_layers: 1 }); }
    frame.size = want;
}

/// A copy of the image target `old` at `size`, added as a new image: the caller points its cameras, nodes and
/// materials at it. A target resized in place keeps a stale render-world size (its content came out scaled by the new /
/// old width after an aspect switch); a fresh image starts clean. The old image drops with its last handle.
pub fn fresh_target(images: &mut Assets<Image>, old: &Handle<Image>, size: UVec2) -> Option<Handle<Image>> {
    let mut img = images.get(old)?.clone();
    img.resize(Extent3d { width: size.x.max(1), height: size.y.max(1), depth_or_array_layers: 1 });
    Some(images.add(img))
}

/// The PostUpdate systems that resize camera image targets (crate::menu_render's layer, crate::screen_canvas):
/// [`refresh_resized_targets`] runs after them.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResizeTargets;

/// A camera whose image target was resized this frame (the frame, the HUD layer, the menu layer, the canvases: after
/// their resizing systems) takes the new size before Bevy's camera update. Bevy re-reads an image target's size only
/// on the image's asset event, which reaches `camera_system` a frame later, while a camera whose projection changed
/// (crate::display's tangents on an aspect switch) re-reads it at once: the cameras sharing the frame's depth texture
/// (sized by whichever comes first) would disagree and wgpu rejects the pass (depth 555×416 against colour 740×416).
fn refresh_resized_targets(images: Res<Assets<Image>>, mut cams: Query<(&Camera, &RenderTarget, &mut Projection)>) {
    for (c, t, mut p) in &mut cams {
        let RenderTarget::Image(i) = t else { continue };
        let Some(img) = images.get(&i.handle) else { continue };
        if c.physical_target_size().is_some_and(|s| s != img.size()) { p.set_changed(); }
    }
}

/// F11: fullscreen on / off (saved).
fn toggle_fullscreen(keys: Res<ButtonInput<KeyCode>>, mut settings: ResMut<DisplaySettings>, mut pending: ResMut<DisplayPending>) {
    if !keys.just_pressed(KeyCode::F11) { return; }
    pending.cancel(&mut settings);
    let old = *settings;
    settings.window_kind = if settings.window_kind == WindowKind::Windowed { WindowKind::Borderless } else { WindowKind::Windowed };
    settings.save_changed(&old);
}

fn rollback_display(mut pending: ResMut<DisplayPending>, mut settings: ResMut<DisplaySettings>) {
    if pending.0.is_some_and(|(_, deadline)| std::time::Instant::now() >= deadline) {
        pending.cancel(&mut settings);
        eprintln!("display: change was not confirmed within 15 seconds; previous display settings restored");
    }
}

/// The window mode follows the setting.
fn apply_mode(mut settings: ResMut<DisplaySettings>, mut pending: ResMut<DisplayPending>, monitors: Query<(Entity, &Monitor, Option<&PrimaryMonitor>)>, mut window: Query<(&mut Window, Option<&OnMonitor>), With<PrimaryWindow>>) {
    let Ok((mut w, current)) = window.single_mut() else { return };
    let selected = if settings.monitor == 0 {
        current.and_then(|on| monitors.get(on.0).ok())
            .or_else(|| monitors.iter().find(|(_, _, primary)| primary.is_some()))
    } else { monitors.iter().nth((settings.monitor - 1) as usize) };
    let monitor = selected.map_or(MonitorSelection::Current, |(entity, _, _)| MonitorSelection::Entity(entity));
    let video = selected.and_then(|(_, m, _)| {
        let target = settings.refresh_rate.hz().map(|hz| hz * 1000)
            .or(m.refresh_rate_millihertz).unwrap_or(60_000);
        m.video_modes.iter().copied().filter(|v| v.physical_size == m.physical_size())
            .min_by_key(|v| v.refresh_rate_millihertz.abs_diff(target))
            .filter(|v| settings.refresh_rate.hz().is_none_or(|_| v.refresh_rate_millihertz.abs_diff(target) <= 1000))
    });
    if settings.window_kind == WindowKind::Exclusive && (cfg!(target_os = "windows") || video.is_none()) {
        pending.cancel(&mut settings);
        let old = *settings;
        settings.window_kind = WindowKind::Borderless;
        settings.save_changed(&old);
        eprintln!("display: exclusive fullscreen is unavailable; using borderless fullscreen");
    }
    let want = match settings.window_kind {
        WindowKind::Windowed => WindowMode::Windowed,
        WindowKind::Borderless => WindowMode::BorderlessFullscreen(monitor),
        WindowKind::Exclusive => WindowMode::Fullscreen(monitor, VideoModeSelection::Specific(video.expect("exclusive video mode checked above"))),
    };
    if w.mode != want { w.mode = want; }
    if settings.is_changed() && settings.window_kind == WindowKind::Windowed && settings.monitor != 0 {
        w.position = WindowPosition::Centered(monitor);
    }
    let present = if settings.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };
    if w.present_mode != present { w.present_mode = present; }
}

/// A stale guard means the previous process ended during startup or shortly after a graphics change.
fn graphics_guard_path() -> Option<std::path::PathBuf> { crate::render_settings::settings_path().map(|p| p.with_extension("graphics-pending")) }

fn graphics_guard(settings: Res<DisplaySettings>, mut checked: Local<(Option<DisplaySettings>, Option<std::time::Instant>)>) {
    let now = std::time::Instant::now();
    let changed = checked.0 != Some(*settings);
    if changed {
        if let Some(path) = graphics_guard_path() { let _ = std::fs::write(path, "Graphics startup or a display change is pending.\n"); }
        checked.0 = Some(*settings);
        checked.1 = Some(now);
    }
    if checked.1.is_some_and(|since| now.duration_since(since).as_secs() >= 10) {
        if let Some(path) = graphics_guard_path() { let _ = std::fs::remove_file(path); }
        checked.1 = None;
    }
}

fn clear_graphics_guard_on_exit(mut exits: MessageReader<AppExit>) {
    if exits.read().next().is_some() { clear_graphics_guard(); }
}

/// The window runner can return before another `Last` schedule sees `AppExit` (for example a
/// normal close during the first few seconds). Call this after `App::run` returns as well.
pub fn clear_graphics_guard() {
    if let Some(path) = graphics_guard_path() { let _ = std::fs::remove_file(path); }
}

/// The [`UiBoxed`] nodes on the 4:3 box.
fn place_boxed(frame: Res<GameFrame>, mut nodes: Query<&mut Node, With<UiBoxed>>) {
    let (o, s) = ui_box(frame.size);
    let fw = frame.size.x.max(1) as f32;
    let want = (Val::Percent(o.x * 100.0 / fw), Val::Percent(s.x * 100.0 / fw));
    for mut n in &mut nodes {
        if (n.left, n.width) != want {
            (n.left, n.width) = want;
            (n.top, n.height) = (Val::Px(0.0), Val::Percent(100.0));
        }
    }
}

/// The [`BoxedView`] cameras' viewport: the 4:3 box (none when it is the whole frame).
fn box_views(frame: Res<GameFrame>, mut cams: Query<&mut Camera, With<BoxedView>>) {
    let (o, s) = ui_box(frame.size);
    let want = (s.x < frame.size.x as f32).then(|| bevy::camera::Viewport { physical_position: o.as_uvec2(), physical_size: s.as_uvec2().max(UVec2::ONE), ..default() });
    for mut c in &mut cams {
        let same = match (&c.viewport, &want) {
            (Some(a), Some(b)) => a.physical_position == b.physical_position && a.physical_size == b.physical_size,
            (None, None) => true,
            _ => false,
        };
        if !same { c.viewport = want.clone(); }
    }
}

/// The projection tangents for the game's horizontal tangent `game_tan` (`0x16cf70`, a scene camera's): x widened by
/// [`hor_scale`] (Hor+), y the game's (`× 0.775`, NTSC). Every writer of the world cameras' projection goes through
/// this (crate::play_camera, crate::scene_render, crate::flight_render, [`fly_view`]); the game's own screen-space
/// code keeps `game_tan` (its 512×416 screen is the 4:3 box).
pub fn view_tans(game_tan: f32, aspect: Aspect) -> (f32, f32) { (game_tan * hor_scale(aspect), game_tan * crate::game_camera::NTSC_Y_RATIO) }

/// Sets `p`'s tangents to [`view_tans`] of `game_tan` when they differ; true when it changed them.
pub fn set_view_tans(p: &mut Projection, game_tan: f32, aspect: Aspect) -> bool {
    let (tx, ty) = view_tans(game_tan, aspect);
    let Projection::Custom(c) = p else { return false };
    let Some(g) = c.get_mut::<crate::game_camera::GameProjection>() else { return false };
    if g.tan_x == tx && g.tan_y == ty { return false; }
    (g.tan_x, g.tan_y) = (tx, ty);
    true
}

/// The fly camera (no game camera writes the projection): the game's default tangent through [`view_tans`].
fn fly_view(settings: Res<DisplaySettings>, source: Option<Res<crate::game_camera::CameraSource>>, mut cams: Query<&mut Projection, With<crate::fly_cam::FlyCam>>) {
    if source.is_some_and(|s| *s == crate::game_camera::CameraSource::Play) { return; }
    for mut p in &mut cams { set_view_tans(&mut p, crate::game_camera::TAN_HALF_FOV_X, settings.aspect); }
}

/// The frame's sprite fills the window at the frame's aspect, centred (the camera clears the rest to black).
fn fit_present(frame: Res<GameFrame>, window: Option<Single<&Window, With<PrimaryWindow>>>, mut sprites: Query<&mut Sprite, With<PresentSprite>>) {
    let Some(w) = window else { return };
    let size = fit(frame.size, Vec2::new(w.width(), w.height()));
    for mut s in &mut sprites {
        if s.custom_size != Some(size) { s.custom_size = Some(size); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        let (o, w) = (Aspect::Original, Aspect::Wide16x9);
        assert_eq!(frame_size(Resolution::Window, o, UVec2::new(1024, 768)), UVec2::new(1024, 768));
        assert_eq!(frame_size(Resolution::Window, o, UVec2::new(2560, 1440)), UVec2::new(1920, 1440));
        assert_eq!(frame_size(Resolution::Window, w, UVec2::new(2560, 1440)), UVec2::new(2560, 1440));
        assert_eq!(frame_size(Resolution::Height(1080), o, UVec2::new(300, 300)), UVec2::new(1440, 1080));
        assert_eq!(frame_size(Resolution::Height(1080), Aspect::Wide16x10, UVec2::new(300, 300)), UVec2::new(1728, 1080));
        assert_eq!(frame_size(Resolution::Height(1080), w, UVec2::new(300, 300)), UVec2::new(1920, 1080));
        assert_eq!(frame_size(Resolution::Window, Aspect::Wide16x10, UVec2::new(3024, 1890)), UVec2::new(3024, 1890));
        assert_eq!(frame_size(Resolution::Height(720), Aspect::Wide21x9, UVec2::new(300, 300)), UVec2::new(1680, 720));
        assert_eq!(frame_size(Resolution::Height(720), Aspect::Wide32x9, UVec2::new(300, 300)), UVec2::new(2560, 720));
        assert_eq!(side_extra(Aspect::Wide16x10), 51);
        assert_eq!(Resolution::parse("720"), Some(Resolution::Height(720)));
        assert_eq!(Resolution::parse("901p"), Some(Resolution::Height(901)));
        assert_eq!(Resolution::parse("9000p"), None);
        assert_eq!(Resolution::Height(901).index(), 6);
        assert_eq!(scaled_frame_size(Resolution::Height(720), Aspect::Wide16x9, UVec2::new(1920, 1080), RenderScale::Half), UVec2::new(640, 360));
        assert_eq!(scaled_frame_size(Resolution::Window, Aspect::Wide16x9, UVec2::new(1920, 1080), RenderScale::OneAndHalf), UVec2::new(2880, 1620));
        assert_eq!(ui_box(UVec2::new(2560, 1440)), (Vec2::new(320.0, 0.0), Vec2::new(1920.0, 1440.0)));
        assert_eq!(ui_box(UVec2::new(1024, 768)), (Vec2::ZERO, Vec2::new(1024.0, 768.0)));
        assert_eq!(hor_scale(o), 1.0);
        assert!((hor_scale(w) - 4.0 / 3.0).abs() < 1e-6);
        assert_eq!(fit(UVec2::new(1024, 832), Vec2::new(1600.0, 832.0)), Vec2::new(1024.0, 832.0));
        assert_eq!(fit(UVec2::new(1024, 832), Vec2::new(1024.0, 1000.0)), Vec2::new(1024.0, 832.0));
        for r in Resolution::ALL { assert_eq!(Resolution::from_index(r.index()), r); assert_eq!(Resolution::parse(&r.key()), Some(r)); }
        for a in Aspect::ALL { assert_eq!(Aspect::from_index(a.index()), a); assert_eq!(Aspect::parse(a.key()), Some(a)); }
        for v in FrameLimit::ALL { assert_eq!(FrameLimit::from_index(v.index()), v); assert_eq!(FrameLimit::parse(&v.key()), Some(v)); }
        for v in RenderScale::ALL { assert_eq!(RenderScale::from_index(v.index()), v); assert_eq!(RenderScale::parse(&v.key()), Some(v)); }
        for v in WindowKind::ALL { assert_eq!(WindowKind::from_index(v.index()), v); assert_eq!(WindowKind::parse(v.key()), Some(v)); }
        for v in RefreshRate::ALL { assert_eq!(RefreshRate::from_index(v.index()), v); assert_eq!(RefreshRate::parse(&v.key()), Some(v)); }
        assert_eq!(FrameLimit::parse("999"), None);
        assert_eq!(parse_bool("garbage"), None);
    }
}
