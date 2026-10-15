//! The page menu's front-end media parts (docs/plan/progression.md `## media`): the Goodies unlocks (`FUN_0029aa60`,
//! run by every `EnterMenuMode`), the Cheats page's list (enter 0x28dbe8, the toggle list 0x294830 on the cheat bytes),
//! the debug code entry `MenuInput` 0x298f80 (every page-menu tick: `crate::cheats::CodeEntry`), and the end-of-game page
//! 0x1b7670 (kind 0x21 → 0x22, no close keys) that the boss 1422 opens after the ending movie: its statistics widget
//! (update 0x295c98, draw 0x2964a0) and the Helpdesk girl widget (0x2992f0 / 0x299670 / 0x299190 / 0x299268).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x28bf50 tail → 0x29aa60 | skill points (0x13d408[32], ≤ 30) > 14: Sketchbook action 3 (else 2 "????"); = 30: Epilogue 3 (else 2); gold weapons (0x13e520[37], ≤ 10) = 10: Making Of action 10 (else 2), Commercials 3 (else 2); the Goodies description table 0x1b6df8 [5..8] = 0x4fd3 / 0x4fd9, 0x4fd4 / 0x4fda, 0x4fd7 / 0x4fdb, 0x4fd8 / 0x4fdb | [`goodies_unlocks`] |
//! | 0x28bf50 0x1ba248 | Goodies shown = 0x15eea0 ‖ 0x15ee20 ‖ 0x1ba268 | `PageMenu::enter` (0x1ba268: no writer found in the level-01 code: [`MediaMenu::goodies_extra`], false) [L] |
//! | 0x28dbe8 | the Cheats list (`crate::cheats::entries`), entries `{label, &0x15edb0[slot], 0x4f5a, 0x4f5b, 0}` | [`cheats_enter`] |
//! | 0x294830 ✕ (entry flag 0) | the cheat byte flipped (`*flag = *flag == 0`) | `options::toggle_update` (`options::set_opt_byte` maps 0x15edb0..0x15edbb) |
//! | 0x298f80 | the code entry and its effects | `PageMenu::tick` ([`MediaMenu::code`], `crate::cheats::apply_code`) |
//! | 0x295c98 ○ (0x13cae4 & 0x20) | memory card status 0x15eeb0 ∈ {1, 0x10}: target = the save page 0x1b9440; else `fun_00226b08(−1)` (challenge mode), `FUN_002a29a0(0)` (0x15f5c0 = 0, 0x15f570 = 1: leave for Veldin), 0x13e05a = 1 | [`stats_update`] ([`EndChoice::Challenge`], `MenuOut::level_exit`) |
//! | 0x295c98 ✕ (0x40) | 0x14bf08[4] (level 18's gold bolts) and 0x13d425 kept across `memcard_RestoreGame(0x1ba250)`, 0x15eea0 = 1, `memcard_Save(0, −1)`, `FUN_0029abc0` (the checkpoint records 0x1baa50 / 0x1bb6b0 / 0x1ba950 cleared), `FadeToBlack(ticks(16))`, 0x141401 = 1, return −1 (close, post-action 1) | [`stats_update`] ([`EndChoice::Timewarp`]: the restore and the save are the `saves` lane's) |
//! | 0x2964a0 | rows h/5 apart: "You've got:" centred at (w/2, h/5 − 8), then gold bolts (0x14bec0[20·4], ≤ 40), gold weapons (≤ 10), skill points (≤ 30) as "%d of %d …" at (0x14, y) with a check box at (0xb, y + 9) ticked when complete; the small font when the widest line + 0x18 > w; colour codes off; 0x80ffa888 | [`stats_draw`] |
//! | 0x295de0 / 0x295df8 / 0x296170 | the kind-0x23 page 0x1b6fb8's scrolling text widget (`EnterMenuMode(0x23)`: no caller in the boot or the 19 overlays; 0x1ba24c = kind 0x23): see [`scroller_update`] | [`scroller_enter`] / [`scroller_update`] / [`scroller_draw`] |
//! | 0x2932f0 | the check box: 0x80ffa888 (x ± 5, y ± 5), navy *0x160270 (x ± 4, y ± 4), ticked: icon 0xe99e frame 1, 30×30 at (x − 13, y − 18), alpha 0x80 | [`check_box`] |
//!
//! **The Helpdesk girl** (the end page's widget 0x1b76f8; G-CUT-008). Her moby (class 0x7a5) stands 2.2 ahead of the
//! menu camera and 1.6 below it, turned by π, and is drawn into the widget's panel (`DrawMobyList`, return 4: aspect
//! crop; the engine's `menu_models`). Her talking sequences 1..3 are not in the level: they are read from the global
//! lump `post_credits_helpdesk_girl_seq` (TOC 0x1610: WAD, a table of three 8-byte entries whose first word is the
//! offset of the sequence, each sequence's pointers relative to itself) into the class's sequence slots 1..3. Her
//! moby state +0x20 runs 0 → 1 → 2 → 3 → 6 → 0 … on a first ending (times completed 0x15ee20 = 0), else 4 → 5 → 6 → 4 …:
//! the even states 0 / 2 / 4 request line k = state / 2 (0x1516ec = 60000 + 6·k + max(language − 1, 0), `play_dialogue`
//! ≥ 60000 → `post_credits_audio[n − 60000]`, never muted by the HelpDesk voice option) and wait `ticks(120)` and the
//! line loaded (0x15172a = 3), then blend (`ticks(24)`) into sequence k + 1; the odd states start the line once the
//! blend has landed (+0x52 = +0x53, `continue_audio_stream_if_ready`) and, when the sequence ends (+0x70 & 2), blend
//! back to sequence 0 and go to 2 (after 1) or 6; state 6 waits `ticks(240)`.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x299190 enter | `SpawnHandGadgetMoby(0x7a5)` (the class loaded or nothing: +0x34 = 3): +0x44 = moby, +0x34 = 0; the moby: +0x34 (u16) = 0, at the menu camera 0x167240 + (2.2, 0, −1.6), rot z = π, update 0x2996c0 (empty), pvars {widget, 0, 0}; `select_next_stream_buffer(1)` into +0x3c (none: +0x34 = 3) | [`girl_enter`] (the class: [`MediaMenu::girl_class`], given by the engine; the stream buffers: n/a, memory) |
//! | 0x2992f0 +0x34 = 0 | 0x1516d8 (a stream read pending) → wait; `start_audio_stream_read(buffer + 0x4f000 − size·0x800, sector 0x139190)`: started → +0x40 = that offset, +0x34 = 1, 0x1ba23b = 1, the buffer flagged (`fun_00225dd8`); else +0x34 = 3 | [`girl_update`] (the read is the engine's and never pends: ready on the next frame [L]) |
//! | 0x2992f0 +0x34 = 1 | 0x1516d8 → wait; 0x1ba23b = 0; WAD-decompress into the buffer; sequences 1..3 of the class = buffer + the table's offsets, relocated (`relocate_asset_entry_pointers`); +0x34 = 2; the moby's state = 0, or 4 when 0x15ee20 ≠ 0; +0xbc = 0 | [`girl_update`] (the splice: the engine's [`MediaMenu::girl_class`]) |
//! | 0x2992f0 states 0 / 2 / 4 | +0xbc = 0: 0x1516ec = 60000 + 6·(state / 2) + max(0x15ed88 − 1, 0); +0xbc + 1, clamped to ticks(120); below ticks(120) or 0x15172a ≠ 3: wait; else +0xbc = 0, state + 1, `fun_00212f90(moby, state / 2 + 1, 0, ticks(24))` | [`girl_update`] ([`MenuOut::voice`]) |
//! | 0x2992f0 states 1 / 3 / 5 | +0xbc = 0 and +0x52 = +0x53: `continue_audio_stream_if_ready`, +0xbc = 1; +0x70 & 2: state = 2 after 1, else 6; +0xbc = 0; `fun_00212f90(moby, 0, 0, ticks(24))` | [`girl_update`] ([`MenuOut::voice_continue`]) |
//! | 0x2992f0 state 6 | +0xbc + 1 clamped to ticks(240); at ticks(240): state 0 (or 4 when 0x15ee20 ≠ 0), +0xbc = 0 | [`girl_update`] |
//! | MobyUpdateLoop 0x2793d8 | the girl's `MobyAnimAdvance` every menu frame (after the widget updates) | [`girl_tick`] |
//! | 0x299670 draw | `DrawMobyList(+0x44[i], 1)` for the 24 slots; return 4 | [`girl_draw`] (`MediaMenu::girl_view`, drawn by the engine) |
//! | 0x299268 leave | the 24 mobys deleted (`FUN_00298f38`), the buffer released (`complete_stream_buffer_transfer`); 0x15172a ∉ {6, 7} → 5 (the line stopped) | [`girl_leave`] ([`MenuOut::voice_stop`]) |

use super::super::{MenuAssets, MenuDraw, MenuInput};
use super::{Data, MenuOut, PageMenu, LIGHT_BLUE};
use crate::cheats::{CheatTables, CodeEntry};
use crate::menus::scale_ticks;
use crate::game_state::GameState;
use crate::hud::{text as wtext, Draw, Rot};
use crate::pad::button;
use rc_formats::font::Font;
use rc_formats::moby_anim::{self, AnimState, MobyAnimClass, MobyFrame};
use std::sync::Arc;

/// The widgets' callbacks (level-01 labels).
pub mod func {
    pub const CHEATS_ENTER: u32 = 0x28dbe8;
    /// The kind-0x23 page's scrolling text (enter, update, draw).
    pub const SCROLL_ENTER: u32 = 0x295de0;
    pub const SCROLL_UPDATE: u32 = 0x295df8;
    pub const SCROLL_DRAW: u32 = 0x296170;
    pub const STATS_UPDATE: u32 = 0x295c98;
    pub const STATS_DRAW: u32 = 0x2964a0;
    pub const GIRL_UPDATE: u32 = 0x2992f0;
    pub const GIRL_DRAW: u32 = 0x299670;
    pub const GIRL_ENTER: u32 = 0x299190;
    pub const GIRL_LEAVE: u32 = 0x299268;
    /// The Sketchbook's pager (its hints list's update) and the Epilogue's pager / arrows.
    pub const SKETCH_PAGER: u32 = 0x295770;
    pub const EPILOGUE_PAGER: u32 = 0x295858;
    pub const EPILOGUE_ARROWS: u32 = 0x295960;
}

/// Pages and tables named by the code (level-01 labels).
pub mod label {
    /// The Goodies page and its description table.
    pub const GOODIES_PAGE: u32 = 0x1b6ca0;
    pub const GOODIES_TEXTS: u32 = 0x1b6df8;
    /// The end page's ○ target with a memory card: the save page (`SavingDataMenu2` 0x296fc0).
    pub const SAVE_PAGE: u32 = 0x1b9440;
    /// The image widget's language table (flag 0x1000): the first entry of each language (s32 × 6).
    pub const LANG_OFFSETS: u32 = 0x20a2d0;
    /// The kind-0x23 page's label (its +0x34 is 0x1b7074) and its message lists (s32 ids ending at −1).
    pub const SCROLL_LABEL: u32 = 0x1b7040;
    pub const SCROLL_LISTS: [u32; 5] = [0x1b9618, 0x1b96c8, 0x1b9788, 0x1b9818, 0x1b9830];
}

/// Pictures in the Sketchbook (`sketchbook`, 30) and per language in the Epilogue (12).
pub const SKETCH_PAGES: i32 = 0x1e;
pub const EPILOGUE_PAGES: i32 = 0xc;

/// The Goodies list's entries `0x29aa60` rewrites (items 5..8).
pub const SKETCHBOOK: usize = 5;
pub const EPILOGUE: usize = 6;
pub const MAKING_OF: usize = 7;
pub const COMMERCIALS: usize = 8;

/// What the end page asks of the game outside the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndChoice {
    /// ✕ "Timewarp to before you defeated Drek": `memcard_RestoreGame(0x1ba250)` keeping 0x14bf08[4] and 0x13d425, then
    /// 0x15eea0 = 1, `memcard_Save(0, −1)`, `FUN_0029abc0`, `FadeToBlack(ticks(16))`, 0x141401 = 1.
    Timewarp,
    /// ○ "Start new game with current weapons and bolts" without a memory card: `fun_00226b08(−1)` and the level exit
    /// to 0 (`MenuOut::level_exit`), 0x13e05a = 1.
    Challenge,
}

/// The media state of the page menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaMenu {
    pub cheats: CheatTables,
    /// The debug code entry (0x1ba928, gp−0x6808).
    pub code: CodeEntry,
    /// This level's Goodies page, its list widget and its description table; the table's disc ids.
    pub goodies_page: u32,
    pub goodies_list: u32,
    pub goodies_texts: u32,
    pub texts: Vec<u32>,
    /// This level's save page 0x1b9440.
    pub save_page: u32,
    /// 0x15eeb0: the memory card status (1 / 0x10: a card is in and usable). The port's saves are on disk: 1 unless
    /// the `saves` lane sets it.
    pub card_status: i32,
    /// 0x1ba268 (a third Goodies unlock; no writer found).
    pub goodies_extra: bool,
    /// 0x20a2d0: the image widget's first entry per language (flag 0x1000).
    pub lang_offsets: Vec<i32>,
    /// The Epilogue pager's page (its widget's +0x54), by that word's address (the image widget's +0x40 points at it).
    pub pagers: std::collections::BTreeMap<u32, i32>,
    /// The Helpdesk girl's class 0x7a5 with its streamed sequences 1..3 (the engine loads it when the level has the class
    /// and the lump reads; None: `SpawnHandGadgetMoby` finds no class, or the read fails).
    pub girl_class: Option<GirlClass>,
    /// The Helpdesk girl widget's state while it exists (between its enter and leave).
    pub girl: Option<Girl>,
    /// Her moby as the last page-menu draw left it (the engine renders it; cleared by every draw).
    pub girl_view: Option<super::gadgets::GirlView>,
    /// 0x15172a, the dialogue player's state as the engine sets it before every tick (3 = a line loaded and waiting:
    /// `crate::help::Voice::state`).
    pub voice_state: u16,
    /// 0x1ba24c: the menu was entered with kind 0x23 (`EnterMenuMode`).
    pub kind23: bool,
    /// This level's kind-0x23 label widget and the scroller's message lists (by their level-01 label).
    pub scroll_label: u32,
    pub scroll_lists: std::collections::BTreeMap<u32, Vec<i32>>,
}

/// What the scroller's +0x34 holds: a message list (by its level-01 label) or one message id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollSrc {
    List(u32),
    Id(i32),
}

/// The kind-0x23 scroller widget (+0x34 source, +0x3c scroll in 1/16 px, +0x40 timer, +0x50 state, +0x54 the list
/// scrolled past).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scroller {
    pub src: ScrollSrc,
    pub scroll: i32,
    pub timer: i32,
    pub state: i32,
    pub fits: bool,
}

impl Default for Scroller {
    fn default() -> Self { Scroller { src: ScrollSrc::Id(0), scroll: 0, timer: 0, state: 0, fits: false } }
}

/// The Helpdesk girl's moby class (compared by identity).
#[derive(Clone, Debug)]
pub struct GirlClass(pub Arc<MobyAnimClass>);

impl PartialEq for GirlClass {
    fn eq(&self, o: &GirlClass) -> bool { Arc::ptr_eq(&self.0, &o.0) }
}
impl Eq for GirlClass {}

/// The Helpdesk girl widget (+0x34 stream state, +0x44 her moby) and her moby (+0x20 state, +0xbc counter, the animation).
#[derive(Clone, Debug, PartialEq)]
pub struct Girl {
    /// +0x34: 0 the read to start, 1 reading, 2 running, 3 nothing (no class / no buffer / the read refused).
    pub stream: i32,
    /// +0x44 ≠ 0: her moby exists.
    pub spawned: bool,
    /// The moby's +0x20 (0..6) and +0xbc.
    pub state: u8,
    pub count: u8,
    pub anim: AnimState,
    pub snapshot: Option<MobyFrame>,
}

impl Eq for Girl {}

/// The girl's class (`SpawnHandGadgetMoby` 0x298e98's argument).
pub const GIRL_CLASS: i16 = 0x7a5;
/// Her offset from the menu camera (0x167240): x + 2.2 (0x400ccccd), z − 1.6 (0xbfcccccd); yaw π (0x40490fdb).
pub const GIRL_OFFSET: [f32; 3] = [2.2, 0.0, -1.6];
/// Her voice lines: `play_dialogue` ids from 60000 (`post_credits_audio[id − 60000]`).
pub const GIRL_VOICE_BASE: i32 = 60000;

impl Default for MediaMenu {
    fn default() -> Self {
        MediaMenu {
            cheats: CheatTables::default(),
            code: CodeEntry::default(),
            goodies_page: 0,
            goodies_list: 0,
            goodies_texts: 0,
            texts: Vec::new(),
            save_page: 0,
            card_status: 1,
            goodies_extra: false,
            lang_offsets: vec![0, 0, 12, 36, 48, 24],
            pagers: Default::default(),
            girl_class: None,
            girl: None,
            girl_view: None,
            voice_state: 0,
            kind23: false,
            scroll_label: label::SCROLL_LABEL,
            scroll_lists: Default::default(),
        }
    }
}

impl MediaMenu {
    /// The tables (from the level overlay) and the Goodies page's widgets (from the loaded records).
    pub fn read(ov: &super::super::Overlay, pages: &std::collections::BTreeMap<u32, super::Page>) -> MediaMenu {
        let goodies_page = ov.at(label::GOODIES_PAGE);
        let ws = pages.get(&goodies_page).map(|p| p.widgets).unwrap_or([0; 14]);
        let goodies_texts = ov.at(label::GOODIES_TEXTS);
        let texts = (0..9u32).map(|k| ov.u32(goodies_texts + 4 * k).unwrap_or(0)).collect();
        MediaMenu {
            cheats: CheatTables::read(ov),
            goodies_page,
            goodies_list: ws[1],
            goodies_texts,
            texts,
            save_page: ov.at(label::SAVE_PAGE),
            lang_offsets: (0..6u32).map(|k| ov.i32(ov.at(label::LANG_OFFSETS) + 4 * k).unwrap_or(0)).collect(),
            scroll_label: ov.at(label::SCROLL_LABEL),
            scroll_lists: label::SCROLL_LISTS
                .iter()
                .map(|&l| {
                    let a = ov.at(l);
                    (l, (0..64u32).map(|k| ov.i32(a + 4 * k).unwrap_or(-1)).take_while(|&v| v != -1).collect())
                })
                .collect(),
            ..MediaMenu::default()
        }
    }
}

/// Nonzero bytes of `b`, at most `max` (`count_nonzero_entries_up_to_*`).
fn count(b: &[u8], max: usize) -> i32 { b.iter().filter(|&&x| x != 0).count().min(max) as i32 }

/// `count_nonzero_entries_up_to_40` 0x279118: the gold bolts 0x14bec0[20][4].
pub fn gold_bolts(gs: &GameState) -> i32 {
    let n: usize = gs.levels.iter().take(20).map(|l| l.gold_bolts.iter().filter(|&&b| b != 0).count()).sum();
    n.min(40) as i32
}

/// `count_nonzero_entries_up_to_10` 0x279188: the gold weapons 0x13e520[37].
pub fn gold_weapons(gs: &GameState) -> i32 { count(&gs.global.gold_weapons[..37], 10) }

/// `count_nonzero_entries_up_to_30` 0x2791d0: the skill points 0x13d408[32].
pub fn skill_points(gs: &GameState) -> i32 { count(&gs.global.skill_points, 30) }

/// `FUN_0029aa60` (end of `EnterMenuMode`).
pub fn goodies_unlocks(m: &mut PageMenu, gs: &GameState) {
    let (sp, gw) = (skill_points(gs), gold_weapons(gs));
    let all_gold = gw > 9;
    let list = m.media.goodies_list;
    if let Some(Data::List(l)) = m.widgets.get_mut(&list).map(|w| &mut w.data) {
        let mut set = |i: usize, a: i16| {
            if let Some(it) = l.items.get_mut(i) { it.action = a; }
        };
        set(SKETCHBOOK, if sp > 0xe { 3 } else { 2 });
        set(EPILOGUE, if sp > 0x1d { 3 } else { 2 });
        set(MAKING_OF, if all_gold { 10 } else { 2 });
        set(COMMERCIALS, if all_gold { 3 } else { 2 });
    }
    let mut t = m.media.texts.clone();
    if t.len() > COMMERCIALS {
        t[SKETCHBOOK] = if sp > 0xe { 0x4fd3 } else { 0x4fd9 };
        t[EPILOGUE] = if sp > 0x1d { 0x4fd4 } else { 0x4fda };
        t[MAKING_OF] = if all_gold { 0x4fd7 } else { 0x4fdb };
        t[COMMERCIALS] = if all_gold { 0x4fd8 } else { 0x4fdb };
        m.label_tables.insert(m.media.goodies_texts, t);
    }
}

/// `0x28dbe8`: the Cheats page's toggle list rebuilt from the cheats ever activated.
pub fn cheats_enter(m: &mut PageMenu, w: u32, gs: &GameState) {
    let entries: Vec<super::options::ToggleEntry> = crate::cheats::entries(&m.media.cheats, gs)
        .into_iter()
        .map(|e| super::options::ToggleEntry { label: e.label, var: crate::cheats::var_addr(e.slot), on: crate::cheats::msg::ON, off: crate::cheats::msg::OFF, flags: 0 })
        .collect();
    if let Some(Data::Toggle(t)) = m.widgets.get_mut(&w).map(|x| &mut x.data) { t.entries = entries; }
}

/// `0x295c98` (the end page's keys; not focus-gated; 0x13cae4 = the pressed buttons after the lock).
pub fn stats_update(m: &mut PageMenu, inp: &MenuInput, out: &mut MenuOut) -> i32 {
    if inp.pressed & button::CIRCLE != 0 {
        if m.media.card_status == 1 || m.media.card_status == 0x10 {
            m.target = m.media.save_page;
        } else {
            out.end_choice = Some(EndChoice::Challenge);
            out.level_exit = Some(0);
        }
        return 0;
    }
    if inp.pressed & button::CROSS != 0 {
        out.end_choice = Some(EndChoice::Timewarp);
        return -1;
    }
    0
}

/// `draw_menu_selection_marker` 0x2932f0 (panel-local).
pub fn check_box(a: &MenuAssets, navy: u32, x: i32, y: i32, ticked: bool, out: &mut Vec<MenuDraw>) {
    out.push(MenuDraw::Rect { x0: x - 5, y0: y - 5, x1: x + 5, y1: y + 5, rgba: LIGHT_BLUE });
    out.push(MenuDraw::Rect { x0: x - 4, y0: y - 4, x1: x + 4, y1: y + 4, rgba: navy });
    if ticked {
        let f = a.frame(0xe99e, 1);
        out.push(MenuDraw::Hud(Draw::Sprite { frame: f, x: x - 0xd, y: y - 0x12, w: 0x1e, h: 0x1e, alpha: 0x80, rot: Rot::None }));
    }
}

/// `sprintf(buf, msg_string(fmt), n, max)` for the "%d of %d" lines.
fn counted(a: &MenuAssets, fmt: i32, n: i32, max: i32) -> Vec<u8> {
    let f = a.msg(fmt);
    let mut out = Vec::with_capacity(f.len() + 8);
    let mut args = [n, max].into_iter();
    let mut i = 0;
    while i < f.len() {
        if f[i] == b'%' && f.get(i + 1) == Some(&b'd') {
            out.extend_from_slice(args.next().unwrap_or(0).to_string().as_bytes());
            i += 2;
        } else {
            out.push(f[i]);
            i += 1;
        }
    }
    out
}

/// `DrawEndScreenMenuMaybe` 0x2964a0 (panel-local; returns 2: 1:1 blit).
pub fn stats_draw(m: &PageMenu, w: u32, a: &MenuAssets, gs: &GameState, out: &mut Vec<MenuDraw>) -> u32 {
    let Some(wd) = m.widgets.get(&w) else { return 1 };
    let [_, _, ww, wh] = wd.rect;
    let row = wh / 5;
    let (gb, gw, sp) = (gold_bolts(gs), gold_weapons(gs), skill_points(gs));
    let lines = [counted(a, 0x522f, gb, 0x28), counted(a, 0x5230, gw, 10), counted(a, 0x5231, sp, 0x1e)];
    // FUN_0021cc90: the widths in the regular font (codes as measured).
    let widest = lines.iter().map(|l| a.width(Font::Regular, l)).max().unwrap_or(0).max(0);
    let font = if ww < widest + 0x18 { Font::Small } else { Font::Regular };
    // Colour codes off from here (FUN_0021cc38) to the end (FUN_0021cc28).
    let title = a.msg(0x522e).to_vec();
    let tw = a.width(Font::Regular, &title);
    crate::menus::text_plain(out, Font::Regular, (ww >> 1) - (tw >> 1), row - 8, LIGHT_BLUE, &title);
    let mut y = row - 8 + row;
    for (line, done) in lines.iter().zip([gb == 0x28, gw == 10, sp == 0x1e]) {
        check_box(a, m.consts.navy, 0xb, y + 9, done, out);
        crate::menus::text_plain(out, font, 0x14, y, LIGHT_BLUE, line);
        y += row;
    }
    2
}

/// The Epilogue pager's page that an image widget's +0x40 points at (0 until the pager runs).
pub fn pager_page(m: &PageMenu, ptr: u32) -> i32 { m.media.pagers.get(&ptr).copied().unwrap_or(0) }

/// `fun_00221d68` 0x295770, the Sketchbook's pager (its hints list's update, not focus-gated): the close keys (1), △
/// (the parent, or −1), then ✕ (0x13cae4 & 0x40) the next picture, ○ (& 0x20) the previous, of 30 (the list's +0x40;
/// the image widget shows the focused list's cursor), sound 1 on a change.
pub fn sketch_pager(m: &mut PageMenu, w: u32, inp: &MenuInput, out: &mut MenuOut) -> i32 {
    if let Some(r) = super::options::keys(m, inp, 1) { return r; }
    let Some(Data::List(l)) = m.widgets.get_mut(&w).map(|x| &mut x.data) else { return 0 };
    let old = l.cursor;
    if inp.pressed & button::CROSS != 0 {
        l.cursor = (old + 1) % SKETCH_PAGES;
    } else if inp.pressed & button::CIRCLE != 0 {
        l.cursor = (old + SKETCH_PAGES - 1) % SKETCH_PAGES;
    }
    if l.cursor != old { out.sounds.push(super::super::MenuSound::Cursor); }
    0
}

/// `fun_00221e50` 0x295858, the Epilogue's pager: the close keys (1), △ (the parent, or −1), then Right / ✕ (0x13cae4 &
/// 0x2040) the next of 12 pages (+0x54), else Left / ○ (& 0x8020) the previous, sound 1.
pub fn epilogue_pager(m: &mut PageMenu, w: u32, inp: &MenuInput, out: &mut MenuOut) -> i32 {
    if let Some(r) = super::options::keys(m, inp, 1) { return r; }
    let at = w + 0x54;
    let page = pager_page(m, at);
    let next = if inp.pressed & (button::RIGHT | button::CROSS) != 0 {
        (page + 1) % EPILOGUE_PAGES
    } else if inp.pressed & (button::LEFT | button::CIRCLE) != 0 {
        (page + EPILOGUE_PAGES - 1) % EPILOGUE_PAGES
    } else {
        return 0;
    };
    m.media.pagers.insert(at, next);
    out.sounds.push(super::super::MenuSound::Cursor);
    0
}

/// `fun_00221f58` 0x295960, the Epilogue's arrows (panel-local, returns 2): with +0x38 set "\x10" (✕) in the regular
/// font at (4, h/2 − 8) and the arrow (icon 0xe99e frame 6, 0x20 × 0x10 texels) centred at (40, h/2), 8 × 16; else "\x11"
/// (○) at (w − 0x18, h/2 − 8) and the arrow turned by π at (12, h/2); 0x80ffa888.
pub fn epilogue_arrows(m: &PageMenu, w: u32, a: &MenuAssets, out: &mut Vec<MenuDraw>) -> u32 {
    let Some(wd) = m.widgets.get(&w) else { return 2 };
    let [_, _, ww, wh] = wd.rect;
    let y = wh / 2 - 8;
    let frame = a.frame(0xe99e, 6);
    if wd.raw[2] != 0 {
        crate::menus::text(out, Font::Regular, 4, y, LIGHT_BLUE, b"\x10");
        out.push(super::map_page::rotated(frame, 40.0, (wh << 3) as f32 / 16.0, 8.0, 16.0, 0.0, 0x20, 0x10));
    } else {
        crate::menus::text(out, Font::Regular, ww - 0x18, y, LIGHT_BLUE, b"\x11");
        out.push(super::map_page::rotated(frame, 12.0, (wh << 3) as f32 / 16.0, 8.0, 16.0, std::f32::consts::PI, 0x20, 0x10));
    }
    2
}

// ---------------------------------------------------------------------------------------------------
// The Helpdesk girl (G-CUT-008)

/// `fun_00225588` 0x299190, the girl widget's enter.
pub fn girl_enter(m: &mut PageMenu) {
    let class = m.media.girl_class.clone();
    m.media.girl = Some(match class {
        // SpawnHandGadgetMoby(0x7a5): the moby on its sequence 0 (`InitMobyInstance`), +0x34 = 0; the stream buffer
        // (`select_next_stream_buffer(1)`) is always there in the port.
        Some(c) => Girl { stream: 0, spawned: true, state: 0, count: 0, anim: AnimState::spawn(&c.0), snapshot: None },
        None => Girl { stream: 3, spawned: false, state: 0, count: 0, anim: AnimState::spawn(&MobyAnimClass { joint_count: 0, skeleton: Vec::new(), rest: Vec::new(), parent_word: Vec::new(), sequences: Vec::new() }), snapshot: None },
    });
}

/// `fun_00225660` 0x299268, the girl widget's leave: her moby deleted, the line stopped (0x15172a ∉ {6, 7} → 5).
pub fn girl_leave(m: &mut PageMenu, out: &mut MenuOut) {
    m.media.girl = None;
    out.voice_stop = true;
}

/// `fun_002256e8` 0x2992f0, the girl widget's update (not focus-gated). [`MediaMenu::voice_state`] = 0x15172a.
pub fn girl_update(m: &mut PageMenu, gs: &GameState, out: &mut MenuOut) -> i32 {
    let voice_state = m.media.voice_state;
    let completes = gs.global.completes;
    let lang = m.lang as i32;
    let Some(class) = m.media.girl_class.clone() else {
        if let Some(g) = m.media.girl.as_mut() { if g.stream < 2 { g.stream = 3; } }
        return 0;
    };
    let Some(g) = m.media.girl.as_mut() else { return 0 };
    let c = &class.0;
    match g.stream {
        // The read starts (the engine has the lump; a refused read: no girl class, +0x34 = 3).
        0 => g.stream = 1,
        // The read is done: the sequences are in the class (`GirlClass`); the first state.
        1 => {
            g.stream = 2;
            g.state = if completes == 0 { 0 } else { 4 };
            g.count = 0;
        }
        2 => match g.state {
            0 | 2 | 4 => {
                let k = (g.state >> 1) as i32;
                if g.count == 0 { out.voice = Some(GIRL_VOICE_BASE + 6 * k + (lang - 1).max(0)); }
                g.count = g.count.wrapping_add(1);
                let wait = scale_ticks(0x78);
                if wait < g.count as i32 { g.count = wait as u8; }
                if (g.count as i32) < wait || voice_state != 3 { return 0; }
                g.count = 0;
                g.state += 1;
                moby_anim::set_sequence(&mut g.anim, c, (k + 1) as u8, 0, scale_ticks(0x18), &mut g.snapshot);
            }
            1 | 3 | 5 => {
                if g.count == 0 && g.anim.seq_a == g.anim.seq_b {
                    out.voice_continue = true;
                    g.count = 1;
                }
                if g.anim.flags & 2 == 0 { return 0; }
                g.state = if g.state == 1 { 2 } else { 6 };
                g.count = 0;
                moby_anim::set_sequence(&mut g.anim, c, 0, 0, scale_ticks(0x18), &mut g.snapshot);
            }
            6 => {
                g.count = g.count.wrapping_add(1);
                let wait = scale_ticks(0xf0);
                if wait < g.count as i32 { g.count = wait as u8; }
                if wait <= g.count as i32 {
                    g.state = if completes == 0 { 0 } else { 4 };
                    g.count = 0;
                }
            }
            _ => {}
        },
        _ => {}
    }
    0
}

/// MobyUpdateLoop's `MobyAnimAdvance` of the girl (every menu frame, after the widget updates).
pub fn girl_tick(m: &mut PageMenu) {
    let (Some(c), Some(g)) = (m.media.girl_class.as_ref(), m.media.girl.as_mut()) else { return };
    if g.spawned { moby_anim::advance(&mut g.anim, &c.0); }
}

/// `draw_moby_entries_from_object` 0x299670: her moby into the panel (the engine draws [`MediaMenu::girl_view`]); return 4.
pub fn girl_draw(m: &mut PageMenu, w: u32) -> u32 {
    let rect = m.widgets.get(&w).map_or([0; 4], |x| x.rect);
    if let Some(g) = m.media.girl.as_ref().filter(|g| g.spawned) {
        m.media.girl_view = Some(super::gadgets::GirlView { rect, anim: g.anim, snapshot: g.snapshot.clone() });
    }
    4
}

// ---------------------------------------------------------------------------------------------------
// The kind-0x23 page's scrolling text (0x295de0 / 0x295df8 / 0x296170)

/// 0x295de0: +0x3c = +0x40 = +0x50 = 0.
pub fn scroller_enter(m: &mut PageMenu, w: u32) {
    if let Some(Data::Scroller(x)) = m.widgets.get_mut(&w).map(|x| &mut x.data) {
        x.scroll = 0;
        x.timer = 0;
        x.state = 0;
    }
}

/// `FUN_00295df8`: the keys (the close keys and △ only when not opened as kind 0x23, or with L1 + R1 + L2 + R2 held and
/// △), the timer − 1 (− 2 with the four shoulders held, ≥ 0), then the script by state: 0 → the label 0x50a9 and the
/// list 0x1b9618, `ticks(180)`; the odd waits (1, 5, 8, 12, 16) for the timer, then scroll; the scrolls (2, 6, 9, 13, 17)
/// add 10 (20 fast) per tick in 1/16 px until the list has passed (+0x54), then `ticks(180)`; 3 → label 0x50d4 and the
/// message 0x50d5; 4 → label 0x50d6 and the list 0x1b96c8; 7 → 0x5106 / 0x1b9788; 10 → 0x5136 / 0x5137; 11 → 0x5138 /
/// 0x1b9818 (state 12); 14 → no label, 0x5143; 15 → 0x5144 / 0x1b9830 (state 16); 18 → no label, 0x5175, `ticks(300)`;
/// 19: at the timer's end, opened as kind 0x23 → close. A single message waits `ticks(240)`, a list `ticks(180)`.
pub fn scroller_update(m: &mut PageMenu, w: u32, inp: &MenuInput) -> i32 {
    let fast = inp.held & 0xf == 0xf;
    let generic = !m.media.kind23 || (fast && inp.held & button::TRIANGLE != 0);
    if generic {
        if inp.pressed_u & 0xd00 != 0 && !m.no_close { return 1; }
        if inp.pressed_u & button::TRIANGLE != 0 {
            let parent = m.pages.get(&m.current).map_or(0, |p| p.parent);
            if parent != 0 {
                m.target = parent;
            } else if !m.no_close {
                return -1;
            }
        }
    }
    let label = m.media.scroll_label;
    let kind23 = m.media.kind23;
    let Some(Data::Scroller(x)) = m.widgets.get(&w).map(|x| &x.data) else { return 0 };
    let mut x = *x;
    x.timer -= if fast { 2 } else { 1 };
    if x.timer < 0 { x.timer = 0; }
    let mut set_label: Option<u32> = None;
    let mut ret = 0;
    // The waits end when the timer is 0.
    let single = |x: &mut Scroller, lab: u32, id: i32, set: &mut Option<u32>| {
        *set = Some(lab);
        x.src = ScrollSrc::Id(id);
        x.timer = scale_ticks(0xf0);
        x.scroll = 0;
        x.state += 1;
    };
    let list = |x: &mut Scroller, lab: u32, l: u32, state: i32, set: &mut Option<u32>| {
        x.timer = scale_ticks(0xb4);
        x.scroll = 0;
        *set = Some(lab);
        x.src = ScrollSrc::List(l);
        x.state = state;
    };
    let lists = label::SCROLL_LISTS;
    match x.state {
        0 => list(&mut x, 0x50a9, lists[0], 1, &mut set_label),
        1 | 5 | 8 | 0xc | 0x10 => {
            if x.timer == 0 {
                x.fits = false;
                x.state += 1;
            }
        }
        2 | 6 | 9 | 0xd | 0x11 => {
            x.scroll += if fast { 0x14 } else { 10 };
            if x.fits {
                x.timer = scale_ticks(0xb4);
                x.state += 1;
            }
        }
        3 if x.timer == 0 => single(&mut x, 0x50d4, 0x50d5, &mut set_label),
        4 if x.timer == 0 => list(&mut x, 0x50d6, lists[1], 5, &mut set_label),
        7 if x.timer == 0 => {
            set_label = Some(0x5106);
            x.src = ScrollSrc::List(lists[2]);
            x.timer = scale_ticks(0xf0);
            x.scroll = 0;
            x.state += 1;
        }
        10 if x.timer == 0 => single(&mut x, 0x5136, 0x5137, &mut set_label),
        0xb if x.timer == 0 => list(&mut x, 0x5138, lists[3], 0xc, &mut set_label),
        0xe if x.timer == 0 => single(&mut x, 0, 0x5143, &mut set_label),
        0xf if x.timer == 0 => list(&mut x, 0x5144, lists[4], 0x10, &mut set_label),
        0x12 if x.timer == 0 => {
            set_label = Some(0);
            x.src = ScrollSrc::Id(0x5175);
            x.state += 1;
            x.timer = scale_ticks(300);
        }
        0x13 if x.timer == 0 && kind23 => ret = 1,
        _ => {}
    }
    if let Some(Data::Scroller(y)) = m.widgets.get_mut(&w).map(|x| &mut x.data) { *y = x; }
    // 0x1b7074: the page's label widget's id.
    if let Some(id) = set_label {
        if let Some(Data::Label(l)) = m.widgets.get_mut(&label).map(|x| &mut x.data) { l.id = id; }
    }
    ret
}

/// `fun_00222768` 0x296170 (drawn direct: screen coordinates of the widget's rect): states with a list: its messages one
/// under the other from y + 4 − scroll/16, 10 px apart, in a window (y + 4 .. y + h − 4, x .. x + w, centred at
/// x + w/2, lines 16, flags 9), the list passed when its end + 0x18 is above the bottom (+0x54 = 1); states 4, 11, 15,
/// 19: the one message from y + 0x20; regular font, 0x80ffa888; return 2.
pub fn scroller_draw(m: &mut PageMenu, w: u32, a: &MenuAssets, out: &mut Vec<MenuDraw>) -> u32 {
    let lists = m.media.scroll_lists.clone();
    let Some(wd) = m.widgets.get_mut(&w) else { return 2 };
    let [x, y, ww, wh] = wd.rect;
    let Data::Scroller(sc) = &mut wd.data else { return 2 };
    let mut win = wtext::Window::new((y + 4) as i16, (y + wh - 4) as i16, x as i16, (x + ww) as i16, (x + (ww >> 1)) as i16, 0, 0x10, 9);
    win.sub_y = ((sc.scroll as u8) >> 4) as i16;
    let glyphs = &a.hud.glyphs[Font::Regular as usize];
    match sc.state {
        1 | 2 | 3 | 5..=10 | 0xc..=0xe | 0x10..=0x12 => {
            let ids = match sc.src {
                ScrollSrc::List(l) => lists.get(&l).cloned().unwrap_or_default(),
                ScrollSrc::Id(id) => vec![id],
            };
            let mut ly = y - ((sc.scroll >> 4) - 4);
            for id in ids {
                let t = a.msg(id).to_vec();
                win.y_start = ly as i16;
                let mut m2 = win;
                m2.flags |= wtext::MEASURE_ONLY;
                wtext::layout(&mut m2, &t, -1, glyphs, true);
                out.push(MenuDraw::Hud(Draw::TextWindow { font: Font::Regular, window: win, rgba: LIGHT_BLUE, text: t }));
                ly += m2.height as i32 + 10;
            }
            if ly + 0x18 < y + wh { sc.fits = true; }
        }
        4 | 0xb | 0xf | 0x13 => {
            if let ScrollSrc::Id(id) = sc.src {
                win.y_start = (y + 0x20) as i16;
                out.push(MenuDraw::Hud(Draw::TextWindow { font: Font::Regular, window: win, rgba: LIGHT_BLUE, text: a.msg(id).to_vec() }));
            }
        }
        _ => {}
    }
    2
}
