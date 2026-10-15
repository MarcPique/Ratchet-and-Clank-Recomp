//! End-to-end tests on synthetic disc images built in-test (no disc bytes in the repo): error codes for
//! non-R&C, truncated and random images, and a full identify → extract → verify cycle on a small RC1-shaped image
//! with an in-test build DB.

use rc_extract::build_db::{self, Build, Game};
use rc_extract::extract::{self, Options};
use rc_extract::json::{self, Value};
use rc_extract::sha1::{hex, sha1};
use rc_extract::{identify, verify, Code, Event, INFO_FILE};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

const SS: usize = 2048;

fn both32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
    b[o + 4..o + 8].copy_from_slice(&v.to_be_bytes());
}

fn dir_record(lba: u32, size: u32, dir: bool, name: &[u8]) -> Vec<u8> {
    let len = (33 + name.len() + 1) & !1;
    let mut r = vec![0u8; len];
    r[0] = len as u8;
    both32(&mut r, 2, lba);
    both32(&mut r, 10, size);
    r[25] = if dir { 2 } else { 0 };
    r[32] = name.len() as u8;
    r[33..33 + name.len()].copy_from_slice(name);
    r
}

/// An ISO 9660 image of `sectors` sectors with root files `SYSTEM.CNF` (booting `serial`) at sector 22 and the boot
/// ELF at sector 24.
fn iso(sectors: usize, serial: &str, elf: &[u8]) -> Vec<u8> {
    let mut img = vec![0u8; sectors * SS];
    let pvd = &mut img[16 * SS..17 * SS];
    pvd[0] = 1;
    pvd[1..6].copy_from_slice(b"CD001");
    pvd[6] = 1;
    pvd[40..72].copy_from_slice(format!("{:<32}", "SYNTHETIC").as_bytes());
    both32(pvd, 80, sectors as u32);
    pvd[128..130].copy_from_slice(&(SS as u16).to_le_bytes());
    pvd[156..156 + 34].copy_from_slice(&dir_record(20, SS as u32, true, &[0]));
    let cnf = format!("BOOT2 = cdrom0:\\{serial};1\r\nVER = 1.00\r\nVMODE = NTSC\r\n\r\n");
    let mut root = Vec::new();
    root.extend(dir_record(20, SS as u32, true, &[0]));
    root.extend(dir_record(20, SS as u32, true, &[1]));
    root.extend(dir_record(24, elf.len() as u32, false, format!("{serial};1").as_bytes()));
    root.extend(dir_record(22, cnf.len() as u32, false, b"SYSTEM.CNF;1"));
    img[20 * SS..20 * SS + root.len()].copy_from_slice(&root);
    img[22 * SS..22 * SS + cnf.len()].copy_from_slice(cnf.as_bytes());
    img[24 * SS..24 * SS + elf.len()].copy_from_slice(elf);
    img
}

fn put(img: &mut [u8], at: usize, v: i32) { img[at..at + 4].copy_from_slice(&v.to_le_bytes()); }

fn elf(tag: &str) -> Vec<u8> {
    let mut e = b"\x7fELF".to_vec();
    e.extend(format!("..{tag}..").as_bytes());
    e.resize(5000, 0x11);
    e
}

/// A small RC1-shaped disc: TOC at 1500, one level (id 1) with NTSC + PAL gameplay and scene regions, and global
/// save_game, an NTSC and a PAL FMV, and a PAL credits image. Returns the image.
fn rc1_disc(elf_tag: &str) -> Vec<u8> {
    let mut img = iso(1600, "SCUS_971.99", &elf(elf_tag));
    for (i, b) in img[1500 * SS..].iter_mut().enumerate() { *b = (i as u32).wrapping_mul(2654435761).rotate_right(13) as u8; }
    let toc = 1500 * SS;
    img[toc..toc + 0x2960].fill(0);
    put(&mut img, toc, 1);
    put(&mut img, toc + 4, 0x2960);
    put(&mut img, toc + 0x10, 1530);                      // save_game: 1 sector
    put(&mut img, toc + 0x14, 1);
    put(&mut img, toc + 0x1748, 1533);                    // credits_images_pal[0]
    put(&mut img, toc + 0x174c, 1);
    put(&mut img, toc + 0x17f8 + 21 * 8, 1531);           // mpegs[21] (PAL), 1000 bytes
    put(&mut img, toc + 0x17f8 + 21 * 8 + 4, 1000);
    put(&mut img, toc + 0x17f8 + 40 * 8, 1532);           // mpegs[40] (NTSC), 500 bytes
    put(&mut img, toc + 0x17f8 + 40 * 8 + 4, 500);
    put(&mut img, toc + 0x28c8, 1510);                    // level table slot 0
    put(&mut img, toc + 0x28cc, 5);
    let h = 1510 * SS;
    img[h..h + 0x2434].fill(0);
    for (o, v) in [(0, 1), (4, 0x2434), (8, 1516), (12, 2), (16, 1518), (20, 1), (24, 1519), (28, 1)] { put(&mut img, h + o, v); }
    for (o, v) in [(0x19c, 1520), (0x1a0, 1521), (0x2b8, 1522), (0x2bc, 1523)] { put(&mut img, h + o, v); }   // scene 0 NTSC / PAL
    let d = 1516 * SS;
    img[d..d + 0x58].fill(0);
    for (o, v) in [(0, 0x80), (4, 0x20), (0x50, 0x100), (0x54, 0x50)] { put(&mut img, d + o, v); }             // overlay, core_data
    img
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ratchet_clank-extract-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

fn collect<T>(f: impl FnOnce(&mut dyn FnMut(Event)) -> T) -> (T, Vec<Event>) {
    let mut ev = Vec::new();
    let r = f(&mut |e| ev.push(e));
    (r, ev)
}

#[test]
fn error_codes_for_bad_images() {
    let dir = scratch("codes");
    let builds = build_db::BUILDS;
    let code = |p: &Path| identify::identify(p, builds, &mut |_| {}).map(|_| ()).unwrap_err().code;

    assert_eq!(code(&dir.join("missing.iso")), Code::CannotRead);
    assert_eq!(code(&dir), Code::CannotRead);
    let random: Vec<u8> = (0..300_000u32).map(|i| (i.wrapping_mul(2654435761) >> 7) as u8).collect();
    assert_eq!(code(&write(&dir, "random.iso", &random)), Code::NotIso);
    assert_eq!(code(&write(&dir, "tiny.iso", b"hello")), Code::NotIso);

    let other = iso(64, "SLUS_123.45", &elf("some other game"));
    let (r, ev) = collect(|e| identify::identify(&write(&dir, "other.iso", &other), builds, e).map(|_| ()));
    assert_eq!(r.unwrap_err().code, Code::NotRatchet);
    assert!(ev.iter().any(|e| matches!(e, Event::Disc(d) if d.game == Game::Unknown && d.serial == "SLUS_123.45" && !d.supported)));
    // Truncated after the directory: the PVD declares 64 sectors.
    assert_eq!(code(&write(&dir, "cut.iso", &other[..30 * SS])), Code::CannotRead);
    // ISO 9660 without SYSTEM.CNF.
    let mut no_cnf = other.clone();
    no_cnf[20 * SS..21 * SS].fill(0);
    no_cnf[20 * SS..20 * SS + 34].copy_from_slice(&dir_record(20, SS as u32, true, &[0]));
    assert_eq!(code(&write(&dir, "nocnf.iso", &no_cnf)), Code::NotRatchet);

    // An RC1 serial with an unknown boot ELF: 21 and a ready-to-paste DB row.
    let (r, ev) = collect(|e| identify::identify(&write(&dir, "rc1.iso", &rc1_disc("Ratchet & Clank")), builds, e).map(|_| ()));
    assert_eq!(r.unwrap_err().code, Code::Unsupported);
    let row = ev.iter().find_map(|e| match e { Event::Info(m) => m.strip_prefix("new build DB row: "), _ => None }).expect("DB row");
    assert!(row.starts_with("Build { serial: \"SCUS_971.99\", version: \"1.00\", region: \"NTSC-U\", elf_size: 5000"), "{row}");
    // A sequel.
    assert_eq!(code(&write(&dir, "rac2.iso", &iso(64, "SCUS_972.68", &elf("x")))), Code::Unsupported);
    // Unknown serial, but the ELF names the game.
    assert_eq!(code(&write(&dir, "proto.iso", &iso(64, "PROTO.ELF", &elf("Ratchet & Clank")))), Code::Unsupported);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn identify_extract_verify_on_a_synthetic_rc1_disc() {
    let dir = scratch("e2e");
    let img = rc1_disc("Ratchet & Clank");
    let iso_path = write(&dir, "disc.iso", &img);
    let elf_sha = hex(&sha1(&elf("Ratchet & Clank")));

    // Table from the image (the `table` subcommand's code path), then a build DB that supports it.
    let probe_db = [Build { serial: "SCUS_971.99", version: "1.00", region: "NTSC-U", elf_size: 5000, elf_sha1: &elf_sha, image_sectors: 1600, supported: false, table: None }];
    let (id, rows) = extract::table(&iso_path, &probe_db, 2, &mut |_| {}).unwrap();
    assert_eq!(id.game, Game::Rac1);
    let paths: Vec<&str> = rows.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(paths, [
        "boot/SCUS_971.99", "boot/SYSTEM.CNF", "toc.bin", "global/save_game.bin", "global/credits_images_pal/000.bin",
        "global/mpegs/021.bin", "global/mpegs/040.bin", "levels/01/level_header.bin", "levels/01/overlay.bin", "levels/01/core_data.bin",
        "levels/01/gameplay_ntsc.bin", "levels/01/gameplay_pal.bin", "levels/01/scene/00_ntsc.bin", "levels/01/scene/00_pal.bin",
    ]);
    let table = build_db::format_table("SCUS_971.99", "1.00", &rows);
    let db = [Build { supported: true, table: Some(&table), ..probe_db[0] }];

    let (r, ev) = collect(|e| identify::identify(&iso_path, &db, e).map(|(id, _)| id));
    assert!(r.unwrap().supported);
    let disc_line = ev.iter().find(|e| matches!(e, Event::Disc(_))).unwrap().to_json();
    let obj = json::parse_object(&disc_line).unwrap();
    for (k, v) in [("type", "disc"), ("serial", "SCUS_971.99"), ("region", "NTSC-U"), ("version", "1.00"), ("game", "rac1")] {
        assert_eq!(json::get(&obj, k), Some(&Value::Str(v.into())), "{k}");
    }
    assert_eq!(json::get(&obj, "supported"), Some(&Value::Bool(true)));

    // Full extraction: every file equals its byte range of the image.
    let out = dir.join("data");
    std::fs::create_dir_all(out.join("levels/01")).unwrap();
    write(&out.join("levels/01"), "core_data.bin.partial", b"left by a killed run");
    write(&out, INFO_FILE, b"stale");
    let (r, ev) = collect(|e| extract::extract(&iso_path, &out, &db, &Options { threads: 3, ..Options::default() }, e));
    let (_, s) = r.unwrap();
    assert_eq!(s.files, 14);
    for r in &rows {
        let got = std::fs::read(out.join(&r.path)).unwrap();
        assert_eq!((got.len() as u64, sha1(&got)), (r.size, r.sha1), "{}", r.path);
    }
    assert_eq!(std::fs::read(out.join("toc.bin")).unwrap(), &img[1500 * SS..1500 * SS + 0x2960]);
    assert_eq!(std::fs::read(out.join("levels/01/overlay.bin")).unwrap(), &img[1516 * SS + 0x80..1516 * SS + 0xa0]);
    assert!(!out.join("levels/01/core_data.bin.partial").exists());
    let info = verify::read_info(&out).unwrap();
    assert_eq!((info.disc.as_str(), info.data_format, info.ntsc_only, info.files, info.bytes as u64), ("SCUS_971.99", 1, false, 14, s.bytes));
    let progress: Vec<_> = ev.iter().filter_map(|e| match e { Event::Progress { stage, done, total, .. } => Some((*stage, *done, *total)), _ => None }).collect();
    let copy_end = progress.iter().rposition(|p| p.0 == rc_extract::Stage::Copy).unwrap();
    assert_eq!(progress[copy_end], (rc_extract::Stage::Copy, s.bytes, s.bytes));
    // Then the prepare stage. The synthetic level lumps are not WAD streams, so the engine cache is not built; that
    // is one info line, not a failed extraction (the archive is complete, the game builds the cache itself).
    assert!(progress[copy_end + 1..].iter().all(|p| p.0 == rc_extract::Stage::Prepare) && progress.len() > copy_end + 1, "{progress:?}");
    let infos: Vec<&String> = ev.iter().filter_map(|e| match e { Event::Info(m) => Some(m), _ => None }).collect();
    assert!(infos.iter().any(|m| m.starts_with("the engine cache was not built (") && m.contains("not a valid WAD stream")), "{infos:?}");
    assert!(out.join("cache/v1/stamp.toml").is_file());

    // Verify passes, then names a corrupted and a missing file.
    let (_, vs) = verify::verify(&out, &db, 2, None, &mut |_| {}).unwrap();
    assert_eq!(vs.files, 14);
    let mut b = std::fs::read(out.join("global/mpegs/040.bin")).unwrap();
    b[7] ^= 1;
    std::fs::write(out.join("global/mpegs/040.bin"), b).unwrap();
    std::fs::remove_file(out.join("levels/01/scene/00_pal.bin")).unwrap();
    let (r, ev) = collect(|e| verify::verify(&out, &db, 2, None, e));
    let err = r.unwrap_err();
    assert_eq!((err.code, err.message.starts_with("2 file(s)")), (Code::VerifyFailed, true), "{}", err.message);
    let infos: Vec<String> = ev.iter().filter_map(|e| match e { Event::Info(m) => Some(m.clone()), _ => None }).collect();
    assert!(infos.iter().any(|m| m.starts_with("global/mpegs/040.bin: SHA-1")), "{infos:?}");
    assert!(infos.iter().any(|m| m == "levels/01/scene/00_pal.bin: missing"), "{infos:?}");

    // --ntsc-only into a fresh folder: no PAL copies, and verify accepts that.
    let ntsc = dir.join("ntsc");
    let (_, s) = extract::extract(&iso_path, &ntsc, &db, &Options { ntsc_only: true, ..Options::default() }, &mut |_| {}).unwrap();
    assert_eq!(s.files, 10);
    for p in ["global/mpegs/021.bin", "global/credits_images_pal/000.bin", "levels/01/gameplay_pal.bin", "levels/01/scene/00_pal.bin"] {
        assert!(!ntsc.join(p).exists(), "{p}");
    }
    assert!(ntsc.join("global/mpegs/040.bin").exists());
    assert!(verify::read_info(&ntsc).unwrap().ntsc_only);
    verify::verify(&ntsc, &db, 1, None, &mut |_| {}).unwrap();

    // Cancelling (library callers) or killing (the binary) never leaves a complete-looking folder.
    let cancelled = dir.join("cancelled");
    let flag = AtomicBool::new(false);
    let r = extract::extract(&iso_path, &cancelled, &db, &Options { cancel: Some(&flag), ..Options::default() }, &mut |e| {
        if matches!(e, Event::Info(ref m) if m.starts_with("extracting")) { flag.store(true, Ordering::Relaxed); }
    });
    assert_eq!(r.unwrap_err().message, "cancelled");
    assert!(!cancelled.join(INFO_FILE).exists());
    assert_eq!(walk(&cancelled).iter().filter(|p| p.ends_with(".partial")).count(), 0);
    assert_eq!(verify::verify(&cancelled, &db, 1, None, &mut |_| {}).unwrap_err().code, Code::VerifyFailed);

    // A damaged image of the supported build: extraction stops with 40 and names the file.
    let mut bad = img.clone();
    bad[1532 * SS + 3] ^= 0xff;
    let bad_path = write(&dir, "bad.iso", &bad);
    let (r, ev) = collect(|e| extract::extract(&bad_path, &dir.join("bad"), &db, &Options::default(), e));
    assert_eq!(r.unwrap_err().code, Code::VerifyFailed);
    assert!(ev.iter().any(|e| matches!(e, Event::Info(m) if m.starts_with("global/mpegs/040.bin"))));
    assert!(!dir.join("bad/global/mpegs/040.bin").exists() && !dir.join("bad").join(INFO_FILE).exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

fn walk(d: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![d.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            if e.file_type().unwrap().is_dir() { stack.push(e.path()) } else { out.push(e.path().to_string_lossy().into_owned()) }
        }
    }
    out
}

/// The binary: JSON lines, one terminal line, exit code = error code.
#[test]
fn binary_json_lines_and_exit_codes() {
    let dir = scratch("bin");
    let exe = env!("CARGO_BIN_EXE_ratchet_clank-extract");
    let run = |args: &[&str]| {
        let o = std::process::Command::new(exe).args(args).env_remove("RC_ISO").output().unwrap();
        let lines: Vec<Vec<(String, Value)>> = String::from_utf8(o.stdout).unwrap().lines().map(|l| json::parse_object(l).unwrap_or_else(|| panic!("not JSON: {l}"))).collect();
        (o.status.code().unwrap(), lines)
    };
    let ty = |l: &[(String, Value)]| match json::get(l, "type") { Some(Value::Str(s)) => s.clone(), _ => panic!() };

    let other = write(&dir, "other.iso", &iso(64, "SLUS_123.45", &elf("x")));
    let random = write(&dir, "random.bin", &vec![0x5a; 100_000]);
    let rc1 = write(&dir, "rc1.iso", &rc1_disc("Ratchet & Clank"));
    for (args, code) in [
        (vec!["identify", "--iso", other.to_str().unwrap(), "--json"], 20),
        (vec!["identify", "--iso", random.to_str().unwrap(), "--json"], 11),
        (vec!["identify", "--iso", "/no/such/file.iso", "--json"], 10),
        (vec!["identify", "--iso", rc1.to_str().unwrap(), "--json"], 21),
        (vec!["extract", "--iso", rc1.to_str().unwrap(), "--out", dir.join("o").to_str().unwrap(), "--json"], 21),
        (vec!["verify", "--out", dir.join("o").to_str().unwrap(), "--json"], 40),
        (vec!["identify", "--json"], 99),
    ] {
        let (status, lines) = run(&args);
        assert_eq!(status, code, "{args:?}");
        let last = lines.last().unwrap();
        assert_eq!(ty(last), "error");
        assert_eq!(json::get(last, "code"), Some(&Value::Num(code as i64)));
        assert_eq!(lines.iter().filter(|l| matches!(ty(l).as_str(), "error" | "done")).count(), 1);
        for l in &lines {
            match ty(l).as_str() {
                "progress" => for k in ["stage", "done", "total", "file"] { assert!(json::get(l, k).is_some(), "{k}"); },
                "disc" => for k in ["serial", "region", "version", "supported"] { assert!(json::get(l, k).is_some(), "{k}"); },
                "info" | "error" => assert!(matches!(json::get(l, "message"), Some(Value::Str(_)))),
                t => panic!("unexpected line type {t}"),
            }
        }
    }
    let (status, lines) = run(&["identify", "--iso", rc1.to_str().unwrap(), "--json"]);
    assert_eq!(status, 21);
    assert!(lines.iter().any(|l| ty(l) == "progress" && json::get(l, "stage") == Some(&Value::Str("identify".into()))));
    assert!(lines.iter().any(|l| matches!(json::get(l, "message"), Some(Value::Str(m)) if m.starts_with("new build DB row: "))));
    let v = std::process::Command::new(exe).arg("--version").output().unwrap();
    assert_eq!((v.status.code(), String::from_utf8(v.stdout).unwrap().trim()), (Some(0), format!("ratchet_clank-extract {}", rc_extract::EXTRACTOR_VERSION).as_str()));
    std::fs::remove_dir_all(&dir).unwrap();
}
