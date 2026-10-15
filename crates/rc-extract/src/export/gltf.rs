//! glTF 2.0 writer (`.gltf` JSON + one `.bin` buffer) and a structural validator for our own output.
//!
//! The writer only appends: typed accessors over a single buffer (each in its own 4-byte-aligned buffer view),
//! and the JSON arrays (meshes, nodes, materials, textures, images, samplers, skins, animations, scenes) as
//! [`J`] values. PS2 data glTF has no slot for goes in `extras`.
//!
//! [`validate`] checks what the glTF 2.0 specification requires of the parts we write: every index in range,
//! buffer views inside the buffer, accessors inside their views with the alignment vertex attributes need,
//! POSITION min/max present and correct, index values below the vertex count, equal attribute counts per
//! primitive, skins (joints, inverse bind matrix count), animation samplers (strictly increasing input times,
//! output counts) and image URIs present. The ignored export test runs it on every exported document.

use super::jsonv::{self, Obj, J};
use std::collections::HashMap;

pub const ARRAY_BUFFER: u32 = 34962;
pub const ELEMENT_ARRAY_BUFFER: u32 = 34963;
pub const BYTE: u32 = 5120;
pub const UNSIGNED_BYTE: u32 = 5121;
pub const SHORT: u32 = 5122;
pub const UNSIGNED_SHORT: u32 = 5123;
pub const UNSIGNED_INT: u32 = 5125;
pub const FLOAT: u32 = 5126;
/// Sampler filters and wraps.
pub const NEAREST: u32 = 9728;
pub const LINEAR: u32 = 9729;
pub const NEAREST_MIPMAP_NEAREST: u32 = 9984;
pub const LINEAR_MIPMAP_NEAREST: u32 = 9985;
pub const NEAREST_MIPMAP_LINEAR: u32 = 9986;
pub const LINEAR_MIPMAP_LINEAR: u32 = 9987;
pub const CLAMP_TO_EDGE: u32 = 33071;
pub const REPEAT: u32 = 10497;

/// Game (right-handed, Z up) to glTF (right-handed, Y up): −90° about X, `(x, y, z) → (x, z, −y)`, the same
/// mapping as the engine's `game_to_bevy`. Applied once as the root node's rotation, so every buffer keeps the
/// game's own coordinates.
pub const Z_UP_TO_Y_UP: [f32; 4] = [-std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2];

#[derive(Default)]
pub struct Doc {
    pub bin: Vec<u8>,
    views: Vec<J>,
    accessors: Vec<J>,
    pub meshes: Vec<J>,
    pub nodes: Vec<J>,
    pub materials: Vec<J>,
    textures: Vec<J>,
    images: Vec<J>,
    samplers: Vec<J>,
    pub skins: Vec<J>,
    pub animations: Vec<J>,
    scenes: Vec<J>,
    image_index: HashMap<String, usize>,
    sampler_index: HashMap<[u32; 4], usize>,
    texture_index: HashMap<(usize, usize), usize>,
    /// `asset.extras`.
    pub extras: Obj,
    /// `extensionsUsed` (optional extensions only; none is required).
    pub extensions_used: Vec<&'static str>,
}

/// Little-endian bytes of plain numbers and arrays of them (the buffer layout glTF requires).
pub trait Le: Copy { fn put(self, out: &mut Vec<u8>); }
impl Le for f32 { fn put(self, out: &mut Vec<u8>) { out.extend_from_slice(&self.to_le_bytes()); } }
impl Le for u32 { fn put(self, out: &mut Vec<u8>) { out.extend_from_slice(&self.to_le_bytes()); } }
impl Le for u16 { fn put(self, out: &mut Vec<u8>) { out.extend_from_slice(&self.to_le_bytes()); } }
impl Le for u8 { fn put(self, out: &mut Vec<u8>) { out.push(self); } }
impl<T: Le, const N: usize> Le for [T; N] { fn put(self, out: &mut Vec<u8>) { for x in self { x.put(out); } } }

fn le<T: Le>(v: &[T]) -> Vec<u8> {
    let mut out = Vec::with_capacity(std::mem::size_of_val(v));
    for &x in v { x.put(&mut out); }
    out
}

fn f32s(v: &[f32]) -> J { J::Arr(v.iter().map(|&x| J::F32(x)).collect()) }

impl Doc {
    pub fn new() -> Self { Doc::default() }

    fn view(&mut self, bytes: &[u8], stride: Option<usize>, target: Option<u32>) -> usize {
        while !self.bin.len().is_multiple_of(4) { self.bin.push(0); }
        let mut v = Obj::new().set("buffer", 0).set("byteOffset", self.bin.len()).set("byteLength", bytes.len());
        if let Some(s) = stride { v.put("byteStride", s); }
        if let Some(t) = target { v.put("target", t); }
        self.bin.extend_from_slice(bytes);
        self.views.push(v.build());
        self.views.len() - 1
    }

    #[allow(clippy::too_many_arguments)]
    fn accessor(&mut self, bytes: &[u8], count: usize, component: u32, ty: &str, normalized: bool, stride: Option<usize>, target: Option<u32>, minmax: Option<(Vec<f32>, Vec<f32>)>) -> usize {
        let view = self.view(bytes, stride, target);
        let mut a = Obj::new().set("bufferView", view).set("componentType", component).set("count", count).set("type", ty);
        if normalized { a.put("normalized", true); }
        if let Some((min, max)) = minmax { a.put("min", f32s(&min)); a.put("max", f32s(&max)); }
        self.accessors.push(a.build());
        self.accessors.len() - 1
    }

    fn bounds<const N: usize>(v: &[[f32; N]]) -> (Vec<f32>, Vec<f32>) {
        let mut min = vec![f32::INFINITY; N];
        let mut max = vec![f32::NEG_INFINITY; N];
        for p in v { for k in 0..N { min[k] = min[k].min(p[k]); max[k] = max[k].max(p[k]); } }
        (min, max)
    }

    /// FLOAT VEC3 vertex attribute; `minmax` for POSITION (required there).
    pub fn vec3(&mut self, v: &[[f32; 3]], minmax: bool) -> usize {
        let mm = minmax.then(|| Self::bounds(v));
        self.accessor(&le(v), v.len(), FLOAT, "VEC3", false, None, Some(ARRAY_BUFFER), mm)
    }
    /// FLOAT VEC2 vertex attribute.
    pub fn vec2(&mut self, v: &[[f32; 2]]) -> usize { self.accessor(&le(v), v.len(), FLOAT, "VEC2", false, None, Some(ARRAY_BUFFER), None) }
    /// FLOAT VEC4 vertex attribute.
    pub fn vec4(&mut self, v: &[[f32; 4]]) -> usize { self.accessor(&le(v), v.len(), FLOAT, "VEC4", false, None, Some(ARRAY_BUFFER), None) }
    /// UNSIGNED_BYTE VEC4 vertex attribute (4-byte elements, so no padding is needed).
    pub fn u8x4(&mut self, v: &[[u8; 4]], normalized: bool) -> usize {
        self.accessor(&le(v), v.len(), UNSIGNED_BYTE, "VEC4", normalized, None, Some(ARRAY_BUFFER), None)
    }
    /// UNSIGNED_SHORT VEC4 vertex attribute (JOINTS_0).
    pub fn u16x4(&mut self, v: &[[u16; 4]]) -> usize { self.accessor(&le(v), v.len(), UNSIGNED_SHORT, "VEC4", false, None, Some(ARRAY_BUFFER), None) }
    /// UNSIGNED_INT SCALAR indices.
    pub fn indices(&mut self, v: &[u32]) -> usize { self.accessor(&le(v), v.len(), UNSIGNED_INT, "SCALAR", false, None, Some(ELEMENT_ARRAY_BUFFER), None) }
    /// FLOAT MAT4 (column-major) data, e.g. inverse bind matrices.
    pub fn mat4(&mut self, v: &[[f32; 16]]) -> usize { self.accessor(&le(v), v.len(), FLOAT, "MAT4", false, None, None, None) }
    /// FLOAT SCALAR with min/max (animation sampler input).
    pub fn times(&mut self, v: &[f32]) -> usize {
        let one: Vec<[f32; 1]> = v.iter().map(|&t| [t]).collect();
        let mm = Self::bounds(&one);
        self.accessor(&le(v), v.len(), FLOAT, "SCALAR", false, None, None, Some(mm))
    }
    /// FLOAT VEC3 / VEC4 without a target (animation sampler output).
    pub fn anim_vec3(&mut self, v: &[[f32; 3]]) -> usize { self.accessor(&le(v), v.len(), FLOAT, "VEC3", false, None, None, None) }
    pub fn anim_vec4(&mut self, v: &[[f32; 4]]) -> usize { self.accessor(&le(v), v.len(), FLOAT, "VEC4", false, None, None, None) }

    /// An image by relative URI (deduplicated).
    pub fn image(&mut self, uri: &str) -> usize {
        if let Some(&i) = self.image_index.get(uri) { return i; }
        self.images.push(Obj::new().set("uri", uri).set("mimeType", "image/png").build());
        let i = self.images.len() - 1;
        self.image_index.insert(uri.to_string(), i);
        i
    }

    /// A sampler (deduplicated): `[mag, min, wrap_s, wrap_t]`.
    pub fn sampler(&mut self, key: [u32; 4]) -> usize {
        if let Some(&i) = self.sampler_index.get(&key) { return i; }
        self.samplers.push(Obj::new().set("magFilter", key[0]).set("minFilter", key[1]).set("wrapS", key[2]).set("wrapT", key[3]).build());
        let i = self.samplers.len() - 1;
        self.sampler_index.insert(key, i);
        i
    }

    /// A texture = image + sampler (deduplicated).
    pub fn texture(&mut self, image: usize, sampler: usize) -> usize {
        if let Some(&i) = self.texture_index.get(&(image, sampler)) { return i; }
        self.textures.push(Obj::new().set("source", image).set("sampler", sampler).build());
        let i = self.textures.len() - 1;
        self.texture_index.insert((image, sampler), i);
        i
    }

    pub fn push(list: &mut Vec<J>, v: J) -> usize { list.push(v); list.len() - 1 }

    pub fn scene(&mut self, name: &str, nodes: Vec<usize>, extras: Option<J>) {
        let mut s = Obj::new().set("name", name).set("nodes", J::Arr(nodes.into_iter().map(J::from).collect()));
        if let Some(e) = extras { s.put("extras", e); }
        self.scenes.push(s.build());
    }

    /// The document JSON (compact) for a buffer stored next to it as `bin_uri`, and the buffer bytes.
    pub fn finish(mut self, bin_uri: &str, generator: &str) -> (String, Vec<u8>) {
        while !self.bin.len().is_multiple_of(4) { self.bin.push(0); }
        let mut asset = Obj::new().set("version", "2.0").set("generator", generator);
        if !self.extras.0.is_empty() { asset.put("extras", self.extras.build()); }
        let mut root = Obj::new().set("asset", asset);
        if !self.extensions_used.is_empty() { root.put("extensionsUsed", J::Arr(self.extensions_used.iter().map(|&e| J::from(e)).collect())); }
        if !self.scenes.is_empty() { root.put("scene", 0); root.put("scenes", J::Arr(self.scenes)); }
        for (k, v) in [
            ("nodes", self.nodes), ("meshes", self.meshes), ("materials", self.materials), ("textures", self.textures),
            ("images", self.images), ("samplers", self.samplers), ("skins", self.skins), ("animations", self.animations),
            ("accessors", self.accessors), ("bufferViews", self.views),
        ] {
            if !v.is_empty() { root.put(k, J::Arr(v)); }
        }
        if !self.bin.is_empty() {
            root.put("buffers", J::Arr(vec![Obj::new().set("uri", bin_uri).set("byteLength", self.bin.len()).build()]));
        }
        (root.build().compact(), self.bin)
    }
}

/// A primitive (triangle list) under construction.
pub struct Prim {
    pub attributes: Vec<(&'static str, usize)>,
    pub indices: usize,
    pub material: Option<usize>,
    pub extras: Option<J>,
}

impl Prim {
    pub fn json(self) -> J {
        let mut a = Obj::new();
        for (k, v) in self.attributes { a.put(k, v); }
        let mut p = Obj::new().set("attributes", a).set("indices", self.indices).set("mode", 4);
        if let Some(m) = self.material { p.put("material", m); }
        if let Some(e) = self.extras { p.put("extras", e); }
        p.build()
    }
}

/// What [`validate`] counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub meshes: usize,
    pub primitives: usize,
    pub triangles: usize,
    pub vertices: usize,
    pub nodes: usize,
    pub materials: usize,
    pub images: usize,
    pub skins: usize,
    pub animations: usize,
    pub scenes: usize,
}

fn comp_size(c: usize) -> Option<usize> { match c as u32 { BYTE | UNSIGNED_BYTE => Some(1), SHORT | UNSIGNED_SHORT => Some(2), UNSIGNED_INT | FLOAT => Some(4), _ => None } }
fn type_count(t: &str) -> Option<usize> { match t { "SCALAR" => Some(1), "VEC2" => Some(2), "VEC3" => Some(3), "VEC4" => Some(4), "MAT2" => Some(4), "MAT3" => Some(9), "MAT4" => Some(16), _ => None } }

struct Acc<'a> { comp: usize, n: usize, count: usize, bytes: &'a [u8], stride: usize, is_attr_view: bool }

impl Acc<'_> {
    fn f32_at(&self, i: usize, k: usize) -> f32 {
        let o = i * self.stride + k * comp_size(self.comp).unwrap();
        let b = &self.bytes[o..];
        match self.comp as u32 {
            FLOAT => f32::from_le_bytes(b[..4].try_into().unwrap()),
            UNSIGNED_INT => u32::from_le_bytes(b[..4].try_into().unwrap()) as f32,
            UNSIGNED_SHORT => u16::from_le_bytes(b[..2].try_into().unwrap()) as f32,
            SHORT => i16::from_le_bytes(b[..2].try_into().unwrap()) as f32,
            UNSIGNED_BYTE => b[0] as f32,
            _ => b[0] as i8 as f32,
        }
    }
    fn u32_at(&self, i: usize) -> u32 { self.f32_at(i, 0) as u32 }
}

/// Validates a document and its buffer. `uri_exists` answers whether an image URI resolves (relative to the
/// document).
pub fn validate(json: &str, bin: &[u8], uri_exists: &dyn Fn(&str) -> bool) -> Result<Stats, String> {
    let root = jsonv::parse(json)?;
    let arr = |k: &str| root.get(k).map_or(Ok(&[][..]), |v| v.as_arr().ok_or(format!("{k} is not an array")));
    if root.get("asset").and_then(|a| a.get("version")).and_then(J::as_str) != Some("2.0") { return Err("asset.version is not 2.0".into()); }
    let (buffers, views, accessors, meshes, nodes, materials, textures, images, samplers, skins, animations, scenes) = (
        arr("buffers")?, arr("bufferViews")?, arr("accessors")?, arr("meshes")?, arr("nodes")?, arr("materials")?, arr("textures")?,
        arr("images")?, arr("samplers")?, arr("skins")?, arr("animations")?, arr("scenes")?,
    );
    let idx = |v: Option<&J>, n: usize, what: &str| -> Result<usize, String> {
        let i = v.and_then(J::as_usize).ok_or(format!("{what}: missing or not an index"))?;
        if i >= n { return Err(format!("{what}: index {i} out of range ({n})")); }
        Ok(i)
    };
    if buffers.len() > 1 { return Err("more than one buffer".into()); }
    if let Some(b) = buffers.first() {
        if b.get("byteLength").and_then(J::as_usize) != Some(bin.len()) { return Err(format!("buffer byteLength != {} bytes of .bin", bin.len())); }
    }
    // Buffer views.
    let mut vranges = Vec::new();
    for (i, v) in views.iter().enumerate() {
        idx(v.get("buffer"), buffers.len(), &format!("bufferView {i}.buffer"))?;
        let off = v.get("byteOffset").and_then(J::as_usize).unwrap_or(0);
        let len = v.get("byteLength").and_then(J::as_usize).ok_or(format!("bufferView {i}: no byteLength"))?;
        if off + len > bin.len() { return Err(format!("bufferView {i} runs past the buffer")); }
        let stride = v.get("byteStride").and_then(J::as_usize);
        if let Some(s) = stride { if !(4..=252).contains(&s) || s % 4 != 0 { return Err(format!("bufferView {i}: bad byteStride {s}")); } }
        vranges.push((off, len, stride, v.get("target").and_then(J::as_usize)));
    }
    // Accessors.
    let mut accs = Vec::new();
    for (i, a) in accessors.iter().enumerate() {
        let vi = idx(a.get("bufferView"), views.len(), &format!("accessor {i}.bufferView"))?;
        let comp = a.get("componentType").and_then(J::as_usize).ok_or(format!("accessor {i}: no componentType"))?;
        let cs = comp_size(comp).ok_or(format!("accessor {i}: bad componentType {comp}"))?;
        let ty = a.get("type").and_then(J::as_str).ok_or(format!("accessor {i}: no type"))?;
        let n = type_count(ty).ok_or(format!("accessor {i}: bad type {ty}"))?;
        let count = a.get("count").and_then(J::as_usize).filter(|&c| c > 0).ok_or(format!("accessor {i}: count must be >= 1"))?;
        let off = a.get("byteOffset").and_then(J::as_usize).unwrap_or(0);
        let (voff, vlen, vstride, target) = vranges[vi];
        let elem = cs * n;
        let stride = vstride.unwrap_or(elem);
        if off % cs != 0 { return Err(format!("accessor {i}: byteOffset not a multiple of the component size")); }
        if off + stride * (count - 1) + elem > vlen { return Err(format!("accessor {i}: runs past bufferView {vi}")); }
        let is_attr_view = target == Some(ARRAY_BUFFER as usize);
        if is_attr_view && ((voff + off) % 4 != 0 || stride % 4 != 0) { return Err(format!("accessor {i}: vertex attribute not 4-byte aligned")); }
        let bytes = &bin[voff + off..voff + vlen];
        let acc = Acc { comp, n, count, bytes, stride, is_attr_view };
        for (key, cmp) in [("min", 0), ("max", 1)] {
            if let Some(m) = a.get(key) {
                let m = m.as_arr().ok_or(format!("accessor {i}: {key} not an array"))?;
                if m.len() != n { return Err(format!("accessor {i}: {key} has {} values for {n} components", m.len())); }
                for (k, mk) in m.iter().enumerate() {
                    let want = mk.as_f64().ok_or(format!("accessor {i}: {key} not numeric"))? as f32;
                    let got = (0..count).map(|j| acc.f32_at(j, k)).fold(if cmp == 0 { f32::INFINITY } else { f32::NEG_INFINITY }, |x, y| if cmp == 0 { x.min(y) } else { x.max(y) });
                    if got != want { return Err(format!("accessor {i}: {key}[{k}] is {want}, data has {got}")); }
                }
            }
        }
        accs.push(acc);
    }
    let mut st = Stats { meshes: meshes.len(), nodes: nodes.len(), materials: materials.len(), images: images.len(), skins: skins.len(), animations: animations.len(), scenes: scenes.len(), ..Stats::default() };
    // Images, samplers, textures, materials.
    for (i, im) in images.iter().enumerate() {
        let uri = im.get("uri").and_then(J::as_str).ok_or(format!("image {i}: no uri"))?;
        if !uri_exists(uri) { return Err(format!("image {i}: {uri} does not exist")); }
    }
    for (i, s) in samplers.iter().enumerate() {
        for (k, ok) in [("magFilter", &[NEAREST, LINEAR][..]), ("minFilter", &[NEAREST, LINEAR, NEAREST_MIPMAP_NEAREST, LINEAR_MIPMAP_NEAREST, NEAREST_MIPMAP_LINEAR, LINEAR_MIPMAP_LINEAR][..]), ("wrapS", &[CLAMP_TO_EDGE, REPEAT, 33648][..]), ("wrapT", &[CLAMP_TO_EDGE, REPEAT, 33648][..])] {
            if let Some(v) = s.get(k) { if !ok.contains(&(v.as_usize().unwrap_or(0) as u32)) { return Err(format!("sampler {i}: bad {k}")); } }
        }
    }
    for (i, t) in textures.iter().enumerate() {
        idx(t.get("source"), images.len(), &format!("texture {i}.source"))?;
        if t.get("sampler").is_some() { idx(t.get("sampler"), samplers.len(), &format!("texture {i}.sampler"))?; }
    }
    for (i, m) in materials.iter().enumerate() {
        if let Some(t) = m.get("pbrMetallicRoughness").and_then(|p| p.get("baseColorTexture")) { idx(t.get("index"), textures.len(), &format!("material {i} texture"))?; }
        if let Some(a) = m.get("alphaMode").and_then(J::as_str) { if !["OPAQUE", "MASK", "BLEND"].contains(&a) { return Err(format!("material {i}: alphaMode {a}")); } }
    }
    // Meshes.
    let mut mesh_skinned = vec![false; meshes.len()];
    for (mi, m) in meshes.iter().enumerate() {
        let prims = m.get("primitives").and_then(J::as_arr).filter(|p| !p.is_empty()).ok_or(format!("mesh {mi}: no primitives"))?;
        for (pi, p) in prims.iter().enumerate() {
            let what = format!("mesh {mi} primitive {pi}");
            let attrs = p.get("attributes").and_then(J::as_obj).ok_or(format!("{what}: no attributes"))?;
            let mut n = None;
            let mut has_pos = false;
            for (k, v) in attrs {
                let ai = idx(Some(v), accs.len(), &format!("{what} {k}"))?;
                let a = &accs[ai];
                if !a.is_attr_view { return Err(format!("{what} {k}: accessor's view has no ARRAY_BUFFER target")); }
                if *n.get_or_insert(a.count) != a.count { return Err(format!("{what}: attribute counts differ ({k})")); }
                match k.as_str() {
                    "POSITION" => {
                        has_pos = true;
                        if a.comp as u32 != FLOAT || a.n != 3 || accessors[ai].get("min").is_none() { return Err(format!("{what}: POSITION must be FLOAT VEC3 with min/max")); }
                    }
                    "NORMAL" => {
                        if a.comp as u32 != FLOAT || a.n != 3 { return Err(format!("{what}: NORMAL must be FLOAT VEC3")); }
                        for j in 0..a.count {
                            let l = (0..3).map(|k| a.f32_at(j, k).powi(2)).sum::<f32>().sqrt();
                            if (l - 1.0).abs() > 5e-4 { return Err(format!("{what}: NORMAL {j} has length {l}")); }
                        }
                    }
                    "TEXCOORD_0" => if a.comp as u32 != FLOAT || a.n != 2 { return Err(format!("{what}: TEXCOORD_0 must be FLOAT VEC2")); },
                    "COLOR_0" => if !(a.n == 3 || a.n == 4) { return Err(format!("{what}: COLOR_0 must be VEC3/VEC4")); },
                    "JOINTS_0" => {
                        mesh_skinned[mi] = true;
                        if !matches!(a.comp as u32, UNSIGNED_BYTE | UNSIGNED_SHORT) || a.n != 4 { return Err(format!("{what}: JOINTS_0 type")); }
                    }
                    "WEIGHTS_0" => {
                        if a.comp as u32 != FLOAT || a.n != 4 { return Err(format!("{what}: WEIGHTS_0 must be FLOAT VEC4")); }
                        for j in 0..a.count {
                            let s: f32 = (0..4).map(|k| a.f32_at(j, k)).sum();
                            if (s - 1.0).abs() > 2e-3 { return Err(format!("{what}: WEIGHTS_0 {j} sums to {s}")); }
                        }
                    }
                    k if k.starts_with('_') => {}
                    k => return Err(format!("{what}: unexpected attribute {k}")),
                }
            }
            if !has_pos { return Err(format!("{what}: no POSITION")); }
            let vcount = n.unwrap_or(0);
            if p.get("mode").and_then(J::as_usize).unwrap_or(4) != 4 { return Err(format!("{what}: not a triangle list")); }
            let ii = idx(p.get("indices"), accs.len(), &format!("{what} indices"))?;
            let ia = &accs[ii];
            if ia.n != 1 || !matches!(ia.comp as u32, UNSIGNED_BYTE | UNSIGNED_SHORT | UNSIGNED_INT) || ia.count % 3 != 0 {
                return Err(format!("{what}: indices must be an unsigned SCALAR list of whole triangles"));
            }
            if vranges[accessors[ii].get("bufferView").and_then(J::as_usize).unwrap()].3 != Some(ELEMENT_ARRAY_BUFFER as usize) { return Err(format!("{what}: index view target")); }
            for j in 0..ia.count { if ia.u32_at(j) as usize >= vcount { return Err(format!("{what}: index {} >= {vcount} vertices", ia.u32_at(j))); } }
            if p.get("material").is_some() { idx(p.get("material"), materials.len(), &format!("{what} material"))?; }
            st.primitives += 1;
            st.triangles += ia.count / 3;
            st.vertices += vcount;
        }
    }
    // Nodes (and a cycle / multiple-parent check), skins.
    let mut parent = vec![None; nodes.len()];
    for (i, n) in nodes.iter().enumerate() {
        if let Some(c) = n.get("children") {
            for c in c.as_arr().ok_or(format!("node {i}: children"))? {
                let c = idx(Some(c), nodes.len(), &format!("node {i} child"))?;
                if parent[c].replace(i).is_some() || c == i { return Err(format!("node {c} has two parents")); }
            }
        }
        let mesh = n.get("mesh").map(|m| idx(Some(m), meshes.len(), &format!("node {i}.mesh"))).transpose()?;
        let skin = n.get("skin").map(|s| idx(Some(s), skins.len(), &format!("node {i}.skin"))).transpose()?;
        if let Some(m) = mesh { if mesh_skinned[m] && skin.is_none() { return Err(format!("node {i}: skinned mesh {m} without a skin")); } }
        if n.get("matrix").is_some() && (n.get("translation").is_some() || n.get("rotation").is_some() || n.get("scale").is_some()) { return Err(format!("node {i}: matrix and TRS")); }
        if let Some(m) = n.get("matrix") { if m.as_arr().map(<[J]>::len) != Some(16) { return Err(format!("node {i}: matrix needs 16 numbers")); } }
    }
    for i in 0..nodes.len() {
        let (mut p, mut steps) = (parent[i], 0);
        while let Some(q) = p { steps += 1; if steps > nodes.len() { return Err(format!("node {i}: cycle")); } p = parent[q]; }
    }
    for (i, s) in skins.iter().enumerate() {
        let joints = s.get("joints").and_then(J::as_arr).filter(|j| !j.is_empty()).ok_or(format!("skin {i}: no joints"))?;
        for j in joints { idx(Some(j), nodes.len(), &format!("skin {i} joint"))?; }
        if let Some(ib) = s.get("inverseBindMatrices") {
            let a = &accs[idx(Some(ib), accs.len(), &format!("skin {i} inverseBindMatrices"))?];
            if a.n != 16 || a.count != joints.len() { return Err(format!("skin {i}: inverseBindMatrices count/type")); }
        }
    }
    for (i, an) in animations.iter().enumerate() {
        let samplers = an.get("samplers").and_then(J::as_arr).ok_or(format!("animation {i}: samplers"))?;
        for (si, s) in samplers.iter().enumerate() {
            let inp = &accs[idx(s.get("input"), accs.len(), &format!("animation {i} sampler {si} input"))?];
            let out = &accs[idx(s.get("output"), accs.len(), &format!("animation {i} sampler {si} output"))?];
            if inp.n != 1 || inp.comp as u32 != FLOAT { return Err(format!("animation {i} sampler {si}: input type")); }
            for j in 1..inp.count { if inp.f32_at(j, 0) <= inp.f32_at(j - 1, 0) { return Err(format!("animation {i} sampler {si}: times not increasing")); } }
            if out.count != inp.count { return Err(format!("animation {i} sampler {si}: {} outputs for {} times", out.count, inp.count)); }
        }
        for (ci, c) in an.get("channels").and_then(J::as_arr).ok_or(format!("animation {i}: channels"))?.iter().enumerate() {
            let s = idx(c.get("sampler"), samplers.len(), &format!("animation {i} channel {ci} sampler"))?;
            let t = c.get("target").ok_or(format!("animation {i} channel {ci}: no target"))?;
            idx(t.get("node"), nodes.len(), &format!("animation {i} channel {ci} node"))?;
            let out = &accs[samplers[s].get("output").and_then(J::as_usize).unwrap()];
            let want = match t.get("path").and_then(J::as_str) { Some("translation" | "scale") => 3, Some("rotation") => 4, p => return Err(format!("animation {i} channel {ci}: path {p:?}")) };
            if out.n != want { return Err(format!("animation {i} channel {ci}: output type")); }
        }
    }
    for (i, s) in scenes.iter().enumerate() {
        for n in s.get("nodes").and_then(J::as_arr).ok_or(format!("scene {i}: nodes"))? {
            let n = idx(Some(n), nodes.len(), &format!("scene {i} node"))?;
            if parent[n].is_some() { return Err(format!("scene {i}: node {n} is not a root")); }
        }
    }
    if root.get("scene").is_some() { idx(root.get("scene"), scenes.len(), "scene")?; }
    Ok(st)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle_doc() -> Doc {
        let mut d = Doc::new();
        let pos = d.vec3(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 2.0]], true);
        let uv = d.vec2(&[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
        let col = d.u8x4(&[[255, 0, 0, 255]; 3], true);
        let ind = d.indices(&[0, 1, 2]);
        let img = d.image("tex.png");
        let smp = d.sampler([LINEAR, LINEAR_MIPMAP_NEAREST, REPEAT, CLAMP_TO_EDGE]);
        let tex = d.texture(img, smp);
        assert_eq!(d.texture(img, smp), tex, "deduplicated");
        let mat = Doc::push(&mut d.materials, Obj::new().set("pbrMetallicRoughness", Obj::new().set("baseColorTexture", Obj::new().set("index", tex))).build());
        let prim = Prim { attributes: vec![("POSITION", pos), ("TEXCOORD_0", uv), ("COLOR_0", col)], indices: ind, material: Some(mat), extras: None };
        let mesh = Doc::push(&mut d.meshes, Obj::new().set("primitives", J::Arr(vec![prim.json()])).build());
        let child = Doc::push(&mut d.nodes, Obj::new().set("mesh", mesh).build());
        let root = Doc::push(&mut d.nodes, Obj::new().set("rotation", Z_UP_TO_Y_UP).set("children", J::Arr(vec![child.into()])).build());
        d.scene("s", vec![root], None);
        d
    }

    #[test]
    fn a_written_document_validates_and_bounds_are_checked() {
        let (json, bin) = triangle_doc().finish("t.bin", "test");
        let st = validate(&json, &bin, &|u| u == "tex.png").unwrap();
        assert_eq!((st.meshes, st.primitives, st.triangles, st.vertices, st.nodes, st.images, st.scenes), (1, 1, 1, 3, 2, 1, 1));
        assert_eq!(bin.len() % 4, 0);
        // Every failure mode we rely on is detected.
        assert!(validate(&json, &bin[..bin.len() - 4], &|_| true).unwrap_err().contains("byteLength"));
        assert!(validate(&json, &bin, &|_| false).unwrap_err().contains("does not exist"));
        let bad_index = json.replace("\"indices\":3", "\"indices\":9");
        assert!(validate(&bad_index, &bin, &|_| true).is_err());
        let mut bin2 = bin.clone();
        let n = bin2.len();
        bin2[n - 4..].copy_from_slice(&7u32.to_le_bytes()); // index 7 of 3 vertices
        assert!(validate(&json, &bin2, &|_| true).unwrap_err().contains(">= 3 vertices"));
        let bad_max = json.replace("\"max\":[1,1,2]", "\"max\":[1,1,3]");
        assert_ne!(bad_max, json);
        assert!(validate(&bad_max, &bin, &|_| true).unwrap_err().contains("max"));
        assert!(validate(&json.replace("\"2.0\"", "\"1.0\""), &bin, &|_| true).is_err());
    }

    #[test]
    fn skins_and_animations_are_checked() {
        let mut d = triangle_doc();
        let j = Doc::push(&mut d.nodes, Obj::new().set("name", "joint").build());
        let ibm = d.mat4(&[[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]]);
        Doc::push(&mut d.skins, Obj::new().set("joints", J::Arr(vec![j.into()])).set("inverseBindMatrices", ibm).build());
        let t = d.times(&[0.0, 0.5, 1.0]);
        let r = d.anim_vec4(&[[0.0, 0.0, 0.0, 1.0]; 3]);
        let an = Obj::new()
            .set("samplers", J::Arr(vec![Obj::new().set("input", t).set("output", r).build()]))
            .set("channels", J::Arr(vec![Obj::new().set("sampler", 0).set("target", Obj::new().set("node", j).set("path", "rotation")).build()]))
            .build();
        d.animations.push(an);
        let (json, bin) = d.finish("t.bin", "test");
        let st = validate(&json, &bin, &|_| true).unwrap();
        assert_eq!((st.skins, st.animations), (1, 1));
        assert!(validate(&json.replace("\"path\":\"rotation\"", "\"path\":\"scale\""), &bin, &|_| true).is_err());
    }
}
