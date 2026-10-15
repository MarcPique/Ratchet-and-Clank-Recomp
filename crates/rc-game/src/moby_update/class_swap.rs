//! The run-time class swap of a moby (G-CLS-031): class code that turns its own moby into another class while it
//! runs. **Not an engine function**: the game has no `SetMobyClass`; the class code writes the fields inline, the
//! class-dependent part of `InitMobyInstance` (level01 0x263488) over a live moby. Ported once here as a moby
//! service ([`swap`]) so a class that swaps only calls it, and every level copy of such a class registers its classes
//! in [`PORTS`] (the renderer reads [`targets`] to load the models a placed moby may take).
//!
//! **Every caller on the disc** (a scan of the 19 overlays for `sh _, 0xa6(_)` next to `sb _, 0x22(_)`, and for the
//! callers of `update_moby_animation_state` (L01 0x263718, `rc-trace overlay-diff`: the same code on every level)):
//! the engine's 18 callers per level (the sequence changes, `InitMobyInstance`), the crate update's branch toward
//! class 0x1fe (L01 `CrateUpdate` 0x2ea4c8..0x2ea580 and its copy on every level: code no level reaches, classes
//! 503 / 504 / 506–510 are in no class table; `classes::crate_`, G-CLS-009), and level04 0x2c6858 (Eudora's crank
//! followers 432 ↔ 1052, `units::eudora_crank_follower`), two swaps (0x2c690c, 0x2c6a84). No other overlay writes a
//! moby's +0xa6 outside `InitMobyInstance` (the other `sh 0xa6` stores of L01 `ElevatorUpdate` / L07 0x2fb830 are into
//! pvar records). The scan's pairing misses one bare store: level07's parked ship 529 (`0x2fbdb8`) writes +0xa6 =
//! 0x215 alone each tick (no +0x22, no animation reset): a class word for its draw, ported in place by
//! `units::umbris_small` (no row here: nothing of the swap's animation or table side runs).
//!
//! The swap of level04 0x2c690c (the same at 0x2c6a84), what it writes and what the port does:
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2c690c | +0xa6 o_class = the new class | [`swap`] (`Moby::o_class`; the port's lookups by class follow it: the animation class, the sound table `PlayClassSound` reads, the collision blob, the joint lists, the reaction table) |
//! | 0x2c691c | +0x22 class slot = `0x198040[class]` (level04: 0x197d40) | [`swap`] (`ClassInfo::slot`; the run list's slot order follows it, `scheduler::build_active_list`) |
//! | 0x2c6938 | +0x71 = 0xff (the sequence-sphere cache key: the next matrix build reloads the sphere) | [`swap`] |
//! | 0x2c693c | +0x24 header = `0x197780[slot]` (level04: 0x197480) | [`swap`] (`Moby::has_class`) |
//! | 0x2c6948 | +0x2c scale = class +0x24 (the class scale alone: the instance scale is dropped) | [`swap`] |
//! | 0x2c6944 | `update_moby_animation_state` (L04 0x2415b0 = L01 0x263718): +0x68 / +0x6c key pointers (the port evaluates by class: n/a), +0x7e = trigger count and +0x7c = loop sound of sequence A (+0x52) of the new class; A = 0xff (a snapshot): +0x7c = 0xff, +0x7e = 0; +0x7d (the loop voice) is not touched | [`update_anim_state`] |
//! | 0x2c6958 | +0x72 = class +0x0e | [`swap`] |
//! | 0x2c6964 | +0x94 collision = class +0x10 | [`swap`] (`Moby::has_collision`) |
//! | 0x2c6960 | `MobyAnimSphereLerp` (L04 0x243cd0 = L01 0x265d78): the bounding sphere from the new class's sequences, the change counter, the grid | [`swap`] (`creature::react::sphere_lerp`) |
//! | — | not written: +0x74 update (the moby keeps running the code that swapped it), the pvars, the state, mode (the glow bit 0x10, 0x400), +0x90 glow colour, +0x73 shine, +0x7f, the animation keys and speed | n/a (left as they are) |
//!
//! The draw: the port's renderer draws a placed moby with the model of its live class (`rc-engine` `moby_render`:
//! the models of [`targets`] are loaded with the level and the instance switches to the live class's entities and
//! palette); a created moby (a dynamic slot) is always drawn by its live class.
//!
//! The crate's branch (unreached) writes only +0xa6, +0x22, +0x24, `update_moby_animation_state`, +0x72 and state 2: a
//! subset of the above (n/a, no level reaches it). Inferred [L]: a class the level does not load, or a slot without a
//! class header, has no counterpart on the disc (the game would read through a null header); the port then changes
//! only +0xa6 / +0x22 / +0x24.

use crate::moby_runtime::{Moby, MobyId};
use crate::moby_update::classes::{units, ClassUpdate, LevelPorts};
use crate::moby_update::services::World;
use rc_formats::moby_anim::{MobyAnimClass, SNAPSHOT_SEQ};

/// One class port whose code swaps classes: the level and function of its reference copy (its row in
/// `units::PORTS`) and every class it swaps between.
#[derive(Clone, Copy, Debug)]
pub struct SwapPort {
    pub level: u32,
    pub func: u32,
    pub classes: &'static [i16],
}

/// The class swaps on the disc (module doc: the scan). A new consumer adds its row here and calls [`swap`].
pub const PORTS: &[SwapPort] = &[SwapPort {
    level: units::eudora_crank_follower::REFERENCE_LEVEL,
    func: units::eudora_crank_follower::UPDATE_FN,
    classes: &units::eudora_crank_follower::CLASSES,
}];

/// The other classes a moby of `o_class` may become on a level (`ports`: its class table): the classes of every
/// [`PORTS`] row whose function the level runs for `o_class` (code identity, `LevelPorts`).
pub fn targets(ports: &LevelPorts, o_class: i16) -> Vec<i16> {
    let Some(u) = ports.get(o_class) else { return Vec::new() };
    let mut out: Vec<i16> = PORTS
        .iter()
        .filter(|p| units::row(p.level, p.func).map(ClassUpdate::Unit) == Some(u))
        .flat_map(|p| p.classes.iter().copied())
        .filter(|&c| c != o_class)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The run-time class swap (module table): moby `id` becomes class `o_class`.
pub fn swap(w: &mut World, id: MobyId, o_class: i16) {
    let classes = w.classes;
    let info = classes.info(o_class);
    let m = w.mm(id);
    m.o_class = o_class;
    let Some(c) = info else { return };
    m.class_slot = c.slot;
    m.b71 = 0xff;
    m.has_class = !c.no_header;
    if c.no_header { return; }
    m.scale = c.scale;
    update_anim_state(m, classes.anim(o_class));
    m.b72 = c.b0e;
    m.has_collision = c.has_collision;
    crate::moby_update::creature::react::sphere_lerp(w, id);
}

/// `update_moby_animation_state` (L01 0x263718)'s moby bytes on `class`: sequence A (+0x52) = 0xff → +0x7c = 0xff,
/// +0x7e = 0; else +0x7e / +0x7c = that sequence's trigger count / loop sound. A sequence the class lacks (the game
/// would read past the table, no disc case [L]) leaves both.
pub fn update_anim_state(m: &mut Moby, class: Option<&MobyAnimClass>) {
    let a = m.anim.seq_a;
    if a == SNAPSHOT_SEQ {
        m.b7c = 0xff;
        m.anim.trigger_count = 0;
        return;
    }
    if let Some(q) = class.and_then(|c| c.sequence(a)) {
        m.anim.trigger_count = q.header.trigger_count;
        m.b7c = q.header.loop_sound;
    }
}
