//! Hero trace analysis: the jumps of a trace (segmentation and per-jump numbers), the per-tick diff of two traces
//! (PCSX2 vs port) and the report text. Doc: docs/plan/hero_feel_pass.md.

use crate::hero_trace::{Sample, ANGLES, FIELDS};
use std::collections::HashMap;
use std::fmt::Write as _;

/// Air runs shorter than this (ticks) without a jump / glide state are not jumps (steps, slope bumps).
pub const MIN_AIR: u32 = 4;
/// Movement groups of the jump states (4) and the glide (5).
pub fn jump_group(g: i32) -> bool { g == 4 || g == 5 }

fn airborne(s: &Sample) -> bool { s.air_ticks > 0 }
fn hspeed(s: &Sample) -> f64 { ((s.disp_x as f64).powi(2) + (s.disp_y as f64).powi(2)).sqrt() * 60.0 }
fn hdist(a: &Sample, b: &Sample) -> f64 { ((a.pos_x as f64 - b.pos_x as f64).powi(2) + (a.pos_y as f64 - b.pos_y as f64).powi(2)).sqrt() }

/// One jump (or fall) of a trace, as sample indices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jump {
    /// The first sample of the jump states before the takeoff (the windup: crouch jump entries, the jump states'
    /// takeoff tick), or the takeoff when it has none.
    pub entry: usize,
    /// The first airborne sample (air ticks 0x13f65e > 0).
    pub takeoff: usize,
    /// The first grounded sample after it (`samples.len()` when the trace ends in the air).
    pub landing: usize,
    /// The distinct states from the entry to the landing, in order.
    pub states: Vec<i32>,
}

impl Jump {
    pub fn kind(&self) -> String { self.states.iter().map(|s| format!("{s:#x}")).collect::<Vec<_>>().join(">") }
}

/// The jumps of `s`: each maximal airborne run with its windup; runs shorter than [`MIN_AIR`] ticks count only
/// when they reach a jump / glide state.
pub fn segment(s: &[Sample]) -> Vec<Jump> {
    let mut out: Vec<Jump> = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if !airborne(&s[i]) || (i > 0 && airborne(&s[i - 1])) { i += 1; continue; }
        let takeoff = i;
        let landing = (i..s.len()).find(|&j| !airborne(&s[j])).unwrap_or(s.len());
        let floor = out.last().map_or(0, |j| j.landing);
        let mut entry = takeoff;
        while entry > floor && jump_group(s[entry - 1].group) { entry -= 1; }
        let air = s[landing.min(s.len() - 1)].tick.wrapping_sub(s[takeoff].tick);
        let any_jump = (entry..landing).any(|k| jump_group(s[k].group));
        if air >= MIN_AIR || any_jump || landing == s.len() {
            let mut states: Vec<i32> = Vec::new();
            for x in &s[entry..landing] { if states.last() != Some(&x.state) { states.push(x.state); } }
            out.push(Jump { entry, takeoff, landing, states });
        }
        i = landing;
    }
    out
}

/// The numbers of one jump (units: game units, seconds via 60 ticks/s; ticks are gameplay ticks).
#[derive(Clone, Debug, Default)]
pub struct JumpStats {
    pub kind: String,
    pub entry_tick: u32,
    pub takeoff_tick: u32,
    pub landing_tick: Option<u32>,
    pub windup: u32,
    pub takeoff_hspeed: f64,
    pub takeoff_vz: f64,
    pub apex_h: f64,
    pub apex_rel: u32,
    pub airtime: Option<u32>,
    pub hdist: Option<f64>,
    pub air_hspeed: Option<f64>,
    /// Mean second difference of z (u/s²) while rising / falling.
    pub g_up: Option<f64>,
    pub g_down: Option<f64>,
    pub land_state: Option<i32>,
    pub land_anim: Option<u8>,
    pub land_hspeed: Option<f64>,
    pub hspeed_5: Option<f64>,
    pub hspeed_10: Option<f64>,
    pub land_momentum: Option<f64>,
    pub land_t524: Option<i32>,
    pub land_lockout: Option<i32>,
    /// Landing → the next jump's entry / takeoff (ticks).
    pub to_next_entry: Option<u32>,
    pub to_next_takeoff: Option<u32>,
    /// The camera: rise (max z in the air and 15 ticks after − z at takeoff), the tick of that max relative to the
    /// hero's apex, the mean horizontal distance to the hero in the air.
    pub cam_rise: f64,
    pub cam_lag: i64,
    pub cam_dist: f64,
    /// (ticks after takeoff, horizontal speed u/s, height above the takeoff z) per airborne tick.
    pub curve: Vec<(u32, f64, f64)>,
}

fn mean(v: &[f64]) -> Option<f64> { (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64) }

/// [`JumpStats`] of the jumps `j` of `s`.
pub fn jump_stats(s: &[Sample], jumps: &[Jump]) -> Vec<JumpStats> {
    let by_tick: HashMap<u32, usize> = s.iter().enumerate().map(|(i, x)| (x.tick, i)).collect();
    jumps
        .iter()
        .enumerate()
        .map(|(n, j)| {
            let t0 = &s[j.takeoff];
            let before = if j.takeoff > 0 { &s[j.takeoff - 1] } else { t0 };
            let land = s.get(j.landing);
            let air = &s[j.takeoff..j.landing];
            let (apex_i, apex) = air.iter().enumerate().fold((0, f64::MIN), |m, (k, x)| if x.pos_z as f64 > m.1 { (k, x.pos_z as f64) } else { m });
            let apex_tick = air[apex_i].tick;
            // Second differences over consecutive ticks, away from the takeoff / landing ticks.
            let mut up = Vec::new();
            let mut down = Vec::new();
            for k in 2..air.len().saturating_sub(1) {
                let (a, b, c) = (&air[k - 2], &air[k - 1], &air[k]);
                if b.tick.wrapping_sub(a.tick) != 1 || c.tick.wrapping_sub(b.tick) != 1 { continue; }
                let g = ((c.pos_z as f64 - b.pos_z as f64) - (b.pos_z as f64 - a.pos_z as f64)) * 3600.0;
                if c.tick <= apex_tick { up.push(g) } else { down.push(g) }
            }
            let after = |d: u32| land.and_then(|l| by_tick.get(&(l.tick + d))).map(|&i| hspeed(&s[i]));
            let next = jumps.get(n + 1);
            let cam_end = by_tick.get(&(land.map_or(s[s.len() - 1].tick, |l| l.tick) + 15)).copied().unwrap_or(j.landing.min(s.len() - 1));
            let cam = &s[j.takeoff..=cam_end.max(j.takeoff)];
            let (cam_i, cam_max) = cam.iter().enumerate().fold((0, f64::MIN), |m, (k, x)| if x.cam_z as f64 > m.1 { (k, x.cam_z as f64) } else { m });
            JumpStats {
                kind: j.kind(),
                entry_tick: s[j.entry].tick,
                takeoff_tick: t0.tick,
                landing_tick: land.map(|l| l.tick),
                windup: t0.tick.wrapping_sub(s[j.entry].tick),
                takeoff_hspeed: hspeed(t0),
                takeoff_vz: t0.disp_z as f64 * 60.0,
                apex_h: apex - before.pos_z as f64,
                apex_rel: apex_tick.wrapping_sub(t0.tick),
                airtime: land.map(|l| l.tick.wrapping_sub(t0.tick)),
                hdist: land.map(|l| hdist(before, l)),
                air_hspeed: land.map(|l| hdist(before, l) / (l.tick.wrapping_sub(before.tick)).max(1) as f64 * 60.0),
                g_up: mean(&up),
                g_down: mean(&down),
                land_state: land.map(|l| l.state),
                land_anim: land.map(|l| l.anim_seq),
                land_hspeed: land.map(hspeed),
                hspeed_5: after(5),
                hspeed_10: after(10),
                land_momentum: land.map(|l| ((l.mom_x as f64).powi(2) + (l.mom_y as f64).powi(2)).sqrt() * 60.0),
                land_t524: land.map(|l| l.t_524),
                land_lockout: land.map(|l| l.t_lockout),
                to_next_entry: land.zip(next).map(|(l, nx)| s[nx.entry].tick.wrapping_sub(l.tick)),
                to_next_takeoff: land.zip(next).map(|(l, nx)| s[nx.takeoff].tick.wrapping_sub(l.tick)),
                cam_rise: cam_max - t0.cam_z as f64,
                cam_lag: cam[cam_i].tick as i64 - apex_tick as i64,
                cam_dist: mean(&air.iter().map(|x| ((x.cam_x as f64 - x.pos_x as f64).powi(2) + (x.cam_y as f64 - x.pos_y as f64).powi(2)).sqrt()).collect::<Vec<_>>()).unwrap_or(0.0),
                curve: air.iter().map(|x| (x.tick.wrapping_sub(t0.tick), hspeed(x), x.pos_z as f64 - before.pos_z as f64)).collect(),
            }
        })
        .collect()
}

impl JumpStats {
    /// (metric, value) rows for the tables; `None` = not available.
    pub fn rows(&self) -> Vec<(&'static str, Option<f64>)> {
        let u = |v: u32| Some(v as f64);
        vec![
            ("entry tick (jump state)", u(self.entry_tick)),
            ("takeoff tick", u(self.takeoff_tick)),
            ("windup ticks", u(self.windup)),
            ("takeoff h speed u/s", Some(self.takeoff_hspeed)),
            ("takeoff vz u/s", Some(self.takeoff_vz)),
            ("apex height", Some(self.apex_h)),
            ("apex tick (after takeoff)", u(self.apex_rel)),
            ("airtime ticks", self.airtime.map(|v| v as f64)),
            ("h distance", self.hdist),
            ("mean air h speed u/s", self.air_hspeed),
            ("gravity rising u/s^2", self.g_up),
            ("gravity falling u/s^2", self.g_down),
            ("landing tick", self.landing_tick.map(|v| v as f64)),
            ("landing state", self.land_state.map(|v| v as f64)),
            ("landing anim seq", self.land_anim.map(|v| v as f64)),
            ("h speed at landing u/s", self.land_hspeed),
            ("h speed landing+5", self.hspeed_5),
            ("h speed landing+10", self.hspeed_10),
            ("momentum at landing u/s", self.land_momentum),
            ("timer 0x13f524 at landing", self.land_t524.map(|v| v as f64)),
            ("lockout 0x13f514 at landing", self.land_lockout.map(|v| v as f64)),
            ("landing -> next entry", self.to_next_entry.map(|v| v as f64)),
            ("landing -> next takeoff", self.to_next_takeoff.map(|v| v as f64)),
            ("camera rise", Some(self.cam_rise)),
            ("camera max - hero apex ticks", Some(self.cam_lag as f64)),
            ("camera mean h distance", Some(self.cam_dist)),
        ]
    }
}

fn fmt_v(v: Option<f64>) -> String {
    match v {
        None => "-".into(),
        Some(x) if x.fract() == 0.0 && x.abs() < 1e7 => format!("{x:.0}"),
        Some(x) if x.abs() >= 0.1 || x == 0.0 => format!("{x:.3}"),
        Some(x) => format!("{x:.5}"),
    }
}

/// The per-jump table of one trace.
pub fn jumps_table(stats: &[JumpStats]) -> String {
    let mut o = String::new();
    for (n, j) in stats.iter().enumerate() {
        let _ = writeln!(o, "jump {} ({})", n + 1, j.kind);
        for (name, v) in j.rows() { let _ = writeln!(o, "  {name:<30} {:>12}", fmt_v(v)); }
    }
    if stats.is_empty() { o += "no jumps\n"; }
    o
}

/// The per-jump comparison (k-th jump against k-th jump).
pub fn jumps_compare(a: &[JumpStats], b: &[JumpStats], names: (&str, &str)) -> String {
    let mut o = String::new();
    for n in 0..a.len().max(b.len()) {
        let (ja, jb) = (a.get(n), b.get(n));
        let _ = writeln!(o, "jump {}: {} {} | {} {}", n + 1, names.0, ja.map_or("-".into(), |j| j.kind.clone()), names.1, jb.map_or("-".into(), |j| j.kind.clone()));
        let _ = writeln!(o, "  {:<30} {:>12} {:>12} {:>10}", "", names.0, names.1, "diff");
        let ra = ja.map(|j| j.rows()).unwrap_or_default();
        let rb = jb.map(|j| j.rows()).unwrap_or_default();
        let rows = if ra.is_empty() { &rb } else { &ra };
        for (k, (name, _)) in rows.iter().enumerate() {
            let va = ra.get(k).and_then(|r| r.1);
            let vb = rb.get(k).and_then(|r| r.1);
            let d = va.zip(vb).map(|(x, y)| y - x);
            let _ = writeln!(o, "  {name:<30} {:>12} {:>12} {:>10}", fmt_v(va), fmt_v(vb), d.map_or("".into(), |d| if d == 0.0 { "".into() } else { fmt_v(Some(d)) }));
        }
    }
    if a.is_empty() && b.is_empty() { o += "no jumps in either trace\n"; }
    o
}

/// The horizontal speed and height curves of the k-th jumps side by side (every tick).
pub fn curves_compare(a: &[JumpStats], b: &[JumpStats], names: (&str, &str)) -> String {
    let mut o = String::new();
    for n in 0..a.len().max(b.len()) {
        let _ = writeln!(o, "jump {} curves: tick after takeoff | {} h speed, height | {} h speed, height", n + 1, names.0, names.1);
        let ca = a.get(n).map(|j| j.curve.as_slice()).unwrap_or(&[]);
        let cb = b.get(n).map(|j| j.curve.as_slice()).unwrap_or(&[]);
        let ma: HashMap<u32, (f64, f64)> = ca.iter().map(|c| (c.0, (c.1, c.2))).collect();
        let mb: HashMap<u32, (f64, f64)> = cb.iter().map(|c| (c.0, (c.1, c.2))).collect();
        let last = ca.iter().chain(cb).map(|c| c.0).max().unwrap_or(0);
        for t in 0..=last {
            let f = |m: &HashMap<u32, (f64, f64)>| m.get(&t).map_or("        -        -".into(), |(h, z)| format!("{h:9.3}{z:9.3}"));
            let _ = writeln!(o, "  {t:4} | {} | {}", f(&ma), f(&mb));
        }
    }
    o
}

// ------------------------------------------------------------------------------------------------
// The per-tick diff.

/// Columns left out of the diff: host time, the tick itself, the inputs (identical by construction), the PCSX2-only
/// raw blocks.
pub const NOT_DIFFED: &[&str] = &["t_us", "tick", "pad", "lib_pad", "pad_held", "pad_pressed", "pad_held_u", "jump_raw", "mirror"];

/// One column's error over the paired ticks.
#[derive(Clone, Debug, Default)]
pub struct FieldDiff {
    pub name: &'static str,
    pub n: usize,
    pub max_err: f64,
    pub max_tick: u32,
    pub sum_err: f64,
    /// Ticks beyond the tolerance.
    pub over: usize,
    /// First tick beyond the tolerance: (tick, pcsx2 value, port value).
    pub first: Option<(u32, f64, f64)>,
    /// Byte blocks: ticks that differ.
    pub hex: bool,
}

impl FieldDiff {
    pub fn mean(&self) -> f64 { if self.n == 0 { 0.0 } else { self.sum_err / self.n as f64 } }
}

/// The diff of two traces paired by tick.
#[derive(Clone, Debug, Default)]
pub struct Diff {
    pub paired: usize,
    pub only_a: usize,
    pub only_b: usize,
    pub fields: Vec<FieldDiff>,
    /// Earliest divergence over the hero columns (not the camera): (tick, field, a, b).
    pub first: Option<(u32, &'static str, f64, f64)>,
    pub first_cam: Option<(u32, &'static str, f64, f64)>,
}

fn wrap(d: f64) -> f64 {
    let t = std::f64::consts::TAU;
    let d = d.rem_euclid(t);
    if d > std::f64::consts::PI { t - d } else { d }
}

/// Per-tick diff of `a` (reference) and `b` with float tolerance `tol` (integers must be equal).
pub fn diff(a: &[Sample], b: &[Sample], tol: f64) -> Diff {
    let bi: HashMap<u32, usize> = b.iter().enumerate().map(|(i, s)| (s.tick, i)).collect();
    let pairs: Vec<(&Sample, &Sample)> = a.iter().filter_map(|s| bi.get(&s.tick).map(|&j| (s, &b[j]))).collect();
    let mut d = Diff { paired: pairs.len(), only_a: a.len() - pairs.len(), only_b: b.len() - pairs.len(), ..Default::default() };
    for f in FIELDS.iter().filter(|f| !NOT_DIFFED.contains(&f.name)) {
        let is_float = a.first().or(b.first()).is_some_and(|s| is_float_field(s, f.name));
        let mut fd = FieldDiff { name: f.name, ..Default::default() };
        for (x, y) in &pairs {
            let (va, vb) = match (x.num(f.name), y.num(f.name)) {
                (Some(va), Some(vb)) => (va, vb),
                _ => {
                    // Byte blocks: equality.
                    fd.hex = true;
                    let (ra, rb) = (x.row_cell(f.name), y.row_cell(f.name));
                    let e = if ra == rb { 0.0 } else { 1.0 };
                    (0.0, e)
                }
            };
            let err = if ANGLES.contains(&f.name) { wrap(vb - va) } else if va.is_nan() && vb.is_nan() { 0.0 } else { (vb - va).abs() };
            let err = if err.is_nan() { f64::INFINITY } else { err };
            fd.n += 1;
            fd.sum_err += if err.is_finite() { err } else { 0.0 };
            if err > fd.max_err || fd.n == 1 { fd.max_err = err; fd.max_tick = x.tick; }
            let over = if is_float && !fd.hex { err > tol } else { err > 0.0 };
            if over {
                fd.over += 1;
                if fd.first.is_none() { fd.first = Some((x.tick, va, vb)); }
            }
        }
        if let Some((t, va, vb)) = fd.first {
            let slot = if f.name.starts_with("cam_") { &mut d.first_cam } else { &mut d.first };
            if slot.is_none_or(|s| t < s.0) { *slot = Some((t, f.name, va, vb)); }
        }
        d.fields.push(fd);
    }
    d
}

fn is_float_field(s: &Sample, name: &str) -> bool {
    // A float column prints with a '.' or an exponent ({:?}); integers never do.
    let c = s.row_cell(name);
    c.contains('.') || c.contains('e') || c.contains("NaN") || c.contains("inf")
}

impl Sample {
    /// Column `name` as its text.
    pub fn row_cell(&self, name: &str) -> String {
        let k = FIELDS.iter().position(|f| f.name == name);
        k.and_then(|k| self.row().split('\t').nth(k).map(String::from)).unwrap_or_default()
    }
}

/// The diff as text: the first divergence and the per-field table.
pub fn diff_text(d: &Diff, names: (&str, &str)) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "paired ticks {} (only in {}: {}, only in {}: {})", d.paired, names.0, d.only_a, names.1, d.only_b);
    match d.first {
        Some((t, f, a, b)) => { let _ = writeln!(o, "first divergence (hero): tick {t}, {f}: {} {a} vs {} {b}", names.0, names.1); }
        None => { let _ = writeln!(o, "first divergence (hero): none within tolerance"); }
    }
    if let Some((t, f, a, b)) = d.first_cam { let _ = writeln!(o, "first divergence (camera): tick {t}, {f}: {a} vs {b}"); }
    let _ = writeln!(o, "  {:<14} {:>12} {:>12} {:>10} {:>8} {:>10}  first beyond tolerance ({} -> {})", "field", "max err", "mean err", "at tick", "over", "first", names.0, names.1);
    for f in &d.fields {
        let first = f.first.map_or(String::new(), |(t, a, b)| format!("{t}: {a} -> {b}"));
        let _ = writeln!(o, "  {:<14} {:>12.6} {:>12.6} {:>10} {:>8} {:>10}  {}", f.name, f.max_err, f.mean(), f.max_tick, f.over, f.first.map_or("-".into(), |x| x.0.to_string()), first);
    }
    o
}

/// The state sequences side by side: one line at each tick where either side's state changes (`max` lines).
pub fn states_side_by_side(a: &[Sample], b: &[Sample], names: (&str, &str), max: usize) -> String {
    let bi: HashMap<u32, usize> = b.iter().enumerate().map(|(i, s)| (s.tick, i)).collect();
    let mut o = format!("  {:>8}  {:<16} {:<16}\n", "tick", format!("{} state/grp", names.0), format!("{} state/grp", names.1));
    let mut last: Option<(i32, Option<i32>)> = None;
    let mut n = 0;
    for s in a {
        let p = bi.get(&s.tick).map(|&j| &b[j]);
        let key = (s.state, p.map(|p| p.state));
        if last == Some(key) { continue; }
        last = Some(key);
        n += 1;
        if n > max { o += "  …\n"; break; }
        let mark = if p.is_some_and(|p| p.state != s.state) { "  <>" } else { "" };
        let _ = writeln!(o, "  {:>8}  {:<16} {:<16}{mark}", s.tick, format!("{:#x}/{}", s.state, s.group), p.map_or("-".into(), |p| format!("{:#x}/{}", p.state, p.group)));
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic trace: standing, then a jump every `period` ticks with windup `w`, airtime `air`, parabolic arc of
    /// height `h`, moving at `v` u/tick in x.
    fn synth(jumps: usize, period: u32, w: u32, air: u32, h: f32, v: f32) -> Vec<Sample> {
        let mut out = Vec::new();
        let mut x = 0.0f32;
        let total = 20 + jumps as u32 * period + 30;
        for t in 0..total {
            let mut s = Sample { tick: 1000 + t, ..Default::default() };
            let rel = t as i64 - 20;
            let (k, ph) = if rel >= 0 { ((rel / period as i64) as usize, (rel % period as i64) as u32) } else { (usize::MAX, 0) };
            let in_jump = k < jumps;
            let (mut z, mut air_t, mut state, mut group) = (0.0f32, 0i16, 0, 0);
            if in_jump && ph < w { state = 0xa; group = 4; }
            if in_jump && ph >= w && ph < w + air {
                let u = (ph - w + 1) as f32 / air as f32;
                z = 4.0 * h * u * (1.0 - u);
                air_t = (ph - w + 1) as i16;
                state = 0xa;
                group = 4;
            }
            if in_jump && ph == w + air { state = 3; group = 1; }
            let moving = in_jump && ph >= w && ph < w + air;
            let px = x;
            if moving { x += v; }
            s.pos_x = x;
            s.disp_x = x - px;
            s.pos_z = z;
            s.air_ticks = air_t;
            s.state = state;
            s.group = group;
            s.cam_z = 2.0 + z * 0.5;
            s.cam_x = x - 5.0;
            out.push(s);
        }
        // disp_z
        for i in 1..out.len() { out[i].disp_z = out[i].pos_z - out[i - 1].pos_z; }
        out
    }

    #[test]
    fn segments_chained_jumps() {
        let s = synth(5, 40, 3, 30, 2.0, 0.2);
        let j = segment(&s);
        assert_eq!(j.len(), 5);
        for (n, jj) in j.iter().enumerate() {
            assert_eq!(s[jj.takeoff].tick, 1000 + 20 + n as u32 * 40 + 3);
            assert_eq!(jj.takeoff - jj.entry, 3, "windup");
            assert_eq!(jj.landing - jj.takeoff, 30);
            assert_eq!(jj.states, vec![0xa]);
        }
        let st = jump_stats(&s, &j);
        assert_eq!(st[0].airtime, Some(30));
        assert_eq!(st[0].windup, 3);
        assert!((st[0].hdist.unwrap() - 6.0).abs() < 1e-3, "{:?}", st[0].hdist);
        assert!((st[0].takeoff_hspeed - 12.0).abs() < 1e-3);
        assert!((st[0].apex_h - 2.0).abs() < 0.01, "{}", st[0].apex_h);
        assert_eq!(st[0].apex_rel, 14);
        // z = 4h·u(1−u) with u = n/30: second difference −8h/900 per tick², ×3600.
        let g = -8.0 * 2.0 / 900.0 * 3600.0;
        assert!((st[0].g_up.unwrap() - g).abs() < 0.05, "{:?}", st[0].g_up);
        assert!((st[0].g_down.unwrap() - g).abs() < 0.05);
        assert_eq!(st[0].land_state, Some(3));
        assert_eq!(st[0].to_next_takeoff, Some(40 - 30));
        assert_eq!(st[0].to_next_entry, Some(40 - 30 - 3));
        assert_eq!(st[4].to_next_takeoff, None);
        // The camera (z = 2 + hero z / 2) peaks at the hero's apex: 3.0 against its z at the takeoff tick.
        assert!((st[0].cam_rise - (3.0 - s[j[0].takeoff].cam_z as f64)).abs() < 0.01, "{}", st[0].cam_rise);
        assert_eq!(st[0].cam_lag, 0);
        assert_eq!(st[0].curve.len(), 30);
        assert!(jumps_table(&st).contains("jump 5 (0xa)"));
    }

    #[test]
    fn ignores_short_drops_and_handles_trace_ending_in_air() {
        let mut s = synth(0, 40, 0, 0, 0.0, 0.0);
        for (k, x) in s[10..12].iter_mut().enumerate() { x.air_ticks = k as i16 + 1; } // a 2-tick step down: not a jump
        let n = s.len();
        for x in &mut s[n - 5..n] { x.air_ticks = 1; x.state = 6; x.group = 2; }
        let j = segment(&s);
        assert_eq!(j.len(), 1);
        assert_eq!(j[0].landing, n);
        let st = jump_stats(&s, &j);
        assert_eq!(st[0].airtime, None);
        assert_eq!(st[0].kind, "0x6");
    }

    #[test]
    fn diff_reports_first_divergence_and_errors() {
        let a = synth(2, 40, 3, 30, 2.0, 0.2);
        let mut b = a.clone();
        // Port: a lower jump from tick 1030 on, and one state off at 1060.
        for s in b.iter_mut().filter(|s| s.tick >= 1030) { s.pos_z *= 0.9; }
        let i = b.iter().position(|s| s.tick == 1060).unwrap();
        b[i].state = 6;
        b.pop(); // one tick only in a
        let d = diff(&a, &b, 1e-4);
        assert_eq!(d.paired, a.len() - 1);
        assert_eq!(d.only_a, 1);
        let (t, f, _, _) = d.first.unwrap();
        assert_eq!((t, f), (1030, "pos_z"));
        let st = d.fields.iter().find(|f| f.name == "state").unwrap();
        assert_eq!((st.over, st.first.map(|x| x.0)), (1, Some(1060)));
        let pz = d.fields.iter().find(|f| f.name == "pos_z").unwrap();
        assert!((pz.max_err - 0.2).abs() < 0.01, "{}", pz.max_err);
        assert!(pz.mean() > 0.0 && pz.mean() < pz.max_err);
        assert!(d.fields.iter().all(|f| f.name != "tick" && f.name != "pad"));
        // Identical traces: no divergence.
        let d = diff(&a, &a, 0.0);
        assert!(d.first.is_none() && d.fields.iter().all(|f| f.over == 0));
        let txt = states_side_by_side(&a, &b, ("pcsx2", "port"), 100);
        assert!(txt.contains("<>"));
        assert!(diff_text(&d, ("pcsx2", "port")).contains("none within tolerance"));
    }

    #[test]
    fn angle_errors_wrap() {
        let a = vec![Sample { tick: 1, yaw: 3.1, ..Default::default() }];
        let b = vec![Sample { tick: 1, yaw: -3.1, ..Default::default() }];
        let d = diff(&a, &b, 0.1);
        let y = d.fields.iter().find(|f| f.name == "yaw").unwrap();
        assert!(y.max_err < 0.1, "{}", y.max_err);
    }
}
