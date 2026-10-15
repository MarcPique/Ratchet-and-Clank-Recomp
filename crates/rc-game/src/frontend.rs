//! The front end: the boot program's flow from power-on to a level (boot `startlevel` 0x1e9658 and
//! `transition_do_transition` 0x1eb798 with its update `fun_001eb0a8` and draw `transition_default_draw` 0x1eb410).
//! Spec: docs/plan/progression.md `## saves` ("Boot → title → main menu").
//!
//! The boot game mode (0x15f604; the levels' 0x15f5c4) is 0 on the title, 3 in the main menu (the page menu, kind 0x2d:
//! New Game / Load Game / Options, the same records every level overlay carries at L01 0x1b8b48), 4 in the card dialog
//! ([`crate::menus::freeze`]). The title runs until a page asks for a level (`initialize_global_state_entry(level)`, the
//! L01 `FUN_002a29a0`: 0x15f600 = level, 0x15f5b0 = 1), then the boot loads it through `DoSpaceTransition` (the travel
//! lane's level change; 0x13e05a = 1 runs the new game's story trip).
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | 0x1e9658 | 0x15f5e8 = 1 (the front end flag the card dialog reads) | [`FrontEnd::new`] (`MemCard::front_end`, set by the engine) |
//! | 0x1e9658 card check | `fun_00209168` every vsync: 0 ok, 1 no / not a PS2 card, 2 no save folder and < 350 KB; while ≠ 0 the warning picture (IRX lump +0x10 / +0x40 + 8·language) until it is 0 or after 10 vsyncs any button; then `FadeToBlack(10)` if one was shown | [`Phase::CardCheck`] (`CardFs::boot_check`) |
//! | 0x1e9658 logos | the PSS `mpegs[0]` (PAL [1]), `FadeToBlack(ticks(18))` | [`Phase::Logos`] (the engine plays it) |
//! | 0x1e9658 still | the IRX still (+0x00 NTSC) on screen while the sound bank and the title world load | [`Phase::Still`] |
//! | 0x1e9658 sound bank, music globals 0x15f630 / 0x186100 | the front end's sounds | the engine's audio (n/a here) |
//! | 0x1eb798 entry | level 0x15ed84 = 0, `load_and_initialize_level_chunk` (the template restored), the title world (`transition_load_wad`), the HUD banks; the still stays ≥ ticks(180) since the clock start; sound reset; `rand() % 4` the first attract movie | [`Phase::Still`], [`FrontEnd::new`] (the reset is the engine's) |
//! | 0x1eb798 loop | per vsync: `memcard_update_state`, the card monitor, `UpdatePad`, the voice slots, `fun_001eb0a8`, the draw; in mode 0 a frame counter: at ticks(1500) the attract movie `fun_001e9488(i)` (i cycles 0..3), the fade 0x15f43c = 1.0, the camera chunk 0 again, 0x15ef50 / 0x15ef54 / 0x15ef58 = 0; any other mode clears the counter | [`FrontEnd::frame`] (the card's two calls run in the engine's card frame) |
//! | 0x1eb798 PAL switch | 0x16034c ≠ 0x15ed80: the video mode switched | n/a: NTSC only |
//! | 0x1eb798 exit | 0x15f5b0 set: the loop ends (0x15ee74 / 0x15ee78 VRAM reset) | [`FrontOut::exit`] |
//! | 0x1e9488 | the attract movie: stops sounds and music, `FadeToBlack(ticks(12))`, waits for the card, plays `mpegs[80 + i]` (PAL 84) with replay mode 2, `FadeToBlack(4)`, mode 0 | [`FrontOut::attract`] (`rc-engine` `movie_render::play_attract`) |
//! | 0x1eb0a8 | the fade 0x15f43c −= 0.0625 (≥ 0); the title world's camera chunk (`parse_space_scene_chunk` every 0x60 frames) and its mobys | the fade: ported; the world: [`crate::travel::title`] (`rc-engine` `title_world`) |
//! | 0x1eb0a8 mode 0 | counter 0x15ef58 + 1; past ticks(60) the logo alpha 0x15ef50 + 1 (≤ 0x40); past ticks(120) the PRESS START alpha 0x15ef54 = `(int)(cos(((counter − ticks(120)) % 60)·0.10471976 − π)·32) + 0x60`; Start or ✕ (0x13cae4 & 0x840): `initialize_transfer_command` (the page menu, kind 0x2d, mode 3) | [`FrontEnd::frame`] |
//! | 0x1eb0a8 mode 3 | counter = ticks(60); logo and PRESS START alphas − 0x10 (≥ 0); the page menu (`fun_002192a8` = L01 0x28c990) | [`FrontEnd::frame`] (the page menu: `menus::pause::PageMenu`) |
//! | 0x1eb0a8 mode 4 | `update_mode_freeze`, `sound_update` | the engine (`menus::freeze`) |
//! | 0x2192a8 kind 0x14 end | the menu closed: 0x15f618 = 1, mode 0 (back to the title) | [`FrontEnd::menu_closed`] |
//! | 0x1eb410 | the world, then (mode 3) the page menu's frame mobys instead of the world's mobys; particles; the logo `draw_textured_quad(0xec, 0x10, 0x100, 0x80, 0, 0, 0x100, 0x80, a << 24 \| 0x808080)` when a ≠ 0; PRESS START `GetEffectTex(max(lang − 1, 0) + 4)` at (0xa0, H − 0x50, 0xc0, 0x60) texels (0, 0, 0x100, 0x80) with its alpha; the black fade at `fade·128`; mode 4: `DrawDialogText` | [`FrontEnd::draw`] (the world: `rc-engine` `title_world`; the logo at x 0xec..0x1ec, y 0x10..0x90 of the 512×416 frame, i.e. the top right, as `draw_textured_quad`'s (x, y, w, h) from the left edge `InitViewContext` puts at 0x13e510 = (0x800 − 256)·16) |

use crate::menus::{scale_ticks, ImageSrc, MenuDraw};
use crate::pad::button;
use rc_formats::texture::Texture;
use std::sync::Arc;

/// The boot sequence's phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// `fun_00209168` ≠ 0: the warning picture; `frames` vsyncs so far, `shown` = a warning was drawn.
    CardCheck { frames: i32, shown: bool },
    /// The logo movie `mpegs[0]` (the engine plays it; `started` once requested).
    Logos { started: bool },
    /// The loading still, `frames` so far (≥ ticks(180)).
    Still { frames: i32 },
    /// The title loop (`transition_do_transition`).
    Title,
    /// A level was asked for: the front end is over.
    Done,
}

/// What a front-end frame asks the engine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrontOut {
    /// `FadeToBlack(n)` vsyncs over the last image.
    pub fade_to_black: Option<i32>,
    /// Play the logos movie `mpegs[0]`.
    pub logos: bool,
    /// `fun_001e9488(i)`: the attract movie i.
    pub attract: Option<i32>,
    /// `initialize_transfer_command`: open the page menu, kind 0x2d.
    pub open_menu: bool,
    /// The front end ended for this level (`0x15f5b0`).
    pub exit: Option<i32>,
}

/// The front end's globals.
#[derive(Clone, Debug)]
pub struct FrontEnd {
    pub phase: Phase,
    /// The boot game mode 0x15f604 (0 title, 3 menu, 4 card dialog).
    pub mode: i32,
    /// 0x15f43c: the black fade (1.0 → 0, −0.0625 per vsync).
    pub fade: f32,
    /// 0x15ef50 / 0x15ef54: the logo's and PRESS START's alpha; 0x15ef58 the title's frame counter.
    pub logo: i32,
    pub press: i32,
    pub counter: i32,
    /// The attract loop: the idle frames (iVar2) and the next movie (iVar9).
    pub idle: i32,
    pub attract_next: i32,
    /// The pictures (the engine loads them): the logo, PRESS START by language, the still, the card warning.
    pub logo_tex: Option<Arc<Texture>>,
    pub press_tex: Option<Arc<Texture>>,
    pub still_tex: Option<Arc<Texture>>,
    pub warning_tex: Option<Arc<Texture>>,
    /// 0x15ed88: the language PRESS START is shown in (its picture key).
    pub lang: u32,
    /// PRESS START of every language (`GetEffectTex(max(language − 1, 0) + 4)`, by language): the draw follows a
    /// language change at once ([`FrontEnd::set_language`]).
    pub press_texs: Vec<Option<Arc<Texture>>>,
}

/// The language slots 0x15ed88 indexes (the text blocks of `all_text` and the levels' text, `rc_formats::strings`).
pub const LANGUAGES: u32 = rc_formats::strings::LANGUAGE_SLOTS;

/// The attract loop's idle time (ticks(1500) vsyncs = 25 s).
pub const ATTRACT_IDLE: i32 = 0x5dc;
/// The still's minimum time (ticks(180)).
pub const STILL_MIN: i32 = 0xb4;
/// The fade step per vsync.
pub const FADE_STEP: f32 = 0.0625;
/// Where the logo and PRESS START are drawn.
pub const LOGO_RECT: [i32; 4] = [0xec, 0x10, 0x100, 0x80];
pub const PRESS_X: i32 = 0xa0;
pub const PRESS_DY: i32 = 0x50;
pub const PRESS_W: i32 = 0xc0;
pub const PRESS_H: i32 = 0x60;
/// The screen height (0x13e504, NTSC).
pub const SCREEN_H: i32 = 416;

impl FrontEnd {
    /// `startlevel`'s start; `rand` = the first `rand()` of `transition_do_transition` (the first attract movie is
    /// `rand() % 4`), `card_check` = `fun_00209168`.
    pub fn new(rand: i32, card_check: i32) -> FrontEnd {
        FrontEnd {
            phase: if card_check != 0 { Phase::CardCheck { frames: 0, shown: false } } else { Phase::Logos { started: false } },
            mode: 0,
            fade: 1.0,
            logo: 0,
            press: 0,
            counter: 0,
            idle: 0,
            attract_next: rand.rem_euclid(4),
            logo_tex: None,
            press_tex: None,
            still_tex: None,
            warning_tex: None,
            lang: 0,
            press_texs: Vec::new(),
        }
    }

    /// 0x15ed88 = `lang` (the Options' Language list, action 9): `transition_default_draw` 0x1eb410 takes PRESS START's
    /// picture of the language every frame.
    pub fn set_language(&mut self, lang: u32) {
        self.lang = lang;
        if let Some(t) = self.press_texs.get(lang as usize) { self.press_tex = t.clone(); }
    }

    /// The card check's frame: `check` = `fun_00209168` now, `pressed` any button this vsync.
    pub fn card_frame(&mut self, check: i32, pressed: u32) -> FrontOut {
        let mut out = FrontOut::default();
        if let Phase::CardCheck { frames, shown } = &mut self.phase {
            let leave = check == 0 || (*frames > 10 && pressed != 0);
            if leave {
                if *shown { out.fade_to_black = Some(10); }
                self.phase = Phase::Logos { started: false };
            } else {
                *shown = true;
                *frames += 1;
            }
        }
        out
    }

    /// One vsync after the boot screens: the logos, the still, then the title loop (`fun_001eb0a8` + the loop's
    /// attract counter). `movie_busy`: a movie is playing (the logos, an attract movie); `menu_open`: the page menu runs.
    pub fn frame(&mut self, pressed: u32, movie_busy: bool) -> FrontOut {
        let mut out = FrontOut::default();
        match self.phase {
            Phase::CardCheck { .. } | Phase::Done => {}
            Phase::Logos { started } => {
                if !started {
                    out.logos = true;
                    self.phase = Phase::Logos { started: true };
                } else if !movie_busy {
                    out.fade_to_black = Some(scale_ticks(0x12));
                    self.phase = Phase::Still { frames: 0 };
                }
            }
            Phase::Still { frames } => {
                if frames + 1 >= scale_ticks(STILL_MIN) {
                    self.phase = Phase::Title;
                    self.fade = 1.0;
                } else {
                    self.phase = Phase::Still { frames: frames + 1 };
                }
            }
            Phase::Title => {
                if movie_busy { return out; }
                // fun_001eb0a8.
                self.fade -= FADE_STEP;
                if self.fade < 0.0 { self.fade = 0.0; }
                match self.mode {
                    0 => {
                        self.counter += 1;
                        if scale_ticks(0x3c) < self.counter {
                            self.logo += 1;
                            if self.logo > 0x40 { self.logo = 0x40; }
                        }
                        if scale_ticks(0x78) < self.counter {
                            let k = ((self.counter - scale_ticks(0x78)) % 0x3c) as f32;
                            let c = (k * 0.104_719_76 + -std::f32::consts::PI).cos();
                            self.press = (c * 32.0) as i32 + 0x60;
                        }
                        if pressed & (button::START | button::CROSS) != 0 {
                            out.open_menu = true;
                            self.mode = 3;
                        }
                    }
                    3 => {
                        self.counter = scale_ticks(0x3c);
                        self.logo = (self.logo - 0x10).max(0);
                        self.press = (self.press - 0x10).max(0);
                    }
                    _ => {}
                }
                // The loop's attract counter.
                if self.mode != 0 {
                    self.idle = 0;
                } else {
                    self.idle += 1;
                    if self.idle >= scale_ticks(ATTRACT_IDLE) {
                        self.idle = 0;
                        out.attract = Some(self.attract_next);
                        self.attract_next = (self.attract_next + 1).rem_euclid(4);
                        self.fade = 1.0;
                        self.logo = 0;
                        self.press = 0;
                        self.counter = 0;
                    }
                }
            }
        }
        out
    }

    /// The page menu finished closing (`fun_002192a8` kind 0x14): back to the title, mode 0.
    pub fn menu_closed(&mut self) { self.mode = 0; }

    /// A page asked for a level (`initialize_global_state_entry`): the front end ends.
    pub fn exit(&mut self, level: i32) -> FrontOut {
        self.phase = Phase::Done;
        FrontOut { exit: Some(level), ..Default::default() }
    }

    /// The front end's own 2D draws of this vsync (`transition_default_draw` / `startlevel`'s pictures), in order; the
    /// page menu's and the card dialog's draws come after the logo / PRESS START and before the fade (the engine adds
    /// them).
    pub fn draw(&self, out: &mut Vec<MenuDraw>) {
        let full = |t: &Arc<Texture>, key: u64, out: &mut Vec<MenuDraw>| {
            out.push(MenuDraw::Image { src: ImageSrc::Pixels { key, tex: t.clone() }, x: 0, y: 0, w: 512, h: SCREEN_H, u: 0, v: 0, tw: 512, th: SCREEN_H, rgba: 0x8080_8080 });
        };
        match self.phase {
            Phase::CardCheck { .. } => {
                if let Some(t) = &self.warning_tex { full(t, KEY_WARNING, out); }
            }
            Phase::Still { .. } => {
                if let Some(t) = &self.still_tex { full(t, KEY_STILL, out); }
            }
            Phase::Title => {
                if self.logo != 0 {
                    if let Some(t) = &self.logo_tex {
                        let [x, y, w, h] = LOGO_RECT;
                        out.push(MenuDraw::Image { src: ImageSrc::Pixels { key: KEY_LOGO, tex: t.clone() }, x, y, w, h, u: 0, v: 0, tw: 0x100, th: 0x80, rgba: (self.logo as u32) << 24 | 0x80_8080 });
                    }
                }
                if self.press != 0 {
                    if let Some(t) = &self.press_tex {
                        out.push(MenuDraw::Image {
                            src: ImageSrc::Pixels { key: KEY_PRESS | (self.lang as u64) << 32, tex: t.clone() },
                            x: PRESS_X,
                            y: SCREEN_H - PRESS_DY,
                            w: PRESS_W,
                            h: PRESS_H,
                            u: 0,
                            v: 0,
                            tw: 0x100,
                            th: 0x80,
                            rgba: (self.press as u32) << 24 | 0x80_8080,
                        });
                    }
                }
            }
            Phase::Logos { .. } | Phase::Done => {}
        }
    }

    /// The black fade over everything (`emit_rgba_draw_packet(0, 0, 0, fade·128)` when > 0), drawn last.
    pub fn draw_fade(&self, out: &mut Vec<MenuDraw>) {
        if self.phase == Phase::Title && self.fade > 0.0 {
            out.push(MenuDraw::Darken { alpha: (self.fade.min(1.0) * 128.0) as i32 });
        }
    }
}

/// Picture keys of [`ImageSrc::Pixels`] (constant pictures).
const KEY_LOGO: u64 = 0xf0_0001;
const KEY_PRESS: u64 = 0xf0_0002;
const KEY_STILL: u64 = 0xf0_0003;
const KEY_WARNING: u64 = 0xf0_0004;
