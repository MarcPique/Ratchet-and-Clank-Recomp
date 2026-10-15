//! The runtime moby record (0x100 bytes in the game, array at `0x15ffd8`, level01) and the moby table
//! with the game's slot allocator. Spec: `docs/plan/moby_update_catalogue.md` §1/§7,
//! `docs/plan/moby_render_notes.md` §1, `docs/plan/moby_animation.md`.
//!
//! The Rust struct keeps the fields the gameplay code touches, named, with the game offset in each
//! doc comment, so a trace of the real 0x100-byte record can be diffed field by field.
//! [`init_instance`](Moby::init_instance) is `InitMobyInstance` (level01 0x263488, boot 0x20c5f0) minus the
//! class-table lookups the caller supplies; [`MobyTable::create`] / [`MobyTable::delete`] /
//! [`MobyTable::free_slot_pass`] are `CreateMoby` 0x263390, `DeleteMoby` 0x2636c0 and the per-frame
//! bookkeeping 0x263300.
//!
//! **Allocator (the game's "free list").** There is no list: `CreateMoby` scans the dynamic part of the
//! array (`[0x15ffdc, 0x15ffe0)`, i.e. after the level's static instances) for the first slot whose state
//! byte is ≥ 0xfe (0xfe = deleted dynamic, 0xff = end of the used array) **and** whose deletion tick
//! (u64 at +0x38, written by `DeleteMoby` as `counter + 2`) is ≤ the tick counter `0x15f5cc`. Taking the
//! 0xff end marker moves the marker one slot on. Deleted static mobys get state 0xfd and are never reused.
//! A dynamic slot's pvars are the fixed 0x80-byte block `0x15ffe8 + slot·0x80`, zeroed on create.
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::assign_op_pattern)] // FPU compare semantics and op order are spelled out on purpose.

use rc_formats::moby_anim::AnimState;

/// Size of a dynamic moby's pvar block (`CreateMoby`).
pub const DYNAMIC_PVAR_SIZE: usize = 0x80;

/// State byte (+0x20) values with a fixed meaning.
pub mod state {
    /// Deleted static moby (never reused).
    pub const DELETED_STATIC: u8 = 0xfd;
    /// Deleted dynamic moby (reusable two ticks later).
    pub const DELETED: u8 = 0xfe;
    /// End of the used array.
    pub const END: u8 = 0xff;
}

/// Mode bits (+0x34).
pub mod mode {
    /// Hidden (MobyProc skips `mode & 0x81`).
    pub const HIDDEN: u16 = 0x1;
    /// No update (no update function, or driven elsewhere: Ratchet).
    pub const NO_UPDATE: u16 = 0x2;
    /// Keep the matrix (skip `fun_0020def8`).
    pub const KEEP_MATRIX: u16 = 0x4;
    /// Class has a glow colour (class +0x40).
    pub const GLOW: u16 = 0x10;
    /// No animation advance.
    pub const NO_ANIM: u16 = 0x40;
    /// Keep the rotation rows (not rebuilt from the Euler angles).
    pub const KEEP_ROWS: u16 = 0x100;
    /// Class +0xf set (0x7f/0x84/0x88/0xbd initialised).
    pub const CLASS_F: u16 = 0x400;
    /// Targetable (collected into the 0x1abe80 list).
    pub const TARGETABLE: u16 = 0x1000;
    /// Mirror: row 1 negated.
    pub const MIRROR: u16 = 0x8000;
}

/// One runtime moby.
#[derive(Clone, Debug)]
pub struct Moby {
    /// +0x00: bounding sphere (centre xyz, radius w), rebuilt by `fun_0020def8`.
    pub bsphere: [f32; 4],
    /// +0x10: position (game units; for Ratchet: the feet).
    pub position: [f32; 4],
    /// +0x20: state byte (per-class state machine, 0 = init; 0xfd/0xfe deleted, 0xff end of array).
    pub state: u8,
    /// +0x21: group (s8, −1 = none).
    pub group: i8,
    /// +0x22: class slot (`0x198040[o_class]`).
    pub class_slot: u8,
    /// +0x23: alpha (0x80).
    pub alpha: u8,
    /// +0x24: class header pointer (`0x197780[slot]`), here the class slot's presence.
    pub has_class: bool,
    /// +0x2c: scale = class scale (class +0x24) × instance scale.
    pub scale: f32,
    /// +0x30: update distance (0xff = always).
    pub update_dist: u8,
    /// +0x31: "drawn last frame" (set by MobyProc; keeps the moby active).
    pub visible: u8,
    /// +0x32: draw distance (s16).
    pub draw_dist: i16,
    /// +0x34: mode bits ([`mode`]).
    pub mode: u16,
    /// +0x36: occlusion bits (0x7f80 default).
    pub occlusion: u16,
    /// +0x38: light word (set 0, set 1, cross-fade). For a deleted moby the u64 at +0x38 is the tick it
    /// becomes reusable ([`delete_tick`](Self::delete_tick)).
    pub light: u32,
    /// +0x3c..0x3e: ambient r, g, b (0x40 default), +0x3f.
    pub ambient: [u8; 4],
    /// +0x40: Euler rotation (x, y, z radians; w unused).
    pub rotation: [f32; 4],
    /// +0x50..+0x5f, +0x70, +0x7e: animation state (frames/seqs +0x50..+0x53, t +0x54, speed +0x58,
    /// rate +0x5c, flags +0x70, trigger count +0x7e). `rc_formats::moby_anim`.
    pub anim: AnimState,
    /// +0x71 / +0x72 / +0x73: 0xff, class +0xe, 0x18 when class +6 ≠ 0.
    pub b71: u8,
    pub b72: u8,
    pub b73: u8,
    /// +0x74: update function (level class table), `None` → mode |= 2.
    pub update_fn: Option<u32>,
    /// +0x78: pvar block (raw bytes; per-class layout).
    pub pvars: Vec<u8>,
    /// +0x7c: loop-sound byte (0xff), +0x7d (0xff), +0x7f (0x18 with class +0xf: the shadow range in units of 1024).
    pub b7c: u8,
    pub b7d: u8,
    pub b7f: u8,
    /// +0x84 / +0x88: the shadow's ground slab `[lo, hi]` (hi ≤ 0: no shadow), filled each tick by the class's
    /// slab rule (crate::shadows); 0 from `InitMobyInstance`.
    pub shadow_lo: f32,
    pub shadow_hi: f32,
    /// +0x90: glow colour (class +0x40).
    pub glow: u32,
    /// +0x94: collision blob (class +0x10) present.
    pub has_collision: bool,
    /// +0x98: per-primitive collision disable bits.
    pub coll_disable: u32,
    /// +0xa0..+0xa3: 0x7f, 0x7f, 0x80, 0x80.
    pub ba0: [u8; 4],
    /// +0xa4: hit-message slot (0xff = none).
    pub hit_slot: u8,
    /// +0xa6: o_class (s16). Ratchet = 0.
    pub o_class: i16,
    /// +0xa8 / +0xac: `index << 16` and the array index (uid).
    pub uid_hi: u32,
    pub index: u32,
    /// +0xb0: mission byte index, +0xb1: spawn flag byte, +0xb2: spawn id (s16), +0xb4/+0xb6 (s16).
    pub mission: u8,
    pub spawn_flag: u8,
    pub spawn_id: i16,
    pub b4: i16,
    pub b6: i16,
    /// +0xb8: spawner (pointer in the game): `BoltSpawn` 0x2bcdb8 stores the root of the spawner's own +0xb8
    /// chain; `CollectBolt` 0x2bc4f0 credits the spawner's +0xb1 byte. (Added for `moby_update`.)
    pub parent: Option<MobyId>,
    /// +0xbc: scratch / external command byte; +0xbd.
    pub cmd: u8,
    pub bbd: u8,
    /// +0xc0 / +0xd0 / +0xe0: rotation rows (row i = image of model axis i), +0xf0 row 3.
    pub rows: [[f32; 4]; 4],
    /// u64 at +0x38 while deleted: the tick counter value from which the slot can be reused.
    pub delete_tick: u64,
    /// +0x64: the runtime joint-modifier list, head first, with each node's target joint resolved
    /// (`rc_formats::moby_anim::JointModifier`; written by the moby's owner, read by every pose evaluation).
    pub joint_mods: Vec<rc_formats::moby_anim::JointModifier>,
    /// Per node of [`Moby::joint_mods`] (same order): the owner record a class manipulator node is
    /// (`crate::moby_update::manip::key`); empty while the list is written whole (Ratchet's write-back).
    pub joint_mod_keys: Vec<u32>,
}

impl Default for Moby {
    fn default() -> Self { Moby::zeroed() }
}

impl Moby {
    /// `FastMemSet(moby, 0, 0x100)`.
    pub fn zeroed() -> Moby {
        Moby {
            bsphere: [0.0; 4], position: [0.0; 4], state: 0, group: 0, class_slot: 0, alpha: 0, has_class: false,
            scale: 0.0, update_dist: 0, visible: 0, draw_dist: 0, mode: 0, occlusion: 0, light: 0, ambient: [0; 4],
            rotation: [0.0; 4],
            anim: AnimState { seq_a: 0, frame_a: 0, seq_b: 0, frame_b: 0, t: 0.0, speed: 0.0, rate: 0.0, flags: 0, trigger_count: 0, skip_advance: false },
            b71: 0, b72: 0, b73: 0, update_fn: None, pvars: Vec::new(), b7c: 0, b7d: 0, b7f: 0, shadow_lo: 0.0, shadow_hi: 0.0, glow: 0,
            has_collision: false, coll_disable: 0, ba0: [0; 4], hit_slot: 0, o_class: 0, uid_hi: 0, index: 0,
            mission: 0, spawn_flag: 0, spawn_id: 0, b4: 0, b6: 0, parent: None, cmd: 0, bbd: 0, rows: [[0.0; 4]; 4], delete_tick: 0,
            joint_mods: Vec::new(),
            joint_mod_keys: Vec::new(),
        }
    }

    /// `InitMobyInstance` (0x263488) for array slot `index`. `class` = what the level class tables give for
    /// `o_class` (`None` = no class header loaded: mode |= 5).
    pub fn init_instance(index: u32, o_class: i16, class: Option<&ClassInfo>) -> Moby {
        let mut m = Moby::zeroed();
        m.alpha = 0x80;
        m.hit_slot = 0xff;
        m.group = -1;
        m.b71 = 0xff;
        m.b72 = 0xff;
        m.o_class = o_class;
        m.ambient = [0x40, 0x40, 0x40, 0];
        m.occlusion = 0x7f80;
        m.index = index;
        m.uid_hi = index << 16;
        m.b7d = 0xff;
        m.ba0 = [0x7f, 0x7f, 0x80, 0x80];
        m.b7c = 0xff;
        let Some(c) = class else {
            m.update_fn = None;
            m.mode |= mode::NO_UPDATE | 5;
            return m;
        };
        m.class_slot = c.slot;
        m.update_fn = c.update_fn;
        if m.update_fn.is_none() { m.mode |= mode::NO_UPDATE; }
        // A slot without a class header (`0x197780[slot] == 0`, a class with no blob in the level core): the
        // update function stays (boot 0x20c5f0 reads it from the slot's table entry first), +0x24 / +0x94 = 0,
        // mode |= 5.
        if c.no_header {
            m.mode |= 5;
            return m;
        }
        m.has_class = true;
        m.b72 = c.b0e;
        m.mode |= c.mode_bits;
        m.has_collision = c.has_collision;
        m.anim.rate = 1.0;
        m.scale = c.scale;
        m.anim.speed = 1.0;
        if let Some(g) = c.glow { m.mode |= mode::GLOW; m.glow = g; }
        if c.b0f != 0 { m.b7f = 0x18; m.mode |= mode::CLASS_F; }
        if c.b06 != 0 { m.b73 = 0x18; }
        if let Some(seq0) = c.seq0 {
            if seq0.frame_count >= 2 { m.mode &= !mode::NO_UPDATE; }
            if c.b0c == 1 && seq0.frame_count < 2 {
                m.anim.speed = 0.0;
                if seq0.loop_sound_bit7 { m.mode |= mode::NO_ANIM; m.anim.skip_advance = true; }
            }
        }
        m
    }

    pub fn is_deleted(&self) -> bool { self.state >= state::DELETED_STATIC }
}

/// What `InitMobyInstance` reads from the class tables (class header fields by offset).
#[derive(Clone, Copy, Debug, Default)]
pub struct ClassInfo {
    pub slot: u8,
    /// The slot has no class header (`0x197780[slot] == 0`): only `slot` and `update_fn` are used.
    pub no_header: bool,
    /// Level class table (`0x197b00[slot]`): the update function address, `None` for no entry.
    pub update_fn: Option<u32>,
    /// +0x06, +0x0c, +0x0e, +0x0f bytes.
    pub b06: u8,
    pub b0c: u8,
    pub b0e: u8,
    pub b0f: u8,
    /// +0x10 collision blob present.
    pub has_collision: bool,
    /// +0x24 class scale.
    pub scale: f32,
    /// +0x40 glow colour (non-zero).
    pub glow: Option<u32>,
    /// +0x44 mode bits.
    pub mode_bits: u16,
    /// +0x46 the class type byte (5: a creature the mouse 1818's search shoots at, `classes::mouse::search`).
    pub ty: u8,
    /// +0x48 sequence 0 header: frame count (+0x10) and bit 7 of the loop-sound byte (+0x11). (+0x0c = sequence count.)
    pub seq0: Option<Seq0Info>,
    /// +0x28 the class's sound definitions are present (the classes that test it before `PlayClassSound`).
    pub has_sounds: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Seq0Info {
    pub frame_count: u8,
    pub loop_sound_bit7: bool,
}

/// Index of a moby in the table.
pub type MobyId = usize;

/// The moby array: static instances first (`[0x15ffd8, 0x15ffdc)`), then the dynamic slots
/// (`[0x15ffdc, 0x15ffe0)`).
#[derive(Clone, Debug, Default)]
pub struct MobyTable {
    pub mobys: Vec<Moby>,
    /// First dynamic slot (`0x15ffdc`).
    pub first_dynamic: usize,
    /// `0x15ffbc`: the free dynamic slots, a **count** (0x263300, verified): recounted by the bookkeeping pass
    /// [`free_slot_pass`](Self::free_slot_pass) at the start of every tick and decremented by every
    /// `CreateMoby` ([`create`](Self::create)). Bolts (idle despawn `< 200`), `SetDeathBits` (flags 8 / 2 below
    /// 100 / 50) and the crate break (debris counts) read it.
    pub free_slots: i32,
}

impl MobyTable {
    /// A table with the level's static mobys followed by `dynamic` free slots (the first marked 0xff).
    pub fn new(static_mobys: Vec<Moby>, dynamic: usize) -> MobyTable {
        let first_dynamic = static_mobys.len();
        let mut mobys = static_mobys;
        for i in 0..dynamic {
            let mut m = Moby::zeroed();
            m.state = state::END;
            m.index = (first_dynamic + i) as u32;
            mobys.push(m);
        }
        MobyTable { mobys, first_dynamic, free_slots: 0 }
    }

    /// Ratchet's moby: the one with o_class (+0xa6) == 0 (`FUN_00226b70`, hero init → 0x1413d0).
    pub fn hero(&self) -> Option<MobyId> {
        self.mobys.iter().position(|m| m.o_class == 0 && !m.is_deleted() && m.has_class)
    }

    fn reusable(m: &Moby, counter: u64) -> bool { m.state >= state::DELETED && m.delete_tick <= counter }

    /// `CreateMoby__Fi` (0x263390): the first reusable dynamic slot at tick `counter` (`0x15f5cc`),
    /// initialised by [`Moby::init_instance`] with a zeroed 0x80-byte pvar block. `None` when full.
    pub fn create(&mut self, o_class: i16, class: Option<&ClassInfo>, counter: u64) -> Option<MobyId> {
        let n = self.mobys.len();
        for i in self.first_dynamic..n {
            if Self::reusable(&self.mobys[i], counter) {
                if self.mobys[i].state == state::END && i + 1 < n { self.mobys[i + 1].state = state::END; }
                let mut m = Moby::init_instance(i as u32, o_class, class);
                m.pvars = vec![0; DYNAMIC_PVAR_SIZE];
                self.mobys[i] = m;
                if self.free_slots != 0 { self.free_slots -= 1; }
                return Some(i);
            }
        }
        None
    }

    /// `DeleteMoby` (0x2636c0): state 0xfd (static) / 0xfe (dynamic), reusable from `counter + 2`.
    /// (The moby-grid removal `UpdateMobyGrids` is not modelled.)
    pub fn delete(&mut self, id: MobyId, counter: u64) {
        let m = &mut self.mobys[id];
        m.state = if id < self.first_dynamic { state::DELETED_STATIC } else { state::DELETED };
        m.delete_tick = counter + 2;
    }

    /// 0x263300, first call of the gameplay tick: `[0x15ffbc]` = the number of dynamic slots `CreateMoby`
    /// could take at tick `counter`: +1 for each slot with state ≥ 0xfe whose deletion tick (+0x38) ≤
    /// `counter`; once the 0xff end marker has been counted, +1 for every later slot too.
    pub fn free_slot_pass(&mut self, counter: u64) {
        let mut n = 0;
        let mut past_end = false;
        for m in &self.mobys[self.first_dynamic..] {
            if Self::reusable(m, counter) || past_end {
                n += 1;
                if m.state == state::END { past_end = true; }
            }
        }
        self.free_slots = n;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_reuses_slots_two_ticks_after_delete() {
        let mut t = MobyTable::new(vec![Moby::init_instance(0, 0, Some(&ClassInfo::default()))], 3);
        let a = t.create(13, None, 0).unwrap();
        let b = t.create(13, None, 0).unwrap();
        assert_eq!((a, b), (1, 2));
        assert_eq!(t.mobys[3].state, state::END);
        t.delete(a, 10);
        assert_eq!(t.mobys[a].state, state::DELETED);
        assert_eq!(t.create(14, None, 11), Some(3)); // slot 1 not reusable before tick 12
        t.delete(3, 11);
        assert_eq!(t.create(14, None, 12), Some(1));
        assert_eq!(t.mobys[1].pvars.len(), DYNAMIC_PVAR_SIZE);
        assert_eq!(t.hero(), Some(0));
        t.delete(0, 12);
        assert_eq!(t.mobys[0].state, state::DELETED_STATIC);
    }

    #[test]
    fn a_slot_without_header_keeps_its_update() {
        let c = ClassInfo { slot: 7, no_header: true, update_fn: Some(0x2bd100), ..Default::default() };
        let m = Moby::init_instance(3, 27, Some(&c));
        assert_eq!((m.class_slot, m.update_fn, m.mode, m.has_class), (7, Some(0x2bd100), 5, false));
        let c = ClassInfo { no_header: true, ..c };
        let c = ClassInfo { update_fn: None, ..c };
        assert_eq!(Moby::init_instance(3, 27, Some(&c)).mode, 7);
    }

    #[test]
    fn free_slot_pass_counts_reusable_and_trailing_slots() {
        let mut t = MobyTable::new(vec![Moby::init_instance(0, 0, Some(&ClassInfo::default()))], 4);
        t.free_slot_pass(0);
        assert_eq!(t.free_slots, 4);
        let a = t.create(13, None, 0).unwrap();
        assert_eq!(t.free_slots, 3, "CreateMoby decrements");
        t.create(13, None, 0).unwrap();
        t.free_slot_pass(0);
        assert_eq!(t.free_slots, 2, "the end marker and the slot after it");
        t.delete(a, 5);
        t.free_slot_pass(6);
        assert_eq!(t.free_slots, 2, "deleted at 5: not reusable before 7");
        t.free_slot_pass(7);
        assert_eq!(t.free_slots, 3);
    }
}
