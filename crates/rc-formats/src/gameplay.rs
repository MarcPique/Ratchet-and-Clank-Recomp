//! Gameplay (instances) file: moby instances. Spec: docs/formats/wad_layouts_rac1.md §3.5 and
//! docs/formats/moby_rac1.md §7.3; field use from the level loader that turns each record into a
//! runtime moby, `FUN_00255958` in the level01 overlay (the loop after `piVar27[0x11]`, pointer 0x44),
//! with `InitMobyInstance` (boot 0x20c5f0), `pack_render_command_fields` (boot 0x20d4f0) and the
//! matrix builder `fun_0020def8` (boot 0x20def8). Notes: docs/plan/moby_render_notes.md.
//!
//! The file is the decompressed `gameplay_ntsc` lump. Other sections (lights: `tfrag_light`,
//! level settings: the engine's `game_camera`) are read elsewhere.

use crate::buf::{invalid, Buf, Result};
use bytemuck::{Pod, Zeroable};

/// Section pointer of the moby instances in the gameplay header (37 `s32` pointers).
pub const MOBY_INSTANCES_POINTER: usize = 0x44;
/// Record size; the loader advances by each record's own `size` field, which is always this.
pub const MOBY_INSTANCE_SIZE: usize = 0x78;

/// One static moby instance as stored in the gameplay file (0x78 bytes). "moby+X" names the field of
/// the 0x100-byte runtime moby the loader copies it to.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct MobyInstance {
    /// 0x00: record size, 0x78; the loader steps by it.
    pub size: i32,
    /// 0x04: → moby+0xb0 (byte) and the index into a table at 0x15fc88 used by the spawn test. -1 on most.
    pub unknown_4: i32,
    /// 0x08: spawn condition flags (0 = always). Bits 0..3 select save/game-state bit tests on
    /// `spawn_id`; 0x10 picks moby+0xb1 from `FUN_0029ab50`. See [`MobyInstance::spawn_flags`] notes.
    pub spawn_flags: i32,
    /// 0x0c: → moby+0xb2 (s16); bit index for the spawn tests (unique per instance, -2 on some).
    pub spawn_id: i32,
    /// 0x10: → moby+0xb4 (s16) (halved by some spawn tests).
    pub unknown_10: i32,
    /// 0x14: alternative for `unknown_10` in one spawn-test branch.
    pub unknown_14: i32,
    /// 0x18: moby class number (`MobyClassEntry::o_class`); passed to `InitMobyInstance`.
    pub o_class: i32,
    /// 0x1c: instance scale; moby+0x2c = class scale (class header 0x24) × this (EE FPU `mul.s`).
    pub scale: f32,
    /// 0x20: → moby+0x32 (s16). Wrench calls it an f32 `draw_distance`; the loader reads it as an
    /// integer (64 on Novalis), and MobyProc culls/fades against it (rc-engine `moby_lod.rs`).
    pub draw_distance: i32,
    /// 0x24: → moby+0x30 (byte), update distance.
    pub update_distance: i32,
    /// 0x28: 32 on the disc, unused by the loader.
    pub unused_28: i32,
    /// 0x2c: 64 on the disc, unused by the loader.
    pub unused_2c: i32,
    /// 0x30: world position, game units (→ moby+0x10).
    pub position: [f32; 3],
    /// 0x3c: Euler angles in radians (→ moby+0x40). `fun_0020def8` turns them into rotation rows with
    /// VU0 program 28259 at 0xd18: R = Rz(z)·Ry(y)·Rx(x) (x applied first), see
    /// [`crate::moby_light::rotation_rows`].
    pub rotation: [f32; 3],
    /// 0x48: moby group, or -1 (→ moby+0x21, byte).
    pub group: i32,
    /// 0x4c: non-zero: z = ground height below the position (`FUN_0026e618`) + `rooted_distance` (the loader's snap:
    /// `rc_game::moby_update::scheduler::root_instances`).
    pub is_rooted: i32,
    /// 0x50: see `is_rooted`.
    pub rooted_distance: f32,
    /// 0x54: unused by the loader.
    pub unknown_54: i32,
    /// 0x58: pvar index or -1 (→ moby+0x78, later relocated to a pointer).
    pub pvar_index: i32,
    /// 0x5c: 0 clears moby+0x36 (occlusion bits), else moby+0x36 = 0x7f80.
    pub occlusion: i32,
    /// 0x60: OR'd into moby+0x34 (u16 mode bits) and into the class header's 0x44. 0x8000 mirrors the
    /// second rotation row (`fun_0020def8`); 0x100 keeps the existing rows (not rebuilt from `rotation`).
    pub mode_bits: i32,
    /// 0x64: ambient colour r, g, b (0..255): `pack_render_command_fields(moby, b<<16 | g<<8 | r, ..)`
    /// puts it at moby+0x3c..0x3e, the per-moby ambient of the lighting (128 = 1.0).
    pub color: [i32; 3],
    /// 0x70: written as an s32 to moby+0x38: byte 0 = directional light set, byte 1 = second set,
    /// byte 2 = cross-fade weight (0 = none) of the MobyProc light selection.
    pub light: i32,
    /// 0x74: if not -1 the loader calls `FUN_0025e7b0(moby)` (`MobyUnknown74Hook`: the NPC's talk slot, rc-game
    /// `moby_update::interact`).
    pub unknown_74: i32,
}
const _: () = assert!(std::mem::size_of::<MobyInstance>() == MOBY_INSTANCE_SIZE);

impl MobyInstance {
    /// moby+0x38..0x3b as the loader leaves it: the `light` word (little-endian bytes: set 0, set 1,
    /// cross-fade, unused).
    pub fn light_word(&self) -> u32 { self.light as u32 }

    /// moby+0x3c..0x3e: the low three bytes of `b * 0x10000 + g * 0x100 + r` (s32 arithmetic, so
    /// channels above 255 would carry; none do on the disc).
    pub fn ambient_rgb(&self) -> [u8; 3] {
        let [r, g, b] = self.color;
        let w = b.wrapping_mul(0x10000).wrapping_add(g.wrapping_mul(0x100)).wrapping_add(r) as u32;
        [w as u8, (w >> 8) as u8, (w >> 16) as u8]
    }

    /// moby+0x34 bits contributed by the instance.
    pub fn mode(&self) -> u16 { self.mode_bits as u16 }
}

/// Reads the static moby instances. Section layout: `s32 static_count; s32 spawnable_count; s32 pad[2]`,
/// then the records. The loader walks `static_count` records, stepping by each record's `size`;
/// here every size must be 0x78. An absent section (pointer 0) gives no instances.
pub fn parse_moby_instances(gameplay: &[u8]) -> Result<Vec<MobyInstance>> {
    let g = Buf(gameplay);
    let ofs = g.u32(MOBY_INSTANCES_POINTER)? as usize;
    if ofs == 0 { return Ok(Vec::new()); }
    let count = g.i32(ofs)?;
    if count < 0 { return invalid("negative moby instance count"); }
    let mut out = Vec::with_capacity(count as usize);
    let mut p = ofs + 0x10;
    for i in 0..count as usize {
        let m: MobyInstance = g.pod(p, "moby instance")?;
        if m.size as usize != MOBY_INSTANCE_SIZE {
            return invalid(format!("moby instance {i}: size {:#x}, expected 0x78", m.size));
        }
        out.push(m);
        p += MOBY_INSTANCE_SIZE;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------
// Pvars (per-instance class variables). Loader: `FUN_00255958` right after the instance loop
// (docs/plan/moby_update_catalogue.md §2). Header pointers are indices into the 37-word header.

/// 0x54: pvar table, `{s32 offset (into the data), s32 size}` per pvar index. No count in the file.
pub const PVAR_TABLE_POINTER: usize = 0x54;
/// 0x58: pvar data. When this pointer is 0 the loader copies nothing (every moby+0x78 stays the index).
pub const PVAR_DATA_POINTER: usize = 0x58;
/// 0x50: moby-link fixups `{s32 pvar_index, s32 byte_offset}`, terminated by `pvar_index < 0`.
pub const PVAR_MOBY_LINK_FIXUPS_POINTER: usize = 0x50;
/// 0x5c: pointer fixups, same record shape and terminator.
pub const PVAR_POINTER_FIXUPS_POINTER: usize = 0x5c;
/// 0x4c: shared data blob `{s32 size, s32 count, pad[2], data[size], {u16 pvar_index, u16 byte_offset,
/// s32 data_offset}[count]}`; each record stores the absolute address `blob + data_offset` into the pvar
/// (loader, before the spline copy). Empty on Novalis (size 0, count 0). [`parse_pvars_spawned`] applies the records
/// the port's way: the field gets `data_offset` (blob-relative; [`parse_pvar_shared_data`] gives the blob, which the
/// moby system keeps as `Services::pvar_shared`). Levels 00 / 02 / 05 / 18 use it for the lamps 1060 (one word per
/// moby group: the tick the group's glow callback was last registered), 05 for 838 / 855, and others (census).
pub const PVAR_SHARED_DATA_POINTER: usize = 0x4c;

/// The shared data blob of section 0x4c (`data[size]`; empty when the section is absent or empty).
pub fn parse_pvar_shared_data(gameplay: &[u8]) -> Result<Vec<u8>> {
    let g = Buf(gameplay);
    let p = g.u32(PVAR_SHARED_DATA_POINTER)? as usize;
    if p == 0 { return Ok(Vec::new()); }
    let size = g.i32(p)?;
    if size < 0 { return invalid("negative shared data size"); }
    Ok(g.sub(p + 0x10, size as usize, "pvar shared data")?.bytes().to_vec())
}

/// The records of section 0x4c: `(pvar_index, byte_offset, data_offset)`.
pub fn parse_pvar_shared_records(gameplay: &[u8]) -> Result<Vec<(u16, u16, i32)>> {
    let g = Buf(gameplay);
    let p = g.u32(PVAR_SHARED_DATA_POINTER)? as usize;
    if p == 0 { return Ok(Vec::new()); }
    let (size, count) = (g.i32(p)?, g.i32(p + 4)?);
    if size < 0 || count < 0 { return invalid("negative shared data size / count"); }
    let r = p + 0x10 + size as usize;
    (0..count as usize).map(|k| Ok((g.u16(r + 8 * k)?, g.u16(r + 8 * k + 2)?, g.i32(r + 8 * k + 4)?))).collect()
}

/// The two pvar fixup lists (terminated lists of `(pvar_index, byte_offset)`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PvarFixups {
    /// 0x50: the s32 at `byte_offset` is a gameplay instance index; the loader replaces it with the runtime
    /// moby index `0x1acc00[index]` (s16, −1 when that instance did not spawn). Negative values are kept.
    pub moby_links: Vec<(i32, i32)>,
    /// 0x5c: the s32 at `byte_offset` is an offset relative to the pvar block; the loader adds the block's
    /// heap address. The port keeps the block-relative value (a host pointer has no meaning in the bytes).
    pub pointers: Vec<(i32, i32)>,
}

fn fixup_list(g: Buf, header: usize) -> Result<Vec<(i32, i32)>> {
    let mut p = g.u32(header)? as usize;
    let mut out = Vec::new();
    if p == 0 { return Ok(out); }
    loop {
        let index = g.i32(p)?;
        if index < 0 { return Ok(out); }
        out.push((index, g.i32(p + 4)?));
        p += 8;
    }
}

/// Reads the two fixup lists (0x50, 0x5c).
pub fn parse_pvar_fixups(gameplay: &[u8]) -> Result<PvarFixups> {
    let g = Buf(gameplay);
    Ok(PvarFixups { moby_links: fixup_list(g, PVAR_MOBY_LINK_FIXUPS_POINTER)?, pointers: fixup_list(g, PVAR_POINTER_FIXUPS_POINTER)? })
}

/// The pvar blocks as the level loader leaves them, indexed by **pvar index** (the instance's
/// `pvar_index`, see [`MobyInstance::pvar`]), assuming every static instance spawns (the port ignores the
/// spawn conditions, moby_render_notes.md §1), so the runtime moby index of instance i is i.
/// See [`parse_pvars_spawned`].
pub fn parse_pvars(gameplay: &[u8]) -> Result<Vec<Option<Vec<u8>>>> {
    let n = parse_moby_instances(gameplay)?.len();
    parse_pvars_spawned(gameplay, &vec![true; n])
}

/// `FUN_00255958`'s pvar pass. Each block is `data[offset .. offset + size]` of its table entry (the loader
/// copies it into the level heap and points moby+0x78 at the copy). Then, in list order:
/// * moby-link fixups (0x50): for an entry whose block was copied, the s32 at `byte_offset`, when ≥ 0, is
///   an instance index and becomes `0x1acc00[index]`, the runtime index: the number of spawned instances
///   before it, or −1 when it did not spawn (`spawned[index] == false`; an index past the instance list
///   also gives −1 here, the game would read its uninitialised table);
/// * pointer fixups (0x5c): `+= copy address` in the game; the port leaves the block-relative offset;
/// * shared data (0x4c): `= blob + data_offset` in the game; the port stores `data_offset` ([`PVAR_SHARED_DATA_POINTER`]).
///
/// "Copied" = used by a spawned moby, a camera or a level callback (the loader copies those three kinds
/// and checks `table.offset ≥ gameplay base`, true only after the copy). The port applies the fixups to
/// every block: blocks nobody copies are never read. Entry count: the highest pvar index any instance or
/// fixup refers to, + 1 (the file has no count). `None` for an index whose entry lies outside the data.
/// An absent data section (0x58 = 0) gives an empty list, as the loader then copies nothing.
pub fn parse_pvars_spawned(gameplay: &[u8], spawned: &[bool]) -> Result<Vec<Option<Vec<u8>>>> {
    let g = Buf(gameplay);
    let (table, data) = (g.u32(PVAR_TABLE_POINTER)? as usize, g.u32(PVAR_DATA_POINTER)? as usize);
    if table == 0 || data == 0 { return Ok(Vec::new()); }
    let instances = parse_moby_instances(gameplay)?;
    let fixups = parse_pvar_fixups(gameplay)?;
    let max_index = instances.iter().map(|m| m.pvar_index).chain(fixups.moby_links.iter().chain(&fixups.pointers).map(|f| f.0)).max().unwrap_or(-1);
    let mut blocks: Vec<Option<Vec<u8>>> = (0..(max_index + 1).max(0) as usize)
        .map(|i| {
            let (ofs, size) = (g.i32(table + 8 * i).ok()?, g.i32(table + 8 * i + 4).ok()?);
            let start = data.checked_add(usize::try_from(ofs).ok()?)?;
            Some(g.sub(start, usize::try_from(size).ok()?, "pvar block").ok()?.bytes().to_vec())
        })
        .collect();
    // Runtime index of instance i: 0x1acc00[i] (u16 -1 = not spawned, read sign-extended).
    let mut runtime = Vec::with_capacity(instances.len());
    let mut next = 0i32;
    for i in 0..instances.len() {
        if spawned.get(i).copied().unwrap_or(true) { runtime.push(next); next += 1 } else { runtime.push(-1) }
    }
    let field = |blocks: &[Option<Vec<u8>>], (index, ofs): (i32, i32)| -> Result<Option<(usize, usize)>> {
        let Some(Some(b)) = blocks.get(index as usize) else { return Ok(None) };
        let o = usize::try_from(ofs).map_err(|_| crate::FormatError::Invalid(format!("pvar {index}: negative fixup offset {ofs}")))?;
        Buf(b).check(o, 4, "pvar fixup field")?;
        Ok(Some((index as usize, o)))
    };
    for &f in &fixups.moby_links {
        let Some((b, o)) = field(&blocks, f)? else { continue };
        let block = blocks[b].as_mut().unwrap();
        let v = i32::from_le_bytes(block[o..o + 4].try_into().unwrap());
        if v >= 0 {
            let r = runtime.get(v as usize).copied().unwrap_or(-1);
            block[o..o + 4].copy_from_slice(&r.to_le_bytes());
        }
    }
    for &f in &fixups.pointers { field(&blocks, f)?; } // validated only: kept block-relative
    // Shared data (0x4c): the field points into the blob; the port stores the blob-relative offset.
    for (index, ofs, data) in parse_pvar_shared_records(gameplay)? {
        let Some((b, o)) = field(&blocks, (index as i32, ofs as i32))? else { continue };
        blocks[b].as_mut().unwrap()[o..o + 4].copy_from_slice(&data.to_le_bytes());
    }
    Ok(blocks)
}

impl MobyInstance {
    /// This instance's pvar block from [`parse_pvars`] (None for `pvar_index` −1 or a missing entry).
    pub fn pvar<'a>(&self, pvars: &'a [Option<Vec<u8>>]) -> Option<&'a [u8]> {
        pvars.get(usize::try_from(self.pvar_index).ok()?)?.as_deref()
    }
}

// ---------------------------------------------------------------------------------------------------
// Paths (splines) and the level settings' ship placement

/// 0x70: paths. Header `{s32 count, s32 data_offset, s32 data_size, pad}` (offsets from the section
/// start), `count` s32 spline offsets at +0x10 relative to the data. The loader copies the data and fills
/// the table `0x1b0930[i] = copy + offset[i]` (`FUN_00255958`).
pub const PATHS_POINTER: usize = 0x70;

/// The level's splines (`0x1b0930[i]`): each is `s32 count, pad[3]`, then `count` × `f32 x, y, z, w` at
/// +0x10 (enemy 459 reads point 0 at +0x10 and the w words at +0x1c + 16·k).
pub fn parse_splines(gameplay: &[u8]) -> Result<Vec<Vec<[f32; 4]>>> {
    let g = Buf(gameplay);
    let s = g.u32(PATHS_POINTER)? as usize;
    if s == 0 { return Ok(Vec::new()); }
    let (count, data_ofs, data_size) = (g.i32(s)?, g.i32(s + 4)?, g.i32(s + 8)?);
    if count < 0 || data_ofs < 0 || data_size < 0 { return invalid("negative path section field"); }
    let data = g.sub(s + data_ofs as usize, data_size as usize, "path data")?;
    (0..count as usize)
        .map(|i| {
            let o = g.i32(s + 0x10 + 4 * i)?;
            let o = usize::try_from(o).map_err(|_| crate::FormatError::Invalid(format!("spline {i}: negative offset")))?;
            let n = data.i32(o)?;
            if n < 0 { return invalid(format!("spline {i}: negative point count")); }
            data.pod_slice::<[f32; 4]>(o + 0x10, n as usize, "spline points")
        })
        .collect()
}

/// Header pointer 0x00: level settings.
pub const LEVEL_SETTINGS_POINTER: usize = 0x00;

/// The 37 section pointers at the start of the gameplay file, in pointer order (docs/formats/wad_layouts_rac1.md,
/// gameplay spec 3.2); the loader-facing constants above and below name the ones the port reads.
pub const SECTION_NAMES: [&str; 37] = [
    "level_settings", "directional_lights", "cameras", "sound_instances",
    "help_us_english", "help_uk_english", "help_french", "help_german", "help_spanish", "help_italian", "help_japanese", "help_korean",
    "tie_classes", "tie_instances", "shrub_classes", "shrub_instances", "moby_classes", "moby_instances", "moby_groups",
    "shared_data", "pvar_moby_links", "pvar_table", "pvar_data", "pvar_pointer_fixups",
    "cuboids", "spheres", "cylinders", "pills", "paths", "grind_paths", "point_light_grid", "point_lights",
    "env_transitions", "camera_collision_grid", "env_sample_points", "occlusion_mappings", "unused_90",
];

/// The present sections of a decompressed gameplay file as (name, byte range), in file order: each runs from its
/// pointer to the next pointer in file order, the last to the end of the file.
pub fn sections(gameplay: &[u8]) -> Result<Vec<(&'static str, std::ops::Range<usize>)>> {
    let g = Buf(gameplay);
    let mut starts = Vec::new();
    for (i, name) in SECTION_NAMES.iter().enumerate() {
        let o = g.i32(4 * i)?;
        if o > 0 { starts.push((o as usize, *name)); }
    }
    starts.sort();
    let mut out = Vec::with_capacity(starts.len());
    for (k, &(start, name)) in starts.iter().enumerate() {
        let end = starts.get(k + 1).map_or(gameplay.len(), |s| s.0);
        if end > gameplay.len() { return invalid(format!("gameplay section {name} runs past the end")); }
        out.push((name, start..end));
    }
    Ok(out)
}

/// The bytes of one named section (see [`sections`]), or None when the file does not have it.
pub fn section<'a>(gameplay: &'a [u8], name: &str) -> Result<Option<&'a [u8]>> {
    Ok(sections(gameplay)?.into_iter().find(|s| s.0 == name).map(|(_, r)| &gameplay[r]))
}

/// Where the level loader creates the player's ship (`FUN_00255958`, after the camera/sound sections):
/// if level settings +0x2c (x) > 0, `CreateMoby(0x160548[ship])` gets position = settings +0x2c/+0x30/+0x34
/// and rotation z (moby+0x48) = settings +0x38; x ≤ 0 means no ship in this level. Returns (position, yaw).
pub fn ship_placement(gameplay: &[u8]) -> Result<Option<([f32; 3], f32)>> {
    let g = Buf(gameplay);
    let s = g.u32(LEVEL_SETTINGS_POINTER)? as usize;
    let x = g.f32(s + 0x2c)?;
    if x.is_nan() || x <= 0.0 { return Ok(None); }
    Ok(Some(([x, g.f32(s + 0x30)?, g.f32(s + 0x34)?], g.f32(s + 0x38)?)))
}

// ---------------------------------------------------------------------------------------------------
// Environment (fog) transition zones: section 0x80. Loader `FUN_00255958` (level01; after the lights):
// `count = *s; memcpy(0x17f140, s + 0x10, count·16); memcpy(0x17f340, s + 0x10 + count·16, count·0x80)`,
// then every record's hero light −1 becomes 0xb. Readers: lookup 0x26bbc0, fog `fun_001ee4b0` (level
// 0x2102b8), hero lighting 0x26be04. Spec: docs/plan/world_animation.md §5.

/// 0x80: environment transitions (fog zones).
pub const FOG_ZONES_POINTER: usize = 0x80;
/// Record size (0x17f340 + i·0x80).
pub const FOG_ZONE_SIZE: usize = 0x80;

/// One end of a fog transition: the level fog at t = 0 (side 1) or t = 1 (side 2), in game units and
/// densities (`F = 255 − 255·d`, so 0 = no fog).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct FogZoneSide {
    pub near_dist: f32,
    pub near_density: f32,
    pub far_dist: f32,
    pub far_density: f32,
}

/// One transition record as the loader copies it to 0x17f340 + i·0x80.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct FogZone {
    /// 0x00: world → cuboid-local matrix, VU row-vector form: `l = x·row0 + y·row1 + z·row2 + row3`.
    /// The cuboid is |l.xyz| ≤ 1; the blend runs along local x (t = (l.x + 1)/2).
    pub inverse: [[f32; 4]; 4],
    /// 0x40 / 0x44: hero colour at t = 0 / t = 1 (bytes r, g, b, a; hero moby +0x80, flags & 1).
    pub hero_color: [u32; 2],
    /// 0x48 / 0x4c: hero directional light bank at t = 0 / t = 1 (hero moby +0x38 bytes 0 / 1). The file
    /// stores −1 for "none"; [`parse_fog_zones`] applies the loader's −1 → 0xb.
    pub hero_light: [i32; 2],
    /// 0x50: bit 0 = cross-fade the hero lighting (0x26be04), bit 1 = lerp the level fog (`fun_001ee4b0`).
    pub flags: u32,
    /// 0x54 / 0x58: fog colour at t = 0 / t = 1 (bytes r, g, b; the fourth byte is unused).
    pub fog_color: [u32; 2],
    /// 0x5c..0x6b (t = 0) and 0x6c..0x7b (t = 1).
    pub side: [FogZoneSide; 2],
    /// 0x7c: not read by any of the three readers (0 on Novalis).
    pub unused_7c: u32,
}
const _: () = assert!(std::mem::size_of::<FogZone>() == FOG_ZONE_SIZE);

impl FogZone {
    /// `flags & 2`: the zone lerps the level fog.
    pub fn lerps_fog(&self) -> bool { self.flags & 2 != 0 }
    /// `flags & 1`: the zone cross-fades the hero's lighting.
    pub fn lerps_hero_light(&self) -> bool { self.flags & 1 != 0 }
    /// Fog colour of one side as (r, g, b).
    pub fn fog_rgb(&self, side: usize) -> [u8; 3] {
        let c = self.fog_color[side];
        [c as u8, (c >> 8) as u8, (c >> 16) as u8]
    }
}

/// Section 0x80 as the game holds it: `circles[i]` (0x17f140) = `(x, y, z, r²)`, a 2-D bounding circle in
/// XY (z is not read), and `zones[i]` (0x17f340) the record it guards.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FogZones {
    pub circles: Vec<[f32; 4]>,
    pub zones: Vec<FogZone>,
}

/// Reads section 0x80: `s32 count, pad[3]`, `count` circles (16 bytes), then `count` records (0x80 bytes).
/// Applies the loader's fixup (hero light < 0 → 0xb). An absent section (pointer 0) or count 0 gives none.
pub fn parse_fog_zones(gameplay: &[u8]) -> Result<FogZones> {
    let g = Buf(gameplay);
    let s = g.u32(FOG_ZONES_POINTER)? as usize;
    if s == 0 { return Ok(FogZones::default()); }
    let count = g.i32(s)?;
    if count < 0 { return invalid("negative fog zone count"); }
    let n = count as usize;
    let circles = g.pod_slice::<[f32; 4]>(s + 0x10, n, "fog zone circles")?;
    let mut zones = g.pod_slice::<FogZone>(s + 0x10 + 16 * n, n, "fog zone records")?;
    for z in &mut zones {
        for l in &mut z.hero_light {
            if *l < 0 { *l = 0xb; }
        }
    }
    Ok(FogZones { circles, zones })
}

/// One of the level's static point lights (section `point_lights`, 0x20 bytes from +0x10; the loader's pointer
/// 0x15fb80): position (x, y, z) and its squared radius (w), the colour +0x10 (bytes r, g, b, a).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StaticLight {
    pub pos: [f32; 4],
    pub rgba: u32,
}

/// The level's static point lights and their grid (section `point_light_grid` past its 0x10-byte header, the loader's
/// pointer 0x15fbc0): 64 × 64 cells of 16 units (word `(y >> 4)·64 + (x >> 4)`: the byte offset from the cells' start
/// of the cell's list `s32 count, count × s32 light index`, 0 = none), kept as words.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StaticLights {
    pub lights: Vec<StaticLight>,
    pub grid: Vec<u32>,
}

/// Reads the sections `point_lights` (`s32 count, pad[3]`, then the records) and `point_light_grid`; absent ones give
/// none.
pub fn parse_static_lights(gameplay: &[u8]) -> Result<StaticLights> {
    let secs = sections(gameplay)?;
    let range = |n: &str| secs.iter().find(|s| s.0 == n).map(|s| s.1.clone());
    let mut out = StaticLights::default();
    if let Some(r) = range("point_lights") {
        let b = Buf(&gameplay[r.clone()]);
        let n = b.i32(0)?.max(0) as usize;
        for k in 0..n {
            let o = 0x10 + 0x20 * k;
            let f = |j: usize| b.u32(o + 4 * j).map(f32::from_bits);
            out.lights.push(StaticLight { pos: [f(0)?, f(1)?, f(2)?, f(3)?], rgba: b.u32(o + 0x10)? });
        }
    }
    // A 0x10-byte header (the lists' size, pad[3]) before the cells: `InitLevelRenderGlobals` copies from +0x10 and
    // 0x15fbc0 points there, so the cells and the lists' offsets are relative to +0x10.
    if let Some(r) = range("point_light_grid") {
        let body = gameplay.get(r.start + 0x10..r.end).unwrap_or(&[]);
        out.grid = body.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(o_class: i32, pos: [f32; 3], rot: [f32; 3], color: [i32; 3], light: i32) -> Vec<u8> {
        let m = MobyInstance {
            size: 0x78,
            unknown_4: -1,
            spawn_id: 7,
            o_class,
            scale: 1.5,
            draw_distance: 64,
            update_distance: 32,
            unused_28: 32,
            unused_2c: 64,
            position: pos,
            rotation: rot,
            group: -1,
            pvar_index: -1,
            occlusion: 1,
            mode_bits: 0x20,
            color,
            light,
            unknown_74: -1,
            ..Default::default()
        };
        bytemuck::bytes_of(&m).to_vec()
    }

    #[test]
    fn parses_synthetic_section() {
        let mut g = vec![0u8; 0x100];
        g[MOBY_INSTANCES_POINTER..MOBY_INSTANCES_POINTER + 4].copy_from_slice(&0x100u32.to_le_bytes());
        g.extend_from_slice(&2i32.to_le_bytes());
        g.extend_from_slice(&256i32.to_le_bytes());
        g.extend_from_slice(&[0; 8]);
        g.extend(record(11, [1.0, 2.0, 3.0], [0.0, 0.0, 1.5], [41, 42, 43], 0x0f0f));
        g.extend(record(726, [-4.0, 5.0, 6.5], [0.1, -0.2, 0.3], [1, 2, 3], 2));
        let v = parse_moby_instances(&g).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].o_class, 11);
        assert_eq!(v[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(v[0].rotation, [0.0, 0.0, 1.5]);
        assert_eq!(v[0].ambient_rgb(), [41, 42, 43]);
        assert_eq!(v[0].light_word().to_le_bytes(), [15, 15, 0, 0]);
        assert_eq!(v[1].o_class, 726);
        assert_eq!(v[1].scale, 1.5);
        assert_eq!(v[1].mode(), 0x20);
        assert_eq!(v[1].light, 2);

        // Wrong record size is rejected; a zero section pointer means no instances.
        let mut bad = g.clone();
        bad[0x110] = 0x70;
        assert!(parse_moby_instances(&bad).is_err());
        let mut none = g.clone();
        none[MOBY_INSTANCES_POINTER..MOBY_INSTANCES_POINTER + 4].copy_from_slice(&[0; 4]);
        assert!(parse_moby_instances(&none).unwrap().is_empty());
        // Truncated data is an error, not a panic.
        assert!(parse_moby_instances(&g[..g.len() - 1]).is_err());
    }

    /// Synthetic gameplay file: 3 instances (pvar 1, −1, 0), a pvar table with 3 entries, both fixup lists,
    /// two splines and level settings.
    fn pvar_file() -> Vec<u8> {
        let mut g = vec![0u8; 0x100];
        let put = |g: &mut Vec<u8>, at: usize, v: i32| g[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let append = |g: &mut Vec<u8>, words: &[i32]| -> usize {
            let at = g.len();
            for w in words { g.extend_from_slice(&w.to_le_bytes()); }
            at
        };
        // Instances.
        let at = append(&mut g, &[3, 0, 0, 0]);
        put(&mut g, MOBY_INSTANCES_POINTER, at as i32);
        for (cls, pv) in [(459, 1i32), (500, -1), (1818, 0)] {
            let mut r = record(cls, [0.0; 3], [0.0; 3], [0; 3], 0);
            r[0x58..0x5c].copy_from_slice(&pv.to_le_bytes());
            g.extend(r);
        }
        // Pvar data: block 0 = 16 bytes, block 1 = 8 bytes, block 2 = 4 bytes (unused by instances).
        let data = append(&mut g, &[2, 7, -1, 0x10, 0, 0x08, 0x77]);
        let table = append(&mut g, &[0, 16, 16, 8, 24, 4]);
        put(&mut g, PVAR_DATA_POINTER, data as i32);
        put(&mut g, PVAR_TABLE_POINTER, table as i32);
        // Links: block 0 +0 (instance 2), +4 (instance 7: past the list), +8 (-1 kept); block 1 +0 (instance 0).
        let links = append(&mut g, &[0, 0, 0, 4, 0, 8, 1, 0, -1, -1]);
        put(&mut g, PVAR_MOBY_LINK_FIXUPS_POINTER, links as i32);
        // Pointers: block 0 +12, block 1 +4 (kept block-relative).
        let ptrs = append(&mut g, &[0, 12, 1, 4, -1, -1]);
        put(&mut g, PVAR_POINTER_FIXUPS_POINTER, ptrs as i32);
        // Paths: 2 splines of 1 and 2 points.
        let paths = g.len();
        put(&mut g, PATHS_POINTER, paths as i32);
        append(&mut g, &[2, 0x20, 0x50, 0, 0, 0x20, 0, 0]);
        let f = |v: f32| v.to_bits() as i32;
        append(&mut g, &[1, 0, 0, 0, f(1.0), f(2.0), f(3.0), f(-1.0)]);
        append(&mut g, &[2, 0, 0, 0, f(4.0), f(5.0), f(6.0), f(1.0), f(7.0), f(8.0), f(9.0), f(-1.0)]);
        // Level settings: ship at (167.25, 125.5, 61.0), yaw pi/2.
        let ls = append(&mut g, &[0; 16]);
        put(&mut g, LEVEL_SETTINGS_POINTER, ls as i32);
        for (k, v) in [167.25f32, 125.5, 61.0, std::f32::consts::FRAC_PI_2].iter().enumerate() { put(&mut g, ls + 0x2c + 4 * k, f(*v)); }
        g
    }

    fn word(b: &[u8], o: usize) -> i32 { i32::from_le_bytes(b[o..o + 4].try_into().unwrap()) }

    #[test]
    fn pvar_fixups_as_the_loader() {
        let g = pvar_file();
        let fx = parse_pvar_fixups(&g).unwrap();
        assert_eq!(fx.moby_links, vec![(0, 0), (0, 4), (0, 8), (1, 0)]);
        assert_eq!(fx.pointers, vec![(0, 12), (1, 4)]);

        // All spawned: instance i -> runtime i; out-of-range index -> -1; negative kept; pointers kept.
        let pv = parse_pvars(&g).unwrap();
        assert_eq!(pv.len(), 2); // highest index referenced = 1
        let b0 = pv[0].as_deref().unwrap();
        assert_eq!([word(b0, 0), word(b0, 4), word(b0, 8), word(b0, 12)], [2, -1, -1, 0x10]);
        let b1 = pv[1].as_deref().unwrap();
        assert_eq!([word(b1, 0), word(b1, 4)], [0, 0x08]);

        // Instance 1 not spawned: instance 2 becomes runtime 1, instance 0 stays 0.
        let pv = parse_pvars_spawned(&g, &[true, false, true]).unwrap();
        assert_eq!(word(pv[0].as_deref().unwrap(), 0), 1);
        let pv = parse_pvars_spawned(&g, &[false, true, true]).unwrap();
        assert_eq!(word(pv[1].as_deref().unwrap(), 0), -1);

        // Lookup through the instance record.
        let inst = parse_moby_instances(&g).unwrap();
        let pv = parse_pvars(&g).unwrap();
        assert_eq!(inst[0].pvar(&pv).map(|b| b.len()), Some(8));
        assert_eq!(inst[1].pvar(&pv), None);
        assert_eq!(inst[2].pvar(&pv).map(|b| b.len()), Some(16));

        // A fixup past its block is an error; no data section means no blocks.
        let mut bad = g.clone();
        let links = word(&g, PVAR_MOBY_LINK_FIXUPS_POINTER) as usize;
        bad[links + 12..links + 16].copy_from_slice(&14i32.to_le_bytes());
        assert!(parse_pvars(&bad).is_err());
        let mut none = g.clone();
        none[PVAR_DATA_POINTER..PVAR_DATA_POINTER + 4].copy_from_slice(&[0; 4]);
        assert!(parse_pvars(&none).unwrap().is_empty());
    }

    #[test]
    fn splines_and_ship_placement() {
        let g = pvar_file();
        let s = parse_splines(&g).unwrap();
        assert_eq!(s, vec![vec![[1.0, 2.0, 3.0, -1.0]], vec![[4.0, 5.0, 6.0, 1.0], [7.0, 8.0, 9.0, -1.0]]]);
        assert_eq!(ship_placement(&g).unwrap(), Some(([167.25, 125.5, 61.0], std::f32::consts::FRAC_PI_2)));
        let mut no_ship = g.clone();
        let ls = word(&g, LEVEL_SETTINGS_POINTER) as usize;
        no_ship[ls + 0x2c..ls + 0x30].copy_from_slice(&0f32.to_le_bytes());
        assert_eq!(ship_placement(&no_ship).unwrap(), None);
    }

    #[test]
    fn fog_zones_synthetic() {
        let mut g = vec![0u8; 0x100];
        g[FOG_ZONES_POINTER..FOG_ZONES_POINTER + 4].copy_from_slice(&0x100u32.to_le_bytes());
        g.extend_from_slice(&2i32.to_le_bytes());
        g.extend_from_slice(&[0; 12]);
        for c in [[1.0f32, 2.0, 3.0, 16.0], [-5.0, 6.0, 0.0, 4.0]] { g.extend(c.iter().flat_map(|v| v.to_le_bytes())); }
        let a = FogZone {
            inverse: [[0.5, 0.0, 0.0, 0.0], [0.0, 0.25, 0.0, 0.0], [0.0, 0.0, 0.125, 0.0], [-1.0, 2.0, 3.0, 1.0]],
            hero_color: [0x261919, 0x282828],
            hero_light: [1, -1],
            flags: 3,
            fog_color: [0x0f0505, 0xb37e69],
            side: [
                FogZoneSide { near_dist: 0.0, near_density: 0.0, far_dist: 50.0, far_density: 0.8 },
                FogZoneSide { near_dist: 0.0, near_density: 0.0, far_dist: 240.0, far_density: 0.6 },
            ],
            unused_7c: 0,
        };
        let b = FogZone { hero_light: [-1, -2], flags: 2, ..a };
        g.extend_from_slice(bytemuck::bytes_of(&a));
        g.extend_from_slice(bytemuck::bytes_of(&b));
        let z = parse_fog_zones(&g).unwrap();
        assert_eq!(z.circles, vec![[1.0, 2.0, 3.0, 16.0], [-5.0, 6.0, 0.0, 4.0]]);
        assert_eq!(z.zones.len(), 2);
        assert_eq!(z.zones[0].hero_light, [1, 0xb], "loader turns -1 into 0xb");
        assert_eq!(z.zones[1].hero_light, [0xb, 0xb]);
        assert_eq!(z.zones[0].inverse, a.inverse);
        assert_eq!((z.zones[0].fog_rgb(0), z.zones[0].fog_rgb(1)), ([5, 5, 15], [105, 126, 179]));
        assert!(z.zones[0].lerps_fog() && z.zones[0].lerps_hero_light() && !z.zones[1].lerps_hero_light());
        assert_eq!(z.zones[1].side[1].far_dist, 240.0);
        // Truncated, absent.
        assert!(parse_fog_zones(&g[..g.len() - 1]).is_err());
        let mut none = g.clone();
        none[FOG_ZONES_POINTER..FOG_ZONES_POINTER + 4].copy_from_slice(&[0; 4]);
        assert_eq!(parse_fog_zones(&none).unwrap(), FogZones::default());
    }

    /// Novalis (level 01, NTSC gameplay): the 11 records of docs/plan/world_animation.md §5.
    #[test]
    fn fog_zones_novalis_disc() {
        let path = crate::test_data::root().join("levels/01/gameplay_ntsc.bin");
        let Ok(wad) = std::fs::read(&path) else { eprintln!("skipped: no {}", path.display()); return };
        let g = crate::wad::decompress(&wad).unwrap();
        let z = parse_fog_zones(&g).unwrap();
        assert_eq!((z.circles.len(), z.zones.len()), (11, 11));
        // (circle centre x, y, radius), flags, side-1 rgb, side-1 (near dist, near d, far dist, far d), side-2 rgb, side 2.
        type Row = ([f32; 3], u32, [u8; 3], [f32; 4], [u8; 3], [f32; 4]);
        let outdoor_a = ([105, 126, 179], [0.0, 0.0, 240.0, 0.6]);
        let outdoor_b = ([105, 125, 179], [0.0, 0.0, 240.0, 0.6]);
        let rows: [Row; 11] = [
            ([147.8, 130.7, 9.2], 3, [5, 5, 15], [0.0, 0.0, 50.0, 0.8], outdoor_a.0, outdoor_a.1),
            ([177.6, 83.8, 7.9], 3, [5, 5, 15], [0.0, 0.0, 50.0, 0.8], outdoor_a.0, outdoor_a.1),
            ([186.0, 127.9, 14.4], 1, [0, 39, 75], [20.0, 0.25, 50.0, 0.6], outdoor_a.0, outdoor_a.1),
            ([260.9, 178.5, 6.4], 3, [0, 44, 120], [15.0, 0.0, 100.0, 0.4], outdoor_a.0, outdoor_a.1),
            ([278.9, 192.3, 6.6], 3, [0, 44, 120], [15.0, 0.0, 100.0, 0.4], outdoor_a.0, outdoor_a.1),
            ([227.8, 115.5, 4.6], 2, [0, 44, 120], [15.0, 0.0, 100.0, 0.4], outdoor_b.0, outdoor_b.1),
            ([229.1, 125.2, 5.9], 2, [0, 44, 120], [15.0, 0.0, 100.0, 0.4], outdoor_b.0, outdoor_b.1),
            ([45.1, 147.6, 5.5], 2, [72, 83, 12], [0.0, 0.0, 30.0, 0.75], outdoor_b.0, outdoor_b.1),
            ([51.5, 135.9, 5.1], 2, [72, 83, 12], [0.0, 0.0, 30.0, 0.74], outdoor_b.0, outdoor_b.1),
            ([53.3, 126.9, 5.7], 2, [72, 83, 12], [0.0, 0.0, 30.0, 1.0], [72, 83, 12], [0.0, 0.0, 30.0, 0.5]),
            ([36.7, 116.6, 6.6], 2, [72, 83, 12], [0.0, 0.0, 30.0, 1.0], [72, 83, 12], [0.0, 0.0, 25.0, 0.5]),
        ];
        let side = |s: &FogZoneSide| [s.near_dist, s.near_density, s.far_dist, s.far_density];
        let close = |a: [f32; 4], b: [f32; 4]| a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-4);
        for (i, (c, flags, rgb1, s1, rgb2, s2)) in rows.iter().enumerate() {
            let (circle, zone) = (z.circles[i], &z.zones[i]);
            assert!((circle[0] - c[0]).abs() < 0.06 && (circle[1] - c[1]).abs() < 0.06, "zone {i} circle {circle:?}");
            assert!((circle[3].sqrt() - c[2]).abs() < 0.05, "zone {i}: w = r² ({})", circle[3]);
            assert_eq!(zone.flags, *flags, "zone {i} flags");
            assert_eq!((zone.fog_rgb(0), zone.fog_rgb(1)), (*rgb1, *rgb2), "zone {i} colours");
            assert!(close(side(&zone.side[0]), *s1) && close(side(&zone.side[1]), *s2), "zone {i} sides {:?}", zone.side);
            assert_eq!(zone.inverse[3][3], 1.0, "zone {i}: affine inverse");
        }
        // Hero lights: 0–1 bank 1 → 0, 2–4 bank 2 → 0, 7–8 bank 3 → 0, the rest none (−1 → 0xb).
        let lights: Vec<[i32; 2]> = z.zones.iter().map(|z| z.hero_light).collect();
        assert_eq!(lights, [[1, 0], [1, 0], [2, 0], [2, 0], [2, 0], [0xb, 0xb], [0xb, 0xb], [3, 0], [3, 0], [0xb, 0xb], [0xb, 0xb]]);
        assert_eq!(z.zones[0].hero_color, [0x261919, 0x282828]);
        assert_eq!(z.zones[2].hero_color, [0x332e2a, 0x282828]);
    }
}
