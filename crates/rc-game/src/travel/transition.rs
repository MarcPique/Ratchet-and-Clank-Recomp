//! `DoSpaceTransition` (level01 0x2a68f8, the boot's 0x231ff0): what runs between leaving a level and the next
//! level's `entry` — the story cards, the transition movies, the flight that hides the load, and the saved-game
//! writes — as a list of steps the engine plays ([`plan`]). Spec: docs/plan/cutscenes_transitions.md §4.2–4.3, §6.
//!
//! | address | what | here |
//! |---|---|---|
//! | 0x2a6904 | 0x1742d0 \|= 0x80000000; mode 6; the ship index 0x13e056 (`super::ship_for`) | [`plan`] (`Plan::ship`) |
//! | 0x2a69b8 | `snd_StopAll`, `sound_StopAllSounds`, `music_Stop`, 0x1516db = 1 (the streams), the level's sound bank unloaded (`0x15f5f4 + 0x1c`, logged), the fog block 0x16d0d8.. (colour 0, near 524288, far 128 / 255, `0x16d0f8` = 0x10), `SetBackgroundColor(0, 0, 0)` | the engine's `Step::Begin` (sounds, music, the black background) |
//! | 0x2a6aac dest < 0 | wait for the memory card; `FadeToBlack(ticks(6))`; 0x15ed84 = dest; `load_level_chunk_from_disc` (0x257d50: the front end) | [`Step::Fade`], [`Step::SetLevel`], [`Step::FrontEnd`] |
//! | 0x2a6b44 dest 0, visited[0] = 0 | `FadeToBlack(ticks(6))`; cards 0 + 1 (`ticks(240)`), movie 0 (`mpegs[40]`); card 2 (180), movie 1; 0x15ed84 = 0; cards 3 + 4 (240, loading), movie 2 | the steps of the new game's opening |
//! | 0x2a6bf8 level 0 → 1, visited[1] = 0 | `FadeToBlack(ticks(6))`; cards 5 + 6 (240), movies 3, 4; card 7 (180), movie 5; visited[**0**] = 2 (the old level: its index is loaded before the store of 0x15ed84, 0x2a6c88); 0x15ed84 = 1; card 8 (240, loading) | the first arrival at Novalis |
//! | 0x2a6cd0 dest 4, visited[4] = 0 | `FadeToBlack(ticks(12))`, cards 9 + 10, movie 6 | [`plan`] |
//! | 0x2a6d2c leaving 7, visited[7] ≠ 2, planet 8 unlocked | `FadeToBlack(ticks(12))`, card 11, movie 7 | [`plan`] |
//! | 0x2a6d98 dest 13, visited[13] = 0 | `FadeToBlack(ticks(12))`, cards 12 + 13, movie 8 | [`plan`] |
//! | 0x2a6df4 leaving 14, visited[14] ≠ 2, planet 15 unlocked | `FadeToBlack(ticks(12))`, card 14, movie 9 | [`plan`] |
//! | 0x2a6e60 dest 16, visited[16] = 0 | `FadeToBlack(ticks(12))`, cards 15 + 16, movie 10 | [`plan`] |
//! | 0x2a6ebc | the level left (< 0x13) visited = 2, unless 7 with planet 8 locked or 14 with planet 15 locked; 0x15ee4a = 1 (the flight runs), 0x15ed84 = dest, 0x15ee48 = 0 (the loader's state) | [`Step::SetLevel`] (`GameState::apply_transition`) |
//! | 0x2a6f2c | `EnterSpaceLoadingLoop`, `read_file_entry_with_retry(dest)`, the frame loop (`GameStateUpdate` sub 4) polling the loader `FUN_00257dc8` until the flight sets 0x15f5d8; then the loader, then the memory card | [`Step::Flight`] (`super::space::ShipMode::flight_start`) |
//! | 0x2a7078 | `func_0x00120c30(0)`, 0x1516db = 0, `0x2b4f28` | the engine's hand-over to the new level's `entry` |
//!
//! The card steps carry `cards::lang_index` of 0x15ed88 (`lang − 1`, ≥ 0); a card's `load` = the loader polled while it
//! shows (it stays up ≥ 20 frames past the load's end, [`super::cards::CardPlayer`]).

use crate::game_state::GameState;

/// One step of the transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The common start (sounds stopped, the music stopped, the background black; module docs).
    Begin,
    /// `FadeToBlack(n)` (blocking, over the last image).
    Fade(u32),
    /// `PlayStoryTransition(lang, a, b, ticks, load)` 0x2a64e0: cards `a` and `b` (`space_plates[lang]` entries a + 1
    /// and b + 1; a = b: one line), then `FadeToBlack(2)`.
    Card { a: usize, b: usize, ticks: i32, load: bool },
    /// `fun_00231608(n)` 0x2a5f10: the transition movie `mpegs[40 + n]` (NTSC; PAL `mpegs[52 + n]`), then `FadeToBlack(4)`.
    Movie(i32),
    /// The saved-game writes of the branch and 0x15ed84 = dest (`GameState::apply_transition`); the load of `dest` may
    /// start from here.
    SetLevel,
    /// The flight (`EnterSpaceLoadingLoop` and the sub-4 loop) until it ends with the level loaded.
    Flight,
    /// `load_level_chunk_from_disc` with level −1: the front end (the title; the saves lane's).
    FrontEnd,
    /// The new level's `entry` (the load finished).
    Enter,
}

/// The transition from the current level to `dest`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub dest: i32,
    pub from: i32,
    /// 0x13e056 for the destination.
    pub ship: i16,
    pub steps: Vec<Step>,
    /// The flight's first-arrival form (dest 0, or dest 1 with planet 3 locked: variant 4 alone, the ship hidden).
    pub first_arrival: bool,
}

/// Movie numbers of `fun_00231608(n)` and their file (`mpegs[40 + n]`).
pub const TRANSITION_MOVIE_BASE_NTSC: i32 = 40;
pub const TRANSITION_MOVIE_BASE_PAL: i32 = 52;

/// `DoSpaceTransition`'s branches for the state `gs` (before any of its writes) and the destination (module docs).
/// `ticks` = `fun_001f96f8` (n on NTSC).
pub fn plan(gs: &GameState, dest: i32, ticks: &dyn Fn(i32) -> i32) -> Plan {
    let g = &gs.global;
    let from = g.level;
    let visited = |l: i32| usize::try_from(l).ok().and_then(|l| gs.levels.get(l)).map_or(0, |s| s.visited);
    let unlocked = |p: usize| g.planet_unlocked.get(p).is_some_and(|&b| b != 0);
    let ship = super::ship_for(&g.planet_unlocked, dest);
    let mut steps = vec![Step::Begin];
    let card = |a: usize, b: usize, t: i32, load: bool| Step::Card { a, b, ticks: ticks(t), load };
    let fade = |n: i32| Step::Fade(ticks(n) as u32);
    if dest < 0 {
        steps.extend([fade(6), Step::SetLevel, Step::FrontEnd]);
        return Plan { dest, from, ship, steps, first_arrival: false };
    }
    if dest == 0 && visited(0) == 0 {
        steps.extend([fade(6), card(0, 1, 0xf0, false), Step::Movie(0), card(2, 2, 0xb4, false), Step::Movie(1), Step::SetLevel, card(3, 4, 0xf0, true), Step::Movie(2), Step::Enter]);
        return Plan { dest, from, ship, steps, first_arrival: true };
    }
    if from == 0 && dest == 1 && visited(1) == 0 {
        steps.extend([fade(6), card(5, 6, 0xf0, false), Step::Movie(3), Step::Movie(4), card(7, 7, 0xb4, false), Step::Movie(5), Step::SetLevel, card(8, 8, 0xf0, true), Step::Enter]);
        return Plan { dest, from, ship, steps, first_arrival: true };
    }
    if dest == 4 && visited(4) == 0 { steps.extend([fade(0xc), card(9, 10, 0xf0, false), Step::Movie(6)]); }
    if from == 7 && visited(7) != 2 && unlocked(8) { steps.extend([fade(0xc), card(0xb, 0xb, 0xf0, false), Step::Movie(7)]); }
    if dest == 0xd && visited(0xd) == 0 { steps.extend([fade(0xc), card(0xc, 0xd, 0xf0, false), Step::Movie(8)]); }
    if from == 0xe && visited(0xe) != 2 && unlocked(0xf) { steps.extend([fade(0xc), card(0xe, 0xe, 0xf0, false), Step::Movie(9)]); }
    if dest == 0x10 && visited(0x10) == 0 { steps.extend([fade(0xc), card(0xf, 0x10, 0xf0, false), Step::Movie(10)]); }
    steps.extend([Step::SetLevel, Step::Flight, Step::Enter]);
    let first_arrival = dest == 0 || (dest == 1 && !unlocked(3));
    Plan { dest, from, ship, steps, first_arrival }
}
