//! Moby state at the end of level load: what the per-class init code has done to each moby before the
//! first frame is drawn, without porting the update functions. Spec: docs/plan/moby_update_catalogue.md
//! (§1 mechanism, §4 top classes, §6 "Sequence at spawn"; "In the port" at the end).
//!
//! Mechanism (level01 addresses; the class → update table is the level overlay's at 0x20bb00):
//! * `InitMobyInstance` 0x263488 zeroes the moby (sequence 0, frame 0/0, t 0, speed = rate = 1; a class
//!   with one sequence of ≤ 1 frame gets speed 0, plus mode 0x40 when its loop-sound byte has bit 7 set:
//!   [`AnimState::spawn`]).
//! * The loader `FUN_00255958` then creates the player's ship (`SHIP_CLASSES[ship]`, see
//!   [`first_visit_ship`]; its class comes from the `spaceships` file, [`parse_spaceship`], unless the
//!   level core has it) with `CreateMoby` and hard-cuts it to sequence 1 (`fun_00212ed8(ship, 1, 0)`),
//!   see [`ship_state`].
//! * `FUN_002792d0` (called once from `FUN_00258128`) runs every moby with state ≥ 0 and no mode bit 2,
//!   with no distance gate: `MobyAnimAdvance` (unless mode 0x40), then the class update, then the matrix.
//!   Every update's state-0 branch (its "init") therefore runs before the first frame.
//!
//! [`initial_state`] reproduces those init branches for the classes whose init changes something visible
//! (hidden, sequence, position). The rules are those of the Novalis (level 01) table; the same class id is
//! assumed to run the same code in other levels (true for the shared clusters in the catalogue, not checked
//! per level). Not modelled: the random mirror bit of 577 (`rand() & 1` → mode 0x8000) and every later frame's
//! behaviour. The instance loop's spawn test is [`spawn_test`] (which records the loader creates, and the
//! instance → moby map [`instance_to_moby`]); the ship's arrival hide is [`ship_hidden_on_arrival`].

use crate::gameplay::MobyInstance;
use crate::moby_anim::{self, AnimState, MobyAnimClass, MobyFrame};
use crate::tfrag_light::ps2;

/// `0x160548[ship]`, the ship classes the loader picks from (identical in every level overlay).
/// `ship` = the s16 at 0x13e056, see [`first_visit_ship`].
pub const SHIP_CLASSES: [i32; 3] = [531, 532, 533];

/// `0x160558[ship]`: the second class of each spaceship file (one joint, one sequence). The loader only
/// registers it; nothing is created from it at load.
pub const SHIP_EXTRA_CLASSES: [i32; 3] = [535, 536, 537];

/// The ship index (0x13e056) on a level's first visit in a story playthrough. `DoSpaceTransition`
/// (level01 0x2a68f8 = boot 0x231ff0) sets it before loading destination planet `level` (0x15f5c0):
/// 0, then 1 if planet 8 is unlocked (0x13dd40[8]) or `level` > 7, then 2 if planet 14 is unlocked
/// (0x13dd40[14]) or `level` > 13. A new game (Veldin, level 0) starts from the zeroed save: 0. Planets 8
/// and 14 are unlocked by finishing 7 and 13, so on the story's first visits the flags never raise the
/// index above the destination rule; a revisit of a level ≤ 13 after those unlocks uses the later ship.
pub fn first_visit_ship(level: u32) -> usize {
    match level {
        0..=7 => 0,
        8..=13 => 1,
        _ => 2,
    }
}

/// TOC `spaceships` entry (TOC+0x12c8, 4 × SectorRange; in RAM at 0x138e48) the loader reads for `ship`:
/// `FUN_00257dc8` case 1 (level01) streams entry `ship + 1` to 0x15ee54. Entry k holds class 530 + k
/// (entry 0 = class 530, which only Veldin 1 has, in its core).
pub fn spaceships_entry(ship: usize) -> usize { ship + 1 }

/// One `spaceships` file: two moby classes and one texture each (texture lists of one PIF).
/// Layout (word header, then the blocks in the order class, extra class, texture, extra texture):
/// `+0` ship class offset, `+4` ship texture list, `+8` extra class, `+0xc` extra texture list.
pub struct Spaceship<'a> {
    /// The ship class blob (`CreateMoby(SHIP_CLASSES[ship])`), up to the extra class.
    pub class: &'a [u8],
    /// The extra class blob ([`SHIP_EXTRA_CLASSES`]), up to the ship texture.
    pub extra_class: &'a [u8],
    /// Mip 0 of the ship texture (256×256 PSMT8).
    pub texture: crate::texture::Texture,
    /// The extra class's texture (128×128 PSMT8).
    pub extra_texture: crate::texture::Texture,
}

/// Parses a `spaceships` file the way the loader `FUN_00253a18` (level01) uses it: it copies the whole
/// file, then for each texture list at `t` takes the CLUT from `t + 0x30` (a 16×16 PSMCT32 upload, CSM1
/// order like the level palettes) and the indices from `t + 0x430` (mips 0–1 of the ship texture,
/// 0x14000 bytes, stay in EE RAM; mips 2 and 3 at `t + 0x14430`/`t + 0x15430` go to GS memory; the extra
/// texture is one 0x4000-byte level). The GS descriptors are the static `TextureEntry`s at 0x15fc00
/// (256×256, 4 levels) and 0x15fc10 (128×128, 1 level). Those fixed offsets are what the texture lists'
/// PIF headers (`t + 0x10`) describe; the header is checked.
pub fn parse_spaceship(blob: &[u8]) -> crate::buf::Result<Spaceship<'_>> {
    use crate::buf::{invalid, Buf};
    let b = Buf(blob);
    let (class, tex, extra, extra_tex) = (b.u32(0)? as usize, b.u32(4)? as usize, b.u32(8)? as usize, b.u32(0xc)? as usize);
    if !(class < extra && extra < tex && tex < extra_tex && extra_tex < blob.len()) {
        return invalid(format!("spaceship file: unexpected block order {class:#x} {tex:#x} {extra:#x} {extra_tex:#x}"));
    }
    let texture = |t: usize, size: u32| -> crate::buf::Result<crate::texture::Texture> {
        // Texture list: count 1, PIF at +0x10: "2FIP", size, width, height, format 0x13.
        let (n, first, magic) = (b.u32(t)?, b.u32(t + 4)?, b.sub(t + 0x10, 4, "PIF magic")?.bytes());
        let (w, h, fmt) = (b.u32(t + 0x18)?, b.u32(t + 0x1c)?, b.u32(t + 0x20)?);
        if n != 1 || first != 0x10 || magic != b"2FIP" || (w, h, fmt) != (size, size, 0x13) {
            return invalid(format!("spaceship texture at {t:#x}: not a one-PIF list of {size}x{size} PSMT8"));
        }
        let clut = b.sub(t + 0x30, 0x400, "spaceship CLUT")?;
        let px = b.sub(t + 0x430, (size * size) as usize, "spaceship texture")?;
        crate::texture::decode_indexed8(px.bytes(), size, size, clut.bytes())
    };
    Ok(Spaceship {
        class: &blob[class..extra],
        extra_class: &blob[extra..tex],
        texture: texture(tex, 256)?,
        extra_texture: texture(extra_tex, 128)?,
    })
}

/// When the sequence change of a [`SpawnState`] happens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeAt {
    /// At creation, before the load pass (the ship: the loader's own `fun_00212ed8`).
    Create,
    /// In the load pass, after its `MobyAnimAdvance` (the class init branch).
    LoadPass,
    /// In the first update that runs after load (the moby's second update: the load pass only moved it to
    /// the state that makes the change). In the game that is the first frame the moby is active
    /// (`fun_0020d868`: drawn last frame, within its update distance of the camera, or group-activated).
    FirstUpdate,
}

/// The moby's state after the load pass, as far as drawing is concerned.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnState {
    /// Mode bit 1 is set after the load pass: `MobyProc` skips the moby (`mode & 0x81`). Also true when
    /// the init deleted the moby.
    pub hidden: bool,
    /// The init called `DeleteMoby` (state 0xfd/0xfe: never drawn or updated again).
    pub deleted: bool,
    /// Sequence change: target sequence, frame, and `Some(ticks)` for a blend `fun_00212f90(seq, frame,
    /// ticks)` or `None` for a hard cut `fun_00212ed8(seq, frame)`; `change_at` = None means no change (the
    /// moby keeps playing sequence 0 from its spawn state).
    pub sequence: u8,
    pub frame: u8,
    pub blend_ticks: Option<u8>,
    pub change_at: Option<ChangeAt>,
    /// Speed (moby+0x58) written by the init after the change, if any (the hidden amoeboids: 0).
    pub speed: Option<f32>,
    /// Mode bits the init sets (moby+0x34 |=): 1 hidden, 0x40 no advance (0x41 = the "hide and freeze"
    /// idiom), 0x1000 targetable. Includes 0x40 for the static classes of [`AnimState::spawn`].
    pub mode_bits: u16,
    /// Mode bits the init clears (the hidden amoeboids clear 0x1000; the ship clears 2).
    pub mode_clear: u16,
    /// New position (moby+0x10) when the init moves the moby.
    pub position: Option<[f32; 3]>,
    /// The load pass runs this moby (advance + update): false only for Ratchet (class 0), whose hero init
    /// sets mode 2 and whose class has no sequence 0 on the disc.
    pub runs_load_pass: bool,
}

impl SpawnState {
    /// No init effect: sequence 0 from the spawn state, advanced once by the load pass.
    pub fn plain(o_class: i32, class: &MobyAnimClass) -> SpawnState {
        let spawn = AnimState::spawn(class);
        SpawnState {
            hidden: false,
            deleted: false,
            sequence: 0,
            frame: 0,
            blend_ticks: None,
            change_at: None,
            speed: None,
            mode_bits: if spawn.skip_advance { 0x40 } else { 0 },
            mode_clear: 0,
            position: None,
            runs_load_pass: o_class != 0,
        }
    }

    fn change(&mut self, at: ChangeAt, seq: u8, frame: u8, blend_ticks: Option<u8>) {
        self.change_at = Some(at);
        self.sequence = seq;
        self.frame = frame;
        self.blend_ticks = blend_ticks;
    }

    fn hide_and_freeze(&mut self) {
        self.hidden = true;
        self.mode_bits |= 0x41;
    }

    /// Applies this state's sequence change to `s` (with `snap` the moby's snapshot frame, see
    /// [`moby_anim::set_sequence`]). Used for [`ChangeAt::FirstUpdate`] by the caller's first active tick,
    /// and by [`SpawnState::anim_after_load`] for the other two. The guards of the game code
    /// (`if (m+0x53 != seq)`) are applied: no change when seq B is already the target.
    pub fn apply_change(&self, s: &mut AnimState, class: &MobyAnimClass, snap: &mut Option<MobyFrame>) -> bool {
        if self.change_at.is_none() || (s.seq_b == self.sequence && self.blend_ticks.is_some()) { return false; }
        match self.blend_ticks {
            None => moby_anim::hard_cut(s, class, self.sequence, self.frame as i32),
            Some(n) => moby_anim::set_sequence(s, class, self.sequence, self.frame as i32, n as i32, snap),
        }
    }

    /// The animation state after the load pass: spawn → (creation cut) → advance (when the pass runs the
    /// moby and mode 0x40 is clear) → (load-pass change) → speed and mode 0x40 of the init. Returns the
    /// state and its snapshot frame (a load-pass blend never needs one: t is 0 after the first advance).
    pub fn anim_after_load(&self, class: &MobyAnimClass) -> (AnimState, Option<MobyFrame>) {
        let mut s = AnimState::spawn(class);
        let mut snap = None;
        if self.change_at == Some(ChangeAt::Create) { self.apply_change(&mut s, class, &mut snap); }
        if self.runs_load_pass && !s.skip_advance { moby_anim::advance(&mut s, class); }
        if self.change_at == Some(ChangeAt::LoadPass) { self.apply_change(&mut s, class, &mut snap); }
        if let Some(v) = self.speed { s.speed = v; }
        if self.mode_bits & 0x40 != 0 { s.skip_advance = true; }
        (s, snap)
    }
}

/// What the init branches read besides the moby and its pvars.
pub trait SpawnEnv {
    /// `0x1b0930[index]`: the level's spline (gameplay paths, [`crate::gameplay::parse_splines`]).
    fn spline(&self, index: i32) -> Option<&[[f32; 4]]>;
    /// `FUN_0026e618(0.5, pos, 0)`: `CollLine_Fix` from (x, y, z + 0.5) down to (x, y, 0.01) with flags 2
    /// (world mesh only); the hit's z, or 0.0 when nothing is hit.
    fn ground_height(&self, pos: [f32; 3]) -> f32;
    /// Position (moby+0x10) of runtime moby `index` as the loader placed it.
    fn moby_position(&self, index: i32) -> Option<[f32; 3]>;
}

fn rd<const N: usize>(p: Option<&[u8]>, o: usize) -> Option<[u8; N]> { p?.get(o..o + N)?.try_into().ok() }
fn pv_i32(p: Option<&[u8]>, o: usize) -> Option<i32> { rd::<4>(p, o).map(i32::from_le_bytes) }
fn pv_i16(p: Option<&[u8]>, o: usize) -> Option<i16> { rd::<2>(p, o).map(i16::from_le_bytes) }
fn pv_f32(p: Option<&[u8]>, o: usize) -> Option<f32> { rd::<4>(p, o).map(f32::from_le_bytes) }
/// EE FPU `add.s` (round toward zero), as the init code moves positions.
fn fadd(a: f32, b: f32) -> f32 { f32::from_bits(ps2::add(a.to_bits(), b.to_bits())) }

/// The load-pass result for one placed instance. `pvar` is the instance's block after the loader's
/// fixups ([`crate::gameplay::parse_pvars`]); `class` its animation data. Per-class rules, each from the
/// state-0 branch of the class's update function (level01):
///
/// | class | fn | rule |
/// |---|---|---|
/// | 459 | 0x2e6bf0 | moves to point 0 of spline pvar+0x134 (if ≥ 0) else pvar+0x164, +20 z when pvar+0x134 < 0 and pvar+0x1b4 = 0; targetable; **hidden + frozen (0x41) when z − ground > 0.5**; pvar+0x164 = −1: nothing (printf) |
/// | 572, 865, 866 | 0x2edca0 | pvar+0x258 ≠ 0 sets pvar+0x230 = 1. pvar+0x230 = 0: **hidden** (mode 1, 0x1000 cleared), speed 0, **z − 20** (state 0xc/0xd). Else targetable, and the first update blends to **sequence 1 over 5 ticks** |
/// | 577 | 0x2efc60 | pvar+0x20a (s16) ≠ 0: **sequence 4, blend 0 ticks** (lands on the next advance). Else **z + 6** (hover state 0xe; the update keeps it at ground + 6) |
/// | 666 | 0x2f4960 | pvar+0x170 = −1: **sequence 5, blend 0** (re-applied every frame). Else pvar+0x174 (s16) = 0: **hidden + frozen** |
/// | 730, 790 | 0x2fad68 | **hidden + frozen**, z + pvar f32[0] (it descends from there when triggered) |
/// | 750 | 0x2fbf80 | pvar s32[0] = 0: **hidden + frozen**. pvar+0x50 = −1 and pvar+0x70 ≠ −1: moves to point 0 of spline pvar+0x70 |
/// | 1818 | 0x30df40 | moves to the position of moby pvar+0x48 (a moby link) + 0.02 z; link −1: **deleted** |
/// | 459, 604, 1818 | | force sequence 0 when seq B ≠ 0: never at spawn (seq B is 0), so no change |
pub fn initial_state(o_class: i32, inst: &MobyInstance, pvar: Option<&[u8]>, class: &MobyAnimClass, env: &dyn SpawnEnv) -> SpawnState {
    let mut st = SpawnState::plain(o_class, class);
    let pos = inst.position;
    match o_class {
        459 => {
            // FUN_002e6bf0 case 0.
            let (Some(path), Some(alt)) = (pv_i32(pvar, 0x164), pv_i32(pvar, 0x134)) else { return st };
            if path == -1 { return st; }
            let (index, raise) = if alt < 0 { (path, pv_i32(pvar, 0x1b4) == Some(0)) } else { (alt, false) };
            let Some(p0) = env.spline(index).and_then(|s| s.first()) else { return st };
            let mut p = [p0[0], p0[1], p0[2]];
            if raise { p[2] = fadd(p[2], 20.0); }
            st.position = Some(p);
            st.mode_bits |= 0x1000;
            let ground = env.ground_height(p);
            if f32::from_bits(ps2::sub(p[2].to_bits(), ground.to_bits())) > 0.5 { st.hide_and_freeze(); }
        }
        572 | 865 | 866 => {
            // FUN_002edca0 case 0 (then case 1 on the first active frame).
            let big = pv_i32(pvar, 0x258).is_some_and(|v| v != 0) || pv_i32(pvar, 0x230).is_some_and(|v| v != 0);
            if big {
                st.mode_bits |= 0x1000;
                st.change(ChangeAt::FirstUpdate, 1, 0, Some(5));
            } else if pvar.is_some() {
                st.hidden = true;
                st.mode_bits |= 1;
                st.mode_clear |= 0x1000;
                st.speed = Some(0.0);
                st.position = Some([pos[0], pos[1], f32::from_bits(ps2::sub(pos[2].to_bits(), 20f32.to_bits()))]);
            }
        }
        577 => {
            // FUN_002efc60 case 0.
            match pv_i16(pvar, 0x20a) {
                Some(0) => st.position = Some([pos[0], pos[1], fadd(pos[2], 6.0)]),
                Some(_) => st.change(ChangeAt::LoadPass, 4, 0, Some(0)),
                None => {}
            }
        }
        666 => {
            // FUN_002f4960: the pvar+0x170 = −1 branch runs before the state machine, every frame.
            if pv_i32(pvar, 0x170) == Some(-1) {
                st.change(ChangeAt::LoadPass, 5, 0, Some(0));
            } else if pv_i16(pvar, 0x174) == Some(0) {
                st.hide_and_freeze();
            }
        }
        730 | 790 => {
            // FUN_002fad68 case 0.
            if let Some(lift) = pv_f32(pvar, 0) {
                st.position = Some([pos[0], pos[1], fadd(pos[2], lift)]);
                st.hide_and_freeze();
            }
        }
        750 => {
            // FUN_002fbf80 case 0.
            if pv_i32(pvar, 0) == Some(0) { st.hide_and_freeze(); }
            if pv_i32(pvar, 0x50) == Some(-1) {
                if let Some(path) = pv_i32(pvar, 0x70).filter(|&v| v != -1) {
                    if let Some(p0) = env.spline(path).and_then(|s| s.first()) { st.position = Some([p0[0], p0[1], p0[2]]); }
                }
            }
        }
        1818 => {
            // FUN_0030df40 case 0 (the save-bit "already completed" deletion never fires on a new game).
            match pv_i32(pvar, 0x48) {
                Some(-1) => { st.deleted = true; st.hidden = true; }
                Some(link) => {
                    if let Some(p) = env.moby_position(link) { st.position = Some([p[0], p[1], fadd(p[2], 0.02)]); }
                }
                None => {}
            }
        }
        _ => {}
    }
    st
}

/// The loader's ship (`FUN_00255958`): `CreateMoby(SHIP_CLASSES[i])` at the level-settings position
/// ([`crate::gameplay::ship_placement`]), update = `FUN_002a1c40` (engine-wide ship code), draw distance
/// 0xff, update distance 0x10, mode bit 2 cleared, then `fun_00212ed8(ship, 1, 0)`: **sequence 1 by hard
/// cut** before the load pass, which then advances it once. `CreateMoby`'s `InitMobyInstance` leaves the
/// render fields at their defaults: light word 0, ambient 0x40 per channel, occlusion word 0x7f80 (always
/// visible; the loader's occlusion resolution skips 0x7f80), scale = class scale.
///
/// `hidden` ([`ship_hidden_on_arrival`]): the mission NPC's `FUN_002a2450` hides it (mode |= 3, +0x94 = 0)
/// from the first gameplay tick on; the port applies that at creation, so the ship is neither drawn nor run
/// (the game's one load-pass `MobyAnimAdvance` and tick-0 update of the ship change nothing visible).
pub fn ship_state(o_class: i32, class: &MobyAnimClass, hidden: bool) -> SpawnState {
    let mut st = SpawnState::plain(o_class, class);
    st.change(ChangeAt::Create, 1, 0, None);
    if hidden {
        st.hidden = true;
        st.mode_bits |= 3;
        st.runs_load_pass = false;
    } else {
        st.mode_clear |= 2;
    }
    st
}

// ---------------------------------------------------------------------------------------------------
// The loader's spawn test (`FUN_00255958`, the instance loop at 0x256890..0x256a58, level01)

/// The save state the spawn test reads, for one level (the level being loaded, `L` = 0x15ed84). The default
/// is a first visit on a new game: every byte zero (as `rc_game::game_state` leaves a direct boot, and as the
/// Novalis savestate shows at 0x15fc88 / 0x14c190 + 0x100 / 0x1ba950 / 0x1bbb04).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnSave {
    /// `0x15fc88[16]`: the level's mission bytes, copied from the save (`0x14c050 + L·16`, chunk 3004) by
    /// `LoadLevelCoreData` 0x258128 before the loader runs; 0xff = mission done.
    pub missions: [u8; 16],
    /// `0x14c190 + L·0x100` (chunk 3005): the persistent killed bits, bit `id & 31` of word `id >> 5`
    /// (little-endian bytes: byte `id >> 3`, bit `id & 7`).
    pub killed: [u8; 0x100],
    /// `0x1ba950`: the death bits of this visit, same layout (words from `0x1ba950`), as spawn ids.
    pub visit_death: std::collections::BTreeSet<i32>,
    /// `0x1bbb04[id]` (byte per spawn id): non-zero = never spawn again (a collected placed bolt writes
    /// `mission + 2`).
    pub id_flags: std::collections::BTreeMap<i32, u8>,
    /// `0x14d590 + L·0x100 + k·4` (s16, chunk 3006 `first`): the 64 spawner slots `FUN_0029ab50` hands out to
    /// instances with spawn flag 0x10 (slot value = spawn id + 1, 0 = free).
    pub spawner: [i16; 64],
}

impl Default for SpawnSave {
    fn default() -> Self {
        SpawnSave { missions: [0; 16], killed: [0; 0x100], visit_death: Default::default(), id_flags: Default::default(), spawner: [0; 64] }
    }
}

impl SpawnSave {
    fn killed_bit(&self, id: i32) -> bool {
        usize::try_from(id).ok().and_then(|i| self.killed.get(i >> 3)).is_some_and(|b| b >> (id & 7) & 1 != 0)
    }
    /// `FUN_0029ab50(L, id)`: the spawner slot of `id` (the slot already holding `id + 1`, else the highest
    /// free one, which then takes `id + 1`), scanning 63 down to 0; −1 when `id < 0` or every slot is taken.
    pub fn spawner_slot(&mut self, id: i32) -> i32 {
        if id < 0 { return -1; }
        let v = (id + 1) as i16;
        for k in (0..64).rev() {
            if self.spawner[k] == v || self.spawner[k] == 0 {
                self.spawner[k] = v;
                return k as i32;
            }
        }
        -1
    }
}

/// The loader's result for one instance record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnTest {
    /// The loader creates the moby (else `0x1acc00[i] = −1`: not created on this load).
    pub spawn: bool,
    /// moby+0xb1: 0xfe (no spawn flags), 0xff (flags without 0x10), else the spawner slot (flag 0x10).
    pub b1: u8,
    /// moby+0xb4: record +0x10, or record +0x14 on the "mission done" branch, halved (`(v + 1) / 2`, C
    /// division) when that branch's death bit is set.
    pub b4: i16,
    /// moby+0xb6: record +0x10, or record +0x14 on the "mission done" branch.
    pub b6: i16,
}

/// The spawn test of one record (`flags` = record +0x08, `id` = +0x0c, `m` = +0x04). Nothing is tested when
/// `flags == 0`. Otherwise, in the code's order:
/// 1. `b1` = the spawner slot (`FUN_0029ab50`, flag 0x10) or 0xff;
/// 2. `0x1bbb04[id] != 0` → not created;
/// 3. flags & 3 ≠ 0 (mission-gated): if `0x15fc88[m] == 0xff` (mission done) the record needs flag 2, and
///    b4/b6 come from +0x14 (halved when this visit's death bit 0x1ba950 is set); else it needs flag 1 and b4
///    is +0x10 halved when the persistent killed bit (0x14c190 + L·0x100) is set. A record without the needed
///    flag is not created (so flag 2 alone = "only once mission m is done", flag 1 alone = "only before");
/// 4. else flag 8: not created when this visit's death bit is set;
/// 5. else flags & 0xc == 4: not created when the persistent killed bit is set;
/// 6. else created.
///
/// `m` is read as the record's s32 (−1 would index 0x15fc87; no mission-gated record has it on the disc).
pub fn spawn_test(inst: &MobyInstance, save: &mut SpawnSave) -> SpawnTest {
    let (flags, id) = (inst.spawn_flags as u32, inst.spawn_id);
    let (r10, r14) = (inst.unknown_10, inst.unknown_14);
    let mut t = SpawnTest { spawn: true, b1: 0xfe, b4: r10 as i16, b6: r10 as i16 };
    if flags == 0 { return t; }
    t.b1 = if flags & 0x10 != 0 { save.spawner_slot(id) as u8 } else { 0xff };
    let halve = |v: i32| ((v + 1) / 2) as i16;
    let flagged = usize::try_from(id).ok().is_some_and(|_| save.id_flags.get(&id).is_some_and(|&f| f != 0));
    if flagged {
        t.spawn = false;
    } else if flags & 3 != 0 {
        let done = usize::try_from(inst.unknown_4).ok().and_then(|m| save.missions.get(m)).is_some_and(|&b| b == 0xff);
        if done {
            if flags & 2 == 0 {
                t.spawn = false;
            } else {
                t.b6 = r14 as i16;
                t.b4 = if save.visit_death.contains(&id) { halve(r14) } else { r14 as i16 };
            }
        } else if flags & 1 == 0 {
            t.spawn = false;
        } else if save.killed_bit(id) {
            t.b4 = halve(r10);
        }
    } else if flags & 8 != 0 {
        t.spawn = !save.visit_death.contains(&id);
    } else if flags & 0xc == 4 {
        t.spawn = !save.killed_bit(id);
    }
    t
}

/// [`spawn_test`] over the level's records in order (the spawner slots are handed out in that order).
pub fn loader_spawns(instances: &[MobyInstance], save: &mut SpawnSave) -> Vec<SpawnTest> {
    instances.iter().map(|i| spawn_test(i, save)).collect()
}

/// `0x1acc00`: instance index → runtime moby index (the number of created instances before it), `None` for
/// an instance the loader did not create. The loader applies it to the group lists (gameplay +0x48) and the
/// pvar moby links (gameplay +0x50, [`crate::gameplay::parse_pvars_spawned`]).
pub fn instance_to_moby(tests: &[SpawnTest]) -> Vec<Option<usize>> {
    let mut next = 0;
    tests.iter().map(|t| t.spawn.then(|| { next += 1; next - 1 })).collect()
}

/// The mission NPC classes (update `MissionNpcUpdate` 0x2fad68).
pub const MISSION_NPC_CLASSES: [i32; 2] = [730, 790];

/// The ship on arrival. `MissionNpcUpdate` 0x2fad68 state 1 (every tick from the first gameplay tick; its
/// state 0 in the load pass sets update distance 0xff, so it is always in the run list): when its mission
/// byte `0x14c050[L·16 + moby+0xb0]` is not 0xff (not done), it calls `FUN_002a2450`: ship (0x13e030) mode
/// |= 3 (hidden, no update), +0x94 = 0 (no collision). Its state 8 (mission done) calls `FUN_002a2480`
/// (mode &= ~3, collision back). So the ship is hidden while a created mission NPC's mission is open: the
/// first arrival on Novalis (the crash: NPC 730/790, mission 0), and shown on a revisit once it is done, and
/// on every level without such an NPC.
pub fn ship_hidden_on_arrival(instances: &[MobyInstance], tests: &[SpawnTest], level_missions: &[u8; 16]) -> bool {
    instances.iter().zip(tests).any(|(i, t)| {
        t.spawn && MISSION_NPC_CLASSES.contains(&i.o_class) && level_missions.get(i.unknown_4 as u8 as usize).is_none_or(|&b| b != 0xff)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_anim::{MobyFrameHeader, MobySequence, MobySequenceHeader};

    struct Env {
        splines: Vec<Vec<[f32; 4]>>,
        ground: f32,
        mobys: Vec<[f32; 3]>,
    }
    impl SpawnEnv for Env {
        fn spline(&self, index: i32) -> Option<&[[f32; 4]]> { self.splines.get(usize::try_from(index).ok()?).map(|v| v.as_slice()) }
        fn ground_height(&self, _pos: [f32; 3]) -> f32 { self.ground }
        fn moby_position(&self, index: i32) -> Option<[f32; 3]> { self.mobys.get(usize::try_from(index).ok()?).copied() }
    }
    fn env() -> Env { Env { splines: vec![vec![[10.0, 20.0, 30.0, 1.0]], vec![[1.0, 2.0, 3.0, -1.0], [4.0, 5.0, 6.0, 1.0]]], ground: 30.0, mobys: vec![[7.0, 8.0, 9.0]] } }

    /// A one-joint class with `n` sequences of 4 identity keys (rate 0.25; sequence k's key rate 0.25 + k/8).
    fn class(n: usize) -> MobyAnimClass {
        let key = |rate: f32| {
            let payload = vec![0, 0, 0, 0, 0, 0, 0xff, 0x7f, 0, 0, 0, 0, 0, 0, 0, 0];
            MobyFrame {
                header: MobyFrameHeader { rate, time: 0, qwc: 1, quat_bytes: 8, scale_count: 0, trans_offset: 8, trans_count: 0 },
                quats: vec![[0, 0, 0, 0x7fff]],
                scales: vec![],
                trans: vec![],
                payload,
            }
        };
        let seqs = (0..n)
            .map(|k| Some(MobySequence { header: MobySequenceHeader { frame_count: 4, loop_sound: 0xff, ..Default::default() }, frames: vec![key(0.25 + k as f32 / 8.0); 4], triggers: vec![] }))
            .collect();
        MobyAnimClass { joint_count: 1, skeleton: vec![moby_anim::IDENTITY], rest: vec![[0.0; 3]], parent_word: vec![0], sequences: seqs }
    }
    fn inst(o_class: i32, pos: [f32; 3]) -> MobyInstance { MobyInstance { o_class, position: pos, pvar_index: 0, ..Default::default() } }
    fn pvar(len: usize, fields: &[(usize, &[u8])]) -> Vec<u8> {
        let mut p = vec![0u8; len];
        for (o, b) in fields { p[*o..*o + b.len()].copy_from_slice(b); }
        p
    }
    fn le(v: i32) -> [u8; 4] { v.to_le_bytes() }

    #[test]
    fn plain_classes_play_sequence_0_advanced_once() {
        let c = class(1);
        let st = initial_state(500, &inst(500, [1.0; 3]), None, &c, &env());
        assert_eq!(st, SpawnState::plain(500, &c));
        let (s, snap) = st.anim_after_load(&c);
        assert!(snap.is_none());
        assert_eq!((s.seq_a, s.frame_a, s.seq_b, s.frame_b, s.t, s.rate), (0, 0, 0, 1, 0.0, 0.25));
        // Ratchet is not run by the load pass.
        let (s, _) = initial_state(0, &inst(0, [1.0; 3]), None, &c, &env()).anim_after_load(&c);
        assert_eq!(s, AnimState::spawn(&c));
    }

    #[test]
    fn enemy_459_moves_to_its_path_and_hides_in_the_air() {
        let c = class(1);
        // pvar+0x134 < 0, +0x1b4 = 0: spline 0x164 = 0, raised 20 → 50 above ground 30: hidden.
        let p = pvar(0x200, &[(0x164, &le(0)), (0x134, &le(-1)), (0x1b4, &le(0))]);
        let st = initial_state(459, &inst(459, [0.0; 3]), Some(&p), &c, &env());
        assert_eq!(st.position, Some([10.0, 20.0, 50.0]));
        assert!(st.hidden && st.mode_bits & 0x1041 == 0x1041);
        assert!(st.anim_after_load(&c).0.skip_advance);
        // +0x1b4 = 1: not raised; 30 − 30 = 0 ≤ 0.5: visible.
        let p = pvar(0x200, &[(0x164, &le(0)), (0x134, &le(-1)), (0x1b4, &le(1))]);
        let st = initial_state(459, &inst(459, [0.0; 3]), Some(&p), &c, &env());
        assert_eq!((st.position, st.hidden), (Some([10.0, 20.0, 30.0]), false));
        // pvar+0x134 ≥ 0 wins (spline 1, never raised); 3 − 30 < 0.5: visible.
        let p = pvar(0x200, &[(0x164, &le(0)), (0x134, &le(1)), (0x1b4, &le(0))]);
        let st = initial_state(459, &inst(459, [0.0; 3]), Some(&p), &c, &env());
        assert_eq!((st.position, st.hidden), (Some([1.0, 2.0, 3.0]), false));
        // Just above the 0.5 threshold; no ground hit (0.0) at z 0.4 is not airborne.
        let e = Env { ground: 29.4, ..env() };
        let p = pvar(0x200, &[(0x164, &le(0)), (0x134, &le(-1)), (0x1b4, &le(1))]);
        assert!(initial_state(459, &inst(459, [0.0; 3]), Some(&p), &c, &e).hidden);
        let e = Env { ground: 0.0, splines: vec![vec![[1.0, 1.0, 0.4, 0.0]]], ..env() };
        assert!(!initial_state(459, &inst(459, [0.0; 3]), Some(&p), &c, &e).hidden);
        // No path: untouched.
        let p = pvar(0x200, &[(0x164, &le(-1)), (0x134, &le(-1))]);
        assert_eq!(initial_state(459, &inst(459, [0.0; 3]), Some(&p), &c, &env()), SpawnState::plain(459, &c));
    }

    #[test]
    fn amoeboids_big_walk_small_hide_underground() {
        let c = class(2);
        let big = pvar(0x270, &[(0x230, &le(1)), (0x25c, &le(-1))]);
        let st = initial_state(572, &inst(572, [5.0, 5.0, 40.0]), Some(&big), &c, &env());
        assert!(!st.hidden && st.position.is_none() && st.mode_bits & 0x1000 != 0);
        assert_eq!((st.change_at, st.sequence, st.blend_ticks), (Some(ChangeAt::FirstUpdate), 1, Some(5)));
        // The load pass leaves sequence 0; the first update blends (t = 0.25 after that frame's advance:
        // snapshot) and lands on sequence 1 five ticks later.
        let (mut s, mut snap) = st.anim_after_load(&c);
        assert_eq!(s.seq_b, 0);
        moby_anim::advance(&mut s, &c);
        assert!(st.apply_change(&mut s, &c, &mut snap));
        assert_eq!((s.seq_a, s.seq_b, s.rate.to_bits()), (moby_anim::SNAPSHOT_SEQ, 1, ps2::div(ps2::ONE, 5f32.to_bits())));
        assert!(snap.is_some());
        assert!(!st.apply_change(&mut s, &c, &mut snap), "seq B is already 1");
        // pvar+0x258 ≠ 0 makes a big one too.
        let big2 = pvar(0x270, &[(0x258, &le(1))]);
        assert!(!initial_state(866, &inst(866, [5.0; 3]), Some(&big2), &c, &env()).hidden);
        // Small: hidden, speed 0, 20 lower.
        let small = pvar(0x270, &[(0x25c, &le(-1))]);
        let st = initial_state(866, &inst(866, [5.0, 5.0, 40.0]), Some(&small), &c, &env());
        assert!(st.hidden && st.mode_bits & 1 != 0 && st.mode_clear & 0x1000 != 0);
        assert_eq!((st.position, st.speed, st.change_at), (Some([5.0, 5.0, 20.0]), Some(0.0), None));
        assert_eq!(st.anim_after_load(&c).0.speed, 0.0);
    }

    #[test]
    fn critter_577_sequence_4_or_hover() {
        let c = class(5);
        let p = pvar(0x270, &[(0x20a, &1i16.to_le_bytes())]);
        let st = initial_state(577, &inst(577, [1.0, 1.0, 10.0]), Some(&p), &c, &env());
        assert_eq!((st.change_at, st.sequence, st.blend_ticks, st.position), (Some(ChangeAt::LoadPass), 4, Some(0), None));
        let (mut s, _) = st.anim_after_load(&c);
        assert_eq!((s.seq_a, s.seq_b, s.frame_b, s.t, s.rate.to_bits()), (0, 4, 0, 0.0, ps2::MAX));
        moby_anim::advance(&mut s, &c); // first frame: on sequence 4, one tick into its first key
        assert_eq!((s.seq_a, s.frame_a, s.frame_b, s.t), (4, 0, 1, 0.75));
        let p = pvar(0x270, &[]);
        let st = initial_state(577, &inst(577, [1.0, 1.0, 10.0]), Some(&p), &c, &env());
        assert_eq!((st.position, st.change_at, st.hidden), (Some([1.0, 1.0, 16.0]), None, false));
    }

    #[test]
    fn dropship_npcs_infobot_mouse() {
        let c = class(6);
        let p = pvar(0x1a0, &[(0x170, &le(-1))]);
        let st = initial_state(666, &inst(666, [0.0; 3]), Some(&p), &c, &env());
        assert_eq!((st.hidden, st.sequence, st.blend_ticks), (false, 5, Some(0)));
        let p = pvar(0x1a0, &[(0x170, &le(27))]);
        assert!(initial_state(666, &inst(666, [0.0; 3]), Some(&p), &c, &env()).hidden);
        let p = pvar(0x1a0, &[(0x170, &le(27)), (0x174, &1i16.to_le_bytes())]);
        assert!(!initial_state(666, &inst(666, [0.0; 3]), Some(&p), &c, &env()).hidden);

        let p = pvar(0x40, &[(0, &30f32.to_le_bytes())]);
        let st = initial_state(730, &inst(730, [1.0, 2.0, 40.5]), Some(&p), &c, &env());
        assert!(st.hidden && st.mode_bits & 0x41 == 0x41);
        assert_eq!(st.position, Some([1.0, 2.0, 70.5]));

        let p = pvar(0xd0, &[(0x50, &le(-1)), (0x70, &le(-1))]);
        assert!(initial_state(750, &inst(750, [0.0; 3]), Some(&p), &c, &env()).hidden);
        let p = pvar(0xd0, &[(0, &le(1)), (0x50, &le(-1)), (0x70, &le(1))]);
        let st = initial_state(750, &inst(750, [0.0; 3]), Some(&p), &c, &env());
        assert_eq!((st.hidden, st.position), (false, Some([1.0, 2.0, 3.0])));

        let p = pvar(0x60, &[(0x48, &le(0))]);
        let st = initial_state(1818, &inst(1818, [0.0; 3]), Some(&p), &c, &env());
        assert_eq!(st.position, Some([7.0, 8.0, fadd(9.0, 0.02)]));
        let p = pvar(0x60, &[(0x48, &le(-1))]);
        assert!(initial_state(1818, &inst(1818, [0.0; 3]), Some(&p), &c, &env()).deleted);
    }

    #[test]
    fn ship_index_follows_the_destination_planet() {
        let idx: Vec<usize> = (0..19).map(first_visit_ship).collect();
        assert_eq!(idx, [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2]);
        assert_eq!((0..3).map(|i| (SHIP_CLASSES[i], spaceships_entry(i))).collect::<Vec<_>>(), [(531, 1), (532, 2), (533, 3)]);
    }

    #[test]
    fn spaceship_file_layout() {
        // Header, class @0x40, extra @0x80, texture list @0xc0 (256x256), extra list after it (128x128).
        let pif = |t: &mut Vec<u8>, size: u32| {
            let at = t.len();
            t.resize(at + 0x430 + (size * size) as usize, 0);
            t[at..at + 8].copy_from_slice(&[1, 0, 0, 0, 0x10, 0, 0, 0]);
            t[at + 0x10..at + 0x14].copy_from_slice(b"2FIP");
            for (o, v) in [(0x18, size), (0x1c, size), (0x20, 0x13)] { t[at + o..at + o + 4].copy_from_slice(&v.to_le_bytes()); }
            // CLUT entry 8 (CSM1 slot 16) = opaque red; pixel (1, 0) uses index 8.
            t[at + 0x30 + 16 * 4..at + 0x30 + 17 * 4].copy_from_slice(&[255, 0, 0, 0x80]);
            t[at + 0x431] = 8;
            at
        };
        let mut f = vec![0u8; 0xc0];
        let tex = pif(&mut f, 256);
        let extra_tex = pif(&mut f, 128);
        for (k, v) in [0x40u32, tex as u32, 0x80, extra_tex as u32].iter().enumerate() { f[k * 4..k * 4 + 4].copy_from_slice(&v.to_le_bytes()); }
        let s = parse_spaceship(&f).unwrap();
        assert_eq!((s.class.len(), s.extra_class.len()), (0x40, 0x40));
        assert_eq!((s.texture.width, s.extra_texture.width), (256, 128));
        assert_eq!(&s.texture.rgba[4..8], &[255, 0, 0, 255]);
        assert_eq!(&s.extra_texture.rgba[4..8], &[255, 0, 0, 255]);
        f[0xc0 + 0x19] = 0; // width 0: not the fixed layout
        assert!(parse_spaceship(&f).is_err());
    }

    #[test]
    fn ship_is_cut_to_sequence_1_then_advanced() {
        let c = class(3);
        let st = ship_state(531, &c, false);
        assert_eq!((st.change_at, st.sequence, st.blend_ticks, st.mode_clear), (Some(ChangeAt::Create), 1, None, 2));
        let (s, _) = st.anim_after_load(&c);
        // Cut: (1,0) -> (1,1), rate 0.375; the load pass advances t by 0.375.
        assert_eq!((s.seq_a, s.frame_a, s.seq_b, s.frame_b, s.t, s.rate), (1, 0, 1, 1, 0.375, 0.375));
        // Hidden by the mission NPC: mode 3, not run.
        let st = ship_state(531, &c, true);
        assert_eq!((st.hidden, st.mode_bits & 3, st.runs_load_pass), (true, 3, false));
    }

    fn rec(flags: i32, mission: i32, id: i32) -> MobyInstance {
        MobyInstance { spawn_flags: flags, unknown_4: mission, spawn_id: id, unknown_10: 7, unknown_14: 9, ..Default::default() }
    }

    #[test]
    fn spawn_test_follows_the_loader_branches() {
        let mut s = SpawnSave::default();
        let t = |r: MobyInstance, s: &mut SpawnSave| { let t = spawn_test(&r, s); (t.spawn, t.b1, t.b4, t.b6) };
        // No flags: always, b1 0xfe.
        assert_eq!(t(rec(0, -1, 5), &mut s), (true, 0xfe, 7, 7));
        // Mission-gated, mission 2 open (0): flag 1 spawns, flag 2 alone does not (Novalis 577s), 3 spawns.
        assert_eq!(t(rec(1, 2, 5), &mut s), (true, 0xff, 7, 7));
        assert_eq!(t(rec(2, 2, 5), &mut s), (false, 0xff, 7, 7));
        assert_eq!(t(rec(3, 2, 5), &mut s), (true, 0xff, 7, 7));
        // Mission 2 done: flag 2 spawns with +0x14; flag 1 alone does not.
        s.missions[2] = 0xff;
        assert_eq!(t(rec(2, 2, 5), &mut s), (true, 0xff, 9, 9));
        assert_eq!(t(rec(1, 2, 5), &mut s), (false, 0xff, 7, 7));
        s.visit_death.insert(5);
        assert_eq!(t(rec(2, 2, 5), &mut s).2, 5, "(9 + 1) / 2");
        // Flag 8: this visit's death bit; flags & 0xc == 4: the persistent killed bit.
        assert!(!t(rec(8, -1, 5), &mut s).0);
        assert!(t(rec(8, -1, 6), &mut s).0);
        assert!(t(rec(4, -1, 9), &mut s).0);
        s.killed[1] = 1 << 1; // id 9
        assert!(!t(rec(4, -1, 9), &mut s).0);
        assert!(t(rec(12, -1, 9), &mut s).0, "flag 8 wins over 4");
        s.missions[2] = 0;
        assert_eq!(t(rec(1, 2, 9), &mut s), (true, 0xff, 4, 7), "open mission, killed: b4 halved");
        // 0x1bbb04: never again (bolts, flags 0x20).
        s.id_flags.insert(11, 2);
        assert!(!t(rec(0x20, -1, 11), &mut s).0);
        assert!(t(rec(0x20, -1, 12), &mut s).0);
        // Flag 0x10: spawner slots from 63 down, the same id keeps its slot.
        assert_eq!(t(rec(0x10, -1, 3), &mut s).1, 63);
        assert_eq!(t(rec(0x10, -1, 4), &mut s).1, 62);
        assert_eq!(t(rec(0x10, -1, 3), &mut s).1, 63);
        // Map: created records get consecutive indices.
        let tests = [true, false, true, false, false, true].map(|spawn| SpawnTest { spawn, b1: 0, b4: 0, b6: 0 });
        assert_eq!(instance_to_moby(&tests), [Some(0), None, Some(1), None, None, Some(2)]);
    }

    #[test]
    fn ship_is_hidden_while_the_mission_npc_mission_is_open() {
        let recs = [MobyInstance { o_class: 730, ..Default::default() }, MobyInstance { o_class: 500, ..Default::default() }];
        let on = [SpawnTest { spawn: true, b1: 0xfe, b4: 0, b6: 0 }; 2];
        let mut m = [0u8; 16];
        assert!(ship_hidden_on_arrival(&recs, &on, &m));
        m[0] = 0xff;
        assert!(!ship_hidden_on_arrival(&recs, &on, &m), "revisit after the mission");
        assert!(!ship_hidden_on_arrival(&recs[1..], &on[1..], &[0; 16]), "no NPC");
    }
}
