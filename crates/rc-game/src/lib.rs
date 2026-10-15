//! Gameplay logic of Ratchet & Clank (PS2, SCUS_971.99), ported from the game's own code.
//!
//! Modules mirror game subsystems and cite the level01/boot addresses they reproduce:
//! * [`collision_query`]: line / sphere / capsule queries against the level collision mesh
//!   (`docs/plan/collision_queries.md`).
//! * [`ps2v`]: VU0 macro-mode vector arithmetic on PS2 float bit patterns used by the kernels.
//! * [`rng`]: the newlib `rand` stream and the game's random helpers (`docs/plan/particles.md` §5).
//! * [`pad`]: the controller record (`UpdatePad`/`ProcessPadInput`) and its input-history queries.
//! * [`moby_runtime`]: the runtime moby record and the moby table with `CreateMoby`/`DeleteMoby`.
//! * [`moby_update`]: the moby scheduler (load pass, active list, per-moby loop), the services the class
//!   updates call, and the ported classes: bolts, crates, grass (`docs/plan/moby_update_catalogue.md`).
//! * [`hero`]: Ratchet on foot — the hero block, per-state physics, move + collide, SetState and the
//!   transitions, the animation interface (`docs/plan/player_controller.md` §1–§6).
//! * [`follow_camera`]: the type-0 gameplay camera and the `Camera` record (§7).
//! * [`tick`]: one gameplay tick in the game's order (pad → mobys → hero → particles → camera).
//! * [`particles`]: the particle pool, allocator, `UpdateParts` scheduler and type 6 (`docs/plan/particles.md`).
//! * [`sky_stars`]: the sky's star sprites, generated on the first frame and updated per frame (`docs/plan/sky_render_notes.md`).
//! * [`audio`]: sound slots, 989snd grain VM, music and a software SPU2 mixer (`docs/plan/audio.md`).
//! * [`water`]: strip-water scroll / wobble / bob and the ripple-patch simulation (`docs/plan/world_animation.md` §1–§3).
//! * [`menus`]: the mode system, the quick-select ring and the mode-3 page menus (`docs/plan/menus.md`).
//! * [`fog_zones`]: fog zones, the underwater test and `UpdateFog`'s fog selection (`docs/plan/world_animation.md` §5, §6).
//! * [`scene_player`]: the in-engine scene (cutscene) player, game mode 2 (`docs/plan/cutscenes_transitions.md` §3).
//! * [`spline`]: the spline follower (paths and grind paths: step, advance, nearest point) the rail riders use.
//! * [`shadows`]: the moby shadows' directions, slab probes and shadow volumes (`docs/plan/shadows.md`).
//! * [`game_state`]: the saved game state, new game, level-start rules, transitions, saves and options (`docs/plan/game_state.md`).
//! * [`memcard`]: the memory card natively (a folder): the card driver, the card monitor, previews, the saves and loads (`docs/plan/progression.md`).
//! * [`frontend`]: the boot flow: the card check, the logos, the title, the attract loop, the main menu's hand-over (`docs/plan/progression.md`).
//! * [`inventory`]: owned items, the equipped item per slot and the equip rules (`docs/plan/gadgets.md`).
//! * [`slideshow`]: game mode 7, the credits slideshow (`docs/plan/progression.md`).
//! * [`cheats`]: the cheat bytes, the move-sequence cheat entry, the Cheats list and the debug code entry (`docs/plan/progression.md`).
//! * [`travel`]: the ship, game mode 6 (take-off, landing, fly-away, the flight) and `DoSpaceTransition` (`docs/plan/progression.md` `## travel`).

pub mod collision_query;
pub mod ps2v;
pub mod rng;
pub mod particles;
pub mod point_lights;
pub mod pad;
pub mod moby_runtime;
pub mod moby_update;
pub mod hero;
pub mod follow_camera;
pub mod tick;
pub mod fog_zones;
pub mod sky_stars;
pub mod water;
pub mod hud;
pub mod help;
pub mod map;
pub mod audio;
pub mod game_state;
pub mod memcard;
pub mod frontend;
pub mod inventory;
pub mod menus;
pub mod scene_player;
pub mod scene_actors;
pub mod movie_player;
pub mod cheats;
pub mod slideshow;
pub mod cinematic;
pub mod travel;
pub mod spline;
pub mod path;
pub mod shadows;
pub mod targeting;
pub mod afterimage;
pub mod vehicle;
