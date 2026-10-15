//! Game mode 7, the credits slideshow: `EnterSlideshowMode` 0x2ad558, `SlideshowModeUpdate` 0x2ad738, its exit
//! `FUN_002ad6f0` and the render `SlideshowModeRender` 0x21ab78 (the picture `FUN_0021e158`, the black fade, the text
//! lines `FUN_0021e8a0` → `FUN_0021e728`). Callers: the Goodies' Credits (menu post-action 7, after `FadeToBlack(16)`)
//! and the end of the game (the boss 1422's phase 9, level18 `0x299610`). Spec: docs/plan/progression.md `## media`.
//! Addresses are level01; the tables are read from the level overlay.
//!
//! **Pictures.** `credits_images_ntsc` (TOC 0x16a8, memory 0x139228 = TOC + 0x137b80; PAL 0x1748 / 0x1392c8): 20
//! raw 512×416 PSMCT32 pictures (`global/credits_images_ntsc/NNN.bin`). The mode keeps two buffers of 512×448 words
//! (0x15f628 shown, 0x15f62c loading, carved from the top of the level heap 0x1611cc − 0xe0000); picture index i
//! (0x15f622) loads file `i % 20` (the entry's first two loads index the TOC without the modulo: files 0 and 1).
//!
//! **Text.** 183 entries of 16 bytes at 0x173500: `{s32 msg, s16 x, s16 y, s16 start, s16 duration, s16 align, s16
//! join}`, drawn when start ≤ t ≤ start + duration (t = 0x15f626).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2ad558 | `update_audio_stream_until_idle(0)`, `FadeToBlack(8)` (blocking, over the last image), 0x15f5d8 = 1, mode 7, 0x15f3fc = 1.0, 0x15f620..0x15f62f = 0, the buffers, picture 0 read into the shown buffer (waited for), index 1, picture 1 read into the loading buffer | [`Slideshow::new`] (the engine: the fade, the mode, the reads) |
//! | 0x2ad738 +0x00 | `update_audio_stream_until_idle(0)`; 0x15f3fc −= 0.05, below 0 → 0 | [`Slideshow::tick`] |
//! | 0x2ad738 state 0 | t ≥ times[i] − 0x24: wait for the read (`update_audio_stream_until_idle(1)`); (non-US and i = 0x27) or i > 0x2a → the exit; else counter = −1, state 1 | [`Slideshow::tick`] (the reads are synchronous in the port) |
//! | 0x2ad738 state 1, counter < 0x20 | `VU1_syncChain(1)`; the dissolve: every word w of the 0x38000 with w % 32 = counter copied from the loading to the shown buffer (the pixels of column x % 32 = counter) | [`Slideshow::tick`] ([`Out::dissolve`]) |
//! | 0x2ad738 state 1, counter > 0x23 | US (0x13d1d2 = 'A'): i = 0x11 → i = 0x1b, t = 0x2238; i = 0x25 → i = 0x27, t = 0x32b4; else i = 8 → i = 0x11, t = 0x1590; state 0, i + 1, counter = −1, picture `i % 20` read into the loading buffer | [`Slideshow::tick`] ([`Out::load`]) |
//! | 0x2ad738 tail | counter + 1, t + 1; Start pressed (0x13cae4 & 0x800) → the exit; `sound_update` | [`Slideshow::tick`] (the engine runs `sound_update`) |
//! | 0x2ad6f0 | `VU1_syncChain(1)`, `FadeToBlack(12)` over the last image, `request_audio_stream_break`, mode 0, 0x15f5d8 = 1, 0x15f3fc = 0 | [`Out::exit`] (the engine: the fade and the mode) |
//! | 0x21ab78 | not while 0x15f5d8; the shown buffer uploaded over the frame (0x13e504 = 416 rows); 0x15f3fc > 0: (> 1: 1.0) black at `trunc(f·128)`; then the 183 text entries in order at t | [`Slideshow::draw`] |
//! | 0x21e8a0 | visible s ≤ t ≤ e (e = s + d); frame fade a = 1, (t − s)/16 below s + 16, (e − t)/8 above e − 8; text fade b = 1, max((t − 16 − s)/16, 0) below s + 32, max((e − (t + 8))/8, 0) above e − 16; join = 1: one line, every run of 0x01 → " − "; else one line per 0x01 run, y + 0x1a each | [`Slideshow::draw`] |
//! | 0x21e728 | width = (int)(measure(regular)·a); align 0: x ± w/2, 1: x − w .. x, −1: x .. x + w; the bar `draw_stretchable_ui_frame(x0 − 0x20, y − 0xe, x1 − x0 + 0x40, 0x1c, trunc(a·112))`; colour codes off; the regular font, black shadow at (x0 + 1, y − 7) and the text 0xe0e0e0 at (x0, y − 8), alpha `(int)(b·128)` | [`Slideshow::draw`] |
//!
//! Not modelled: the IOP stream's timing (the pictures are read at once; a slow disc would hold the mode in the state-0
//! wait), PAL (the port is NTSC: the PAL pictures and the non-US jumps are selected by `us = false` only).

use crate::hud::{Draw, Rot};
use crate::menus::{ImageSrc, MenuAssets, MenuDraw};
use rc_formats::font::Font;

/// The level-01 labels of the tables.
pub mod label {
    /// The picture times, s32 per picture index.
    pub const TIMES: u32 = 0x174070;
    /// The text entries, 16 bytes each.
    pub const TEXT: u32 = 0x173500;
}

/// Text entries drawn by `SlideshowModeRender` (0xb6 + 1).
pub const TEXT_ENTRIES: usize = 0xb7;
/// Picture indices the times table covers (the last index tested is 0x2b).
pub const TIMES_LEN: usize = 0x2c;
/// The credits pictures on the disc (`credits_images_ntsc`).
pub const PICTURES: i32 = 20;
/// The picture size.
pub const WIDTH: usize = 512;
pub const HEIGHT: usize = 416;
/// `EnterSlideshowMode`'s `FadeToBlack(8)` and the exit's `FadeToBlack(12)`.
pub const ENTER_FADE: u32 = 8;
pub const EXIT_FADE: u32 = 12;
/// The menu's `FadeToBlack(ticks(16))` before the slideshow (post-action 7).
pub const MENU_FADE: u32 = 16;
/// The bar icon of `draw_stretchable_ui_frame`.
pub const BAR_ICON: u16 = 0x7580;

/// One text entry (0x173500 + 16·k).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextEntry {
    pub msg: i32,
    pub x: i16,
    pub y: i16,
    pub start: i16,
    pub duration: i16,
    /// 0 centred, 1 ending at x, −1 starting at x.
    pub align: i16,
    /// 1: the lines joined with " − " on one row.
    pub join: i16,
}

/// The tables of the mode.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tables {
    pub times: Vec<i32>,
    pub text: Vec<TextEntry>,
}

impl Tables {
    pub fn read(ov: &crate::menus::Overlay) -> Tables {
        let (t, x) = (ov.at(label::TIMES), ov.at(label::TEXT));
        let times = (0..TIMES_LEN as u32).map(|i| ov.i32(t + 4 * i).unwrap_or(i32::MAX)).collect();
        let text = (0..TEXT_ENTRIES as u32)
            .map(|k| {
                let a = x + 16 * k;
                let h = |o: u32| ov.i16(a + o).unwrap_or(0);
                TextEntry { msg: ov.i32(a).unwrap_or(0), x: h(4), y: h(6), start: h(8), duration: h(10), align: h(12), join: h(14) }
            })
            .collect();
        Tables { times, text }
    }
}

/// What a tick asks of the engine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Out {
    /// Copy the loading buffer's pixels of columns `x % 32 == c` into the shown buffer.
    pub dissolve: Option<u32>,
    /// Read picture file `n` (0..20) into the loading buffer.
    pub load: Option<i32>,
    /// `FUN_002ad6f0`: `FadeToBlack(12)`, then mode 0.
    pub exit: bool,
    /// Start pressed (the exit came from the pad).
    pub skipped: bool,
}

/// The mode's globals 0x15f620..0x15f62f and 0x15f3fc.
#[derive(Clone, Debug, PartialEq)]
pub struct Slideshow {
    /// 0x15f620: 0 waiting for the next picture's time, 1 dissolving.
    pub state: i16,
    /// 0x15f622: the picture index.
    pub index: i16,
    /// 0x15f624: the dissolve counter.
    pub counter: i16,
    /// 0x15f626: the time (ticks).
    pub time: i16,
    /// 0x15f3fc: the fade from black.
    pub fade: f32,
    /// 0x13d1d2 == 'A' (the US build: the port is always US).
    pub us: bool,
    /// The mode was left (no more ticks).
    pub done: bool,
}

impl Slideshow {
    /// `EnterSlideshowMode` (the state; the engine reads picture 0 into the shown buffer and file 1 into the loading
    /// one: [`Slideshow::first_loads`]).
    pub fn new(us: bool) -> Slideshow { Slideshow { state: 0, index: 1, counter: 0, time: 0, fade: 1.0, us, done: false } }

    /// The two reads of `EnterSlideshowMode`: (shown, loading) picture files (the TOC indexed by 0 and 1, no modulo).
    pub fn first_loads() -> (i32, i32) { (0, 1) }

    /// `SlideshowModeUpdate` (one main-loop frame of mode 7). `start` = Start newly pressed (0x13cae4 & 0x800).
    pub fn tick(&mut self, t: &Tables, start: bool) -> Out {
        let mut out = Out::default();
        if self.done { return out; }
        self.fade -= 0.05;
        if self.fade < 0.0 { self.fade = 0.0; }
        if self.state == 0 {
            let at = t.times.get(self.index as usize).copied().unwrap_or(i32::MAX);
            if at - 0x24 <= self.time as i32 {
                if (!self.us && self.index == 0x27) || self.index > 0x2a {
                    self.exit(&mut out);
                    return out;
                }
                self.counter = -1;
                self.state = 1;
            }
        } else if self.state == 1 {
            if self.counter < 0x20 {
                out.dissolve = Some(self.counter as u32);
            } else if self.counter > 0x23 {
                if self.us {
                    if self.index == 0x11 {
                        self.index = 0x1b;
                        self.time = 0x2238;
                    }
                    if self.index == 0x25 {
                        self.time = 0x32b4;
                        self.index = 0x27;
                    }
                } else if self.index == 8 {
                    self.time = 0x1590;
                    self.index = 0x11;
                }
                self.state = 0;
                self.index += 1;
                self.counter = -1;
                out.load = Some(self.index as i32 % PICTURES);
            }
        }
        self.counter = self.counter.wrapping_add(1);
        self.time = self.time.wrapping_add(1);
        if start {
            out.skipped = true;
            self.exit(&mut out);
        }
        out
    }

    /// `FUN_002ad6f0`: 0x15f3fc = 0, mode 0 (the engine runs the `FadeToBlack(12)` before).
    fn exit(&mut self, out: &mut Out) {
        self.fade = 0.0;
        self.done = true;
        out.exit = true;
    }

    /// `SlideshowModeRender` after the shown buffer (the engine draws it first, `image`): the black fade and the text.
    pub fn draw(&self, t: &Tables, a: &MenuAssets, image: Option<ImageSrc>, out: &mut Vec<MenuDraw>) {
        if let Some(src) = image {
            out.push(MenuDraw::Image { src, x: 0, y: 0, w: WIDTH as i32, h: HEIGHT as i32, u: 0, v: 0, tw: WIDTH as i32, th: HEIGHT as i32, rgba: 0x8080_8080 });
        }
        if self.fade > 0.0 {
            let f = self.fade.min(1.0);
            let alpha = (f * 128.0) as i32 as u32;
            out.push(MenuDraw::Rect { x0: 1, y0: 1, x1: WIDTH as i32 + 1, y1: HEIGHT as i32 + 1, rgba: alpha << 24 });
        }
        for e in &t.text { draw_entry(e, self.time as i32, a, out); }
    }
}

/// `FUN_0021e8a0(entry, t)`.
fn draw_entry(e: &TextEntry, t: i32, a: &MenuAssets, out: &mut Vec<MenuDraw>) {
    let (s, end) = (e.start as i32, e.start as i32 + e.duration as i32);
    if !(s <= t && t <= end) { return; }
    let mut fa = 1.0f32;
    if t < s + 0x10 { fa = (t - s) as f32 * 0.0625; }
    if end - 8 < t { fa = (end - t) as f32 * 0.125; }
    let mut fb = 1.0f32;
    if t < s + 0x20 {
        fb = ((t - 0x10) - s) as f32 * 0.0625;
        if fb < 0.0 { fb = 0.0; }
    }
    if end - 0x10 < t {
        fb = (end - (t + 8)) as f32 * 0.125;
        if fb < 0.0 { fb = 0.0; }
    }
    let text = a.msg(e.msg);
    if e.join == 1 {
        let mut line = Vec::with_capacity(text.len() + 8);
        let mut i = 0;
        while i < text.len() {
            if text[i] == 1 {
                line.extend_from_slice(b" - ");
                while i < text.len() && text[i] == 1 { i += 1; }
            } else {
                line.push(text[i]);
                i += 1;
            }
        }
        draw_line(fa, fb, e.x as i32, e.y as i32, e.align as i32, &line, a, out);
    } else {
        let mut y = e.y as i32;
        let mut line: Vec<u8> = Vec::new();
        let mut i = 0;
        while i < text.len() {
            if text[i] == 1 {
                draw_line(fa, fb, e.x as i32, y, e.align as i32, &line, a, out);
                y += 0x1a;
                line.clear();
                while i < text.len() && text[i] == 1 { i += 1; }
            } else {
                line.push(text[i]);
                i += 1;
            }
        }
        if !line.is_empty() { draw_line(fa, fb, e.x as i32, y, e.align as i32, &line, a, out); }
    }
}

/// `FUN_0021e728(a, b, x, y, align, text)`.
#[allow(clippy::too_many_arguments)]
fn draw_line(fa: f32, fb: f32, x: i32, y: i32, align: i32, text: &[u8], a: &MenuAssets, out: &mut Vec<MenuDraw>) {
    let w = (a.width(Font::Regular, text) as f32 * fa) as i32;
    let (x0, x1) = match align {
        0 => (x - (w >> 1), x + (w >> 1)),
        1 => (x - w, x),
        -1 => (x, x + w),
        _ => (x, x),
    };
    // draw_stretchable_ui_frame 0x251ab0: cap, stretched middle, the cap turned 180°.
    let alpha = (fa * 112.0) as i32;
    let (bx, by, bw, bh) = (x0 - 0x20, y - 0xe, (x1 - x0) + 0x40, 0x1c);
    let (mid, cap) = (a.frame(BAR_ICON, 0), a.frame(BAR_ICON, 1));
    out.push(MenuDraw::Hud(Draw::Sprite { frame: cap, x: bx, y: by, w: 0x20, h: bh, alpha, rot: Rot::None }));
    out.push(MenuDraw::Hud(Draw::Sprite { frame: mid, x: bx + 0x20, y: by, w: bw - 0x40, h: bh, alpha, rot: Rot::None }));
    out.push(MenuDraw::Hud(Draw::Sprite { frame: cap, x: bx + bw - 0x20, y: by, w: 0x20, h: bh, alpha, rot: Rot::R180 }));
    // FUN_0021cc38: colour codes off for the two prints (FUN_0021cc28 turns them back on).
    let ta = ((fb * 128.0) as i32 as u32) << 24;
    crate::menus::text_plain(out, Font::Regular, x0 + 1, y - 7, ta, text);
    crate::menus::text_plain(out, Font::Regular, x0, y - 8, ta | 0x00e0_e0e0, text);
}

/// The dissolve step of the shown buffer (`SlideshowModeUpdate` state 1): the pixels of the columns `x % 32 == c` of
/// `src` copied into `dst` (both 512 wide RGBA rows).
pub fn dissolve(dst: &mut [u8], src: &[u8], c: u32) {
    let n = dst.len().min(src.len()) / 4;
    let mut p = c as usize;
    while p < n {
        dst[4 * p..4 * p + 4].copy_from_slice(&src[4 * p..4 * p + 4]);
        p += 32;
    }
}
