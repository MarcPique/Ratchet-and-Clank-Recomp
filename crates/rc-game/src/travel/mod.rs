//! Planet travel: the player's ship (class 531 / 532 / 533), game mode 6 (take-off, landing, fly-away, the flight
//! between planets), `DoSpaceTransition` (the story cards and the transition movies) and the hand-over to the next
//! level. Spec: docs/plan/progression.md `## travel`, docs/plan/cutscenes_transitions.md §4, docs/plan/menus.md §5.
//! Addresses are level01.elf; every function here has a code-identical copy on all 19 overlays (masked overlay-diff,
//! 2026-10-01: `ShipUpdate` 0x2a1c40, `GameStateUpdate` 0x2a4080, `ShipTravelTo` 0x2a2848, `ShipLandingStart`
//! 0x2a29c0, `DoSpaceTransition` 0x2a68f8 found on 00..18) and the tables they read (the hatch 0x1bdd00, the riders
//! 0x1be200 / 0x1be230, the trail points 0x1bdf30, the flight's shell directions 0x1be060) are byte-identical on
//! every level: one port, the constants below.
//!
//! **What is a system here.** The game has one leave-the-level path: a caller sets the destination 0x15f5c0 and
//! 0x15f570 = 1, the level's main loop (`entry` 0x259c40) ends and the boot runs `DoSpaceTransition` 0x2a68f8, which
//! plays the story cards / movies its rules pick, loads the destination behind the flight or a card, and starts the
//! new overlay's `entry` (level init, the landing). The callers: the fly-away's end ([`space`] sub 3), the story
//! trips `0x2a29a0(p)` (Veldin's Clank 834, class 436, the memory-card resets of `UpdateModeFreeze`), Quit Game
//! (destination −1: the title), New Game / Load (the boot path). In the port that is [`transition::plan`] (the
//! pure rules) driven by the engine's one runtime level change (`rc-engine` `level_switch`). The death reload is
//! **not** a level change (`LoadLevelCoreData(0, 1)` inside the same `entry` loop; G-CLS-030).
//!
//! | module | game | here |
//! |---|---|---|
//! | [`ship`] | `ShipUpdate` 0x2a1c40, the adoption init 0x2a2360, `0x2a2450` / `0x2a2480`, the draw callbacks 0x2a2130 (shadow), 0x2a2ab8 (flames), 0x2a2d28 (trail), the exhaust `fun_0022f5b0` | the class port and its quads |
//! | [`space`] | `EnterShipMode` 0x2a24b8, `ShipTakeOff` 0x2a27c8, `ShipTravelTo` 0x2a2848, `0x2a29a0`, `ShipLandingStart` 0x2a29c0, `GameStateUpdate` 0x2a4080 (subs 0 / 8 / 3), `SpaceLoadingLoop` 0x2a33b0 (sub 4), `EnterSpaceLoadingLoop` 0x2a5868 | the mode-6 state machine |
//! | [`transition`] | `DoSpaceTransition` 0x2a68f8 | the step list (fades, cards, movies, flight, the saved-game writes) |
//! | [`cards`] | `PlayStoryTransition` 0x2a64e0 (boot `fun_00231bd8`), `fun_00231878`, `fun_002316e8` | the story title cards (`space_plates`) |
//! | [`title`] | the boot's title world: `fun_001eb0a8` 0x1eb0a8 (its scene), `transition_update_movie_camera` 0x1eaf88, the scene sounds of 0x1eb798 | the space-scene loop behind the title (G-SAV-012) |
//!
//! The ship globals live in the boot block 0x13e030.. ([`ShipGlobals`]); the moby loop's ship update and mode 6 share
//! them through `Services::travel`.

pub mod cards;
pub mod ship;
pub mod space;
pub mod title;
pub mod transition;

use crate::moby_runtime::MobyId;

/// `0x160548[ship]`: the ship classes (531 / 532 / 533).
pub use rc_formats::moby_spawn::SHIP_CLASSES;
/// `0x160558[ship]`: the second class of each spaceships file (535 / 536 / 537; the take-off scenes' extra actor).
pub use rc_formats::moby_spawn::SHIP_EXTRA_CLASSES;

/// Levels the game has (0 Veldin .. 18 Veldin finale): `ShipLandingStart`'s `0x13 < level` test.
pub const LEVEL_COUNT: i32 = 19;

/// `gp−0x6630` 0x1605d0[ship]: the take-off scene's skip tick (`ticks(558)`, the same for the three ships).
pub const TAKEOFF_SKIP_TICK: [i32; 3] = [558; 3];
/// `gp−0x6690` 0x160570[ship]: the landing scene's start tick on a revisit landing (`ShipTravelTo` to the current
/// planet: `ticks(240)`).
pub const LANDING_REVISIT_TICK: [i32; 3] = [240; 3];
/// 0x1bdd00 + ship·0x10: the hatch point in the ship's frame (1, 0, 0) for all three ships.
pub const HATCH: [[f32; 3]; 3] = [[1.0, 0.0, 0.0]; 3];
/// 0x1be200 + ship·0x10: Ratchet's seat in the fly-away (the rider class 0).
pub const RIDER_RATCHET: [[f32; 3]; 3] = [[1.6, 0.45, 0.9], [2.0, 0.5, 2.1], [0.7, 0.3, 0.7]];
/// 0x1be230 + ship·0x10: Clank's seat (the rider class 10).
pub const RIDER_CLANK: [[f32; 3]; 3] = [[1.6, -0.45, 1.3], [2.0, -0.5, 2.6], [0.7, -0.3, 1.25]];
/// 0x1bdf30 + ship·0x20 / 0x1bdf40 + ship·0x20: the two trail points (the wing tips' engines) in the ship's frame.
pub const TRAIL_A: [[f32; 3]; 3] = [[-3.5, 1.5, 1.2], [-3.5, 1.8, 2.6], [-3.5, 1.2, 1.2]];
pub const TRAIL_B: [[f32; 3]; 3] = [[-3.5, -1.5, 1.2], [-3.5, -1.8, 2.6], [-3.5, -1.2, 1.2]];
/// 0x1bdfb0 + ship·8: the trail's head / tail colours (`FastTweenColor` from the first to the second along it). The
/// flight's variant 4 rewrites their alpha bytes (`SpaceLoadingLoop`; [`space::Flight`]).
pub const TRAIL_COLOURS: [[u32; 2]; 3] = [[0x3880_a0c0, 0x0020_58b0], [0x3880_a0c0, 0x0020_58b0], [0x3880_c080, 0x0030_8030]];
/// `gp−0x6670` 0x160590[ship]: the trail's half width.
pub const TRAIL_WIDTH: [f32; 3] = [0.8; 3];
/// 0x13e0f0 / 0x13e2f0: the trail's ring of samples.
pub const TRAIL_SAMPLES: usize = 32;

/// The level settings words the loader copies into the ship block (`InitLevelRenderGlobals` 0x255958: settings
/// +0x3c / +0x40 / +0x44 → 0x13e060 / 0x13e064 / 0x13e068): the fly-away path and the two cuboids the fly-away
/// camera eases between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlyAwaySetup {
    /// 0x13e060: the path (`0x1b0930[i]`), −1: none (no fly-away riders, no path).
    pub path: i32,
    /// 0x13e064 / 0x13e068: the cuboids (`0x1600ec[i]`) the camera eases from / to; −1: the camera stays.
    pub cam_a: i32,
    pub cam_b: i32,
}

impl Default for FlyAwaySetup {
    fn default() -> Self { FlyAwaySetup { path: -1, cam_a: -1, cam_b: -1 } }
}

impl FlyAwaySetup {
    /// From the decompressed gameplay file (level settings = the section at pointer 0).
    pub fn parse(gameplay: &[u8]) -> FlyAwaySetup {
        let rd = |o: usize| gameplay.get(o..o + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap()));
        let Some(base) = rd(0).filter(|&b| b > 0).map(|b| b as usize) else { return FlyAwaySetup::default() };
        FlyAwaySetup { path: rd(base + 0x3c).unwrap_or(-1), cam_a: rd(base + 0x40).unwrap_or(-1), cam_b: rd(base + 0x44).unwrap_or(-1) }
    }
}

/// The ship block (boot 0x13e030..) as the level's classes and mode 6 use it.
#[derive(Clone, Debug, Default)]
pub struct ShipGlobals {
    /// 0x13e030: the ship moby (the loader's `CreateMoby(0x160548[ship])`).
    pub moby: Option<MobyId>,
    /// 0x13e056: the ship index (set by `DoSpaceTransition` before the load; [`ship_for`]).
    pub ship: i16,
    /// 0x13e058: the take-off scene skips at once (the level-10 Clank boarding's take-off).
    pub autoskip: bool,
    /// 0x15f630: the ship's △ (take-off on the next mode-0 frame, `InLevelFrameUpdate`).
    pub take_off: bool,
    /// 0x13e05a: set by the memory-card resets of `UpdateModeFreeze` (with `0x2a29a0(0)`), cleared by `ShipLandingStart`.
    pub reset_trip: bool,
    /// The fly-away path and camera cuboids (0x13e060..0x13e068).
    pub setup: FlyAwaySetup,
    /// 0x13e090 / 0x13e0a0: where the landing scene's end puts Ratchet (`HeroTeleport`): his position and rotation
    /// after `HeroInit` 0x226b70 (unless the level-13 adopted ship's fixed spot, 0x160540).
    pub landing_spot: Option<([f32; 4], [f32; 4])>,
    /// 0x160540: a placed ship was adopted (`0x2a2360`: [`ship::adopt`]); never cleared, so the engine keeps it for the
    /// session (the level-13 loads with ship 2 adopt the placed 533 again).
    pub adopted: bool,
    /// 0x13e050: the mode-6 substate (0 take-off, 8 landing, 3 fly-away, 4 the flight), mirrored by [`space`] for the
    /// moby loop's readers (the shadow's fade).
    pub sub: i32,
    /// 0x13e054: the substate's tick (−1 after an entry, +1 at the start of each `GameStateUpdate`).
    pub tick: i16,
    /// 0x13e0f0 / 0x13e2f0 / 0x13e080 / 0x13e084: the trail ring (the fly-away's and the flight's).
    pub trail: ship::Trail,
}

/// `DoSpaceTransition`'s ship index 0x13e056 for destination `dest`: 0, then 1 when planet 8 is unlocked
/// (0x13dd48) or `dest` > 7, then 2 when planet 14 is unlocked (0x13dd4e) or `dest` > 13.
pub fn ship_for(planet_unlocked: &[u8], dest: i32) -> i16 {
    let u = |p: usize| planet_unlocked.get(p).is_some_and(|&b| b != 0);
    let mut s = 0;
    if u(8) || dest > 7 { s = 1; }
    if u(14) || dest > 13 { s = 2; }
    s
}

/// The ship index clamped to the tables' range.
pub fn ship_index(s: i16) -> usize { s.clamp(0, 2) as usize }

/// `MatrixMulVec3(out, v, rows)` + position: a point of the moby's frame in the world (rows = the images of the model
/// axes, +0xc0..+0xe0).
pub fn to_world(rows: &[[f32; 4]; 4], pos: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|k| rows[0][k] * v[0] + rows[1][k] * v[1] + rows[2][k] * v[2] + pos[k])
}

/// `MatrixMulVec3(out, v, rows)` alone (a direction).
pub fn rotate(rows: &[[f32; 4]; 4], v: [f32; 3]) -> [f32; 3] { std::array::from_fn(|k| rows[0][k] * v[0] + rows[1][k] * v[1] + rows[2][k] * v[2]) }
