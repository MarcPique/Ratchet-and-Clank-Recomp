//! The quick-select ring (△, HUD slot 3): the hero-side open / double-tap code of `0x242930`, the slot's init
//! `QuickSelectInit` 0x24d180, update `QuickSelectUpdate` 0x24d238 and draw `QuickSelectDraw` 0x24d938.
//! Spec: docs/plan/menus.md §2.
//!
//! Per 60 Hz game tick, in the game's order: [`QuickSelect::hero`] (inside the hero update, after its early
//! state changes) → [`QuickSelect::update`] (the HUD update loop) → [`QuickSelect::draw`] (the HUD pass). The
//! game keeps running (mode 0). The float steps (stick length, angle, sector, hysteresis, slot positions,
//! pulse) run on the PS2 FPU model ([`Pf`]) in the game's order.
//!
//! **Clank's command menu** (△ in body 1, HUD slot 3; level copies of L00 `0x238b18` / `0x238b80` / `0x238f88`, on
//! Blarg `0x248040` / `0x2480a8` / `0x2484b0`): the same ring on the page table 0x15f740 ([`Commands`]): four
//! entries (icon 30035, frame +0x04, neighbours, the command +0x18), each command's label (0x15f7e0..). Its update
//! differs: the pad locked every tick, the stick's sector without hysteresis or empty-slot skip, the d-pad from any
//! state, no △ test while picking, and it arms only while gadgetbots listen (the bots' count, level06 0x17ec84:
//! `Globals::bot_listeners`); on △'s release it writes the command 0x141610 ([`QsEvents::command`]: 1 follow, 2 wait,
//! 3 attack, 4 to a pad, 0 none). Its draw: corner frame 1, the icons on a circle of four (greyed, frame + 4, while no
//! bot listens), the cursor, the label in 0x80e0c0a0 with its shadow up-left. Never armed, the release reads the
//! entry before the table (sel −2: the item word of the ring's entry 6) [L: the game's].
//!
//! Disc data comes from the overlay: the gp constants 0x15f718..0x15f7cc, the neighbour table through the
//! page-table pointer `*0x15f738` (level 01: 0x17df40), the d-pad defaults 0x17e098, and the item
//! definitions (icon +0x38, name +0x46, gold name +0x48) at `ItemTables::item_defs_addr`.

use super::{scale_ticks, sprite, text, tween, MenuAssets, MenuDraw, MenuInput, Overlay};
use crate::game_state::{Global, HelpRec, SessionState};
use crate::hero::physics::{fast_add_rotations, fast_cos, fast_sin, fast_subtract_rotations, PI};
use crate::pad::{button, fast_arctan, fast_diff_rots, len2};
use crate::ps2v::Pf;
use rc_formats::font::Font;
use rc_formats::save_game::{ItemTables, ITEM_COUNT, ITEM_DEF_SIZE};

/// Entries on page 0 (`0x15fa90`, set by the init).
pub const SLOTS: usize = 8;
/// Entry stride of the neighbour table.
const ENTRY: u32 = 0x1c;
/// Level-01 addresses (docs/plan/menus.md §2). The table itself is reached through the page-table pointer.
pub const GP_BASE: u32 = 0x15f718;
pub const PAGE_TABLE: u32 = 0x15f738;
pub const DPAD_DEFAULTS: u32 = 0x17e098;
/// HUD slot of the ring and its static anchor (hud_text.md §4.1: slot 3 (20, 208), bits 4 = left edge,
/// vertical centre).
pub const HUD_SLOT: usize = 3;
/// Icons.
pub const RING_ICON: u16 = 59700;
pub const CURSOR_ICON: u16 = 59804;
/// Item 0x18 (Drone Device) and the wrench.
const DRONE: i32 = 0x18;
const WRENCH: i32 = 8;

const TWO_PI: Pf = Pf::b(0x40c9_0fdb); // 6.2831855
const HALF_PI: Pf = Pf::b(0x3fc9_0fdb); // 1.5707964
const HYSTERESIS: Pf = Pf::b(0x3f16_cbe4); // 0.5890486 = 3π/16
const QUARTER_PI: Pf = Pf::b(0x3f49_0fdb); // 0.7853982
const EIGHTH_PI: Pf = Pf::b(0x3ec9_0fdb); // 0.3926991
const PULSE_2PI: Pf = Pf::b(0x40c9_0fd0); // 6.28318 (the draw's literal)
const PULSE_PI: Pf = Pf::b(0x4049_0fd0); // 3.14159
const F128: Pf = Pf::b(0x4300_0000);
const F0_875: Pf = Pf::b(0x3f60_0000);
const F0_125: Pf = Pf::b(0x3e00_0000);
const F0_9: Pf = Pf::b(0x3f66_6666);
const F0_5: Pf = Pf::b(0x3f00_0000);

/// The gp constants of the ring (level01 small data 0x15f718..0x15f7cc).
#[derive(Clone, Debug, PartialEq)]
pub struct QsConsts {
    /// 0x15f718 / 0x15f71c: box size (210 × 200).
    pub size: (i32, i32),
    /// 0x15f720 / 0x15f724: centre offset in the box (105, 100).
    pub centre: (i32, i32),
    /// 0x15f780: fade steps (8).
    pub steps: i32,
    /// 0x15f788: sector edge fraction (0.2).
    pub edge: Pf,
    /// 0x15f78c: radius (74); 0x15f7c4: x stretch (1.05); 0x15f7c0: unselected alpha factor (0.5).
    pub radius: Pf,
    pub x_scale: Pf,
    pub dim: Pf,
    /// 0x15f790..0x15f7ac: name, shadow, empty-ammo and ammo colour tweens (from, to).
    pub name_rgba: (u32, u32),
    pub shadow_rgba: (u32, u32),
    pub ammo_empty_rgba: (u32, u32),
    pub ammo_rgba: (u32, u32),
    /// 0x15f7b0: cursor pulse period (30).
    pub period: i32,
    /// 0x15f7b4 / 0x15f7b8 / 0x15f7bc: name y offset with ammo (−25), split without ammo (−15), else (0).
    pub name_dy: [i32; 3],
    /// 0x15f7c8: the ammo format ("%d/%d").
    pub ammo_format: Vec<u8>,
}

/// One page-0 entry (0x1c bytes): icon +0, +4 unknown, neighbours left/right/up/down +8..+0x14, item +0x18.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub icon: i32,
    pub f4: i32,
    pub left: i32,
    pub right: i32,
    pub up: i32,
    pub down: i32,
    pub item: i32,
}

/// Item definition fields the ring reads (`0x179f40 + 0x4c·item`) and the price record's ammo fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QsItems {
    /// +0x38 icon (60000 + id), +0x46 name id, +0x48 gold name id.
    pub icon: Vec<u16>,
    pub name: Vec<u16>,
    pub gold_name: Vec<u16>,
    /// Record `0x1c4538 + 0x18·item` ≠ 0, and the max ammo `0x1c453e + 0x18·item`.
    pub has_ammo: Vec<bool>,
    pub max_ammo: Vec<u16>,
}

impl QsItems {
    pub fn load(ov: &Overlay, items: &ItemTables) -> Option<QsItems> {
        let def = |i: usize, o: u32| ov.u16(items.item_defs_addr + (i * ITEM_DEF_SIZE) as u32 + o);
        let mut q = QsItems { icon: vec![], name: vec![], gold_name: vec![], has_ammo: vec![], max_ammo: vec![] };
        for i in 0..ITEM_COUNT {
            q.icon.push(def(i, 0x38)?);
            q.name.push(def(i, 0x46)?);
            q.gold_name.push(def(i, 0x48)?);
            let r = &items.records[i].0;
            q.has_ammo.push(items.records[i].has_ammo());
            q.max_ammo.push(u16::from_le_bytes([r[0xe], r[0xf]]));
        }
        Some(q)
    }

    fn icon_of(&self, item: i32) -> i32 { usize::try_from(item).ok().and_then(|i| self.icon.get(i)).map_or(0, |&v| v as i32) }
    fn ammo_of(&self, item: i32) -> Option<u16> {
        let i = usize::try_from(item).ok()?;
        self.has_ammo.get(i).copied().unwrap_or(false).then(|| self.max_ammo[i])
    }
}

/// The static tables: the page-0 entries' neighbours, the d-pad defaults from −1, and the word the C `%`
/// quirk reads at entry −1.
#[derive(Clone, Debug, PartialEq)]
pub struct QsTables {
    pub entries: [Entry; SLOTS],
    /// 0x17e098..a4: left, right, up, down.
    pub defaults: [i32; 4],
    /// The icon word of "entry −1" (table − 0x1c).
    pub entry_m1_icon: i32,
    pub table_addr: u32,
}

impl QsConsts {
    pub fn load(ov: &Overlay) -> Option<QsConsts> {
        let base = ov.at(GP_BASE);
        let g = |o: u32| ov.i32(base + o);
        let gu = |o: u32| ov.u32(base + o);
        let fmt_addr = base + 0xb0;
        let fmt = ov.bytes(fmt_addr, 16)?;
        let fmt = fmt[..fmt.iter().position(|&c| c == 0).unwrap_or(fmt.len())].to_vec();
        Some(QsConsts {
            size: (g(0)?, g(4)?),
            centre: (g(8)?, g(0xc)?),
            steps: g(0x68)?,
            edge: ov.pf(base + 0x70)?,
            radius: ov.pf(base + 0x74)?,
            x_scale: ov.pf(base + 0xac)?,
            dim: ov.pf(base + 0xa8)?,
            name_rgba: (gu(0x78)?, gu(0x7c)?),
            shadow_rgba: (gu(0x80)?, gu(0x84)?),
            ammo_empty_rgba: (gu(0x88)?, gu(0x8c)?),
            ammo_rgba: (gu(0x90)?, gu(0x94)?),
            period: g(0x98)?,
            name_dy: [g(0x9c)?, g(0xa0)?, g(0xa4)?],
            ammo_format: fmt,
        })
    }
}

impl QsTables {
    pub fn load(ov: &Overlay) -> Option<QsTables> {
        let t = ov.u32(ov.at(GP_BASE) + (PAGE_TABLE - GP_BASE))?;
        let e = |k: u32, o: u32| ov.i32(t + k * ENTRY + o);
        let mut entries = [Entry::default(); SLOTS];
        for (k, en) in entries.iter_mut().enumerate() {
            let k = k as u32;
            *en = Entry { icon: 0, f4: e(k, 4)?, left: e(k, 8)?, right: e(k, 0xc)?, up: e(k, 0x10)?, down: e(k, 0x14)?, item: 0 };
        }
        let dpad = ov.at(DPAD_DEFAULTS);
        let d = |k: u32| ov.i32(dpad + 4 * k);
        Some(QsTables { entries, defaults: [d(0)?, d(1)?, d(2)?, d(3)?], entry_m1_icon: ov.i32(t - ENTRY).unwrap_or(0), table_addr: t })
    }
}

/// The hero-block conditions the open test reads (0x242930; meanings [L]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeroGate {
    /// An earlier state change fired this tick (`FUN_00231580(1)`, death, PDA 0x240ed8, 0x2408e8, 0x2406b0,
    /// 0x2405f8): the open and double-tap code does not run.
    pub early_exit: bool,
    /// 0x1403fc (hand-0 swap state; 2 = running).
    pub swap_state: u8,
    /// 0x15f594.
    pub f594: i32,
    /// 0x1413fc, 0x1413f7, 0x1413f4, 0x13f502.
    pub b13fc: u8,
    pub b13f7: u8,
    pub b13f4: u8,
    pub s3f502: i16,
    /// 0x140408: the item in the hand now.
    pub held_item: i32,
}

/// Clank's command page (module doc): page-table pointer 0x15f740 (`gp−0x74c0`), the d-pad defaults 0x98 past the
/// table (left, right, up, down; the same offset on every Clank level), the labels' message ids 0x15f7e0..0x15f7ec,
/// colours 0x15f7d4 / 0x15f7d8 and y offset 0x15f7dc.
#[derive(Clone, Debug, PartialEq)]
pub struct Commands {
    pub entries: [Entry; CMD_SLOTS],
    pub defaults: [i32; 4],
    pub msgs: [i32; CMD_SLOTS],
    pub label_rgba: (u32, u32),
    pub label_dy: i32,
}

/// Entries on the command page (`0x15fa90`, set by its init).
pub const CMD_SLOTS: usize = 4;
pub const CMD_PAGE: u32 = 0x15f740;
const CMD_DEFAULTS: u32 = 0x98;
const CMD_MSGS: u32 = 0x15f7e0;
const CMD_LABEL_RGBA: u32 = 0x15f7d4;
const CMD_LABEL_DY: u32 = 0x15f7dc;

impl Commands {
    /// None when the overlay has no command page (a level without Clank).
    pub fn load(ov: &Overlay) -> Option<Commands> {
        let t = ov.u32(ov.at(CMD_PAGE))?;
        if t == 0 { return None; }
        let e = |k: u32, o: u32| ov.i32(t + k * ENTRY + o);
        let mut entries = [Entry::default(); CMD_SLOTS];
        for (k, en) in entries.iter_mut().enumerate() {
            let k = k as u32;
            *en = Entry { icon: e(k, 0)?, f4: e(k, 4)?, left: e(k, 8)?, right: e(k, 0xc)?, up: e(k, 0x10)?, down: e(k, 0x14)?, item: e(k, 0x18)? };
        }
        let d = |k: u32| ov.i32(t + CMD_DEFAULTS + 4 * k);
        let m = |k: u32| ov.i32(ov.at(CMD_MSGS) + 4 * k);
        Some(Commands {
            entries,
            defaults: [d(0)?, d(1)?, d(2)?, d(3)?],
            msgs: [m(0)?, m(1)?, m(2)?, m(3)?],
            label_rgba: (ov.u32(ov.at(CMD_LABEL_RGBA))?, ov.u32(ov.at(CMD_LABEL_RGBA) + 4)?),
            label_dy: ov.i32(ov.at(CMD_LABEL_DY))?,
        })
    }
}

/// What a tick did besides the state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QsEvents {
    /// The ring opened this tick (`HudShow(3, …)` applied → init).
    pub opened: bool,
    /// Health / bolt visible timers pushed to ≥ 180 (`0x24b4b0`).
    pub show_health_bolts: bool,
    /// The ring closed this tick.
    pub closed: bool,
    /// `0x141408` written by the confirm (item) or the double tap.
    pub request: Option<i32>,
    /// `0x141345 = 1` (Drone Device).
    pub drone: bool,
    /// PAD+0x1cc = 2 for the next `ProcessPadInput`.
    pub lock_pad: bool,
    /// The command menu's write of 0x141610 (`Bodies::command`) this tick.
    pub command: Option<i32>,
}

/// The ring: slot-3 fields and the hero-block timers.
#[derive(Clone, Debug, PartialEq)]
pub struct QuickSelect {
    pub consts: QsConsts,
    pub tables: QsTables,
    pub items: QsItems,
    /// Page-0 entries as the init fills them (icon, item) plus the static neighbours.
    pub entries: [Entry; SLOTS],
    /// 0x141622 lock countdown, 0x141624 tap timer, 0x141626 double-tap timer (hero block, s16).
    pub lock_timer: i16,
    pub tap_timer: i16,
    pub double_timer: i16,
    /// Slot 3 holds the ring element.
    pub open: bool,
    /// +0x74 selection (−2 not armed, −1 none), +0x78 arming countdown / 0x10000ff armed.
    pub sel: i32,
    pub arm: i32,
    /// +0x70 fade (0..steps), +0x71 text fade.
    pub fade: u8,
    pub text_fade: u8,
    /// 0x15fa98 / 0x15fa9c / 0x15fa94: +0x78, sel and the VSync counter at the last close.
    pub last_close: Option<(i32, i32, u32)>,
    /// Clank's command page (None on a level without one) and whether the open ring is it.
    pub commands: Option<Commands>,
    pub command_page: bool,
}

/// `FastDecTimer__FRs` (0x220ea8): non-zero when the timer is 0 or reaches 0 now.
fn fast_dec_timer(t: &mut i16) -> bool {
    if *t == 0 { return true; }
    *t = ((*t as i32).max(1) - 1) as i16;
    *t <= 0
}

/// `FastNormalizeAngle` (0x222088).
fn normalize(mut a: Pf) -> Pf {
    while a >= PI { a = (a - PI) - PI; }
    let npi = -PI;
    while a < npi { a = (a + PI) + PI; }
    a
}

/// The misc-record update the confirm and the double tap do (count +1 saturating at 0xffff when `bump`,
/// time/600 max, level bit).
fn stat(rec: &mut HelpRec, bump: bool, play_time: i32, level: i32) {
    if bump && rec.count != 0xffff { rec.count += 1; }
    let t = scale_ticks(play_time) / 600;
    if (rec.time as i32) < t { rec.time = t as u16; }
    rec.mask |= 1u32.wrapping_shl(level as u32 & 31) | 0x8000_0000;
}

impl QuickSelect {
    pub fn new(consts: QsConsts, tables: QsTables, items: QsItems) -> QuickSelect {
        QuickSelect {
            entries: tables.entries,
            consts,
            tables,
            items,
            lock_timer: 0,
            tap_timer: 0,
            double_timer: 0,
            open: false,
            sel: -2,
            arm: 0,
            fade: 0,
            text_fade: 0,
            last_close: None,
            commands: None,
            command_page: false,
        }
    }

    pub fn load(ov: &Overlay, items: &ItemTables) -> Option<QuickSelect> {
        let mut q = QuickSelect::new(QsConsts::load(ov)?, QsTables::load(ov)?, QsItems::load(ov, items)?);
        q.commands = Commands::load(ov);
        Some(q)
    }

    /// Box top-left (X0, Y0): the slot-3 anchor with its bits 4 (left edge, vertical centre) resolved by
    /// 0x24b108, offset +0x48/+0x4a = 0.
    pub fn origin(&self) -> (i32, i32) {
        let (ax, ay) = crate::hud::ANCHORS[HUD_SLOT];
        (ax, ay - self.consts.size.1 / 2)
    }

    /// `QuickSelectInit` 0x24d180.
    fn init(&mut self, g: &Global) {
        self.sel = -2;
        self.arm = scale_ticks(30);
        self.fade = 0;
        for (k, e) in self.entries.iter_mut().enumerate() {
            let item = g.quick_select[k];
            e.item = item;
            e.icon = self.items.icon_of(item);
        }
    }

    /// The icon word of entry `k` of the page table (k = −1 reads the word before the table).
    fn icon_at(&self, k: i32) -> i32 {
        match usize::try_from(k) {
            Ok(i) if i < SLOTS => self.entries[i].icon,
            _ if k == -1 => self.tables.entry_m1_icon,
            _ => 0,
        }
    }

    /// The hero update's part (0x242930, after its early state changes): the open test and the double tap.
    pub fn hero(&mut self, inp: &MenuInput, gate: &HeroGate, g: &mut Global, s: &mut SessionState) -> QsEvents {
        let mut ev = QsEvents::default();
        if gate.early_exit { return ev; }
        let tri = button::TRIANGLE;
        let base = gate.swap_state != 2
            && gate.f594 == 0
            && (gate.b13fc == 0 || gate.b13f4 == 1 || gate.s3f502 != 0)
            && (gate.b13f7 == 0 || gate.b13f4 == 1 || gate.s3f502 != 0);
        let allow = base && (gate.b13f4 == 0 || gate.b13f4 == 3);
        let pressed = inp.pressed & tri != 0 || inp.pressed_u & tri != 0;
        // Body 1: `HudShow(3, …)` with the command menu (no lock timer, no health / bolts).
        if pressed && base && gate.b13f4 == 1 && self.commands.is_some() && !(self.open && self.command_page) {
            self.open = true;
            self.command_page = true;
            self.init_commands();
            ev.opened = true;
        }
        if pressed && allow {
            self.lock_timer = scale_ticks(if self.double_timer != 0 { 27 } else { 15 }) as i16;
            if !self.open || self.command_page {
                // HudShow with arguments differing from the slot's last request (the empty element) → applied.
                self.open = true;
                self.command_page = false;
                self.init(g);
                ev.opened = true;
            }
            ev.show_health_bolts = true;
        }
        fast_dec_timer(&mut self.tap_timer);
        fast_dec_timer(&mut self.double_timer);
        if inp.pressed & tri != 0 { self.tap_timer = scale_ticks(20) as i16; }
        if (self.double_timer != 0 || self.tap_timer != 0) && inp.raw_released & tri != 0 {
            if self.double_timer == 0 {
                self.double_timer = scale_ticks(20) as i16;
            } else {
                let prev = if gate.held_item == WRENCH { g.equipped[0] } else { g.last_hand_item };
                if prev != 0 && gate.held_item != g.last_hand_item {
                    stat(&mut g.move_help[31], true, g.play_time, g.level);
                    self.double_timer = 0;
                    self.tap_timer = 0;
                    s.temp_hand = prev;
                    ev.request = Some(prev);
                }
            }
        }
        ev
    }

    /// The command menu's init (L00 `0x238b18`): 4 entries, the page 0, the box, cursor −2, `ticks(30)` to arm.
    fn init_commands(&mut self) {
        self.sel = -2;
        self.arm = scale_ticks(30);
    }

    /// `QuickSelectUpdate` 0x24d238 (only while the ring is open), or the command menu's ([`Self::update_commands`]).
    /// `vsync` = `0x15f3f8`; `listeners` = the gadgetbots' count (`Globals::bot_listeners`).
    pub fn update(&mut self, inp: &MenuInput, gate: &HeroGate, g: &mut Global, s: &mut SessionState, vsync: u32, listeners: i32) -> QsEvents {
        let mut ev = QsEvents::default();
        if !self.open { return ev; }
        if self.command_page { return self.update_commands(inp, vsync, listeners); }
        let n = SLOTS as i32;
        if fast_dec_timer(&mut self.lock_timer) { ev.lock_pad = true; }
        let (x, y) = (inp.stick_x, inp.stick_y);
        let len = len2(x, y);
        let (mut nx, mut ny) = (x, y);
        if !len.is_zero() {
            nx = nx / len;
            ny = ny / len;
        }
        let angle = fast_arctan(nx, ny);
        if self.arm >> 24 == 0 && (inp.pressed_u & 0xf000 != 0 || len < F0_5 || { self.arm -= 1; self.arm == -1 }) {
            self.sel = -1;
            self.arm = 0x0100_00ff;
        }
        let start = self.sel;
        let mut dir = 0i32;
        let mut cur = self.sel;
        let armed = (self.arm >> 24) & 0xff == 1;
        if armed && inp.held_u & button::TRIANGLE != 0 {
            if F0_9 < len {
                let nf = Pf::from_i32(n);
                let a = (((angle + PI) + PI) + HALF_PI) + PI / nf;
                let v = a * (nf / TWO_PI);
                let mut keep = false;
                if start != -1 {
                    let u = fast_subtract_rotations(angle, HALF_PI);
                    let d = fast_diff_rots((Pf::from_i32(start) * TWO_PI) / nf - PI, u);
                    keep = HYSTERESIS >= d;
                }
                if !keep {
                    let mut k = v.to_i32() % n;
                    if k & 1 == 0 {
                        let f = v - Pf::from_i32(v.to_i32());
                        if Pf::ONE - self.consts.edge < f {
                            k += 1;
                        } else if f < self.consts.edge {
                            k -= 1;
                        }
                        k = (k + n) % n;
                    }
                    self.sel = k;
                    let dev = normalize(a - (Pf::from_i32(k) * QUARTER_PI + EIGHTH_PI));
                    dir = if Pf::ZERO < dev { 1 } else { -1 };
                }
            } else if !inp.stick_active && inp.pressed_u & 0xf000 != 0 {
                let p = inp.pressed_u;
                let step = |sel: i32, bit: u32, def: i32, f: fn(&Entry) -> i32, e: &[Entry; SLOTS]| -> i32 {
                    if p & bit == 0 { return sel; }
                    if sel == -1 { return def; }
                    usize::try_from(sel).ok().and_then(|i| e.get(i)).map_or(sel, f)
                };
                let [dl, dr, du, dd] = self.tables.defaults;
                let e = &self.entries;
                let mut sel = step(start, button::UP, du, |e| e.up, e);
                sel = step(sel, button::DOWN, dd, |e| e.down, e);
                sel = step(sel, button::LEFT, dl, |e| e.left, e);
                sel = step(sel, button::RIGHT, dr, |e| e.right, e);
                self.sel = sel;
            }
            // LAB_0024d6ac: an empty slot hands the stick's choice to its neighbour in the swing direction.
            if self.icon_at(self.sel) == 0 && dir != 0 {
                let k = (self.sel + dir) % n;
                if self.icon_at(k) != 0 { self.sel = k; }
            }
            cur = self.sel;
        }
        if cur == start {
            if (self.text_fade as i32) < self.consts.steps { self.text_fade += 1; }
        } else {
            self.text_fade = 0;
        }
        if inp.held_u & button::TRIANGLE == 0 {
            if let Ok(i) = usize::try_from(self.sel) {
                let item = self.entries.get(i).map_or(0, |e| e.item);
                if item != 0 {
                    stat(&mut g.move_help[20], gate.held_item != item, g.play_time, g.level);
                    let req = if item == DRONE {
                        ev.drone = true;
                        s.drone = true;
                        s.temp_hand
                    } else {
                        item
                    };
                    s.temp_hand = req;
                    ev.request = Some(req);
                    if let Some(max) = self.items.ammo_of(item) {
                        let a = &mut g.ammo[item as usize];
                        if (max as i32) < *a { *a = max as i32; }
                    }
                }
            }
            if self.fade == 0 {
                self.close(vsync, &mut ev);
                return ev;
            }
            self.fade -= 1;
        } else if (self.fade as i32) < self.consts.steps {
            self.fade += 1;
        }
        if self.fade == 0 { self.close(vsync, &mut ev); }
        ev
    }

    /// The command menu's update (L00 `0x238b80`, module doc).
    fn update_commands(&mut self, inp: &MenuInput, vsync: u32, listeners: i32) -> QsEvents {
        let mut ev = QsEvents { lock_pad: true, ..Default::default() };
        let Some(cmd) = self.commands.clone() else { return ev };
        let n = CMD_SLOTS as i32;
        let (x, y) = (inp.stick_x, inp.stick_y);
        let len = len2(x, y);
        let (mut nx, mut ny) = (x, y);
        if !len.is_zero() {
            nx = nx / len;
            ny = ny / len;
        }
        let angle = fast_arctan(nx, ny);
        if self.arm >> 24 == 0 && listeners != 0 && (inp.pressed_u & 0xf000 != 0 || len < F0_5 || { self.arm -= 1; self.arm == -1 }) {
            self.sel = -1;
            self.arm = 0x0100_00ff;
        }
        let start = self.sel;
        if (self.arm >> 24) as u8 == 1 {
            if F0_9 < len {
                let nf = Pf::from_i32(n);
                self.sel = ((((((angle + PI) + PI) + HALF_PI) + PI / nf) * (nf / TWO_PI)).to_i32()) % n;
            }
            let p = inp.pressed_u;
            if p & 0xf000 != 0 {
                let e = &cmd.entries;
                let step = |sel: i32, bit: u32, def: i32, f: fn(&Entry) -> i32| -> i32 {
                    if p & bit == 0 { return sel; }
                    if sel == -1 { return def; }
                    usize::try_from(sel).ok().and_then(|i| e.get(i)).map_or(sel, f)
                };
                let [dl, dr, du, dd] = cmd.defaults;
                let mut sel = step(self.sel, button::UP, du, |e| e.up);
                sel = step(sel, button::DOWN, dd, |e| e.down);
                sel = step(sel, button::LEFT, dl, |e| e.left);
                sel = step(sel, button::RIGHT, dr, |e| e.right);
                self.sel = sel;
            }
        }
        if self.sel == start {
            if (self.text_fade as i32) < self.consts.steps { self.text_fade += 1; }
        } else {
            self.text_fade = 0;
        }
        if inp.held_u & button::TRIANGLE == 0 {
            ev.command = Some(match self.sel {
                -1 => 0,
                -2 => self.entries[SLOTS - 2].item,
                k => usize::try_from(k).ok().and_then(|i| cmd.entries.get(i)).map_or(0, |e| e.item),
            });
            if self.fade == 0 {
                self.close(vsync, &mut ev);
                return ev;
            }
            self.fade -= 1;
        } else if (self.fade as i32) < self.consts.steps {
            self.fade += 1;
        }
        if self.fade == 0 { self.close(vsync, &mut ev); }
        ev
    }

    fn close(&mut self, vsync: u32, ev: &mut QsEvents) {
        self.last_close = Some((self.arm, self.sel, vsync));
        self.open = false;
        ev.closed = true;
    }

    /// Slot i's centre (px, py) (the draw's loop).
    pub fn slot_pos(&self, i: usize) -> (i32, i32) { self.ring_pos(i, SLOTS) }

    /// Slot i's centre on a ring of `n`.
    fn ring_pos(&self, i: usize, n: usize) -> (i32, i32) {
        let (x0, y0) = self.origin();
        let n = Pf::from_i32(n as i32);
        let i = Pf::from_i32(i as i32);
        let th = fast_add_rotations(((i + i) * PI) / n - PI, HALF_PI);
        let px = x0 + self.consts.centre.0 + (self.consts.radius * fast_cos(th) * self.consts.x_scale).to_i32();
        let py = y0 + self.consts.centre.1 + (self.consts.radius * fast_sin(th)).to_i32();
        (px, py)
    }

    /// `QuickSelectDraw` 0x24d938. `tick` = the game tick 0x15f5cc (cursor pulse).
    pub fn draw(&self, a: &MenuAssets, g: &Global, tick: u64, listeners: i32, out: &mut Vec<MenuDraw>) {
        if !self.open { return; }
        if self.command_page {
            self.draw_commands(a, tick, listeners, out);
            return;
        }
        let c = &self.consts;
        let steps = Pf::from_i32(c.steps);
        let clamp01 = |v: Pf| if Pf::ONE < v { Pf::ONE } else if v < Pf::ZERO { Pf::ZERO } else { v };
        let s = clamp01(Pf::from_i32(self.fade as i32) / steps);
        let f = clamp01(Pf::from_i32(self.text_fade as i32) / steps);
        let per = c.period.max(1);
        let t = Pf::from_i32((tick % per as u64) as i32);
        let pulse = crate::hero::physics::fast_sin((t / Pf::from_i32(per)) * PULSE_2PI - PULSE_PI);
        let s128 = s * F128;
        let alpha = s128.to_i32();
        let cursor_alpha = (s128 * (pulse * F0_125 + F0_875)).to_i32();
        let (x0, y0) = self.origin();
        let (w, h) = c.size;
        let (hw, hh) = (w / 2, h / 2);
        let ring = a.frame(RING_ICON, 0);
        sprite(out, ring, x0, y0, hw, hh, alpha);
        sprite(out, ring, x0 + w, y0 + h, -hw, -hh, alpha);
        sprite(out, ring, x0 + w, y0, -hw, hh, alpha);
        sprite(out, ring, x0, y0 + h, hw, -hh, alpha);
        for i in 0..SLOTS {
            let (px, py) = self.slot_pos(i);
            let e = &self.entries[i];
            if self.sel == i as i32 && e.icon != 0 {
                sprite(out, a.frame(CURSOR_ICON, 0), px - 19, py - 19, 38, 38, cursor_alpha);
            }
            if e.icon != 0 {
                let al = if i as i32 != self.sel { (Pf::from_i32(alpha) * c.dim).to_i32() } else { alpha };
                let gold = usize::try_from(e.item).ok().and_then(|k| g.gold_weapons.get(k)).is_some_and(|&b| b != 0);
                let fr = a.frame(e.icon as u16, if gold { 4 } else { 0 });
                let (tw, th) = a.frame_size(fr);
                // HudFrame flags 1: centred on the frame's texture size; slot offset +0x48/+0x4a = 0.
                sprite(out, fr, px - (tw >> 1), py - (th >> 1), tw, th, al);
            }
        }
        let Ok(si) = usize::try_from(self.sel) else { return };
        let Some(e) = self.entries.get(si).filter(|e| e.icon != 0) else { return };
        let sf = (s * f).to_f32();
        let shadow = tween(sf, c.shadow_rgba.0, c.shadow_rgba.1);
        let (cx, cy) = (x0 + c.centre.0, y0 + c.centre.1);
        let centre = |out: &mut Vec<MenuDraw>, x: i32, y: i32, rgba: u32, t: &[u8]| {
            let wd = a.width(Font::Regular, t);
            text(out, Font::Regular, x - (wd >> 1), y, rgba, t);
        };
        let slot_item = g.quick_select[si];
        let has_ammo = self.items.ammo_of(slot_item);
        if let Some(max) = has_ammo {
            let ammo = g.ammo.get(slot_item as usize).copied().unwrap_or(0);
            let s_txt = format_ammo(&c.ammo_format, ammo, max as i32);
            let (from, to) = if ammo != 0 { c.ammo_rgba } else { c.ammo_empty_rgba };
            let col = tween(sf, from, to);
            centre(out, cx + 1, cy + 13, shadow, &s_txt);
            centre(out, cx, cy + 12, col, &s_txt);
        }
        let col = tween(sf, c.name_rgba.0, c.name_rgba.1);
        let item = e.item;
        let gold = usize::try_from(item).ok().and_then(|k| g.gold_weapons.get(k)).is_some_and(|&b| b != 0);
        let id = usize::try_from(item).ok().and_then(|k| if gold { self.items.gold_name.get(k) } else { self.items.name.get(k) }).copied().unwrap_or(0);
        let name = a.msg(id as i32);
        if name.is_empty() { return; }
        let split = split_name(name);
        let dy = if has_ammo.is_some() { c.name_dy[0] } else if split.is_some() { c.name_dy[1] } else { c.name_dy[2] };
        let y = cy + dy;
        match split {
            Some((l1, rest)) => {
                centre(out, cx + 1, y + 1, shadow, &name[..l1]);
                centre(out, cx, y, col, &name[..l1]);
                centre(out, cx + 1, y + 0x11, shadow, &name[rest..]);
                centre(out, cx, y + 0x10, col, &name[rest..]);
            }
            None => {
                centre(out, cx + 1, y + 1, shadow, name);
                centre(out, cx, y, col, name);
            }
        }
    }
}

impl QuickSelect {
    /// The command menu's draw (L00 `0x238f88`, module doc).
    fn draw_commands(&self, a: &MenuAssets, tick: u64, listeners: i32, out: &mut Vec<MenuDraw>) {
        let Some(cmd) = self.commands.as_ref() else { return };
        let c = &self.consts;
        let steps = Pf::from_i32(c.steps);
        let clamp01 = |v: Pf| if Pf::ONE < v { Pf::ONE } else if v < Pf::ZERO { Pf::ZERO } else { v };
        let s = clamp01(Pf::from_i32(self.fade as i32) / steps);
        let f = clamp01(Pf::from_i32(self.text_fade as i32) / steps);
        let per = c.period.max(1);
        let t = Pf::from_i32((tick % per as u64) as i32);
        let pulse = crate::hero::physics::fast_sin((t / Pf::from_i32(per)) * PULSE_2PI - PULSE_PI);
        let s128 = s * F128;
        let alpha = s128.to_i32();
        let cursor_alpha = (s128 * (pulse * F0_125 + F0_875)).to_i32();
        let (x0, y0) = self.origin();
        let (w, h) = c.size;
        let (hw, hh) = (w / 2, h / 2);
        let ring = a.frame(RING_ICON, 1);
        sprite(out, ring, x0, y0, hw, hh, alpha);
        sprite(out, ring, x0 + w, y0 + h, -hw, -hh, alpha);
        sprite(out, ring, x0 + w, y0, -hw, hh, alpha);
        sprite(out, ring, x0, y0 + h, hw, -hh, alpha);
        for (i, e) in cmd.entries.iter().enumerate() {
            let (px, py) = self.ring_pos(i, CMD_SLOTS);
            if self.sel == i as i32 && e.icon != 0 {
                sprite(out, a.frame(CURSOR_ICON, 0), px - 19, py - 19, 38, 38, cursor_alpha);
            }
            if e.icon == 0 { continue; }
            let (fr, al) = if listeners == 0 {
                (a.frame(e.icon as u16, e.f4 + 4), alpha)
            } else {
                let al = if i as i32 != self.sel { (Pf::from_i32(alpha) * c.dim).to_i32() } else { alpha };
                (a.frame(e.icon as u16, e.f4), al)
            };
            let (tw, th) = a.frame_size(fr);
            sprite(out, fr, px - (tw >> 1), py - (th >> 1), tw, th, al);
        }
        let Ok(si) = usize::try_from(self.sel) else { return };
        if cmd.entries.get(si).is_none_or(|e| e.icon == 0) { return; }
        let label = a.msg(cmd.msgs[si]);
        if label.is_empty() { return; }
        let sf = (f * s).to_f32();
        let col = tween(sf, cmd.label_rgba.0, cmd.label_rgba.1);
        let shadow = tween(sf, 0, 0x8000_0000);
        let (cx, y) = (x0 + c.centre.0, y0 + cmd.label_dy + c.centre.1);
        let wd = a.width(Font::Regular, label);
        text(out, Font::Regular, cx - 1 - (wd >> 1), y - 1, shadow, label);
        text(out, Font::Regular, cx - (wd >> 1), y, col, label);
    }
}

/// `sprintf(buf, "%d/%d", a, b)` with the overlay's format (only `%d` conversions).
fn format_ammo(fmt: &[u8], a: i32, b: i32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut args = [a, b].into_iter();
    let mut i = 0;
    while i < fmt.len() {
        if fmt[i] == b'%' && fmt.get(i + 1) == Some(&b'd') {
            out.extend(args.next().unwrap_or(0).to_string().bytes());
            i += 2;
        } else {
            out.push(fmt[i]);
            i += 1;
        }
    }
    out
}

/// The draw's name split: the first '-' at index j ≥ 1 → line 1 = bytes 0..=j, line 2 from j+1; else the first
/// ' ' at j ≥ 1 → line 1 = bytes 0..j, line 2 from j+1. Returns (line-1 length, line-2 start).
pub fn split_name(s: &[u8]) -> Option<(usize, usize)> {
    let n = s.len();
    if n == 0 { return None; }
    if let Some(j) = (1..n).find(|&j| s[j] == b'-') { return Some((j + 1, j + 1)); }
    (1..n).find(|&j| s[j] == b' ').map(|j| (j, j + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::Global;
    use crate::hud::HudAssets;
    use crate::pad::{PadInput, PadState};
    use rc_formats::font::{Glyph, GLYPHS};

    /// The level-01 values (docs/plan/menus.md §2) for the unit tests; the overlay test below checks them.
    fn consts() -> QsConsts {
        QsConsts {
            size: (210, 200),
            centre: (105, 100),
            steps: 8,
            edge: Pf::b(0x3e4c_cccd),
            radius: Pf::b(0x4294_0000),
            x_scale: Pf::b(0x3f86_6666),
            dim: Pf::b(0x3f00_0000),
            name_rgba: (0x00ff_8080, 0x80ff_8080),
            shadow_rgba: (0, 0x8000_0000),
            ammo_empty_rgba: (0x0040_40ff, 0x8040_40ff),
            ammo_rgba: (0x0040_e020, 0x8040_e020),
            period: 30,
            name_dy: [-25, -15, 0],
            ammo_format: b"%d/%d".to_vec(),
        }
    }

    fn tables() -> QsTables {
        let lrud = [[7, 1, 0, 4], [0, 2, 0, 2], [6, 2, 1, 3], [4, 2, 2, 4], [5, 3, 0, 4], [6, 4, 6, 4], [6, 2, 7, 5], [6, 0, 0, 6]];
        let entries = lrud.map(|[l, r, u, d]| Entry { left: l, right: r, up: u, down: d, ..Default::default() });
        QsTables { entries, defaults: [6, 2, 0, 4], entry_m1_icon: 314159, table_addr: 0x17df40 }
    }

    fn items() -> QsItems {
        let n = ITEM_COUNT;
        let mut q = QsItems { icon: vec![0; n], name: vec![0; n], gold_name: vec![0; n], has_ammo: vec![false; n], max_ammo: vec![0; n] };
        for i in 9..n { q.icon[i] = 60000 + i as u16; }
        q.has_ammo[10] = true;
        q.max_ammo[10] = 40;
        q
    }

    fn ring() -> QuickSelect { QuickSelect::new(consts(), tables(), items()) }

    fn global(slots: [i32; 8]) -> Global {
        let mut g = Global::zeroed();
        g.quick_select = slots;
        g
    }

    /// Pad frames → MenuInput through the real `ProcessPadInput` port.
    struct Pad(PadState);
    impl Pad {
        fn step(&mut self, p: PadInput) -> MenuInput {
            self.0.update(Some(&p.bytes()), false);
            MenuInput::from_pad(&self.0, true)
        }
    }

    fn tick(q: &mut QuickSelect, pad: &mut Pad, p: PadInput, g: &mut Global, s: &mut SessionState) -> (QsEvents, QsEvents) {
        let inp = pad.step(p);
        let gate = HeroGate::default();
        let h = q.hero(&inp, &gate, g, s);
        let u = q.update(&inp, &gate, g, s, 0, 0);
        if u.lock_pad { pad.0.lock = 2; }
        (h, u)
    }

    #[test]
    fn layout_positions_for_8_slots() {
        let q = ring();
        let want = [(125, 134), (179, 156), (202, 208), (179, 260), (125, 282), (71, 260), (48, 208), (71, 156)];
        assert_eq!(q.origin(), (20, 108));
        for (i, w) in want.iter().enumerate() {
            let p = q.slot_pos(i);
            assert!((p.0 - w.0).abs() <= 1 && (p.1 - w.1).abs() <= 1, "slot {i}: {p:?} vs {w:?}");
        }
    }

    fn stick_sel(q: &mut QuickSelect, deg: f32) -> i32 {
        let (s, c) = deg.to_radians().sin_cos();
        let inp = MenuInput { held_u: button::TRIANGLE, stick_x: Pf::f(c), stick_y: Pf::f(s), stick_active: true, ..Default::default() };
        let (mut g, mut st) = (global([10, 11, 12, 13, 14, 15, 16, 17]), SessionState::default());
        q.update(&inp, &HeroGate::default(), &mut g, &mut st, 0, 0);
        q.sel
    }

    fn armed(sel: i32) -> QuickSelect {
        let mut q = ring();
        q.init(&global([10, 11, 12, 13, 14, 15, 16, 17]));
        q.open = true;
        q.arm = 0x0100_00ff;
        q.sel = sel;
        q
    }

    #[test]
    fn sector_edges_27_and_63_degrees_with_hysteresis() {
        // Screen angle (y down): 0° = right = slot 2, −90° = up = slot 0, 45° = down-right = slot 3.
        let from_none = |d: f32| stick_sel(&mut armed(-1), d);
        assert_eq!(from_none(-90.0), 0);
        assert_eq!(from_none(0.0), 2);
        assert_eq!(from_none(45.0), 3);
        assert_eq!(from_none(135.0), 5);
        // The cardinal sector of slot 2 is ±13.5° (27° wide): frac(v) = 0.5 ± 0.3 at its edges.
        assert_eq!(from_none(13.0), 2);
        assert_eq!(from_none(14.0), 3);
        assert_eq!(from_none(-13.0), 2);
        assert_eq!(from_none(-14.0), 1);
        // Diagonal slot 3 spans 13.5°..76.5° (63°).
        assert_eq!(from_none(76.0), 3);
        assert_eq!(from_none(77.0), 4);
        // Hysteresis: keep the slot while within 3π/16 (33.75°) of its centre.
        assert_eq!(stick_sel(&mut armed(2), 30.0), 2);
        assert_eq!(stick_sel(&mut armed(2), 35.0), 3);
        assert_eq!(stick_sel(&mut armed(3), 12.0), 3);
        assert_eq!(stick_sel(&mut armed(3), 10.0), 2);
    }

    fn dpad(q: &mut QuickSelect, bits: u32) -> i32 {
        let inp = MenuInput { held_u: button::TRIANGLE, pressed_u: bits, ..Default::default() };
        let (mut g, mut st) = (global([10, 11, 12, 13, 14, 15, 16, 17]), SessionState::default());
        q.update(&inp, &HeroGate::default(), &mut g, &mut st, 0, 0);
        q.sel
    }

    #[test]
    fn dpad_chains_from_none_and_each_slot() {
        let (u, d, l, r) = (button::UP, button::DOWN, button::LEFT, button::RIGHT);
        assert_eq!(dpad(&mut armed(-1), l), 6);
        assert_eq!(dpad(&mut armed(-1), r), 2);
        assert_eq!(dpad(&mut armed(-1), u), 0);
        assert_eq!(dpad(&mut armed(-1), d), 4);
        assert_eq!(dpad(&mut armed(-1), u | r), 1, "Up then Right: −1 → 0 → 1");
        let lrud = [[7, 1, 0, 4], [0, 2, 0, 2], [6, 2, 1, 3], [4, 2, 2, 4], [5, 3, 0, 4], [6, 4, 6, 4], [6, 2, 7, 5], [6, 0, 0, 6]];
        for (s, [el, er, eu, ed]) in lrud.iter().enumerate() {
            let s = s as i32;
            assert_eq!(dpad(&mut armed(s), l), *el, "slot {s} left");
            assert_eq!(dpad(&mut armed(s), r), *er, "slot {s} right");
            assert_eq!(dpad(&mut armed(s), u), *eu, "slot {s} up");
            assert_eq!(dpad(&mut armed(s), d), *ed, "slot {s} down");
        }
    }

    #[test]
    fn tap_closes_without_equip() {
        let (mut q, mut pad, mut g, mut s) = (ring(), Pad(PadState::default()), global([10, 0, 0, 0, 0, 0, 0, 0]), SessionState::default());
        let tri = PadInput::neutral().press(button::TRIANGLE);
        let (h, u) = tick(&mut q, &mut pad, tri, &mut g, &mut s);
        assert!(h.opened && !u.closed);
        assert_eq!((q.sel, q.fade), (-1, 1));
        let (_, u) = tick(&mut q, &mut pad, PadInput::neutral(), &mut g, &mut s);
        // Released at fade 1 with nothing selected: fade → 0 → closed, no request.
        assert!(u.closed && u.request.is_none());
        assert_eq!(s.temp_hand, 0);
        assert!(!q.open);
    }

    #[test]
    fn hold_then_release_equips_and_locks_after_15_ticks() {
        let (mut q, mut pad, mut g, mut s) = (ring(), Pad(PadState::default()), global([10, 0, 0, 0, 0, 0, 0, 0]), SessionState::default());
        g.ammo[10] = 55;
        let tri = PadInput::neutral().press(button::TRIANGLE);
        let mut locks = vec![];
        for t in 0..20 {
            let p = if t >= 3 { tri.stick(0.0, -1.0) } else { tri };
            let (_, u) = tick(&mut q, &mut pad, p, &mut g, &mut s);
            locks.push(u.lock_pad);
        }
        assert_eq!(locks.iter().position(|&l| l), Some(14), "the lock starts when the 15-tick timer runs out");
        assert!(locks[14..].iter().all(|&l| l));
        assert_eq!((q.sel, q.fade), (0, 8));
        let mut closed_at = None;
        for t in 0..10 {
            let (_, u) = tick(&mut q, &mut pad, PadInput::neutral(), &mut g, &mut s);
            assert_eq!(u.request.is_some(), t < 8, "tick {t}");
            if u.closed { closed_at = Some(t); }
        }
        assert_eq!(closed_at, Some(7));
        assert_eq!(s.temp_hand, 10);
        assert_eq!(g.ammo[10], 40, "ammo clamped to the max");
        assert_eq!(g.move_help[20].count, 8, "the stats count rises on every fade-out tick");
    }

    #[test]
    fn double_tap_window_20_20() {
        let run = |gap: usize| -> Option<i32> {
            let (mut q, mut pad, mut g, mut s) = (ring(), Pad(PadState::default()), global([10, 0, 0, 0, 0, 0, 0, 0]), SessionState::default());
            g.last_hand_item = 15;
            let gate = HeroGate { held_item: 10, ..Default::default() };
            let tri = PadInput::neutral().press(button::TRIANGLE);
            let mut req = None;
            let seq: Vec<PadInput> = [vec![tri], vec![PadInput::neutral(); gap], vec![tri], vec![PadInput::neutral(); 3]].concat();
            for p in seq {
                let inp = pad.step(p);
                let h = q.hero(&inp, &gate, &mut g, &mut s);
                q.update(&inp, &gate, &mut g, &mut s, 0, 0);
                if h.request.is_some() { req = h.request; }
            }
            req
        };
        // Second release within the double-tap window (first release starts 20 ticks) → previous weapon.
        assert_eq!(run(1), Some(15));
        assert_eq!(run(18), Some(15));
        assert_eq!(run(19), None);
        assert_eq!(run(25), None);
    }

    /// The overlay's tables and constants are the values above (skipped without `extracted/`).
    #[test]
    fn overlay_tables_match_level01() {
        let p = rc_formats::test_data::root().join("levels/01/overlay.bin");
        let Ok(bytes) = std::fs::read(&p) else { eprintln!("skipped: no {}", p.display()); return };
        let ov = Overlay::parse(&bytes).unwrap();
        assert_eq!(QsConsts::load(&ov).unwrap(), consts());
        assert_eq!(QsTables::load(&ov).unwrap(), tables());
    }

    #[test]
    fn name_split_rules() {
        assert_eq!(split_name(b"Bomb Glove"), Some((4, 5)));
        assert_eq!(split_name(b"Morph-o-Ray"), Some((6, 6)));
        assert_eq!(split_name(b"R.Y.N.O."), None);
        assert_eq!(split_name(b" x"), None);
    }

    #[test]
    fn draw_emits_ring_icons_cursor_and_text() {
        let mut glyphs = [[Glyph::default(); GLYPHS]; 3];
        for t in glyphs.iter_mut() { for g in &mut t[0x20..0x7f] { g.advance = 10; } }
        let hud = HudAssets { icons: vec![], frame_sizes: vec![(32, 32); 4], glyphs, messages: vec![] };
        let a = MenuAssets::new(hud, Overlay::default());
        let mut q = armed(0);
        q.fade = 8;
        q.text_fade = 8;
        let mut g = global([10, 11, 12, 13, 14, 15, 16, 17]);
        g.ammo[10] = 12;
        let mut out = vec![];
        q.draw(&a, &g, 0, 0, &mut out);
        let sprites: Vec<_> = out.iter().filter_map(|d| match d { MenuDraw::Hud(crate::hud::Draw::Sprite { x, y, w, h, alpha, .. }) => Some((*x, *y, *w, *h, *alpha)), _ => None }).collect();
        assert_eq!(&sprites[..4], &[(20, 108, 105, 100, 128), (230, 308, -105, -100, 128), (230, 108, -105, 100, 128), (20, 308, 105, -100, 128)]);
        // Cursor on slot 0 (t = 0: sin(−π) ≈ 0 → 0.875·128 = 112), then the 8 icons (unselected at alpha 64).
        assert_eq!(sprites[4].2, 38);
        assert!((sprites[4].4 - 112).abs() <= 1);
        assert_eq!(sprites.len(), 4 + 1 + 8);
        assert_eq!(sprites[6].4, 64);
        let texts = out.iter().filter(|d| matches!(d, MenuDraw::Hud(crate::hud::Draw::Text { .. }))).count();
        // Ammo + shadow, then the name: id 0 resolves to the "Paradox! …" fallback, split at its first space.
        assert_eq!(texts, 6);
    }
}
