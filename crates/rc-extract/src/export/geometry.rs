//! Level geometry → glTF 2.0: `levels/NN/level.gltf` (tfrag, tie and shrub instances, moby placements, sky) and
//! `levels/NN/collision.gltf`. Buffers keep the game's own coordinates (Z up, world units); one root node per scene
//! rotates them to glTF's Y up. Materials reference the exported PNGs.
//!
//! Scenes of `level.gltf`:
//! 0. `level`: tfrag LOD 0 (the highest-detail strip list), every tie instance with its class's LOD 0 mesh, every
//!    shrub instance, and one empty node per moby instance (placement; the model is under `models/`);
//! 1. `lod1`, 2. `lod2`: the tfrag and tie LOD 1 / LOD 2 meshes the game switches to with distance;
//! 3. `sky`: the sky shells, camera-relative (the game draws them around the camera with the view translation
//!    removed), in raw sky units.
//!
//! What glTF has no slot for is in `extras` (and the custom `_PS2_*` vertex attributes): the raw vertex RGBA
//! (0x80 = 1.0), tie light slots and LOD-morph deltas, per-instance colours, the GS pass (TEST_1 / AREF, ALPHA_1)
//! and texture registers, LOD distances, billboards. Vertex colours: `COLOR_0` = the stored RGBA as display
//! values (×2, clamped, sRGB-decoded to glTF's linear space), so a viewer's `texture × COLOR_0` matches the GS
//! MODULATE; the load-time vertex lighting is not baked (tie and shrub colours are per instance, in `extras`).
//! Triangles with vertex normals (tie, shrub, moby) are turned to face along them (`Verts::orient`); the game
//! draws both sides, so every material is double-sided and nothing visible changes.

use super::data::{self, rel_uri, LevelData};
use super::gltf::{self, Doc, Prim};
use super::jsonv::{hex, Obj, J};
use super::textures::{self, TexIndex, TexInfo};
use super::Out;
use crate::Error;
use rc_formats::collision;
use rc_formats::shrub::{self, LevelShrubClass};
use rc_formats::sky;
use rc_formats::texture::TextureTable;
use rc_formats::tfrag::{self, GsFilter, GsWrap};
use rc_formats::tie::{self, LevelTieClass};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub(crate) const GENERATOR: &str = concat!("ratchet_clank-extract ", env!("CARGO_PKG_VERSION"));

/// A GS drawing pass as the game sets it up (engine `gs_state.rs` table).
#[derive(Clone, Copy)]
pub(crate) struct Pass { pub name: &'static str, pub test_1: u32, pub aref: Option<u32> }

pub(crate) const TFRAG: Pass = Pass { name: "tfrag", test_1: 0x5360b, aref: Some(0x60) };
pub(crate) const TIE: Pass = Pass { name: "tie", test_1: 0x5360b, aref: Some(0x60) };
pub(crate) const SHRUB: Pass = Pass { name: "shrub", test_1: 0x5320b, aref: Some(0x20) };
pub(crate) const MOBY: Pass = Pass { name: "moby", test_1: 0x5360b, aref: Some(0x60) };
const SKY_TEXTURED: Pass = Pass { name: "sky_textured", test_1: 0x3180b, aref: None };
const SKY_GOURAUD: Pass = Pass { name: "sky_gouraud", test_1: 0x30000, aref: None };

/// sRGB (display) byte → linear byte.
fn srgb_to_linear(v: u8) -> u8 {
    let c = v as f32 / 255.0;
    let l = if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    (l * 255.0).round() as u8
}

/// PS2 vertex RGBA (0x80 = 1.0) → glTF `COLOR_0` (normalized bytes, linear).
pub(crate) fn ps2_color(c: [u8; 4]) -> [u8; 4] {
    let d = |v: u8| (v as u16 * 2).min(255) as u8;
    [srgb_to_linear(d(c[0])), srgb_to_linear(d(c[1])), srgb_to_linear(d(c[2])), d(c[3])]
}

/// Vertex arrays of one primitive, with dedup by a caller key.
#[derive(Default)]
pub(crate) struct Verts<K: std::hash::Hash + Eq> {
    pub pos: Vec<[f32; 3]>,
    pub nrm: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub col: Vec<[u8; 4]>,
    pub raw: Vec<[u8; 4]>,
    pub slots: Vec<[u8; 4]>,
    pub morph: Vec<[f32; 3]>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub idx: Vec<u32>,
    remap: HashMap<K, u32>,
}

impl<K: std::hash::Hash + Eq> Verts<K> {
    /// Index of the vertex `key`, adding it with `add` the first time.
    pub fn vertex(&mut self, key: K, add: impl FnOnce(&mut Self)) -> u32 {
        if let Some(&v) = self.remap.get(&key) { return v; }
        let v = self.pos.len() as u32;
        add(self);
        self.remap.insert(key, v);
        v
    }

    /// Turns every triangle whose face normal points away from its vertex normals (the game's strips alternate
    /// winding and the GS draws both sides, so the stored order carries no facing), so glTF viewers see one
    /// consistent front. Without normals the stored order is kept. Returns the number of triangles turned.
    pub fn orient(&mut self) -> usize {
        if self.nrm.len() != self.pos.len() { return 0; }
        let mut turned = 0;
        for t in self.idx.as_chunks_mut::<3>().0 {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| i as usize);
            let (p, q, r) = (self.pos[a], self.pos[b], self.pos[c]);
            let (u, v) = ([q[0] - p[0], q[1] - p[1], q[2] - p[2]], [r[0] - p[0], r[1] - p[1], r[2] - p[2]]);
            let f = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let n: [f32; 3] = std::array::from_fn(|k| self.nrm[a][k] + self.nrm[b][k] + self.nrm[c][k]);
            if f[0] * n[0] + f[1] * n[1] + f[2] * n[2] < 0.0 { t.swap(1, 2); turned += 1; }
        }
        turned
    }

    /// Accessors and the primitive. `slots_name` names the `slots` attribute when present.
    pub fn prim(&self, doc: &mut Doc, material: Option<usize>, slots_name: &'static str, extras: Option<J>) -> J {
        let n = self.pos.len();
        let mut attributes = vec![("POSITION", doc.vec3(&self.pos, true))];
        if self.nrm.len() == n { attributes.push(("NORMAL", doc.vec3(&self.nrm, false))); }
        if self.uv.len() == n { attributes.push(("TEXCOORD_0", doc.vec2(&self.uv))); }
        if self.col.len() == n { attributes.push(("COLOR_0", doc.u8x4(&self.col, true))); }
        if self.joints.len() == n { attributes.push(("JOINTS_0", doc.u16x4(&self.joints))); attributes.push(("WEIGHTS_0", doc.vec4(&self.weights))); }
        if self.raw.len() == n { attributes.push(("_PS2_RGBA", doc.u8x4(&self.raw, false))); }
        if self.slots.len() == n { attributes.push((slots_name, doc.u8x4(&self.slots, false))); }
        if self.morph.len() == n { attributes.push(("_PS2_MORPH_DELTA", doc.vec3(&self.morph, false))); }
        let indices = doc.indices(&self.idx);
        Prim { attributes, indices, material, extras }.json()
    }
}

/// A unit normal, or +Z for a degenerate one.
pub(crate) fn unit(n: [f32; 3]) -> [f32; 3] {
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l > 1e-6 && l.is_finite() { n.map(|c| c / l) } else { [0.0, 0.0, 1.0] }
}

pub(crate) fn wrap(clamp: bool) -> u32 { if clamp { gltf::CLAMP_TO_EDGE } else { gltf::REPEAT } }

fn gs_wrap(w: GsWrap) -> u32 { if w == GsWrap::Repeat { gltf::REPEAT } else { gltf::CLAMP_TO_EDGE } }

fn gs_filter(f: GsFilter) -> u32 {
    match f {
        GsFilter::Nearest => gltf::NEAREST,
        GsFilter::Linear => gltf::LINEAR,
        GsFilter::NearestMipmapNearest => gltf::NEAREST_MIPMAP_NEAREST,
        GsFilter::NearestMipmapLinear => gltf::NEAREST_MIPMAP_LINEAR,
        GsFilter::LinearMipmapNearest => gltf::LINEAR_MIPMAP_NEAREST,
        GsFilter::LinearMipmapLinear => gltf::LINEAR_MIPMAP_LINEAR,
    }
}

/// On-disc MMIN of an ad-gif's TEX1 (`data_hi`) as a glTF min filter.
pub(crate) fn mmin(v: i32) -> u32 {
    match v { 0 => gltf::NEAREST, 1 => gltf::LINEAR, 2 => gltf::NEAREST_MIPMAP_NEAREST, 3 => gltf::NEAREST_MIPMAP_LINEAR, 4 => gltf::LINEAR_MIPMAP_NEAREST, _ => gltf::LINEAR_MIPMAP_LINEAR }
}

/// Materials of one document, deduplicated.
#[derive(Default)]
pub(crate) struct Materials { cache: HashMap<String, usize> }

impl Materials {
    /// A material for `pass` with texture `tex` (None = untextured) sampled with `[mag, min, wrap_s, wrap_t]`.
    /// `doc_dir` is the document's folder under the export root (for the image URI). `special` names untextured
    /// kinds (`chrome`, `glass`, `gouraud`, …).
    pub fn get(&mut self, doc: &mut Doc, doc_dir: &str, pass: Pass, tex: Option<&TexInfo>, sampler: [u32; 4], special: Option<&str>) -> usize {
        let key = format!("{}|{:?}|{sampler:?}|{special:?}", pass.name, tex.map(|t| &t.path));
        if let Some(&m) = self.cache.get(&key) { return m; }
        let mut pbr = Obj::new().set("metallicFactor", 0.0f32).set("roughnessFactor", 1.0f32);
        if let Some(t) = tex {
            let img = doc.image(&rel_uri(doc_dir, &t.path));
            let smp = doc.sampler(sampler);
            pbr.put("baseColorTexture", Obj::new().set("index", doc.texture(img, smp)));
        }
        let name = match (tex, special) {
            (Some(t), _) => format!("{}:{}", pass.name, t.path.rsplit('/').next().unwrap_or(&t.path).trim_end_matches(".png")),
            (None, Some(s)) => format!("{}:{s}", pass.name),
            (None, None) => format!("{}:untextured", pass.name),
        };
        let mut ps2 = Obj::new().set("pass", pass.name).set("test_1", hex(pass.test_1 as u64)).set("alpha_1", "0x8000000044")
            .set("tfx", "MODULATE").set("blend", "Cout = ((Cs − Cd)·As >> 7) + Cd; As = At·Af >> 7 (0x80 = 1.0)");
        match pass.aref {
            Some(a) => { ps2.put("aref", a); ps2.put("alpha_test", "As >= AREF writes RGB and Z; As < AREF writes blended RGB only"); }
            None => ps2.put("alpha_test", "none (sky: ZTST ALWAYS)"),
        }
        if let Some(s) = special { ps2.put("special", s); }
        let mut m = Obj::new().set("name", name).set("doubleSided", true).set("pbrMetallicRoughness", pbr)
            .set("extensions", Obj::new().set("KHR_materials_unlit", Obj::new()));
        let alpha = tex.is_some_and(|t| t.has_alpha) || matches!(special, Some("gouraud" | "vertex_alpha"));
        match (alpha, pass.aref) {
            (false, _) => {}
            (true, Some(a)) if !matches!(special, Some("vertex_alpha")) => { m.put("alphaMode", "MASK"); m.put("alphaCutoff", a as f32 / 128.0); }
            (true, _) => m.put("alphaMode", "BLEND"),
        }
        m.put("extras", Obj::new().set("ps2", ps2));
        let i = Doc::push(&mut doc.materials, m.build());
        self.cache.insert(key, i);
        i
    }
}

fn node(name: String, mesh: Option<usize>, extras: Option<J>) -> Obj {
    let mut n = Obj::new().set("name", name);
    if let Some(m) = mesh { n.put("mesh", m); }
    if let Some(e) = extras { n.put("extras", e); }
    n
}

/// A root node (Z up → Y up) with `children`.
pub(crate) fn root(doc: &mut Doc, name: &str, children: Vec<usize>) -> usize {
    Doc::push(&mut doc.nodes, Obj::new().set("name", name).set("rotation", gltf::Z_UP_TO_Y_UP).set("children", J::Arr(children.into_iter().map(J::from).collect())).build())
}

pub(crate) fn group_node(doc: &mut Doc, name: &str, children: Vec<usize>) -> usize {
    Doc::push(&mut doc.nodes, Obj::new().set("name", name).set("children", J::Arr(children.into_iter().map(J::from).collect())).build())
}

/// Column-major `matrix[col][row]` with `[3][3]` = 1 as the glTF node matrix.
fn node_matrix(m: [[f32; 4]; 4]) -> J { J::Arr(m.iter().flatten().map(|&v| J::F32(v)).collect()) }

/// The quaternion of `R = Rz(z)·Ry(y)·Rx(x)` (x applied first), the moby rotation rule.
pub(crate) fn euler_zyx_quat(r: [f32; 3]) -> [f32; 4] {
    let h = |a: f32| ((a * 0.5).sin(), (a * 0.5).cos());
    let (sx, cx) = h(r[0]);
    let (sy, cy) = h(r[1]);
    let (sz, cz) = h(r[2]);
    // qz * qy * qx
    [
        cz * cy * sx - sz * sy * cx,
        cz * sy * cx + sz * cy * sx,
        sz * cy * cx - cz * sy * sx,
        cz * cy * cx + sz * sy * sx,
    ]
}

fn write_doc(out: &Out, dir: &str, stem: &str, doc: Doc) -> Result<(), Error> {
    let (json, bin) = doc.finish(&format!("{stem}.bin"), GENERATOR);
    out.write(&format!("{dir}/{stem}.bin"), &bin)?;
    out.write(&format!("{dir}/{stem}.gltf"), json.as_bytes())
}

struct Level<'a> { ld: &'a LevelData, tex: &'a TexIndex, dir: String }

impl Level<'_> {
    fn tex(&self, table: TextureTable, index: Option<usize>) -> Option<&TexInfo> { self.tex.table.get(&(table, index?)) }
}

/// The tfrag meshes of the three strip lists; returns the mesh index per LOD (None when empty).
fn tfrag_meshes(lv: &Level, doc: &mut Doc, mats: &mut Materials, tfrags: &[tfrag::Tfrag]) -> Result<[Option<usize>; 3], Error> {
    let mut meshes = [None; 3];
    for (lod, slot) in meshes.iter_mut().enumerate() {
        type Key = (i32, u32, u32, u32);
        type Batch = (Verts<(u32, u16)>, std::collections::BTreeSet<i16>);
        let mut batches: BTreeMap<Key, Batch> = BTreeMap::new();
        for (ti, t) in tfrags.iter().enumerate() {
            let tris = tfrag::tfrag_triangles(t, lod).map_err(|e| data::damaged(&lv.ld.rel("core_data.bin"), e))?;
            for tri in tris {
                let ad = &t.ad_gifs[tri.ad_gif as usize];
                let key = (t.texture_index(tri.ad_gif as usize), gs_wrap(ad.wrap_s()), gs_wrap(ad.wrap_t()), gs_filter(ad.min_filter()));
                let (b, ks) = batches.entry(key).or_default();
                ks.insert(ad.lod_k_raw());
                for vi in [tri.a, tri.b, tri.c] {
                    let v = b.vertex((ti as u32, vi), |b| {
                        let e = t.vertex_info[vi as usize];
                        let c = t.rgba.get(t.position_index(vi as usize)).map_or([0x80; 4], |c| [c.r, c.g, c.b, c.a]);
                        b.pos.push(t.world_position(vi as usize));
                        b.uv.push([e.s as f32 / 4096.0, e.t as f32 / 4096.0]);
                        b.col.push(ps2_color(c));
                        b.raw.push(c);
                    });
                    b.idx.push(v);
                }
            }
        }
        if batches.is_empty() { continue; }
        let mut prims = Vec::new();
        for ((tex_index, ws, wt, min), (b, ks)) in batches {
            let tex = lv.tex(TextureTable::Tfrag, usize::try_from(tex_index).ok());
            let m = mats.get(doc, &lv.dir, TFRAG, tex, [gltf::LINEAR, min, ws, wt], None);
            let extras = Obj::new().set("texture_index", tex_index).set("tex1_lod_k_raw", J::Arr(ks.into_iter().map(J::from).collect())).build();
            prims.push(b.prim(doc, Some(m), "_PS2_SLOTS", Some(extras)));
        }
        *slot = Some(Doc::push(&mut doc.meshes, Obj::new().set("name", format!("tfrag_lod{lod}")).set("primitives", J::Arr(prims)).build()));
    }
    Ok(meshes)
}

/// One tie class's mesh per LOD.
fn tie_meshes(lv: &Level, doc: &mut Doc, mats: &mut Materials, lt: &LevelTieClass) -> [Option<usize>; 3] {
    let c = &lt.class;
    let scale = c.header.scale;
    let mut out = [None; 3];
    for (lod, slot) in out.iter_mut().enumerate() {
        let mut groups: BTreeMap<u16, Verts<(usize, u16)>> = BTreeMap::new();
        for (pi, p) in c.lods[lod].iter().enumerate() {
            for tri in tie::tie_triangles(p) {
                let g = groups.entry(tri.ad_gif).or_default();
                for vi in [tri.a, tri.b, tri.c] {
                    let v = g.vertex((pi, vi), |g| {
                        let x = p.vertices[vi as usize];
                        g.pos.push(x.class_position(scale));
                        g.nrm.push(unit(c.normal(x.color).unwrap_or([0.0, 0.0, 1.0])));
                        g.uv.push(x.uv());
                        g.slots.push([x.color, x.morph_colors[0], x.morph_colors[1], x.fat]);
                        g.morph.push(x.class_morph_delta(scale));
                    });
                    g.idx.push(v);
                }
            }
        }
        if groups.is_empty() { continue; }
        let mut prims = Vec::new();
        for (ad, mut g) in groups {
            g.orient();
            let tex_index = lt.texture_table_index(ad);
            let tex = lv.tex(TextureTable::Tie, tex_index);
            let a = c.ad_gifs.get(ad as usize);
            let sampler = a.map_or([gltf::LINEAR, gltf::LINEAR_MIPMAP_NEAREST, gltf::REPEAT, gltf::REPEAT], |a| {
                [gltf::LINEAR, mmin(a.tex1.data_hi), wrap(a.clamp.data_lo & 1 != 0), wrap(a.clamp.data_hi & 1 != 0)]
            });
            let m = mats.get(doc, &lv.dir, TIE, tex, sampler, if tex.is_none() { Some("missing_texture") } else { None });
            let mut e = Obj::new().set("ad_gif", ad).set("texture_index", tex_index);
            if let Some(a) = a {
                e.put("ad_gif_regs", Obj::new().set("tex0", hex(a.tex0.value())).set("tex1", hex(a.tex1.value())).set("miptbp1", hex(a.miptbp1.value()))
                    .set("clamp", hex(a.clamp.value())).set("miptbp2", hex(a.miptbp2.value())));
            }
            prims.push(g.prim(doc, Some(m), "_PS2_TIE_SLOTS", Some(e.build())));
        }
        let h = &c.header;
        let extras = Obj::new().set("o_class", lt.o_class).set("lod", lod).set("scale", scale).set("bsphere", h.bsphere)
            .set("lod_distances", [h.near_dist, h.mid_dist, h.far_dist])
            .set("attributes", "_PS2_TIE_SLOTS = (light slot, morph slot 0, morph slot 1, fat); _PS2_MORPH_DELTA: a fat vertex is drawn at POSITION + k·delta as it morphs; NORMAL = the class normal of the light slot")
            .build();
        *slot = Some(Doc::push(&mut doc.meshes, Obj::new().set("name", format!("tie_{:04}_lod{lod}", lt.o_class)).set("primitives", J::Arr(prims)).set("extras", extras).build()));
    }
    out
}

/// One shrub class's mesh.
fn shrub_mesh(lv: &Level, doc: &mut Doc, mats: &mut Materials, ls: &LevelShrubClass) -> Option<usize> {
    let c = &ls.class;
    let scale = c.header.scale;
    let mut groups: BTreeMap<u16, Verts<(usize, u16)>> = BTreeMap::new();
    let mut ad_by_slot: HashMap<u16, shrub::ShrubAdGifs> = HashMap::new();
    for (pi, p) in c.packets.iter().enumerate() {
        for a in &p.ad_gifs { ad_by_slot.entry(a.tex0.data_lo as u16).or_insert(*a); }
        for tri in shrub::shrub_triangles(p) {
            let g = groups.entry(tri.texture).or_default();
            for vi in [tri.a, tri.b, tri.c] {
                let v = g.vertex((pi, vi), |g| {
                    let x = p.vertices[vi as usize];
                    g.pos.push(x.class_position(scale));
                    g.nrm.push(unit(c.normal(x.normal).unwrap_or([0.0, 0.0, 1.0])));
                    g.uv.push(x.uv());
                    g.slots.push([x.normal, x.stop, 0, 0]);
                });
                g.idx.push(v);
            }
        }
    }
    if groups.is_empty() { return None; }
    let mut prims = Vec::new();
    for (slot, mut g) in groups {
        g.orient();
        let tex_index = ls.texture_table_index(slot);
        let tex = lv.tex(TextureTable::Shrub, tex_index);
        let a = ad_by_slot.get(&slot);
        let sampler = a.map_or([gltf::LINEAR, gltf::LINEAR_MIPMAP_NEAREST, gltf::REPEAT, gltf::REPEAT], |a| {
            let (s, t) = a.clamp_st();
            [gltf::LINEAR, mmin(a.tex1.data_hi), wrap(s), wrap(t)]
        });
        let m = mats.get(doc, &lv.dir, SHRUB, tex, sampler, if tex.is_none() { Some("missing_texture") } else { None });
        prims.push(g.prim(doc, Some(m), "_PS2_SHRUB_SLOTS", Some(Obj::new().set("texture_slot", slot).set("texture_index", tex_index).build())));
    }
    let h = &c.header;
    let mut extras = Obj::new().set("o_class", ls.o_class).set("scale", scale).set("bsphere", h.bsphere).set("mip_distance", h.mip_distance)
        .set("mode_bits", hex(h.mode_bits as u64))
        .set("attributes", "_PS2_SHRUB_SLOTS = (normal / instance palette index, VU1 stop flag, 0, 0); NORMAL = that class normal");
    if let Some(b) = &c.billboard {
        let bb = ls.billboard_texture().and_then(|_| lv.tex.table.get(&(TextureTable::Billboard, ls_index(lv, ls))).map(|t| rel_uri(&lv.dir, &t.path)));
        extras.put("billboard", Obj::new().set("fade_distance", b.fade_distance).set("width", b.width).set("height", b.height).set("z_ofs", b.z_ofs)
            .set("tex1", hex(b.tex1.value())).set("texture", bb));
    }
    Some(Doc::push(&mut doc.meshes, Obj::new().set("name", format!("shrub_{:04}", ls.o_class)).set("primitives", J::Arr(prims)).set("extras", extras).build()))
}

/// Index of a shrub class in `LevelCore::shrub_classes` (billboard textures are keyed by it).
fn ls_index(lv: &Level, ls: &LevelShrubClass) -> usize {
    lv.ld.core.shrub_classes.iter().position(|e| e.base.o_class == ls.o_class).unwrap_or(usize::MAX)
}

/// Sky shells: one mesh per shell, one primitive per texture (0xff = gouraud).
fn sky_meshes(lv: &Level, doc: &mut Doc, mats: &mut Materials, s: &sky::Sky) -> Vec<usize> {
    let mut nodes = Vec::new();
    for (si, shell) in s.shells.iter().enumerate() {
        let mut groups: BTreeMap<u8, Verts<usize>> = BTreeMap::new();
        for c in &shell.clusters {
            for v in sky::sky_gs_vertices(shell, c) {
                let g = groups.entry(v.texture).or_default();
                let k = g.pos.len();
                let i = g.vertex(k, |g| {
                    g.pos.push(v.position.map(|x| x as f32));
                    g.col.push(if shell.textured() { let a = ps2_color(v.rgba); [255, 255, 255, a[3]] } else { ps2_color(v.rgba) });
                    g.raw.push(v.rgba);
                    if shell.textured() { g.uv.push(v.st); }
                });
                g.idx.push(i);
            }
        }
        if groups.is_empty() { continue; }
        let mut prims = Vec::new();
        for (t, g) in groups {
            let (pass, tex, special) = if t == 0xff { (SKY_GOURAUD, None, Some("gouraud")) } else { (SKY_TEXTURED, lv.tex.sky.get(&(t as usize)), Some("vertex_alpha")) };
            let m = mats.get(doc, &lv.dir, pass, tex, [gltf::LINEAR, gltf::LINEAR, gltf::REPEAT, gltf::REPEAT], special);
            prims.push(g.prim(doc, Some(m), "_PS2_SLOTS", Some(Obj::new().set("texture", t).build())));
        }
        let mesh = Doc::push(&mut doc.meshes, Obj::new().set("name", format!("sky_shell_{si}")).set("primitives", J::Arr(prims))
            .set("extras", Obj::new().set("shell", si).set("flags", shell.flags).set("textured", shell.textured())).build());
        nodes.push(Doc::push(&mut doc.nodes, node(format!("sky_shell_{si}"), Some(mesh), None).build()));
    }
    nodes
}

/// `levels/NN/level.gltf`.
pub(crate) fn export_level(data: &Path, out: &Out, id: u32) -> Result<(), Error> {
    let ld = LevelData::load(data, id)?;
    let tex = textures::index_level(&ld)?;
    let g = data::gameplay(data, id)?;
    let lv = Level { ld: &ld, tex: &tex, dir: format!("levels/{id:02}") };
    let core_rel = ld.rel("core_data.bin");
    let gp_rel = data::lvl(id, "gameplay_ntsc.bin");
    let mut doc = Doc::new();
    let mut mats = Materials::default();

    // Tfrag.
    let tblock = tfrag::tfrag_block(&ld.core, &ld.core_data).map_err(|e| data::damaged(&core_rel, e))?;
    let tfrags = tfrag::parse_tfrags(tblock).map_err(|e| data::damaged(&core_rel, e))?;
    let tb = tfrag::parse_tfrag_block_header(tblock).map_err(|e| data::damaged(&core_rel, e))?;
    let tf = tfrag_meshes(&lv, &mut doc, &mut mats, &tfrags)?;

    // Ties.
    let ties = tie::parse_level_ties(&ld.core, &ld.core_data).map_err(|e| data::damaged(&core_rel, e))?;
    let tie_meshes: HashMap<i32, [Option<usize>; 3]> = ties.iter().map(|t| (t.o_class, tie_meshes(&lv, &mut doc, &mut mats, t))).collect();
    let tie_inst = tie::parse_tie_instances(&g).map_err(|e| data::damaged(&gp_rel, e))?;
    let mut tie_nodes: [Vec<usize>; 3] = Default::default();
    for (i, t) in tie_inst.iter().enumerate() {
        let Some(ms) = tie_meshes.get(&t.o_class) else { out.skip(format!("level {id:02} tie instance {i}: class {} has no geometry", t.o_class)); continue };
        for (lod, nodes) in tie_nodes.iter_mut().enumerate() {
            let Some(m) = ms[lod] else { continue };
            let mut e = Obj::new().set("o_class", t.o_class).set("index", i).set("uid", t.uid);
            if lod == 0 {
                e.put("draw_distance", t.draw_distance);
                e.put("occlusion_index", t.occlusion_index);
                e.put("directional_lights", t.directional_lights);
                e.put("ambient_rgba5551", &t.ambient_rgbas[..]);
                e.put("colour_note", "per light slot (_PS2_TIE_SLOTS.x): RGBA5551 ambient, expanded like PEXT5 (5 bits to bits 7..3, alpha bit to 0x80)");
            }
            let n = node(format!("tie_{:04}_{i}", t.o_class), Some(m), Some(e.build())).set("matrix", node_matrix(t.world_matrix()));
            nodes.push(Doc::push(&mut doc.nodes, n.build()));
        }
    }

    // Shrubs.
    let shrubs = shrub::parse_level_shrubs(&ld.core, &ld.core_data).map_err(|e| data::damaged(&core_rel, e))?;
    let shrub_meshes: HashMap<i32, Option<usize>> = shrubs.iter().map(|s| (s.o_class, shrub_mesh(&lv, &mut doc, &mut mats, s))).collect();
    let mut shrub_nodes = Vec::new();
    for (i, s) in shrub::parse_shrub_instances(&g).map_err(|e| data::damaged(&gp_rel, e))?.iter().enumerate() {
        let Some(&Some(m)) = shrub_meshes.get(&s.o_class) else { out.skip(format!("level {id:02} shrub instance {i}: class {} has no geometry", s.o_class)); continue };
        let e = Obj::new().set("o_class", s.o_class).set("index", i).set("draw_distance", s.draw_distance).set("colour", s.colour)
            .set("dir_lights", hex(s.dir_lights as u32 as u64)).build();
        shrub_nodes.push(Doc::push(&mut doc.nodes, node(format!("shrub_{:04}_{i}", s.o_class), Some(m), Some(e)).set("matrix", node_matrix(s.world_matrix())).build()));
    }

    // Moby placements.
    let mut moby_nodes = Vec::new();
    match rc_formats::gameplay::parse_moby_instances(&g) {
        Ok(v) => for (i, m) in v.iter().enumerate() {
            let e = Obj::new().set("o_class", m.o_class).set("index", i).set("pvar_index", m.pvar_index).set("group", m.group)
                .set("model", rel_uri(&lv.dir, &format!("models/levels/{id:02}/mobys/{:04}.gltf", m.o_class))).build();
            let n = node(format!("moby_{:04}_{i}", m.o_class), None, Some(e)).set("translation", m.position).set("rotation", euler_zyx_quat(m.rotation))
                .set("scale", [m.scale; 3]);
            moby_nodes.push(Doc::push(&mut doc.nodes, n.build()));
        },
        Err(e) => out.skip(format!("{gp_rel} moby instances: {e}")),
    }

    // Scenes.
    let mut main = Vec::new();
    if let Some(m) = tf[0] { main.push(Doc::push(&mut doc.nodes, node("tfrag".into(), Some(m), None).build())); }
    let children = std::mem::take(&mut tie_nodes[0]);
    main.push(group_node(&mut doc, "ties", children));
    main.push(group_node(&mut doc, "shrubs", shrub_nodes));
    main.push(group_node(&mut doc, "mobys", moby_nodes));
    let r = root(&mut doc, &format!("level_{id:02}"), main);
    doc.scene("level", vec![r], None);
    for lod in 1..3 {
        let mut kids = Vec::new();
        if let Some(m) = tf[lod] { kids.push(Doc::push(&mut doc.nodes, node(format!("tfrag_lod{lod}"), Some(m), None).build())); }
        let children = std::mem::take(&mut tie_nodes[lod]);
        kids.push(group_node(&mut doc, &format!("ties_lod{lod}"), children));
        let r = root(&mut doc, &format!("level_{id:02}_lod{lod}"), kids);
        doc.scene(&format!("lod{lod}"), vec![r], Some(Obj::new().set("note", "tfrag strip list and tie meshes of this LOD; shrubs and mobys are in scene 0").build()));
    }
    match sky::parse_sky(&ld.core, &ld.core_data) {
        Ok(Some(s)) => {
            let kids = sky_meshes(&lv, &mut doc, &mut mats, &s);
            let r = root(&mut doc, &format!("sky_{id:02}"), kids);
            doc.scene("sky", vec![r], Some(Obj::new().set("note", "camera-relative: the game draws the shells around the camera (view translation removed), in shell order, before everything else; raw sky units").set("colour", s.header.colour).build()));
        }
        Ok(None) => {}
        Err(e) => out.skip(format!("{core_rel} sky: {e}")),
    }
    doc.extras = Obj::new()
        .set("level", id)
        .set("sources", J::Arr(vec![J::from(core_rel.as_str()), J::from(ld.rel("core_index.bin")), J::from(ld.rel("gs_ram.bin")), J::from(gp_rel.as_str())]))
        .set("units", "game world units (1024 raw tfrag units = 1); buffers are Z up, the scene roots rotate to Y up")
        .set("tfrag_lod", Obj::new().set("L", tb.unknown_8).set("distances", tb.lod_distances()).set("note", "switch distances D0, D1, D2 = 6L, 4L, 2L; vertices morph between tiers (not baked)"))
        .set("vertex_colours", "COLOR_0 = stored PS2 RGBA ×2 (0x80 = 1.0), clamped, sRGB-decoded; _PS2_RGBA = the stored bytes; load-time lighting not applied")
        .set("scenes", "0 level (LOD 0), 1 lod1, 2 lod2, 3 sky");
    doc.extensions_used.push("KHR_materials_unlit");
    write_doc(out, &lv.dir, "level", doc)
}

/// Debug colour of a surface id, for viewers.
fn surface_colour(s: u8) -> [f32; 4] {
    let h = (s as u32).wrapping_mul(2_654_435_761);
    [0.25 + (h & 0xff) as f32 / 400.0, 0.25 + (h >> 8 & 0xff) as f32 / 400.0, 0.25 + (h >> 16 & 0xff) as f32 / 400.0, 1.0]
}

/// `levels/NN/collision.gltf`: the collision mesh (world space), one primitive per surface byte.
pub(crate) fn export_collision(data: &Path, out: &Out, id: u32) -> Result<(), Error> {
    let ld = LevelData::load(data, id)?;
    let core_rel = ld.rel("core_data.bin");
    let c = collision::parse_collision(&ld.core, &ld.core_data).map_err(|e| data::damaged(&core_rel, e))?;
    let mut groups: BTreeMap<u8, (Verts<usize>, usize)> = BTreeMap::new();
    for t in collision::collision_triangles(&c) {
        let (g, faces) = groups.entry(t.surface).or_default();
        if t.part < 2 { *faces += 1; }
        for p in [t.a, t.b, t.c] {
            let k = g.pos.len();
            let i = g.vertex(k, |g| g.pos.push(p));
            g.idx.push(i);
        }
    }
    let mut doc = Doc::new();
    let mut prims = Vec::new();
    for (s, (g, faces)) in groups {
        let f = collision::CollisionFace { v: [0; 3], surface: s };
        let m = Doc::push(&mut doc.materials, Obj::new().set("name", format!("surface_{s:#04x}")).set("doubleSided", true)
            .set("pbrMetallicRoughness", Obj::new().set("baseColorFactor", surface_colour(s)).set("metallicFactor", 0.0f32).set("roughnessFactor", 1.0f32)).build());
        let e = Obj::new().set("surface", s).set("surface_id", f.surface_id()).set("sound_class", f.sound_class()).set("high_bit", f.high_bit()).set("faces", faces).build();
        prims.push(g.prim(&mut doc, Some(m), "_PS2_SLOTS", Some(e)));
    }
    let mut kids = Vec::new();
    if !prims.is_empty() {
        let mesh = Doc::push(&mut doc.meshes, Obj::new().set("name", "collision").set("primitives", J::Arr(prims)).build());
        kids.push(Doc::push(&mut doc.nodes, node("collision".into(), Some(mesh), None).build()));
    }
    let r = root(&mut doc, &format!("collision_{id:02}"), kids);
    doc.scene("collision", vec![r], None);
    doc.extras = Obj::new().set("level", id).set("source", core_rel.as_str())
        .set("surfaces", "one primitive per surface byte: surface_id = bits 0..4 (0x1f = none), sound_class = bits 5..6, high_bit = bit 7")
        .set("note", "every face of every 4-unit cell; quads split along v0-v2; faces shared by cells appear once per cell (rc_formats::collision::collision_triangles)")
        .set("cells", c.cells.len()).set("hero_groups", c.hero_groups.len());
    write_doc(out, &format!("levels/{id:02}"), "collision", doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rot(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
        let [x, y, z, w] = q;
        let t = [2.0 * (y * v[2] - z * v[1]), 2.0 * (z * v[0] - x * v[2]), 2.0 * (x * v[1] - y * v[0])];
        [v[0] + w * t[0] + (y * t[2] - z * t[1]), v[1] + w * t[1] + (z * t[0] - x * t[2]), v[2] + w * t[2] + (x * t[1] - y * t[0])]
    }

    #[test]
    fn euler_order_is_x_then_y_then_z() {
        let q = euler_zyx_quat([std::f32::consts::FRAC_PI_2, 0.0, std::f32::consts::FRAC_PI_2]);
        // x first: +y → +z; then z: +z stays.
        let p = rot(q, [0.0, 1.0, 0.0]);
        assert!((p[0]).abs() < 1e-6 && (p[1]).abs() < 1e-6 && (p[2] - 1.0).abs() < 1e-6, "{p:?}");
        // +x: Rx keeps it, Rz turns it to +y.
        let p = rot(q, [1.0, 0.0, 0.0]);
        assert!((p[1] - 1.0).abs() < 1e-6, "{p:?}");
        // The root rotation maps game +Z up to glTF +Y up and game +Y to glTF −Z.
        let p = rot(gltf::Z_UP_TO_Y_UP, [0.0, 0.0, 1.0]);
        assert!((p[1] - 1.0).abs() < 1e-6, "{p:?}");
        let p = rot(gltf::Z_UP_TO_Y_UP, [0.0, 1.0, 0.0]);
        assert!((p[2] + 1.0).abs() < 1e-6, "{p:?}");
    }

    #[test]
    fn orient_turns_triangles_against_their_normals() {
        let mut v: Verts<usize> = Verts {
            pos: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            nrm: vec![[0.0, 0.0, 1.0]; 3],
            idx: vec![0, 1, 2, 0, 2, 1],
            ..Default::default()
        };
        assert_eq!(v.orient(), 1);
        assert_eq!(v.idx, [0, 1, 2, 0, 1, 2]);
        v.nrm.clear();
        v.idx = vec![0, 2, 1];
        assert_eq!(v.orient(), 0, "no normals: stored order kept");
    }

    #[test]
    fn ps2_colours_map_0x80_to_one() {
        assert_eq!(ps2_color([0x80, 0x80, 0x80, 0x80]), [255, 255, 255, 255]);
        assert_eq!(ps2_color([0, 0, 0, 0]), [0, 0, 0, 0]);
        assert_eq!(ps2_color([0xff, 0x40, 0x10, 0x40])[3], 0x80);
        assert_eq!(ps2_color([0xff; 4])[0], 255);
    }
}
