//! The map (Select, page 0x1b3998), the ship's planet select (0x1b6508) and its confirm page (0x1b6878):
//! the planet list (enter 0x295208, draw 0x290eb0; its update is the list widget's), the galaxy map (update
//! 0x295310, draw 0x295338 → `FUN_002252d0`), the map widget's keys (0x28f868, Lombyte `DrawMapScreen`), the
//! confirm keys (0x295370) and the missions widget's planet stepping (0x28fec8). Spec: docs/plan/menus.md §3
//! "Map", "Planet select", "Confirm".
//!
//! The missions list, the streamed pictures 0x293398 / 0x293670, the globe 0x294258 and the confirm page's gold-bolt panel
//! 0x292980 are [`super::map_page`]'s.

use super::super::{sprite, text, text_plain, MenuAssets, MenuDraw, MenuInput, MenuSound};
use super::{list_font, lf, Data, Item, List, MenuEnv, MenuOut, PageMenu, DISABLED, DISABLED_SELECTED, LIGHT_BLUE, YELLOW};
use crate::game_state::GameState;
use crate::hud::{text as wtext, Draw};
use crate::pad::button;
use rc_formats::font::Font;

/// Galaxy-map icon 59802: frame 12 dot, 13 ring, 14 map, 15 scrolling overlay.
pub const MAP_ICON: u16 = 59802;
/// Blink: on for ScaleTicks(22) of every ScaleTicks(22) + ScaleTicks(8) frames.
pub const BLINK_ON: i32 = 22;
pub const BLINK_OFF: i32 = 8;
pub const LINE_WHITE: u32 = 0x80f0_f0f0;

/// The enter 0x295208: one item per id of the unlocked list 0x13d510 (≤ 20, acquisition order), label =
/// location, sub-label = planet, action 1; the cursor on `dest`; flag 0x8000 (jump scroll on the next draw).
pub fn build_list(l: &mut List, gs: &GameState, dest: i32, names: &[(u32, u32)]) {
    l.items.clear();
    for &id in gs.global.map_order.iter().take(20) {
        if id == 0 { break; }
        let (loc, planet) = usize::try_from(id).ok().and_then(|i| names.get(i)).copied().unwrap_or((0, 0));
        l.items.push(Item { label: loc as u16 as i16, action: 1, arg: 0, sublabel: planet as u16 as i16, hl: 0 });
    }
    l.cursor = 0;
    l.flags |= lf::JUMP_SCROLL;
    if let Some(k) = gs.global.map_order.iter().take_while(|&&id| id != 0).position(|&id| id == dest) { l.cursor = k as i32; }
}

/// The planet list draw 0x290eb0 (panel-local). The sub-label's x anchor is lost in the decompile: 20 [M].
pub fn list_draw(m: &mut PageMenu, w: u32, a: &MenuAssets, out: &mut Vec<MenuDraw>) -> u32 {
    let focus = m.pages.get(&m.current).map_or(0, |p| p.focus);
    let swapped = m.text_swap == 2;
    let Some(wd) = m.widgets.get_mut(&w) else { return 1 };
    let [_, _, ww, wh] = wd.rect;
    let Data::List(l) = &mut wd.data else { return 1 };
    let (font, size) = list_font(l.flags);
    let n = l.items.len() as i32;
    let step = if l.flags & lf::STEP_FROM_FONT != 0 { size + 3 } else { wh / (n + 1) };
    let glyphs = &a.hud.glyphs[font as usize];
    let lh = size + 2;
    let mut win = wtext::Window::new(4, (wh - 4) as i16, 0, (ww - 2) as i16, 4, (step - size / 2 - l.scroll) as i16, lh as i16, 0);
    for (i, it) in l.items.clone().iter().enumerate() {
        let sel = focus == w && l.cursor == i as i32;
        let col = if l.flags & lf::FIXED_COLOUR != 0 {
            LIGHT_BLUE
        } else if it.action != 0 {
            if sel { YELLOW } else { LIGHT_BLUE }
        } else if sel {
            DISABLED_SELECTED
        } else {
            DISABLED
        };
        if l.flags & 0x10000 == 0 && sel && win.y_start <= 3 { l.scroll -= 4; }
        let mut anchor = 4;
        if l.flags & 0xa00 != 0 { anchor = 0x20; }
        if l.flags & 0x400 != 0 {
            win.flags = wtext::CENTRE_LINES;
            anchor = ww >> 1;
        }
        win.x_anchor = anchor as i16;
        let label = a.msg_in(it.label as i32, swapped);
        let mut m1 = win;
        wtext::layout(&mut m1, label, -1, glyphs, !sel);
        push_window(out, font, win, col, label, sel);
        let mut y = win.y_start as i32 + m1.height as i32;
        let mut last_h = m1.height as i32;
        if it.sublabel != 0 {
            let sub = a.msg_in(it.sublabel as i32, swapped);
            let mut w2 = win;
            w2.y_start = y as i16;
            w2.x_anchor = 20;
            let mut m2 = w2;
            wtext::layout(&mut m2, sub, -1, glyphs, !sel);
            push_window(out, font, w2, col, sub, sel);
            last_h = m2.height as i32;
            y += step;
        }
        win.y_start = (y + 8) as i16;
        if l.flags & 0x10000 == 0 && sel {
            let bottom = y + 8 + last_h;
            if (win.y_max as i32) < bottom {
                l.scroll += if l.flags & lf::JUMP_SCROLL != 0 { bottom - win.y_max as i32 } else { 4 };
            }
        }
    }
    if l.flags & lf::JUMP_SCROLL != 0 {
        l.flags ^= lf::JUMP_SCROLL;
        return 1;
    }
    2
}

fn push_window(out: &mut Vec<MenuDraw>, font: Font, win: wtext::Window, rgba: u32, t: &[u8], codes_off: bool) {
    let t: Vec<u8> = if codes_off { t.iter().copied().filter(|c| c.wrapping_sub(8) >= 8).collect() } else { t.to_vec() };
    out.push(MenuDraw::Hud(Draw::TextWindow { font, window: win, rgba, text: t }));
}

/// Galaxy-map dot state of level `lvl`: 3 visited (steady), 2 known (blinking), 0 hidden.
pub fn planet_state(gs: &GameState, lvl: usize) -> u8 {
    if gs.levels.get(lvl).is_some_and(|l| l.visited != 0) {
        3
    } else if gs.global.planet_unlocked.get(lvl).is_some_and(|&b| b != 0) {
        2
    } else {
        0
    }
}

/// The blink: on while `vsync % (22 + 8) < 22`.
pub fn blink_on(vsync: u32) -> bool {
    let on = super::super::scale_ticks(BLINK_ON);
    let period = on + super::super::scale_ticks(BLINK_OFF);
    ((vsync % period as u32) as i32) < on
}

/// `FUN_002252d0` through the galaxy draw 0x295338 (panel-local).
pub fn galaxy_draw(m: &mut PageMenu, w: u32, a: &MenuAssets, gs: &GameState, env: &MenuEnv, out: &mut Vec<MenuDraw>) -> u32 {
    let Some(wd) = m.widgets.get(&w) else { return 1 };
    let [_, _, ww, wh] = wd.rect;
    out.push(MenuDraw::Rect { x0: 0, y0: 0, x1: 0x200, y1: 0x1c0, rgba: 0x8000_0000 });
    let f14 = a.frame(MAP_ICON, 0xe);
    let (tw, th) = a.frame_size(f14);
    out.push(MenuDraw::SpriteUv { frame: f14, x0: 0, y0: 0, x1: ww * 16, y1: wh * 16, u0: 0, v0: 0, u1: tw * 16, v1: th * 16, alpha: 0x80, repeat_u: false });
    let f15 = a.frame(MAP_ICON, 0xf);
    let (tw, th) = a.frame_size(f15);
    let u = (env.vsync & 0xfff) as i32;
    out.push(MenuDraw::SpriteUv { frame: f15, x0: 0, y0: 0, x1: ww * 16, y1: wh * 16, u0: u, v0: 0, u1: u + tw * 16, v1: th * 16, alpha: 0x80, repeat_u: true });
    for lvl in 1..=19usize {
        let Some(&[x, y0, dx, dy]) = m.planet_points.get(lvl - 1) else { continue };
        if x == 0 { continue; }
        let st = planet_state(gs, lvl);
        if st == 0 { continue; }
        let y = if env.pal { y0 * 0x1c0 / 0x1a0 } else { y0 };
        if st == 3 || blink_on(env.vsync) { sprite(out, a.frame(MAP_ICON, 0xc), x - 5, y - 5, 10, 10, 0x80); }
        if lvl as i32 != m.dest { continue; }
        let (ex, ey) = (x + dx, y + dy);
        let len = ((dx as f32) * (dx as f32) + (dy as f32) * (dy as f32)).sqrt();
        let l = (len * 1000.0) as i32;
        if l == 0 { continue; }
        let sx = ex + ((x - ex) * (l - 8000)) / l;
        let sy = ey + ((y - ey) * (l - 8000)) / l;
        let name = a.msg(m.level_names.get(lvl).map_or(0, |n| n.1) as i32);
        let mut nw = a.width(Font::Regular, name);
        if dx < 0 { nw = -nw; }
        let nx = ex + nw;
        let tx = ex.min(nx);
        out.push(MenuDraw::Line { x0: sx + 1, y0: sy + 1, x1: ex + 1, y1: ey + 1, rgba: 0x8000_0000 });
        out.push(MenuDraw::Line { x0: ex + 1, y0: ey + 1, x1: nx + 1, y1: ey + 1, rgba: 0x8000_0000 });
        text_plain(out, Font::Regular, tx + 1, ey - m.name_dy + 1, 0x8000_0000, name);
        out.push(MenuDraw::Line { x0: sx, y0: sy, x1: ex, y1: ey, rgba: LINE_WHITE });
        out.push(MenuDraw::Line { x0: ex, y0: ey, x1: nx, y1: ey, rgba: LINE_WHITE });
        text(out, Font::Regular, tx, ey - m.name_dy, LINE_WHITE, name);
        sprite(out, a.frame(MAP_ICON, 0xd), x - 10, y - 10, 20, 20, 0x80);
    }
    2
}

/// The map widget's update 0x28f868 (not focus-gated; +0x34 & 0x40 = passive, streaming only).
pub fn map_update(m: &mut PageMenu, w: u32, inp: &MenuInput, gs: &mut GameState, out: &mut MenuOut) -> i32 {
    *m.stub_calls.entry("map stream/compose 0x28f868").or_default() += 1;
    let passive = matches!(m.widgets.get(&w).map(|x| &x.data), Some(Data::Map { passive: true }));
    if passive { return 0; }
    let old = m.dest;
    if inp.pressed_u & 0xd00 != 0 && !m.no_close { return 1; }
    if inp.pressed_u & button::TRIANGLE != 0 {
        let parent = m.pages.get(&m.current).map_or(0, |p| p.parent);
        if parent != 0 {
            m.target = parent;
        } else if !m.no_close {
            return -1;
        }
    }
    if inp.pressed_u & button::CROSS != 0 { m.target = m.addrs.map_missions; }
    if inp.pressed_u & button::CIRCLE != 0 && m.dest != 0 {
        m.arg = m.dest as u32;
        m.return_page = m.addrs.map;
        m.return_kind = 0xb;
        m.post = 3;
        out.sounds.push(MenuSound::Confirm);
        return 0;
    }
    let order = &gs.global.map_order;
    let k = order.iter().take_while(|&&v| v != 0).position(|&v| v == m.dest);
    if let Some(k) = k {
        if inp.pressed_u & button::R1 != 0 && k < 0x13 && order[k + 1] != 0 { m.dest = order[k + 1]; }
        if inp.pressed_u & button::L1 != 0 && k != 0 && order[k - 1] != 0 { m.dest = order[k - 1]; }
    }
    if m.dest != old {
        out.sounds.push(MenuSound::Cursor);
        super::map_page::dest_changed(m, gs);
    }
    0
}

/// The confirm page's keys 0x295370: △ back, Start = stay (dest = level) and close, ✕ blast off (close; kind
/// 0x10 turns it into post-action 2), ○ the Infobot movie (post 3, back to this page in kind 0xf).
pub fn confirm_update(m: &mut PageMenu, inp: &MenuInput, gs: &mut GameState, out: &mut MenuOut) -> i32 {
    if inp.pressed_u & button::TRIANGLE != 0 {
        let parent = m.pages.get(&m.current).map_or(0, |p| p.parent);
        if parent == 0 { return if m.no_close { 0 } else { -1 }; }
        m.target = parent;
        return 0;
    }
    if inp.pressed_u & button::START != 0 {
        m.dest = gs.global.level;
        return 1;
    }
    if inp.pressed_u & button::CROSS != 0 {
        out.sounds.push(MenuSound::Confirm);
        return 1;
    }
    if inp.pressed_u & button::CIRCLE != 0 {
        m.arg = m.dest as u32;
        m.return_page = m.current;
        m.return_kind = 0xf;
        m.post = 3;
        out.sounds.push(MenuSound::Confirm);
    }
    0
}

/// `FUN_0028fec8` (the missions widget of the missions page and the ship's confirm page; `map_page` has its list):
/// unfocused, the destination's cursor = −1; focused: Start / Select / R3 close, R1 / L1 step to the next / previous
/// known planet (stopping at the current level; sound 1 and the destination's mission status), Up / Down move the
/// mission cursor ([`super::map_page::missions_keys`]), △ back (or close at a root).
pub fn missions_update(m: &mut PageMenu, w: u32, inp: &MenuInput, gs: &mut GameState, out: &mut MenuOut) -> i32 {
    if !m.is_focus(w) {
        super::map_page::missions_unfocused(m, w);
        return 0;
    }
    if inp.pressed_u & 0xd00 != 0 && !m.no_close { return 1; }
    let (old, level) = (m.dest, gs.global.level);
    let known = |i: i32| usize::try_from(i).ok().and_then(|i| gs.global.planet_unlocked.get(i)).is_some_and(|&b| b != 0);
    let mut d = m.dest;
    if inp.pressed_u & button::R1 != 0 {
        let mut i = d;
        loop {
            let n = i + 1;
            if n > 0x13 {
                d = m.dest;
                break;
            }
            d = n;
            if known(i + 1) || level == n { break; }
            i = n;
        }
    }
    let base = d;
    if inp.pressed_u & button::L1 != 0 {
        let mut i = base;
        loop {
            let n = i - 1;
            if n < 0 {
                d = base;
                break;
            }
            d = n;
            if known(i - 1) || level == n { break; }
            i = n;
        }
    }
    m.dest = d;
    let changed = m.dest != old;
    if changed {
        out.sounds.push(MenuSound::Cursor);
        super::map_page::dest_changed(m, gs);
    }
    super::map_page::missions_keys(m, w, inp.pressed_u, changed, out);
    if inp.pressed_u & button::TRIANGLE != 0 {
        let parent = m.pages.get(&m.current).map_or(0, |p| p.parent);
        if parent != 0 {
            m.target = parent;
        } else if !m.no_close {
            return -1;
        }
    }
    0
}
