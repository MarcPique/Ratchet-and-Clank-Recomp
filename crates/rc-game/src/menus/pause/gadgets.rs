//! The Gadgets page 0x1b2d68 (kind 4; the Weapons page 0x1b3238 shares its widgets): the icon grids, the item name
//! label, the 3D Ratchet and the item preview (docs/plan/gadgets.md §3; addresses level01.elf).
//!
//! * **Grid** (update 0x28f260, draw 0x291350, enter 0x28f1b0): `+0x30` flags, `+0x34 / +0x38` the margins in
//!   frame-moby units, `+0x3c` cursor, `+0x40` rows, `+0x44` columns, `+0x48` cells (10 bytes: `u16 icon, s16
//!   variant base, s16 kind (0 item, else a global flag), s16 item`), `+0x4c / +0x50 / +0x54 / +0x58` the grids
//!   above / below / left / right. Flags: 1 no ✕, 2 fixed row step, 4 the back packs, 8 the head items (the two the
//!   unusable flags grey out), 0x20 no "equipped" icon (the hand items), 0x8000 no wrap.
//!   The Gadgets page: hand 2×3 {Trespasser, Hydrodisplacer, Swingshot, PDA, Metal Detector, Hologuise}, back 1×3
//!   {Heli, Thruster, Hydro}, head 1×3 {O2 Mask, Sonic Summoner, Pilot's Helmet}, feet 1×2 {Grindboots,
//!   Magneboots}; focus starts on the back packs.
//! * ✕ equips into the page's copy of the saved items ([`crate::inventory::menu_select`]); the close requests every
//!   changed slot ([`crate::inventory::menu_close_requests`], `PageMenuClose`).
//! * **3D Ratchet** (enter 0x297ad0, update 0x297d70 `LoadHandGadget`, draw 0x291800, leave 0x297cc0): Ratchet
//!   (class 0) 4 units in front of the menu camera, 0.6 below it, turned by π, with Clank, the pending hand item,
//!   head item, boots and pack (the Heli-Pack's class 607 on its rotor sequence 6), the Persuader / Map-o-matic /
//!   Bolt Grabber when owned; returned as [`ModelView`] for the engine (draw return 4: aspect crop).
//! * **Item preview** (enter 0x291938, update 0x291c38, draw 0x292010, leave 0x291960): the focused cell's item
//!   (class `+0x10`; the Drone Device 0x18 shows 0x1df) placed from the table 0x1c4988 (0x20 bytes per item: x for
//!   flag 1 / else, y, z, rot x, rot y, the pivot a / b), turned about z by +0x38 (π at the enter, +0.01 rad a
//!   tick), on sequence 1 (6 for the Heli-Pack), with Clank for a back item; only while the item is owned
//!   ([`PreviewView`], draw return 8: centre crop).

use super::super::{MenuAssets, MenuDraw, MenuInput, MenuSound, Overlay};
use super::{Data, MenuOut, PageMenu};
use crate::game_state::GameState;
use crate::inventory::{menu_select, Select};
use crate::pad::button;

/// Widget callbacks (level01).
pub mod func {
    pub const GRID_UPDATE: u32 = 0x28f260;
    pub const GRID_DRAW: u32 = 0x291350;
    pub const GRID_ENTER: u32 = 0x28f1b0;
    pub const MODEL_UPDATE: u32 = 0x297d70;
    pub const MODEL_DRAW: u32 = 0x291800;
    pub const MODEL_ENTER: u32 = 0x297ad0;
    pub const MODEL_LEAVE: u32 = 0x297cc0;
    pub const PREVIEW_UPDATE: u32 = 0x291c38;
    pub const PREVIEW_DRAW: u32 = 0x292010;
    pub const PREVIEW_ENTER: u32 = 0x291938;
    pub const PREVIEW_LEAVE: u32 = 0x291960;
}

/// The Gadgets page (level01).
pub const PAGE: u32 = 0x1b2d68;
/// The item preview table (0x20 bytes per item).
pub const PREVIEW_TABLE: u32 = 0x1c4988;
/// gp−0x68b0 / gp−0x68ac: the grid cell size in frame-moby units (0.45).
pub const CELL_W: u32 = 0x160350;
pub const CELL_H: u32 = 0x160354;
/// The Drone Device and the class its preview shows.
const DRONE: i32 = 0x18;
const DRONE_CLASS: i32 = 0x1df;

/// A grid cell (10 bytes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub icon: u16,
    pub variant: i16,
    /// 0: an item cell (drawn when owned); else a cell shown when the global flag `0x13d388[item]` is set.
    pub kind: i16,
    pub item: i16,
    /// +8: the picture index the streamed image widget shows for the cell (flag 8: the Help pages' tables).
    pub image: i16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub flags: u32,
    pub margin: [f32; 2],
    pub cursor: i32,
    pub rows: i32,
    pub cols: i32,
    pub cells: Vec<Cell>,
    pub above: u32,
    pub below: u32,
    pub left: u32,
    pub right: u32,
    /// gp−0x68b0 / −0x68ac.
    pub cell: [f32; 2],
}

impl Eq for Grid {}

impl Grid {
    pub fn read(ov: &Overlay, a: u32) -> Option<Grid> {
        let u = |o: u32| ov.u32(a + o);
        let f = |x: u32| f32::from_bits(x);
        let (rows, cols) = (u(0x40)? as i32, u(0x44)? as i32);
        let cells_addr = u(0x48)?;
        let n = (rows * cols).max(0) as u32;
        let cells = (0..n)
            .map(|k| {
                let b = cells_addr + 10 * k;
                Cell { icon: ov.u16(b).unwrap_or(0), variant: ov.i16(b + 2).unwrap_or(0), kind: ov.i16(b + 4).unwrap_or(0), item: ov.i16(b + 6).unwrap_or(0), image: ov.i16(b + 8).unwrap_or(0) }
            })
            .collect();
        Some(Grid {
            flags: u(0x30)?,
            margin: [f(u(0x34)?), f(u(0x38)?)],
            cursor: u(0x3c)? as i32,
            rows,
            cols,
            cells,
            above: u(0x4c)?,
            below: u(0x50)?,
            left: u(0x54)?,
            right: u(0x58)?,
            cell: [ov.u32(ov.at(CELL_W)).map_or(0.45, f), ov.u32(ov.at(CELL_H)).map_or(0.45, f)],
        })
    }

    /// The item of the cursor's cell (0 none).
    pub fn item(&self) -> i32 { self.cells.get(self.cursor.max(0) as usize).map_or(0, |c| c.item as i32) }
}

/// The item preview widget's state (+0x34 state, +0x38 angle; the moby it made).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Preview {
    pub state: i32,
    pub angle: f32,
    /// The item shown (its moby class `+0xa6`), −1 none.
    pub class: i32,
    pub item: i32,
    pub flags: u32,
}

impl Eq for Preview {}

/// What the engine draws for the 3D Ratchet widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelView {
    /// The widget's panel (game pixels).
    pub rect: [i32; 4],
    /// The page's copy of the saved items (hand, feet, head, back: `0x1ba1a0`).
    pub equip: [i32; 4],
    /// Owned: the Persuader (item 35, class 0x197), the Map-o-matic (33, 0x266), the Bolt Grabber (34, 0x26a).
    pub extras: [bool; 3],
}

/// What the engine draws for the item preview widget.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewView {
    pub rect: [i32; 4],
    pub item: i32,
    pub o_class: i32,
    /// With Clank (a back item).
    pub clank: bool,
    /// The sequence the moby is cut to (`min(1, or 6 for item 2; count − 1)`, the engine clamps).
    pub seq: u8,
    /// Position relative to the menu camera (game units) and rotation (x, y, z).
    pub offset: [f32; 3],
    pub rot: [f32; 3],
}

/// What the engine draws for the end page's Helpdesk girl (`media::girl_draw`, `MediaMenu::girl_view`): her moby (class `media::GIRL_CLASS`) at
/// the menu camera + `media::GIRL_OFFSET`, turned by π, posed by `anim` (and the blend's snapshot key).
#[derive(Clone, Debug, PartialEq)]
pub struct GirlView {
    pub rect: [i32; 4],
    pub anim: rc_formats::moby_anim::AnimState,
    pub snapshot: Option<rc_formats::moby_anim::MobyFrame>,
}

impl Eq for GirlView {}

/// The 3D widgets of the current page, for the engine (rebuilt by every draw).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GadgetsView {
    pub model: Option<ModelView>,
    pub preview: Option<PreviewView>,
    /// The Weapons page's ammo model (`pages::ammo_model_draw`) and the Items page's gold bolt (`pages::gold_draw`).
    pub ammo: Option<PreviewView>,
    pub gold_bolt: Option<PreviewView>,
}

/// The grid of the page's focused widget (the label and the preview read its cursor cell).
pub fn focused_grid(m: &PageMenu) -> Option<&Grid> {
    let f = m.pages.get(&m.current)?.focus;
    match &m.widgets.get(&f)?.data {
        Data::Grid(g) => Some(g),
        _ => None,
    }
}

/// The item of the focused widget's cursor cell (`*(focus + 0x48) + 10·*(focus + 0x3c) + 6`): a grid's, or the Help
/// pages' icon list's (the same fields).
pub fn focused_item(m: &PageMenu) -> Option<i32> {
    let f = m.pages.get(&m.current)?.focus;
    match &m.widgets.get(&f)?.data {
        Data::Grid(g) => Some(g.item()),
        Data::IconList(l) => Some(l.cells.get(l.cursor.max(0) as usize).map_or(0, |c| c.item as i32)),
        _ => None,
    }
}

fn unusable(m: &PageMenu, flags: u32) -> bool { (m.unusable_head && flags & 8 != 0) || (m.unusable_back && flags & 4 != 0) }

impl PageMenu {
    /// A neighbour chain from `start` (`+0x4c` or `+0x50` link) past the grids the unusable flags disable.
    fn usable_grid(&self, mut w: u32, up: bool) -> u32 {
        for _ in 0..16 {
            let Some(Data::Grid(g)) = self.widgets.get(&w).map(|x| &x.data) else { return w };
            if !unusable(self, g.flags) { return w; }
            let next = if up { g.above } else { g.below };
            if next == 0 { return w; }
            w = next;
        }
        w
    }
}

/// `FUN_0028f1b0(w, refocus)`: on the page's entry (refocus 0), a grid the unusable flags disable passes the focus up
/// its `+0x4c` chain to the first usable grid (page pending focus).
pub fn grid_enter(m: &mut PageMenu, w: u32, refocus: bool) {
    if refocus { return; }
    let Some(Data::Grid(g)) = m.widgets.get(&w).map(|x| &x.data) else { return };
    if !unusable(m, g.flags) { return; }
    let t = m.usable_grid(g.above, true);
    if let Some(p) = m.pages.get_mut(&m.current) { p.pending = t; }
}

/// `FUN_0028f260`: the focused grid's keys (module docs).
pub fn grid_update(m: &mut PageMenu, w: u32, inp: &MenuInput, gs: &mut GameState, out: &mut MenuOut) -> i32 {
    if !m.is_focus(w) { return 0; }
    if let Some(r) = m.generic_keys(inp, false, gs.global.level) { return r; }
    let Some(Data::Grid(g)) = m.widgets.get(&w).map(|x| &x.data) else { return 0 };
    let g = g.clone();
    let keys = inp.pressed_u;
    let (old, cols, rows) = (g.cursor, g.cols.max(1), g.rows);
    let mut cursor = old;
    let mut col = old % cols;
    let mut pending = 0u32;
    // The column a neighbour grid of `tc` columns gets (3 ↔ 5 column pages shift by one).
    let adjust = |col: i32, tc: i32| -> i32 {
        let mut c = col;
        if tc == 5 && cols == 3 { c += 1; }
        if tc == 3 && cols == 5 { c = (c - 1).clamp(0, 2); }
        c
    };
    let mut target_cursor: Option<(u32, i32)> = None;
    if keys & button::UP != 0 {
        if old / cols == 0 {
            if g.above != 0 {
                let t = m.usable_grid(g.above, true);
                pending = t;
                if let Some(Data::Grid(tg)) = m.widgets.get(&t).map(|x| &x.data) {
                    col = adjust(col, tg.cols);
                    let tc = tg.cols.max(1);
                    target_cursor = Some((t, (tg.rows - 1) * tc + col.min(tc - 1)));
                }
            } else if g.flags & 0x8000 == 0 {
                cursor += cols * (rows - 1);
            }
        } else {
            cursor -= cols;
        }
    }
    if keys & button::DOWN != 0 {
        if old / cols + 1 < rows {
            cursor += cols;
        } else if g.below != 0 {
            let t = m.usable_grid(g.below, false);
            pending = t;
            if let Some(Data::Grid(tg)) = m.widgets.get(&t).map(|x| &x.data) {
                col = adjust(col, tg.cols);
                target_cursor = Some((t, col.min(tg.cols.max(1) - 1)));
            }
        } else if g.flags & 0x8000 == 0 {
            cursor -= cols * (rows - 1);
        }
    }
    if keys & button::LEFT != 0 {
        if col == 0 {
            if g.left != 0 { pending = g.left } else if g.flags & 0x8000 == 0 { cursor += cols - 1 }
        } else {
            cursor -= 1;
        }
    }
    if keys & button::RIGHT != 0 {
        if col + 1 < cols {
            cursor += 1;
        } else if g.right != 0 {
            pending = g.right;
        } else if g.flags & 0x8000 == 0 {
            cursor += 1 - cols;
        }
    }
    if let Some((t, c)) = target_cursor {
        if let Some(Data::Grid(tg)) = m.widgets.get_mut(&t).map(|x| &mut x.data) { tg.cursor = c; }
    }
    if pending != 0 {
        if let Some(p) = m.pages.get_mut(&m.current) { p.pending = pending; }
    }
    if let Some(Data::Grid(gm)) = m.widgets.get_mut(&w).map(|x| &mut x.data) { gm.cursor = cursor; }
    let has_pending = m.pages.get(&m.current).is_some_and(|p| p.pending != 0);
    if cursor != old || has_pending { out.sounds.push(MenuSound::Cursor); }
    if keys & button::CROSS != 0 && g.flags & 1 == 0 {
        let id = g.cells.get(cursor.max(0) as usize).map_or(0, |c| c.item as i32);
        let owned = usize::try_from(id).ok().and_then(|i| gs.global.owned.get(i)).is_some_and(|&b| b != 0);
        let slot = m.items.as_ref().and_then(|d| d.get(id)).map_or(-1, |d| d.slot);
        match menu_select(&mut m.equip, id, owned, slot) {
            Select::Denied => out.sounds.push(MenuSound::Denied),
            Select::Drone => {
                // The Drone Device is never put in the slot: in the pause menu (kind 3) its drones are launched
                // (`MenuOut::drone`: the ammo test and the globals are the engine's).
                out.sounds.push(MenuSound::Confirm);
                if m.kind == 3 { out.drone = true; }
            }
            _ => out.sounds.push(MenuSound::Confirm),
        }
    }
    0
}

/// `fun_0021d948` (0x291350): the icons (owned items only; the equipped / gold / unusable variants) and, on the
/// focused grid, the pulsing cursor frame. Panel-local, 1/16 pixel. Returns 2 (1:1 blit).
pub fn grid_draw(m: &mut PageMenu, w: u32, a: &MenuAssets, gs: &GameState, vsync: u32, out: &mut Vec<MenuDraw>) -> u32 {
    let focus = m.pages.get(&m.current).map_or(0, |p| p.focus);
    let Some(wd) = m.widgets.get(&w) else { return 1 };
    let Data::Grid(g) = &wd.data else { return 1 };
    let [_, _, ww, wh] = wd.rect;
    // pvar +0x40 / +0x44 of the widget's frame moby: |c1 − c0|, |c2 − c0| (3D size of the panel).
    let Some((sw, sh)) = m.frames.as_ref().and_then(|f| f.slots.as_ref()).and_then(|s| s.get(wd.moby.max(0) as usize)).map(|fm| {
        let p = |k: usize| [0, 1, 2].map(|i| f32::from_bits(fm.corners[k][i]));
        let d = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        (d(p(1), p(0)), d(p(2), p(0)))
    }) else { return 1 };
    let (cw, ch) = (g.cell[0], g.cell[1]);
    let cols = g.cols;
    let (x0, xstep) = if cols < 2 { ((sw - cw) * 0.5, 0.0) } else { (g.margin[0], cw + ((sw - 2.0 * g.margin[0]) - cw * cols as f32) / (cols - 1) as f32) };
    let rows = g.rows;
    let (y0, ystep) = if g.flags & 2 != 0 {
        (g.margin[1], ch + 0.15)
    } else if rows < 2 {
        ((sh - ch) * 0.5, 0.0)
    } else {
        (g.margin[1], ch + ((sh - 2.0 * g.margin[1]) - ch * rows as f32) / (rows - 1) as f32)
    };
    let s = ((ww.max(wh)) << 4) as f32 / sw.max(sh);
    let (pw, ph) = ((s * cw) as i32, (s * ch) as i32);
    let pulse = ((((vsync & 0x3f) as i32 - 0x20).abs() + 0x40) as u32 * 0x10202) | 0x8000_0000;
    // A rect in 1/16 pixel (`fun_00200e08(..., 1)`): the pixels whose centres it covers.
    let rect = |out: &mut Vec<MenuDraw>, x0: i32, y0: i32, x1: i32, y1: i32, rgba: u32| {
        let c = |v: i32| (v + 15).div_euclid(16);
        out.push(MenuDraw::Rect { x0: c(x0), y0: c(y0), x1: c(x1), y1: c(y1), rgba });
    };
    let mut y = y0;
    for r in 0..rows {
        let mut x = x0;
        let iy = (s * y) as i32;
        for c in 0..cols {
            let k = (r * cols + c) as usize;
            let ix = (s * x) as i32;
            if focus == w && g.cursor == k as i32 {
                rect(out, ix - 0x30, iy - 0x30, ix + pw + 0x30, iy + ph + 0x30, pulse);
                rect(out, ix - 0x10, iy - 0x10, ix + pw + 0x10, iy + ph + 0x10, m.consts.navy);
            }
            let cell = g.cells.get(k).copied().unwrap_or_default();
            let item = cell.item as i32;
            let shown = if cell.kind == 0 {
                usize::try_from(item).ok().and_then(|i| gs.global.owned.get(i)).is_some_and(|&b| b != 0)
            } else {
                usize::try_from(item).ok().and_then(|i| gs.global.flags.get(i)).is_some_and(|&b| b != 0)
            };
            if shown {
                let mut v = 0;
                if cell.kind == 0 {
                    let slot = m.items.as_ref().and_then(|d| d.get(item)).map_or(-1, |d| d.slot);
                    let equipped = usize::try_from(slot).ok().and_then(|t| m.equip.get(t)).is_some_and(|&p| p == item);
                    if equipped { v = (g.flags & 0x20 == 0) as i32; }
                    if v == 0 && gs.global.gold_weapons.get(item.max(0) as usize).is_some_and(|&b| b != 0) { v = 4; }
                    if m.unusable_head && g.flags & 8 != 0 { v = 2; }
                    if m.unusable_back && g.flags & 4 != 0 { v = 2; }
                }
                let frame = a.frame(cell.icon, cell.variant as i32 + v);
                let (tw, th) = a.frame_size(frame);
                out.push(MenuDraw::SpriteUv { frame, x0: ix, y0: iy, x1: ix + pw, y1: iy + ph, u0: 0, v0: 0, u1: tw * 16, v1: th * 16, alpha: 0x80, repeat_u: false });
            }
            x += xstep;
        }
        y += ystep;
    }
    2
}

/// `FUN_00297ad0` / `fun_002242b8`: the 3D Ratchet exists while the page is shown (engine side).
pub fn model_enter(m: &mut PageMenu, w: u32) { m.view_model = Some(w); }
pub fn model_leave(m: &mut PageMenu, w: u32) { if m.view_model == Some(w) { m.view_model = None; } }

/// `fun_0021ddf8` (0x291800): returns 4 (aspect crop); the engine draws the models ([`ModelView`]).
pub fn model_draw(m: &mut PageMenu, w: u32, gs: &GameState) -> u32 {
    let rect = m.widgets.get(&w).map_or([0; 4], |x| x.rect);
    let owned = |i: usize| gs.global.owned.get(i).is_some_and(|&b| b != 0);
    m.view.model = Some(ModelView { rect, equip: m.equip, extras: [owned(35), owned(33), owned(34)] });
    4
}

/// `fun_0021df58` / 0x291938.
pub fn preview_enter(m: &mut PageMenu, w: u32) {
    if let Some(Data::Preview(p)) = m.widgets.get_mut(&w).map(|x| &mut x.data) {
        *p = Preview { state: 0, angle: std::f32::consts::PI, class: -1, item: 0, flags: p.flags };
    }
}
pub fn preview_leave(m: &mut PageMenu, w: u32) {
    if let Some(Data::Preview(p)) = m.widgets.get_mut(&w).map(|x| &mut x.data) { p.class = -1; }
}

/// `FUN_00291c38`: the angle turns 0.01 rad a tick; states 0 → 1 → 2 (make the moby of the focused cell's item) → 3
/// (made; back to 2 when the item's class changes).
pub fn preview_update(m: &mut PageMenu, w: u32) -> i32 {
    let item = focused_item(m).unwrap_or(0);
    let class = if item == DRONE { DRONE_CLASS } else { m.items.as_ref().map_or(-1, |d| d.class(item)) };
    let Some(Data::Preview(p)) = m.widgets.get_mut(&w).map(|x| &mut x.data) else { return 0 };
    p.angle = wrap(p.angle + 0.01);
    match p.state {
        0 => p.state = 1,
        1 if class != -1 => p.state = 2,
        2 => {
            if class == -1 {
                p.state = 1;
            } else {
                (p.class, p.item, p.state) = (class, item, 3);
            }
        }
        3 if p.class != class => p.state = 2,
        _ => {}
    }
    0
}

/// `fun_0021e608` (0x292010): nothing when the focused cell's item is not owned (return 0); else 8 (centre crop), and
/// the engine draws the moby ([`PreviewView`]).
pub fn preview_draw(m: &mut PageMenu, w: u32, a: &MenuAssets, gs: &GameState) -> u32 {
    let item = focused_item(m).unwrap_or(0);
    let owned = usize::try_from(item).ok().and_then(|i| gs.global.owned.get(i)).is_some_and(|&b| b != 0);
    let Some(wd) = m.widgets.get(&w) else { return 0 };
    let Data::Preview(p) = &wd.data else { return 0 };
    if !owned || p.state != 3 { return if owned { 8 } else { 0 }; }
    let t = |k: u32| a.overlay.u32(a.overlay.at(PREVIEW_TABLE) + 0x20 * p.item as u32 + 4 * k).map_or(0.0, f32::from_bits);
    let x = if p.flags & 1 == 0 { t(1) } else { t(0) };
    let (pa, pb) = (t(6), t(7));
    let (sn, cs) = p.angle.sin_cos();
    // FUN_002920a0: the position about the pivot (a, b) turned by the angle.
    let offset = [x - pb * sn + pa * cs, t(2) + pb * cs + pa * sn, t(3)];
    let slot = m.items.as_ref().and_then(|d| d.get(p.item)).map_or(-1, |d| d.slot);
    m.view.preview = Some(PreviewView {
        rect: wd.rect,
        item: p.item,
        o_class: p.class,
        clank: slot == 3,
        seq: if p.item == 2 { 6 } else { 1 },
        offset,
        rot: [t(4), t(5), p.angle],
    });
    8
}

/// `fast_add_rotations`: into [−π, π].
fn wrap(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let mut a = a % TAU;
    if PI < a { a -= TAU } else if a < -PI { a += TAU }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(rows: i32, cols: i32, items: &[i16]) -> Grid {
        Grid {
            flags: 0,
            margin: [0.1, 0.1],
            cursor: 0,
            rows,
            cols,
            cells: items.iter().map(|&i| Cell { icon: 60000 + i as u16, variant: 0, kind: 0, item: i, image: 0 }).collect(),
            above: 0,
            below: 0,
            left: 0,
            right: 0,
            cell: [0.45, 0.45],
        }
    }

    #[test]
    fn cursor_cell_item() {
        let mut g = grid(2, 3, &[26, 22, 12, 32, 27, 31]);
        assert_eq!(g.item(), 26);
        g.cursor = 5;
        assert_eq!(g.item(), 31);
    }

    #[test]
    fn angle_wraps() {
        assert!((wrap(std::f32::consts::PI + 0.01) + std::f32::consts::PI - 0.01).abs() < 1e-5);
    }
}
