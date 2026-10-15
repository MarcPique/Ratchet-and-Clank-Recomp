//! The game's text and UI-frame drawing on top of [`crate::hud_render::Hud2d`]: `FontPrint` (level01 0x21ccf0),
//! `FontPrintWindow` (0x21db48), `DrawUIFrame` (0x21c958), and the executor for the draw calls
//! [`rc_game::hud::HudState`] returns. Spec: docs/plan/hud_text.md §3.
//!
//! `FontPrint` per byte `c` (stops at the NUL or after `len` bytes; `len ≤ 0`… `len < 0` runs to the NUL,
//! `len == 0` prints nothing):
//! * 0x08..0x0f: colour code. With the colour-code switch 0x15f45c on (set by the level loader), the colour
//!   becomes `(colour & 0xff000000) | (table[c − 8] & 0xffffff)` (table 0x16ccb8, slot 0 = the caller's colour);
//!   no glyph, no advance either way;
//! * `advance == 0`: skipped;
//! * 0x80..0xa7: the accent glyph `g[c + 0x40]` first, 16×16 at `(x + accent.advance, y + accent.y_off)`;
//! * `c < 0x20` (pad icons): 24×16 at `(x, y + y_off)` in grey `avg(R, G, B)` with the colour's alpha;
//!   `c > 0x20`: 16×16 at `(x, y + y_off)`; space: advance only;
//! * `x += advance`.
//!
//! Every quad is `DrawTexturedQuad` with the font's FX texture, UV = the atlas cell at 1:1.
//! `FontPrintWindow` sets the scissor to the window, locks colour slot 0 to its caller's colour (0x15f460),
//! lays the text out with [`rc_game::hud::text::layout`] and prints each line with `FontPrint`, starting in the
//! colour slot the layout remembered for it. The float-position variant (flag 8, `0x21d0f0`) is drawn like the
//! integer path (no caller here uses it). Drop shadows are drawn by the callers (a black copy at +1, +1).

use crate::hud_render::Hud2d;
use bevy::prelude::GamepadButton;
use rc_formats::font::{Font, GlyphTable, COLOUR_TABLE};
use rc_game::hud::{text, Draw};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum PromptStyle { #[default] PlayStation = 0, Xbox = 1, Nintendo = 2, DualSense = 3, DualShock4 = 4, DualShock3 = 5, XboxSeries = 6, XboxOne = 7, Xbox360 = 8 }

/// FX slot reserved for the CC0 64-pixel icons assembled from Kenney Input Prompts.
pub const PROMPT_FX: usize = 256;

const DEFAULT_PROMPT_PACK: u64 = {
    let physical = [0u8, 1, 3, 2, 4, 5, 6, 7, 10, 11];
    let mut bits = 0u64;
    let mut i = 0;
    while i < 10 { bits |= (physical[i] as u64) << (4 + i * 4); i += 1; }
    bits
};
static PROMPTS: AtomicU64 = AtomicU64::new(DEFAULT_PROMPT_PACK);

fn physical_index(button: GamepadButton) -> u8 {
    match button {
        GamepadButton::South => 0, GamepadButton::East => 1,
        GamepadButton::West => 2, GamepadButton::North => 3,
        GamepadButton::LeftTrigger => 4, GamepadButton::RightTrigger => 5,
        GamepadButton::LeftTrigger2 => 6, GamepadButton::RightTrigger2 => 7,
        GamepadButton::Start => 8, GamepadButton::Select => 9,
        GamepadButton::LeftThumb => 10, GamepadButton::RightThumb => 11,
        GamepadButton::DPadUp => 12, GamepadButton::DPadDown => 13,
        GamepadButton::DPadLeft => 14, GamepadButton::DPadRight => 15,
        _ => 8,
    }
}

/// One atomic snapshot for every text pass, including the in-game HUD, native
/// menus, cutscenes and the vendor screen. The mapping follows remapped buttons.
pub fn set_button_prompts(style: PromptStyle, buttons: &[GamepadButton; 10]) {
    PROMPTS.store(pack_prompts(style, buttons), Ordering::Relaxed);
}

fn pack_prompts(style: PromptStyle, buttons: &[GamepadButton; 10]) -> u64 {
    let mut pack = style as u64;
    for (i, button) in buttons.iter().enumerate() { pack |= (physical_index(*button) as u64) << (4 + i * 4); }
    pack
}

fn physical_for(pack: u64, code: u8) -> u8 { ((pack >> (4 + (code - 0x10) as u64 * 4)) & 0xf) as u8 }

fn draw_prompt(out: &mut Hud2d, pack: u64, code: u8, x: i32, y: i32, rgba: u32) {
    let physical = physical_for(pack, code);
    let row = (pack & 15).min(8) as i32;
    // Kenney's transparent 64x64 icon sits inside the game's 24x16 prompt cell.
    // Menus print a dark text shadow before the lit text. Preserve that dark
    // pass, otherwise both passes become white icons offset by two pixels.
    // Lit prompts keep the pack's own PlayStation/Xbox/Switch colours.
    let grey = rgba & 0xff;
    let tint = if grey < 0x40 { grey } else { 0x7f };
    out.strip_glyph(PROMPT_FX, x + 4, y, 16, 16, physical as i32 * 64, row * 64, 64, 64,
        (rgba & 0xff00_0000) | (tint << 16) | (tint << 8) | tint);
}

/// The global text state `FontPrint` reads.
#[derive(Clone, Copy, Debug)]
pub struct TextState {
    /// 0x15f45c: colour codes honoured (1 during play; `FUN_0021cc38` / `FUN_0021cc28` toggle it).
    pub colour_codes: bool,
    /// 0x15f460: slot 0 of the colour table is not overwritten by `FontPrint` (inside `FontPrintWindow`).
    pub window_lock: bool,
    /// 0x16ccb8.
    pub table: [u32; 8],
    prompt_pack: u64,
}

impl Default for TextState {
    fn default() -> Self { TextState { colour_codes: true, window_lock: false, table: COLOUR_TABLE, prompt_pack: PROMPTS.load(Ordering::Relaxed) } }
}

/// `FontPrint(x, y, rgba, text, len)` with the glyph table and FX texture of `font`.
#[allow(clippy::too_many_arguments)]
pub fn font_print(out: &mut Hud2d, st: &mut TextState, glyphs: &GlyphTable, font: Font, x: i32, y: i32, rgba: u32, text: &[u8], len: i32) {
    if !st.window_lock { st.table[0] = rgba; }
    let at = |i: usize| text.get(i).copied().unwrap_or(0);
    if len == 0 || at(0) == 0 { return; }
    let fx = font.fx_texture();
    let (mut x, mut colour) = (x, rgba);
    let mut i = 0usize;
    loop {
        let c = at(i);
        if c.wrapping_sub(8) < 8 {
            if st.colour_codes { colour = (colour & 0xff00_0000) | (st.table[(c - 8) as usize] & 0x00ff_ffff); }
        } else if let Some(g) = glyphs.get(c as usize).filter(|g| g.advance != 0) {
            if c.wrapping_add(0x80) < 0x28 {
                let e = glyphs[c as usize + 0x40];
                out.strip_glyph(fx, x + e.advance as i32, y + e.y_off as i32, 16, 16, e.u as i32, e.v as i32, 16, 16, colour);
            }
            if c < 0x20 {
                let avg = ((colour & 0xff) + ((colour >> 8) & 0xff) + ((colour >> 16) & 0xff)) / 3;
                let grey = (colour & 0xff00_0000).wrapping_add(avg << 16).wrapping_add(avg << 8).wrapping_add(avg);
                if (0x10..=0x19).contains(&c) {
                    draw_prompt(out, st.prompt_pack, c, x, y + g.y_off as i32, grey);
                } else {
                    out.strip_glyph(fx, x, y + g.y_off as i32, 24, 16, g.u as i32, g.v as i32, 24, 16, grey);
                }
            } else if c > 0x20 {
                out.strip_glyph(fx, x, y + g.y_off as i32, 16, 16, g.u as i32, g.v as i32, 16, 16, colour);
            }
            x += g.advance as i32;
        }
        i += 1;
        if i as i32 == len || at(i) == 0 { break; }
    }
}

/// `FontPrintWindow(window, rgba, text, len)`; fills the window's measured width and height.
#[allow(clippy::too_many_arguments)]
pub fn font_print_window(out: &mut Hud2d, st: &mut TextState, glyphs: &GlyphTable, font: Font, win: &mut text::Window, rgba: u32, text: &[u8], len: i32) {
    out.set_scissor(win.x_min as i32, win.x_max as i32 - 1, win.y_min as i32, win.y_max as i32 - 1);
    st.window_lock = true;
    let lines = text::layout(win, text, len, glyphs, st.colour_codes);
    if win.flags & text::MEASURE_ONLY == 0 {
        for l in lines {
            st.table[0] = rgba;
            let colour = st.table[l.colour as usize & 7];
            font_print(out, st, glyphs, font, l.x, l.y, colour, text.get(l.start..).unwrap_or(&[]), l.count);
        }
    }
    st.window_lock = false;
    out.reset_scissor();
}

/// `DrawUIFrame(top, bottom, left, right, alpha)`: an RGB (4, 4, 4) rectangle with three stepped columns on each
/// side that round its ends.
pub fn draw_ui_frame(out: &mut Hud2d, top: i32, bottom: i32, left: i32, right: i32, alpha: i32) {
    let c = (alpha as u32) << 24 | 0x0004_0404;
    out.rect(top, bottom, left, right, c);
    out.rect(top + 1, bottom - 1, left - 2, left, c);
    out.rect(top + 2, bottom - 2, left - 3, left - 2, c);
    out.rect(top + 4, bottom - 4, left - 4, left - 3, c);
    out.rect(top + 1, bottom - 1, right, right + 2, c);
    out.rect(top + 2, bottom - 2, right + 2, right + 3, c);
    out.rect(top + 4, bottom - 4, right + 3, right + 4, c);
}

/// Draws the HUD's calls in order.
pub fn execute(out: &mut Hud2d, st: &mut TextState, glyphs: &[GlyphTable; 3], draws: &[Draw]) {
    for d in draws {
        match d {
            Draw::Sprite { frame, x, y, w, h, alpha, rot } => out.sprite(*frame, *x, *y, *w, *h, *alpha, *rot),
            Draw::Text { font, x, y, rgba, text } => font_print(out, st, &glyphs[*font as usize], *font, *x, *y, *rgba, text, -1),
            Draw::TextWindow { font, window, rgba, text } => {
                let mut w = *window;
                font_print_window(out, st, &glyphs[*font as usize], *font, &mut w, *rgba, text, -1);
            }
            Draw::UiFrame { top, bottom, left, right, alpha } => draw_ui_frame(out, *top, *bottom, *left, *right, *alpha),
            Draw::FxQuad { fx, x, y, w, h, u, v, tw, th, rgba } => out.strip_glyph(*fx, *x, *y, *w, *h, *u, *v, *tw, *th, *rgba),
            Draw::Sprite16 { frame, x, y, w, h, alpha } => out.sprite_fine(*frame, *x, *y, *w, *h, *alpha),
            Draw::Rect16 { x0, y0, x1, y1, rgba } => out.rect_fine(*x0, *y0, *x1, *y1, *rgba),
            Draw::SpriteSub { frame, x, y, w, h, alpha } => out.sprite_sub(*frame, *x, *y, *w, *h, *alpha),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud_render::Tex;
    use rc_formats::font::{Glyph, GLYPHS};

    fn glyphs() -> GlyphTable {
        let mut t = [Glyph::default(); GLYPHS];
        t[b'A' as usize] = Glyph { u: 0, v: 0, y_off: 0, advance: 14 };
        t[b'B' as usize] = Glyph { u: 16, v: 0, y_off: 0, advance: 14 };
        t[b' ' as usize] = Glyph { u: 0, v: 0, y_off: 0, advance: 5 };
        t[0x10] = Glyph { u: 0, v: 96, y_off: -2, advance: 20 };
        t[0x11] = Glyph { u: 32, v: 96, y_off: -2, advance: 20 };
        t[0x8c] = Glyph { u: 64, v: 32, y_off: 0, advance: 11 };
        t[0x8c + 0x40] = Glyph { u: 200, v: 96, y_off: -4, advance: 2 };
        t
    }

    #[test]
    fn inline_prompt_tracks_style_and_remapped_button() {
        let g = glyphs();
        let buttons = [GamepadButton::East; 10];
        let mut st = TextState::default();
        st.prompt_pack = pack_prompts(PromptStyle::PlayStation, &buttons);
        let mut out = Hud2d::default();
        font_print(&mut out, &mut st, &g, Font::Small, 0, 0, 0xffff_ffff, b"\x10", -1);
        assert_eq!(out.prims.len(), 1);
        assert_eq!(out.prims[0].uv[0], [64, 0]); // Cross action, physical Circle button
        assert_eq!(out.prims[0].rgba & 0x00ff_ffff, 0x007f_7f7f);

        out.prims.clear();
        font_print(&mut out, &mut st, &g, Font::Small, 2, 2, 0x8000_0000, b"\x10", -1);
        assert_eq!(out.prims[0].rgba, 0x8000_0000); // A menu's dark shadow remains dark.

        st.prompt_pack = pack_prompts(PromptStyle::Xbox, &[GamepadButton::South; 10]);
        out.prims.clear();
        font_print(&mut out, &mut st, &g, Font::Small, 0, 0, 0xffff_ffff, b"\x10", -1);
        assert_eq!(out.prims.len(), 1);
        assert_eq!(out.prims[0].uv[0], [0, 64]); // Xbox A

        st.prompt_pack = pack_prompts(PromptStyle::Nintendo, &[GamepadButton::South; 10]);
        out.prims.clear();
        font_print(&mut out, &mut st, &g, Font::Small, 0, 0, 0xffff_ffff, b"\x10", -1);
        assert_eq!(out.prims.len(), 1);
        assert_eq!(out.prims[0].uv[0], [0, 128]); // Nintendo B
        for (style, row) in [(PromptStyle::DualSense, 3), (PromptStyle::DualShock4, 4),
            (PromptStyle::DualShock3, 5), (PromptStyle::XboxSeries, 6),
            (PromptStyle::XboxOne, 7), (PromptStyle::Xbox360, 8)] {
            st.prompt_pack = pack_prompts(style, &[GamepadButton::Select; 10]);
            out.prims.clear();
            font_print(&mut out, &mut st, &g, Font::Small, 0, 0, 0xffff_ffff, b"\x10", -1);
            assert_eq!(out.prims[0].uv[0], [9 * 64, row * 64]);
        }
    }

    #[test]
    fn font_print_rules() {
        let g = glyphs();
        let mut out = Hud2d::default();
        let mut st = TextState::default();
        font_print(&mut out, &mut st, &g, Font::Small, 10, 20, 0x8010_2030, b"A \x0cA\x11\x8c\x08A", -1);
        let q: Vec<_> = out.prims.iter().map(|p| (p.tex, p.rect(), p.rgba)).collect();
        assert_eq!(q.len(), 6);
        assert_eq!(q[0], (Tex::Fx(2), [10, 20, 16, 16], 0x8010_2030));
        // Orange after 0x0c, keeps alpha.
        assert_eq!(q[1], (Tex::Fx(2), [29, 20, 16, 16], 0x8040_80c0));
        // HD pad icon: 64×64 source, drawn inside the original 24×16 cell.
        assert_eq!(q[2], (Tex::Fx(PROMPT_FX), [47, 18, 16, 16], 0x807f_7f7f));
        // Accent (at x + its advance, y + its y_off) before the letter.
        assert_eq!(q[3], (Tex::Fx(2), [65, 16, 16, 16], 0x8040_80c0));
        assert_eq!(q[4], (Tex::Fx(2), [63, 20, 16, 16], 0x8040_80c0));
        // 0x08 back to the caller's colour.
        assert_eq!(q[5], (Tex::Fx(2), [74, 20, 16, 16], 0x8010_2030));
        // len limits the bytes.
        let mut out = Hud2d::default();
        font_print(&mut out, &mut st, &g, Font::Small, 0, 0, 0x8080_8080, b"AAAA", 2);
        assert_eq!(out.prims.len(), 2);
    }

    #[test]
    fn ui_frame_rects() {
        let mut out = Hud2d::default();
        draw_ui_frame(&mut out, 100, 140, 50, 150, 0x60);
        assert_eq!(out.prims.len(), 7);
        assert_eq!(out.prims[0].rect(), [50, 100, 100, 40]);
        assert_eq!(out.prims[3].rect(), [46, 104, 1, 32]);
        assert!(out.prims.iter().all(|p| p.rgba == 0x6004_0404 && p.tex == Tex::None));
    }
}
