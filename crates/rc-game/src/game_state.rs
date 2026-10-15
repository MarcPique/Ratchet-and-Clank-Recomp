//! The persistent game state (everything the memory card saves), new game, the per-level-start rules,
//! space transitions, the save paths and the options. Spec: `docs/plan/game_state.md`; the on-card format
//! is [`rc_formats::save_game`].
//!
//! [`GameState`] maps every chunk of the two descriptor tables (read from the boot ELF) to a typed field
//! by chunk id: [`Global`] (47 chunks, ids 0..37 and 1000..1011) and one [`LevelState`] per level slot (11
//! chunks, ids 3001..3008 and 4000..4002). A chunk id the tables carry but this module does not type, or
//! whose table size differs from the typed field, is kept as raw bytes, so [`GameState::to_save_file`] ∘
//! [`GameState::from_save_file`] is byte-exact for every file the game writes (pad bytes included).
//!
//! Session-only fields (not saved: the hero block 0x13f350..0x141660 incl. HP, the PAL tick scale) live in
//! [`SessionState`].
//!
//! Ported code, L01 = level01 overlay, boot addresses in brackets:
//! * `RestoreData` 0x20af20 → [`GameState::restore`]; `RestoreGame` 0x209298 → [`GameState::load_card_file`];
//!   new game `load_and_initialize_level_chunk` 0x209370 → [`GameState::new_game`].
//! * `memcard_Save` 0x20b178 + `memcard_Update` state 0x10 → [`GameState::save_incremental`];
//!   `MakeWholeSave` 0x20abb0 → [`GameState::to_save_file`].
//! * Level start: L01 `FUN_00251c30` / `FUN_00251da0` / `FUN_00251fe0`, `GiveItem` 0x275760, planet unlock
//!   0x2756d0, level `entry` 0x259c40 (visited, record 3007), hero init 0x226b70 → [`GameState::apply_level_start`].
//! * `DoSpaceTransition` [0x231ff0] → [`GameState::apply_transition`].
//! * Veldin class 834 (L00 0x2d9dc8) first update → [`GameState::on_veldin_clank_init`].
//! * The landmark capture `fun_00208770` (L01 0x25eb68) → [`GameState::capture_landmarks`]; challenge mode's state
//!   `fun_00226b08` (L01 0x29a710) → [`GameState::challenge_reset`]. The card itself, `memcard_Save`'s queueing, the slot
//!   previews and the whole saves of the menus are [`crate::memcard`].
//!
//! Not modelled (presentation only): item-pickup help sounds (0x1b07c0 / 0x2789e0), the new-planet message
//! (0x277c38), space cutscenes.

use crate::follow_camera::{CameraOptions, YAW_RATES};
use crate::ps2v::Pf;
use crate::tick::GameOptions;
use rc_formats::save_game::{section_size, Chunk, ChunkDesc, ChunkTables, ItemTables, SaveFile, Section, ITEM_COUNT, LEVEL_SLOTS};
use rc_formats::FormatError;
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, FormatError>;

/// Item ids used by the rules below (`docs/plan/game_state.md` §1).
pub mod item {
    pub const HELI_PACK: usize = 2;
    pub const THRUSTER_PACK: usize = 3;
    pub const WRENCH: usize = 8;
    pub const BOMB_GLOVE: usize = 10;
    pub const PYROCITOR: usize = 16;
}

/// Global flag set by Veldin's Clank (class 834) and read by the Veldin music rule.
pub const FLAG_VELDIN_CLANK: usize = 8;
/// Vendor stock byte: low 6 bits item, 0x40 owned (the vendor sells its ammo), 0xff empty.
pub const VENDOR_OWNED: u8 = 0x40;
pub const VENDOR_EMPTY: u8 = 0xff;
/// Levels the game has (0 Veldin .. 18 Veldin finale); `apply_level_start` takes its debug branch from here.
pub const LEVEL_COUNT: i32 = 19;
/// `FUN_00251c30`: elapsed-time bonus per level load (60 Hz ticks).
pub const LEVEL_LOAD_ELAPSED: i32 = 180;

// ---------------------------------------------------------------------------------------------------
// Little-endian field (de)serialisation.

/// A fixed-size little-endian value of a chunk.
pub trait Field: Sized {
    const SIZE: usize;
    fn put(&self, out: &mut Vec<u8>);
    /// Reads from the first `SIZE` bytes of `b`.
    fn get(b: &[u8]) -> Self;
}

macro_rules! prim_field {
    ($($t:ty),*) => { $(
        impl Field for $t {
            const SIZE: usize = std::mem::size_of::<$t>();
            fn put(&self, out: &mut Vec<u8>) { out.extend_from_slice(&self.to_le_bytes()); }
            fn get(b: &[u8]) -> Self { <$t>::from_le_bytes(b[..Self::SIZE].try_into().unwrap()) }
        }
    )* };
}
prim_field!(u8, i16, u16, i32, u32);

impl Field for f32 {
    const SIZE: usize = 4;
    fn put(&self, out: &mut Vec<u8>) { out.extend_from_slice(&self.to_bits().to_le_bytes()); }
    fn get(b: &[u8]) -> Self { f32::from_bits(u32::get(b)) }
}

impl<T: Field, const N: usize> Field for [T; N] {
    const SIZE: usize = T::SIZE * N;
    fn put(&self, out: &mut Vec<u8>) { for x in self { x.put(out); } }
    fn get(b: &[u8]) -> Self { std::array::from_fn(|i| T::get(&b[i * T::SIZE..])) }
}

macro_rules! record_field {
    ($name:ident { $($f:ident : $t:ty),* }) => {
        impl Field for $name {
            const SIZE: usize = 0 $(+ <$t as Field>::SIZE)*;
            fn put(&self, out: &mut Vec<u8>) { $( self.$f.put(out); )* }
            #[allow(unused_assignments)]
            fn get(b: &[u8]) -> Self {
                let mut o = 0;
                $( let $f = <$t>::get(&b[o..]); o += <$t as Field>::SIZE; )*
                $name { $($f),* }
            }
        }
    };
}

/// Help / move / level-entry record `{u16 count, u16 time/600, u32 level mask | 0x80000000}` (chunks
/// 16, 17, 18, 3007). The level entry treats `count == 0xffff` as "stop counting".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HelpRec { pub count: u16, pub time: u16, pub mask: u32 }
record_field!(HelpRec { count: u16, time: u16, mask: u32 });

/// Map landmark `{f32 x, f32 y, f32 rot, u32 flags}` (chunk 15, copied from mobys at save by 0x208770).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Landmark { pub x: f32, pub y: f32, pub rot: f32, pub flags: u32 }
record_field!(Landmark { x: f32, y: f32, rot: f32, flags: u32 });

/// Chunk 3006 entry `{s16, s16 bolts collected}` of drop group `moby+0xb1`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoltDrop { pub first: i16, pub collected: i16 }
record_field!(BoltDrop { first: i16, collected: i16 });

/// Declares a chunk struct: `id field: Type` per chunk, plus the by-id (de)serialisers.
macro_rules! chunk_struct {
    ($(#[$m:meta])* $name:ident { $( $(#[$fm:meta])* $id:literal $field:ident : $ty:ty ),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Debug, PartialEq)]
        pub struct $name { $( $(#[$fm])* pub $field: $ty ),* }

        impl $name {
            /// Chunk ids with a typed field, in declaration order.
            pub const IDS: &'static [i32] = &[$($id),*];
            /// All fields zero (the state before the template is restored).
            pub fn zeroed() -> Self { $name { $( $field: <$ty as Field>::get(&[0u8; <$ty as Field>::SIZE]) ),* } }
            /// Byte size of the typed field of chunk `id`.
            pub fn field_size(id: i32) -> Option<usize> { match id { $( $id => Some(<$ty as Field>::SIZE), )* _ => None } }
            fn put(&self, id: i32, out: &mut Vec<u8>) -> bool { match id { $( $id => { self.$field.put(out); true } )* _ => false } }
            fn set(&mut self, id: i32, b: &[u8]) -> bool { match id { $( $id => { self.$field = <$ty>::get(b); true } )* _ => false } }
        }
    };
}

chunk_struct! {
    /// The 47 global chunks (table boot 0x1a04c0), `docs/plan/game_state.md` §1. Addresses are the EE copies.
    Global {
        /// 0x15ed84: current level (template −1 = empty slot).
        0 level: i32,
        /// 0x15ed98.
        1 bolts: i32,
        /// 0x15ee20: times completed (challenge mode +1).
        2 completes: i32,
        /// 0x15ee24: elapsed time, 60 Hz ticks (+180 per level load).
        3 elapsed: i32,
        /// 0x15ee98: last-save `sceCdCLOCK` (BCD).
        4 save_clock: [u8; 8],
        /// 0x13d388: global flags ([4] premium / [5] ultra nanotech, [8] Veldin Clank).
        5 flags: [u8; 128],
        /// 0x15edb0: cheats active ([4] mirror 0x15edb4, [5] mirrored anim 0x15edb5).
        7 cheats_active: [u8; 12],
        /// 0x13d408.
        8 skill_points: [u8; 32],
        /// 0x13d428: ammo by item id.
        9 ammo: [i32; 37],
        /// 0x13d4c0: item owned.
        10 owned: [u8; 37],
        /// 0x13d4e8: item ever acquired.
        11 acquired: [u8; 37],
        /// 0x15edd0: vendor stock (low 6 bits item, 0x40 owned, 0xff empty).
        12 vendor: [u8; 12],
        /// 0x141ea0: quick-select item ids (0 empty).
        13 quick_select: [i32; 8],
        /// 0x15eda0: max HP (4; 5 / 8 with the nanotech upgrades).
        19 max_hp: i32,
        /// 0x13dd40: planet unlocked (ship destinations).
        14 planet_unlocked: [u8; 20],
        /// 0x13d510: galaxy-map order (planet ids in unlock order).
        20 map_order: [i32; 20],
        /// 0x13d5b0.
        15 landmarks: [Landmark; 121],
        /// 0x141968.
        16 help: [HelpRec; 148],
        /// 0x141848: move help records.
        17 move_help: [HelpRec; 36],
        /// 0x141720: gadget help records.
        18 gadget_help: [HelpRec; 37],
        /// 0x15ed8c.
        21 last_hand_item: i32,
        /// 0x15ed90: hand shows the wrench (item 8).
        22 wrench_held: i32,
        /// 0x15ed94: back shows the thruster pack (item 3) instead of the heli-pack.
        23 thruster_last: i32,
        /// 0x15ed9c: no reference in the code.
        24 unused_24: i32,
        /// 0x15ede0: camera left/right 1 Normal / 0 Reversed.
        25 cam_yaw_normal: i32,
        /// 0x15eddc: camera up/down 1 Normal / 0 Reversed.
        26 cam_pitch_normal: i32,
        /// 0x15ede4: rotation speed 0 slow / 1 medium / 2 fast.
        27 cam_speed: i32,
        /// 0x15ee1c.
        28 helpdesk_voice: u8,
        /// 0x15ee1d.
        29 helpdesk_text: u8,
        /// 0x13e520.
        30 gold_weapons: [u8; 40],
        /// 0x15eea0.
        31 game_beaten: i32,
        /// 0x141660: hand, feet, head, back, 3 unused.
        32 equipped: [i32; 7],
        /// 0x15ee40.
        33 subtitles: u8,
        /// 0x15ede8: 1 stereo / 0 mono.
        34 stereo: i32,
        /// 0x15edec: 0..0x400.
        35 music_volume: i32,
        /// 0x15edf0: 0..0x400.
        36 effects_volume: i32,
        /// 0x15edc0.
        37 cheats_ever: [u8; 12],
        /// 0x13dd70.
        1000 ammo_bought: [i32; 37],
        /// 0x13de08.
        1001 ammo_picked_up: [i32; 37],
        /// 0x13dea0.
        1002 ammo_used: [i32; 37],
        /// 0x15eea4: gameplay ticks.
        1003 play_time: i32,
        /// 0x15eea8.
        1004 total_hits: i32,
        /// 0x15eeac.
        1005 total_deaths: i32,
        /// 0x15ee28.
        1008 bolt_window_ticks: i32,
        /// 0x15ee2c.
        1009 bolt_window_bolts: i32,
        /// 0x141e08.
        1010 help_log: [u8; 150],
        /// 0x15ee30.
        1011 help_log_pos: i32,
    }
}

chunk_struct! {
    /// The 11 chunks of one level slot (table boot 0x1a07c0; slot L at `addr + L·size`), §2.
    LevelState {
        /// 0x13dd58: 0 never, 1 visited, 2 left / completed.
        3001 visited: u8,
        /// 0x141ec0: the level's map fog mask as the saves pack it (`crate::map::Mask::pack` / `unpack`).
        3002 map_mask: [u8; 0x800],
        /// 0x14bec0: gold bolts collected.
        3003 gold_bolts: [u8; 4],
        /// 0x14c050: mission bytes (0xff done).
        3004 missions: [u8; 16],
        /// 0x14c190: killed bitset by spawn id.
        3005 killed: [u8; 0x100],
        /// 0x14d590.
        3006 bolt_drops: [BoltDrop; 64],
        /// 0x141680: level-entry record (count, time/600, level mask).
        3007 entry: HelpRec,
        /// 0x14bf10: metal-detector bytes.
        3008 metal_detector: [u8; 16],
        /// 0x13df38: bolts collected on this level.
        4000 bolts: i32,
        /// 0x13df88.
        4001 hits: i32,
        /// 0x13dfd8.
        4002 deaths: i32,
    }
}

/// Which section a chunk belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope { Global, Level(usize) }

/// `RestoreData` error counts (0x13d33c[card]) per section: `[0]` global, `[1 + L]` level L.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RestoreReport { pub errors: Vec<u32> }

impl RestoreReport {
    pub fn total(&self) -> u32 { self.errors.iter().sum() }
}

/// Not saved: reset at every level start (hero block clear 0x226b70) or fixed per session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SessionState {
    /// 0x1415f8: hit points (= max HP at level start).
    pub hp: i32,
    /// 0x141408: hand item to show (GiveItem with equip, slot type 0).
    pub temp_hand: i32,
    /// 0x14140c: the feet slot's request (the Gadgets menu's close; `GiveItem` never writes it).
    pub temp_feet: i32,
    /// 0x141410: slot-type-2 (head) item to show.
    pub temp_head: i32,
    /// 0x141414: slot-type-3 (back) item to show.
    pub temp_back: i32,
    /// 0x141628: Clank hidden (set by Veldin's class 834).
    pub clank_hidden: i16,
    /// 0x15ed68: `fun_001f96f8` tick scale (1.0 NTSC).
    pub tick_scale: f32,
    /// 0x141345: launch the Drone Device's drones (the quick select / the vendor set it; the hand swap takes it:
    /// `crate::hero::items::ItemGlobals::drone`).
    pub drone: bool,
}

impl Default for SessionState {
    fn default() -> Self { SessionState { hp: 0, temp_hand: 0, temp_feet: 0, temp_head: 0, temp_back: 0, clank_hidden: 0, tick_scale: 1.0, drone: false } }
}

impl SessionState {
    /// Hero init 0x226b70: the hero block (0x13f350..0x141660) is cleared and HP = max HP.
    pub fn hero_init(&mut self, max_hp: i32) {
        *self = SessionState { hp: max_hp, tick_scale: self.tick_scale, ..SessionState::default() };
    }
    /// `fun_001f96f8`: `(int)((float)t · scale + 0.5)` in PS2 float arithmetic.
    pub fn scale_ticks(&self, t: i32) -> i32 { (Pf::from_i32(t) * Pf::f(self.tick_scale) + Pf::b(0x3f00_0000)).to_i32() }
}

/// Typed view of the options (`docs/plan/game_state.md` §5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub yaw_normal: bool,
    pub pitch_normal: bool,
    /// Index into [`YAW_RATES`] (0x162228: 1.0°, 1.3°, 1.6° per tick).
    pub rotation_speed: usize,
    pub helpdesk_voice: bool,
    pub helpdesk_text: bool,
    pub subtitles: bool,
    pub stereo: bool,
    /// 0..0x400.
    pub music_volume: i32,
    pub effects_volume: i32,
    /// Mirror cheat 0x15edb4 (pad L/R swap) and 0x15edb5 (mirrored animation swap).
    pub mirror: bool,
    pub mirror_anim: bool,
}

impl Options {
    /// Yaw rate per tick in radians (PS2 float) for the selected speed.
    pub fn yaw_rate(&self) -> Pf { YAW_RATES[self.rotation_speed.min(2)] }
    /// What the tick reads.
    pub fn game_options(&self) -> GameOptions {
        GameOptions {
            mirror: self.mirror,
            camera: CameraOptions { pitch_normal: self.pitch_normal, yaw_normal: self.yaw_normal, yaw_rate: self.rotation_speed, mirror: self.mirror },
            ..GameOptions::default()
        }
    }
}

/// The whole persistent state.
#[derive(Clone, Debug, PartialEq)]
pub struct GameState {
    /// The descriptor tables (chunk order and sizes) this state encodes with.
    pub tables: ChunkTables,
    pub global: Global,
    /// [`LEVEL_SLOTS`] entries.
    pub levels: Vec<LevelState>,
    /// Chunks without a matching typed field (unknown id or different size), by scope and id.
    pub raw: BTreeMap<(Scope, i32), Vec<u8>>,
    /// Non-zero pad bytes of restored chunks, re-emitted by [`GameState::to_save_file`].
    pub pads: BTreeMap<(Scope, i32), Vec<u8>>,
}

fn align4(n: usize) -> usize { (n + 3) & !3 }

impl GameState {
    /// All chunks zero (the RAM before a restore).
    pub fn zeroed(tables: ChunkTables) -> GameState {
        GameState { tables, global: Global::zeroed(), levels: (0..LEVEL_SLOTS).map(|_| LevelState::zeroed()).collect(), raw: BTreeMap::new(), pads: BTreeMap::new() }
    }

    /// New game (`load_and_initialize_level_chunk` 0x209370): restore the disc template
    /// (`SaveGameLump::template`), then `level = 0`.
    pub fn new_game(tables: ChunkTables, template: &[u8]) -> Result<GameState> {
        let mut s = GameState::zeroed(tables);
        let r = s.load_card_file(template)?;
        if r.total() != 0 { return Err(FormatError::Invalid(format!("save template restored with errors {:?}", r.errors))); }
        s.global.level = 0;
        Ok(s)
    }

    fn descs(&self, scope: Scope) -> &[ChunkDesc] { match scope { Scope::Global => &self.tables.global, Scope::Level(_) => &self.tables.level } }

    fn typed_size(scope: Scope, id: i32) -> Option<usize> {
        match scope { Scope::Global => Global::field_size(id), Scope::Level(_) => LevelState::field_size(id) }
    }

    /// The current bytes of chunk `d` in `scope` (`d.size` bytes).
    pub fn chunk_bytes(&self, scope: Scope, d: &ChunkDesc) -> Vec<u8> {
        let size = d.size as usize;
        if Self::typed_size(scope, d.id) == Some(size) {
            let mut v = Vec::with_capacity(size);
            match scope { Scope::Global => self.global.put(d.id, &mut v), Scope::Level(l) => self.levels[l].put(d.id, &mut v) };
            return v;
        }
        let mut v = self.raw.get(&(scope, d.id)).cloned().unwrap_or_default();
        v.resize(size, 0);
        v
    }

    /// Replaces chunk `d` in `scope` with `bytes` (`d.size` bytes).
    pub fn set_chunk_bytes(&mut self, scope: Scope, d: &ChunkDesc, bytes: &[u8]) {
        let size = d.size as usize;
        debug_assert_eq!(bytes.len(), size);
        if Self::typed_size(scope, d.id) == Some(size) {
            match scope { Scope::Global => self.global.set(d.id, bytes), Scope::Level(l) => self.levels[l].set(d.id, bytes) };
        } else {
            self.raw.insert((scope, d.id), bytes.to_vec());
        }
    }

    /// `RestoreData` 0x20af20 for one section: nothing on a bad CRC (1 error); else chunks matched by id,
    /// copying `min(saved, table)` bytes; unknown ids and a size-sum mismatch are errors, as is every table
    /// chunk not restored at its exact size. (The game stops that last scan early at a descriptor whose id
    /// equals the word after the terminator in its buffer; that stale word is not modelled.)
    pub fn restore_section(&mut self, scope: Scope, sec: &Section) -> u32 {
        if !sec.crc_ok { return 1; }
        let descs = self.descs(scope).to_vec();
        let mut status = vec![0i32; descs.len()];
        let (mut errors, mut sum) = (0u32, 8usize);
        for c in &sec.chunks {
            let Some(i) = descs.iter().position(|d| d.id == c.id) else { errors += 1; continue };
            let d = descs[i];
            let n = c.data.len().min(d.size as usize);
            status[i] = match c.data.len().cmp(&(d.size as usize)) { std::cmp::Ordering::Equal => 1, std::cmp::Ordering::Less => -1, _ => -2 };
            let mut cur = self.chunk_bytes(scope, &d);
            cur[..n].copy_from_slice(&c.data[..n]);
            self.set_chunk_bytes(scope, &d, &cur);
            if status[i] == 1 && c.pad.iter().any(|&b| b != 0) { self.pads.insert((scope, d.id), c.pad.clone()); } else { self.pads.remove(&(scope, d.id)); }
            sum += 8 + align4(n);
        }
        if sum + 8 != section_size(&descs) { errors += 1; }
        errors += status.iter().filter(|&&s| s < 1).count() as u32;
        errors
    }

    /// `RestoreGame` for a parsed file: global, then the 20 level sections.
    pub fn restore(&mut self, file: &SaveFile) -> RestoreReport {
        let mut errors = vec![self.restore_section(Scope::Global, &file.global)];
        for (l, s) in file.levels.iter().enumerate() { errors.push(self.restore_section(Scope::Level(l), s)); }
        RestoreReport { errors }
    }

    /// `RestoreGame` 0x209298 on raw file bytes: restores only if the header sizes equal the tables'
    /// `GetDataSize` (the game's version check); otherwise an error and nothing changes.
    pub fn load_card_file(&mut self, bytes: &[u8]) -> Result<RestoreReport> {
        let h = |o: usize| bytes.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize);
        if h(0) != Some(self.tables.global_size()) || h(4) != Some(self.tables.level_size()) {
            return Err(FormatError::Invalid("save file: section sizes differ from the chunk tables".into()));
        }
        Ok(self.restore(&SaveFile::parse(bytes)?))
    }

    /// A state restored from `file` onto zeroed RAM.
    pub fn from_save_file(tables: ChunkTables, file: &SaveFile) -> (GameState, RestoreReport) {
        let mut s = GameState::zeroed(tables);
        let r = s.restore(file);
        (s, r)
    }

    fn section(&self, scope: Scope) -> Section {
        Section::new(self.descs(scope).iter().map(|d| {
            let mut c = Chunk::new(d.id, self.chunk_bytes(scope, d));
            if let Some(p) = self.pads.get(&(scope, d.id)) { if p.len() == c.pad.len() { c.pad = p.clone(); } }
            c
        }).collect())
    }

    /// `PrepData` of the global section.
    pub fn global_section(&self) -> Section { self.section(Scope::Global) }
    /// `PrepData` of level slot `l`.
    pub fn level_section(&self, l: usize) -> Section { self.section(Scope::Level(l)) }

    /// `MakeWholeSave` 0x20abb0.
    pub fn to_save_file(&self) -> SaveFile {
        SaveFile { global: self.global_section(), levels: std::array::from_fn(|l| self.level_section(l)) }
    }

    /// `memcard_Save(force, pretend)` 0x20b178 + the state-0x10 write into an existing card file: stamps
    /// the clock (chunk 4, kept), then — with `pretend` = Some(p) — encodes the global section with
    /// `level = p` and `visited[p]` raised to 1 if 0 (both only for the save), and writes it plus the
    /// **current** level's section; the other sections of `card_file` are left as they were. Landmark
    /// capture (0x208770) is not ported (chunk 15 saves as held); the 3002 packer is `crate::map::MapState::pack`, which
    /// the caller runs first (the game's `memcard_Save` packs the current level's mask before it writes).
    pub fn save_incremental(&mut self, card_file: &mut [u8], pretend: Option<i32>, clock: [u8; 8]) -> Result<()> {
        self.global.save_clock = clock;
        let cur = self.global.level;
        let Some(slot) = usize::try_from(cur).ok().filter(|&l| l < LEVEL_SLOTS) else {
            return Err(FormatError::Invalid(format!("incremental save on level {cur}")));
        };
        let mut s = self.clone();
        if let Some(p) = pretend.filter(|&p| p >= 0) {
            s.global.level = p;
            if let Some(lv) = s.levels.get_mut(p as usize) { if lv.visited == 0 { lv.visited = 1; } }
        }
        let mut file = SaveFile { global: s.global_section(), levels: std::array::from_fn(|_| Section::new(Vec::new())) };
        file.levels[slot] = s.level_section(slot);
        file.write_incremental(slot, card_file)
    }

    // -----------------------------------------------------------------------------------------------
    // Rules.

    /// `GiveItem` L01 0x275760 (help sound not modelled).
    pub fn give_item(&mut self, id: usize, equip: bool, items: &ItemTables, session: &mut SessionState) {
        let g = &mut self.global;
        g.acquired[id] = 1;
        if g.owned[id] != 0 { return; }
        g.owned[id] = 1;
        let rec = &items.records[id];
        if rec.has_ammo() { g.ammo[id] = g.ammo[id].max(rec.grant_ammo() as i32); }
        if id != 0 && items.vendor_items.contains(&(id as u32)) {
            let b = id as u8 | VENDOR_OWNED;
            // First slot holding this item or empty (the game has no bound; 12 slots never fill up).
            if let Some(s) = g.vendor.iter().position(|&v| (v & 0x3f) as usize == id || v == VENDOR_EMPTY) { g.vendor[s] = b; }
        }
        let ty = items.slot_type[id];
        if equip {
            match ty { 0 => session.temp_hand = id as i32, 3 => session.temp_back = id as i32, 2 => session.temp_head = id as i32, _ => {} }
        }
        if ty == 0 {
            if let Some(q) = g.quick_select.iter().position(|&q| q == 0) { g.quick_select[q] = id as i32; }
        }
    }

    /// Planet unlock L01 0x2756d0 (the new-planet message 0x277c38 is not modelled).
    pub fn unlock_planet(&mut self, p: usize) {
        let g = &mut self.global;
        if g.planet_unlocked[p] != 0 { return; }
        let n = g.planet_unlocked.iter().filter(|&&u| u != 0).count();
        g.planet_unlocked[p] = 1;
        if let Some(slot) = g.map_order.get_mut(n) { *slot = p as i32; }
    }

    /// `FUN_00251fe0`: put this level's vendor item into the first matching or empty stock slot.
    fn add_level_vendor_item(&mut self, level: usize, items: &ItemTables) {
        let it = items.vendor_items[level];
        if it == 0 { return; }
        let v = &mut self.global.vendor;
        // First slot whose item matches or that is empty (bounded to the 12 slots).
        let Some(i) = v.iter().position(|&b| (b & 0x3f) as u32 == it || b == VENDOR_EMPTY) else { return };
        if v[i] == VENDOR_EMPTY { v[i] = it as u8; }
    }

    /// `FUN_00251da0`: bomb-glove grant, vendor prune, this level's vendor item (debug branch for level ≥ 19).
    fn level_items(&mut self, level: i32, items: &ItemTables, session: &mut SessionState) {
        if level < LEVEL_COUNT {
            if self.global.owned[item::BOMB_GLOVE] == 0 {
                self.give_item(item::BOMB_GLOVE, true, items, session);
                self.global.vendor[0] = item::BOMB_GLOVE as u8 | VENDOR_OWNED;
                self.global.equipped[0] = item::BOMB_GLOVE as i32;
            }
            let vt = &items.vendor_items;
            for b in self.global.vendor.iter_mut() {
                if *b == VENDOR_EMPTY { continue; }
                let it = (*b & 0x3f) as u32;
                if it as usize == item::BOMB_GLOVE { continue; }
                let sold = vt[0] == it || vt[1..LEVEL_COUNT as usize].contains(&it);
                if !sold || it == 0 { *b = VENDOR_EMPTY; }
            }
            self.add_level_vendor_item(level as usize, items);
        } else {
            let g = &mut self.global;
            g.owned = [1; ITEM_COUNT];
            g.acquired = [1; ITEM_COUNT];
            g.quick_select = [15, 12, 13, 11, 17, 10, 16, 19];
            g.vendor = [0x4a, 0x50, 0x4f, 0x54, 0x51, 0x4e, 0x4b, 0x52, 0x4d, 0x59, 0x58, 0x53];
        }
    }

    /// Everything a level start does to the saved state and the session (§4 steps 1-5), in the game's
    /// order: `FUN_00251da0` (items / vendor), planet unlock if `level ≠ 0`, elapsed += 180, hero init
    /// (session cleared, HP = max HP, empty hand → bomb glove, no head item), `visited = 1` if 0, record 3007.
    pub fn apply_level_start(&mut self, level: i32, items: &ItemTables, session: &mut SessionState) {
        self.level_items(level, items, session);
        if level != 0 { if let Ok(p) = usize::try_from(level) { if p < LEVEL_SLOTS { self.unlock_planet(p); } } }
        self.global.elapsed = self.global.elapsed.wrapping_add(LEVEL_LOAD_ELAPSED);
        session.hero_init(self.global.max_hp);
        if self.global.equipped[0] == 0 { self.global.equipped[0] = item::BOMB_GLOVE as i32; }
        // `FUN_00226e90` (every level load): the saved head item 0x141668 = 0.
        self.global.equipped[2] = 0;
        let Some(l) = usize::try_from(level).ok().filter(|&l| l < LEVEL_SLOTS) else { return };
        let lv = &mut self.levels[l];
        if lv.visited == 0 { lv.visited = 1; }
        if lv.entry.count != 0xffff { lv.entry.count = lv.entry.count.wrapping_add(1); }
        let t = session.scale_ticks(self.global.play_time) / 600;
        if (lv.entry.time as i32) < t { lv.entry.time = t as u16; }
        lv.entry.mask |= (1u32 << (l as u32 & 31)) | 0x8000_0000;
    }

    /// `DoSpaceTransition` [0x231ff0] from the current level to `dest` (cutscenes not modelled):
    /// * `dest < 0` or first Veldin arrival: only `level = dest`;
    /// * Veldin → first Novalis: `visited[0] = 2`, `level = 1`;
    /// * otherwise the level being left is marked completed (`visited = 2`) unless it is Umbris (7) with
    ///   Batalia (8) locked or Oltanis (14) with Quartu (15) locked; `level = dest`.
    pub fn apply_transition(&mut self, dest: i32) {
        let from = self.global.level;
        let visited = |s: &Self, l: usize| s.levels[l].visited;
        if dest < 0 || (dest == 0 && visited(self, 0) == 0) {
            self.global.level = dest;
            return;
        }
        if from == 0 && dest == 1 && visited(self, 1) == 0 {
            self.levels[0].visited = 2;
            self.global.level = 1;
            return;
        }
        if (0..LEVEL_COUNT).contains(&from) {
            let pl = &self.global.planet_unlocked;
            let done = !((from == 7 && pl[8] == 0) || (from == 14 && pl[15] == 0));
            if done { self.levels[from as usize].visited = 2; }
        }
        self.global.level = dest;
    }

    /// Veldin's Clank (class 834, L00 0x2d9dc8) on its first update: hides Clank for the session
    /// (0x141628 = 1) and sets global flag 8.
    pub fn on_veldin_clank_init(&mut self, session: &mut SessionState) {
        session.clank_hidden = 1;
        self.global.flags[FLAG_VELDIN_CLANK] = 1;
    }

    /// `fun_00208770` (L01 0x25eb68): before a save, every landmark hook of `level` (0x179638, registered by the loader's
    /// `MobyUnknown74Hook` 0x25e7b0 at index `base[level] + k`) copies its moby's x (+0x10), y (+0x14) and angle (+0x48)
    /// into landmark `base[level] + k` (chunk 15); the flags word stays. Levels ≥ 0x13 have no range. `hooks`: (k, (x, y,
    /// angle)) of the registered mobys; `base`: the level ranges 0x1c4938 (`TalkTables::base`).
    pub fn capture_landmarks(&mut self, level: i32, base: &[i32], hooks: &[(i32, (f32, f32, f32))]) {
        let Some(l) = usize::try_from(level).ok().filter(|&l| l < 0x13) else { return };
        let (Some(&lo), Some(&hi)) = (base.get(l), base.get(l + 1)) else { return };
        for &(k, (x, y, rot)) in hooks {
            let i = lo + k;
            if !(lo..hi).contains(&i) { continue; }
            if let Some(m) = usize::try_from(i).ok().and_then(|i| self.global.landmarks.get_mut(i)) {
                m.x = x;
                m.y = y;
                m.rot = rot;
            }
        }
    }

    /// The state part of challenge mode `fun_00226b08` (L01 0x29a710): keeps the gold weapons (0x13e520), the owned flags
    /// of the items in `keep_owned` (the list L01 0x1ba120), the ammo (0x13d428), the gold bolts of every level (0x14bec0,
    /// 0x50 bytes = chunk 3003 × 20), the quick select (0x141ea0; an item no longer owned → 0), the vendor stock (0x15edd0),
    /// the skill points (0x13d408) and the bolts across `reset` (`load_and_initialize_level_chunk`: the template restore,
    /// level 0); then the nanotech flags [4] / [5] (max HP 5 / 8), HelpDesk voice and text off, times completed + 1.
    pub fn challenge_reset(&mut self, keep_owned: &[usize], reset: impl FnOnce(&mut GameState)) {
        let g = self.global.clone();
        let gold_bolts: Vec<[u8; 4]> = self.levels.iter().map(|l| l.gold_bolts).collect();
        let (premium, ultra) = (g.flags[4] != 0, g.flags[5] != 0);
        reset(self);
        let n = &mut self.global;
        n.gold_weapons = g.gold_weapons;
        for &i in keep_owned {
            if let (Some(d), Some(&s)) = (n.owned.get_mut(i), g.owned.get(i)) { *d = s; }
        }
        n.ammo = g.ammo;
        for (l, b) in self.levels.iter_mut().zip(gold_bolts) { l.gold_bolts = b; }
        let n = &mut self.global;
        for (d, &q) in n.quick_select.iter_mut().zip(&g.quick_select) {
            *d = if usize::try_from(q).ok().and_then(|q| n.owned.get(q)).is_some_and(|&o| o != 0) { q } else { 0 };
        }
        n.vendor = g.vendor;
        n.skill_points = g.skill_points;
        n.bolts = g.bolts;
        if premium {
            n.flags[4] = 1;
            n.max_hp = 5;
        }
        if ultra {
            n.flags[5] = 1;
            n.max_hp = 8;
        }
        n.helpdesk_voice = 0;
        n.helpdesk_text = 0;
        n.completes = g.completes.wrapping_add(1);
    }

    /// The options, typed.
    pub fn options(&self) -> Options {
        let g = &self.global;
        Options {
            // The game reads these 4-byte globals as `*(u8*)` (the Options lists write only the low byte).
            yaw_normal: g.cam_yaw_normal & 0xff != 0,
            pitch_normal: g.cam_pitch_normal & 0xff != 0,
            rotation_speed: (g.cam_speed & 0xff).clamp(0, 2) as usize,
            helpdesk_voice: g.helpdesk_voice != 0,
            helpdesk_text: g.helpdesk_text != 0,
            subtitles: g.subtitles != 0,
            stereo: g.stereo & 0xff != 0,
            music_volume: g.music_volume,
            effects_volume: g.effects_volume,
            mirror: g.cheats_active[4] != 0,
            mirror_anim: g.cheats_active[5] != 0,
        }
    }

    /// Writes the option chunks back (inverse of [`GameState::options`]).
    pub fn set_options(&mut self, o: &Options) {
        let g = &mut self.global;
        g.cam_yaw_normal = o.yaw_normal as i32;
        g.cam_pitch_normal = o.pitch_normal as i32;
        g.cam_speed = o.rotation_speed.min(2) as i32;
        g.helpdesk_voice = o.helpdesk_voice as u8;
        g.helpdesk_text = o.helpdesk_text as u8;
        g.subtitles = o.subtitles as u8;
        g.stereo = o.stereo as i32;
        g.music_volume = o.music_volume;
        g.effects_volume = o.effects_volume;
        g.cheats_active[4] = o.mirror as u8;
        g.cheats_active[5] = o.mirror_anim as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables() -> ChunkTables {
        let g = [(0x15ed84, 4, 0), (0x13d4c0, 37, 10), (0x13d388, 128, 5), (0x99, 3, 99)];
        let l = [(0x13dd58, 1, 3001), (0x141680, 8, 3007)];
        let m = |v: &[(u32, u32, i32)]| v.iter().map(|&(addr, size, id)| ChunkDesc { addr, size, id }).collect();
        ChunkTables { global: m(&g), level: m(&l) }
    }

    #[test]
    fn typed_raw_and_pads_round_trip() {
        let mut s = GameState::zeroed(tables());
        s.global.level = 3;
        s.global.owned[10] = 1;
        s.levels[2].visited = 2;
        s.levels[5].entry = HelpRec { count: 1, time: 2, mask: 0x8000_0020 };
        let d99 = s.tables.global[3];
        s.set_chunk_bytes(Scope::Global, &d99, &[1, 2, 3]);
        let mut f = s.to_save_file();
        f.levels[2].chunks[0].pad = vec![0xaa, 0xbb, 0xcc];
        let bytes = SaveFile::parse(&f.to_bytes()).unwrap().to_bytes();
        let (back, r) = GameState::from_save_file(tables(), &SaveFile::parse(&bytes).unwrap());
        assert_eq!(r.total(), 0);
        assert_eq!(back.raw.get(&(Scope::Global, 99)).map(Vec::as_slice), Some(&[1u8, 2, 3][..]));
        assert_eq!(back.global.owned[10], 1);
        assert_eq!(back.levels[5].entry.mask, 0x8000_0020);
        assert_eq!(back.to_save_file().to_bytes(), bytes, "pads and raw chunks survive");
    }

    #[test]
    fn restore_rules() {
        let mut s = GameState::zeroed(tables());
        // Saved chunk shorter than the table: copies the prefix, counts an error.
        let sec = Section::new(vec![Chunk::new(0, vec![7, 0, 0, 0]), Chunk::new(10, vec![1; 5]), Chunk::new(5, vec![0; 128]), Chunk::new(99, vec![0; 3]), Chunk::new(1234, vec![0; 4])]);
        let sec = Section::decode(&sec.encode()).unwrap();
        let e = s.restore_section(Scope::Global, &sec);
        assert_eq!(s.global.level, 7);
        assert_eq!(&s.global.owned[..6], &[1, 1, 1, 1, 1, 0]);
        // Unknown id 1234 (1) + size sum mismatch (1) + id 10 restored short (1).
        assert_eq!(e, 3);
        let mut bad = sec.clone();
        bad.crc_ok = false;
        let before = s.clone();
        assert_eq!(s.restore_section(Scope::Global, &bad), 1);
        assert_eq!(s, before, "a bad CRC restores nothing");
    }

    #[test]
    fn transition_rules() {
        let mut s = GameState::zeroed(tables());
        s.apply_transition(0);
        assert_eq!(s.global.level, 0);
        s.levels[0].visited = 1;
        s.apply_transition(1);
        assert_eq!((s.global.level, s.levels[0].visited), (1, 2));
        s.global.level = 7;
        s.levels[7].visited = 1;
        s.apply_transition(3);
        assert_eq!((s.global.level, s.levels[7].visited), (3, 1), "Umbris stays open while Batalia is locked");
        s.global.planet_unlocked[8] = 1;
        s.global.level = 7;
        s.apply_transition(3);
        assert_eq!(s.levels[7].visited, 2);
    }

    #[test]
    fn scale_ticks_ntsc_is_identity() {
        let s = SessionState::default();
        for t in [0, 1, 599, 600, 123_456, 16_777_215] { assert_eq!(s.scale_ticks(t), t); }
    }
}
