//! Frame-exact runs: a game tick counter that never reads the wall clock in deterministic mode, and a
//! screenshot taken at an exact frame number.
//!
//! Environment:
//! - `RC_SCREENSHOT_FRAME=N` capture the frame rendered by update N (1-based) to `RC_SCREENSHOT`, then exit.
//!   Implies `RC_DETERMINISTIC=1`. `main.rs` then does not schedule its wall-clock capture (`RC_SCREENSHOT_DELAY`).
//! - `RC_DETERMINISTIC=1` every update advances exactly one 60 Hz game tick: `Time<Real>`, `Time<Virtual>`
//!   and `Time` advance by exactly 1/60 s and `FixedUpdate` runs exactly once per update
//!   (`TimeUpdateStrategy::FixedTimesteps(1)` with a 60 Hz fixed clock), so everything driven by `Time`
//!   or `FixedUpdate` is a function of the frame number, not of how fast the machine renders.
//! - `RC_DUMP_FRAMES=<start>..<end>` (dev only, not a Port Option) capture every frame from update `start` to update
//!   `end` (inclusive) as `<RC_DUMP_DIR>/frame_NNNNN.png` (default folder `frames`), then exit. Implies
//!   `RC_DETERMINISTIC=1` and the offscreen target below; `RC_SCREENSHOT_FRAME` wins when both are set. A frame
//!   reached while pipelines still compile is captured anyway, with a warning naming it. Leave `RC_SCREENSHOT` unset
//!   (`main.rs` would schedule its wall-clock capture and end the run).
//! - `RC_DUMP_REALTIME=1` (dev only) keeps `RC_DUMP_FRAMES` in real-time mode (ticks from the wall clock, as in
//!   play; the PNG saving slows the frames, so updates carry 0..n ticks).
//! - `RC_DUMP_TICKS=n,m,…` (dev only) in frame-exact mode, update k advances the n-th entry's ticks (cycled) instead of
//!   one: `1,0` is a 120 Hz display (every other frame has no game tick), `2` a 30 Hz one. `GameTicks` counts ticks,
//!   not updates.
//!
//! [`GameTicks`] is the game's 60 Hz logic tick count since level start: one per update in deterministic
//! mode; otherwise the catch-up count `floor(60·t_virtual)`, like the game's main loop at 30 fps.
//!
//! Always (independent of the mode above), for run-to-run identical frames:
//! - every `Camera3d` gets `NoIndirectDrawing` as it is spawned (`RC_INDIRECT=1` keeps Bevy's GPU indirect
//!   mode): indirect mode orders the instances of each instanced draw with GPU atomics, see
//!   [`no_indirect_drawing`];
//! - the blended (Transparent3d) phase breaks equal-distance ties by entity, see [`stable_transparent_order`].
//!
//! The capture also refuses to fire while the render world still has pipelines compiling (Bevy compiles
//! them asynchronously, so a mesh whose pipeline is not ready is silently skipped): if frame N is reached
//! with pipelines pending, it waits and says so, since that capture would not be comparable.
//!
//! Capture robustness (docs/plan/level_sweep.md H1/H2):
//! - **Offscreen target.** On macOS a window that is fully covered, on another Space or behind a
//!   full-screen app is "occluded": wgpu-hal's Metal surface then refuses to hand out a drawable
//!   (`SurfaceError::Occluded`), Bevy renders nothing to the window, and a window screenshot comes back all
//!   zeros for as long as the window stays hidden (2,000+ frames seen). Every game camera renders into the
//!   game frame (crate::display: an offscreen image in the swapchain format `Bgra8UnormSrgb`), and the capture
//!   is of that image: frame N does not depend on the window being visible. `RC_CAPTURE_WINDOW=1` captures the
//!   window (the frame as presented, on black).
//! - **Bounded retry.** A capture that still comes back blank is retried on later frames for at most
//!   [`MAX_RETRY_FRAMES`] frames past N; then whatever came back is saved, a warning names the frame, and the
//!   process exits with status 3 (the image is not frame N). A later-frame success also exits 3.
//! - **Monitor blips.** Bevy links each window to its monitor (`OnMonitor` → `HasWindows`, `linked_spawn`),
//!   so when winit briefly stops listing the monitor, `create_monitors` despawns the monitor entity and the
//!   window with it, and `exit_on_all_closed` ends the run ("Monitor removed" → "No windows are open,
//!   exiting", exit 0, no PNG). The link is dropped as soon as it is made (nothing here uses it), in every
//!   mode.
//! - **Exit before capture.** If the app exits before the capture was saved, the process exits with
//!   status 2 instead of 0.

use bevy::core_pipeline::core_3d::Transparent3d;
use bevy::math::FloatOrd;
use bevy::prelude::*;
use bevy::render::render_phase::{sort_phase_system, ViewSortedRenderPhases};
use bevy::render::render_resource::PipelineCache;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::OnMonitor;
use bevy::render::view::NoIndirectDrawing;
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::time::TimeUpdateStrategy;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Frames past `RC_SCREENSHOT_FRAME` a blank capture is retried for before it is saved anyway.
pub const MAX_RETRY_FRAMES: u64 = 30;

/// Game logic ticks per second (NTSC).
pub const TICK_HZ: f64 = 60.0;

/// 60 Hz game ticks since start (see the module docs); updated in `First`.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct GameTicks(pub u64);

/// Updates run so far, 1 in the first `Update`; updated in `First`.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct FrameNumber(pub u64);

/// Whether this run is frame-exact (`RC_DETERMINISTIC=1` or `RC_SCREENSHOT_FRAME` set).
#[derive(Resource, Clone, Copy, Debug)]
pub struct Deterministic(pub bool);

fn env(name: &str) -> Option<String> { std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) }

pub fn screenshot_frame() -> Option<u64> { env("RC_SCREENSHOT_FRAME").and_then(|v| v.parse().ok()) }

/// `RC_DUMP_FRAMES=<start>..<end>` (module docs): the inclusive frame range, when `RC_SCREENSHOT_FRAME` is not set.
pub fn dump_frames() -> Option<(u64, u64)> {
    if screenshot_frame().is_some() { return None; }
    let v = env("RC_DUMP_FRAMES")?;
    let (a, b) = v.split_once("..")?;
    let (a, b): (u64, u64) = (a.trim().parse().ok()?, b.trim().trim_start_matches('=').parse().ok()?);
    (a >= 1 && b >= a).then_some((a, b))
}

pub fn deterministic() -> bool {
    screenshot_frame().is_some() || (dump_frames().is_some() && !dump_realtime()) || env("RC_DETERMINISTIC").as_deref() == Some("1")
}

/// `RC_DUMP_REALTIME=1` (dev only): `RC_DUMP_FRAMES` without frame-exact mode (wall-clock ticks, as in play).
pub fn dump_realtime() -> bool { env("RC_DUMP_REALTIME").as_deref() == Some("1") }

/// Pipeline cache state as seen by the render world at the end of its last frame.
#[derive(Resource, Clone, Default)]
struct PipelineStatus(Arc<[AtomicU64; 2]>);

impl PipelineStatus {
    /// (pipelines known, pipelines still waiting to be created).
    fn get(&self) -> (u64, u64) { (self.0[0].load(Ordering::Acquire), self.0[1].load(Ordering::Acquire)) }
}

#[derive(Resource)]
struct FrameShot {
    path: PathBuf,
    frame: u64,
    /// Set by the capture observer when the image came back blank.
    retry: Arc<AtomicBool>,
    /// Set by the capture observer once it has saved (or given up on) the image.
    saved: Arc<AtomicBool>,
    /// The offscreen image the cameras render to (None with `RC_CAPTURE_WINDOW=1`).
    target: Option<Handle<Image>>,
}

/// `RC_DUMP_FRAMES`: the range, the folder and the number of frames saved so far.
#[derive(Resource)]
struct FrameDump {
    dir: PathBuf,
    range: (u64, u64),
    target: Option<Handle<Image>>,
    saved: Arc<AtomicU64>,
}

pub struct DeterminismPlugin;

/// Frame-exact mode: clears this frame's keyboard, mouse and gamepad state right after Bevy's input systems, so no
/// live device input reaches the game, the dev keys or the fly camera.
fn no_live_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut motion: ResMut<bevy::input::mouse::AccumulatedMouseMotion>,
    mut scroll: ResMut<bevy::input::mouse::AccumulatedMouseScroll>,
    mut pads: Query<&mut bevy::input::gamepad::Gamepad>,
) {
    keys.reset_all();
    mouse.reset_all();
    motion.delta = Vec2::ZERO;
    scroll.delta = Vec2::ZERO;
    for mut g in &mut pads {
        g.digital_mut().reset_all();
        let axes: Vec<_> = g.analog().all_axes().copied().collect();
        for a in axes { g.analog_mut().set(a, 0.0); }
    }
}

impl Plugin for DeterminismPlugin {
    fn build(&self, app: &mut App) {
        let det = deterministic();
        app.insert_resource(Deterministic(det))
            .init_resource::<GameTicks>()
            .init_resource::<FrameNumber>()
            .add_systems(First, advance_ticks.after(bevy::time::TimeSystems));
        if env("RC_INDIRECT").as_deref() != Some("1") { app.add_observer(no_indirect_drawing); }
        app.add_observer(unlink_monitor);
        if det {
            app.insert_resource(Time::<Fixed>::from_hz(TICK_HZ)).insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
            // No live input in frame-exact mode: a key the OS delivers to the new window (a stray R respawned Ratchet in
            // one of ten identical captures) must not change the run. Scripted pads (`RC_PLAY_SCRIPT`) are not device input.
            app.add_systems(PreUpdate, no_live_input.after(bevy::input::InputSystems));
            match tick_pattern() {
                Some(p) => {
                    app.add_systems(First, pattern_steps.before(bevy::time::TimeSystems));
                    println!("determinism: frame-exact mode, RC_DUMP_TICKS {p:?} ticks of 1/60 s per update (cycled)");
                }
                None => println!("determinism: frame-exact mode (1 tick of 1/60 s per update)"),
            }
        }
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Render,
                stable_transparent_order.after(sort_phase_system::<Transparent3d>).in_set(RenderSystems::PhaseSort),
            );
        }
        let dump = dump_frames();
        let frame = screenshot_frame();
        if frame.is_none() && dump.is_none() { return; }
        let path = std::env::var_os("RC_SCREENSHOT");
        if frame.is_some() && path.is_none() {
            eprintln!("RC_SCREENSHOT_FRAME is set without RC_SCREENSHOT: no capture");
            return;
        }
        // The game frame (crate::display: every game camera renders into it, at its size); RC_CAPTURE_WINDOW=1 captures
        // the window (the frame presented on black).
        let target = (env("RC_CAPTURE_WINDOW").as_deref() != Some("1")).then(|| app.world().resource::<crate::display::GameFrame>().image.clone());
        if target.is_some() { println!("determinism: capturing the game frame (RC_CAPTURE_WINDOW=1 captures the window)"); }
        let status = PipelineStatus::default();
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.insert_resource(status.clone()).add_systems(Render, record_pipelines.in_set(RenderSystems::Cleanup));
        }
        app.insert_resource(status);
        if let Some(range) = dump {
            let dir = PathBuf::from(std::env::var_os("RC_DUMP_DIR").unwrap_or_else(|| "frames".into()));
            if let Err(e) = std::fs::create_dir_all(&dir) { eprintln!("RC_DUMP_FRAMES: cannot create {}: {e}", dir.display()); }
            println!("determinism: RC_DUMP_FRAMES: frames {}..={} into {}", range.0, range.1, dir.display());
            app.insert_resource(FrameDump { dir, range, target, saved: Arc::default() }).add_systems(Update, dump_frame);
            return;
        }
        let (Some(frame), Some(path)) = (frame, path) else { return };
        app.insert_resource(FrameShot { path: path.into(), frame: frame.max(1), retry: Arc::default(), saved: Arc::default(), target })
            .add_systems(Update, frame_screenshot)
            .add_systems(Last, exit_before_capture.after(bevy::window::ExitSystems));
    }
}

/// Bevy's indirect mode (GPU preprocessing with culling) gives every instance of a batch its output slot
/// with an `atomicAdd` in `mesh_preprocess.wgsl`, so the order of instances inside one instanced draw
/// changes from frame to frame and run to run. Where instances of one mesh overlap at exactly equal depth
/// (coplanar decal-like mobys, instances placed at the same spot) the later one wins the depth test, so
/// the pixel flips. Without indirect drawing the CPU assigns the slots in bin order. Inserted as each
/// camera is spawned (Bevy supports it only from spawn).
fn no_indirect_drawing(add: On<Add, Camera3d>, mut commands: Commands) {
    commands.entity(add.entity).insert(NoIndirectDrawing);
}

fn advance_ticks(det: Res<Deterministic>, time: Res<Time<Virtual>>, mut ticks: ResMut<GameTicks>, mut frame: ResMut<FrameNumber>) {
    frame.0 += 1;
    ticks.0 = if det.0 {
        match tick_pattern() {
            Some(p) => ticks.0 + p[((frame.0 - 1) % p.len() as u64) as usize] as u64,
            None => frame.0,
        }
    } else {
        (time.elapsed().as_nanos() * TICK_HZ as u128 / 1_000_000_000) as u64
    };
}

/// `RC_DUMP_TICKS=n,m,…` (dev only; module docs): the game ticks of each update in frame-exact mode, cycled.
pub fn tick_pattern() -> Option<&'static [u32]> {
    static P: std::sync::OnceLock<Option<Vec<u32>>> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let v: Vec<u32> = env("RC_DUMP_TICKS")?.split(',').map(|t| t.trim().parse().ok()).collect::<Option<_>>()?;
        (!v.is_empty() && v.iter().any(|&n| n > 0)).then_some(v)
    })
    .as_deref()
}

/// Frame-exact mode with `RC_DUMP_TICKS`: this update's fixed steps (the time advances by as many ticks).
fn pattern_steps(frame: Res<FrameNumber>, mut strategy: ResMut<TimeUpdateStrategy>) {
    let Some(p) = tick_pattern() else { return };
    *strategy = TimeUpdateStrategy::FixedTimesteps(p[(frame.0 % p.len() as u64) as usize]);
}

/// Bevy sorts the blended (Transparent3d) phase by the view depth of each entity's origin (plus the
/// material's depth bias) with a stable sort over a retained map whose order is the order items were first
/// queued, i.e. the order their pipelines finished compiling on the async task pool. Items at equal depth
/// (every part entity of one tie / shrub / moby instance shares its transform) therefore drew in a
/// run-dependent order. Break those ties by the main-world entity, whose index follows the deterministic
/// spawn order (per instance, its parts in class order).
fn stable_transparent_order(mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>) {
    for phase in phases.values_mut() {
        phase.items.sort_by(|ka, a, kb, b| FloatOrd(a.distance).cmp(&FloatOrd(b.distance)).then_with(|| ka.1.cmp(&kb.1)));
    }
}

fn record_pipelines(cache: Res<PipelineCache>, status: Res<PipelineStatus>) {
    status.0[0].store(cache.pipelines().count() as u64, Ordering::Release);
    status.0[1].store(cache.waiting_pipelines().count() as u64, Ordering::Release);
}

/// See the module docs ("Monitor blips"): a window must not be despawned with a monitor entity.
fn unlink_monitor(add: On<Insert, OnMonitor>, mut commands: Commands) {
    commands.entity(add.entity).try_remove::<OnMonitor>();
}

/// H1: the run ends (window closed, monitor blip) before the capture was saved: exit status 2.
fn exit_before_capture(mut exits: MessageReader<AppExit>, shot: Res<FrameShot>, frame: Res<FrameNumber>) {
    if exits.read().next().is_some() && !shot.saved.load(Ordering::Acquire) {
        eprintln!("determinism: the app is exiting at frame {} before the frame-{} capture was saved; exit status 2", frame.0, shot.frame);
        std::process::exit(2);
    }
}

/// `RC_DUMP_FRAMES`: one capture per frame of the range, saved as `frame_NNNNN.png`; exit once the last is saved.
fn dump_frame(mut commands: Commands, dump: Res<FrameDump>, frame: Res<FrameNumber>, status: Res<PipelineStatus>) {
    let (a, b) = dump.range;
    let n = frame.0;
    if n < a || n > b { return; }
    let (total, waiting) = status.get();
    if waiting != 0 || total == 0 { eprintln!("determinism: RC_DUMP_FRAMES: frame {n} captured with {waiting} of {total} pipelines still compiling"); }
    let request = match &dump.target {
        Some(image) => Screenshot::image(image.clone()),
        None => Screenshot::primary_window(),
    };
    let (path, saved, count) = (dump.dir.join(format!("frame_{n:05}.png")), dump.saved.clone(), b - a + 1);
    commands.spawn(request).observe(move |shot: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
        match shot.image.clone().try_into_dynamic() {
            Ok(img) => {
                if let Err(e) = img.to_rgb8().save(&path) { eprintln!("cannot save {}: {e}", path.display()); }
            }
            Err(e) => eprintln!("cannot convert the frame-{n} capture: {e:?}"),
        }
        if saved.fetch_add(1, Ordering::AcqRel) + 1 == count {
            println!("determinism: RC_DUMP_FRAMES: {count} frames saved; exiting");
            exit.write(AppExit::Success);
        }
    });
}

fn frame_screenshot(
    mut commands: Commands,
    shot: Res<FrameShot>,
    frame: Res<FrameNumber>,
    status: Res<PipelineStatus>,
    mut done: Local<bool>,
    mut warned: Local<bool>,
) {
    if shot.retry.swap(false, Ordering::AcqRel) { *done = false; }
    if *done || frame.0 < shot.frame { return; }
    let (total, waiting) = status.get();
    if waiting != 0 || total == 0 {
        if !*warned {
            eprintln!("determinism: frame {} reached with {waiting} of {total} pipelines still compiling; waiting (raise RC_SCREENSHOT_FRAME)", shot.frame);
            *warned = true;
        }
        return;
    }
    *done = true;
    let (path, n, want, retry, saved) = (shot.path.clone(), frame.0, shot.frame, shot.retry.clone(), shot.saved.clone());
    let give_up = n >= want + MAX_RETRY_FRAMES;
    let request = match &shot.target {
        Some(image) => Screenshot::image(image.clone()),
        None => Screenshot::primary_window(),
    };
    commands.spawn(request).observe(
        move |shot: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
            let mut status = 0;
            match shot.image.clone().try_into_dynamic() {
                Ok(img) => {
                    let rgb = img.to_rgb8();
                    // An all-zero image is a frame that was never drawn (an occluded window: see the module docs);
                    // nothing the game draws is pure black edge to edge. Capture a later frame instead, for at most
                    // MAX_RETRY_FRAMES frames.
                    if rgb.as_raw().iter().all(|&b| b == 0) {
                        if !give_up {
                            eprintln!("determinism: frame {n} came back blank (window not presented?); capturing a later frame");
                            retry.store(true, Ordering::Release);
                            return;
                        }
                        eprintln!("determinism: WARNING frame {n} came back blank and {MAX_RETRY_FRAMES} frames of retries are used up; saving it anyway");
                    }
                    match rgb.save(&path) {
                        Ok(()) => println!("screenshot of frame {n} saved to {}", path.display()),
                        Err(e) => { eprintln!("cannot save screenshot to {}: {e}", path.display()); status = 1; }
                    }
                    if n != want {
                        eprintln!("determinism: WARNING the saved screenshot is frame {n}, not frame {want}; exit status 3");
                        status = status.max(3);
                    }
                }
                Err(e) => { eprintln!("cannot convert screenshot: {e:?}"); status = 1; }
            }
            saved.store(true, Ordering::Release);
            if status != 0 { std::process::exit(status); }
            exit.write(AppExit::Success);
        },
    );
}
