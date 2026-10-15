//! The cheats (docs/plan/progression.md `## media`): the cheat bytes 0x15edb0[12] (chunk 7, "active") and 0x15edc0[12]
//! (chunk 37, "ever activated": the Cheats page lists only those), the in-game cheat entry (a sequence of Ratchet's
//! moves, open once the game is beaten or completed), the Goodies / Cheats page's list, and the pause menu's debug code
//! entry (`MenuInput` 0x298f80). Addresses are level01; the tables are read from the level overlay (one copy per
//! overlay at the same labels, [`crate::menus::Overlay::at`]).
//!
//! **The cheats** (the Cheats page's table 0x20a228: slot → label; the move patterns 0x179b80[slot]):
//!
//! | slot | byte | label | effect (readers) |
//! |---|---|---|---|
//! | 0 | 0x15edb0 | 20508 "Actors have oversized craniums" | the NPCs' head records ×2.75 (talking NPC 774 P+0xc0, Quartu's mission NPC +0x1d0, the commando 114, the mouse 0x1b1), the scene actors' head (`0x2fac80` / `0x2fb4b8` / `0x2ff028` / `0x2f2b18` / `0x2ec678`: a manipulator on list 0 of the matching scene actors, record 0x17c8c0 cleared by every `DialogStreamStart`), the vendor's head 2.37 (`0x2aee20`), the scene camera's FOV × 0x15f59c and the offset 0x15f5a0 (both reset to 1.0 / 0 by `DialogStreamStart`: no effect) |
//! | 1 | 0x15edb1 | 20506 "Ratchet has a big head" | Ratchet's head record 17 scale 0x15ee14 approaches 1.57 instead of 0.92 (`0x22b928`); body 3's 0x17c04c = 1.9 (`HeroUpdateAlt`) |
//! | 2 | 0x15edb2 | 20513 "Trippy contrails" | the vehicles' contrails (level05 `0x2d7920`, level11 `0x3192c8`, level17 `0x2f3b68`) |
//! | 3 | 0x15edb3 | 20507 "Clank has a large noggin" | Clank's head 0x15ee18 → 1.8 (`0x2278c0`), record 18 = 0x15ee18 (`0x22b928`, `0x235e60`), his back-pack manipulator (`0x228bc0`), his sway (`0x229440`), the follow camera's eye height in body 1 1.2 (`0x3160b8`) |
//! | 4 | 0x15edb4 | 20510 "Levels are mirrored" | the pad's left / right swapped (`ProcessPadInput`), the camera's left row mirrored (gameplay `0x20eca8`, scenes `0x2ac8d8`, ship `0x2a4080`), the map's arrow, the move classification of this module |
//! | 5 | 0x15edb5 | — (no pattern, no list entry: only a save can set it) | the mirrored animations (`SetAnim`, `SetState`, `HeroSyncMoby`, `HeroItemsAttach`, the gold bolt, the bolt crank) |
//! | 6 | 0x15edb6 | 20512 "Health gives invincibility at max" | the nanotech pickups at full health (`0x300de0`: the invincibility timer 0x13f510) |
//! | 7 | 0x15edb7 | 20509 "Enemies have massive domes" | the enemies' head manipulators (`0x278720` and its level copies) |
//! | 8..11 | — | — | no pattern, no list entry |
//!
//! **Coverage.**
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2285a0 +0x00 | only when the game is beaten (0x15eea0) or completed (0x15ee20) | [`MoveEntry::step`] |
//! | 0x2285a0 classify | state ticks 0x13f4e8 = `ticks(15)`: group 4: state 0xb (flip) by sector 0x13f7a0: 0 → 1 (2 mirrored), 1 → 2 (1 mirrored), 3 → 3; state 0x11 → 9; 10 → 10; 0xe → 5; group 6: 0x14 → 6; 0x13 → 0x13fdb0 + 12; 0x15 → 4; other groups: 0x22 → 11. State ticks = `ticks(60)`: state 4 → 7, 8 → 8 | [`classify`] |
//! | 0x2285a0 ring | code into the 16-byte ring 0x141514 at 0x141524, index + 1 | [`MoveEntry::step`] |
//! | 0x2285a0 match | each of the 12 patterns 0x179b80 + 16·i (NUL-terminated, empty skipped) against the last n codes (start `idx − n`, +16 when negative; wrap at 16) | [`MoveEntry::step`] |
//! | 0x2285a0 hit | 0x15edc0[i] = 1, 0x15edb0[i] toggled, `ShowBanner(0x4fbe "Cheat Enabled" / 0x4fbf "Cheat Disabled", −1)`, first match only | [`MoveEntry::step`] (the engine applies the bytes and the banner) |
//! | 0x2285a0 tail | the index back to 0 past 15 | [`MoveEntry::step`] |
//! | 0x28dbe8 | the Cheats list: for each table entry (slot ≠ −1, 0x15edc0[slot] ≠ 0): `{label, &0x15edb0[slot], 0x4f5a "on", 0x4f5b "off", 0}`, the list ended by a 0 label | [`CheatList::entries`] |
//! | 0x28dbe8 iterations 8..11 | the loop runs 12 times over a 15-word stack copy: iterations 8..11 read the labels as slots (20506..20509 + 0x15edc0 = level bss 0x162ada.., zero in play) | n/a (never true: the bytes are in the cleared level bss) [L] |
//! | 0x298f80 | while held & 0xf = 6 (R2 + L1): each press of Up / Down / Left / Right / □ / ○ (pressed & 0xf0a0) records 0 / 1 / 2 / 3 / 4 / 5 into 0x1ba928[count < 20]; released: count = 0 | [`CodeEntry::step`] |
//! | 0x298f80 match | the 20th press: code k (2..0x92) matches when key i = table 0x1ba020[(k·(i + 1)) & 0xff] for all i; id = k − 2 | [`CodeEntry::step`] |
//! | 0x298f80 effects | id < 0x25: item id owned (0x13d4c0) and acquired (0x13d4e8); 0x25..0x36: `UnlockPlanet(id − 0x24)`; 0x37..0x3c: global flag id − 0x37 = 1; 0x3d..0x5a: skill point id − 0x3d (when not yet: = 1, `PlayLevelSoundAtMoby(1, 0, 0)`, `ShowBanner(0x53d6, −1)`) | [`CodeEffect`], [`apply_code`] |

use crate::game_state::GameState;
use crate::menus::Overlay;

/// The cheat slots (0x15edb0 + slot).
pub mod slot {
    pub const ACTORS: usize = 0;
    pub const RATCHET: usize = 1;
    pub const CONTRAILS: usize = 2;
    pub const CLANK: usize = 3;
    pub const MIRROR: usize = 4;
    pub const MIRROR_ANIM: usize = 5;
    pub const HEALTH: usize = 6;
    pub const ENEMIES: usize = 7;
}

/// Cheat bytes (0x15edb0 and 0x15edc0 have 12).
pub const SLOTS: usize = 12;

/// The level-01 labels of the tables.
pub mod label {
    /// The Cheats page's slot table (8 × s32) and its labels (7 × s32) right after.
    pub const LIST_SLOTS: u32 = 0x20a228;
    pub const LIST_LABELS: u32 = 0x20a248;
    /// The move patterns, 12 × 16 bytes.
    pub const PATTERNS: u32 = 0x179b80;
    /// The debug code table, 256 bytes.
    pub const CODES: u32 = 0x1ba020;
}

/// Messages.
pub mod msg {
    /// `ShowBanner` on a toggle.
    pub const ENABLED: i32 = 0x4fbe;
    pub const DISABLED: i32 = 0x4fbf;
    /// The Cheats list's value strings.
    pub const ON: u32 = 0x4f5a;
    pub const OFF: u32 = 0x4f5b;
    /// "You just earned a Skill Point!"
    pub const SKILL_POINT: i32 = 0x53d6;
}

/// `ShowBanner(msg, −1)`'s countdown (`ticks(180)`).
pub const BANNER_TICKS: i32 = 180;

/// The cheat bytes as the code reads them (a copy of chunk 7, 0x15edb0[12]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cheats(pub [u8; SLOTS]);

impl Cheats {
    /// 0x15edb0[slot] ≠ 0.
    pub fn on(&self, slot: usize) -> bool { self.0.get(slot).is_some_and(|&b| b != 0) }
}

/// The game's tables of this module, read from the level overlay.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CheatTables {
    /// The Cheats page's entries: (slot, label), in table order (slot −1 entries dropped, as the enter skips them).
    pub list: Vec<(usize, u32)>,
    /// The 12 move patterns (each up to its first 0).
    pub patterns: Vec<Vec<u8>>,
    /// The debug code table 0x1ba020.
    pub codes: Vec<u8>,
}

impl CheatTables {
    pub fn read(ov: &Overlay) -> CheatTables {
        let at = |l: u32| ov.at(l);
        let mut list = Vec::new();
        for k in 0..8u32 {
            let s = ov.i32(at(label::LIST_SLOTS) + 4 * k).unwrap_or(-1);
            let lab = ov.u32(at(label::LIST_LABELS) + 4 * k).unwrap_or(0);
            // Word 15 of the stack copy is not initialised (the copy is 15 words): entry 7's label is never read
            // because its slot is −1.
            if let Ok(s) = usize::try_from(s) {
                if s < SLOTS && k < 7 { list.push((s, lab)); }
            }
        }
        let patterns = (0..SLOTS as u32)
            .map(|i| {
                let b = ov.bytes(at(label::PATTERNS) + 16 * i, 16).map(|b| b.to_vec()).unwrap_or_default();
                b.iter().take_while(|&&c| c != 0).copied().collect()
            })
            .collect();
        let codes = ov.bytes(at(label::CODES), 256).map(|b| b.to_vec()).unwrap_or_default();
        CheatTables { list, patterns, codes }
    }
}

/// One Cheats-page entry (`{label, u8* flag, on, off, flags}`), the flag being cheat byte `slot`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListEntry {
    pub label: u32,
    pub slot: usize,
}

/// The cheat byte address of slot `s` (the Cheats page's entries point at it).
pub fn var_addr(s: usize) -> u32 { 0x15edb0 + s as u32 }

/// The cheat slot of a toggle variable address (0x15edb0..0x15edbb).
pub fn slot_of(addr: u32) -> Option<usize> { addr.checked_sub(0x15edb0).map(|d| d as usize).filter(|&d| d < SLOTS) }

/// `0x28dbe8`: the Cheats page's entries, the cheats ever activated in table order.
pub fn entries(t: &CheatTables, gs: &GameState) -> Vec<ListEntry> {
    t.list.iter().filter(|&&(s, _)| gs.global.cheats_ever[s] != 0).map(|&(slot, label)| ListEntry { label, slot }).collect()
}

/// What the move classification reads of the hero block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MoveInput {
    /// 0x13f4e8: ticks in the state.
    pub state_ticks: i32,
    /// 0x1413dc: the movement group.
    pub group: i32,
    /// 0x1413d4: the state.
    pub state: i32,
    /// 0x13f7a0: the flip's stick sector (0 left, 1 right, 3 back).
    pub flip_sector: i32,
    /// 0x13fdb0: the melee table row.
    pub combo: i32,
    /// 0x15edb4.
    pub mirror: bool,
}

/// The move code of this tick (0: none), `0x2285a0`'s classification (module doc).
pub fn classify(i: &MoveInput) -> u8 {
    let t15 = crate::hud::scale_ticks(0xf);
    if i.state_ticks == t15 {
        match i.group {
            4 => match i.state {
                0xb => match i.flip_sector {
                    0 => if i.mirror { 2 } else { 1 },
                    1 => if i.mirror { 1 } else { 2 },
                    3 => 3,
                    _ => 0,
                },
                0x11 => 9,
                10 => 10,
                0xe => 5,
                _ => 0,
            },
            6 => match i.state {
                0x14 => 6,
                0x13 => (i.combo + 0xc) as u8,
                0x15 => 4,
                _ => 0,
            },
            _ => if i.state == 0x22 { 0xb } else { 0 },
        }
    } else if i.state_ticks == crate::hud::scale_ticks(0x3c) {
        match i.state {
            4 => 7,
            8 => 8,
            _ => 0,
        }
    } else {
        0
    }
}

/// The move ring (hero block 0x141514[16], index 0x141524).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MoveEntry {
    pub ring: [u8; 16],
    pub idx: i32,
}

/// A cheat toggled by the move entry: the slot and its new value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Toggled {
    pub slot: usize,
    pub on: bool,
}

impl Toggled {
    /// The banner `0x2285a0` shows.
    pub fn banner(&self) -> i32 { if self.on { msg::ENABLED } else { msg::DISABLED } }
}

impl MoveEntry {
    /// `0x2285a0` (after the footsteps in the hero update `0x228870`, on foot only): `open` = the game beaten or
    /// completed. Toggles the byte in `active` (and `ever`) at once, as the game; returns the toggle for the banner and the
    /// saved game.
    pub fn step(&mut self, open: bool, inp: &MoveInput, patterns: &[Vec<u8>], active: &mut [u8; SLOTS]) -> Option<Toggled> {
        if !open { return None; }
        let code = classify(inp);
        if code == 0 { return None; }
        self.ring[(self.idx & 0xf) as usize] = code;
        self.idx += 1;
        let mut hit = None;
        for (i, p) in patterns.iter().enumerate().take(SLOTS) {
            let n = p.len() as i32;
            if n == 0 { continue; }
            let mut at = self.idx - n;
            if at < 0 { at += 0x10; }
            let mut j = 0;
            let matched = loop {
                if self.ring[(at & 0xf) as usize] != p[j as usize] { break false; }
                at = if at + 1 > 0xf { at - 0xf } else { at + 1 };
                j += 1;
                if j >= n { break j == n; }
            };
            if matched {
                let on = active[i] == 0;
                active[i] = on as u8;
                hit = Some(Toggled { slot: i, on });
                break;
            }
        }
        if self.idx > 0xf { self.idx = 0; }
        hit
    }
}

/// The debug code entry's state (0x1ba928 u16[20], the count at gp−0x6808).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodeEntry {
    pub keys: [u16; 20],
    pub count: i32,
}

/// What a recognised code does (`MenuInput` 0x298f80).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeEffect {
    /// Items owned 0x13d4c0[id] and acquired 0x13d4e8[id] = 1.
    Item(usize),
    /// `UnlockPlanet(p)`.
    Planet(usize),
    /// Global flag 0x13d388[i] = 1.
    Flag(usize),
    /// Skill point i (0x13d408[i]): set when not yet, with the jingle and the banner.
    SkillPoint(usize),
    /// A code with no effect (ids 0x5b..0x90).
    None(usize),
}

impl CodeEntry {
    /// One page-menu tick (`MenuInput` is called by `SceneController` 0x28c990 every tick): `held` / `pressed` =
    /// 0x13cb00 / 0x13cb04.
    pub fn step(&mut self, held: u32, pressed: u32, table: &[u8]) -> Option<CodeEffect> {
        if held & 0xf != 6 {
            self.count = 0;
            return None;
        }
        if pressed & 0xf0a0 == 0 || self.count >= 0x14 { return None; }
        let k = if pressed & 0x1000 != 0 {
            0
        } else if pressed & 0x4000 != 0 {
            1
        } else if pressed & 0x8000 != 0 {
            2
        } else if pressed & 0x2000 != 0 {
            3
        } else if pressed & 0x80 != 0 {
            4
        } else {
            5
        };
        self.keys[self.count as usize] = k;
        self.count += 1;
        if self.count != 0x14 || table.len() < 256 { return None; }
        let id = (2u32..0x93).find(|&c| (0..0x14u32).all(|i| self.keys[i as usize] == table[((c * (i + 1)) & 0xff) as usize] as u16))? as usize - 2;
        Some(match id {
            0..=0x24 => CodeEffect::Item(id),
            0x25..=0x36 => CodeEffect::Planet(id - 0x24),
            0x37..=0x3c => CodeEffect::Flag(id - 0x37),
            0x3d..=0x5a => CodeEffect::SkillPoint(id - 0x3d),
            _ => CodeEffect::None(id),
        })
    }
}

/// What applying a code leaves for the engine (the banner of the unlocked planet unless current, the skill point's
/// jingle and banner).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodeOut {
    /// `ShowBanner(msg, ticks)`.
    pub banner: Option<(i32, i32)>,
    /// `PlayLevelSoundAtMoby(1, 0, 0)`.
    pub jingle: bool,
}

/// The effect of a recognised code on the saved game (`MenuInput` 0x298f80's tail).
pub fn apply_code(e: CodeEffect, gs: &mut GameState) -> CodeOut {
    let g = &mut gs.global;
    match e {
        CodeEffect::Item(i) => {
            g.acquired[i] = 1;
            g.owned[i] = 1;
            CodeOut::default()
        }
        CodeEffect::Planet(p) => {
            // UnlockPlanet 0x2756d0: nothing when already unlocked; else the bit, the map order and the planet banner
            // unless p is the current level.
            if g.planet_unlocked[p] != 0 { return CodeOut::default(); }
            let current = g.level;
            gs.unlock_planet(p);
            let banner = (p as i32 != current).then(|| (crate::cinematic::PLANET_BANNERS[p.min(18)], crate::hud::scale_ticks(crate::cinematic::PLANET_BANNER_TICKS)));
            CodeOut { banner, jingle: false }
        }
        CodeEffect::Flag(i) => {
            g.flags[i] = 1;
            CodeOut::default()
        }
        CodeEffect::SkillPoint(i) => {
            if g.skill_points[i] != 0 { return CodeOut::default(); }
            g.skill_points[i] = 1;
            CodeOut { banner: Some((msg::SKILL_POINT, crate::hud::scale_ticks(BANNER_TICKS))), jingle: true }
        }
        CodeEffect::None(_) => CodeOut::default(),
    }
}
