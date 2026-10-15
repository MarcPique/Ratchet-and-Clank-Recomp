//! The Options sub-pages' widgets (docs/plan/menus.md §3 "Options sub-pages"): the toggle list (HelpDesk,
//! Subtitles; update 0x294830, draw 0x294a38), the camera list (0x294cc0 / 0x294e68), the Sound page
//! (`SoundOptionsMenu` 0x290538 / `DrawSoundMenu` 0x290808) and Quit Game (0x2921d0 / `DrawQuitGameMenu`
//! 0x292298). All draws are panel-local; all fonts are the regular one (the `font_print_*` wrappers 0x21cf70,
//! 0x21d3f8, 0x21d5a8).
//!
//! The option variables are the pointers stored in the entries (0x15ee1c, 0x15ede0, …); they are mapped to
//! the [`Global`] fields at those addresses by [`opt_byte`] / [`set_opt_byte`].

use super::super::{text, MenuAssets, MenuDraw, MenuInput, MenuSound, Overlay};
use super::{Data, MenuOut, PageMenu, LIGHT_BLUE, YELLOW};
use crate::game_state::{GameState, Global};
use crate::hud::{text as wtext, Draw};
use crate::pad::button;
use rc_formats::font::Font;

/// A toggle entry (0x14 bytes): `{label, u8* flag, on_id, off_id, flags}`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToggleEntry {
    pub label: u32,
    pub var: u32,
    pub on: u32,
    pub off: u32,
    pub flags: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Toggle {
    pub flags: u32,
    pub entries: Vec<ToggleEntry>,
    /// +0x38 cursor, +0x3c countdown of the cheat path.
    pub cursor: i32,
    pub countdown: i32,
}

/// A camera entry (0x18 bytes): `{label, u8* var, str[4]}`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CameraEntry {
    pub label: u32,
    pub var: u32,
    pub strs: [u32; 4],
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Camera {
    pub entries: Vec<CameraEntry>,
    pub cursor: i32,
}

impl Toggle {
    pub fn read(ov: &Overlay, raw: &[u32; 8]) -> Toggle {
        let mut entries = Vec::new();
        for k in 0..32u32 {
            let b = raw[1] + 0x14 * k;
            let w = |o: u32| ov.u32(b + o).unwrap_or(0);
            if w(0) == 0 { break; }
            entries.push(ToggleEntry { label: w(0), var: w(4), on: w(8), off: w(0xc), flags: w(0x10) });
        }
        Toggle { flags: raw[0], entries, cursor: raw[2] as i32, countdown: raw[3] as i32 }
    }
}

impl Camera {
    pub fn read(ov: &Overlay, raw: &[u32; 8]) -> Camera {
        let mut entries = Vec::new();
        for k in 0..32u32 {
            let b = raw[1] + 0x18 * k;
            let w = |o: u32| ov.u32(b + o).unwrap_or(0);
            if w(0) == 0 { break; }
            entries.push(CameraEntry { label: w(0), var: w(4), strs: [w(8), w(0xc), w(0x10), w(0x14)] });
        }
        Camera { entries, cursor: raw[2] as i32 }
    }
}

/// The byte at an option variable's address (the low byte of the 4-byte globals, as `*(u8*)` reads it).
pub fn opt_byte(g: &Global, addr: u32) -> Option<u8> {
    Some(match addr {
        0x15ee1c => g.helpdesk_voice,
        0x15ee1d => g.helpdesk_text,
        0x15ee40 => g.subtitles,
        0x15ede0 => g.cam_yaw_normal as u8,
        0x15eddc => g.cam_pitch_normal as u8,
        0x15ede4 => g.cam_speed as u8,
        0x15ede8 => g.stereo as u8,
        // The Cheats page's entries (0x28dbe8): the cheat bytes 0x15edb0[12].
        a if crate::cheats::slot_of(a).is_some() => g.cheats_active[crate::cheats::slot_of(a)?],
        _ => return None,
    })
}

/// Writes the low byte of the variable at `addr`.
pub fn set_opt_byte(g: &mut Global, addr: u32, v: u8) -> bool {
    let lo = |x: &mut i32| *x = (*x & !0xff) | v as i32;
    match addr {
        0x15ee1c => g.helpdesk_voice = v,
        0x15ee1d => g.helpdesk_text = v,
        0x15ee40 => g.subtitles = v,
        0x15ede0 => lo(&mut g.cam_yaw_normal),
        0x15eddc => lo(&mut g.cam_pitch_normal),
        0x15ede4 => lo(&mut g.cam_speed),
        0x15ede8 => lo(&mut g.stereo),
        a if crate::cheats::slot_of(a).is_some() => {
            if let Some(i) = crate::cheats::slot_of(a) { g.cheats_active[i] = v; }
        }
        _ => return false,
    }
    true
}

fn toggle_mut(m: &mut PageMenu, w: u32) -> Option<&mut Toggle> {
    match m.widgets.get_mut(&w).map(|x| &mut x.data) {
        Some(Data::Toggle(t)) => Some(t),
        _ => None,
    }
}

fn camera_mut(m: &mut PageMenu, w: u32) -> Option<&mut Camera> {
    match m.widgets.get_mut(&w).map(|x| &mut x.data) {
        Some(Data::Camera(t)) => Some(t),
        _ => None,
    }
}

/// Start/Select/R3 → `close` (1 or −1), △ → parent or −1 without one (`0x1ba294` blocks both).
pub(super) fn keys(m: &mut PageMenu, inp: &MenuInput, close: i32) -> Option<i32> {
    if inp.pressed_u & 0xd00 != 0 && !m.no_close { return Some(close); }
    if inp.pressed_u & button::TRIANGLE != 0 {
        let parent = m.pages.get(&m.current).map_or(0, |p| p.parent);
        if parent != 0 {
            m.target = parent;
        } else if !m.no_close {
            return Some(-1);
        }
    }
    None
}

/// 0x294830.
pub fn toggle_update(m: &mut PageMenu, w: u32, inp: &MenuInput, gs: &mut GameState, out: &mut MenuOut) -> i32 {
    if let Some(t) = toggle_mut(m, w) {
        if t.countdown != 0 {
            // The cheat path's fade (0x15f3fc = min(c, 4)·0.25): not used by the Options toggles.
            t.countdown -= 1;
            return 0;
        }
    }
    if !m.is_focus(w) { return 0; }
    if let Some(r) = keys(m, inp, 1) { return r; }
    let Some(t) = toggle_mut(m, w) else { return 0 };
    let start = t.cursor;
    if inp.pressed_u & button::UP != 0 && t.cursor != 0 { t.cursor -= 1; }
    if inp.pressed_u & button::DOWN != 0 && t.entries.get(t.cursor as usize + 1).is_some_and(|e| e.label != 0) { t.cursor += 1; }
    if inp.pressed_u & button::CROSS != 0 {
        out.sounds.push(MenuSound::Confirm);
        let e = t.entries.get(t.cursor as usize).copied().unwrap_or_default();
        if e.flags & 1 == 0 {
            if e.var != 0 {
                let v = opt_byte(&gs.global, e.var).unwrap_or(0);
                set_opt_byte(&mut gs.global, e.var, (v == 0) as u8);
            }
        } else {
            *m.stub_calls.entry("cheat toggle 0x294830").or_default() += 1;
        }
    }
    let Some(t) = toggle_mut(m, w) else { return 0 };
    if t.cursor != start { out.sounds.push(MenuSound::Cursor); }
    0
}

/// 0x294cc0.
pub fn camera_update(m: &mut PageMenu, w: u32, inp: &MenuInput, gs: &mut GameState, out: &mut MenuOut) -> i32 {
    if !m.is_focus(w) { return 0; }
    if let Some(r) = keys(m, inp, 1) { return r; }
    let Some(c) = camera_mut(m, w) else { return 0 };
    let start = c.cursor;
    if inp.pressed_u & button::UP != 0 && c.cursor != 0 { c.cursor -= 1; }
    if inp.pressed_u & button::DOWN != 0 && c.entries.get(c.cursor as usize + 1).is_some_and(|e| e.label != 0) { c.cursor += 1; }
    if c.cursor != start { out.sounds.push(MenuSound::Cursor); }
    let e = c.entries.get(c.cursor as usize).copied().unwrap_or_default();
    let mut count = 0;
    if e.strs[0] != 0 {
        for k in 1..=4 {
            count = k;
            if k == 4 || e.strs[k] == 0 { break; }
        }
    }
    if inp.pressed_u & button::CROSS != 0 && e.var != 0 && count != 0 {
        let v = opt_byte(&gs.global, e.var).unwrap_or(0) as i32;
        set_opt_byte(&mut gs.global, e.var, ((v + 1) % count as i32) as u8);
        out.sounds.push(MenuSound::Confirm);
    }
    0
}

/// `SoundOptionsMenu` 0x290538 (not focus-gated).
pub fn sound_update(m: &mut PageMenu, w: u32, inp: &MenuInput, gs: &mut GameState, out: &mut MenuOut) -> i32 {
    if let Some(r) = keys(m, inp, 1) { return r; }
    let pending = m.pages.get(&m.current).is_some_and(|p| p.pending != 0);
    let Some(Data::Sound { cursor }) = m.widgets.get_mut(&w).map(|x| &mut x.data) else { return 0 };
    let start = *cursor;
    if inp.pressed_u & button::UP != 0 { *cursor = (start + 2) % 3; }
    if inp.pressed_u & button::DOWN != 0 { *cursor = (*cursor + 1) % 3; }
    let row = *cursor;
    if row != start || pending { out.sounds.push(MenuSound::Cursor); }
    let g = &mut gs.global;
    let before = (g.effects_volume, g.music_volume);
    if inp.held_u & button::RIGHT != 0 {
        if row == 0 { g.effects_volume = (g.effects_volume + 3).min(0x400); }
        if row == 1 { g.music_volume = (g.music_volume + 3).min(0x400); }
    }
    if inp.held_u & button::LEFT != 0 {
        if row == 0 { g.effects_volume = (g.effects_volume - 3).max(0); }
        if row == 1 { g.music_volume = (g.music_volume - 3).max(0); }
    }
    if before != (g.effects_volume, g.music_volume) { out.sound_settings = true; }
    if inp.pressed_u & button::CROSS != 0 {
        if row == 2 { g.stereo = (g.stereo == 0) as i32; }
        out.sound_settings = true;
        out.sounds.push(MenuSound::Confirm);
    }
    0
}

/// 0x2921d0 (not focus-gated; the close keys ignore `0x1ba294`).
pub fn quit_update(m: &mut PageMenu, _w: u32, inp: &MenuInput, _gs: &mut GameState, out: &mut MenuOut) -> i32 {
    if inp.pressed_u & 0xd00 != 0 { return -1; }
    if inp.pressed_u & button::TRIANGLE != 0 {
        let parent = m.pages.get(&m.current).map_or(0, |p| p.parent);
        if parent != 0 {
            m.target = parent;
        } else if !m.no_close {
            return -1;
        }
    }
    if inp.pressed_u & button::CIRCLE != 0 {
        out.quit = true;
        // 0x15f5c0 = −1, 0x15f570 = 1: the level loop ends for the title.
        out.level_exit = Some(-1);
    }
    0
}

pub(super) fn right(out: &mut Vec<MenuDraw>, a: &MenuAssets, x: i32, y: i32, rgba: u32, t: &[u8]) { text(out, Font::Regular, x - a.width(Font::Regular, t), y, rgba, t); }

/// 0x294a38 (Lombyte `DrawCheatsMenu`).
pub fn toggle_draw(m: &mut PageMenu, w: u32, a: &MenuAssets, gs: &GameState, out: &mut Vec<MenuDraw>) -> u32 {
    let Some(wd) = m.widgets.get(&w) else { return 1 };
    let [_, _, ww, wh] = wd.rect;
    let Data::Toggle(t) = &wd.data else { return 1 };
    let step = wh / (t.entries.len() as i32 + 1);
    let mut y = step - 8;
    for (i, e) in t.entries.iter().enumerate() {
        let col = if i as i32 == t.cursor { YELLOW } else { LIGHT_BLUE };
        let on = e.var != 0 && opt_byte(&gs.global, e.var).unwrap_or(0) != 0;
        text(out, Font::Regular, 0xc, y, col, a.msg(e.label as i32));
        right(out, a, ww - 0xc, y, LIGHT_BLUE, a.msg(if on { e.on } else { e.off } as i32));
        y += step;
    }
    2
}

/// 0x294e68.
pub fn camera_draw(m: &mut PageMenu, w: u32, a: &MenuAssets, gs: &GameState, out: &mut Vec<MenuDraw>) -> u32 {
    let Some(wd) = m.widgets.get(&w) else { return 1 };
    let [_, _, ww, wh] = wd.rect;
    let Data::Camera(c) = &wd.data else { return 1 };
    let step = wh / (c.entries.len() as i32 + 1);
    let mut y = step - 8;
    for (i, e) in c.entries.iter().enumerate() {
        let col = if i as i32 == c.cursor { YELLOW } else { LIGHT_BLUE };
        text(out, Font::Regular, 0xc, y, col, a.msg(e.label as i32));
        let v = opt_byte(&gs.global, e.var).unwrap_or(0) as usize;
        right(out, a, ww - 0xc, y, LIGHT_BLUE, a.msg(e.strs.get(v).copied().unwrap_or(0) as i32));
        y += step;
    }
    2
}

/// `DrawSoundMenu` 0x290808. Slider fill: icon 59806 frame 8 / 9 through `fun_00200958` with UV (0, 0xa0)..
/// (0x1f0, v1); v1 and the alpha are lost in the decompile (the texture's height and 0x80 here) [M].
pub fn sound_draw(m: &mut PageMenu, w: u32, a: &MenuAssets, gs: &GameState, out: &mut Vec<MenuDraw>) -> u32 {
    let Some(wd) = m.widgets.get(&w) else { return 1 };
    let [_, _, ww, wh] = wd.rect;
    let Data::Sound { cursor } = wd.data else { return 1 };
    let (hx, q) = (ww >> 1, wh >> 2);
    let g = &gs.global;
    let col = |row: i32| if cursor == row { YELLOW } else { LIGHT_BLUE };
    for (row, id, vol, frame_k) in [(0, 0x5212, g.effects_volume, 8), (1, 0x5213, g.music_volume, 9)] {
        let y = q * (row + 1);
        right(out, a, hx - 8, y - 8, col(row), a.msg(id));
        out.push(MenuDraw::Rect { x0: hx + 7, y0: y - 8, x1: ww - 0x3f, y1: y + 8, rgba: 0x8069_6969 });
        out.push(MenuDraw::Rect { x0: hx + 9, y0: y - 6, x1: ww - 0x41, y1: y + 6, rgba: 0x8038_3838 });
        let fill = ((ww - (hx + 0x4a)) * vol) / 1024;
        let frame = a.frame(59806, frame_k);
        let th = a.frame_size(frame).1;
        out.push(MenuDraw::SpriteUv { frame, x0: (hx + 9) * 16, y0: (y - 6) * 16, x1: (hx + fill + 8) * 16, y1: (y + 5) * 16, u0: 0, v0: 0xa0, u1: 0x1f0, v1: th * 16, alpha: 0x80, repeat_u: false });
    }
    let y = q * 3 - 8;
    right(out, a, hx - 8, y, col(2), a.msg(0x5214));
    text(out, Font::Regular, hx + 8, y, LIGHT_BLUE, a.msg(if g.stereo == 0 { 0x5215 } else { 0x5216 }));
    2
}

/// `DrawQuitGameMenu` 0x292298.
pub fn quit_draw(m: &mut PageMenu, w: u32, a: &MenuAssets, out: &mut Vec<MenuDraw>) -> u32 {
    let Some(wd) = m.widgets.get(&w) else { return 1 };
    let [_, _, ww, wh] = wd.rect;
    let win = wtext::Window::new(0, wh as i16, 4, (ww - 4) as i16, (ww / 2) as i16, 6, 0x10, wtext::CENTRE_LINES);
    out.push(MenuDraw::Hud(Draw::TextWindow { font: Font::Regular, window: win, rgba: LIGHT_BLUE, text: a.msg(0x4f6d).to_vec() }));
    let q = a.msg(0x4f3f);
    text(out, Font::Regular, ww / 2 - (a.width(Font::Regular, q) >> 1), wh - 0x40, LIGHT_BLUE, q);
    let (no, yes) = (a.msg(0x524b), a.msg(0x524f));
    let x = (ww - a.width(Font::Regular, no).max(a.width(Font::Regular, yes))) >> 1;
    text(out, Font::Regular, x, wh - 0x28, LIGHT_BLUE, no);
    text(out, Font::Regular, x, wh - 0x14, LIGHT_BLUE, yes);
    2
}
