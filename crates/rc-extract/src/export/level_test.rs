//! The ignored end-to-end test: export level 01 completely from the development archive, then read every file
//! back with the test decoders (PNG, WAV, JSON) and the glTF validator, and cross-check against the loaders.
//!
//! Run: `RC_EXPORT_TEST_DIR=<dir> cargo xtask test-quick rc-extract --filter export_level_01 --ignored --nocapture`
//! (default dir: `<tmp>/ratchet_clank-export-test`; removed afterwards unless `RC_EXPORT_KEEP=1`). `RC_EXPORT_TEST_ALL=1`
//! exports and checks every level and the global data instead (about 60,000 files, 3.5 GiB).

use super::{export, gltf, jsonv, png, wav, Kinds, Options};
use rc_formats::{level, sound_bank, texture, vag, wad};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() { walk(&p, out) } else { out.push(p) }
    }
}

#[test]
#[ignore = "needs the extracted archive; writes about 180 MB"]
fn export_level_01_fully() {
    let data = rc_formats::test_data::root();
    if !data.join("toc.bin").exists() { eprintln!("skipped: no extracted/"); return; }
    let to = std::env::var_os("RC_EXPORT_TEST_DIR").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("ratchet_clank-export-test"));
    let _ = std::fs::remove_dir_all(&to);
    let t0 = std::time::Instant::now();
    let mut lines = Vec::new();
    let all = std::env::var_os("RC_EXPORT_TEST_ALL").is_some_and(|v| v != "0");
    let level = if all { None } else { Some(1) };
    let ex = export(&data, &Options { to: to.clone(), kinds: Kinds::ALL, level, threads: 4, cancel: None }, &mut |e| lines.push(e.to_json())).unwrap();
    let secs = t0.elapsed().as_secs_f64();
    assert_eq!(ex.skipped, 0, "skipped items: {lines:#?}");
    assert!(lines.last().is_some_and(|l| l.contains("\"stage\":\"export\"") && jsonv::parse(l).is_ok()));

    let mut files = Vec::new();
    walk(&to, &mut files);
    assert_eq!(files.len() as u64, ex.files, "every written file is counted");
    let info = jsonv::parse(&std::fs::read_to_string(to.join(super::INFO_FILE)).unwrap()).unwrap();
    assert_eq!(info.get("files").and_then(jsonv::J::as_usize), Some(files.len() - 1), "export-info.json counts the other files");
    let mut by_ext: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut gl = gltf::Stats::default();
    let (mut pngs, mut wavs, mut wav_samples) = (0usize, 0usize, 0usize);
    for f in &files {
        let name = f.to_string_lossy();
        assert!(!name.ends_with(".partial"), "{name}");
        let bytes = std::fs::read(f).unwrap();
        let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
        let e = by_ext.entry(ext.clone()).or_default();
        e.0 += 1;
        e.1 += bytes.len() as u64;
        match ext.as_str() {
            "png" => { png::decode::decode(&bytes).unwrap_or_else(|e| panic!("{name}: {e}")); pngs += 1; }
            "wav" => { let w = wav::decode::decode(&bytes).unwrap_or_else(|e| panic!("{name}: {e}")); wavs += 1; wav_samples += w.samples.len(); }
            "json" => { jsonv::parse(std::str::from_utf8(&bytes).unwrap()).unwrap_or_else(|e| panic!("{name}: {e}")); }
            "gltf" => {
                let dir = f.parent().unwrap();
                let bin = std::fs::read(f.with_extension("bin")).unwrap();
                let st = gltf::validate(std::str::from_utf8(&bytes).unwrap(), &bin, &|uri| dir.join(uri).is_file()).unwrap_or_else(|e| panic!("{name}: {e}"));
                gl.meshes += st.meshes; gl.primitives += st.primitives; gl.triangles += st.triangles; gl.vertices += st.vertices;
                gl.nodes += st.nodes; gl.materials += st.materials; gl.images += st.images; gl.skins += st.skins; gl.animations += st.animations;
            }
            "bin" => {}
            _ => panic!("unexpected file {name}"),
        }
    }

    // One PNG per texture the loader decodes (tables and billboards; mips are extra files).
    let idx = std::fs::read(data.join("levels/01/core_index.bin")).unwrap();
    let core_data = wad::decompress(&std::fs::read(data.join("levels/01/core_data.bin")).unwrap()).unwrap();
    let gs = std::fs::read(data.join("levels/01/gs_ram.bin")).unwrap();
    let core = level::parse_level_core(&idx, core_data.len()).unwrap();
    let texs = texture::parse_textures(&core, &core_data, &gs).unwrap();
    for t in &texs {
        let p = to.join(format!("textures/levels/01/{}.png", t.key()));
        let img = png::decode::decode(&std::fs::read(&p).unwrap()).unwrap();
        assert_eq!((img.width, img.height), (t.texture.width, t.texture.height), "{}", p.display());
        assert!(img.rgba == t.texture.rgba, "{}: pixels differ from the loader", p.display());
    }
    let table_pngs = files.iter().filter(|f| {
        let s = f.to_string_lossy();
        ["tfrag", "moby", "tie", "shrub", "billboard"].iter().any(|t| s.contains(&format!("textures/levels/01/{t}/"))) && s.ends_with(".png") && !s.contains(".mip")
    }).count();
    assert_eq!(table_pngs, texs.len());

    // Bank WAVs hold exactly the decoder's samples.
    let bank = sound_bank::parse_bank(&std::fs::read(data.join("levels/01/sound_bank.bin")).unwrap()).unwrap();
    for (i, ext) in bank.vags.iter().enumerate() {
        let w = wav::decode::decode(&std::fs::read(to.join(format!("audio/levels/01/sound_bank/{i:03}.wav"))).unwrap()).unwrap();
        assert_eq!(w.samples, vag::decode_extent(&bank.samples, ext), "bank sample {i}");
        assert_eq!(w.lp.map(|l| (l.start as usize, l.end as usize)), ext.loop_points(), "bank sample {i} loop");
    }

    eprintln!("{} export: {} files, {:.1} MiB in {secs:.2} s (4 workers)", if all { "full" } else { "level 01" }, ex.files, ex.bytes as f64 / 1048576.0);
    for (k, (n, b)) in &ex.by_folder { eprintln!("  {k:<9} {n:>5} files {:>8.1} MiB", *b as f64 / 1048576.0); }
    for (k, (n, b)) in &by_ext { eprintln!("  .{k:<8} {n:>5} files {:>8.1} MiB", *b as f64 / 1048576.0); }
    eprintln!("  checked: {pngs} PNG, {wavs} WAV ({wav_samples} samples), glTF {gl:?}");
    if std::env::var_os("RC_EXPORT_KEEP").is_none() { let _ = std::fs::remove_dir_all(&to); }
}
