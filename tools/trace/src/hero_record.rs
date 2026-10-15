//! `rc-trace record`: a live per-tick recorder of the hero over PCSX2's PINE socket. **Read-only**: it never writes
//! EE memory; the only other request it can make is a savestate, and only when asked (`--savestate N`). Doc:
//! docs/plan/hero_feel_pass.md.
//!
//! Each poll is ONE PINE message of `MsgRead64`s: the tick counter, [`RANGES`], Ratchet's moby anim fields, the
//! camera record, and the tick counter again. A poll whose two counter reads differ straddled a tick and is
//! thrown away (counted as torn). A row is written the first time a new counter value is seen, so each tick is
//! recorded at most once; a jump of the counter by more than one is counted as missed ticks.

use crate::hero_trace::{self, sample_from, Mem, Trace, MOBY_PTR, RANGES, TICK};
use crate::pine::Pine;
use anyhow::{Context, Result};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// One poll's bytes: 8-byte words at their addresses.
pub struct Snapshot {
    pub ranges: Vec<(u32, Vec<u8>)>,
}

impl Mem for Snapshot {
    fn get(&self, addr: u32, len: usize) -> Option<&[u8]> {
        self.ranges.iter().find_map(|(a, b)| {
            let off = addr.checked_sub(*a)? as usize;
            (off + len <= b.len()).then(|| &b[off..off + len])
        })
    }
}

/// The poll plan: the ranges of one message, with the counter first and last.
pub struct Plan {
    pub ranges: Vec<(u32, u32)>,
    pub addrs: Vec<u32>,
}

impl Plan {
    pub fn new(moby: u32, camera: Option<u32>) -> Plan {
        let mut ranges: Vec<(u32, u32)> = RANGES.to_vec();
        if moby != 0 { ranges.push((moby + 0x50, 8)); }
        if let Some(c) = camera {
            ranges.push((c, 0x20));
            ranges.push((c + 0x210, 0x30));
        }
        let mut addrs = vec![TICK & !7];
        for &(a, n) in &ranges { addrs.extend((0..n / 8).map(|k| a + 8 * k)); }
        addrs.push(TICK & !7);
        Plan { ranges, addrs }
    }

    /// Splits a reply into the snapshot and the two counter reads.
    pub fn split(&self, words: &[[u8; 8]]) -> (Snapshot, u32, u32) {
        let ctr = |w: &[u8; 8]| u32::from_le_bytes(w[(TICK & 7) as usize..(TICK & 7) as usize + 4].try_into().unwrap());
        let (t0, t1) = (ctr(&words[0]), ctr(&words[words.len() - 1]));
        let mut k = 1;
        let ranges = self
            .ranges
            .iter()
            .map(|&(a, n)| {
                let b: Vec<u8> = words[k..k + (n / 8) as usize].iter().flatten().copied().collect();
                k += (n / 8) as usize;
                (a, b)
            })
            .collect();
        (Snapshot { ranges }, t0, t1)
    }
}

/// Options of a recording.
pub struct RecordOptions {
    pub slot: u16,
    pub out: PathBuf,
    pub seconds: Option<f64>,
    pub savestate: Option<u8>,
    /// Sleep between polls that saw no new tick.
    pub poll_us: u64,
    pub quiet: bool,
}

/// What a recording saw.
#[derive(Debug, Default)]
pub struct RecordSummary {
    pub rows: usize,
    pub polls: u64,
    pub torn: u64,
    pub missed: u64,
    /// (last recorded tick, next recorded tick) of each gap.
    pub gaps: Vec<(u32, u32)>,
    pub counter_resets: u32,
    pub seconds: f64,
    pub first_tick: Option<u32>,
    pub last_tick: Option<u32>,
    /// Rows whose rebuilt pad bytes differ from libpad2's newer DMA buffer (diagnostic).
    pub pad_vs_libpad: usize,
    pub level_changes: u32,
}

/// `~/PS2/ratchet1/traces/` (`RC_PERSONAL` overrides `~/PS2/ratchet1`; see [`crate::recordings_dir`]).
pub fn default_dir() -> PathBuf { crate::recordings_dir() }

/// UTC `YYYYMMDD-HHMMSS` of now (no date crate: the civil-from-days algorithm).
pub fn utc_stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3600, rem / 60 % 60, rem % 60)
}

pub const INSTRUCTIONS: &str = "\
Recording Ratchet over PINE (read-only: nothing is written to the game's memory).
  * Keep PCSX2 running at full speed (no fast-forward, no frame advance); don't pause.
  * Start with Ratchet standing still on open ground (the replay starts from the first standing tick).
  * Play the move script, then wait ~1 s standing still.
  * Press Enter here to stop (or wait for --seconds).";

/// The camera record and the moby pointer for the current level (reads the scan block once).
fn locate(p: &mut Pine) -> Result<(i32, u32, Option<u32>)> {
    let level = u32::from_le_bytes(p.read(hero_trace::LEVEL & !7, 8)?[4..8].try_into().unwrap()) as i32;
    let moby = u32::from_le_bytes(p.read(MOBY_PTR, 8)?[..4].try_into().unwrap());
    let hero = {
        let b = p.read(0x13f3d0, 16)?;
        [0, 4, 8].map(|o| f32::from_le_bytes(b[o..o + 4].try_into().unwrap()))
    };
    let (base, len) = hero_trace::CAMERA_SCAN;
    let mut ranges = vec![(base, p.read(base, len)?)];
    for &(l, a) in hero_trace::KNOWN_CAMERAS {
        if l == level && !(base..base + len as u32).contains(&a) { ranges.push((a, p.read(a, 0x240)?)); }
    }
    let snap = Snapshot { ranges };
    Ok((level, moby, hero_trace::locate_camera(&snap, level, hero)))
}

/// Records until Enter, `--seconds`, or an error.
pub fn record(opt: &RecordOptions) -> Result<RecordSummary> {
    let mut p = Pine::connect(opt.slot)?;
    let game = p.game_id().unwrap_or_default();
    let status = p.status_name();
    println!("PCSX2 {} | game {game} | {status}", p.version().unwrap_or_default());
    if status != "running" { println!("warning: the emulator is {status}; ticks are recorded only while the game runs"); }
    if let Some(n) = opt.savestate {
        p.save_state(n)?;
        println!("asked PCSX2 to save state slot {n} (redo a take by loading it)");
    }
    let (mut level, mut moby, mut camera) = locate(&mut p)?;
    println!("level {level}, Ratchet's moby {moby:#x}, camera record {}", camera.map_or("NOT FOUND (camera columns = 0)".into(), |c| format!("{c:#x}")));
    let mut trace = Trace::default();
    trace.set_meta("source", "pcsx2-pine");
    trace.set_meta("slot", opt.slot.to_string());
    trace.set_meta("game", game.clone());
    trace.set_meta("started_utc", utc_stamp());
    trace.set_meta("level", level.to_string());
    trace.set_meta("moby", format!("{moby:#x}"));
    trace.set_meta("camera", camera.map_or("none".into(), |c| format!("{c:#x}")));
    crate::ensure_not_extracted(&opt.out)?;
    if let Some(d) = opt.out.parent() { std::fs::create_dir_all(d).with_context(|| format!("creating {}", d.display()))?; }
    let file = std::fs::File::create(&opt.out).with_context(|| format!("creating {}", opt.out.display()))?;
    let mut w = std::io::BufWriter::new(file);
    w.write_all(trace.header().as_bytes())?;
    if !opt.quiet { println!("\n{INSTRUCTIONS}\n"); }
    println!("recording to {}", opt.out.display());

    // Enter on stdin stops (EOF does not: a non-interactive run stops on --seconds).
    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        std::thread::spawn(move || {
            let mut line = String::new();
            if let Ok(n) = std::io::stdin().read_line(&mut line) { if n > 0 { stop.store(true, Ordering::Relaxed); } }
        });
    }
    let mut plan = Plan::new(moby, camera);
    let mut sum = RecordSummary::default();
    let t0 = Instant::now();
    let mut last: Option<u32> = None;
    let mut last_print = Instant::now();
    loop {
        if stop.load(Ordering::Relaxed) || opt.seconds.is_some_and(|s| t0.elapsed().as_secs_f64() >= s) { break; }
        let words = p.read_many(&plan.addrs)?;
        sum.polls += 1;
        let (snap, c0, c1) = plan.split(&words);
        if c0 != c1 { sum.torn += 1; continue; }
        if last == Some(c0) {
            std::thread::sleep(Duration::from_micros(opt.poll_us));
            continue;
        }
        // A new tick. Level or moby changed: relocate and poll again.
        let lv = snap.get(hero_trace::LEVEL, 4).map(|b| i32::from_le_bytes(b.try_into().unwrap())).unwrap_or(level);
        let mb = snap.get(MOBY_PTR, 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).unwrap_or(moby);
        if lv != level || mb != moby {
            (level, moby, camera) = locate(&mut p)?;
            sum.level_changes += 1;
            println!("level {level}, moby {moby:#x}, camera {:?} (changed at tick {c0})", camera.map(|c| format!("{c:#x}")));
            plan = Plan::new(moby, camera);
            continue;
        }
        let mut s = sample_from(&snap, moby, camera)?;
        s.t_us = t0.elapsed().as_micros() as u64;
        if s.pad != s.lib_pad { sum.pad_vs_libpad += 1; }
        if let Some(l) = last {
            let d = c0.wrapping_sub(l);
            if d > 1 && d < 100_000 {
                sum.missed += (d - 1) as u64;
                sum.gaps.push((l, c0));
            } else if d >= 100_000 {
                sum.counter_resets += 1;
            }
        }
        last = Some(c0);
        sum.first_tick.get_or_insert(c0);
        sum.last_tick = Some(c0);
        w.write_all(s.row().as_bytes())?;
        w.write_all(b"\n")?;
        sum.rows += 1;
        trace.samples.push(s);
        if sum.rows.is_multiple_of(60) { w.flush()?; }
        if !opt.quiet && last_print.elapsed() >= Duration::from_secs(1) {
            last_print = Instant::now();
            let x = trace.samples.last().unwrap();
            println!("  {:6.1} s  tick {c0}  rows {}  missed {}  state {:#x}  pos ({:.2}, {:.2}, {:.2})", t0.elapsed().as_secs_f64(), sum.rows, sum.missed, x.state, x.pos_x, x.pos_y, x.pos_z);
        }
    }
    w.flush()?;
    sum.seconds = t0.elapsed().as_secs_f64();
    print_summary(&sum, &trace, &opt.out);
    Ok(sum)
}

pub fn print_summary(sum: &RecordSummary, trace: &Trace, out: &Path) {
    println!("\n== recording summary");
    println!("  file            {}", out.display());
    println!("  rows            {} in {:.1} s ({} polls, {:.0} polls/s, {} torn)", sum.rows, sum.seconds, sum.polls, sum.polls as f64 / sum.seconds.max(1e-9), sum.torn);
    if let (Some(a), Some(b)) = (sum.first_tick, sum.last_tick) { println!("  ticks           {a}..={b} ({} ticks)", b.wrapping_sub(a) + 1); }
    println!("  missed ticks    {}{}", sum.missed, if sum.gaps.is_empty() { String::new() } else { format!(" in {} gaps: {:?}", sum.gaps.len(), &sum.gaps[..sum.gaps.len().min(10)]) });
    if sum.counter_resets > 0 { println!("  counter resets  {} (level load / death)", sum.counter_resets); }
    println!("  pad rebuilt != libpad2 buffer: {} rows (diagnostic; the buffer can already hold the next frame)", sum.pad_vs_libpad);
    let s = &trace.samples;
    if s.is_empty() { return; }
    let mut states: Vec<i32> = Vec::new();
    for x in s { if states.last() != Some(&x.state) { states.push(x.state); } }
    let moved = s.iter().map(|x| ((x.pos_x - s[0].pos_x).powi(2) + (x.pos_y - s[0].pos_y).powi(2)).sqrt()).fold(0.0f32, f32::max);
    let stick = s.iter().filter(|x| x.stick_mag > 0.0).count();
    println!("  level {}  heli-pack owned {}  back item {} (state {})", s[0].level, s[0].owned.0.get(2).is_some_and(|&b| b != 0), s[0].back_id, s[0].back_state);
    println!("  states          {}", states.iter().take(60).map(|v| format!("{v:#x}")).collect::<Vec<_>>().join(" "));
    println!("  stick used on {stick} rows; farthest from the start {moved:.2} units");
    let jumps = crate::hero_analysis::segment(s);
    let stats = crate::hero_analysis::jump_stats(s, &jumps);
    println!("  jumps           {}", stats.iter().map(|j| format!("{} ({} ticks, {:.2} u)", j.kind, j.airtime.map_or("-".into(), |a| a.to_string()), j.hdist.unwrap_or(0.0))).collect::<Vec<_>>().join(", "));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_brackets_the_counter_and_splits() {
        let plan = Plan::new(0x1ece640, Some(0x166ec0));
        assert_eq!(plan.addrs[0], 0x15f5c8);
        assert_eq!(*plan.addrs.last().unwrap(), 0x15f5c8);
        assert!(plan.addrs.iter().all(|a| a % 8 == 0));
        assert!(plan.addrs.len() < 200, "{} reads", plan.addrs.len());
        // A fake reply: every word = its address; counters 5 and 6.
        let mut words: Vec<[u8; 8]> = plan.addrs.iter().map(|&a| (a as u64 | (a as u64) << 32).to_le_bytes()).collect();
        words[0] = (5u64 << 32).to_le_bytes();
        let n = words.len();
        words[n - 1] = (6u64 << 32).to_le_bytes();
        let (snap, c0, c1) = plan.split(&words);
        assert_eq!((c0, c1), (5, 6));
        assert_eq!(snap.get(0x13f3d0, 4).unwrap(), &0x13f3d0u32.to_le_bytes());
        assert_eq!(snap.get(0x13f3d4, 4).unwrap(), &0x13f3d0u32.to_le_bytes(), "high half of the word at 0x13f3d0");
        assert_eq!(snap.get(0x1ece690, 4).unwrap(), &0x1ece690u32.to_le_bytes());
        assert!(snap.get(0x13f3f0, 4).is_none(), "not read");
        // Every field of a sample is readable from a poll.
        assert!(sample_from(&snap, 0x1ece640, Some(0x166ec0)).is_ok());
    }

    #[test]
    fn utc_stamp_shape() {
        let s = utc_stamp();
        assert_eq!(s.len(), 15);
        assert_eq!(&s[8..9], "-");
        assert!(s.starts_with("20"));
    }
}
