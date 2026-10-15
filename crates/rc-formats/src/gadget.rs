//! RAC1 gadget classes: Ratchet's hand-held weapons and gadgets (the wrench among them). Spec:
//! docs/formats/moby_rac1.md section 0.4. Ported from the retired C++ reference extractor
//! (git 2230812); `tests/formats/golden.rs` checks every section against the committed snapshot table (`data/loader_snapshots.tsv`).
//!
//! Layout. The core index header's `gadget_count` (+0x80) / `gadget_offset` (+0x84) give a table of
//! [`GadgetEntry`] records (0x10 bytes: `offset_in_asset_wad`, `class_number` = o_class,
//! `compressed_size`, `pad` = 0). Each entry points at one WAD stream inside the decompressed core
//! data (`core_data[offset .. offset + compressed_size]`, the stream's own header repeats the size);
//! it decompresses to an ordinary moby class blob ([`parse_moby_class`]). The class also has a normal
//! moby class table entry with `offset_in_asset_wad = 0`, whose `textures[16]` map its GS texture
//! slots into the **moby** texture table, exactly like any other moby class, so
//! [`LevelMobyClass::texture_table_index`] applies unchanged and the texture pixels are the
//! `moby/NNN_...` entries of [`crate::texture::parse_textures`].
//!
//! How the game uses it (level01.elf = the in-game engine; boot ELF addresses in brackets):
//! * `FUN_00258128` (the level core loader) copies the table into four arrays indexed by gadget
//!   number: o_class (0x1b0040), pointer to the compressed stream (0x1b00a0), compressed size
//!   (0x1b0100) and the 16 texture slots of the class's moby table entry (0x1b0160, looked up
//!   through the o_class -> class-slot map at 0x198040). The count goes to 0x160008. The per-class
//!   sound remap of a class without geometry is parked in 0x1b02e0 (16 × s16 per gadget).
//! * `select_world_object_resource_tables` (0x259788 [0x204a40]) finds the gadget by o_class,
//!   WAD-decompresses it into one of two alternating 0x18000-byte buffers (0x174290 + n·0x18000),
//!   stores the buffer as the class's slot in the moby class table (0x197780[slot]), relocates it
//!   with the moby texture table and the 16 slots (`fun_00203338`) and applies the parked sound
//!   remap. Only the gadget in Ratchet's hand (and the one it replaces) is resident at a time.
//! * `LoadHandGadget` (0x297d70 [0x224368]) calls it when the hand item changes and spawns the moby.

use crate::buf::{invalid, Buf, FormatError, Result};
use crate::level::{GadgetEntry, LevelCore, TextureEntry};
use crate::moby::{parse_moby_class, LevelMobyClass, MobyClassHeader};
use crate::texture::{decode_indexed8, Texture};
use crate::wad;

/// Size of each of the game's two gadget decompression buffers; every class on the disc fits.
pub const GADGET_BUFFER_SIZE: usize = 0x18000;

/// Ratchet's moby class (class slot 0; its sequences are the core blocks `ratchet_seq/NNN`).
pub const RATCHET_O_CLASS: i32 = 0;
/// The wrench, a gadget-table class on every level (gadget definition 8 in the game's table).
pub const WRENCH_O_CLASS: i32 = 71;
/// Ratchet's joint list (class header `joints`, spec 3.3) the hand item hangs from: the game's
/// gadget definition `+0x00` for the wrench and every hand weapon except the glove types (6).
pub const HAND_JOINT_LIST: usize = 0;

/// One decompressed gadget class.
#[derive(Clone, Debug)]
pub struct GadgetClass {
    /// The gadget table entry.
    pub entry: GadgetEntry,
    /// The decompressed class blob (sequence pointers etc. are relative to it).
    pub blob: Vec<u8>,
    /// The parsed class with its moby class table entry (texture slots).
    pub moby: LevelMobyClass,
}

/// Parses every gadget class of a level in gadget-table order. `core_data` is the decompressed core
/// data. Fails if a stream is not a WAD of the table's size or a class has no moby table entry.
pub fn parse_gadget_classes(core: &LevelCore, core_data: &[u8]) -> Result<Vec<GadgetClass>> {
    let mut out = Vec::with_capacity(core.gadgets.len());
    for g in &core.gadgets {
        let o = g.class_number;
        if g.offset_in_asset_wad <= 0 || (g.compressed_size as usize) < wad::WAD_HEADER_SIZE {
            return invalid(format!("gadget {o}: bad table entry"));
        }
        let stream = Buf(core_data).sub(g.offset_in_asset_wad as usize, g.compressed_size as usize, "gadget stream")?;
        if !wad::is_wad(stream.bytes()) || wad::compressed_size(stream.bytes())? != g.compressed_size as u32 {
            return invalid(format!("gadget {o}: not a WAD stream of the table's size"));
        }
        let Some(entry) = core.moby_classes.iter().find(|e| e.o_class == o) else {
            return invalid(format!("gadget {o}: no moby class table entry"));
        };
        let blob = wad::decompress(stream.bytes())?;
        let class = parse_moby_class(&blob).map_err(|err| FormatError::Invalid(format!("gadget {o}: {err}")))?;
        out.push(GadgetClass { entry: *g, blob, moby: LevelMobyClass { o_class: o, entry: *entry, class } });
    }
    Ok(out)
}

/// [`parse_gadget_classes`] without the blobs, for code that handles them like level moby classes.
pub fn parse_level_gadgets(core: &LevelCore, core_data: &[u8]) -> Result<Vec<LevelMobyClass>> {
    Ok(parse_gadget_classes(core, core_data)?.into_iter().map(|g| g.moby).collect())
}

/// One used texture slot of a moby class entry.
#[derive(Clone, Debug)]
pub struct ClassTexture {
    /// GS texture slot of the class (`MobyTriangle::texture`), 0..16.
    pub slot: usize,
    /// Index into `LevelCore::moby_textures`.
    pub index: usize,
    /// None when the moby texture entry has no pixels (non-positive size or negative offsets).
    pub texture: Option<Texture>,
}

/// Decodes the moby textures a class uses (every `entry.textures[k] != 0xff`), in slot order.
/// Works for any [`LevelMobyClass`]; `gs_ram` is the raw gs_ram lump.
pub fn class_textures(core: &LevelCore, core_data: &[u8], gs_ram: &[u8], class: &LevelMobyClass) -> Result<Vec<ClassTexture>> {
    let (data, gs) = (Buf(core_data), Buf(gs_ram));
    let mut out = Vec::new();
    for (slot, &ix) in class.entry.textures.iter().enumerate() {
        if ix == 0xff { continue; }
        let index = ix as usize;
        let Some(e): Option<&TextureEntry> = core.moby_textures.get(index) else {
            return invalid(format!("class {}: texture slot {slot} past the moby texture table", class.o_class));
        };
        let texture = if e.width <= 0 || e.height <= 0 || e.data_offset < 0 || e.palette < 0 { None } else {
            let px = data.sub(core.header.textures_base_offset as usize + e.data_offset as usize, e.width as usize * e.height as usize, "texture pixels")?;
            let clut = gs.sub(e.palette as usize * 0x100, 1024, "texture palette")?;
            Some(decode_indexed8(px.bytes(), e.width as u32, e.height as u32, clut.bytes())?)
        };
        out.push(ClassTexture { slot, index, texture });
    }
    Ok(out)
}

/// The two byte lists of joint list `list` of a class (spec 3.3: header `joints` -> `s32 count`,
/// `s32 pointer[count]`; each list `s16 n1, s16 n2, u8 a[n1], u8 b[n2], 0xff`).
///
/// The first list is a root-to-joint chain; its **last** entry is the joint the game attaches to:
/// `fun_0020cca8` (boot 0x20cca8, L01 0x264508) passes the list to `fun_00210850` (boot 0x210850),
/// which evaluates the pose of exactly those joints and returns the pose matrix of the last one.
pub fn joint_list(blob: &[u8], header: &MobyClassHeader, list: usize) -> Result<(Vec<u8>, Vec<u8>)> {
    let b = Buf(blob);
    if header.joints <= 0 { return invalid("class has no joint lists"); }
    let base = header.joints as usize;
    let count = b.i32(base)?;
    if list >= count.max(0) as usize { return invalid(format!("joint list {list} of {count}")); }
    let p = b.i32(base + 4 + 4 * list)?;
    if p <= 0 { return invalid("null joint list pointer"); }
    let p = p as usize;
    let (n1, n2) = (b.i16(p)?, b.i16(p + 2)?);
    if n1 < 0 || n2 < 0 { return invalid("negative joint list length"); }
    let (n1, n2) = (n1 as usize, n2 as usize);
    let a = b.sub(p + 4, n1, "joint list")?.bytes().to_vec();
    let c = b.sub(p + 4 + n1, n2, "joint list")?.bytes().to_vec();
    if b.u8(p + 4 + n1 + n2)? != 0xff { return invalid("joint list without 0xff terminator"); }
    Ok((a, c))
}

/// The joint an attachment list resolves to (last entry of the first byte list of [`joint_list`]).
pub fn attachment_joint(blob: &[u8], header: &MobyClassHeader, list: usize) -> Result<u8> {
    let (chain, _) = joint_list(blob, header, list)?;
    match chain.last() {
        Some(&j) if j < header.joint_count => Ok(j),
        Some(&j) => invalid(format!("attachment joint {j} >= joint count {}", header.joint_count)),
        None => invalid("empty attachment chain"),
    }
}
