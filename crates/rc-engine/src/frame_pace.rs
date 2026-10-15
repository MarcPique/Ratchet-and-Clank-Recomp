//! **Port-only: frame pacing.** The game runs at 60 Hz (one logic tick per NTSC field). The frame limit is
//! configurable from Advanced Options. Above 60 FPS, the camera interpolates between 60 Hz
//! simulation views; game state and object animations still advance at 60 Hz. At 30 FPS, a frame
//! may advance two game ticks.
//!
//! * **Ticks** (`First`, before Bevy's time update): each frame advances `TimeUpdateStrategy::FixedTimesteps(n)`
//!   ticks, `n` from the real time since the last frame start counted in ticks with a remainder that carries over:
//!   at 30/60 FPS the elapsed ticks are rounded to keep the original cadence; above 60 FPS they are floored
//!   so extra render frames do not accelerate the game. At most [`MAX_STEPS`] game ticks run in one frame.
//! * **Limiter** (`Last`): the frame waits for the configured deadline. At the default 60 FPS with V-Sync on,
//!   the period follows the primary monitor: at ~60 Hz none (V-Sync paces the loop); at a multiple of ~60 Hz,
//!   that many refreshes; otherwise 1/60 s. Other limits use their own deadline, and Unlimited waits for none.
//!   The wait sleeps to 0.3 ms before the deadline, then yields until it (the yield loop costs CPU: keep it short).
//!
//! `RC_FRAME_CAP=0` (or `RC_NOVSYNC=1`, which measures headroom) turns the limiter off; frame-exact runs
//! (`crate::determinism`) keep their own one tick per update, without the limiter unless `RC_FRAME_CAP=1`.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::{Monitor, PrimaryMonitor};
use std::time::{Duration, Instant};

/// Ticks a frame may run at most (a longer stall's time is dropped).
pub const MAX_STEPS: u32 = 4;
/// The tick period (1/60 s).
const TICK: f64 = 1.0 / crate::determinism::TICK_HZ;

pub struct FramePacePlugin;

#[derive(Resource)]
struct Pace {
    /// The last frame's start (the ticks' clock).
    last: Option<Instant>,
    /// Elapsed ticks not yet run.
    acc: f64,
    /// The limiter's next deadline and its period.
    next: Option<Instant>,
    period: Option<Duration>,
}

/// Fraction of the current 60 Hz tick elapsed at this render frame. Render-only interpolation
/// may use this without changing how many simulation ticks the game runs.
#[derive(Resource, Clone, Copy, Default)]
pub struct TickFraction(pub f32);

fn env_is(name: &str, v: &str) -> bool { std::env::var(name).is_ok_and(|x| x.trim() == v) }

impl Plugin for FramePacePlugin {
    fn build(&self, app: &mut App) {
        if crate::determinism::deterministic() {
            // Frame-exact runs keep their one tick per update; `RC_FRAME_CAP=1` still limits them to 60 frames a second
            // (a capture run otherwise renders as fast as the machine allows).
            if env_is("RC_FRAME_CAP", "1") {
                app.insert_resource(Pace { last: None, acc: 0.0, next: None, period: None }).add_systems(Last, wait);
            }
            return;
        }
        app.insert_resource(Pace { last: None, acc: 0.0, next: None, period: None })
            .init_resource::<TickFraction>()
            .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
            .add_systems(First, steps.before(bevy::time::TimeSystems))
            .add_systems(Last, wait);
    }
}

/// This frame's ticks (module docs).
fn steps(mut pace: ResMut<Pace>, settings: Res<crate::display::DisplaySettings>, mut strategy: ResMut<TimeUpdateStrategy>, mut fraction: ResMut<TickFraction>) {
    let now = Instant::now();
    let Some(last) = pace.last.replace(now) else {
        *strategy = TimeUpdateStrategy::FixedTimesteps(1);
        fraction.0 = 1.0;
        return;
    };
    pace.acc += (now - last).as_secs_f64() / TICK;
    // At 30/60 FPS keep the original one-tick rhythm (or two at 30). Above 60, render
    // frames may repeat; only elapsed real time advances the 60 Hz game simulation.
    let game_rate = settings.frame_limit.fps().is_some_and(|n| n <= 60);
    let n = if game_rate {
        let mut n = pace.acc.round().max(0.0);
        if n == 0.0 && pace.acc > 0.25 { n = 1.0; }
        n
    } else {
        pace.acc.floor()
    };
    let n = n.min(MAX_STEPS as f64);
    pace.acc -= n;
    pace.acc = if game_rate { pace.acc.clamp(-0.5, 0.5) } else { pace.acc.clamp(0.0, 1.0) };
    fraction.0 = if game_rate { 1.0 } else { pace.acc as f32 };
    *strategy = TimeUpdateStrategy::FixedTimesteps(n as u32);
}

/// The limiter's period for a monitor of `mhz` millihertz (module docs): None at ~60 Hz.
fn period_for(mhz: Option<u32>, cap: crate::display::FrameLimit, vsync: bool) -> Option<Duration> {
    let Some(fps) = cap.fps() else { return None };
    if !vsync { return Some(Duration::from_secs_f64(1.0 / fps as f64)); }
    if fps != 60 { return Some(Duration::from_secs_f64(1.0 / fps as f64)); }
    let hz = mhz.map(|m| m as f64 / 1000.0).filter(|&h| h > 1.0);
    let Some(hz) = hz else { return Some(Duration::from_secs_f64(TICK)) };
    let k = (hz / 60.0).round();
    if k >= 1.0 && (hz / k - 60.0).abs() < 1.5 {
        if k == 1.0 { return None; }
        return Some(Duration::from_secs_f64(k / hz));
    }
    Some(Duration::from_secs_f64(TICK))
}

/// The limiter (module docs).
fn wait(mut pace: ResMut<Pace>, settings: Res<crate::display::DisplaySettings>, monitors: Query<&Monitor, With<PrimaryMonitor>>) {
    let period = period_for(monitors.iter().next().and_then(|m| m.refresh_rate_millihertz), settings.frame_limit, settings.vsync);
    if period != pace.period {
        pace.period = period;
        pace.next = None;
    }
    let Some(period) = period else { return };
    let now = Instant::now();
    // A deadline more than a period behind (a stall) starts the rhythm again from now.
    let deadline = match pace.next {
        Some(d) if d + period > now => d,
        _ => now,
    };
    let spin = Duration::from_micros(300);
    if deadline > now + spin { std::thread::sleep(deadline - now - spin); }
    while Instant::now() < deadline { std::thread::yield_now(); }
    pace.next = Some(deadline + period);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::FrameLimit;

    #[test]
    fn the_period_follows_the_display() {
        assert_eq!(period_for(Some(60_000), FrameLimit::F60, true), None);
        assert_eq!(period_for(Some(59_940), FrameLimit::F60, true), None);
        let p = |mhz| period_for(Some(mhz), FrameLimit::F60, true).unwrap().as_secs_f64();
        assert!((p(120_000) - 2.0 / 120.0).abs() < 1e-9);
        assert!((p(119_880) - 2.0 / 119.88).abs() < 1e-9);
        assert!((p(240_000) - 4.0 / 240.0).abs() < 1e-9);
        assert!((p(144_000) - TICK).abs() < 1e-9);
        assert!((period_for(None, FrameLimit::F60, true).unwrap().as_secs_f64() - TICK).abs() < 1e-9);
        assert_eq!(period_for(Some(120_000), FrameLimit::Unlimited, true), None);
        assert!((period_for(Some(120_000), FrameLimit::F30, true).unwrap().as_secs_f64() - 1.0 / 30.0).abs() < 1e-9);
        assert!((period_for(Some(144_000), FrameLimit::F144, true).unwrap().as_secs_f64() - 1.0 / 144.0).abs() < 1e-9);
        assert!((period_for(Some(60_000), FrameLimit::F60, false).unwrap().as_secs_f64() - TICK).abs() < 1e-9);
    }
}
