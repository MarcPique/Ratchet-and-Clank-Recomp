//! Moby classes → glTF 2.0 (`models/levels/NN/mobys/CCCC.gltf`), skinned, with every animation sequence.
//!
//! Mesh: the high-LOD and metal packets (scene 0) and the low-LOD packets (scene 1) in the stored pose (bind pose,
//! world units = packed × class scale / 1024), one primitive per texture (-1 untextured, -2 chrome, -3 glass are
//! named materials). Normals from the packed angles (`rc_formats::moby::moby_normal`).
//!
//! Skinning reproduces the game's result, not its mechanism: the skinning uses the joint palette
//! `F_j = P_j·S_j` (`rc_formats::moby_anim::evaluate`, the port of `MobyAnimEval`), which maps stored-pose
//! vertices straight to posed ones. So each palette entry is one joint node, all siblings under a `skeleton`
//! node, with identity inverse bind matrices: the rest pose (every joint at identity) is the stored pose, and a
//! joint's animated transform is `F_j` itself (translation scaled to world units, then decomposed into
//! translation / rotation / scale; a shear, if any, is dropped and reported in `extras`). One extra `static`
//! joint that never moves carries the vertices the game draws with the identity (low LOD of a class whose
//! low-LOD joint count is 0). The class skeleton (the game's inverse bind matrices) and the joint parents are in
//! `extras` for tools that want a hierarchy.
//!
//! Animations: one per sequence, one key per keyframe at the time the game reaches it (`1 / rate` ticks per key
//! interval at 60 ticks per second, the sequence's rate override when set), linear interpolation (the game
//! nlerps). Channels that stay at the identity are omitted.

use super::data::{self, LevelData};
use super::geometry::{self, Materials, Verts, MOBY};
use super::gltf::{self, Doc};
use super::jsonv::{hex, Obj, J};
use super::textures::{self, TexIndex};
use super::Out;
use crate::Error;
use rc_formats::moby::{self, LevelMobyClass, MobySubmesh};
use rc_formats::moby_anim::{self, AnimState, MobyAnimClass, MobySequence, Rows};
use rc_formats::texture::TextureTable;
use std::collections::BTreeMap;
use std::path::Path;

/// Translation, rotation (x, y, z, w) and scale of a palette matrix (row-vector rows as stored; translation row 3
/// in packed units, scaled by `k` to world units), and how far the rebuilt matrix is from the original (shear).
pub(crate) fn decompose(m: &Rows, k: f32) -> ([f32; 3], [f32; 4], [f32; 3], f32) {
    let col = |j: usize| [m[j][0], m[j][1], m[j][2]];
    let (c0, c1, c2) = (col(0), col(1), col(2));
    let len = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut s = [len(c0), len(c1), len(c2)];
    if dot(c0, cross(c1, c2)) < 0.0 { s[0] = -s[0]; }
    let basis = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let r: [[f32; 3]; 3] = std::array::from_fn(|j| if s[j].abs() > 1e-12 { col(j).map(|v| v / s[j]) } else { basis[j] });
    // r[j] is column j of the rotation: element (row i, col j) = r[j][i].
    let e = |i: usize, j: usize| r[j][i];
    let tr = e(0, 0) + e(1, 1) + e(2, 2);
    let q = if tr > 0.0 {
        let w = (tr + 1.0).sqrt() * 2.0;
        [(e(2, 1) - e(1, 2)) / w, (e(0, 2) - e(2, 0)) / w, (e(1, 0) - e(0, 1)) / w, 0.25 * w]
    } else if e(0, 0) > e(1, 1) && e(0, 0) > e(2, 2) {
        let w = (1.0 + e(0, 0) - e(1, 1) - e(2, 2)).sqrt() * 2.0;
        [0.25 * w, (e(0, 1) + e(1, 0)) / w, (e(0, 2) + e(2, 0)) / w, (e(2, 1) - e(1, 2)) / w]
    } else if e(1, 1) > e(2, 2) {
        let w = (1.0 + e(1, 1) - e(0, 0) - e(2, 2)).sqrt() * 2.0;
        [(e(0, 1) + e(1, 0)) / w, 0.25 * w, (e(1, 2) + e(2, 1)) / w, (e(0, 2) - e(2, 0)) / w]
    } else {
        let w = (1.0 + e(2, 2) - e(0, 0) - e(1, 1)).sqrt() * 2.0;
        [(e(0, 2) + e(2, 0)) / w, (e(1, 2) + e(2, 1)) / w, 0.25 * w, (e(1, 0) - e(0, 1)) / w]
    };
    let n = (q.iter().map(|c| c * c).sum::<f32>()).sqrt();
    let q = if n > 0.0 && n.is_finite() { q.map(|c| c / n) } else { [0.0, 0.0, 0.0, 1.0] };
    // Shear: |R·S − L| over the linear part.
    let rebuilt = |j: usize| r[j].map(|v| v * s[j]);
    let err = (0..3).flat_map(|j| (0..3).map(move |i| (j, i))).map(|(j, i)| (rebuilt(j)[i] - col(j)[i]).abs()).fold(0.0f32, f32::max);
    ([m[3][0] * k, m[3][1] * k, m[3][2] * k], q, s, err)
}

/// Seconds at which each key of a sequence is reached.
fn key_times(seq: &MobySequence) -> Vec<f32> {
    let mut t = 0.0f32;
    let mut out = Vec::with_capacity(seq.frames.len());
    for f in &seq.frames {
        out.push(t);
        let r = if seq.header.rate_override != 0.0 { seq.header.rate_override } else { f.header.rate };
        let ticks = if r > 0.0 && r.is_finite() { (1.0 / r).max(1e-3) } else { 1.0 };
        t += ticks / 60.0;
    }
    out
}

/// The class blob (for its class-relative sequence pointers).
fn class_blob<'a>(ld: &'a LevelData, lm: &LevelMobyClass) -> Option<&'a [u8]> {
    let b = ld.core.blocks.iter().find(|b| b.offset == lm.entry.offset_in_asset_wad as usize && b.name.starts_with("moby_class/"))?;
    ld.core_data.get(b.offset..b.offset + b.size)
}

/// Sequences of a class; Ratchet (class 0) takes the level's `ratchet_seq` blocks when the class has none.
fn sequences(out: &Out, ld: &LevelData, lm: &LevelMobyClass) -> Vec<Option<MobySequence>> {
    let mut seqs = match class_blob(ld, lm).map(|b| moby_anim::parse_sequences(b, &lm.class)) {
        Some(Ok(s)) => s,
        Some(Err(e)) => { out.skip(format!("level {:02} moby {} sequences: {e}", ld.id, lm.o_class)); Vec::new() }
        None => Vec::new(),
    };
    if lm.o_class == 0 && seqs.iter().all(Option::is_none) {
        seqs = (0..ld.core.ratchet_seqs.len())
            .map(|i| {
                let name = format!("ratchet_seq/{i:03}");
                let b = ld.core.blocks.iter().find(|b| b.name == name)?;
                moby_anim::parse_sequence(ld.core_data.get(b.offset..b.offset + b.size)?, 0).ok()
            })
            .collect();
        while seqs.last().is_some_and(Option::is_none) { seqs.pop(); }
    }
    seqs
}

fn highest_joint(lists: &[&[MobySubmesh]]) -> Option<u8> {
    lists.iter().flat_map(|l| l.iter()).flat_map(|s| s.vertices.iter()).flat_map(|v| v.skin.joints[..v.skin.count as usize].to_vec()).max()
}

fn class_model(out: &Out, ld: &LevelData, tex: &TexIndex, lm: &LevelMobyClass) -> Result<(), Error> {
    let dir = format!("models/levels/{:02}/mobys", ld.id);
    let c = &lm.class;
    let h = &c.header;
    let k = h.scale / 1024.0;
    let seqs = sequences(out, ld, lm);
    let n_seqs = seqs.iter().filter(|s| s.is_some()).count();
    let anim = MobyAnimClass::new(c, seqs);
    let jc = anim.joint_count;
    let skinned = jc > 0;
    let n_joints = if skinned { jc.max(highest_joint(&[&c.high_lod, &c.low_lod, &c.metal]).map_or(0, |j| j as usize + 1)) } else { 0 };
    let static_joint = n_joints as u16;

    let mut doc = Doc::new();
    let mut mats = Materials::default();
    // (lod, list): 0 = high + metal, 1 = low.
    let lists: [(usize, &[MobySubmesh], bool); 3] = [(0, &c.high_lod, false), (0, &c.metal, false), (1, &c.low_lod, h.low_lod_joint_count == 0)];
    let mut meshes = [None, None];
    for (lod, mesh_slot) in meshes.iter_mut().enumerate() {
        type Group = Verts<(usize, usize, usize)>;
        let mut groups: BTreeMap<(bool, i32), Group> = BTreeMap::new();
        for (li, (l, list, identity)) in lists.iter().enumerate() {
            if *l != lod { continue; }
            for (pi, sm) in list.iter().enumerate() {
                for t in &sm.triangles {
                    let g = groups.entry((sm.is_metal, t.texture)).or_default();
                    for vi in [t.a, t.b, t.c] {
                        let v = g.vertex((li, pi, vi as usize), |g| {
                            let x = &sm.vertices[vi as usize];
                            g.pos.push(c.position(x));
                            g.nrm.push(geometry::unit(c.normal(x)));
                            g.uv.push(x.st.map(|s| s as f32 / 4096.0));
                            if skinned {
                                let s = x.skin;
                                if *identity || s.count == 0 {
                                    g.joints.push([static_joint, 0, 0, 0]);
                                    g.weights.push([1.0, 0.0, 0.0, 0.0]);
                                } else {
                                    let n = s.count.min(3) as usize;
                                    let mut j = [0u16; 4];
                                    let mut w = [0f32; 4];
                                    for i in 0..n { j[i] = s.joints[i] as u16; w[i] = s.weights[i] as f32 / 256.0; }
                                    g.joints.push(j);
                                    g.weights.push(w);
                                }
                            }
                        });
                        g.idx.push(v);
                    }
                }
            }
        }
        if groups.is_empty() { continue; }
        let mut prims = Vec::new();
        for ((metal, texture), mut g) in groups {
            g.orient();
            let ti = lm.texture_table_index(texture);
            let t = ti.and_then(|i| tex.table.get(&(TextureTable::Moby, i)));
            let special = match texture { -1 => Some("untextured"), -2 => Some("chrome"), -3 => Some("glass"), _ if t.is_none() => Some("missing_texture"), _ => None };
            let m = mats.get(&mut doc, &dir, MOBY, t, [gltf::LINEAR, gltf::LINEAR_MIPMAP_NEAREST, gltf::REPEAT, gltf::REPEAT], special);
            let e = Obj::new().set("texture", texture).set("texture_index", ti).set("metal", metal).build();
            prims.push(g.prim(&mut doc, Some(m), "_PS2_SLOTS", Some(e)));
        }
        *mesh_slot = Some(Doc::push(&mut doc.meshes, Obj::new().set("name", format!("moby_{:04}_{}", lm.o_class, if lod == 0 { "high" } else { "low" })).set("primitives", J::Arr(prims)).build()));
    }
    if meshes.iter().all(Option::is_none) { return Ok(()); }

    // Skeleton.
    let mut skin = None;
    let mut joint_nodes = Vec::new();
    let mut skeleton_node = None;
    if skinned {
        for j in 0..=n_joints {
            let name = if j == n_joints { "static".to_string() } else { format!("joint_{j:02}") };
            joint_nodes.push(Doc::push(&mut doc.nodes, Obj::new().set("name", name).build()));
        }
        let sk = geometry::group_node(&mut doc, "skeleton", joint_nodes.clone());
        skeleton_node = Some(sk);
        skin = Some(Doc::push(&mut doc.skins, Obj::new().set("name", format!("moby_{:04}", lm.o_class)).set("skeleton", sk)
            .set("joints", J::Arr(joint_nodes.iter().map(|&n| J::from(n)).collect())).build()));

        // Animations.
        let mut max_shear = 0.0f32;
        for (si, seq) in anim.sequences.iter().enumerate() {
            let Some(seq) = seq else { continue };
            if seq.frames.is_empty() { continue; }
            let times = key_times(seq);
            // pose[key][joint]
            let mut poses = Vec::with_capacity(seq.frames.len());
            for key in 0..seq.frames.len() {
                let mut st = AnimState::spawn(&anim);
                if !moby_anim::hard_cut(&mut st, &anim, si as u8, key as i32) { break; }
                st.t = 0.0;
                let f = moby_anim::evaluate(&anim, &st);
                poses.push(f.iter().map(|m| decompose(m, k)).collect::<Vec<_>>());
            }
            if poses.len() != times.len() { continue; }
            let input = doc.times(&times);
            let (mut samplers, mut channels) = (Vec::new(), Vec::new());
            for j in 0..jc.min(poses[0].len()) {
                let ts: Vec<[f32; 3]> = poses.iter().map(|p| p[j].0).collect();
                let rs: Vec<[f32; 4]> = poses.iter().map(|p| p[j].1).collect();
                let ss: Vec<[f32; 3]> = poses.iter().map(|p| p[j].2).collect();
                max_shear = poses.iter().map(|p| p[j].3).fold(max_shear, f32::max);
                let near = |a: &[f32], b: &[f32]| a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 1e-6);
                let mut add = |path: &str, out: usize| {
                    samplers.push(Obj::new().set("input", input).set("output", out).set("interpolation", "LINEAR").build());
                    channels.push(Obj::new().set("sampler", samplers.len() - 1).set("target", Obj::new().set("node", joint_nodes[j]).set("path", path)).build());
                };
                if !ts.iter().all(|t| near(t, &[0.0; 3])) { let a = doc.anim_vec3(&ts); add("translation", a); }
                if !rs.iter().all(|r| near(r, &[0.0, 0.0, 0.0, 1.0]) || near(r, &[0.0, 0.0, 0.0, -1.0])) {
                    // Keep consecutive quaternions in one hemisphere so LINEAR takes the short way.
                    let mut rs = rs;
                    for i in 1..rs.len() {
                        if rs[i].iter().zip(&rs[i - 1]).map(|(a, b)| a * b).sum::<f32>() < 0.0 { rs[i] = rs[i].map(|c| -c); }
                    }
                    let a = doc.anim_vec4(&rs);
                    add("rotation", a);
                }
                if !ss.iter().all(|s| near(s, &[1.0; 3])) { let a = doc.anim_vec3(&ss); add("scale", a); }
            }
            if channels.is_empty() { continue; }
            let e = Obj::new().set("sequence", si).set("frames", seq.frames.len()).set("loop_sound", seq.header.loop_sound)
                .set("rate_override", seq.header.rate_override).set("triggers", J::Arr(seq.triggers.iter().map(|&t| hex(t as u64)).collect())).build();
            doc.animations.push(Obj::new().set("name", format!("seq_{si:03}")).set("samplers", J::Arr(samplers)).set("channels", J::Arr(channels)).set("extras", e).build());
        }
        doc.extras.put("max_shear_dropped", max_shear);
    }

    let mut scene0 = Vec::new();
    let mut scene1 = Vec::new();
    for (lod, m) in meshes.iter().enumerate() {
        let Some(m) = *m else { continue };
        let mut n = Obj::new().set("name", format!("moby_{:04}_{}", lm.o_class, if lod == 0 { "high" } else { "low" })).set("mesh", m);
        if let Some(s) = skin { n.put("skin", s); }
        let node = Doc::push(&mut doc.nodes, n.build());
        if lod == 0 { scene0.push(node) } else { scene1.push(node) }
    }
    // The skeleton has its own root so both LOD scenes can show it (a root node may be in several scenes).
    let skel_root = skeleton_node.map(|sk| geometry::root(&mut doc, "skeleton_root", vec![sk]));
    let r0 = geometry::root(&mut doc, &format!("moby_{:04}", lm.o_class), scene0);
    doc.scene("high_lod", std::iter::once(r0).chain(skel_root).collect(), None);
    if !scene1.is_empty() {
        let r1 = geometry::root(&mut doc, &format!("moby_{:04}_low", lm.o_class), scene1);
        doc.scene("low_lod", std::iter::once(r1).chain(skel_root).collect(), Some(Obj::new().set("note", "low LOD, drawn beyond lod_trans × 1024 depth; same skeleton").build()));
    }
    let skeleton: Vec<J> = anim.skeleton.iter().map(|m| J::Arr(m.iter().flatten().map(|&v| J::F32(v)).collect())).collect();
    doc.extras.put("o_class", lm.o_class);
    doc.extras.put("level", ld.id);
    doc.extras.put("source", ld.rel("core_data.bin"));
    doc.extras.put("header", Obj::new().set("scale", h.scale).set("joint_count", h.joint_count).set("low_lod_joint_count", h.low_lod_joint_count)
        .set("high_lod_count", h.high_lod_count).set("low_lod_count", h.low_lod_count).set("metal_count", h.metal_count).set("sequence_count", h.sequence_count)
        .set("lod_trans", h.lod_trans).set("mip_dist", h.mip_dist).set("bsphere", h.bsphere).set("mode_bits", hex(h.mode_bits as u16 as u64))
        .set("mode_bits2", h.mode_bits2).set("ty", h.ty).set("glow_rgba", hex(h.glow_rgba as u32 as u64)));
    doc.extras.put("class_textures", &lm.entry.textures[..]);
    doc.extras.put("sequences", n_seqs);
    doc.extras.put("skinning", "joint node j = the game's palette entry F_j = P_j·S_j (stored pose → posed); inverse bind = identity; the extra 'static' joint is the identity the game uses for low-LOD lists drawn with joint count 0");
    doc.extras.put("skeleton_rows", J::Arr(skeleton));
    doc.extras.put("joint_parents", J::Arr((0..jc).map(|j| J::from(anim.parent(j))).collect()));
    doc.extensions_used.push("KHR_materials_unlit");
    let (json, bin) = doc.finish(&format!("{:04}.bin", lm.o_class), geometry::GENERATOR);
    out.write(&format!("{dir}/{:04}.bin", lm.o_class), &bin)?;
    out.write(&format!("{dir}/{:04}.gltf", lm.o_class), json.as_bytes())
}

/// Every moby class of one level.
pub(crate) fn export_level(data: &Path, out: &Out, id: u32) -> Result<(), Error> {
    let ld = LevelData::load(data, id)?;
    let tex = textures::index_level(&ld)?;
    let mobys = moby::parse_level_mobys(&ld.core, &ld.core_data).map_err(|e| data::damaged(&ld.rel("core_data.bin"), e))?;
    for lm in &mobys { class_model(out, &ld, &tex, lm)?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decompose_rebuilds_rotation_translation_and_scale() {
        // 90° about z (column-vector), scale (2, 3, 4), translation (10, 20, 30) packed.
        // Rows as stored (row-vector form): row j = image of basis j = column j of the column-vector matrix.
        let m: Rows = [[0.0, 2.0, 0.0, 0.0], [-3.0, 0.0, 0.0, 0.0], [0.0, 0.0, 4.0, 0.0], [10.0, 20.0, 30.0, 1.0]];
        let (t, q, s, err) = decompose(&m, 0.5);
        assert_eq!(t, [5.0, 10.0, 15.0]);
        assert!(s.iter().zip([2.0, 3.0, 4.0]).all(|(a, b)| (a - b).abs() < 1e-6), "{s:?}");
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert!(q.iter().zip([0.0, 0.0, h, h]).all(|(a, b)| (a - b).abs() < 1e-6), "{q:?}");
        assert!(err < 1e-6);
        let (_, q, s, _) = decompose(&moby_anim::IDENTITY, 1.0);
        assert_eq!((q, s), ([0.0, 0.0, 0.0, 1.0], [1.0; 3]));
    }
}
