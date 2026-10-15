//! The decoder on the disc's movies (Tier 0 `global/mpegs/NNN.bin` under `rc_formats::test_data::root()`); every
//! test skips when the files are absent.
//!
//! * `holofilm_start` (always): the first 90 frames of `mpegs[5]` (the Novalis holofilm, in-level movie 3).
//! * `every_movie_decodes` (`--ignored`): every file end to end; prints frames, rates, durations, the header
//!   features used and the decode speed. `RC_MOVIES=005,040` restricts it.
//! * `dump_yuv` (`--ignored`, dev oracle hook): `RC_MOVIE_DUMP=NNN RC_MOVIE_DUMP_OUT=<file>` writes the decoded
//!   frames as raw yuv420p (`RC_MOVIE_DUMP_FRAMES=n` limits them), for comparison with an external decoder.

use rc_video::{Movie, PictureType};
use std::path::PathBuf;
use std::time::Instant;

fn movie_path(index: u32) -> PathBuf { rc_formats::test_data::root().join(format!("global/mpegs/{index:03}.bin")) }

#[test]
fn holofilm_start() {
    let Ok(file) = std::fs::read(movie_path(5)) else { eprintln!("skip: no mpegs/005.bin"); return };
    let mut m = Movie::open(&file, 0).unwrap();
    let s = m.sequence;
    assert_eq!((s.width, s.height, s.fps_num, s.fps_den, s.profile_level, s.progressive, s.chroma_format), (512, 416, 30, 1, 0x48, true, 1));
    assert_eq!(m.channels, [0, 2, 3, 4, 5]);
    assert_eq!((m.audio_channel, m.audio_rate), (Some(0), 48_000));
    let audio = m.audio.as_ref().unwrap().len();
    // SSbd 0x285900 bytes: 2 channels of 16-byte frames, 28 samples each = 2,313,696 samples per channel (48.2 s).
    assert_eq!(audio, 0x285900 / 32 * 28);
    let t = Instant::now();
    let mut kinds = [0usize; 3];
    let mut last = None;
    for k in 0..90 {
        let f = m.next_frame().unwrap().unwrap_or_else(|| panic!("frame {k} missing"));
        assert_eq!((f.width, f.height, f.y.len(), f.cb.len()), (512, 416, 512 * 416, 256 * 208));
        kinds[f.kind as usize] += 1;
        // Display order within a GOP: temporal references count up by one.
        if let Some(prev) = last {
            if f.temporal_reference != 0 { assert_eq!(f.temporal_reference, prev + 1, "frame {k}"); }
        }
        last = Some(f.temporal_reference);
    }
    let secs = t.elapsed().as_secs_f64();
    eprintln!("mpegs[5]: 90 frames ({kinds:?} I/P/B) in {secs:.2} s = {:.0} fps (dev build)", 90.0 / secs);
    assert!(kinds[PictureType::I as usize] >= 1 && kinds[PictureType::B as usize] > 0);
}

#[test]
#[ignore]
fn every_movie_decodes() {
    let dir = rc_formats::test_data::root().join("global/mpegs");
    let Ok(rd) = std::fs::read_dir(&dir) else { eprintln!("skip: no {}", dir.display()); return };
    let only: Option<Vec<String>> = std::env::var("RC_MOVIES").ok().map(|v| v.split(',').map(|s| format!("{}.bin", s.trim())).collect());
    let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "bin")).collect();
    files.retain(|p| only.as_ref().is_none_or(|o| o.iter().any(|n| p.ends_with(n))));
    files.sort();
    let (mut total_frames, mut total_secs, mut total_video) = (0usize, 0.0f64, 0.0f64);
    let mut failures = Vec::new();
    println!("file | frames | I/P/B | fps | video s | audio s (ch 0) | channels | rate | dc prec 8/9/10 | qst 0/1 | ivf 0/1 | gops (closed) | skipped B | decode fps");
    for p in &files {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let file = std::fs::read(p).unwrap();
        let t = Instant::now();
        let mut m = match Movie::open(&file, 0) {
            Ok(m) => m,
            Err(e) => {
                failures.push(format!("{name}: {e}"));
                continue;
            }
        };
        let mut frames = 0usize;
        let res = loop {
            match m.next_frame() {
                Ok(Some(_)) => frames += 1,
                Ok(None) => break Ok(()),
                Err(e) => break Err(e),
            }
        };
        let secs = t.elapsed().as_secs_f64();
        if let Err(e) = res { failures.push(format!("{name}: after {frames} frames: {e}")); }
        let st = &m.decoder.stats;
        let video = frames as f64 / m.fps();
        let audio = m.audio.as_ref().map_or(0.0, |a| a.len() as f64 / 48_000.0);
        println!(
            "{name} | {frames} | {:?} | {:.3} | {video:.2} | {audio:.2} | {:?} | {} | {:?} | {:?} | {:?} | {} ({}) | {} | {:.0}",
            st.pictures, m.fps(), m.channels, m.audio_rate, &st.dc_precision[..3], st.q_scale_type, st.intra_vlc_format, st.gops, st.closed_gops, st.skipped_b, frames as f64 / secs
        );
        total_frames += frames;
        total_secs += secs;
        total_video += video;
    }
    println!("{} files, {total_frames} frames, {total_video:.1} s of video, decoded in {total_secs:.1} s ({:.0} fps, {:.1}× real time at 30 fps)", files.len(), total_frames as f64 / total_secs, total_frames as f64 / total_secs / 30.0);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
#[ignore]
fn dump_yuv() {
    let (Ok(index), Ok(out)) = (std::env::var("RC_MOVIE_DUMP"), std::env::var("RC_MOVIE_DUMP_OUT")) else { eprintln!("skip: RC_MOVIE_DUMP / RC_MOVIE_DUMP_OUT not set"); return };
    let limit: usize = std::env::var("RC_MOVIE_DUMP_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
    let file = std::fs::read(movie_path(index.trim().parse().unwrap())).unwrap();
    let mut m = Movie::open(&file, 0).unwrap();
    let mut w = std::io::BufWriter::new(std::fs::File::create(&out).unwrap());
    let mut n = 0;
    while n < limit {
        let Some(f) = m.next_frame().unwrap() else { break };
        use std::io::Write;
        w.write_all(&f.y).unwrap();
        w.write_all(&f.cb).unwrap();
        w.write_all(&f.cr).unwrap();
        n += 1;
    }
    eprintln!("wrote {n} frames to {out}");
}
