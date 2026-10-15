//! Textures → PNG + JSON sidecar. Every RAC1 texture is 8-bit indexed (PSMT8) with a 256 × RGBA32 CLUT in CSM1
//! order and PS2 alpha (0x80 = opaque). The PNG is indexed too: the pixel indices are the stored ones, `PLTE`/`tRNS`
//! the CLUT in linear order with alpha scaled like `rc_formats::texture::decode_indexed8` (0x80 → 255), so a PNG
//! decoder shows exactly what the loaders decode. The sidecar keeps what the PNG cannot: the CLUT as stored (raw
//! alpha, CSM1 order), the texture-table entry and the GS TEX0 format, so a future importer can restore the bytes.

use super::data::{self, LevelData};
use super::jsonv::{hex_bytes, Obj, J};
use super::png;
use super::{Kinds, Out};
use crate::{Code, Error};
use rc_formats::level::TextureEntry;
use rc_formats::texture::{self, IndexedImage, LevelTexture, TextureSource, TextureTable};
use rc_formats::{hud, particle_tex, sky};
use std::collections::HashMap;
use std::path::Path;

/// An exported texture as geometry exports reference it.
#[derive(Clone, Debug)]
pub(crate) struct TexInfo {
    /// Export-root relative path of the PNG.
    pub path: String,
    /// Some texel has alpha below 0x80 (PNG alpha < 255).
    pub has_alpha: bool,
}

/// The level's textures by table and index (what class entries and ad-gifs refer to), and sky textures by index.
#[derive(Default)]
pub(crate) struct TexIndex {
    pub table: HashMap<(TextureTable, usize), TexInfo>,
    pub sky: HashMap<usize, TexInfo>,
}

pub(crate) fn dir(level: u32) -> String { format!("textures/levels/{level:02}") }

fn has_alpha(rgba: &[u8]) -> bool { rgba.as_chunks::<4>().0.iter().any(|p| p[3] < 255) }

/// Paths and alpha of every level texture, computed with the same loaders (no files written).
pub(crate) fn index_level(ld: &LevelData) -> Result<TexIndex, Error> {
    let mut ix = TexIndex::default();
    let texs = texture::parse_textures(&ld.core, &ld.core_data, &ld.gs_ram).map_err(|e| data::damaged(&ld.rel("core_data.bin"), e))?;
    for t in &texs {
        ix.table.insert((t.table, t.index), TexInfo { path: format!("{}/{}.png", dir(ld.id), t.key()), has_alpha: has_alpha(&t.texture.rgba) });
    }
    if let Ok(Some(block)) = sky::sky_block(&ld.core, &ld.core_data) {
        if let Ok(s) = sky::parse_sky_block(block) {
            for st in sky::parse_sky_textures(block, &s).unwrap_or_default() {
                ix.sky.insert(st.index, TexInfo { path: format!("{}/{}.png", dir(ld.id), st.key()), has_alpha: has_alpha(&st.texture.rgba) });
            }
        }
    }
    Ok(ix)
}

/// PNG palette of a stored CLUT: linear order, alpha scaled like `decode_indexed8`.
fn palette(clut: &[u8]) -> Vec<[u8; 4]> {
    (0..256u32)
        .map(|i| {
            let e = &clut[texture::clut_index(i) as usize * 4..][..4];
            [e[0], e[1], e[2], texture::scale_alpha(e[3])]
        })
        .collect()
}

fn log2(v: u32) -> u32 { 31 - v.max(1).leading_zeros() }

/// The fields every texture sidecar has: size, the stored CLUT, and the GS TEX0 format.
fn base_sidecar(img: &IndexedImage, source: &str) -> Obj {
    Obj::new()
        .set("source", source)
        .set("width", img.width)
        .set("height", img.height)
        .set("png", "8-bit indexed; pixel values are the stored PSMT8 indices; PLTE/tRNS = the CLUT in linear order (CSM1 swizzle undone)")
        .set("alpha", Obj::new().set("ps2_opaque", 0x80).set("png_from_ps2", "a < 0x80 ? 2a : 255").set("note", "the stored alpha of every CLUT entry is in clut_csm1"))
        .set("clut_csm1", hex_bytes(img.clut))
        .set(
            "gs",
            Obj::new().set("psm", "PSMT8").set("cpsm", "PSMCT32").set("csm", "CSM1").set("tcc", 1).set("tfx", "MODULATE")
                .set("tw", log2(img.width)).set("th", log2(img.height)),
        )
}

/// Writes one indexed image as PNG + sidecar. `expect` is the loader's decode of the same image, checked texel
/// for texel against what the PNG will show.
fn write_image(out: &Out, rel: &str, img: &IndexedImage, expect: Option<&[u8]>, sidecar: Obj) -> Result<(), Error> {
    let pal = palette(img.clut);
    if let Some(rgba) = expect {
        let ok = rgba.len() == img.indices.len() * 4 && img.indices.iter().zip(rgba.as_chunks::<4>().0).all(|(&i, p)| pal[i as usize] == *p);
        if !ok { return Err(Error::new(Code::Internal, format!("{rel}: indexed export differs from the loader's decode"))); }
    }
    out.write(rel, &png::encode_indexed(img.width, img.height, img.indices, &pal))?;
    out.json(&rel.replace(".png", ".json"), &sidecar.build())
}

fn entry_json(e: &TextureEntry) -> Obj {
    Obj::new().set("data_offset", e.data_offset).set("width", e.width).set("height", e.height).set("ty", e.ty)
        .set("palette", e.palette).set("mipmap", e.mipmap).set("pad", e.pad)
}

fn level_texture(out: &Out, ld: &LevelData, t: &LevelTexture) -> Result<(), Error> {
    let rel = format!("{}/{}.png", dir(ld.id), t.key());
    let (img, mips, mut side) = match t.source {
        TextureSource::Entry(e) => {
            let img = texture::entry_image(&ld.core, &ld.core_data, &ld.gs_ram, &e).map_err(|x| data::damaged(&ld.rel("core_data.bin"), x))?;
            let mips = if t.table == TextureTable::Tfrag { texture::tfrag_mip_images(&ld.core, &ld.core_data, &ld.gs_ram, &e).ok() } else { None };
            let side = base_sidecar(&img, &ld.rel("core_data.bin")).set("table", t.table.name()).set("index", t.index).set("entry", entry_json(&e))
                .set("gs_ram_blocks", "palette, mipmap and pad are 0x100-byte blocks of levels/NN/gs_ram.bin");
            (img, mips, side)
        }
        TextureSource::Billboard { o_class, info } => {
            let img = texture::billboard_image(&ld.gs_ram, &info).map_err(|x| data::damaged(&ld.rel("gs_ram.bin"), x))?;
            let mips = texture::billboard_mip_images(&ld.gs_ram, &info).ok();
            let side = base_sidecar(&img, &ld.rel("gs_ram.bin")).set("table", "billboard").set("shrub_class_index", t.index).set("o_class", o_class)
                .set("billboard", Obj::new().set("width", info.width).set("height", info.height).set("max_mip", info.max_mip)
                    .set("palette_offset", info.palette_offset).set("texture_offset", info.texture_offset).set("mip1", info.mip1).set("mip2", info.mip2).set("mip3", info.mip3));
            (img, mips, side)
        }
    };
    let mut mip_files = Vec::new();
    for (k, m) in mips.iter().flatten().enumerate().skip(1) {
        let mrel = rel.replace(".png", &format!(".mip{k}.png"));
        out.write(&mrel, &png::encode_indexed(m.width, m.height, m.indices, &palette(m.clut)))?;
        mip_files.push(J::from(mrel.rsplit('/').next().unwrap_or(&mrel)));
    }
    if !mip_files.is_empty() { side.put("mips", J::Arr(mip_files)); }
    write_image(out, &rel, &img, Some(&t.texture.rgba), side)
}

/// Every texture of one level: the four texture tables and the shrub billboards (with mip chains), the sky, the
/// particle and FX banks, and the HUD frames.
pub(crate) fn export_level(data: &Path, out: &Out, id: u32) -> Result<(), Error> {
    let ld = LevelData::load(data, id)?;
    let texs = texture::parse_textures(&ld.core, &ld.core_data, &ld.gs_ram).map_err(|e| data::damaged(&ld.rel("core_data.bin"), e))?;
    for t in &texs { level_texture(out, &ld, t)?; }

    // Sky.
    match sky::sky_block(&ld.core, &ld.core_data).and_then(|b| b.map(|b| sky::parse_sky_block(b).map(|s| (b, s))).transpose()) {
        Ok(Some((block, s))) => {
            for (i, def) in s.texture_defs.iter().enumerate() {
                let r = sky::sky_texture_image(block, &s, i).and_then(|img| Ok((img, img.decode()?)));
                match r {
                    Ok((img, rgba)) => {
                        let rel = format!("{}/sky/{:02}_{}x{}.png", dir(id), i, def.width, def.height);
                        let side = base_sidecar(&img, &ld.rel("core_data.bin")).set("table", "sky").set("index", i)
                            .set("def", Obj::new().set("palette_offset", def.palette_offset).set("texture_offset", def.texture_offset).set("width", def.width).set("height", def.height));
                        write_image(out, &rel, &img, Some(&rgba.rgba), side)?;
                    }
                    Err(e) => out.skip(format!("level {id:02} sky texture {i}: {e}")),
                }
            }
        }
        Ok(None) => {}
        Err(e) => out.skip(format!("level {id:02} sky: {e}")),
    }

    // Particle and FX textures.
    match particle_tex::parse_particle_textures(&ld.core, &ld.core_index, &ld.core_data) {
        Ok(p) => {
            let part = if p.entries.is_empty() { Ok(&[][..]) } else { particle_tex::core_bank(&ld.core, &ld.core_data, "part_bank") };
            for (i, e) in p.entries.iter().enumerate() {
                match part.as_ref().map_err(|e| e.to_string()).and_then(|b| particle_tex::bank_texture_image(b, e.palette, e.texture, e.side, e.side).map_err(|e| e.to_string())) {
                    Ok(img) => {
                        let rel = format!("{}/particle/{:03}_{}x{}.png", dir(id), i, e.side, e.side);
                        let side = base_sidecar(&img, &ld.rel("core_data.bin")).set("table", "part_textures").set("index", i)
                            .set("entry", Obj::new().set("palette", e.palette).set("unk4", e.unk4).set("texture", e.texture).set("side", e.side));
                        write_image(out, &rel, &img, Some(&p.textures[i].rgba), side)?;
                    }
                    Err(err) => out.skip(format!("level {id:02} particle texture {i}: {err}")),
                }
            }
            let mut defs = Obj::new().set("header", p.defs.header).set("offsets", &p.defs.offsets[..]).set("blob", &p.defs.blob[..]);
            defs.put("frames_by_type", J::Arr((0..p.defs.offsets.len()).map(|t| J::from(p.defs.frames(t))).collect()));
            out.json(&format!("{}/particle/part_defs.json", dir(id)), &defs.build())?;
            let fx = if p.fx_entries.iter().any(|e| e.present()) { particle_tex::core_bank(&ld.core, &ld.core_data, "fx_bank").ok() } else { None };
            for (i, e) in p.fx_entries.iter().enumerate() {
                let (Some(bank), Some(Some(t))) = (fx, p.fx_textures.get(i)) else { continue };
                match particle_tex::bank_texture_image(bank, e.palette, e.texture, e.width, e.height) {
                    Ok(img) => {
                        let rel = format!("{}/fx/{:03}_{}x{}.png", dir(id), i, e.width, e.height);
                        let side = base_sidecar(&img, &ld.rel("core_data.bin")).set("table", "fx_textures").set("index", i)
                            .set("entry", Obj::new().set("palette", e.palette).set("texture", e.texture).set("width", e.width).set("height", e.height));
                        write_image(out, &rel, &img, Some(&t.rgba), side)?;
                    }
                    Err(err) => out.skip(format!("level {id:02} fx texture {i}: {err}")),
                }
            }
        }
        Err(e) => out.skip(format!("level {id:02} particle textures: {e}")),
    }

    // HUD.
    let header = data::read(data, &ld.rel("hud_header.bin"))?;
    hud_frames(out, &format!("{}/hud", dir(id)), &ld.rel("hud_header.bin"), &header, |b| data::wad(data, &ld.rel(&format!("hud_bank_{b}.bin"))))
}

fn hud_frames(out: &Out, dir: &str, source: &str, header: &[u8], bank: impl Fn(usize) -> Result<Vec<u8>, Error>) -> Result<(), Error> {
    let h = match hud::parse_header(header) { Ok(h) => h, Err(e) => { out.skip(format!("{source}: {e}")); return Ok(()); } };
    let mut banks: [Vec<u8>; hud::BANKS] = Default::default();
    for (b, v) in banks.iter_mut().enumerate() {
        if h.bank_size[b] != 0 { *v = bank(b)?; }
    }
    let hd = match hud::parse_hud(header, std::array::from_fn(|b| banks[b].as_slice())) { Ok(h) => h, Err(e) => { out.skip(format!("{source}: {e}")); return Ok(()); } };
    let mut icons = Vec::new();
    for e in &hd.icons {
        if e.id == 0xffff { break; }
        icons.push(Obj::new().set("id", e.id).set("first_frame", e.first_frame).set("frame_count", e.frame_count).build());
    }
    for i in 0..hd.frames.len() {
        let f = hd.frames[i];
        match hd.frame_image(i).and_then(|img| Ok((img, img.decode()?))) {
            Ok((img, rgba)) => {
                let rel = format!("{dir}/frame_{i:04}_{}x{}.png", img.width, img.height);
                let side = base_sidecar(&img, source).set("table", "hud_frames").set("index", i).set("palette", f.palette).set("texture", f.texture)
                    .set("palette_bank", hd.palette_bank(f.palette.max(0) as usize)).set("texture_bank", hd.texture_bank(f.texture.max(0) as usize));
                write_image(out, &rel, &img, Some(&rgba.rgba), side)?;
            }
            Err(e) => out.skip(format!("{source} frame {i}: {e}")),
        }
    }
    out.json(&format!("{dir}/icons.json"), &Obj::new().set("source", source).set("icons", J::Arr(icons)).build())
}

/// Global textures with a loader: the global HUD set.
pub(crate) fn export_global(data: &Path, out: &Out, _kinds: Kinds) -> Result<(), Error> {
    let header = data::read(data, "global/hud_header.bin")?;
    hud_frames(out, "textures/global/hud", "global/hud_header.bin", &header, |b| data::wad(data, &format!("global/hud_banks/{b:03}.bin")))
}
