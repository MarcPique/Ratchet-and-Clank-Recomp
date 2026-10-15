//! Golden tests of the Rust loaders on all 19 levels: every section they used to compare byte for byte with the
//! retired C++ reference extractor's dumps is now hashed into the committed snapshot table
//! (`data/loader_snapshots.tsv`, `tests/formats/snapshot/mod.rs`), and the structural invariants are checked directly.
//! They are skipped when the extraction is not present (the data never ships with the repo).

use crate::snapshot;

use rc_formats::{level, texture, toc, wad};
use snapshot::{lv, Snap};
use std::path::PathBuf;

fn extracted() -> Option<PathBuf> {
    let root = rc_formats::test_data::root();
    root.join("toc.bin").exists().then_some(root)
}

#[test]
fn toc_level_table() {
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let toc_bytes = std::fs::read(root.join("toc.bin")).unwrap();
    assert_eq!(toc_bytes.len(), toc::TOC_SIZE);
    let sectors = toc::level_header_sectors(&toc_bytes).unwrap();
    assert_eq!(sectors.iter().filter(|s| s.is_some()).count(), 19);
    for (i, s) in sectors.iter().enumerate() {
        let hdr = std::fs::read(root.join(format!("levels/{i:02}/level_header.bin"))).unwrap();
        let h = toc::parse_level_header(&hdr).unwrap();
        assert_eq!(h.id as usize, i);
        assert!(s.is_some());
        assert_eq!(h.data.offset as u32, s.unwrap() + 5, "level {i}: data follows the 0x2434 header rounded up to 5 sectors");
    }
}

/// Every WAD lump of the archive (`global/**` and the top level of `levels/NN/`; scene regions hold several WADs
/// and are covered by the scene test): the Tier 0 path without the `.bin` extension, its snapshot scope and key.
fn wad_lumps(root: &std::path::Path) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![root.join("global")];
    for i in 0..19 {
        for e in std::fs::read_dir(root.join(format!("levels/{i:02}"))).unwrap().flatten() {
            if e.path().is_file() { stack.push(e.path()); }
        }
    }
    while let Some(p) = stack.pop() {
        if p.is_dir() { stack.extend(std::fs::read_dir(&p).unwrap().flatten().map(|e| e.path())); continue; }
        if p.extension().is_none_or(|x| x != "bin") { continue; }
        let mut magic = [0u8; 3];
        let is_wad = std::fs::File::open(&p).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut magic)).is_ok() && &magic == b"WAD";
        if !is_wad { continue; }
        let rel = p.strip_prefix(root).unwrap().with_extension("").to_string_lossy().replace('\\', "/");
        let (scope, key) = match rel.strip_prefix("levels/") { Some(r) => (r[..2].to_string(), r[3..].to_string()), None => ("global".into(), rel.clone()) };
        out.push((rel, scope, key));
    }
    out.sort();
    out
}

#[test]
fn wad_decompression_for_every_lump() {
    let Some(root) = extracted() else { return; };
    let mut snap = Snap::new("wad");
    let lumps = wad_lumps(&root);
    let (mut n, mut bytes) = (0usize, 0usize);
    for (rel, scope, key) in &lumps {
        let got = wad::decompress(&std::fs::read(root.join(format!("{rel}.bin"))).unwrap()).unwrap();
        snap.add(scope, key, &got);
        n += 1;
        bytes += got.len();
    }
    eprintln!("wad: {n} lumps decompressed, {:.1} MiB", bytes as f64 / (1 << 20) as f64);
    snap.finish();
}

#[test]
fn core_index_blocks_tile_the_data() {
    let Some(root) = extracted() else { return; };
    let mut snap = Snap::new("core_blocks");
    for i in 0..19 {
        let idx = std::fs::read(root.join(format!("levels/{i:02}/core_index.bin"))).unwrap();
        let data = wad::decompress(&std::fs::read(root.join(format!("levels/{i:02}/core_data.bin"))).unwrap()).unwrap();
        let dec_len = data.len();
        let core = level::parse_level_core(&idx, dec_len).unwrap();
        assert_eq!(core.header.assets_decompressed_size as usize, dec_len);
        let mut blocks = core.blocks.clone();
        blocks.sort_by_key(|b| b.offset);
        let covered: usize = blocks.iter().map(|b| b.size).sum();
        assert_eq!(covered, dec_len, "level {i}: blocks must cover the data exactly");
        for w in blocks.windows(2) { assert!(w[0].offset + w[0].size <= w[1].offset, "level {i}: overlapping blocks"); }
        let names: std::collections::BTreeSet<&str> = core.blocks.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names.len(), core.blocks.len(), "level {i}: block names are unique");
        // The block list (name, offset, size) in index order; the bytes are the core data's (wad snapshot).
        for b in &core.blocks {
            snap.add(&lv(i), "core.blocks", &[b.name.as_bytes(), &(b.offset as u64).to_le_bytes(), &(b.size as u64).to_le_bytes()].concat());
            assert_eq!(core.block(&data, &b.name).unwrap().len(), b.size);
        }
    }
    snap.finish();
}

/// The gameplay file's pointer-table sections (`gameplay::sections`), which tests read `level_settings` through.
#[test]
fn gameplay_sections_for_every_level() {
    let Some(root) = extracted() else { return; };
    let mut snap = Snap::new("gameplay_sections");
    for i in 0..19 {
        let gp = wad::decompress(&std::fs::read(root.join(format!("levels/{i:02}/gameplay_ntsc.bin"))).unwrap()).unwrap();
        let secs = rc_formats::gameplay::sections(&gp).unwrap();
        assert!(secs.windows(2).all(|w| w[0].1.end == w[1].1.start) && secs.last().unwrap().1.end == gp.len(), "level {i}: sections tile the file");
        assert!(secs.iter().any(|s| s.0 == "level_settings"), "level {i}: no level settings");
        for (name, r) in &secs {
            snap.add(&lv(i), "gameplay.sections", &[name.as_bytes(), &(r.start as u64).to_le_bytes(), &(r.len() as u64).to_le_bytes()].concat());
        }
    }
    snap.finish();
}

/// Every level texture (tfrag, moby, tie, shrub tables and shrub billboards) decoded to RGBA, in table order:
/// per table and level the key, size and pixels of each texture are one snapshot item.
#[test]
fn textures_for_every_level() {
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let mut snap = Snap::new("textures");
    let mut per_table = std::collections::BTreeMap::<&str, usize>::new();
    let mut total = 0;
    for i in 0..19 {
        let dir = root.join(format!("levels/{i:02}"));
        let idx = std::fs::read(dir.join("core_index.bin")).unwrap();
        let data = wad::decompress(&std::fs::read(dir.join("core_data.bin")).unwrap()).unwrap();
        let gs = std::fs::read(dir.join("gs_ram.bin")).unwrap();
        let core = level::parse_level_core(&idx, data.len()).unwrap();
        let got = texture::parse_textures(&core, &data, &gs).unwrap();
        for g in &got {
            let t = &g.texture;
            assert_eq!(t.rgba.len(), t.width as usize * t.height as usize * 4, "level {i} {}: RGBA size", g.key());
            *per_table.entry(g.table.name()).or_default() += 1;
            snap.add(&lv(i), &format!("texture.{}", g.table.name()), &[g.key().as_bytes(), &t.width.to_le_bytes(), &t.height.to_le_bytes(), &t.rgba].concat());
        }
        total += got.len();
    }
    eprintln!("textures: {total} {per_table:?}");
    assert_eq!(total, 9104, "retail disc total (docs/formats/textures_rac1.md 12b)");
    snap.finish();
}

/// Section names of one tfrag record (the layout of the retired C++ `tfrag_dump.bin`; one snapshot row each).
const TFRAG_SECTIONS: [&str; 25] = [
    "header", "vu", "origin", "tier_counts", "positions", "vertex_info",
    "parent_indices_lod01", "unk_indices_2_lod01", "parent_indices_lod0", "unk_indices_2_lod0",
    "strips_lod0", "strips_lod1", "strips_lod2", "indices_lod0", "indices_lod1", "indices_lod2",
    "ad_gifs", "rgba", "lights", "mspheres", "cube",
    "triangles_lod0", "triangles_lod1", "triangles_lod2", "world_positions",
];

/// Serialises a Rust-parsed tfrag, one byte vector per section.
fn tfrag_sections(t: &rc_formats::tfrag::Tfrag) -> Vec<Vec<u8>> {
    use bytemuck::{bytes_of, cast_slice};
    use rc_formats::tfrag::tfrag_triangles;
    let tiers = [t.positions_common, t.positions_lod01, t.positions_lod0, t.vinfo_common, t.vinfo_lod01, t.vinfo_lod0];
    let mut s: Vec<Vec<u8>> = vec![
        bytes_of(&t.header).to_vec(), bytes_of(&t.vu).to_vec(), bytes_of(&t.origin).to_vec(), bytes_of(&tiers).to_vec(),
        cast_slice(&t.positions).to_vec(), cast_slice(&t.vertex_info).to_vec(),
        t.parent_indices_lod01.clone(), t.unk_indices_2_lod01.clone(), t.parent_indices_lod0.clone(), t.unk_indices_2_lod0.clone(),
    ];
    for l in &t.lod { s.push(cast_slice(&l.strips).to_vec()); }
    for l in &t.lod { s.push(l.indices.clone()); }
    s.push(cast_slice(&t.ad_gifs).to_vec());
    s.push(cast_slice(&t.rgba).to_vec());
    s.push(cast_slice(&t.lights).to_vec());
    s.push(cast_slice(&t.mspheres).to_vec());
    s.push(bytes_of(&t.cube).to_vec());
    for lod in 0..3 {
        let tris = tfrag_triangles(t, lod).unwrap();
        s.push(tris.iter().flat_map(|tr| [tr.a, tr.b, tr.c, tr.ad_gif]).flat_map(u16::to_le_bytes).collect());
    }
    s.push((0..t.vertex_info.len()).flat_map(|i| t.world_position(i)).flat_map(f32::to_le_bytes).collect());
    s
}

#[test]
fn tfrags_for_every_level() {
    use rc_formats::tfrag;
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let mut snap = Snap::new("tfrag");
    let (mut n_tfrags, mut n_vinfo, mut n_pos, mut n_tris) = (0usize, 0usize, 0usize, [0usize; 3]);
    let (mut n_kicks, mut n_kick_loads) = (0usize, 0usize);
    for i in 0..19 {
        let dir = root.join(format!("levels/{i:02}"));
        // Rust path end to end: WAD decompression, core index, block split, tfrag parse.
        let idx = std::fs::read(dir.join("core_index.bin")).unwrap();
        let data = wad::decompress(&std::fs::read(dir.join("core_data.bin")).unwrap()).unwrap();
        let core = level::parse_level_core(&idx, data.len()).unwrap();
        let block = tfrag::tfrag_block(&core, &data).unwrap();
        let tfrags = tfrag::parse_tfrags(block).unwrap();
        snap.add(&lv(i), "tfrag.block_header", &block[..16]);
        for (ti, t) in tfrags.iter().enumerate() {
            for (name, ours) in TFRAG_SECTIONS.iter().zip(tfrag_sections(t)) { snap.add(&lv(i), &format!("tfrag.{name}"), &ours); }
            assert_eq!(t.positions.len(), t.header.vert_count as usize, "level {i} tfrag {ti}: vert_count");
            // Structural invariants the renderer relies on:
            // each vertex-info tier is [one per position] ++ [extra], the extra range's count and
            // VU address are the "unknown" header lanes, and index arrays are sized by those counts.
            let v = t.vu;
            let pad4 = |n: u16| (n as usize).div_ceil(4) * 4;
            assert_eq!([t.positions_common, t.positions_lod01, t.positions_lod0],
                       [v.positions_common_count as u32, v.positions_lod_01_count as u32, v.positions_lod_0_count as u32], "level {i} tfrag {ti}: position tiers");
            assert_eq!([t.vinfo_common, t.vinfo_lod01, t.vinfo_lod0],
                       [(v.positions_common_count + v.unk_02) as u32, (v.positions_lod_01_count + v.unk_06) as u32, (v.positions_lod_0_count + v.unk_0a) as u32],
                       "level {i} tfrag {ti}: vertex-info tiers");
            assert_eq!([v.unk_10, v.unk_14, v.unk_18],
                       [v.vertex_info_common_addr + v.positions_common_count, v.vertex_info_lod_01_addr + v.positions_lod_01_count, v.vertex_info_lod_0_addr + v.positions_lod_0_count],
                       "level {i} tfrag {ti}: extra vertex-info addresses");
            assert_eq!([t.parent_indices_lod01.len(), t.unk_indices_2_lod01.len(), t.parent_indices_lod0.len(), t.unk_indices_2_lod0.len()],
                       [pad4(v.positions_lod_01_count), pad4(v.unk_06), pad4(v.positions_lod_0_count), pad4(v.unk_0a)], "level {i} tfrag {ti}: index array sizes");
            assert_eq!(tfrag::tfrag_triangles(t, 0).unwrap().len(), t.header.tri_count as usize, "level {i} tfrag {ti}: tri_count");
            n_vinfo += t.vertex_info.len();
            n_pos += t.positions.len();
            for (lod, n) in n_tris.iter_mut().enumerate() {
                let tris = tfrag::tfrag_triangles(t, lod).unwrap();
                assert!(tris.iter().all(|tr| (tr.ad_gif as usize) < t.ad_gifs.len()), "level {i} tfrag {ti} lod {lod}: ad-gif index out of range");
                *n += tris.len();
                // Every list opens with an ad-gif load; a kick record either loads an ad-gif (z >= 0) or has z = -1.
                let s = &t.lod[lod].strips;
                assert!(s[0].vertex_count_and_flag < 0 && s[0].end_of_packet_flag >= 0, "level {i} tfrag {ti} lod {lod}: first strip record");
                for r in s.iter().take_while(|r| r.vertex_count_and_flag != 0).filter(|r| r.vertex_count_and_flag < 0 && r.end_of_packet_flag < 0) {
                    assert!(r.ad_gif_offset >= -1, "level {i} tfrag {ti} lod {lod}: kick record z");
                    n_kicks += 1;
                    n_kick_loads += (r.ad_gif_offset >= 0) as usize;
                }
            }
        }
        n_tfrags += tfrags.len();
    }
    // Spec 2.6: kicks that also load an ad-gif (VU1 L81 `ibgez vi13, L82`).
    assert_eq!((n_kick_loads, n_kicks), (12_221, 28_838), "kick records that load an ad-gif / all kick records");
    eprintln!("tfrags: 19 levels, {n_tfrags} tfrags, {n_pos} positions, {n_vinfo} vertex-info entries, triangles LOD0/1/2 = {n_tris:?}, \
               {n_kick_loads}/{n_kicks} kicks load an ad-gif");
    snap.finish();
}

/// Serialises one Rust-parsed moby packet, one byte vector per section (the layout of the retired C++
/// `moby_dump.bin`; one snapshot row each).
fn moby_packet_sections(p: &rc_formats::moby::MobySubmesh) -> Vec<Vec<u8>> {
    use bytemuck::{bytes_of, cast_slice};
    let mut recs = Vec::new();
    for v in &p.vertices {
        let n = rc_formats::moby::moby_normal(v.normal_azimuth, v.normal_elevation);
        for h in [v.x, v.y, v.z] { recs.extend(h.to_le_bytes()); }
        recs.extend([v.normal_azimuth, v.normal_elevation]);
        recs.extend(v.id.to_le_bytes());
        recs.extend([v.kind as u8, v.duplicate as u8, v.skin.count]);
        recs.extend(v.skin.joints);
        for w in v.skin.weights { recs.extend(w.to_le_bytes()); }
        for s in v.st { recs.extend(s.to_le_bytes()); }
        for f in n { recs.extend(f.to_le_bytes()); }
    }
    vec![
        bytes_of(&p.entry).to_vec(),
        if p.is_metal { bytes_of(&p.metal_header).to_vec() } else { bytes_of(&p.vertex_table).to_vec() },
        cast_slice(&p.transfers).to_vec(), cast_slice(&p.duplicates).to_vec(), p.raw_vertices.clone(), p.rgba_multipliers.clone(),
        cast_slice(&p.st).to_vec(), p.index_bytes.clone(), p.secret_indices.clone(), cast_slice(&p.texture_indices).to_vec(),
        [p.initial_texture, p.unresolved_duplicates as i32].iter().flat_map(|x| x.to_le_bytes()).collect(),
        recs,
        cast_slice(&p.triangles).to_vec(),
    ]
}

const MOBY_PACKET_SECTIONS: [&str; 13] = [
    "entry", "table_header", "transfers", "duplicates", "raw_vertices", "rgba_multipliers", "st",
    "index_bytes", "secret_indices", "texture_indices", "state", "vertices", "triangles",
];

#[test]
fn mobys_for_every_level() {
    use bytemuck::{bytes_of, cast_slice};
    use rc_formats::moby;
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let mut snap = Snap::new("moby");
    // The game's normal table (boot ELF 0x165500, 256 x (cos, sin), skinning doc 1/4) agrees with
    // moby_normal's axis convention: entry a is (x, y) of the normal with azimuth a, elevation 0.
    let elf = std::fs::read(root.join("boot/SCUS_971.99")).unwrap();
    let rd = |o: usize| u32::from_le_bytes(elf[o..o + 4].try_into().unwrap()) as usize;
    let (phoff, phnum) = (rd(0x1c), u16::from_le_bytes([elf[0x2c], elf[0x2d]]) as usize);
    let seg = (0..phnum).map(|k| phoff + k * 32).find(|&p| (rd(p + 8)..rd(p + 8) + rd(p + 16)).contains(&0x165500)).expect("table segment");
    let table = rd(seg + 4) + 0x165500 - rd(seg + 8);
    for a in 0..256 {
        let n = moby::moby_normal(a as u8, 0);
        let (c, s) = (f32::from_bits(rd(table + a * 8) as u32), f32::from_bits(rd(table + a * 8 + 4) as u32));
        assert!((n[0] - c).abs() < 4e-7 && (n[1] - s).abs() < 4e-7, "trig table entry {a}: ({c}, {s}) vs {n:?}");
    }
    let (mut n_classes, mut n_packets, mut n_verts, mut n_dupes, mut n_tris) = (0usize, [0usize; 3], 0usize, 0usize, [0usize; 3]);
    let mut n_skin = [0usize; 4];
    for i in 0..19 {
        let dir = root.join(format!("levels/{i:02}"));
        // Rust path end to end: WAD decompression, core index, block split, moby parse.
        let idx = std::fs::read(dir.join("core_index.bin")).unwrap();
        let data = wad::decompress(&std::fs::read(dir.join("core_data.bin")).unwrap()).unwrap();
        let core = level::parse_level_core(&idx, data.len()).unwrap();
        let mut classes = moby::parse_level_mobys(&core, &data).unwrap();
        classes.sort_by_key(|c| c.o_class);

        for lc in &classes {
            let (o, mc) = (lc.o_class, &lc.class);
            snap.add(&lv(i), "moby.o_class", &o.to_le_bytes());
            let h = &mc.header;
            let class_secs: [(&str, Vec<u8>); 5] = [
                ("header", bytes_of(h).to_vec()),
                ("sequence_pointers", cast_slice(&mc.sequence_pointers).to_vec()),
                ("skeleton", cast_slice(&mc.skeleton.matrices).to_vec()),
                ("common_trans", cast_slice(&mc.skeleton.trans).to_vec()),
                ("packet_counts", cast_slice(&[mc.high_lod.len() as u32, mc.low_lod.len() as u32, mc.metal.len() as u32]).to_vec()),
            ];
            for (name, ours) in class_secs {
                snap.add(&lv(i), &format!("moby.{name}"), &ours);
            }
            for (li, list) in [&mc.high_lod, &mc.low_lod, &mc.metal].into_iter().enumerate() {
                for p in list {
                    for (name, ours) in MOBY_PACKET_SECTIONS.iter().zip(moby_packet_sections(p)) {
                        snap.add(&lv(i), &format!("moby.packet.{name}"), &ours);
                    }
                    // Invariants the renderer relies on.
                    assert_eq!(moby::moby_triangles(p).unwrap(), p.triangles);
                    assert_eq!(p.unresolved_duplicates, 0, "level {i} class {o}: unresolved duplicate");
                    assert!(p.rgba_multiplier_records().iter().flatten().all(|&m| m == 0x80), "level {i} class {o}: multiplier != 0x80");
                    assert!(p.rgba_multipliers[p.rgba_multiplier_records().len() * 4..].iter().all(|&m| m == 0), "level {i} class {o}: multiplier padding");
                    let joint_limit = if li == 1 { h.low_lod_joint_count } else { h.joint_count }.max(1);
                    for v in &p.vertices {
                        let s = v.skin;
                        assert!((1..=3).contains(&s.count) && s.weights.iter().sum::<u16>() == 256, "level {i} class {o}: skin {s:?}");
                        assert!(s.joints[..s.count as usize].iter().all(|&j| j < joint_limit), "level {i} class {o} list {li}: joint {s:?} >= {joint_limit}");
                        n_skin[s.count as usize] += 1;
                    }
                    for t in &p.triangles {
                        if p.is_metal { assert!(t.texture == -2 || t.texture == -3, "level {i} class {o}: metal texture {}", t.texture); }
                        else { assert!(t.texture == -1 || lc.texture_table_index(t.texture).is_some_and(|x| x < core.moby_textures.len()), "level {i} class {o}: texture {}", t.texture); }
                    }
                    n_packets[li] += 1;
                    n_tris[li] += p.triangles.len();
                    n_verts += p.vertices.len();
                    n_dupes += p.duplicates.len();
                }
            }
            for j in 1..mc.skeleton.trans.len() { assert!(mc.skeleton.parent(j).unwrap() < j, "level {i} class {o}: joint {j} parent not earlier"); }
        }
        n_classes += classes.len();
    }
    eprintln!("mobys: 19 levels, {n_classes} classes, packets high/low/metal = {n_packets:?}, {n_verts} vertices ({n_dupes} duplicates), \
               triangles high/low/metal = {n_tris:?}, skins by joint count 1/2/3 = {:?}", &n_skin[1..]);
    snap.finish();
}

#[test]
fn ties_for_every_level() {
    use bytemuck::{bytes_of, cast_slice};
    use rc_formats::tie::{self, TiePacket};
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let mut snap = Snap::new("tie");
    // Serialises one Rust-parsed packet, one byte vector per section (the layout of the retired C++ `tie_dump.bin`).
    fn packet_sections(p: &TiePacket) -> Vec<Vec<u8>> {
        let mut draws = Vec::new();
        for d in &p.draws {
            draws.extend([d.ad_gif, d.winding]);
            draws.extend((d.vertices.len() as u16).to_le_bytes());
            for &v in &d.vertices { draws.extend(v.to_le_bytes()); }
        }
        let ad: Vec<i32> = p.ad_gif_dest.iter().chain(&p.ad_gif_src).copied().collect();
        vec![
            bytes_of(&p.header).to_vec(), cast_slice(&ad).to_vec(), bytes_of(&p.unpack).to_vec(),
            cast_slice(&p.strips).to_vec(), cast_slice(&p.dinky).to_vec(), cast_slice(&p.fat).to_vec(),
            p.colors.clone(), p.colors_b.clone(), p.slot_table.clone(), cast_slice(&p.vertices).to_vec(), draws,
            cast_slice(&tie::tie_triangles(p)).to_vec(),
        ]
    }
    type Strips = Vec<(u8, Vec<([i16; 3], [i16; 2])>)>;
    const PACKET_SECTIONS: [&str; 12] = [
        "packet_header", "ad_gif_dest_src", "unpack", "strips", "dinky", "fat", "colors", "colors_b", "slot_table", "vertices", "draws", "triangles",
    ];
    // Independent cross-check of the VU1 walker: Wrench's reconstruction (duplicate a vertex whose second
    // slot is set, sort by slot, drop repeated slots, then advance a cursor 6/1/3 qw per ad-gif/tag/vertex)
    // must yield the same strips (material + vertex data) on every retail packet.
    fn wrench_walk(p: &TiePacket) -> Strips {
        let mut vs: Vec<(u16, [i16; 3], [i16; 2])> = Vec::new();
        let mut add = |s: u16, s2: u16, pos: [i16; 3], st: [i16; 2]| { vs.push((s, pos, st)); if s2 != 0 && s2 != s { vs.push((s2, pos, st)); } };
        for v in &p.dinky { add(v.gs_slot, v.gs_slot_2, [v.x, v.y, v.z], [v.s, v.t]); }
        for v in &p.fat { add(v.gs_slot, v.gs_slot_2, [v.x, v.y, v.z], [v.s, v.t]); }
        vs.sort_by_key(|v| v.0);
        vs.dedup_by_key(|v| v.0);
        let (mut cursor, mut si, mut vi, mut next_ad, mut mat) = (6u32, 0, 0, 1, (p.ad_gif_src[0] / 0x50) as u8);
        let mut out: Strips = Vec::new();
        while si < p.strips.len() || vi < vs.len() {
            if si < p.strips.len() && p.strips[si].gif_tag_offset as u32 == cursor { out.push((mat, Vec::new())); cursor += 1; si += 1; }
            else if vi < vs.len() && vs[vi].0 as u32 == cursor { out.last_mut().unwrap().1.push((vs[vi].1, vs[vi].2)); cursor += 3; vi += 1; }
            else if next_ad < 4 && p.ad_gif_dest[next_ad - 1] == cursor as i32 { mat = (p.ad_gif_src[next_ad] / 0x50) as u8; cursor += 6; next_ad += 1; }
            else { panic!("wrench walk stuck at {cursor}"); }
        }
        out
    }
    let (mut n_classes, mut n_packets, mut n_verts, mut n_fat, mut n_tris, mut n_inst, mut n_draws) = (0usize, [0usize; 3], 0usize, 0usize, [0usize; 3], 0usize, 0usize);
    for i in 0..19 {
        let dir = root.join(format!("levels/{i:02}"));
        // Rust path end to end: WAD decompression, core index, block split, tie parse; instances from the gameplay file.
        let idx = std::fs::read(dir.join("core_index.bin")).unwrap();
        let data = wad::decompress(&std::fs::read(dir.join("core_data.bin")).unwrap()).unwrap();
        let core = level::parse_level_core(&idx, data.len()).unwrap();
        let mut classes = tie::parse_level_ties(&core, &data).unwrap();
        classes.sort_by_key(|c| c.o_class);
        let gameplay = wad::decompress(&std::fs::read(dir.join("gameplay_ntsc.bin")).unwrap()).unwrap();
        let instances = tie::parse_tie_instances(&gameplay).unwrap();

        for lc in &classes {
            let (o, tc) = (lc.o_class, &lc.class);
            snap.add(&lv(i), "tie.o_class", &o.to_le_bytes());
            let class_secs: [(&str, Vec<u8>); 4] = [
                ("header", bytes_of(&tc.header).to_vec()), ("header_ext", tc.header_ext.clone()),
                ("normals", cast_slice(&tc.normals).to_vec()), ("ad_gifs", cast_slice(&tc.ad_gifs).to_vec()),
            ];
            for (name, ours) in class_secs {
                snap.add(&lv(i), &format!("tie.{name}"), &ours);
            }
            // Invariants.
            let h = &tc.header;
            assert_eq!(h.o_class, o, "level {i} class {o}: header o_class");
            assert_eq!(h.normals + 0x200, h.ad_gif_ofs, "level {i} class {o}: normals end at the ad-gifs");
            assert_eq!(lc.entry.textures.iter().take_while(|&&t| t != 0xff).count(), h.texture_count as usize, "level {i} class {o}: texture slots");
            for (lod, list) in tc.lods.iter().enumerate() {
                let (mut sv, mut st, mut ns) = (0u32, 0u32, 0u32);
                for (pi, p) in list.iter().enumerate() {
                    for (name, ours) in PACKET_SECTIONS.iter().zip(packet_sections(p)) {
                        snap.add(&lv(i), &format!("tie.packet.{name}"), &ours);
                    }
                    let vu: Strips = p.draws.iter()
                        .map(|d| (d.ad_gif, d.vertices.iter().map(|&v| (p.vertices[v as usize].position, p.vertices[v as usize].st)).collect())).collect();
                    assert!(vu == wrench_walk(p), "level {i} class {o} lod {lod} packet {pi}: VU1 walk differs from Wrench's reconstruction");
                    let used = p.vertices.iter().flat_map(|v| [v.color, v.morph_colors[0], v.morph_colors[1]]);
                    assert!(used.clone().all(|c| c < 64), "level {i} class {o}: light slot >= 64");
                    let d = p.dinky.len();
                    let used_bytes = (0..d).chain((0..p.fat.len()).flat_map(|j| (0..3).map(move |c| d.div_ceil(4) * 4 + 4 * j + c)));
                    assert!(used_bytes.clone().all(|b| p.colors_b[b] == p.colors[b] + 0x40), "level {i} class {o}: colour copy B != A + 0x40");
                    let tris = tie::tie_triangles(p);
                    assert!(tris.iter().all(|t| lc.texture_table_index(t.ad_gif).is_some_and(|x| x < core.tie_textures.len())), "level {i} class {o}: texture");
                    sv += p.strips.iter().map(|s| s.vertex_count as u32).sum::<u32>();
                    st += tris.len() as u32;
                    ns += p.strips.len() as u32;
                    n_packets[lod] += 1;
                    n_tris[lod] += tris.len();
                    n_verts += p.vertices.len();
                    n_fat += p.fat.len();
                    n_draws += p.draws.len();
                }
                snap.add(&lv(i), "tie.lod_packets", &(list.len() as u32).to_le_bytes());
                assert_eq!([h.lod_info[lod].strip_vertex_count, h.lod_info[lod].triangle_count, h.lod_info[lod].strip_count], [sv, st, ns], "level {i} class {o}: lod_info {lod}");
            }
        }
        snap.add(&lv(i), "tie.instances", cast_slice(&instances));
        for inst in &instances { assert!(classes.iter().any(|c| c.o_class == inst.o_class), "level {i}: instance class {} unresolved", inst.o_class); }
        n_classes += classes.len();
        n_inst += instances.len();
    }
    eprintln!("ties: 19 levels, {n_classes} classes, packets LOD0/1/2 = {n_packets:?}, {n_draws} strips, {n_verts} vertices ({n_fat} fat), \
               triangles LOD0/1/2 = {n_tris:?}, {n_inst} instances");
    snap.finish();
}

#[test]
fn sky_for_every_level() {
    use bytemuck::{bytes_of, cast_slice};
    use rc_formats::sky;
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let snap = std::cell::RefCell::new(Snap::new("sky"));
    let (mut n_levels, mut n_shells, mut n_gouraud, mut n_clusters, mut n_verts, mut n_tris, mut n_tex) = (0, 0, 0, 0, 0, 0, 0);
    for i in 0..19 {
        let dir = root.join(format!("levels/{i:02}"));
        // Rust path end to end: WAD decompression, core index, block split, sky parse.
        let idx = std::fs::read(dir.join("core_index.bin")).unwrap();
        let data = wad::decompress(&std::fs::read(dir.join("core_data.bin")).unwrap()).unwrap();
        let core = level::parse_level_core(&idx, data.len()).unwrap();
        let block = sky::sky_block(&core, &data).unwrap().unwrap_or_else(|| panic!("level {i}: no sky block"));
        let s = sky::parse_sky(&core, &data).unwrap().unwrap();
        let textures = sky::parse_sky_textures(block, &s).unwrap();

        // One row per section of the retired C++ `sky_dump.bin` layout, plus its counts and texture sizes.
        let check = |name: &str, ours: &[u8]| snap.borrow_mut().add(&lv(i), &format!("sky.{name}"), ours);
        let count = |name: &str, n: u32| snap.borrow_mut().add(&lv(i), &format!("sky.{name}"), &n.to_le_bytes());
        check("header", bytes_of(&s.header));
        check("fx_list", &s.fx_list);
        check("texture_defs", cast_slice(&s.texture_defs));
        count("shell_count", s.shells.len() as u32);
        for (si, sh) in s.shells.iter().enumerate() {
            check("shell", cast_slice(&[sh.cluster_count, sh.flags]));
            let headers: Vec<_> = sh.clusters.iter().map(|c| c.header).collect();
            check("cluster_headers", cast_slice(&headers));
            let so = s.header.shells[si] as usize;
            assert!(block[so + 8..so + 0x10].iter().all(|&b| b == 0), "level {i} shell {si}: header padding not zero");
            assert!(sh.flags == 0 || sh.flags == 1, "level {i} shell {si}: flags {}", sh.flags);
            for c in &sh.clusters {
                check("vertices", cast_slice(&c.vertices));
                check("st", cast_slice(&c.attrs));
                check("faces", cast_slice(&c.faces));
                let gs = sky::sky_gs_vertices(sh, c);
                check("gs_vertices", cast_slice(&gs));
                // A shell is all-textured or all-gouraud.
                assert!(c.faces.iter().all(|f| (f.texture == 0xff) != sh.textured()), "level {i} shell {si}: mixed textured/untextured faces");
                assert_eq!(c.header.vertex_offset, 0, "level {i}: non-zero vertex_offset");
                n_verts += c.vertices.len();
                n_tris += c.faces.len();
            }
            n_clusters += sh.clusters.len();
            n_gouraud += usize::from(!sh.textured());
        }
        count("texture_count", textures.len() as u32);
        for t in &textures {
            assert_eq!(t.texture.rgba.len(), t.texture.width as usize * t.texture.height as usize * 4, "level {i}: {} RGBA size", t.key());
            count("texture_size", t.texture.width);
            count("texture_size", t.texture.height);
            check("texture rgba", &t.texture.rgba);
            assert!(t.def.width.count_ones() == 1 && t.def.height.count_ones() == 1, "level {i}: {} not a power of two", t.key());
        }
        n_levels += 1;
        n_shells += s.shells.len();
        n_tex += textures.len();
    }
    eprintln!("sky: {n_levels} levels, {n_shells} shells ({n_gouraud} gouraud), {n_clusters} clusters, {n_verts} vertices, \
               {n_tris} triangles, {n_tex} textures");
    snap.into_inner().finish();
}

#[test]
fn shrubs_for_every_level() {
    use bytemuck::{bytes_of, cast_slice};
    use rc_formats::shrub::{self, ShrubPacket};
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let mut snap = Snap::new("shrub");
    // Serialises one Rust-parsed packet, one byte vector per section (the layout of the retired C++ `shrub_dump.bin`).
    fn packet_sections(p: &ShrubPacket) -> Vec<Vec<u8>> {
        let mut draws = Vec::new();
        for d in &p.draws {
            draws.extend([d.texture, d.prim]);
            draws.extend((d.vertices.len() as u16).to_le_bytes());
            for &v in &d.vertices { draws.extend(v.to_le_bytes()); }
        }
        vec![
            bytes_of(&p.entry).to_vec(), bytes_of(&p.header).to_vec(), cast_slice(&p.gif_tags).to_vec(), cast_slice(&p.ad_gifs).to_vec(),
            cast_slice(&p.part1).to_vec(), cast_slice(&p.part2).to_vec(), cast_slice(&p.vertices).to_vec(), draws,
            cast_slice(&shrub::shrub_triangles(p)).to_vec(),
        ]
    }
    const PACKET_SECTIONS: [&str; 9] = ["entry", "packet_header", "gif_tags", "ad_gifs", "part1", "part2", "vertices", "draws", "triangles"];
    type Strips = Vec<(u8, Vec<([i16; 3], [i16; 2])>)>;
    // Independent cross-check of the VU1 walker: Wrench's reconstruction (a running GS cursor matching the
    // tag / ad-gif / part-1 streams in stored order, 1 / 5 / 3 qw each, stopping at the first padding vertex
    // whose slot repeats the previous one) must yield the same strips on every retail packet.
    fn wrench_walk(p: &ShrubPacket, mut tex: u8) -> Strips {
        let (mut cursor, mut ti, mut ai, mut vi) = (0i32, 0, 0, 0);
        let mut out: Strips = Vec::new();
        while ti < p.gif_tags.len() || ai < p.ad_gifs.len() || vi < p.part1.len() {
            if vi < p.part1.len() && p.part1[vi].gs_packet_offset as i32 == cursor - 3 { break; }
            if ai < p.ad_gifs.len() && p.ad_gifs[ai].gs_packet_offset() == cursor { tex = p.ad_gifs[ai].tex0.data_lo as u8; cursor += 5; ai += 1; }
            else if ti < p.gif_tags.len() && p.gif_tags[ti].gs_packet_offset == cursor { out.push((tex, Vec::new())); cursor += 1; ti += 1; }
            else if vi < p.part1.len() && p.part1[vi].gs_packet_offset as i32 == cursor {
                let (a, b) = (p.part1[vi], p.part2[vi]);
                out.last_mut().unwrap().1.push(([a.x, a.y, a.z], [b.s, b.t]));
                cursor += 3;
                vi += 1;
            } else { panic!("wrench walk stuck at {cursor}"); }
        }
        out
    }
    let (mut n_classes, mut n_packets, mut n_verts, mut n_draws, mut n_tris, mut n_inst, mut n_bb, mut n_bb_tex) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut n_pad = 0usize;
    for i in 0..19 {
        let dir = root.join(format!("levels/{i:02}"));
        // Rust path end to end: WAD decompression, core index, block split, shrub parse; instances from the gameplay file.
        let idx = std::fs::read(dir.join("core_index.bin")).unwrap();
        let data = wad::decompress(&std::fs::read(dir.join("core_data.bin")).unwrap()).unwrap();
        let core = level::parse_level_core(&idx, data.len()).unwrap();
        let mut classes = shrub::parse_level_shrubs(&core, &data).unwrap();
        classes.sort_by_key(|c| c.o_class);
        let gameplay = wad::decompress(&std::fs::read(dir.join("gameplay_ntsc.bin")).unwrap()).unwrap();
        let instances = shrub::parse_shrub_instances(&gameplay).unwrap();

        for lc in &classes {
            let (o, sc) = (lc.o_class, &lc.class);
            snap.add(&lv(i), "shrub.o_class", &o.to_le_bytes());
            snap.add(&lv(i), "shrub.packet_count", &(sc.packets.len() as u32).to_le_bytes());
            let bb: Vec<u8> = sc.billboard.map(|b| bytes_of(&b).to_vec()).unwrap_or_default();
            let class_secs: [(&str, Vec<u8>); 3] = [("header", bytes_of(&sc.header).to_vec()), ("normals", cast_slice(&sc.normals).to_vec()), ("billboard", bb)];
            for (name, ours) in class_secs {
                snap.add(&lv(i), &format!("shrub.{name}"), &ours);
            }
            // Invariants.
            let h = &sc.header;
            assert_eq!(h.o_class as i32, o, "level {i} class {o}: header o_class");
            assert_eq!((h.instance_count, h.instances_pointer, h.drawn_count, h.scis_count, h.billboard_count), (0, 0, 0, 0, 0), "level {i} class {o}: runtime fields");
            for n in 0..24 {
                let v = sc.normal(n).unwrap();
                assert!((v[0] * v[0] + v[1] * v[1] + v[2] * v[2] - 1.0).abs() < 1e-3, "level {i} class {o}: normal {n} not unit");
            }
            assert_eq!(sc.billboard.is_some(), lc.billboard_texture().is_some(), "level {i} class {o}: billboard record vs texture");
            n_bb += sc.billboard.is_some() as usize;
            n_bb_tex += lc.billboard_texture().is_some() as usize;
            let mut tex = 0u8;
            for (pi, p) in sc.packets.iter().enumerate() {
                for (name, ours) in PACKET_SECTIONS.iter().zip(packet_sections(p)) {
                    snap.add(&lv(i), &format!("shrub.packet.{name}"), &ours);
                }
                let vu: Strips = p.draws.iter()
                    .map(|d| (d.texture, d.vertices.iter().map(|&v| (p.vertices[v as usize].position, p.vertices[v as usize].st)).collect())).collect();
                assert!(vu == wrench_walk(p, tex), "level {i} class {o} packet {pi}: VU1 walk differs from Wrench's reconstruction");
                tex = p.draws.last().unwrap().texture;
                assert_eq!(p.vertices.len(), p.header.vertex_count as usize, "level {i} class {o} packet {pi}: stop bit at vertex_count - 4");
                assert_eq!(p.draws.len(), p.gif_tags.len(), "level {i} class {o} packet {pi}: every tag drawn");
                assert!(p.draws.iter().all(|d| d.prim == 4), "level {i} class {o} packet {pi}: strips only");
                assert!(p.vertices.iter().all(|v| v.q == 0x1000), "level {i} class {o} packet {pi}: q");
                let tris = shrub::shrub_triangles(p);
                assert!(tris.iter().all(|t| lc.texture_table_index(t.texture).is_some_and(|x| x < core.shrub_textures.len())), "level {i} class {o}: texture");
                n_pad += p.vertices.len() - p.draws.iter().map(|d| d.vertices.len()).sum::<usize>();
                n_packets += 1;
                n_verts += p.vertices.len();
                n_draws += p.draws.len();
                n_tris += tris.len();
            }
        }
        snap.add(&lv(i), "shrub.instances", cast_slice(&instances));
        for inst in &instances {
            assert!(classes.iter().any(|c| c.o_class == inst.o_class), "level {i}: instance class {} unresolved", inst.o_class);
            assert_eq!((inst.unused_08, inst.unused_0c, inst.unused_5c, inst.unused_64), (0, 0, 0, [0; 3]), "level {i}: unused instance fields");
            assert!(inst.matrix[3][3] == 0.01 || inst.matrix[3][3] == 0.0, "level {i}: matrix [3][3] = {}", inst.matrix[3][3]);
            assert!(inst.matrix.iter().all(|c| c[3] == 0.0 || c[3] == 0.01), "level {i}: matrix w row");
        }
        // The class list at 0x38 is the distinct instance classes in first-appearance order.
        let mut seen = Vec::new();
        for inst in &instances { if !seen.contains(&inst.o_class) { seen.push(inst.o_class); } }
        assert_eq!(shrub::parse_shrub_class_list(&gameplay).unwrap(), seen, "level {i}: shrub class list");
        n_classes += classes.len();
        n_inst += instances.len();
    }
    eprintln!("shrubs: 19 levels, {n_classes} classes ({n_bb} with a billboard record, {n_bb_tex} with a billboard texture), {n_packets} packets, \
               {n_draws} strips, {n_verts} vertices ({n_pad} padding), {n_tris} triangles, {n_inst} instances");
    snap.finish();
}

/// One level through the Rust loaders only: core index, WAD-decompressed core data, raw gs_ram, decompressed NTSC gameplay.
struct RustLevel { core: level::LevelCore, data: Vec<u8>, gs_ram: Vec<u8>, gameplay: Vec<u8> }

fn load_rust_level(root: &std::path::Path, i: usize) -> RustLevel {
    let dir = root.join(format!("levels/{i:02}"));
    let idx = std::fs::read(dir.join("core_index.bin")).unwrap();
    let data = wad::decompress(&std::fs::read(dir.join("core_data.bin")).unwrap()).unwrap();
    let core = level::parse_level_core(&idx, data.len()).unwrap();
    let gs_ram = std::fs::read(dir.join("gs_ram.bin")).unwrap();
    let gameplay = wad::decompress(&std::fs::read(dir.join("gameplay_ntsc.bin")).unwrap()).unwrap();
    RustLevel { core, data, gs_ram, gameplay }
}

/// Every tfrag texture is square, and every mip level `decode_tfrag_mip_levels` finds (`ty` levels: base and
/// mip 1 in core data, mips 2/3 in gs_ram) decodes in bounds. Each level is the 2x2 box average of the one above
/// it within a small mean error (the levels are re-quantised to the same CLUT). Spec: tfrag_rac1.md §2.5.1.
#[test]
fn tfrag_texture_mip_chains() {
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let (mut n_tex, mut n_skipped, mut n_levels) = (0usize, 0usize, [0usize; 5]);
    // Per level pair (0->1, 1->2, 2->3): summed absolute channel error and channel count, RGB and alpha apart.
    let (mut err_rgb, mut cnt_rgb, mut err_a, mut cnt_a) = ([0f64; 3], [0usize; 3], [0f64; 3], [0usize; 3]);
    let mut pairs: Vec<(f64, String)> = Vec::new(); // per-texture MAE of each level pair
    for i in 0..19 {
        let lv = load_rust_level(&root, i);
        for (ti, e) in lv.core.tfrag_textures.iter().enumerate() {
            if e.width <= 0 || e.height <= 0 || e.data_offset < 0 || e.palette < 0 { n_skipped += 1; continue; }
            assert_eq!(e.width, e.height, "level {i} tfrag texture {ti}: not square");
            let mips = texture::decode_tfrag_mip_levels(&lv.core, &lv.data, &lv.gs_ram, e)
                .unwrap_or_else(|err| panic!("level {i} tfrag texture {ti} ({}x{} ty {}): {err}", e.width, e.height, e.ty));
            assert_eq!(mips.len(), e.ty as usize, "level {i} tfrag texture {ti}: level count");
            for (l, m) in mips.iter().enumerate() {
                assert_eq!((m.width, m.height), ((e.width >> l) as u32, (e.height >> l) as u32), "level {i} tfrag texture {ti}: mip {l} size");
            }
            for l in 1..mips.len() {
                let (hi, lo) = (&mips[l - 1], &mips[l]);
                let (mut e_pair, mut n_pair) = (0f64, 0usize);
                for y in 0..lo.height as usize {
                    for x in 0..lo.width as usize {
                        for c in 0..4 {
                            let at = |xx: usize, yy: usize| hi.rgba[(yy * hi.width as usize + xx) * 4 + c] as f64;
                            let avg = (at(2 * x, 2 * y) + at(2 * x + 1, 2 * y) + at(2 * x, 2 * y + 1) + at(2 * x + 1, 2 * y + 1)) / 4.0;
                            let d = (avg - lo.rgba[(y * lo.width as usize + x) * 4 + c] as f64).abs();
                            if c < 3 { err_rgb[l - 1] += d; cnt_rgb[l - 1] += 1; } else { err_a[l - 1] += d; cnt_a[l - 1] += 1; }
                            e_pair += d;
                            n_pair += 1;
                        }
                    }
                }
                pairs.push((e_pair / n_pair as f64, format!("level {i} tex {ti} ({}px) mip {}->{l}", e.width, l - 1)));
            }
            n_tex += 1;
            n_levels[mips.len()] += 1;
        }
    }
    let mae_rgb: Vec<f64> = (0..3).map(|k| err_rgb[k] / cnt_rgb[k].max(1) as f64).collect();
    let mae_a: Vec<f64> = (0..3).map(|k| err_a[k] / cnt_a[k].max(1) as f64).collect();
    let total = (err_rgb.iter().sum::<f64>() + err_a.iter().sum::<f64>()) / (cnt_rgb.iter().sum::<usize>() + cnt_a.iter().sum::<usize>()) as f64;
    eprintln!("tfrag textures: {n_tex} decoded ({n_skipped} empty entries skipped), by level count 1..4 = {:?}; \
               2x2 box-average MAE per channel: overall {total:.3}, RGB by pair 0->1/1->2/2->3 = {mae_rgb:.3?}, alpha = {mae_a:.3?}; \
               {} of {} texture level pairs have MAE >= 2, worst {:.2?}", &n_levels[1..],
              pairs.iter().filter(|p| p.0 >= 2.0).count(), pairs.len(), { pairs.sort_by(|a, b| b.0.total_cmp(&a.0)); &pairs[..6] });
    assert!(total < 2.0, "mip levels are not box averages: MAE {total}");
    for k in 0..3 { assert!(mae_rgb[k] < 2.0 && mae_a[k] < 2.0, "mip pair {k}->{}: MAE rgb {} alpha {}", k + 1, mae_rgb[k], mae_a[k]); }
    // Single pairs can be far off on high-contrast repeating patterns (grilles, meshes: the mips are the same image,
    // resampled with some other filter); a level read from the wrong block would be much further off still.
    assert!(pairs[0].0 < 25.0, "a mip level is not a downsample of the one above: {:?}", pairs[0]);
}

/// The LOD linkage the renderer's morph / collapse passes rely on (tfrag_rac1.md §3b.3, §3b.6), on every retail tfrag:
/// (c) no parent-1 target (`unk_indices_2` for extras, `parent_indices` for primaries) lies in the vertex-info tier
///     the collapse pass rewrites, so a pass never reads an entry it has already overwritten;
/// (d) the primary entries of each tier own its positions one to one (so every morphed position is written once);
/// (e) morph parents lie in coarser tiers: LOD-01 parents in the common tier, LOD-0 parents in common or LOD-01.
#[test]
fn tfrag_lod_links_are_well_formed() {
    use rc_formats::tfrag::{self, TfragMorphTier};
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    // [tier][target tier: common, lod01, lod0]
    let (mut extra_targets, mut primary_targets) = ([[0usize; 3]; 2], [[0usize; 3]; 2]);
    let (mut n_primaries, mut n_identity) = ([0usize; 2], [0usize; 2]);
    // Position tier of parent 2 / parent 1 of primaries, and parent 2 of extras: [tier][common, lod01, lod0].
    let (mut p2_tier, mut p1_tier, mut extra_p2_tier) = ([[0usize; 3]; 2], [[0usize; 3]; 2], [[0usize; 3]; 2]);
    let mut n_tfrags = 0usize;
    for i in 0..19 {
        let lv = load_rust_level(&root, i);
        let tfrags = tfrag::parse_level_tfrags(&lv.core, &lv.data).unwrap();
        for (ti, t) in tfrags.iter().enumerate() {
            let (vc, v01, v0) = (t.vinfo_common as usize, t.vinfo_lod01 as usize, t.vinfo_lod0 as usize);
            let (pc, p01, p0) = (t.positions_common as usize, t.positions_lod01 as usize, t.positions_lod0 as usize);
            let vinfo_tier = |v: usize| if v < vc { 0 } else if v < vc + v01 { 1 } else { 2 };
            let pos_tier = |p: usize| if p < pc { 0 } else if p < pc + p01 { 1 } else { 2 };
            let v = t.vu;
            let tiers = [
                (TfragMorphTier::Lod01, vc, v01, pc, p01, v.positions_lod_01_count as usize, v.unk_06 as usize, &t.parent_indices_lod01, &t.unk_indices_2_lod01),
                (TfragMorphTier::Lod0, vc + v01, v0, pc + p01, p0, v.positions_lod_0_count as usize, v.unk_0a as usize, &t.parent_indices_lod0, &t.unk_indices_2_lod0),
            ];
            for (tn, (tier, vstart, vlen, pstart, plen, n_prim, n_extra, parents, extras)) in tiers.into_iter().enumerate() {
                let own_tier = tn + 1;
                assert_eq!(n_prim + n_extra, vlen, "level {i} tfrag {ti} tier {tier:?}: primaries + extras");
                assert_eq!(n_prim, plen, "level {i} tfrag {ti} tier {tier:?}: one primary per position");
                // (c)
                for (k, &p) in parents[..n_prim].iter().enumerate() {
                    let p = p as usize;
                    assert!(p < vc + v01 + v0, "level {i} tfrag {ti} tier {tier:?} primary {k}: parent 1 {p} out of range");
                    assert_ne!(vinfo_tier(p), own_tier, "level {i} tfrag {ti} tier {tier:?} primary {k}: parent 1 {p} in the rewritten tier");
                    primary_targets[tn][vinfo_tier(p)] += 1;
                }
                for (j, &p) in extras[..n_extra].iter().enumerate() {
                    let p = p as usize;
                    assert!(p < vc + v01 + v0, "level {i} tfrag {ti} tier {tier:?} extra {j}: target {p} out of range");
                    assert_ne!(vinfo_tier(p), own_tier, "level {i} tfrag {ti} tier {tier:?} extra {j}: unk_indices_2 target {p} in the rewritten tier");
                    extra_targets[tn][vinfo_tier(p)] += 1;
                }
                // (d) + (e)
                let mut owned = vec![false; plen];
                for k in 0..n_prim {
                    let l = t.lod_link(vstart + k).unwrap_or_else(|| panic!("level {i} tfrag {ti} tier {tier:?} primary {k}: no LOD link"));
                    assert!(l.morphs && l.tier == tier, "level {i} tfrag {ti} tier {tier:?} primary {k}: link {l:?}");
                    assert!((pstart..pstart + plen).contains(&l.own_position), "level {i} tfrag {ti} tier {tier:?} primary {k}: position {} outside its tier", l.own_position);
                    assert!(!std::mem::replace(&mut owned[l.own_position - pstart], true), "level {i} tfrag {ti} tier {tier:?} primary {k}: position {} owned twice", l.own_position);
                    n_identity[tn] += (l.own_position == pstart + k) as usize;
                    p1_tier[tn][pos_tier(l.parent1_position)] += 1;
                    p2_tier[tn][pos_tier(l.parent2_position)] += 1;
                }
                for j in 0..n_extra {
                    let l = t.lod_link(vstart + n_prim + j).unwrap_or_else(|| panic!("level {i} tfrag {ti} tier {tier:?} extra {j}: no LOD link"));
                    assert!(!l.morphs, "level {i} tfrag {ti} tier {tier:?} extra {j}: morphs");
                    extra_p2_tier[tn][pos_tier(l.parent2_position)] += 1;
                }
                n_primaries[tn] += n_prim;
            }
        }
        n_tfrags += tfrags.len();
    }
    eprintln!("tfrag LOD links, {n_tfrags} tfrags: primaries LOD01/LOD0 = {n_primaries:?} (own position = tier start + k: {n_identity:?}); \
               parent-1 vinfo targets [common, lod01, lod0]: primaries LOD01 {:?} LOD0 {:?}, extras (unk_indices_2) LOD01 {:?} LOD0 {:?}; \
               primary parent-1 position tiers LOD01 {:?} LOD0 {:?}; primary parent-2 position tiers LOD01 {:?} LOD0 {:?}; \
               extra parent-2 position tiers LOD01 {:?} LOD0 {:?}",
              primary_targets[0], primary_targets[1], extra_targets[0], extra_targets[1],
              p1_tier[0], p1_tier[1], p2_tier[0], p2_tier[1], extra_p2_tier[0], extra_p2_tier[1]);
    // (e): morph parents come from coarser tiers.
    assert_eq!((p1_tier[0][1], p1_tier[0][2], p2_tier[0][1], p2_tier[0][2]), (0, 0, 0, 0), "LOD-01 primary parents outside the common tier");
    assert_eq!((p1_tier[1][2], p2_tier[1][2]), (0, 0), "LOD-0 primary parents in the LOD-0 tier");
    // Retail totals (2026-09-26): primaries own their tier's positions in order except for 6 LOD-01 entries.
    assert_eq!((n_tfrags, n_primaries, n_identity), (20_016, [36_167, 508_610], [36_161, 508_610]), "primary counts");
}

/// Instance records against their classes on every level: tie `draw_distance` (read as the raw s32 at 0x04, whatever
/// type `TieInstance` gives it) is 0 or 1..=720, and every shrub / tie / moby instance's class number resolves to a parsed
/// class. Moby instances whose class has no geometry blob (core-index entry with offset 0) or no core-index entry
/// at all are counted per level and pinned.
#[test]
fn instance_classes_and_tie_draw_distances() {
    use rc_formats::{gameplay, moby, shrub, tie};
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let (mut n_tie, mut n_tie_zero, mut dd_min, mut dd_max) = (0usize, 0usize, i32::MAX, i32::MIN);
    let mut dd_hist = std::collections::BTreeMap::<i32, usize>::new();
    let (mut n_shrub, mut n_moby) = (0usize, 0usize);
    let mut unresolved = Vec::new(); // (level, moby instances with an index entry but no geometry, with no index entry, distinct classes)
    for i in 0..19 {
        let lv = load_rust_level(&root, i);
        let ties = tie::parse_level_ties(&lv.core, &lv.data).unwrap();
        for inst in tie::parse_tie_instances(&lv.gameplay).unwrap() {
            let raw = bytemuck::bytes_of(&inst);
            let dd = i32::from_le_bytes(raw[4..8].try_into().unwrap());
            assert!(dd == 0 || (1..=720).contains(&dd), "level {i}: tie instance (class {}) draw distance {dd}", inst.o_class);
            assert!(ties.iter().any(|c| c.o_class == inst.o_class), "level {i}: tie instance class {} unresolved", inst.o_class);
            if dd == 0 { n_tie_zero += 1; } else { dd_min = dd_min.min(dd); dd_max = dd_max.max(dd); }
            *dd_hist.entry(dd).or_default() += 1;
            n_tie += 1;
        }
        let shrubs = shrub::parse_level_shrubs(&lv.core, &lv.data).unwrap();
        for inst in shrub::parse_shrub_instances(&lv.gameplay).unwrap() {
            assert!(shrubs.iter().any(|c| c.o_class == inst.o_class), "level {i}: shrub instance class {} unresolved", inst.o_class);
            n_shrub += 1;
        }
        let mobys = moby::parse_level_mobys(&lv.core, &lv.data).unwrap();
        let (mut no_geom, mut no_entry, mut classes) = (0usize, 0usize, std::collections::BTreeSet::new());
        for inst in gameplay::parse_moby_instances(&lv.gameplay).unwrap() {
            n_moby += 1;
            if mobys.iter().any(|c| c.o_class == inst.o_class) { continue; }
            if lv.core.moby_classes.iter().any(|e| e.o_class == inst.o_class) { no_geom += 1; } else { no_entry += 1; }
            classes.insert(inst.o_class);
        }
        unresolved.push((i, no_geom, no_entry, classes));
    }
    let common: Vec<(i32, usize)> = { let mut v: Vec<_> = dd_hist.iter().map(|(&d, &n)| (d, n)).collect(); v.sort_by_key(|&(_, n)| std::cmp::Reverse(n)); v.truncate(8); v };
    eprintln!("tie draw distance: {n_tie} instances, {n_tie_zero} zero, non-zero range {dd_min}..={dd_max}, {} distinct values, most common {common:?}", dd_hist.len());
    eprintln!("instances: {n_tie} ties and {n_shrub} shrubs all resolve; {n_moby} mobys, unresolved per level (no geometry blob / no core-index entry / distinct classes):");
    for (i, g, e, c) in &unresolved {
        if g + e > 0 { eprintln!("  level {i:2}: {g} / {e} / {} {:?}", c.len(), c); }
    }
    // Retail: no instance names a class missing from the core index; per level, instances of classes without geometry.
    assert!(unresolved.iter().all(|u| u.2 == 0), "moby instance class without a core-index entry");
    let no_geom: Vec<usize> = unresolved.iter().map(|u| u.1).collect();
    assert_eq!(no_geom, [10, 65, 35, 35, 32, 34, 61, 41, 52, 37, 71, 32, 36, 48, 120, 46, 35, 45, 38], "moby instances without class geometry per level");
    assert_eq!((n_tie, n_tie_zero, dd_min, dd_max), (44_712, 0, 84, 720), "tie draw distances");
}

#[test]
fn collision_for_every_level() {
    use bytemuck::{bytes_of, cast_slice};
    use rc_formats::collision;
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let snap = std::cell::RefCell::new(Snap::new("collision"));
    let (mut n_cells, mut n_verts, mut n_faces, mut n_quads, mut n_tris, mut n_hero, mut n_hero_tris) = (0, 0, 0, 0, 0, 0, 0);
    let mut surfaces = std::collections::BTreeMap::<u8, usize>::new();
    for i in 0..19 {
        let dir = root.join(format!("levels/{i:02}"));
        // Rust path end to end: WAD decompression, core index, block split, collision parse.
        let idx = std::fs::read(dir.join("core_index.bin")).unwrap();
        let data = wad::decompress(&std::fs::read(dir.join("core_data.bin")).unwrap()).unwrap();
        let core = level::parse_level_core(&idx, data.len()).unwrap();
        let block = collision::collision_block(&core, &data).unwrap();
        let c = collision::parse_collision(&core, &data).unwrap();
        let tris = collision::collision_triangles(&c);
        let mesh = &block[c.header.mesh as usize..];

        // One row per section of the retired C++ `collision_dump.bin` layout; its plain words (counts, offsets)
        // go to "collision.words" in order.
        let word = |v: u32| snap.borrow_mut().add(&lv(i), "collision.words", &v.to_le_bytes());
        let check = |name: &str, ours: &[u8]| snap.borrow_mut().add(&lv(i), &format!("collision.{name}"), ours);
        let node_bytes = |h: collision::CollisionNodeHeader, entries: &[u8]| [bytes_of(&h), entries].concat();
        check("header", bytes_of(&c.header));
        check("root", &node_bytes(c.root.header, cast_slice(&c.root.slabs)));
        word(c.slabs.len() as u32);
        for s in &c.slabs {
            word(s.offset as usize as u32);
            check("slab", &node_bytes(s.header, cast_slice(&s.rows)));
        }
        word(c.rows.len() as u32);
        for r in &c.rows {
            word(r.offset as usize as u32);
            check("row", &node_bytes(r.header, cast_slice(&r.cells)));
        }
        word(c.cells.len() as u32);
        for cell in &c.cells {
            let rec = [cast_slice::<i16, u8>(&[cell.x, cell.y, cell.z, 0]), &cell.leaf_word.to_le_bytes(), bytes_of(&cell.header)].concat();
            check("cell record", &rec);
            check("packed vertices", cast_slice(&cell.packed));
            check("vertices", cast_slice(&cell.vertices));
            check("faces", cast_slice(&cell.faces));
            check("quad_v3", &cell.quad_v3);
            // The raw tree walk finds this cell's leaf word.
            let w = collision::lookup_cell_word(mesh, cell.x as i32, cell.y as i32, cell.z as i32).unwrap();
            assert_eq!(w, Some(cell.leaf_word), "level {i}: tree walk for {:?}", cell.coords());
            n_verts += cell.vertices.len();
            n_faces += cell.faces.len();
            n_quads += cell.header.quad_count as usize;
        }
        word(c.hero_group_count as u32);
        for g in &c.hero_groups {
            check("hero group", bytes_of(&g.header));
            check("hero vertices", cast_slice(&g.vertices));
            check("hero triangles", cast_slice(&g.triangles));
            check("hero sphere", cast_slice(&g.header.sphere_world()));
            let world: Vec<[f32; 3]> = g.vertices.iter().map(|v| v.position()).collect();
            check("hero world vertices", cast_slice(&world));
            n_hero_tris += g.triangles.len();
        }
        check("triangles", cast_slice(&tris));
        n_cells += c.cells.len();
        n_tris += tris.len();
        n_hero += c.hero_groups.len();
        for (s, n) in c.surface_counts() { *surfaces.entry(s).or_insert(0) += n; }
    }
    eprintln!("collision: 19 levels, {n_cells} cells, {n_verts} vertices, {n_faces} faces ({n_quads} quads), {n_tris} triangles, \
               {n_hero} hero groups ({n_hero_tris} triangles)");
    eprintln!("collision surface ids ({}): {surfaces:?}", surfaces.len());
    snap.into_inner().finish();
}

#[test]
fn occlusion_for_every_level() {
    use bytemuck::cast_slice;
    use rc_formats::{gameplay, occlusion, tfrag, tie};
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let snap = std::cell::RefCell::new(Snap::new("occlusion"));
    let fnv = |it: &mut dyn Iterator<Item = u32>| {
        let (mut n, mut h) = (0u32, 2166136261u32);
        for i in it { n += 1; for b in i.to_le_bytes() { h ^= b as u32; h = h.wrapping_mul(16777619); } }
        (n, h)
    };
    let (mut n_cells, mut n_masks, mut totals, mut never) = (0usize, 0usize, [0usize; 3], [0usize; 3]);
    for i in 0..19 {
        let dir = root.join(format!("levels/{i:02}"));
        // Rust path end to end: WAD decompression, core index, block split, grid + mappings, load-time resolution.
        let idx = std::fs::read(dir.join("core_index.bin")).unwrap();
        let data = wad::decompress(&std::fs::read(dir.join("core_data.bin")).unwrap()).unwrap();
        let core = level::parse_level_core(&idx, data.len()).unwrap();
        let o = occlusion::parse_occlusion(&core, &data).unwrap();
        let gp = wad::decompress(&std::fs::read(dir.join("gameplay_ntsc.bin")).unwrap()).unwrap();
        let maps = occlusion::parse_gameplay_occlusion_mappings(&gp).unwrap().expect("retail levels all have mappings");
        let tb = core.blocks.iter().find(|b| b.name == "tfrags").unwrap();
        let tblk = &data[tb.offset..tb.offset + tb.size];
        let bh: tfrag::TfragBlockHeader = bytemuck::pod_read_unaligned(&tblk[..16]);
        let theads: Vec<tfrag::TfragHeader> = cast_slice::<u8, tfrag::TfragHeader>(&tblk[bh.table_offset as usize..][..bh.tfrag_count as usize * 0x40]).to_vec();
        let ties: Vec<i32> = tie::parse_tie_instances(&gp).unwrap().iter().map(|t| t.occlusion_index).collect();
        let mobys = gameplay::parse_moby_instances(&gp).unwrap();
        let lo = occlusion::resolve_level_occlusion(Some(&maps), &theads, &ties, &mobys);
        // The level WAD's `occlusion` lump is a sector-padded copy of the gameplay mappings.
        let ofs = u32::from_le_bytes(gp[0x8c..0x90].try_into().unwrap()) as usize;
        let map_len = 0x10 + 8 * (maps.tfrag.len() + maps.tie.len() + maps.moby.len());
        assert!(std::fs::read(dir.join("occlusion.bin")).unwrap()[..map_len] == gp[ofs..ofs + map_len], "level {i}: WAD occlusion lump differs");

        // One row per section of the retired C++ `occlusion_dump.bin` layout; its plain words (offsets, counts,
        // and per mask and kind the visible-list size and FNV-1a hash) go to "occlusion.words" in order.
        let word = |v: u32| snap.borrow_mut().add(&lv(i), "occlusion.words", &v.to_le_bytes());
        let check = |name: &str, ours: &[u8]| snap.borrow_mut().add(&lv(i), &format!("occlusion.{name}"), ours);
        check("raw block", &o.raw);
        word(o.masks_offset as u32);
        word(o.z_base as u32 | (o.z_count as u32) << 16);
        check("cells", cast_slice(&o.cells));
        word(o.masks.len() as u32);
        let oct = o.octants.as_ref().map(|v| [cast_slice::<f32, u8>(&v.centre), cast_slice(&v.masks)].concat()).unwrap_or_default();
        check("octant override", &oct);
        for n in [maps.tfrag.len(), maps.tie.len(), maps.moby.len()] { word(n as u32); }
        check("mappings", &[cast_slice::<_, u8>(&maps.tfrag), cast_slice(&maps.tie), cast_slice(&maps.moby)].concat());
        check("tfrag bits", cast_slice(&lo.tfrag));
        check("tie bits", cast_slice(&lo.tie));
        check("moby bits", cast_slice(&lo.moby));
        for v in [lo.tfrag_out_of_date as u32, lo.ties_not_found as u32, lo.mobys_not_found as u32] { word(v); }
        assert!(!lo.tfrag_out_of_date && lo.ties_positional && lo.mobys_not_found == 0, "level {i}: retail data takes the in-date paths");
        let mut per_mask = vec![[0u32; 3]; o.masks.len()];
        let mut ever = [vec![false; lo.tfrag.len()], vec![false; lo.tie.len()], vec![false; lo.moby.len()]];
        for (mi, m) in o.masks.iter().enumerate() {
            let frame = occlusion::with_always_bit(*m);
            let lists: [Vec<u32>; 3] = [lo.visible_tfrags(&frame).map(u32::from).collect(), lo.visible_ties(&frame).collect(), lo.visible_mobys(&frame).collect()];
            for (k, l) in lists.iter().enumerate() {
                let (n, h) = fnv(&mut l.iter().copied());
                word(n);
                word(h);
                per_mask[mi][k] = n;
                for &x in l { ever[k][x as usize] = true; }
            }
        }
        // The exact tree walk and the camera → cell mapping find every cell.
        for c in &o.cells {
            assert_eq!(o.lookup(c.x as i32, c.y as i32, c.z as i32), Some(c.mask));
            let centre = [c.x, c.y, c.z].map(|v| v as f32 * occlusion::CELL_SIZE + 2.0);
            assert_eq!(o.cell_for(centre), Some(*c), "level {i}: cell_for at a cell centre");
            for k in 0..3 { totals[k] += per_mask[c.mask as usize][k] as usize; }
        }
        let bits = [&lo.tfrag, &lo.tie, &lo.moby];
        for k in 0..3 { never[k] += (0..bits[k].len()).filter(|&x| bits[k][x] != occlusion::OcclBits::ALWAYS && !ever[k][x]).count(); }
        assert!(theads.iter().all(|h| h.occl_index == 0), "level {i}: tfrag header 0x3a is zero on disc");
        n_cells += o.cells.len();
        n_masks += o.masks.len();
    }
    eprintln!("occlusion: 19 levels, {n_cells} cells, {n_masks} masks; visible entries over all cells tfrag {} tie {} moby {}; \
               mapped objects never visible from any cell: tfrag {} tie {} moby {}",
              totals[0], totals[1], totals[2], never[0], never[1], never[2]);
    snap.into_inner().finish();
}

/// Gadget classes (docs/formats/moby_rac1.md 0.4): the table entry, the moby class table entry, the decompressed
/// size, every moby section and the RGBA of every used texture slot, one snapshot row each (the retired C++
/// `gadget_dump.bin` layout). Also: every class fits the game's 0x18000 buffer, its textures are the moby
/// table's (same pixels as `parse_textures`), and Ratchet's hand attachment list resolves to the same joint.
#[test]
fn gadgets_for_every_level() {
    use bytemuck::{bytes_of, cast_slice};
    use rc_formats::{gadget, moby};
    use std::collections::{BTreeSet, HashSet};
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let mut snap = Snap::new("gadget");
    let (mut n_classes, mut n_verts, mut n_tris, mut n_tex) = (0usize, 0usize, 0usize, 0usize);
    let mut o_classes = BTreeSet::new();
    let mut blobs = HashSet::new();
    let mut hand_joints = BTreeSet::new();
    for i in 0..19 {
        let lv = load_rust_level(&root, i);
        let gadgets = gadget::parse_gadget_classes(&lv.core, &lv.data).unwrap();
        let level_tex = texture::parse_textures(&lv.core, &lv.data, &lv.gs_ram).unwrap();

        for g in &gadgets {
            let (o, mc) = (g.moby.o_class, &g.moby.class);
            snap.add(&snapshot::lv(i), "gadget.o_class", &o.to_le_bytes());
            assert!(g.blob.len() <= gadget::GADGET_BUFFER_SIZE, "level {i} gadget {o}: 0x{:x} bytes", g.blob.len());
            assert_eq!(g.moby.entry.offset_in_asset_wad, 0, "level {i} gadget {o}: moby table entry has geometry");
            let h = &mc.header;
            let mut tex = Vec::new();
            for t in gadget::class_textures(&lv.core, &lv.data, &lv.gs_ram, &g.moby).unwrap() {
                let img = t.texture.as_ref().expect("gadget texture without pixels");
                for v in [t.slot as u32, t.index as u32, img.width, img.height] { tex.extend(v.to_le_bytes()); }
                tex.extend_from_slice(&img.rgba);
                let lt = level_tex.iter().find(|l| l.table == texture::TextureTable::Moby && l.index == t.index).expect("moby texture");
                assert!(lt.texture == *img, "level {i} gadget {o}: slot {} differs from moby texture {}", t.slot, t.index);
                n_tex += 1;
            }
            let class_secs: [(&str, Vec<u8>); 8] = [
                ("entry", bytes_of(&g.entry).to_vec()),
                ("class_entry", bytes_of(&g.moby.entry).to_vec()),
                ("decompressed_size", (g.blob.len() as u32).to_le_bytes().to_vec()),
                ("header", bytes_of(h).to_vec()),
                ("sequence_pointers", cast_slice(&mc.sequence_pointers).to_vec()),
                ("skeleton", cast_slice(&mc.skeleton.matrices).to_vec()),
                ("common_trans", cast_slice(&mc.skeleton.trans).to_vec()),
                ("packet_counts", cast_slice(&[mc.high_lod.len() as u32, mc.low_lod.len() as u32, mc.metal.len() as u32]).to_vec()),
            ];
            for (name, ours) in &class_secs {
                snap.add(&snapshot::lv(i), &format!("gadget.{name}"), ours);
            }
            for (li, list) in [&mc.high_lod, &mc.low_lod, &mc.metal].into_iter().enumerate() {
                for p in list {
                    for (name, ours) in MOBY_PACKET_SECTIONS.iter().zip(moby_packet_sections(p)) {
                        snap.add(&snapshot::lv(i), &format!("gadget.packet.{name}"), &ours);
                    }
                    assert_eq!(moby::moby_triangles(p).unwrap(), p.triangles);
                    assert_eq!(p.unresolved_duplicates, 0, "level {i} gadget {o}: unresolved duplicate");
                    let joint_limit = if li == 1 { h.low_lod_joint_count } else { h.joint_count }.max(1);
                    for v in &p.vertices {
                        let s = v.skin;
                        assert!((1..=3).contains(&s.count) && s.weights.iter().sum::<u16>() == 256, "level {i} gadget {o}: skin {s:?}");
                        assert!(s.joints[..s.count as usize].iter().all(|&j| j < joint_limit), "level {i} gadget {o}: joint {s:?}");
                    }
                    for t in &p.triangles {
                        if p.is_metal { assert!(t.texture == -2 || t.texture == -3, "level {i} gadget {o}: metal texture {}", t.texture); }
                        else { assert!(t.texture == -1 || g.moby.texture_table_index(t.texture).is_some_and(|x| x < lv.core.moby_textures.len()), "level {i} gadget {o}: texture {}", t.texture); }
                    }
                    if li == 0 { n_tris += p.triangles.len(); }
                    n_verts += p.vertices.len();
                }
            }
            snap.add(&snapshot::lv(i), "gadget.textures", &tex);
            o_classes.insert(o);
            blobs.insert(g.blob.clone());
        }
        n_classes += gadgets.len();
        assert!(gadgets.iter().any(|g| g.moby.o_class == gadget::WRENCH_O_CLASS), "level {i}: no wrench");
        // Ratchet (class 0) is an ordinary moby class; the hand item hangs off the end of his joint list 0.
        let blk = lv.core.blocks.iter().find(|b| b.name == "moby_class/0000").expect("Ratchet's class");
        let blob = &lv.data[blk.offset..blk.offset + blk.size];
        let ratchet = moby::parse_moby_class(blob).unwrap();
        hand_joints.insert(gadget::attachment_joint(blob, &ratchet.header, gadget::HAND_JOINT_LIST).unwrap());
    }
    assert_eq!(hand_joints.len(), 1, "Ratchet's hand joint differs between levels: {hand_joints:?}");
    eprintln!("gadgets: 19 levels, {n_classes} classes ({} distinct o_class {:?}, {} distinct decompressed blobs), {n_verts} vertices, \
               {n_tris} high-LOD triangles, {n_tex} textures, Ratchet hand joint {hand_joints:?}",
              o_classes.len(), o_classes, blobs.len());
    snap.finish();
}

/// Particle and FX textures: part_textures, part_defs (header, offsets, blob, resolved starts), the decoded particle
/// textures and the FX textures, for every level (docs/plan/particles.md §6). Snapshot rows follow the retired C++
/// `particles_dump.bin` layout: its sections in order under "particles.sections", its plain words under
/// "particles.words".
#[test]
fn particle_textures_for_every_level() {
    use bytemuck::cast_slice;
    use rc_formats::particle_tex;
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let snap = std::cell::RefCell::new(Snap::new("particles"));
    let (mut n_part, mut n_fx, mut n_px) = (0usize, 0usize, 0usize);
    for i in 0..19 {
        let dir = root.join(format!("levels/{i:02}"));
        let idx = std::fs::read(dir.join("core_index.bin")).unwrap();
        let data = wad::decompress(&std::fs::read(dir.join("core_data.bin")).unwrap()).unwrap();
        let core = level::parse_level_core(&idx, data.len()).unwrap();
        let p = particle_tex::parse_particle_textures(&core, &idx, &data).unwrap();

        let section = |b: &[u8]| snap.borrow_mut().add(&lv(i), "particles.sections", b);
        let word = |v: u32| snap.borrow_mut().add(&lv(i), "particles.words", &v.to_le_bytes());
        let starts: Vec<i32> = (0..p.defs.offsets.len()).map(|t| p.defs.start(t).map_or(-1, |s| s as i32)).collect();
        for b in [cast_slice(&p.entries), cast_slice(&p.defs.header), cast_slice(&p.defs.offsets), &p.defs.blob, cast_slice(&starts)] { section(b); }
        word(p.textures.len() as u32);
        for (k, t) in p.textures.iter().enumerate() {
            word(t.width);
            word(t.height);
            section(&t.rgba);
            assert_eq!(t.rgba.len(), t.width as usize * t.height as usize * 4, "level {i} particle texture {k}: RGBA size");
            // Alpha is the level-texture scaling of PS2 0..0x80: only even values below 0xff.
            assert!(t.rgba.chunks(4).all(|px| px[3] == 0xff || px[3] % 2 == 0), "level {i} particle texture {k}: alpha scaling");
            n_px += t.rgba.len() / 4;
        }
        section(cast_slice::<_, u8>(&p.fx_entries));
        word(p.fx_textures.len() as u32);
        for t in &p.fx_textures {
            word(t.is_some() as u32);
            word(t.as_ref().map_or(0, |t| t.width));
            word(t.as_ref().map_or(0, |t| t.height));
            section(t.as_ref().map_or(&[][..], |t| t.rgba.as_slice()));
            n_fx += t.is_some() as usize;
        }
        // Every level: 81 types, all 32×32, def lists index existing textures.
        assert_eq!(p.defs.header[0] as usize, particle_tex::PART_TYPES, "level {i}: part_defs count");
        assert_eq!(p.defs.header[1] as usize, p.entries.len(), "level {i}: part_defs texture count");
        assert!(p.entries.iter().all(|e| e.side == 32 && e.unk4 == 0), "level {i}: particle texture shape");
        assert!(p.defs.blob.iter().all(|&b| (b as usize) < p.entries.len()), "level {i}: def index out of range");
        if i == 1 {
            // Novalis: 41 textures; the blob starts with type 1's 9-frame run 0..=0xa (particles.md §6); the class-27
            // emitters' def[4][0] is part texture 0.
            assert_eq!(p.textures.len(), 41);
            assert_eq!(&p.defs.blob[..11], &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
            assert_eq!(p.defs.first_frame(4), Some(0));
        }
        n_part += p.textures.len();
    }
    eprintln!("particle textures: 19 levels, {n_part} particle textures ({n_px} texels), {n_fx} fx textures");
    snap.into_inner().finish();
}

/// Sound banks: bank header, sounds, grains, sample extents, both decodes of every sample, the remapped level defs,
/// the map, per-class ids/defs and the music table, for every level and the global bank (docs/plan/audio.md §2,
/// §6.3), one snapshot row per section (the retired C++ `sound_dump.bin` layout). Then the structural and signal
/// checks of §6.3.
#[test]
fn sound_banks_for_every_level() {
    use bytemuck::cast_slice;
    use rc_formats::{sound_bank, vag};
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let mut snap = Snap::new("sound");
    const SECTIONS: [&str; 10] = ["header", "sounds", "grains", "vags", "pcm", "pcm_opengoal", "level_defs", "map", "classes", "music"];

    /// The Rust side serialised in the retired C++ `sound_dump.bin` layout, one byte vector per section.
    fn serialise(b: &sound_bank::Bank, defs: Option<(&sound_bank::LevelSounds, [i32; 15])>) -> Vec<Vec<u8>> {
        let w = |v: &mut Vec<u8>, x: u32| v.extend(x.to_le_bytes());
        let h = &b.header;
        let mut hdr = Vec::new();
        for x in [b.file_type, 2, b.chunks[0].0, b.chunks[0].1, b.chunks[1].0, b.chunks[1].1, h.version, h.flags, h.bank_id, h.bank_num as i32 as u32,
                  h.n_sounds as i32 as u32, h.n_grains as i32 as u32, h.n_vags as i32 as u32, h.first_sound, h.first_grain, h.vags_in_sr,
                  h.vag_data_size, h.sram_alloc_size, h.next_block] { w(&mut hdr, x); }
        let mut sounds = Vec::new();
        for s in &b.sounds {
            for x in [s.vol as i32, s.vol_group as i32, s.pan as i32, s.grains.len() as i32, s.instance_limit as i32, s.flags as i32, s.first_grain as i32] { w(&mut sounds, x as u32); }
        }
        let mut grains = Vec::new();
        for g in &b.grains { w(&mut grains, g.kind); w(&mut grains, g.delay as u32); grains.extend(g.data); }
        let mut vags = Vec::new();
        for v in &b.vags {
            for x in [v.offset as u32, v.frames as u32, v.loop_start.map_or(u32::MAX, |f| f as u32), v.looped as u32, v.next_flags.map_or(u32::MAX, |f| f as u32)] { w(&mut vags, x); }
        }
        let body = |v: &vag::SampleExtent| &b.samples[v.offset..v.offset + v.bytes()];
        let pcm: Vec<i16> = b.vags.iter().flat_map(|v| vag::decode(body(v))).collect();
        let pcm_og: Vec<i16> = b.vags.iter().flat_map(|v| vag::decode_opengoal_variant(body(v))).collect();
        let mut out = vec![hdr, sounds, grains, vags, cast_slice(&pcm).to_vec(), cast_slice(&pcm_og).to_vec()];
        match defs {
            Some((d, music)) => {
                out.push(cast_slice(&d.level_defs).to_vec());
                out.push(cast_slice(&d.map).to_vec());
                let mut classes = Vec::new();
                for c in &d.classes {
                    w(&mut classes, c.o_class as u32);
                    w(&mut classes, c.bank_ids.len() as u32);
                    classes.extend(cast_slice::<u16, u8>(&c.bank_ids));
                    w(&mut classes, c.header_count.map_or(u32::MAX, |n| n as u32));
                    w(&mut classes, c.defs.len() as u32);
                    classes.extend(cast_slice::<_, u8>(&c.defs));
                }
                out.push(classes);
                out.push(cast_slice(&music).to_vec());
            }
            None => out.extend([Vec::new(), Vec::new(), Vec::new(), Vec::new()]),
        }
        out
    }
    let (mut n_sounds, mut n_grains, mut n_vags, mut n_samples, mut n_defs, mut n_class_ids) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let (mut rms_lo, mut rms_hi, mut clipped, mut differing) = (f64::MAX, 0f64, 0usize, 0usize);
    let mut proved_failure = false;
    let mut n_loop_late = 0usize;
    let mut count_mismatch = Vec::new();
    for i in 0..=19 {
        let dir = if i == 19 { root.join("global") } else { root.join(format!("levels/{i:02}")) };
        let bank = sound_bank::parse_bank(&std::fs::read(dir.join("sound_bank.bin")).unwrap()).unwrap();
        let level = (i < 19).then(|| {
            let idx = std::fs::read(dir.join("core_index.bin")).unwrap();
            let data = wad::decompress(&std::fs::read(dir.join("core_data.bin")).unwrap()).unwrap();
            let core = level::parse_level_core(&idx, data.len()).unwrap();
            let defs = sound_bank::parse_level_sounds(&idx, &core, &data).unwrap();
            let music = sound_bank::music_table(&std::fs::read(dir.join("level_header.bin")).unwrap()).unwrap();
            (defs, music)
        });
        let ours = serialise(&bank, level.as_ref().map(|(d, m)| (d, *m)));
        let scope = if i == 19 { "global".to_string() } else { lv(i) };
        for (name, sec) in SECTIONS.iter().zip(&ours) { snap.add(&scope, &format!("sound.{name}"), sec); }
        if !proved_failure {
            // The snapshot sees one flipped PCM bit and one changed def byte.
            let mut bad = ours.clone();
            bad[4][100] ^= 1;
            assert_ne!(snapshot::digest(&bad[4..5]), snapshot::digest(&ours[4..5]), "a changed PCM sample went unnoticed");
            let mut bad = ours.clone();
            bad[6][0x1a] ^= 1;
            assert_ne!(snapshot::digest(&bad[6..7]), snapshot::digest(&ours[6..7]), "a changed def went unnoticed");
            proved_failure = true;
        }

        // Structure (§6.3).
        let h = &bank.header;
        assert_eq!(bank.grains.len(), h.n_grains as usize, "{i}: grain count");
        assert_eq!(bank.vags.len(), h.n_vags as usize, "{i}: distinct tone samples = header vag count");
        assert_eq!(h.first_grain as usize + bank.grains.len() * 0x28, bank.chunks[0].1 as usize, "{i}: grains end the block");
        assert_eq!(h.vag_data_size as usize, bank.samples.len(), "{i}: vag data size");
        for f in bank.samples.as_chunks::<16>().0 {
            assert!(f[0] & 15 <= 12 && f[0] >> 4 <= 4, "{i}: shift/filter out of range");
        }
        let mut followed = 0;
        for (k, v) in bank.vags.iter().enumerate() {
            let s = &bank.samples[v.offset..v.offset + v.bytes()];
            assert!(s[0] == 0 && s[2..16].iter().all(|&b| b == 0), "{i}: sample {k} does not start with a zero frame");
            let flags: Vec<u8> = s.as_chunks::<16>().0.iter().map(|f| f[1]).collect();
            if v.looped {
                assert_eq!(*flags.last().unwrap(), 3, "{i}: loop {k} ends with 3");
                assert_eq!(flags.iter().filter(|&&f| f == 6).count(), 1, "{i}: loop {k} has one 6");
                // The disc has two shapes: `0, 6, 2…, 3` and `0, 2…, 6, 2…, 3` (repeat-flagged frames before the start).
                let ls = v.loop_start.expect("loop start");
                assert!(flags[0] == 0 && flags[1..ls].iter().all(|&f| f == 2) && flags[ls] == 6 && flags[ls + 1..flags.len() - 1].iter().all(|&f| f == 2),
                        "{i}: loop {k} flags");
                n_loop_late += (ls > 0) as usize;
                let (a, b) = v.loop_points().unwrap();
                assert!(a % 28 == 0 && b % 28 == 0 && a < b);
            } else {
                assert_eq!(*flags.last().unwrap(), 1, "{i}: one-shot {k} ends with 1");
                assert!(flags[..flags.len() - 1].iter().all(|&f| f == 0), "{i}: one-shot {k} middle flags");
                if let Some(n) = v.next_flags { assert_eq!(n, 7, "{i}: one-shot {k} is padded with a 7 frame"); }
                if bank.vags.get(k + 1).is_some_and(|w| w.offset == v.offset + v.bytes() + 16) { followed += 1; }
            }
        }
        if i == 1 {
            let one_shots = bank.vags.iter().filter(|v| !v.looped).count();
            assert_eq!((one_shots, bank.vags.len() - one_shots, followed), (230, 31, 229), "Novalis sample kinds");
        }
        for g in bank.grains.iter().filter(|g| g.is_tone()) {
            assert!((g.tone().sample_offset as usize) < bank.samples.len());
        }
        if let Some((d, _)) = &level {
            for c in &d.classes {
                // Header count vs remap count: the loader warns (0x209800) when they differ; Novalis has none.
                if c.header_count.is_some_and(|n| n as usize != c.bank_ids.len()) {
                    assert_ne!(i, 1, "Novalis class {} remap count", c.o_class);
                    count_mismatch.push((i, c.o_class, c.header_count.unwrap(), c.bank_ids.len()));
                }
                assert!(c.bank_ids.iter().all(|&id| id == 0xffff || (id as i16) < h.n_sounds), "{i}: class id out of range");
                n_class_ids += c.bank_ids.len();
            }
            assert!(d.level_defs.iter().all(|x| x.index == 0xffff || (x.index as i16) < h.n_sounds), "{i}: level def id out of range");
            n_defs += d.level_defs.len();
        }

        // Signal.
        for v in &bank.vags {
            let pcm = vag::decode_extent(&bank.samples, v);
            let og = vag::decode_opengoal_variant(&bank.samples[v.offset..v.offset + v.bytes()]);
            differing += pcm.iter().zip(&og).filter(|(a, b)| a != b).count();
            assert!(pcm[..28].iter().all(|&x| x == 0));
            let rms = (pcm.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / pcm.len() as f64).sqrt();
            rms_lo = rms_lo.min(rms);
            rms_hi = rms_hi.max(rms);
            let zc = pcm.windows(2).filter(|w| (w[0] < 0) != (w[1] < 0)).count() as f64 / pcm.len() as f64;
            assert!(zc <= 0.9, "{i}: zero crossings {zc}");
            clipped += pcm.iter().filter(|&&x| x == i16::MAX || x == i16::MIN).count();
            n_samples += pcm.len();
        }
        n_sounds += bank.sounds.len();
        n_grains += bank.grains.len();
        n_vags += bank.vags.len();
    }
    assert!((150.0..20_000.0).contains(&rms_lo) && rms_hi < 20_000.0, "sample RMS range {rms_lo:.0}..{rms_hi:.0}");
    assert!((clipped as f64) < 1e-4 * n_samples as f64, "clipped {clipped} of {n_samples}");

    // Novalis music: Start then Loop, mono 44.1 kHz; decoding through the end frame.
    let music = root.join("levels/01/music");
    if let (Ok(start), Ok(lp)) = (std::fs::read(music.join("000.bin")), std::fs::read(music.join("001.bin"))) {
        let (hs, bs) = vag::parse_vag(&start).unwrap();
        let (hl, bl) = vag::parse_vag(&lp).unwrap();
        assert_eq!((hs.name.as_str(), hl.name.as_str(), hs.sample_rate, hl.sample_rate), ("L01_Enemy_Start", "L01_Enemy_Loop", 44100, 44100));
        let el = vag::sample_extent(bl, 0).unwrap();
        let pcm = vag::decode_extent(bl, &el);
        assert_eq!(pcm.len(), 4_911_004);
        let rms = (pcm.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / pcm.len() as f64).sqrt();
        assert!((rms - 7729.0).abs() < 50.0, "L01_Enemy_Loop RMS {rms:.0}");
        // No interleave: no jump at any 0x100..0x8000-byte block boundary beyond the signal's own steps.
        let max_step = pcm.windows(2).map(|w| (w[1] as i32 - w[0] as i32).abs()).max().unwrap();
        for block in [0x100usize, 0x800, 0x2000, 0x8000] {
            let step = block / 16 * 28;
            for b in (step..pcm.len()).step_by(step) { assert!((pcm[b] as i32 - pcm[b - 1] as i32).abs() <= max_step); }
        }
        let es = vag::sample_extent(bs, 0).unwrap();
        eprintln!("music: L01_Enemy_Start {} samples ({:.2} s), L01_Enemy_Loop {} samples ({:.1} s), RMS {rms:.0}",
                  es.samples(), es.samples() as f64 / 44100.0, pcm.len(), pcm.len() as f64 / 44100.0);
    }
    eprintln!("sound banks: 19 levels + global, {n_sounds} sounds, {n_grains} grains, {n_vags} samples ({n_samples} PCM samples, \
               RMS {rms_lo:.0}..{rms_hi:.0}, {clipped} clipped, {differing} differ in the OpenGOAL variant), {n_defs} level defs, \
               {n_class_ids} class ids, {n_loop_late} loops start after frame 0, class count mismatches (level, class, header, remap) {count_mismatch:?}");
    snap.finish();
}

/// HUD tables, every decoded frame, the glyph tables (found through the font wrappers) and the English messages,
/// for every level (docs/plan/hud_text.md), one snapshot row per part of the retired C++ `hud_dump.bin` layout.
/// Also: the 11 levels with the standard HUD are byte-identical to the global lumps; the glyph tables are
/// byte-identical everywhere although their addresses move; a single flipped byte reaches the snapshot.
#[test]
fn hud_fonts_and_strings_for_every_level() {
    use bytemuck::cast_slice;
    use rc_formats::{font, hud, strings};
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let mut snap = Snap::new("hud");

    struct Rust {
        hud: hud::Hud,
        frames: Vec<rc_formats::texture::Texture>,
        glyphs: [font::GlyphTable; 3],
        addrs: [u32; 3],
        msgs: Vec<strings::Message>,
    }
    fn load(dir: &std::path::Path) -> Rust {
        let header = std::fs::read(dir.join("hud_header.bin")).unwrap();
        let h: hud::HudHeader = bytemuck::pod_read_unaligned(&header[..hud::HEADER_SIZE]);
        let banks: Vec<Vec<u8>> = (0..hud::BANKS)
            .map(|b| if h.bank_size[b] == 0 { Vec::new() } else { wad::decompress(&std::fs::read(dir.join(format!("hud_bank_{b}.bin"))).unwrap()).unwrap() })
            .collect();
        let hud = hud::parse_hud(&header, std::array::from_fn(|b| banks[b].as_slice())).unwrap();
        let frames = (0..hud.frames.len()).map(|i| hud.decode_frame(i).unwrap()).collect();
        let (glyphs, addrs) = font::parse_glyph_tables(&std::fs::read(dir.join("overlay.bin")).unwrap()).unwrap();
        let gameplay = wad::decompress(&std::fs::read(dir.join("gameplay_ntsc.bin")).unwrap()).unwrap();
        let msgs = strings::parse_strings(&gameplay, strings::lang::ENGLISH).unwrap();
        Rust { hud, frames, glyphs, addrs, msgs }
    }
    /// The Rust side in the retired C++ `hud_dump.bin` layout, as named parts whose concatenation is the whole dump:
    /// the HUD tables, every decoded frame, the three glyph-table addresses and tables, and the English messages.
    fn parts(r: &Rust) -> Vec<(&'static str, Vec<u8>)> {
        let sec = |b: &[u8]| [&(b.len() as u32).to_le_bytes()[..], b].concat();
        let w = |v: u32| v.to_le_bytes().to_vec();
        let mut out = vec![
            ("magic", b"RCHD".to_vec()),
            ("header", sec(bytemuck::bytes_of(&r.hud.header))),
            ("icons", sec(cast_slice(&r.hud.icons))),
            ("frames", sec(cast_slice(&r.hud.frames))),
            ("palettes", sec(cast_slice(&r.hud.palettes))),
            ("textures", sec(cast_slice(&r.hud.textures))),
            ("frame_count", w(r.frames.len() as u32)),
        ];
        for t in &r.frames { out.push(("frame_images", [w(t.width), w(t.height), sec(&t.rgba)].concat())); }
        out.push(("glyph_addrs", r.addrs.iter().flat_map(|a| a.to_le_bytes()).collect()));
        out.push(("glyph_tables", sec(&r.glyphs.iter().flat_map(|t| cast_slice::<_, u8>(t).to_vec()).collect::<Vec<u8>>())));
        out.push(("message_count", w(r.msgs.len() as u32)));
        for m in &r.msgs { out.push(("messages", [w(m.id as u32), w(m.help_audio as u32), sec(&m.text)].concat())); }
        out
    }
    /// Names of the parts that differ between two serialisations of the same shape.
    fn changed(a: &[(&'static str, Vec<u8>)], b: &[(&'static str, Vec<u8>)]) -> Vec<&'static str> {
        let mut out: Vec<&str> = a.iter().zip(b).filter(|(x, y)| x.1 != y.1).map(|(x, _)| x.0).collect();
        out.dedup();
        out
    }

    let global_header = std::fs::read(root.join("global/hud_header.bin")).ok();
    let (mut n_frames, mut n_texels, mut n_msgs, mut n_global) = (0usize, 0usize, 0usize, 0usize);
    let mut addrs = std::collections::BTreeSet::new();
    let mut first_glyphs: Option<[font::GlyphTable; 3]> = None;
    let mut novalis = None;
    for i in 0..19 {
        let dir = root.join(format!("levels/{i:02}"));
        let r = load(&dir);
        for (name, b) in &parts(&r) { snap.add(&lv(i), &format!("hud.{name}"), b); }
        n_frames += r.frames.len();
        n_texels += r.frames.iter().map(|t| t.rgba.len() / 4).sum::<usize>();
        n_msgs += r.msgs.len();
        addrs.insert(r.addrs);
        // Same glyph bytes in every overlay.
        match &first_glyphs {
            None => first_glyphs = Some(r.glyphs),
            Some(g) => assert!(*g == r.glyphs, "level {i}: glyph tables differ from level 00"),
        }
        // FontPrint's colour table (codes 0x09..0x0f) sits in every overlay's data.
        let ov = std::fs::read(dir.join("overlay.bin")).unwrap();
        let colours: &[u8] = cast_slice(&font::COLOUR_TABLE[1..]);
        assert!(ov.windows(colours.len()).any(|w| w == colours), "level {i}: font colour table not found");
        // The standard HUD set equals the global lumps (header sector-padded there).
        if let Some(g) = &global_header {
            let header = std::fs::read(dir.join("hud_header.bin")).unwrap();
            let same = g.starts_with(&header)
                && (0..hud::BANKS).all(|b| std::fs::read(root.join(format!("global/hud_banks/{b:03}.bin"))).ok().and_then(|z| wad::decompress(&z).ok()).unwrap_or_default() == r.hud.banks[b]);
            let standard = [0, 1, 2, 3, 4, 8, 9, 11, 12, 14, 17].contains(&i);
            assert_eq!(same, standard, "level {i}: global-HUD identity");
            n_global += same as usize;
        }
        if i == 1 { novalis = Some(r); }
    }
    let mut r = novalis.unwrap();
    // Novalis: the standard set (hud_text.md §1.3).
    assert_eq!((r.hud.icons.len() - 1, r.frames.len(), r.hud.palettes.len(), r.hud.textures.len()), (56, 326, 42, 236));
    let spin = r.hud.icons[r.hud.icon_index(30031)];
    assert_eq!((spin.frame_count, r.hud.icon_frame(30006, 31) - r.hud.icon_frame(30006, 0)), (30, 31));
    assert_eq!(r.hud.frame_size(r.hud.icon_frame(30080, 0)), Some((32, 32)));
    assert_eq!(r.addrs, [0x1c35d0, 0x1c3970, 0x1c3d10]);
    assert_eq!(r.msgs.len(), 1521);
    let m = &r.msgs[strings::find_index(&r.msgs, 1000).unwrap()];
    assert!(m.text.starts_with(b"Gadgetron \x0cInfobots\x08") && m.help_audio == 4);
    // The snapshot sees one flipped texel byte and one changed glyph advance.
    let base = parts(&r);
    r.frames[100].rgba[7] ^= 1;
    assert_eq!(changed(&base, &parts(&r)), ["frame_images"]);
    r.frames[100].rgba[7] ^= 1;
    r.glyphs[2][b'W' as usize].advance += 1;
    assert_eq!(changed(&base, &parts(&r)), ["glyph_tables"]);
    eprintln!("hud: 19 levels ({n_global} with the global HUD), {n_frames} frames ({n_texels} texels), {} distinct glyph-table address \
               triples (tables byte-identical), {n_msgs} English messages",
              addrs.len());
    snap.finish();
}

/// Scene tables and chunks (`rc_formats::scene`) for all 19 levels, NTSC and PAL: the Rust path (level header →
/// `SceneTable`, region file → WAD → chunk parse), serialised in the retired C++ `scene_dump.bin` layout, is one
/// snapshot item per level. Also checks the format invariants of docs/plan/cutscenes_transitions.md §2 and the
/// Novalis scene 5 values of §4.4.
#[test]
fn scenes_for_every_level() {
    use bytemuck::{bytes_of, cast_slice};
    use rc_formats::scene::{self, Region, Scene, SceneChunk, SceneTable};
    let Some(root) = extracted() else { eprintln!("skipped: no extracted/"); return; };
    let mut snap = Snap::new("scene");

    fn w32(d: &mut Vec<u8>, v: u32) { d.extend_from_slice(&v.to_le_bytes()); }
    fn chunk_bytes(d: &mut Vec<u8>, c: &SceneChunk, dec_len: usize) {
        w32(d, dec_len as u32);
        d.extend_from_slice(bytes_of(&c.header));
        w32(d, c.camera.len() as u32);
        d.extend_from_slice(cast_slice(&c.camera));
        for a in &c.actors {
            for v in [a.offset, a.class as u32, a.unknown_04 as u32, a.unknown_08 as u32, a.track_offset as u32] { w32(d, v); }
            d.extend_from_slice(bytes_of(&a.sequence.header));
            for (f, &o) in a.sequence.frames.iter().zip(&a.frame_offsets) {
                w32(d, o);
                d.extend_from_slice(bytes_of(&f.header));
                d.extend_from_slice(&f.payload);
            }
            w32(d, a.sequence.triggers.len() as u32);
            for &t in &a.sequence.triggers { w32(d, t); }
            d.extend_from_slice(cast_slice(&a.positions));
        }
        w32(d, c.subtitles.len() as u32);
        for s in &c.subtitles {
            for v in [s.start, s.end].into_iter().chain(s.text_offsets).chain([s.pad]) { d.extend_from_slice(&v.to_le_bytes()); }
            for t in &s.text { w32(d, t.len() as u32); d.extend_from_slice(t); }
        }
    }
    struct LevelScenes { table: SceneTable, speech: Vec<[u32; 6]>, scenes: Vec<Scene>, dec_lens: Vec<Vec<usize>> }
    let load = |dir: &std::path::Path| -> LevelScenes {
        let hb = std::fs::read(dir.join("level_header.bin")).unwrap();
        let h = toc::parse_level_header(&hb).unwrap();
        let table = SceneTable::new(&h).unwrap();
        let (mut speech, mut scenes, mut dec_lens) = (Vec::new(), Vec::new(), Vec::new());
        for (k, e) in table.scenes.iter().enumerate() {
            // Independent of the file size: the VAG header's own size (rc_formats::toc::probe_lump_size).
            speech.push(std::array::from_fn(|l| e.speech[l].map_or(0, |_| {
                let f = std::fs::read(dir.join(format!("speech/{k:02}_{}.bin", toc::SCENE_LANGUAGES[l]))).unwrap();
                toc::probe_lump_size(&f).0 as u32
            })));
            for region in [Region::Ntsc, Region::Pal] {
                let Some(rt) = e.region(region).filter(|rt| !rt.chunks.is_empty()) else { continue };
                let file = std::fs::read(dir.join(format!("scene/{k:02}_{}.bin", region.name()))).unwrap();
                assert_eq!(file.len(), rt.file_bytes(), "{}: scene {k} {} file size", dir.display(), region.name());
                assert!(file[file.len() - 0x800..].iter().all(|&b| b == 0), "sentinel sector not zero");
                scenes.push(Scene::load(&h, &file, k, region).unwrap_or_else(|e| panic!("{}: scene {k} {}: {e}", dir.display(), region.name())));
                dec_lens.push((0..rt.chunks.len()).map(|i| wad::decompress(&file[rt.chunk_in_file(i).unwrap()]).unwrap().len()).collect());
            }
        }
        LevelScenes { table, speech, scenes, dec_lens }
    };
    let serialise = |r: &LevelScenes| -> Vec<u8> {
        let mut d = b"SCN1".to_vec();
        w32(&mut d, r.table.scenes.len() as u32);
        let mut next = r.scenes.iter().zip(&r.dec_lens);
        for (e, sizes) in r.table.scenes.iter().zip(&r.speech) {
            for (s, &b) in e.speech.iter().zip(sizes) { w32(&mut d, s.unwrap_or(0)); w32(&mut d, b); }
            for region in [Region::Ntsc, Region::Pal] {
                let Some(rt) = e.region(region) else { w32(&mut d, 0); w32(&mut d, 0); continue };
                w32(&mut d, rt.chunks.len() as u32 + 1);
                for c in &rt.chunks { w32(&mut d, c.sector); }
                w32(&mut d, rt.sentinel);
                w32(&mut d, rt.chunks.len() as u32);
                if rt.chunks.is_empty() { continue; }
                let (s, lens) = next.next().unwrap();
                for (c, &n) in s.chunks.iter().zip(lens) { chunk_bytes(&mut d, c, n); }
            }
        }
        d
    };
    let first_diff = |a: &[u8], b: &[u8]| a.iter().zip(b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));

    let (mut n_scenes, mut n_regions, mut n_chunks, mut n_ticks, mut n_actors, mut n_subs, mut n_cuts) = (0, 0, 0, 0i64, 0, 0, 0);
    let mut novalis = None;
    let mut all_missing = Vec::new();
    for i in 0..19 {
        let dir = root.join(format!("levels/{i:02}"));
        let r = load(&dir);
        let mine = serialise(&r);
        snap.add(&lv(i), "scene.serialised", &mine);
        // Invariants (§2) and classes against the level core.
        let idx = std::fs::read(dir.join("core_index.bin")).unwrap();
        let data = wad::decompress(&std::fs::read(dir.join("core_data.bin")).unwrap()).unwrap();
        let core = level::parse_level_core(&idx, data.len()).unwrap();
        let classes: std::collections::BTreeSet<i32> = core.moby_classes.iter().map(|c| c.o_class).collect();
        let mut missing = std::collections::BTreeSet::new();
        let mut ids = std::collections::BTreeSet::new();
        for s in &r.scenes {
            ids.insert(s.index);
            let tpc = s.ticks_per_chunk() as i32;
            let first = s.actor_classes();
            for (n, c) in s.chunks.iter().enumerate() {
                assert_eq!(c.actors.iter().map(|a| a.class).collect::<Vec<_>>(), first, "level {i} scene {}: actor classes change in chunk {n}", s.index);
                // The camera table runs up to the first actor record.
                if let Some(m) = c.actors.iter().map(|a| a.offset).min() {
                    assert_eq!(c.header.camera_offset as usize + 0x20 * c.camera.len(), m as usize, "level {i} scene {} chunk {n}: camera table end", s.index);
                }
                // The last record equals the next chunk's first.
                if let Some(next) = s.chunks.get(n + 1) { assert_eq!(c.camera.last(), next.camera.first(), "level {i} scene {} chunk {n}: camera continuity", s.index); }
                // Enough 30 Hz frames for the last shown chunk tick (frame f and f + 1).
                let last_tick = (s.end_tick() - 1 - n as i32 * tpc).min(tpc - 1);
                for a in &c.actors {
                    assert!(a.positions.len() as i32 >= (last_tick >> 1) + 2, "level {i} scene {} chunk {n}: {} frames", s.index, a.positions.len());
                    assert!(a.positions.iter().all(|p| p[3] == 0.0), "position track w != 0");
                }
                assert!(c.subtitles.is_empty() || c.header.subtitle_offset >= scene::SUBTITLE_MIN_OFFSET);
                n_subs += c.subtitles.len();
            }
            for a in first.iter().filter(|c| !classes.contains(c)) { missing.insert(*a); }
            n_regions += 1;
            n_chunks += s.chunks.len();
            n_ticks += s.end_tick() as i64;
            n_actors += first.len();
            n_cuts += s.cut_ticks().len();
        }
        n_scenes += ids.len();
        // Classes not in the level core: the spaceship classes (TOC `spaceships`) only.
        let gadgets: std::collections::BTreeSet<i32> = core.gadgets.iter().map(|g| g.class_number).collect();
        missing.retain(|c| !gadgets.contains(c));
        all_missing.push(missing.clone());
        eprintln!("level {i:02}: {} scenes, {} regions, classes outside the core: {missing:?}", ids.len(), r.scenes.iter().filter(|s| s.region == Region::Ntsc).count() + r.scenes.iter().filter(|s| s.region == Region::Pal).count());
        if i == 1 { novalis = Some((mine, r)); }
    }
    eprintln!("scene actor classes outside the level core and gadget table: {all_missing:?}");
    assert_eq!((n_scenes, n_regions, n_chunks, n_ticks, n_actors, n_subs, n_cuts), (138, 275, 4081, 348_572, 1004, 4284, 1858), "totals");

    // Novalis scene 5, NTSC (§4.4).
    let (dump, mut r) = novalis.unwrap(); // Novalis serialised as loaded
    let s = r.scenes.iter().find(|s| s.index == 5 && s.region == Region::Ntsc).unwrap();
    assert_eq!((s.end_tick(), s.chunks.len(), s.audio_start()), (1508, 16, -6));
    assert_eq!(s.actor_classes(), [0, 10, 530, 1365]);
    assert_eq!(s.cut_ticks(), [107, 181, 261, 365, 761, 921, 1095, 1265]);
    assert!(s.chunks.iter().flat_map(|c| &c.camera).all(|c| c.tan_half_fov.to_bits() == s.chunks[0].camera[0].tan_half_fov.to_bits()));
    assert!((s.camera_at(1).unwrap().tan_half_fov - 0.4141).abs() < 1e-4);
    let eye = s.camera_at(1).unwrap().eye;
    assert!((eye[0] - 162.60).abs() < 0.01 && (eye[1] - 129.80).abs() < 0.01 && (eye[2] - 82.75).abs() < 0.01, "{eye:?}");
    let sub = s.subtitle_at(259).unwrap();
    assert_eq!((sub.start, sub.end, sub.text[0].as_slice()), (259, 336, b"Ooomph!".as_slice()));
    assert!(s.subtitle_at(258).is_none());
    assert_eq!(s.subtitle_at(1400).unwrap().text[0], b"If there are any left.");
    let ship = &s.chunks[0].actors[2];
    let near = |p: [f32; 4], q: [f32; 3]| (0..3).all(|k| (p[k] - q[k]).abs() < 0.01);
    assert!(near(ship.positions[0], [40.587, 129.492, 126.098]) && near(ship.positions[48], [145.245, 129.648, 87.004]), "ship path {:?} .. {:?}", ship.positions[0], ship.positions[48]);
    assert!(all_missing[1].is_empty(), "Novalis scene classes all in the level core (530 included)");
    let vag = std::fs::read(root.join("levels/01/speech/05_en.bin")).unwrap();
    let secs = ((vag.len() - 0x30) / 16 * 28) as f64 / 44056.0;
    assert!((secs - 24.55).abs() < 0.01, "speech 5 = {secs:.3} s");

    // The serialisation (and so the snapshot) sees one changed camera angle, one changed actor class, one subtitle byte.
    let k = r.scenes.iter().position(|s| s.index == 5 && s.region == Region::Ntsc).unwrap();
    r.scenes[k].chunks[3].camera[10].angles[1] = 0.5;
    let at = first_diff(&serialise(&r), &dump);
    assert!(serialise(&r) != dump && at > 0, "an edited camera angle must be caught");
    let original = load(&root.join("levels/01"));
    r.scenes[k].chunks[3].camera[10] = original.scenes[k].chunks[3].camera[10];
    assert!(serialise(&r) == dump);
    r.scenes[k].chunks[0].actors[2].class = 531;
    assert!(serialise(&r) != dump, "an edited actor class must be caught");
    r.scenes[k].chunks[0].actors[2].class = 530;
    r.scenes[k].chunks[2].subtitles[0].text[0][0] ^= 0x20;
    assert!(serialise(&r) != dump, "an edited subtitle must be caught");
    eprintln!("scenes: 19 levels, {n_scenes} scenes ({n_regions} NTSC/PAL regions), {n_chunks} chunks, {n_ticks} ticks, {n_actors} actors, \
               {n_subs} subtitle entries, {n_cuts} cuts");
    snap.finish();
}
