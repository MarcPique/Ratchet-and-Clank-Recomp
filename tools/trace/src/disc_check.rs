//! `disc-check`: the checks that need the user's disc image, run by hand (`cargo run --release -p rc-trace --
//! disc-check`; docs/workflows/testing.md §10). They were tests until 2026-09-29, when the rule "no test
//! relies on personal files" moved them here: the test suite reads only `extracted/` and committed fixtures.
//!
//! 1. The Rust disc reader (`rc_formats::disc`) against the extracted Tier 0 archive: for all 19 levels every
//!    `LevelFiles` member and every audio/scene lump is byte-identical to its file under `extracted/levels/NN/`, the
//!    TOC to `toc.bin` and the boot ELF to `boot/SCUS_971.99` (formerly `formats::golden::disc_matches_extracted_for_every_level`).
//! 2. The disc's `save_game` lump against `extracted/global/save_game.bin`, and the memory-card folder named in
//!    `SYSTEM.CNF` (formerly `ui::game_state_novalis::disc_save_game_lump_matches_extracted`).
//! 3. The extractor's size/SHA-1 table hashed from the image equals the committed one,
//!    `crates/rc-extract/data/scus_971_99.tsv` (formerly `extract::golden::builtin_table_matches_the_disc`).
//! 4. With `--extract-into <scratch dir>` (~7 GiB): a full extraction holds exactly the table's files and verifies,
//!    and an `--ntsc-only` one skips only PAL copies (formerly `extract::golden::extract_matches_the_committed_table`).

use anyhow::{bail, ensure, Context, Result};
use rc_extract::build_db::{self, BUILDS};
use rc_extract::extract::{self, Options};
use rc_extract::{verify, Event, INFO_FILE};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;

fn quiet(_: Event) {}

/// Runs the checks in order; the first failure stops with its message.
pub fn run(iso: &Path, extracted: &Path, extract_into: Option<&Path>) -> Result<()> {
    ensure!(iso.exists(), "no disc image at {}", iso.display());
    disc_matches_extracted(iso, extracted).context("check 1: disc reader against extracted/")?;
    println!("1. disc reader: every level lump, audio/scene lump, the TOC and the boot ELF equal extracted/");
    save_game_lump_matches_extracted(iso, extracted).context("check 2: save_game lump")?;
    println!("2. save_game lump equals extracted/global/save_game.bin; SYSTEM.CNF names /BASCUS-97199RATCHET");
    builtin_table_matches_the_disc(iso).context("check 3: size/SHA-1 table")?;
    println!("3. the size/SHA-1 table hashed from the image equals crates/rc-extract/data/scus_971_99.tsv");
    match extract_into {
        Some(dir) => {
            extract_matches_the_committed_table(iso, dir).context("check 4: extraction")?;
            println!("4. extraction: exactly the table's files, all verified; --ntsc-only skips only PAL copies");
        }
        None => println!("4. extraction: skipped (pass --extract-into <scratch dir>, ~7 GiB)"),
    }
    Ok(())
}

/// Check 1. Prints per-level ISO read times (first read / repeat read).
fn disc_matches_extracted(iso: &Path, root: &Path) -> Result<()> {
    let read = |rel: String| std::fs::read(root.join(&rel)).with_context(|| format!("reading extracted/{rel}"));
    let t0 = Instant::now();
    let disc = rc_formats::disc::Disc::open(iso)?;
    let open_ms = t0.elapsed().as_secs_f64() * 1e3;
    ensure!(disc.iso().raw_sector_size() == 2048, "raw sector size {}", disc.iso().raw_sector_size());
    ensure!(disc.toc().raw == read("toc.bin".into())?, "TOC differs from toc.bin");
    ensure!(disc.boot_elf()? == read("boot/SCUS_971.99".into())?, "boot ELF differs");
    ensure!(disc.level_ids() == (0..19).collect::<Vec<u32>>(), "level ids {:?}", disc.level_ids());
    println!("ISO {} ({} sectors of 2048): open + TOC + 19 level headers {open_ms:.1} ms", iso.display(), disc.iso().sector_count());

    const LUMPS: [&str; 15] = ["level_header.bin", "overlay.bin", "sound_bank.bin", "core_index.bin", "gs_ram.bin", "hud_header.bin",
        "hud_bank_0.bin", "hud_bank_1.bin", "hud_bank_2.bin", "hud_bank_3.bin", "hud_bank_4.bin", "core_data.bin",
        "gameplay_ntsc.bin", "gameplay_pal.bin", "occlusion.bin"];
    fn count_bins(dir: &Path) -> usize {
        let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
        rd.flatten().map(|e| e.path()).map(|p| if p.is_dir() { count_bins(&p) } else { p.extension().is_some_and(|x| x == "bin") as usize }).sum()
    }
    let (mut total_first, mut total_repeat, mut total_bytes) = (0.0, 0.0, 0usize);
    for id in disc.level_ids() {
        let dir = format!("levels/{id:02}");
        let t = Instant::now();
        let files = disc.level(id)?;
        let first = t.elapsed().as_secs_f64() * 1e3;
        let t = Instant::now();
        let again = disc.level(id)?;
        let repeat = t.elapsed().as_secs_f64() * 1e3;
        ensure!(files.total_bytes() == again.total_bytes(), "level {id}: a repeat read differs");
        for name in LUMPS {
            ensure!(files.file(name).is_some() == root.join(&dir).join(name).exists(), "level {id}: {name} presence differs");
        }
        for (name, bytes) in files.files() {
            ensure!(bytes == read(format!("{dir}/{name}"))?.as_slice(), "level {id}: {name} differs from extracted/{dir}/{name}");
        }
        let t = Instant::now();
        let streams = disc.level_stream_lumps(id)?;
        let mut stream_bytes = 0usize;
        for l in &streams {
            let b = disc.read_lump(l)?;
            stream_bytes += b.len();
            ensure!(b == read(format!("{dir}/{}.bin", l.name))?, "level {id}: {} differs", l.name);
        }
        let stream_ms = t.elapsed().as_secs_f64() * 1e3;
        // Audio: music/NNN, bindata/NNN; scenes: speech/KK_<lang> and scene/KK_ntsc|pal (rc_formats::toc::level_stream_lumps).
        let on_disk: usize = ["music", "bindata", "speech", "scene"].iter().map(|d| count_bins(&root.join(&dir).join(d))).sum();
        ensure!(streams.len() == on_disk, "level {id}: {} audio/scene lumps on the disc, {on_disk} in extracted/", streams.len());
        let mib = files.total_bytes() as f64 / (1 << 20) as f64;
        println!("level {id:02}: {} lumps {mib:6.1} MiB  first {first:7.1} ms  repeat {repeat:6.1} ms | {} audio/scene lumps {:6.1} MiB in {stream_ms:7.1} ms",
            files.files().len(), streams.len(), stream_bytes as f64 / (1 << 20) as f64);
        total_first += first;
        total_repeat += repeat;
        total_bytes += files.total_bytes();
    }
    println!("all levels: {:.1} MiB, first {total_first:.0} ms, repeat {total_repeat:.0} ms", total_bytes as f64 / (1 << 20) as f64);
    Ok(())
}

/// Check 2.
fn save_game_lump_matches_extracted(iso: &Path, root: &Path) -> Result<()> {
    let want = std::fs::read(root.join("global/save_game.bin")).context("reading extracted/global/save_game.bin")?;
    let disc = rc_formats::disc::Disc::open(iso)?;
    ensure!(disc.save_game_lump()? == want, "the disc's save_game lump differs from extracted/global/save_game.bin");
    let cnf = disc.iso().read_file(disc.iso().find("/SYSTEM.CNF").context("no /SYSTEM.CNF")?)?;
    let dir = rc_formats::save_game::card_dir_name(&cnf)?;
    ensure!(dir == "/BASCUS-97199RATCHET", "card folder {dir:?}");
    Ok(())
}

/// Check 3. After regenerating the committed table (`ratchet_clank-extract table --iso <image> --output
/// crates/rc-extract/data/scus_971_99.tsv`) this is how it is checked.
fn builtin_table_matches_the_disc(iso: &Path) -> Result<()> {
    let t = Instant::now();
    let (id, rows) = extract::table(iso, BUILDS, rc_extract::DEFAULT_THREADS, &mut quiet)?;
    println!("table: {} rows hashed from the image in {:.2} s", rows.len(), t.elapsed().as_secs_f64());
    let committed = build_db::parse_table(BUILDS[0].table.context("no built-in table")?).map_err(anyhow::Error::msg)?;
    ensure!(id.serial == BUILDS[0].serial, "disc serial {} (the table is for {})", id.serial, BUILDS[0].serial);
    if rows != committed { bail!("the table hashed from the image differs; regenerate crates/rc-extract/data/scus_971_99.tsv"); }
    Ok(())
}

/// Every file under `root`, relative, with `/` separators.
fn files(root: &Path) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)?.flatten() {
            if e.file_type()?.is_dir() { stack.push(e.path()); continue; }
            out.insert(e.path().strip_prefix(root)?.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(out)
}

/// Check 4: writes `<dir>/full` (~4.0 GiB) and `<dir>/ntsc` (~2.6 GiB); delete them afterwards.
fn extract_matches_the_committed_table(iso: &Path, dir: &Path) -> Result<()> {
    // The scratch folder is wiped per run: never the game data tree.
    crate::ensure_not_extracted(&dir.join("full"))?;
    let full = dir.join("full");
    let _ = std::fs::remove_dir_all(&full);
    let t = Instant::now();
    let (id, s) = extract::extract(iso, &full, BUILDS, &Options::default(), &mut quiet)?;
    ensure!(id.supported, "unsupported disc");
    println!("extract: {} files, {} bytes ({:.1} MiB) in {:.2} s", s.files, s.bytes, rc_extract::mib(s.bytes), t.elapsed().as_secs_f64());

    let mut ours = files(&full)?;
    ensure!(ours.remove(INFO_FILE), "no {INFO_FILE}");
    // The Tier 1 engine cache `extract` ends with (rc-data's own tests check its contents).
    ensure!(ours.iter().any(|p| p == "cache/v1/stamp.toml"), "no engine cache");
    ours.retain(|p| !p.starts_with("cache/"));
    // Exactly the table's files; `verify` below re-hashes every one against its size and SHA-1.
    let table: BTreeSet<String> = build_db::parse_table(BUILDS[0].table.context("no built-in table")?).map_err(anyhow::Error::msg)?.into_iter().map(|e| e.path).collect();
    let only_ours: Vec<_> = ours.difference(&table).collect();
    let only_table: Vec<_> = table.difference(&ours).collect();
    ensure!(only_ours.is_empty() && only_table.is_empty(), "only extracted: {only_ours:?}\nonly in the table: {only_table:?}");
    ensure!(s.files as usize == table.len(), "{} files extracted, {} in the table", s.files, table.len());

    let t = Instant::now();
    let (_, v) = verify::verify(&full, BUILDS, rc_extract::DEFAULT_THREADS, None, &mut quiet)?;
    println!("verify: {} files in {:.2} s", v.files, t.elapsed().as_secs_f64());

    let ntsc = dir.join("ntsc");
    let _ = std::fs::remove_dir_all(&ntsc);
    let t = Instant::now();
    let (_, n) = extract::extract(iso, &ntsc, BUILDS, &Options { ntsc_only: true, ..Options::default() }, &mut quiet)?;
    println!("extract --ntsc-only: {} files, {} bytes ({:.1} MiB) in {:.2} s", n.files, n.bytes, rc_extract::mib(n.bytes), t.elapsed().as_secs_f64());
    let skipped: Vec<_> = ours.difference(&files(&ntsc)?).cloned().collect();
    ensure!(skipped.iter().all(|p| rc_extract::archive::is_pal_copy(p)), "--ntsc-only skipped non-PAL files");
    ensure!(skipped.len() as u64 == s.files - n.files, "{} skipped, {} fewer files", skipped.len(), s.files - n.files);
    verify::verify(&ntsc, BUILDS, rc_extract::DEFAULT_THREADS, None, &mut quiet)?;
    Ok(())
}
