//! Sound data: the 989snd bank file (`sound_bank` lump), the game's `SoundDef` records and the core-index
//! bank-id remap, the level header's music table, and the gameplay sections the sound code reads (sound
//! instances, env sample points). Spec: docs/plan/audio.md §2 (bank, defs, remap), §3.3 (sound instances),
//! §4 (music). Snapshot-tested in `tests/formats/golden.rs` (sound test).
//!
//! Bank file: `u32 type = 3, u32 nchunks = 2, {u32 off, u32 size}[2]`. Chunk 0 is an `SBlk` SFX block
//! (version 1: 12-byte sound records, 0x28-byte grains), chunk 1 the raw SPU ADPCM the tones point into.

use crate::buf::{invalid, Buf, Result};
use crate::level::LevelCore;
use crate::vag::{self, SampleExtent};
use bytemuck::{Pod, Zeroable};

/// `SBlk` header (block-relative). The header length varies (0x3c on levels, 0x34 on the global bank):
/// always locate the sounds through `first_sound`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SfxHeader {
    pub version: u32,
    /// 0x100 names, 0x200 user data (neither is set on the disc).
    pub flags: u32,
    /// Four characters ("DAW\0" on Novalis) or 0.
    pub bank_id: u32,
    pub bank_num: i8,
    pub n_sounds: i16,
    pub n_grains: i16,
    pub n_vags: i16,
    /// Block-relative byte offset of the sound records.
    pub first_sound: u32,
    /// Block-relative byte offset of the grain records.
    pub first_grain: u32,
    pub vags_in_sr: u32,
    pub vag_data_size: u32,
    pub sram_alloc_size: u32,
    pub next_block: u32,
}

/// One sound (12 bytes at `first_sound + 12·i`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sound {
    /// 0..127, the 989snd "sfx volume" (`play vol = sfx.vol · vol >> 10`).
    pub vol: i8,
    /// 989snd volume group (0 or 4 on the levels).
    pub vol_group: i8,
    pub pan: i16,
    pub instance_limit: i8,
    /// Bit 0 = looped sound; 2 solo, 8/0x10/0x20 instance limit modes (989snd `SFXFlags`).
    pub flags: u16,
    /// Byte offset of the first grain, relative to `first_grain`.
    pub first_grain: u32,
    /// Index range into [`Bank::grains`].
    pub grains: std::ops::Range<usize>,
}

impl Sound {
    pub fn looped(&self) -> bool { self.flags & 1 != 0 }
}

/// 989snd grain types (OpenGOAL `GrainType`; RAC1 uses 1, 4 and 20..43).
pub mod grain {
    pub const NULL: u32 = 0;
    pub const TONE: u32 = 1;
    pub const XREF_ID: u32 = 2;
    pub const XREF_NUM: u32 = 3;
    pub const LFO_SETTINGS: u32 = 4;
    pub const START_CHILD_SOUND: u32 = 5;
    pub const STOP_CHILD_SOUND: u32 = 6;
    pub const PLUGIN_MESSAGE: u32 = 7;
    pub const BRANCH: u32 = 8;
    pub const TONE2: u32 = 9;
    pub const CONTROL_NULL: u32 = 20;
    pub const LOOP_START: u32 = 21;
    pub const LOOP_END: u32 = 22;
    pub const LOOP_CONTINUE: u32 = 23;
    pub const STOP: u32 = 24;
    pub const RAND_PLAY: u32 = 25;
    pub const RAND_DELAY: u32 = 26;
    pub const RAND_PB: u32 = 27;
    pub const PB: u32 = 28;
    pub const ADD_PB: u32 = 29;
    pub const SET_REGISTER: u32 = 30;
    pub const SET_REGISTER_RAND: u32 = 31;
    pub const INC_REGISTER: u32 = 32;
    pub const DEC_REGISTER: u32 = 33;
    pub const TEST_REGISTER: u32 = 34;
    pub const MARKER: u32 = 35;
    pub const GOTO_MARKER: u32 = 36;
    pub const GOTO_RANDOM_MARKER: u32 = 37;
    pub const WAIT_FOR_ALL_VOICES: u32 = 38;
    pub const PLAY_CYCLE: u32 = 39;
    pub const ADD_REGISTER: u32 = 40;
    pub const KEY_OFF_VOICES: u32 = 41;
    pub const KILL_VOICES: u32 = 42;
    pub const ON_STOP_MARKER: u32 = 43;
    pub const COPY_REGISTER: u32 = 44;
}

/// Grain record, version 1 (0x28 bytes): `u32 type, s32 delay (240 Hz ticks), 32 bytes of parameters`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grain {
    pub kind: u32,
    pub delay: i32,
    pub data: [u8; 32],
}

/// Tone parameters (grain types 1 and 9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tone {
    /// Voice-steal priority (90, 100 or 0 on Novalis).
    pub priority: i8,
    /// 0..127; −1..−4 = sound register, −5 = random, ≤ −6 = global register.
    pub vol: i8,
    /// Negative = PS2-rate sample (use −note); non-negative = PS1-style, scaled by 44100/48000.
    pub center_note: i8,
    pub center_fine: i8,
    /// Degrees; negative values select registers like `vol`.
    pub pan: i16,
    pub map_low: i8,
    pub map_high: i8,
    /// Pitch-bend range in semitones (down, up).
    pub pb_low: i8,
    pub pb_high: i8,
    pub adsr1: u16,
    pub adsr2: u16,
    /// 1 = to reverb, 8 = noise, 0x10 = reverb only.
    pub flags: u16,
    /// Byte offset into the sample chunk.
    pub sample_offset: u32,
    pub reserved: u32,
}

/// LFO settings (grain type 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LfoParams {
    pub which_lfo: u8,
    pub target: u8,
    pub target_extra: u8,
    pub shape: u8,
    pub duty_cycle: u16,
    pub depth: u16,
    pub flags: u16,
    pub start_offset: u16,
    pub step_size: u32,
}

/// Child-sound / branch parameters (grain types 5, 6, 8).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlaySoundParams {
    pub vol: i32,
    pub pan: i32,
    pub reg_settings: [i8; 4],
    pub sound_id: i32,
}

impl Grain {
    fn u16(&self, o: usize) -> u16 { u16::from_le_bytes([self.data[o], self.data[o + 1]]) }
    fn u32(&self, o: usize) -> u32 { u32::from_le_bytes(self.data[o..o + 4].try_into().unwrap()) }

    pub fn is_tone(&self) -> bool { self.kind == grain::TONE || self.kind == grain::TONE2 }

    /// The tone parameters (meaningful for types 1/9 only).
    pub fn tone(&self) -> Tone {
        let d = &self.data;
        Tone {
            priority: d[0] as i8,
            vol: d[1] as i8,
            center_note: d[2] as i8,
            center_fine: d[3] as i8,
            pan: self.u16(4) as i16,
            map_low: d[6] as i8,
            map_high: d[7] as i8,
            pb_low: d[8] as i8,
            pb_high: d[9] as i8,
            adsr1: self.u16(10),
            adsr2: self.u16(12),
            flags: self.u16(14),
            sample_offset: self.u32(16),
            reserved: self.u32(20),
        }
    }

    /// Control grains (20..44): `s16 param[4]` (v1 grains store them as 16-bit values).
    pub fn params(&self) -> [i16; 4] { [0, 2, 4, 6].map(|o| self.u16(o) as i16) }

    /// RAND_DELAY (26): the v1 grain stores the modulus directly (`rand() % amount`).
    pub fn rand_delay_amount(&self) -> i32 { self.u32(0) as i32 }

    pub fn lfo(&self) -> LfoParams {
        let d = &self.data;
        LfoParams {
            which_lfo: d[0],
            target: d[1],
            target_extra: d[2],
            shape: d[3],
            duty_cycle: self.u16(4),
            depth: self.u16(6),
            flags: self.u16(8),
            start_offset: self.u16(10),
            step_size: self.u32(12),
        }
    }

    pub fn play_sound(&self) -> PlaySoundParams {
        PlaySoundParams {
            vol: self.u32(0) as i32,
            pan: self.u32(4) as i32,
            reg_settings: [self.data[8] as i8, self.data[9] as i8, self.data[10] as i8, self.data[11] as i8],
            sound_id: self.u32(12) as i32,
        }
    }
}

/// A parsed bank file.
#[derive(Clone, Debug)]
pub struct Bank {
    /// Bank file type word (3).
    pub file_type: u32,
    /// `(offset, size)` of the SFX block and of the sample chunk in the bank file.
    pub chunks: [(u32, u32); 2],
    pub header: SfxHeader,
    pub sounds: Vec<Sound>,
    /// Every sound's grains, in sound order ([`Sound::grains`] indexes this).
    pub grains: Vec<Grain>,
    /// The distinct samples the tones use, by ascending offset into [`Bank::samples`].
    pub vags: Vec<SampleExtent>,
    /// Chunk 1: raw SPU ADPCM.
    pub samples: Vec<u8>,
}

impl Bank {
    /// The index into [`Bank::vags`] of the sample at `offset`.
    pub fn vag_at(&self, offset: u32) -> Option<usize> { self.vags.binary_search_by_key(&(offset as usize), |v| v.offset).ok() }
    pub fn sound_grains(&self, sound: usize) -> &[Grain] { &self.grains[self.sounds[sound].grains.clone()] }
}

/// Parses a `sound_bank` lump (level data WAD +0x08, or the global one).
pub fn parse_bank(bytes: &[u8]) -> Result<Bank> {
    let f = Buf(bytes);
    let file_type = f.u32(0)?;
    let nchunks = f.u32(4)?;
    if file_type != 3 || nchunks != 2 { return invalid(format!("sound bank: type {file_type}, {nchunks} chunks (expected 3, 2)")); }
    let chunks = [(f.u32(8)?, f.u32(12)?), (f.u32(16)?, f.u32(20)?)];
    let block = f.sub(chunks[0].0 as usize, chunks[0].1 as usize, "SFX block")?;
    let samples = f.sub(chunks[1].0 as usize, chunks[1].1 as usize, "sample chunk")?.bytes().to_vec();
    if &block.bytes()[..4.min(block.len())] != b"SBlk" { return invalid("sound bank: chunk 0 is not an SBlk block"); }
    let header = SfxHeader {
        version: block.u32(4)?,
        flags: block.u32(8)?,
        bank_id: block.u32(0xc)?,
        bank_num: block.i8(0x10)?,
        n_sounds: block.i16(0x16)?,
        n_grains: block.i16(0x18)?,
        n_vags: block.i16(0x1a)?,
        first_sound: block.u32(0x1c)?,
        first_grain: block.u32(0x20)?,
        vags_in_sr: block.u32(0x24)?,
        vag_data_size: block.u32(0x28)?,
        sram_alloc_size: block.u32(0x2c)?,
        next_block: block.u32(0x30)?,
    };
    if header.version != 1 { return invalid(format!("SBlk version {} (only the v1 0x28-byte grains are supported)", header.version)); }
    if header.n_sounds < 0 { return invalid("negative sound count"); }
    let mut sounds = Vec::with_capacity(header.n_sounds as usize);
    let mut grains = Vec::new();
    for i in 0..header.n_sounds as usize {
        let o = header.first_sound as usize + 12 * i;
        let n = block.u8(o + 4)? as usize;
        let first = block.u32(o + 8)?;
        let start = grains.len();
        for k in 0..n {
            let g = header.first_grain as usize + first as usize + 0x28 * k;
            let data: [u8; 32] = block.sub(g + 8, 32, "grain data")?.bytes().try_into().unwrap();
            grains.push(Grain { kind: block.u32(g)?, delay: block.i32(g + 4)?, data });
        }
        sounds.push(Sound {
            vol: block.i8(o)?,
            vol_group: block.i8(o + 1)?,
            pan: block.i16(o + 2)?,
            instance_limit: block.i8(o + 5)?,
            flags: block.u16(o + 6)?,
            first_grain: first,
            grains: start..grains.len(),
        });
    }
    let mut offsets: Vec<u32> = grains.iter().filter(|g| g.is_tone()).map(|g| g.tone().sample_offset).collect();
    offsets.sort_unstable();
    offsets.dedup();
    let vags = offsets.iter().map(|&o| vag::sample_extent(&samples, o as usize)).collect::<Result<Vec<_>>>()?;
    Ok(Bank { file_type, chunks, header, sounds, grains, vags, samples })
}

// ---------------------------------------------------------------------------------------------------
// SoundDef and the remap (core index +0x70). docs/plan/audio.md §2.2.

/// Game-side sound definition (0x20 bytes), used for level sounds and per-class sounds.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct SoundDef {
    /// 0x00: full volume at or inside this distance.
    pub near: f32,
    /// 0x04: `vol_far` at or beyond this distance.
    pub far: f32,
    /// 0x08: volume at `far` (0x400 = unity).
    pub vol_far: i32,
    /// 0x0c: volume at `near`.
    pub vol_near: i32,
    /// 0x10 / 0x14: pitch-bend range, `lo + rand() % (hi − lo)` per play.
    pub pb_lo: i32,
    pub pb_hi: i32,
    /// 0x18: must equal (play flags & 4 != 0), else the play is refused.
    pub looped: u8,
    /// 0x19: bit0 squared falloff, bit1 no occlusion, bit2 not halved above water, bit3 no underwater pitch drop.
    pub flags: u8,
    /// 0x1a: on disc an index into the remap's map; after load the **bank sound id** (0xffff = none).
    pub index: u16,
    /// 0x1c: bank handle, written by the loader (the port has one level bank; 0 on disc).
    pub bank_handle: u32,
}
const _: () = assert!(std::mem::size_of::<SoundDef>() == 0x20);

/// The sound definitions of a level after the loader's remap (`FUN_00258128`, the block after
/// `ParseParticleTexs`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LevelSounds {
    /// Level defs (the heap copy at 0x15f5f4, count 0x15f5f0) with `index` = bank sound id.
    pub level_defs: Vec<SoundDef>,
    /// The remap's map: bank sound id per def index.
    pub map: Vec<u16>,
    /// Per moby class, in core-index class order.
    pub classes: Vec<ClassSounds>,
}

/// One moby class's sound ids.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClassSounds {
    pub o_class: i32,
    /// Bank sound ids from the remap (`count × {u16 id, u16 0}`), in class sound order.
    pub bank_ids: Vec<u16>,
    /// The class blob's defs (header +0x0d count, +0x28 pointer) with `index` = the remapped bank id;
    /// empty when the class has no blob (its ids are parked at 0x1b02e0, at most 15).
    pub defs: Vec<SoundDef>,
    /// Header +0x0d (None without a blob). The loader prints warning 0x209800 when it differs from the remap count.
    pub header_count: Option<u8>,
}

/// Parses the remap block at core header +0x70 (index-relative `s16 defs_off, defs_count, map_off,
/// map_count`, then one `{s16 off, s16 count}` per moby class) and applies it as the loader does.
/// `core_data` is the decompressed core data (class blobs).
pub fn parse_level_sounds(core_index: &[u8], core: &LevelCore, core_data: &[u8]) -> Result<LevelSounds> {
    let idx = Buf(core_index);
    let base = core.header.sound_remap_offset;
    if base <= 0 { return Ok(LevelSounds::default()); }
    let base = base as usize;
    let rel = |v: i16| -> Result<usize> {
        usize::try_from(v as i32).map(|v| base + v).map_err(|_| crate::FormatError::Invalid(format!("negative remap offset {v}")))
    };
    let (defs_off, defs_count, map_off, map_count) = (idx.i16(base)?, idx.i16(base + 2)?, idx.i16(base + 4)?, idx.i16(base + 6)?);
    if defs_count < 0 || map_count < 0 { return invalid("negative remap count"); }
    let map: Vec<u16> = (0..map_count as usize).map(|k| idx.u16(rel(map_off)? + 4 * k)).collect::<Result<_>>()?;
    let mut level_defs: Vec<SoundDef> = idx.pod_slice(rel(defs_off)?, defs_count as usize, "level sound defs")?;
    // Signed compare: an index ≥ map_count becomes 0xffff (with a warning); a negative one (−1 on levels 0,
    // 11 and 14) passes and reads the u16 at `map + 4·index`, before the map, as the game does.
    for d in &mut level_defs {
        let i = d.index as i16;
        d.index = if i < map_count { idx.u16((base as i64 + map_off as i64 + 4 * i as i64) as usize)? } else { 0xffff };
    }
    let data = Buf(core_data);
    let mut classes = Vec::with_capacity(core.moby_classes.len());
    for (c, entry) in core.moby_classes.iter().enumerate() {
        let at = base + 8 + 4 * c;
        let (off, count) = (idx.i16(at)?, idx.i16(at + 2)?);
        let bank_ids: Vec<u16> = (0..count.max(0) as usize).map(|j| idx.u16(rel(off)? + 4 * j)).collect::<Result<_>>()?;
        let (defs, header_count) = if entry.offset_in_asset_wad > 0 {
            let blob = entry.offset_in_asset_wad as usize;
            let n = data.u8(blob + 0x0d)?;
            let ptr = data.i32(blob + 0x28)?;
            let mut defs: Vec<SoundDef> = if n > 0 && ptr > 0 { data.pod_slice(blob + ptr as usize, n as usize, "class sound defs")? } else { Vec::new() };
            // The loader writes entry[j] into def j for every j < header count, walking past the class's remap
            // list when the header count is larger (level 10 class 1229: 12 defs, 6 ids; it picks up the next
            // class's ids). Same here.
            for (j, d) in defs.iter_mut().enumerate() { d.index = idx.u16(rel(off)? + 4 * j)?; }
            (defs, Some(n))
        } else {
            (Vec::new(), None)
        };
        classes.push(ClassSounds { o_class: entry.o_class, bank_ids, defs, header_count });
    }
    Ok(LevelSounds { level_defs, map, classes })
}

/// The hand items' class sound defs: a gadget class (the wrench, the Swingshot, every weapon: the core's gadget table,
/// `crate::gadget`) has no blob in the level's class table, so [`parse_level_sounds`] leaves its defs empty and the
/// loader parks the first `min(count, 15)` ids of its remap list at 0x1b02e0 (`LoadLevelCoreData` 0x258128; the rest
/// −1). When the gadget is loaded (`select_world_object_resource_tables` 0x259788) its blob's defs (header +0x0d
/// count, +0x28 pointer) take them: def j's `index` = parked id j when that is not negative (s16), else the blob's
/// own. `gadgets` = `(o_class, decompressed blob)` of every gadget class; classes that have a level blob are left
/// alone.
pub fn apply_gadget_defs(sounds: &mut LevelSounds, gadgets: &[(i32, &[u8])]) -> Result<()> {
    for &(o, blob) in gadgets {
        let Some(c) = sounds.classes.iter_mut().find(|c| c.o_class == o && c.header_count.is_none()) else { continue };
        let b = Buf(blob);
        let n = b.u8(0x0d)?;
        let ptr = b.i32(0x28)?;
        let mut defs: Vec<SoundDef> = if n > 0 && ptr > 0 { b.pod_slice(ptr as usize, n as usize, "gadget sound defs")? } else { Vec::new() };
        let parked = c.bank_ids.len().min(15);
        for (j, d) in defs.iter_mut().enumerate().take(parked) {
            let id = c.bank_ids[j];
            if (id as i16) >= 0 { d.index = id; }
        }
        c.defs = defs;
        c.header_count = Some(n);
    }
    Ok(())
}

/// Who owns a sound id request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundOwner {
    /// A level def (sound instances, footsteps, moby-attached level sounds).
    Level,
    /// A moby class (`o_class`): class sound `local` of that class.
    Class(i32),
}

impl LevelSounds {
    /// `resolve_sound(owner, local) -> bank id`: level def `local`'s bank sound, or the class's `local`-th
    /// class sound. `None` for an out-of-range index or an unmapped (0xffff) id.
    pub fn resolve_sound(&self, owner: SoundOwner, local: usize) -> Option<u16> {
        let id = match owner {
            SoundOwner::Level => self.level_defs.get(local)?.index,
            SoundOwner::Class(o) => *self.classes.iter().find(|c| c.o_class == o)?.bank_ids.get(local)?,
        };
        (id != 0xffff).then_some(id)
    }

    /// The def a play of `(owner, local)` uses (level def, or the class blob's def).
    pub fn def(&self, owner: SoundOwner, local: usize) -> Option<&SoundDef> {
        match owner {
            SoundOwner::Level => self.level_defs.get(local),
            SoundOwner::Class(o) => self.classes.iter().find(|c| c.o_class == o)?.defs.get(local),
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// Music table (level header `music[15]`, in memory at 0x13a628): track k = `levels/NN/music/k`.

/// The level header's music sectors (0 = no track). Tracks come in `(Start, Loop)` pairs.
pub fn music_table(level_header: &[u8]) -> Result<[i32; 15]> {
    Ok(crate::toc::parse_level_header(level_header)?.music)
}

// ---------------------------------------------------------------------------------------------------
// Gameplay sections read by the sound code.

/// Gameplay header pointer of the sound instances.
pub const SOUND_INSTANCES_POINTER: usize = 0x0c;
/// Gameplay header pointer of the env sample points.
pub const ENV_SAMPLE_POINTS_POINTER: usize = 0x88;

/// A sound instance (0x90 bytes; docs/formats/wad_layouts_rac1.md §3.13). The runtime copy keeps the
/// same offsets: `+0x40` = matrix row 3 = the position, `+0x50` = the inverse rows the boxes test with.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SoundInstance {
    /// Class: 0 sphere, 1 box volume, 2 box one-shot, 3 reverb box, 5 underwater loop, 6 music box.
    pub o_class: i16,
    pub m_class: i16,
    pub pvar_index: i32,
    pub range: f32,
    pub matrix: [[f32; 4]; 4],
    /// Three rows; `[3][3]` of the forward matrix is 0.01 on the disc (collision_rac1.md).
    pub inverse: [[f32; 4]; 3],
    pub rotation: [f32; 3],
}

impl SoundInstance {
    pub fn position(&self) -> [f32; 3] { [self.matrix[3][0], self.matrix[3][1], self.matrix[3][2]] }
    /// `fun_001f9cf8(out, v, inverse)`: `out = v.x·inv[0] + v.y·inv[1] + v.z·inv[2]` (VU0 `vmula/vmadda`),
    /// the box-local coordinates of a world-space offset from the centre.
    pub fn to_local(&self, v: [f32; 3]) -> [f32; 3] {
        let r = &self.inverse;
        std::array::from_fn(|k| v[0] * r[0][k] + v[1] * r[1][k] + v[2] * r[2][k])
    }
}

/// Reads section 0x0c: `s32 count, pad[3]`, then `count` 0x90-byte records.
pub fn parse_sound_instances(gameplay: &[u8]) -> Result<Vec<SoundInstance>> {
    let g = Buf(gameplay);
    let p = g.u32(SOUND_INSTANCES_POINTER)? as usize;
    if p == 0 { return Ok(Vec::new()); }
    let n = g.i32(p)?;
    if n < 0 { return invalid("negative sound instance count"); }
    g.check(p + 0x10, 0x90 * n as usize, "sound instances")?;
    let f = |o: usize| f32::from_le_bytes(gameplay[o..o + 4].try_into().unwrap());
    let h = |o: usize| i16::from_le_bytes([gameplay[o], gameplay[o + 1]]);
    Ok((0..n as usize)
        .map(|i| {
            let o = p + 0x10 + 0x90 * i;
            SoundInstance {
                o_class: h(o),
                m_class: h(o + 2),
                pvar_index: i32::from_le_bytes(gameplay[o + 8..o + 12].try_into().unwrap()),
                range: f(o + 0xc),
                matrix: [0, 1, 2, 3].map(|r| [0, 1, 2, 3].map(|c| f(o + 0x10 + 16 * r + 4 * c))),
                inverse: [0, 1, 2].map(|r| [0, 1, 2, 3].map(|c| f(o + 0x50 + 16 * r + 4 * c))),
                rotation: [f(o + 0x80), f(o + 0x84), f(o + 0x88)],
            }
        })
        .collect())
}

/// A pvar block by index straight from the pvar table (0x54) and data (0x58): sound instances and cameras
/// share the table with the mobys, so this does not depend on the moby pass.
pub fn pvar_block(gameplay: &[u8], index: i32) -> Result<Option<Vec<u8>>> {
    let g = Buf(gameplay);
    let (table, data) = (g.u32(crate::gameplay::PVAR_TABLE_POINTER)? as usize, g.u32(crate::gameplay::PVAR_DATA_POINTER)? as usize);
    if index < 0 || table == 0 || data == 0 { return Ok(None); }
    let (ofs, size) = (g.i32(table + 8 * index as usize)?, g.i32(table + 8 * index as usize + 4)?);
    if ofs < 0 || size < 0 { return invalid(format!("pvar {index}: negative table entry")); }
    Ok(Some(g.sub(data + ofs as usize, size as usize, "pvar block")?.bytes().to_vec()))
}

/// Env sample point (0x30 bytes, section 0x88): the fields the sound code reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EnvSamplePoint {
    pub position: [f32; 3],
    /// +0x20 reverb depth, +0x24 type (libsd: 3 STUDIO_B, 4 STUDIO_C, 9 PIPE), +0x25 delay, +0x26 feedback,
    /// +0x27 enable.
    pub reverb_depth: i32,
    pub reverb_type: u8,
    pub reverb_delay: u8,
    pub reverb_feedback: u8,
    pub reverb_enable: u8,
    /// +0x28: music track the level starts (while music is idle) near this point.
    pub music_track: i32,
}

pub fn parse_env_sample_points(gameplay: &[u8]) -> Result<Vec<EnvSamplePoint>> {
    let g = Buf(gameplay);
    let p = g.u32(ENV_SAMPLE_POINTS_POINTER)? as usize;
    if p == 0 { return Ok(Vec::new()); }
    let n = g.i32(p)?;
    if n < 0 { return invalid("negative env sample point count"); }
    (0..n as usize)
        .map(|i| {
            let o = p + 0x10 + 0x30 * i;
            Ok(EnvSamplePoint {
                position: [g.f32(o)?, g.f32(o + 4)?, g.f32(o + 8)?],
                reverb_depth: g.i32(o + 0x20)?,
                reverb_type: g.u8(o + 0x24)?,
                reverb_delay: g.u8(o + 0x25)?,
                reverb_feedback: g.u8(o + 0x26)?,
                reverb_enable: g.u8(o + 0x27)?,
                music_track: g.i32(o + 0x28)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-sound bank: sound 0 = one tone (looped sample), sound 1 = marker, rand delay, tone (one-shot).
    fn synthetic_bank() -> Vec<u8> {
        let mut samples = Vec::new();
        for flags in [6u8, 2, 3, 0, 1, 7] {
            let mut f = [0u8; 16];
            f[1] = flags;
            samples.extend(f);
        }
        let mut block = vec![0u8; 0x3c];
        block[..4].copy_from_slice(b"SBlk");
        block[4..8].copy_from_slice(&1u32.to_le_bytes());
        block[0x16..0x18].copy_from_slice(&2i16.to_le_bytes());
        block[0x18..0x1a].copy_from_slice(&4i16.to_le_bytes());
        block[0x1a..0x1c].copy_from_slice(&2i16.to_le_bytes());
        block[0x1c..0x20].copy_from_slice(&0x3cu32.to_le_bytes());
        block[0x20..0x24].copy_from_slice(&0x54u32.to_le_bytes());
        // Sounds at 0x3c: {vol, group, pan, n, limit, flags, first}.
        block.extend([127u8, 0, 0, 0, 1, 0, 1, 0]);
        block.extend(0u32.to_le_bytes());
        block.extend([90u8, 4, 0, 0, 3, 0, 0, 0]);
        block.extend(0x28u32.to_le_bytes());
        assert_eq!(block.len(), 0x54);
        let tone = |off: u32, note: i8| {
            let mut g = vec![0u8; 0x28];
            g[..4].copy_from_slice(&1u32.to_le_bytes());
            g[8] = 90;
            g[9] = 127;
            g[10] = note as u8;
            g[18..20].copy_from_slice(&0x80ffu16.to_le_bytes());
            g[20..22].copy_from_slice(&0x9fc0u16.to_le_bytes());
            g[24..28].copy_from_slice(&off.to_le_bytes());
            g
        };
        block.extend(tone(0, -60));
        let mut marker = vec![0u8; 0x28];
        marker[..4].copy_from_slice(&35u32.to_le_bytes());
        block.extend(marker);
        let mut rd = vec![0u8; 0x28];
        rd[..4].copy_from_slice(&26u32.to_le_bytes());
        rd[4..8].copy_from_slice(&1800i32.to_le_bytes());
        rd[8..12].copy_from_slice(&2400i32.to_le_bytes());
        block.extend(rd);
        block.extend(tone(0x30, 72));
        let mut file = Vec::new();
        for w in [3u32, 2, 0x18, block.len() as u32, 0x18 + block.len() as u32, samples.len() as u32] { file.extend(w.to_le_bytes()); }
        file.extend(block);
        file.extend(samples);
        file
    }

    #[test]
    fn parses_a_synthetic_bank() {
        let b = parse_bank(&synthetic_bank()).unwrap();
        assert_eq!(b.header.n_sounds, 2);
        assert_eq!(b.sounds.len(), 2);
        assert!(b.sounds[0].looped());
        assert_eq!((b.sounds[1].vol, b.sounds[1].vol_group), (90, 4));
        assert_eq!(b.grains.len(), 4);
        assert_eq!(b.sound_grains(1).iter().map(|g| g.kind).collect::<Vec<_>>(), vec![35, 26, 1]);
        assert_eq!(b.sound_grains(1)[1].rand_delay_amount(), 2400);
        assert_eq!(b.sound_grains(1)[1].delay, 1800);
        let t = b.sound_grains(1)[2].tone();
        assert_eq!((t.priority, t.vol, t.center_note, t.adsr1, t.adsr2, t.sample_offset), (90, 127, 72, 0x80ff, 0x9fc0, 0x30));
        assert_eq!(b.vags.len(), 2);
        assert!(b.vags[0].looped && !b.vags[1].looped);
        assert_eq!(b.vags[1].next_flags, Some(7));
        assert_eq!(b.vag_at(0x30), Some(1));
        let mut bad = synthetic_bank();
        bad[0] = 4;
        assert!(parse_bank(&bad).is_err());
    }

    #[test]
    fn resolve_sound_by_owner() {
        let def = |index| SoundDef { index, ..Default::default() };
        let s = LevelSounds {
            level_defs: vec![def(0), def(21), def(0xffff)],
            map: vec![0, 21],
            classes: vec![ClassSounds { o_class: 688, bank_ids: vec![220], defs: vec![def(220)], header_count: Some(1) }],
        };
        assert_eq!(s.resolve_sound(SoundOwner::Level, 1), Some(21));
        assert_eq!(s.resolve_sound(SoundOwner::Level, 2), None);
        assert_eq!(s.resolve_sound(SoundOwner::Level, 3), None);
        assert_eq!(s.resolve_sound(SoundOwner::Class(688), 0), Some(220));
        assert_eq!(s.resolve_sound(SoundOwner::Class(1), 0), None);
        assert_eq!(s.def(SoundOwner::Class(688), 0).map(|d| d.index), Some(220));
    }
}
