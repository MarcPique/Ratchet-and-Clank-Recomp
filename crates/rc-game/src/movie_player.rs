//! The PSS movie mode (game mode 1) as 60 Hz frames: `DialogStreamUpdate(n)` 0x2acf50 → `StartPssMovie` 0x2ad0c0 →
//! `MovieModeUpdate` 0x2ad498 (`fun_0023a3b8` → `readMpeg` 0x31a308) → `MovieExitToGameplay` 0x2ad2b8. Spec:
//! docs/plan/cutscenes.md §5. The timeline and the skip rule only; the decoding and the drawing are the engine's
//! (`rc-engine` `movie_render`, `rc-video`).
//!
//! | frames | what |
//! |---|---|
//! | 4 | `StartPssMovie`: sounds and music stopped, `FadeToBlack(4)`: black quads over the frozen last game image |
//! | 1 | the fade's closing vsync (black); mode 1 |
//! | 2 per picture | the movie: `readMpeg` starts the display and the audio together once two pictures are decoded and 4 KiB of audio are buffered (`startDisplay` + `audioDecStart`); the vblank handler 0x31b280 shows each decoded picture for one field pair (two vsyncs); the pad is polled every loop (skip) |
//! | 4 + 1 | `MovieExitToGameplay`: VRAM restored, `FadeToBlack(4)` over the last movie picture and its closing vsync |
//! | – | mode 0: `music_start_track(level track, 1, 0x400)`, the talker's dialogue refreshed (`FUN_0027b550(npc, rec, 1)`) |
//!
//! The other callers (docs/plan/progression.md `## media`): the page menu's post-actions 3 / 4 / 6 (0x28c990) fade
//! `ticks(16)` first (over the menu image) and set the replay mode 0x15eed8 (3: 2, any button skips; 4 / 6: 1, Start
//! alone; [`MoviePlayer::with_entry`]); `PlayMovieB(n)` 0x2ad050 = `mpegs[40 + n]` ([`MOVIE_B_BASE_NTSC`]), `PlayMovieC(n)` 0x2acfe8 =
//! `mpegs[70 + n]` with language 0 ([`MOVIE_C_BASE_NTSC`]); the title's attract movie `fun_001e9488(i)` (boot) =
//! `mpegs[80 + i]`, replay 2, entry fade `FadeToBlack(ticks(12))`, language 0, no music restart
//! ([`MOVIE_ATTRACT_BASE_NTSC`], [`ATTRACT_FADE`]).
//!
//! The vsync accounting of the two blocking fades is read from the code (as the scene player's), not traced [M].

use crate::pad::{button, PadState};
use crate::scene_player::fade_to_black_coverage;

/// `FadeToBlack(4)` on entry and on exit.
pub const FADE_FRAMES: u32 = 4;
/// In-level movie `n` is `mpegs[MOVIE_BASE_NTSC + n]` (`DialogStreamUpdate`: TOC 0x139388 + 8n; PAL 0x139420 = 21 + n).
pub const MOVIE_BASE_NTSC: i32 = 2;
pub const MOVIE_BASE_PAL: i32 = 21;
/// `PlayMovieB(n)` 0x2ad050: `mpegs[40 + n]` (TOC 0x1394b8 + 8n; PAL 0x139518 = 52 + n): the transitions table (the
/// Cinematics page's replays, the ending `PlayMovieB(11)`).
pub const MOVIE_B_BASE_NTSC: i32 = 40;
pub const MOVIE_B_BASE_PAL: i32 = 52;
/// `PlayMovieC(n)` 0x2acfe8: `mpegs[70 + n]` (TOC 0x1395a8; PAL 0x1395d0 = 75 + n), language 0: the extras.
pub const MOVIE_C_BASE_NTSC: i32 = 70;
pub const MOVIE_C_BASE_PAL: i32 = 75;
/// Boot `fun_001e9488(i)`: `mpegs[80 + i]` (TOC 0x1395f8; PAL 0x139618 = 84 + i): the title's attract loop.
pub const MOVIE_ATTRACT_BASE_NTSC: i32 = 80;
pub const MOVIE_ATTRACT_BASE_PAL: i32 = 84;
/// The attract movie's entry fade `FadeToBlack(ticks(12))`.
pub const ATTRACT_FADE: u32 = 12;
/// The page menu's fade before a replay (`FadeToBlack(ticks(16))`, post-actions 3 / 4 / 5 / 6 / 7).
pub const MENU_FADE: u32 = 16;
/// Audio samples (48 kHz) per 60 Hz vsync: the movie's clock advances by this much per frame.
pub const SAMPLES_PER_VSYNC: u64 = 800;
/// The output rate.
pub const RATE: u64 = 48_000;

/// What the skip rule reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MovieContext {
    /// 0x15eed8: 0 normal play, 2 replaying from the menu (any button skips), −1 never skippable.
    pub replay: i32,
    /// 0x15eea0.
    pub game_beaten: bool,
    /// 0x15ee20.
    pub completes: i32,
    /// 0x15ed84.
    pub level: i32,
}

/// `readMpeg`'s skip test, run once per pad read: in replay mode 2 any pressed button; Start alone once the game
/// is beaten or completed, when replaying, or on level 0; otherwise Start pressed with L1 L2 R1 R2 held; never
/// while 0x15eed8 = −1.
pub fn skip_requested(ctx: &MovieContext, pad: &PadState) -> bool {
    if ctx.replay == -1 { return false; }
    let start = pad.pressed & button::START != 0;
    let normal = !ctx.game_beaten && ctx.completes == 0 && ctx.replay == 0 && ctx.level > 0;
    (ctx.replay == 2 && pad.pressed != 0) || (!normal && start) || (start && pad.held & 0xf == 0xf)
}

/// Where the movie mode is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// `FadeToBlack(4)` step k over the frozen game image (k = 4: the closing black vsync).
    FadeIn(u32),
    /// The movie, vsync k since the display started.
    Playing(u32),
    /// `FadeToBlack(4)` step k over the last movie picture (k = 4: the closing black vsync).
    FadeOut(u32),
    /// Back to mode 0 (the frame after the closing vsync).
    Done,
}

/// One frame's output.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovieFrame {
    pub phase: Phase,
    /// Black coverage to draw over this frame's image (the game image in `FadeIn`, the movie picture otherwise).
    pub black: f32,
    /// The movie picture shown (index in display order) while playing / fading out.
    pub picture: Option<u32>,
}

/// The movie mode's frame sequence. The engine calls [`MoviePlayer::tick`] once per 60 Hz frame with the pad, the
/// number of pictures (once known) and the audio output latency.
///
/// **Clock.** The audio is the clock: vsync k of the display has fed `800·k` samples of the movie's 48 kHz audio,
/// and the picture on screen is the one whose time span holds the sample being heard, `(800·k − latency)·fps /
/// 48000` (latency = samples queued for the device; 0 in frame-exact runs). At 30 fps that is picture `k / 2`, the
/// vblank handler's one picture per field pair; 25 fps (the PAL copies) and 29.97 fps (extras 70–72) follow the
/// same rule. Video and audio are both derived from the vsync count, so they cannot drift apart; the device's own
/// clock is absorbed by the audio ring (crate `rc-engine` `audio_out`).
#[derive(Clone, Debug)]
pub struct MoviePlayer {
    pub ctx: MovieContext,
    phase: Phase,
    /// The last image is already black (the page menu's `FadeToBlack(ticks(16))` of post-actions 3 / 4 / 6 ran before:
    /// the entry fade draws black), and the entry fade's length (`StartPssMovie`'s `FadeToBlack(4)`; the title's attract
    /// movies `fun_001e9488`: `FadeToBlack(ticks(12))`).
    black_entry: bool,
    fade_in: u32,
    /// Frames per second as a fraction (30/1 until [`MoviePlayer::set_rate`]).
    fps: (u32, u32),
    /// The last picture shown.
    last_picture: Option<u32>,
    pub skipped: bool,
}

impl MoviePlayer {
    pub fn new(ctx: MovieContext) -> Self { MoviePlayer { ctx, phase: Phase::FadeIn(0), black_entry: false, fade_in: FADE_FRAMES, fps: (30, 1), last_picture: None, skipped: false } }

    /// The entry fade over an image the caller already faded to black (`black_entry`), and its length (default
    /// [`FADE_FRAMES`]).
    pub fn with_entry(mut self, black_entry: bool, fade_in: u32) -> Self {
        self.black_entry = black_entry;
        self.fade_in = fade_in.max(1);
        self
    }

    /// The stream's frame rate (the sequence header's).
    pub fn set_rate(&mut self, num: u32, den: u32) { self.fps = (num.max(1), den.max(1)); }

    pub fn phase(&self) -> Phase { self.phase }

    pub fn done(&self) -> bool { self.phase == Phase::Done }

    /// The picture shown at vsync `k` of the display with `latency` samples still queued for the device.
    pub fn picture_at(&self, k: u32, latency: u32) -> u32 {
        let heard = (k as u64 * SAMPLES_PER_VSYNC).saturating_sub(latency as u64);
        (heard * self.fps.0 as u64 / (RATE * self.fps.1 as u64)) as u32
    }

    /// The picture this vsync will show when playing (for the engine to fetch it before [`MoviePlayer::tick`]).
    pub fn wanted_picture(&self, latency: u32) -> Option<u32> {
        match self.phase {
            Phase::Playing(k) => Some(self.picture_at(k, latency)),
            _ => None,
        }
    }

    /// One vsync. `pictures`: how many pictures the stream has (None while unknown: the decoder has not reached the
    /// end yet); `latency`: samples queued for the audio device. Returns the frame to show.
    pub fn tick(&mut self, pad: &PadState, pictures: Option<u32>, latency: u32) -> MovieFrame {
        let fi = self.fade_in;
        let frame = match self.phase {
            Phase::FadeIn(_) if self.black_entry => MovieFrame { phase: self.phase, black: 1.0, picture: None },
            Phase::FadeIn(k) if k < fi => MovieFrame { phase: self.phase, black: fade_to_black_coverage(fi, k), picture: None },
            Phase::FadeIn(_) => MovieFrame { phase: self.phase, black: 1.0, picture: None },
            Phase::Playing(k) => {
                let pic = self.picture_at(k, latency);
                let over = pictures.is_some_and(|n| pic >= n);
                let skip = !over && skip_requested(&self.ctx, pad);
                if over || skip {
                    // The decoder is flushed and the display stopped: the last picture stays up for the exit fade.
                    self.skipped = skip;
                    self.phase = Phase::FadeOut(0);
                    return self.tick(pad, pictures, latency);
                }
                self.last_picture = Some(pic);
                MovieFrame { phase: self.phase, black: 0.0, picture: Some(pic) }
            }
            Phase::FadeOut(k) if k < FADE_FRAMES => MovieFrame { phase: self.phase, black: fade_to_black_coverage(FADE_FRAMES, k), picture: self.last_picture },
            Phase::FadeOut(_) => MovieFrame { phase: self.phase, black: 1.0, picture: self.last_picture },
            Phase::Done => MovieFrame { phase: Phase::Done, black: 0.0, picture: None },
        };
        self.phase = match self.phase {
            Phase::FadeIn(k) if k < fi => Phase::FadeIn(k + 1),
            Phase::FadeIn(_) => Phase::Playing(0),
            Phase::Playing(k) => Phase::Playing(k + 1),
            Phase::FadeOut(k) if k < FADE_FRAMES => Phase::FadeOut(k + 1),
            Phase::FadeOut(_) | Phase::Done => Phase::Done,
        };
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(held: u32, pressed: u32) -> PadState { PadState { held, pressed, ..Default::default() } }

    const SHOULDERS: u32 = button::L1 | button::L2 | button::R1 | button::R2;

    #[test]
    fn skip_rule() {
        let normal = MovieContext { replay: 0, game_beaten: false, completes: 0, level: 1 };
        // Normal play: Start alone does nothing; Start pressed with the four shoulders held skips.
        assert!(!skip_requested(&normal, &pad(button::START, button::START)));
        assert!(skip_requested(&normal, &pad(SHOULDERS | button::START, button::START)));
        assert!(!skip_requested(&normal, &pad(SHOULDERS | button::START, 0)), "Start must be a new press");
        assert!(!skip_requested(&normal, &pad(button::L1 | button::L2 | button::R1 | button::START, button::START)));
        // Start alone: beaten, completed, replay (≠ −1), level 0.
        for ctx in [MovieContext { game_beaten: true, ..normal }, MovieContext { completes: 1, ..normal }, MovieContext { replay: 1, ..normal }, MovieContext { level: 0, ..normal }] {
            assert!(skip_requested(&ctx, &pad(button::START, button::START)), "{ctx:?}");
            assert!(!skip_requested(&ctx, &pad(button::CROSS, button::CROSS)), "{ctx:?}");
        }
        // Replay mode 2: any button.
        assert!(skip_requested(&MovieContext { replay: 2, ..normal }, &pad(button::CROSS, button::CROSS)));
        // −1: never.
        assert!(!skip_requested(&MovieContext { replay: -1, ..normal }, &pad(SHOULDERS | button::START, button::START)));
    }

    #[test]
    fn timeline_played_through() {
        let mut p = MoviePlayer::new(MovieContext { level: 1, ..Default::default() });
        let idle = PadState::default();
        let mut frames = Vec::new();
        while !p.done() { frames.push(p.tick(&idle, Some(3), 0)); }
        // 4 fade steps + closing black, 3 pictures × 2 vsyncs, 4 fade steps + closing black.
        assert_eq!(frames.len(), 5 + 6 + 5);
        assert!(frames[..4].windows(2).all(|w| w[0].black < w[1].black));
        assert_eq!(frames[3].black, 1.0);
        assert_eq!(frames[4].black, 1.0);
        let shown: Vec<Option<u32>> = frames[5..11].iter().map(|f| f.picture).collect();
        assert_eq!(shown, [Some(0), Some(0), Some(1), Some(1), Some(2), Some(2)]);
        assert!(frames[11..].iter().all(|f| f.picture == Some(2)));
        assert_eq!(frames[15].black, 1.0);
        assert!(!p.skipped);
    }

    /// 30 fps: one picture per two vsyncs; 25 fps: 2.4 vsyncs per picture; 29.97 fps: 30000 pictures in 60060 vsyncs;
    /// a 50 ms output latency (2400 samples)
    /// holds the picture back by 1.5 pictures at 30 fps.
    #[test]
    fn audio_clock() {
        let mut p = MoviePlayer::new(MovieContext::default());
        assert_eq!((0..8).map(|k| p.picture_at(k, 0)).collect::<Vec<_>>(), [0, 0, 1, 1, 2, 2, 3, 3]);
        assert_eq!(p.picture_at(10, 2400), 3);
        assert_eq!(p.picture_at(1, 2400), 0);
        p.set_rate(25, 1);
        assert_eq!((0..13).map(|k| p.picture_at(k, 0)).collect::<Vec<_>>(), [0, 0, 0, 1, 1, 2, 2, 2, 3, 3, 4, 4, 5]);
        p.set_rate(30000, 1001);
        assert_eq!(p.picture_at(60 * 1001, 0), 30000);
    }

    #[test]
    fn timeline_skipped() {
        let mut p = MoviePlayer::new(MovieContext { level: 1, ..Default::default() });
        let idle = PadState::default();
        for _ in 0..5 + 7 { p.tick(&idle, None, 0); }
        assert_eq!(p.phase(), Phase::Playing(7));
        // The skip vsync already shows the exit fade over picture 3.
        let f = p.tick(&pad(SHOULDERS | button::START, button::START), None, 0);
        assert_eq!((f.phase, f.picture), (Phase::FadeOut(0), Some(3)));
        assert!(p.skipped);
        let mut n = 1;
        while !p.done() { p.tick(&idle, None, 0); n += 1; }
        assert_eq!(n, 5);
    }
}
