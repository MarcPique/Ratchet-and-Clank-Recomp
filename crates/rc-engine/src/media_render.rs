//! Front-end media in the engine (docs/plan/progression.md `## media`): the credits slideshow (game mode 7,
//! `rc_game::slideshow`), the page menu's fade before its movie / scene / slideshow post-actions, the requests the
//! moby loop's classes make outside gameplay (`EnterSlideshowMode`, `PlayMovieB`, `EnterMenuMode`), and the level exit
//! the menus ask for (Quit Game, the end page's challenge restart).
//!
//! **Requests.** [`take_class_requests`] takes the classes' `EngineRequest::{Slideshow, MovieB, EnterMenu}` right after
//! the tick (crate::menu_render, the same frame the game switches mode) and as a fallback where the other requests are
//! taken (crate::scene_render): the slideshow and the menu go to [`request_slideshow`] / [`request_menu`] (run by
//! crate::menu_render), the movie to crate::movie_render ([`crate::movie_render::MovieRequest::movie_b`]).
//!
//! **Slideshow frames** ([`Slides::frame`], one per main-loop frame of mode 7, run by crate::menu_render with the menu
//! assets): `EnterSlideshowMode`'s `FadeToBlack(8)` (over the last image: the frozen world, or black after the page
//! menu's own fade), then `SlideshowModeUpdate` + `SlideshowModeRender` (the picture as a streamed image of the 2D
//! pass, `MenuDraw::Image` of a picture the port composes; the black fade; the text lines), then the exit's
//! `FadeToBlack(12)` over the last frame and mode 0. The pictures are read from `global/credits_images_ntsc/NNN.bin`
//! (raw 512×416 PSMCT32) when the mode asks; the dissolve copies the columns into the shown copy.
//!
//! **Level exit** ([`request_level_exit`]): `0x15f5c0 = dest, 0x15f570 = 1` (Quit Game: −1, the end page's challenge
//! restart: 0): the level loop ends and `DoSpaceTransition` runs, as `EngineRequest::LeaveLevel` (the engine's one level
//! change).
//!
//! Not modelled: the IOP stream's waits; the page menu's fade is drawn over the menu image, the slideshow's entry fade
//! after a menu fade stays black (the game fades the black image again: the same picture).

use rc_game::menus::pause::media::EndChoice;
use rc_game::menus::{ImageSrc, MenuAssets, MenuDraw};
use rc_game::scene_player::fade_to_black_coverage;
use rc_game::slideshow::{self, Slideshow, Tables};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A front-end media request for crate::menu_render.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaRequest {
    /// `EnterSlideshowMode`; `black_entry`: the page menu's `FadeToBlack(ticks(16))` already blackened the image.
    Slideshow { black_entry: bool },
    /// `EnterMenuMode(kind)` from a class.
    Menu { kind: i32 },
}

static QUEUE: Mutex<VecDeque<MediaRequest>> = Mutex::new(VecDeque::new());

fn queue() -> std::sync::MutexGuard<'static, VecDeque<MediaRequest>> { QUEUE.lock().unwrap_or_else(|e| e.into_inner()) }

/// `EnterSlideshowMode` 0x2ad558 (the class's or the Credits entry's).
pub fn request_slideshow(black_entry: bool) { queue().push_back(MediaRequest::Slideshow { black_entry }); }

/// `EnterMenuMode(kind)` 0x28bf50 from a class (the boss 1422's 0x21).
pub fn request_menu(kind: i32) { queue().push_back(MediaRequest::Menu { kind }); }

/// The next pending request.
pub(crate) fn take() -> Option<MediaRequest> { queue().pop_front() }

/// `0x15f5c0 = dest, 0x15f570 = 1` (`FUN_002a29a0(dest)`; Quit Game's −1, the end page's challenge restart 0): the level
/// loop ends for `dest`, through the engine's one level change (`rc_game::cinematic::EngineRequest::LeaveLevel`).
pub fn request_level_exit(play: &mut crate::gameplay::Play, dest: i32) {
    println!("media: level exit: 0x15f5c0 = {dest}, 0x15f570 = 1 (DoSpaceTransition)");
    play.svc.cinematic.requests.push(rc_game::cinematic::EngineRequest::LeaveLevel { dest });
}

static END_CHOICE: Mutex<Option<EndChoice>> = Mutex::new(None);

/// The end-of-game page's choice (`rc_game::menus::pause::media::EndChoice`): the timewarp's `memcard_RestoreGame` /
/// `memcard_Save` and the challenge mode's `fun_00226b08(−1)` belong to the `saves` lane, which takes it.
pub fn request_end_choice(c: EndChoice) {
    println!("media: end page: {c:?} (the saves lane's restore / challenge mode)");
    *END_CHOICE.lock().unwrap_or_else(|e| e.into_inner()) = Some(c);
}

/// The pending end-page choice (the `saves` lane takes it).
#[allow(dead_code)] // its consumer is the saves lane (docs/plan/progression.md `## media`)
pub fn take_end_choice() -> Option<EndChoice> { END_CHOICE.lock().unwrap_or_else(|e| e.into_inner()).take() }

/// The classes' front-end requests of the last tick out of `play`'s request list (the others stay).
pub fn take_class_requests(play: &mut crate::gameplay::Play) {
    use rc_game::cinematic::EngineRequest as R;
    play.svc.cinematic.requests.retain(|r| match *r {
        R::Slideshow => {
            println!("media: EnterSlideshowMode (a class): mode 7, FadeToBlack(8)");
            request_slideshow(false);
            false
        }
        R::MovieB { movie } => {
            println!("media: PlayMovieB({movie}) (a class): mpegs[{}]", rc_game::movie_player::MOVIE_B_BASE_NTSC + movie);
            crate::movie_render::request(crate::movie_render::MovieRequest::movie_b(movie));
            false
        }
        R::EnterMenu { kind } => {
            println!("media: EnterMenuMode({kind:#x}) (a class)");
            request_menu(kind);
            false
        }
        _ => true,
    });
}

/// Where the slideshow is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlidePhase {
    /// `EnterSlideshowMode`'s `FadeToBlack(8)`, step k.
    Enter(u32),
    /// Mode 7.
    Run,
    /// `FUN_002ad6f0`'s `FadeToBlack(12)`, step k, over the last frame.
    Exit(u32),
    /// Back to mode 0 (the frame after the last fade step).
    Done,
}

/// The slideshow runtime: the mode's state, its two picture buffers and the last frame's draws.
pub struct Slides {
    pub show: Slideshow,
    pub phase: SlidePhase,
    black_entry: bool,
    /// The shown (0x15f628) and the loading (0x15f62c) pictures, RGBA 512×416.
    shown: Vec<u8>,
    loading: Vec<u8>,
    /// The shown picture as the 2D pass's image (its key changes with every dissolve step).
    image: Option<ImageSrc>,
    key: u64,
    /// The last mode-7 frame's draws (the exit fade covers them).
    last: Vec<MenuDraw>,
    pub frames: u32,
}

/// The pictures' key base (`ImageSrc::Pixels` keys share one space with the map's compositions).
const KEY_BASE: u64 = 0x534c_4944_0000_0000;

fn read_picture(n: i32) -> Vec<u8> {
    let root = crate::level_load::extracted_root();
    let name = format!("global/credits_images_ntsc/{n:03}.bin");
    match crate::disc_source::read(&root, &name) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("media: {name}: {e:#}; a black picture");
            vec![0; slideshow::WIDTH * slideshow::HEIGHT * 4]
        }
    }
}

impl Slides {
    /// `EnterSlideshowMode`: the state, picture 0 shown, picture 1 loading (the port is the US build).
    pub fn start(black_entry: bool) -> Slides {
        let (a, b) = Slideshow::first_loads();
        let mut s = Slides {
            show: Slideshow::new(true),
            phase: SlidePhase::Enter(0),
            black_entry,
            shown: read_picture(a),
            loading: read_picture(b),
            image: None,
            key: KEY_BASE,
            last: Vec::new(),
            frames: 0,
        };
        s.publish();
        s
    }

    /// The shown buffer as a new image (alpha as on disc: 0x80, opaque).
    fn publish(&mut self) {
        self.key += 1;
        let n = slideshow::WIDTH * slideshow::HEIGHT * 4;
        let mut rgba = self.shown.clone();
        rgba.resize(n, 0);
        let tex = rc_formats::texture::Texture { width: slideshow::WIDTH as u32, height: slideshow::HEIGHT as u32, rgba };
        self.image = Some(ImageSrc::Pixels { key: self.key, tex: Arc::new(tex) });
    }

    /// One main-loop frame of mode 7: its draws into `out`. `start` = Start newly pressed (0x13cae4 & 0x800).
    pub fn frame(&mut self, tables: &Tables, start: bool, assets: &MenuAssets, out: &mut Vec<MenuDraw>) {
        self.frames += 1;
        let black = |c: f32, out: &mut Vec<MenuDraw>| {
            let a = (c.clamp(0.0, 1.0) * 128.0) as u32;
            out.push(MenuDraw::Rect { x0: 1, y0: 1, x1: slideshow::WIDTH as i32 + 1, y1: slideshow::HEIGHT as i32 + 1, rgba: a << 24 });
        };
        match self.phase {
            SlidePhase::Enter(k) => {
                let c = if self.black_entry { 1.0 } else { fade_to_black_coverage(slideshow::ENTER_FADE, k) };
                black(c, out);
                self.phase = if k + 1 < slideshow::ENTER_FADE { SlidePhase::Enter(k + 1) } else { SlidePhase::Run };
            }
            SlidePhase::Run => {
                let o = self.show.tick(tables, start);
                if let Some(c) = o.dissolve {
                    slideshow::dissolve(&mut self.shown, &self.loading, c);
                    self.publish();
                }
                if let Some(n) = o.load { self.loading = read_picture(n); }
                if o.exit {
                    println!("media: slideshow {} at time {} (picture {}); FadeToBlack(12), mode 0", if o.skipped { "skipped (Start)" } else { "ended" }, self.show.time, self.show.index);
                    out.extend(self.last.iter().cloned());
                    black(fade_to_black_coverage(slideshow::EXIT_FADE, 0), out);
                    self.phase = SlidePhase::Exit(1);
                    return;
                }
                // SlideshowModeRender (not on the frame the mode was entered: 0x15f5d8; the entry fade covered it).
                let mut d = Vec::new();
                self.show.draw(tables, assets, self.image.clone(), &mut d);
                self.last = d.clone();
                out.extend(d);
            }
            SlidePhase::Exit(k) => {
                out.extend(self.last.iter().cloned());
                black(fade_to_black_coverage(slideshow::EXIT_FADE, k.min(slideshow::EXIT_FADE - 1)), out);
                self.phase = if k + 1 < slideshow::EXIT_FADE { SlidePhase::Exit(k + 1) } else { SlidePhase::Done };
            }
            SlidePhase::Done => {}
        }
    }

    pub fn done(&self) -> bool { self.phase == SlidePhase::Done }
}

/// The page menu's `FadeToBlack(ticks(16))` before a movie / scene / slideshow post-action (0x28c990), over the menu
/// image: step k of [`rc_game::movie_player::MENU_FADE`].
pub fn menu_fade_draws(k: u32, out: &mut Vec<MenuDraw>) {
    out.push(MenuDraw::Snapshot);
    out.push(MenuDraw::Darken { alpha: rc_game::menus::pause::DARKEN });
    let c = fade_to_black_coverage(rc_game::movie_player::MENU_FADE, k.min(rc_game::movie_player::MENU_FADE - 1));
    let a = (c.clamp(0.0, 1.0) * 128.0) as u32;
    out.push(MenuDraw::Rect { x0: 1, y0: 1, x1: slideshow::WIDTH as i32 + 1, y1: slideshow::HEIGHT as i32 + 1, rgba: a << 24 });
}
