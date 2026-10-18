//! A direct desktop-exit command in the two root menus. The command participates in
//! the native list focus chain; its panel follows the last disc frame.

use super::{func, lf, port, Data, Item, List, MenuAssets, MenuDraw, PageMenu, Widget, LIGHT_BLUE, NAVY, SHADOW, YELLOW};
use crate::menus::text_plain;
use rc_formats::font::Font;

pub const ACTION: i16 = 12;
const PAUSE_BUTTON: u32 = 0x7f00_0300;
const FRONT_BUTTON: u32 = 0x7f00_0304;
const PAUSE_SLOT: usize = 7;
const FRONT_SLOT: usize = 5;

fn button(addr: u32, above: u32, below: u32) -> Widget {
    Widget {
        addr,
        update: func::LIST_UPDATE,
        draw: 0,
        enter: 0,
        leave: 0,
        dflags: 0,
        moby: 0,
        rect: [0; 4],
        raw: [0; 8],
        data: Data::List(List {
            flags: lf::SMALL,
            items_addr: 0,
            items: vec![Item { label: port::text::EXIT_DESKTOP as i16, action: ACTION, arg: 0, sublabel: 0, hl: 0 }],
            above,
            below,
            cursor: 0,
            scroll: 0,
        }),
    }
}

fn list_mut(menu: &mut PageMenu, addr: u32) -> Option<&mut List> {
    match &mut menu.widgets.get_mut(&addr)?.data {
        Data::List(list) => Some(list),
        _ => None,
    }
}

/// Attach both commands once, preserving every original page and menu entry.
pub fn install(menu: &mut PageMenu) -> bool {
    if menu.widgets.contains_key(&PAUSE_BUTTON) { return true; }
    let Some(pause) = menu.pages.get(&menu.addrs.root) else { return false };
    let Some(front) = menu.pages.get(&menu.addrs.kind2d) else { return false };
    let pause_first = pause.widgets[0];
    let front_first = front.widgets[2];
    let front_last = front.widgets[4];
    if pause.widgets[PAUSE_SLOT] != 0 || front.widgets[FRONT_SLOT] != 0 ||
        ![pause_first, menu.addrs.options, menu.addrs.goodies, front_first, front_last]
            .iter().all(|w| matches!(menu.widgets.get(w).map(|w| &w.data), Some(Data::List(_)))) { return false; }

    menu.pages.get_mut(&menu.addrs.root).unwrap().widgets[PAUSE_SLOT] = PAUSE_BUTTON;
    menu.pages.get_mut(&menu.addrs.kind2d).unwrap().widgets[FRONT_SLOT] = FRONT_BUTTON;
    menu.widgets.insert(PAUSE_BUTTON, button(PAUSE_BUTTON, menu.addrs.options, pause_first));
    menu.widgets.insert(FRONT_BUTTON, button(FRONT_BUTTON, front_last, front_first));
    if let Some(list) = list_mut(menu, front_first) { list.above = FRONT_BUTTON; }
    if let Some(list) = list_mut(menu, front_last) { list.below = FRONT_BUTTON; }
    wire_pause(menu);
    true
}

/// The disc rewires Options/Goodies each time the pause menu opens.
pub fn wire_pause(menu: &mut PageMenu) {
    if !menu.widgets.contains_key(&PAUSE_BUTTON) { return; }
    let first = menu.pages.get(&menu.addrs.root).map_or(0, |p| p.widgets[0]);
    let last = if menu.goodies { menu.addrs.goodies } else { menu.addrs.options };
    if let Some(list) = list_mut(menu, first) { list.above = PAUSE_BUTTON; }
    if let Some(list) = list_mut(menu, last) { list.below = PAUSE_BUTTON; }
    if let Some(list) = list_mut(menu, PAUSE_BUTTON) { list.above = last; }
}

fn rect(out: &mut Vec<MenuDraw>, x: i32, y: i32, w: i32, h: i32, rgba: u32) {
    out.push(MenuDraw::Rect { x0: x, y0: y, x1: x + w, y1: y + h, rgba });
}

fn line(out: &mut Vec<MenuDraw>, x0: i32, y0: i32, x1: i32, y1: i32, rgba: u32) {
    out.push(MenuDraw::Line { x0, y0, x1, y1, rgba });
}

/// The same glass and metal treatment is used for the exit button and its confirmation.
fn frame_panel(out: &mut Vec<MenuDraw>, x: i32, y: i32, w: i32, h: i32) {
    rect(out, x - 1, y - 1, w + 2, h + 2, 0x6030_302e);
    rect(out, x, y, w, h, 0x807c_9a9e);
    rect(out, x + 2, y + 2, w - 4, h - 4, 0x8037_4146);
    rect(out, x + 4, y + 4, w - 8, h - 8, NAVY);
    line(out, x + 1, y + 1, x + w - 2, y + 1, 0x80a3_b4b8);
    line(out, x + 1, y + 1, x + 1, y + h - 2, 0x809c_acb1);
    line(out, x + 3, y + 3, x + w - 4, y + 3, 0x807a_8b90);
    line(out, x + 3, y + 3, x + 3, y + h - 4, 0x806e_7e83);
    line(out, x + 3, y + h - 3, x + w - 3, y + h - 3, 0x8063_7074);
    line(out, x + w - 3, y + 3, x + w - 3, y + h - 3, 0x8065_7275);
    for row in ((y + 7)..(y + h - 6)).step_by(2) {
        line(out, x + 7, row, x + w - 7, row, 0x242b_2b35);
    }
}

fn centred_text(out: &mut Vec<MenuDraw>, assets: &MenuAssets, label: &[u8], x: i32, y: i32, w: i32, h: i32, colour: u32) {
    let font = if assets.width(Font::Large, label) <= w - 20 { Font::Large } else { Font::Regular };
    let tx = x + (w - assets.width(font, label)) / 2;
    let ty = y + (h - if font == Font::Large { 14 } else { 12 }) / 2 - 1;
    text_plain(out, font, tx + 1, ty + 1, SHADOW, label);
    text_plain(out, font, tx, ty, colour, label);
}

/// Draw after the disc panels, using their measured size and spacing in 512x416 pixels.
pub fn draw(menu: &PageMenu, assets: &MenuAssets, out: &mut Vec<MenuDraw>) {
    let (button, source_slot) = if menu.current == menu.addrs.root {
        (PAUSE_BUTTON, if menu.goodies { 6 } else { 5 })
    } else if menu.current == menu.addrs.kind2d {
        (FRONT_BUTTON, 4)
    } else { return };
    if !menu.widgets.contains_key(&button) { return; }
    let Some(page) = menu.pages.get(&menu.current) else { return };
    let Some(source) = menu.widgets.get(&page.widgets[source_slot]) else { return };
    let [x, sy, w, native_h] = source.rect;
    if w <= 0 || native_h <= 0 { return; }
    let y = sy + native_h + 9;
    let h = native_h.min(410 - y);
    if h < 24 { return; }
    frame_panel(out, x, y, w, h);

    let selected = page.focus == button;
    let colour = if selected { YELLOW } else { LIGHT_BLUE };
    let label = assets.msg(port::text::EXIT_DESKTOP);
    centred_text(out, assets, label, x, y, w, h, colour);
}

/// Confirmation over either root menu. No is selected first, and Triangle/Circle cancel.
pub fn draw_confirm(menu: &PageMenu, assets: &MenuAssets, out: &mut Vec<MenuDraw>) {
    let Some(yes) = menu.desktop_exit_confirm else { return };
    if menu.current != menu.addrs.root && menu.current != menu.addrs.kind2d { return; }
    rect(out, 1, 1, 510, 414, 0x5800_0000);
    let (x, y, w, h) = (90, 121, 332, 174);
    frame_panel(out, x, y, w, h);
    let question = assets.msg(port::text::EXIT_CONFIRM);
    let font = if assets.width(Font::Regular, question) <= w - 24 { Font::Regular } else { Font::Small };
    let tx = x + (w - assets.width(font, question)) / 2;
    text_plain(out, font, tx + 1, y + 44, SHADOW, question);
    text_plain(out, font, tx, y + 43, LIGHT_BLUE, question);
    for (choice, cy, raw_label) in [(false, 197, assets.msg(0x524b)), (true, 245, assets.msg(0x524f))] {
        // The disc's prompts include controller symbols for its original quit page.
        // This dialog uses CROSS for either selected answer, so show the words alone.
        let label = &raw_label[raw_label.iter().take_while(|&&b| b < 0x20 || b == b' ').count()..];
        frame_panel(out, 143, cy, 226, 37);
        centred_text(out, assets, label, 143, cy, 226, 37, if yes == choice { YELLOW } else { LIGHT_BLUE });
    }
}
