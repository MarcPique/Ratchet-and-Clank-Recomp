//! `overlay-diff`: the masked function × level diff (G-TOOL-008 X1; docs/workflows/ghidra.md "Masked overlay
//! diff"). For a set of reference functions (level 01, or level 00 for the superset code level 01 lacks) it finds
//! each function's counterpart in every level overlay 00–18 and compares the two after masking the link-dependent
//! fields, then prints only the instructions that differ.
//!
//! **Masking** is the port's code identity ([`rc_formats::level_overlay::mask`]: `j`/`jal` targets, `%hi`/`%lo`
//! pairs of main-RAM addresses, `$gp` offsets), with branch offsets and `$sp` offsets also masked for the alignment
//! (a branch across an inserted instruction, or another stack-frame layout, is not a difference by itself). The
//! masked words are aligned (longest common subsequence), and every aligned pair is then checked for what the mask
//! hid:
//! * a `jal` / `j` into the boot ELF must name the same function; one into the overlay must reach the same code
//!   (the callee's own row, or [`Relocation::same_code`]) — else the cell is `c` (only a callee differs);
//! * a branch or in-function jump must land on the aligned instruction;
//! * a boot-ELF data address (below the overlay base 0x15ef00: the hero block, the level number 0x15ed84, …) must be
//!   the same address; an overlay data load (`.lit` constants, `.data` tables) must read the same bytes, a word
//!   that is a main-RAM address on both sides counting as the same (relocated) pointer (a difference is `k`: same
//!   code, other constants); a switch's jump table must send every case to the aligned instruction.
//!
//! Unaligned runs are the hunks (`D`). A function with a switch of 16 or more cases on both sides is aligned case
//! by case (the code before the switch, the default bodies, and each reference body with the level body its cases
//! go to); bodies only one side has are the switch's case-set difference (`S` when that is all).
//!
//! **The superset check.** Level 00 is the superset build (hero_states.md §0): a level's differing words that level
//! 00's copy has too are that build's code (`s`). A level word is the level's own only when it is unaligned against
//! level 01 and level 00 register-blind (register fields cleared: a build with fewer cases allocates registers
//! differently) and no run of three register-blind words around it occurs anywhere in either copy (a build orders a
//! state chain's blocks differently). Runs of one or two words whose instructions the copies have elsewhere
//! (scheduling), an `addiu` whose `%lo` lost its `lui` at a register join, and the switch's own bound check are
//! dropped. What remains is `U`, printed as "level code neither level 01 nor level 00 has". A row with no level-00
//! copy stays `D`.
//!
//! **Counterparts.** An exact masked copy ([`Relocation::copies`], compared over the relocator's extent); else the
//! call sites: a caller whose own counterpart is known calls it at the aligned `jal` (two such sites, or one and a
//! similar body); else the callees: the function that calls the counterparts of its callees (rare callees weigh
//! more), the clearly most similar of those; else the most similar function (shared runs of four masked words),
//! only when clearly best and (under 0.5) sharing a callee. A candidate that is an exact copy of another reference
//! function is never taken. The method is recorded per cell (`copy`, `call site`, `callees`, `similar x.xx`).
//!
//! Outputs (`work/overlay_diff/` by default): `<name>.tsv` (function × level cells), `<name>_matrix.txt` (one
//! character per cell), `<name>_diffs.txt` (per differing cell only the differing instructions, disassembled, with
//! two aligned instructions of context).

use crate::class_census::{names, Edge, Graph};
use crate::mips;
use anyhow::{Context, Result};
use rc_formats::level_overlay::{address_refs, mask, LevelOverlay, Relocation, TEXT_SECTION};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Overlay code and data start here; below is the boot ELF (the same on every level).
const OVERLAY_BASE: u32 = 0x15ef00;
/// The level number (`Ram0015ed84`, boot ELF).
const LEVEL_NUMBER: u32 = 0x15ed84;
/// Longest function compared (bytes): above the relocator's `MAX_EXTENT`, since the superset builds' state
/// switches are larger (level 00's per-state physics is over 32 KiB).
const EXTENT: u32 = 0x2_0000;
/// The references a row can take: level 01 (the port's build) and level 00 (the superset build).
const REFS: [u32; 2] = [1, 0];

pub struct Options {
    /// Source directories whose cited addresses are the rows (`0x…` tokens that are function starts in level 01;
    /// on a line mentioning `L00`, also level-00 starts).
    pub cite: Vec<PathBuf>,
    /// Extra rows: (reference level, address).
    pub fns: Vec<(u32, u32)>,
    /// Also add the overlay callees of the rows, this many calls deep.
    pub callee_depth: u32,
    pub name: String,
    pub out: PathBuf,
    /// Instructions printed per side of a hunk (the rest is counted).
    pub max_lines: usize,
}

pub struct Summary {
    pub rows: usize,
    /// Cells per status character.
    pub counts: BTreeMap<char, usize>,
    /// Rows with an own difference (`D` or `k`) on some level.
    pub own_diff_rows: usize,
    pub out: PathBuf,
}

struct Level {
    ov: LevelOverlay,
    graph: Graph,
    names: HashMap<u32, String>,
}

impl Level {
    fn in_text(&self, a: u32) -> bool { self.ov.sections.get(TEXT_SECTION).is_some_and(|t| a >= t.dest && a < t.dest + t.data.len() as u32) }

    /// The function's words up to the next start with evidence of its own ([`Graph::strong`]; a code address
    /// only formed with `lui`/`%lo` does not end a function here), at most [`EXTENT`] bytes.
    fn words(&self, f: u32) -> Option<&[u32]> {
        let t = self.ov.sections.get(TEXT_SECTION)?;
        let end = t.dest + t.data.len() as u32;
        if f < t.dest || f >= end { return None; }
        let next = self.graph.strong.range(f + 4..).next().copied().unwrap_or(end).min(end).min(f + EXTENT);
        self.ov.code(f, ((next - f) / 4) as usize)
    }
}

#[derive(Clone)]
struct Row {
    reference: u32,
    addr: u32,
    name: String,
    source: &'static str,
}

#[derive(Default, Clone)]
struct Cell {
    counterpart: Option<u32>,
    how: String,
    status: char,
    removed: usize,
    added: usize,
    hunks: Vec<String>,
    /// Constant (data) differences: `k`.
    consts: Vec<String>,
    /// A switch's case-set difference: bodies only one side has.
    case_only: Vec<String>,
    /// Other own differences found on aligned instructions (calls, branches, boot addresses): `D`.
    other: Vec<String>,
    /// (reference callee, level callee) of every aligned overlay call, resolved into `callee_diffs`.
    calls: Vec<(u32, u32)>,
    callee_diffs: Vec<String>,
    /// Reference word index → level word index.
    map: Vec<Option<usize>>,
    /// Level word indices that differ (unaligned, or in a body only the level has).
    lvl_bad: BTreeSet<usize>,
    /// Level word indices a check on an aligned pair flagged (a constant, a boot address, a branch).
    noted: BTreeSet<usize>,
    /// The level's code that neither level 01 nor level 00 (the superset) has: runs, disassembled.
    unique: Vec<String>,
}

fn op(w: u32) -> u32 { w >> 26 }
fn is_branch(w: u32) -> bool { matches!(op(w), 1 | 4..=7 | 0x14..=0x17) || (op(w) == 0x11 && (w >> 21) & 31 == 8) }

/// Masked words with branch offsets and stack-frame offsets also masked (the alignment key): the frame layout
/// (its size, where each saved register and local sits) follows from the other code the level's build compiled
/// into the function, and is not behaviour by itself.
fn norm(words: &[u32]) -> Vec<u32> {
    mask(words).into_iter().zip(words).map(|(m, &w)| if is_branch(w) || is_sp_offset(w) { m & 0xffff_0000 } else { m }).collect()
}

/// [`norm`] with the register fields also cleared (the superset check): a level build that compiled less code
/// into a function allocates and schedules registers differently; the opcodes, immediates and constants stay.
fn norm_loose(words: &[u32]) -> Vec<u32> {
    norm(words)
        .into_iter()
        .map(|w| match op(w) {
            0 | 0x1c => w & !0x03ff_f800,
            0x11 => w & 0xffe0_003f,
            2 | 3 => w,
            _ => w & 0xfc00_ffff,
        })
        .collect()
}

/// A load, store or `addiu` / `daddiu` off `$sp`.
fn is_sp_offset(w: u32) -> bool { (w >> 21) & 31 == 29 && (matches!(op(w), 9 | 0x19 | 0x1a..=0x1f | 0x31 | 0x36 | 0x37 | 0x39 | 0x3e | 0x3f) || (0x20..=0x2e).contains(&op(w))) }

/// Longest-common-subsequence alignment of `a` and `b` (common prefix and suffix first).
fn align(a: &[u32], b: &[u32]) -> Vec<(usize, usize)> {
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] { pre += 1; }
    let mut suf = 0;
    while suf < a.len() - pre && suf < b.len() - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf] { suf += 1; }
    let (x, y) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    let (n, m) = (x.len(), y.len());
    let mut out: Vec<(usize, usize)> = (0..pre).map(|i| (i, i)).collect();
    if n > 0 && m > 0 && (n + 1) * (m + 1) <= 64 << 20 {
        let w = m + 1;
        let mut t = vec![0u16; (n + 1) * w];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                t[i * w + j] = if x[i] == y[j] { t[(i + 1) * w + j + 1] + 1 } else { t[(i + 1) * w + j].max(t[i * w + j + 1]) };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n && j < m {
            if x[i] == y[j] {
                out.push((pre + i, pre + j));
                i += 1;
                j += 1;
            } else if t[(i + 1) * w + j] >= t[i * w + j + 1] {
                i += 1;
            } else {
                j += 1;
            }
        }
    }
    out.extend((0..suf).map(|k| (a.len() - suf + k, b.len() - suf + k)));
    out
}

/// Bytes a load reads (None: not a load; stores and address formation compare no bytes).
fn load_width(w: u32) -> Option<usize> {
    match op(w) {
        0x20 | 0x24 => Some(1),
        0x21 | 0x25 => Some(2),
        0x23 | 0x27 | 0x31 => Some(4),
        0x37 => Some(8),
        0x1e | 0x36 => Some(16),
        _ => None,
    }
}

/// The jump tables `f` (at `base`, `n` words) forms: word index of the instruction that forms the table address →
/// the case targets (consecutive words that are addresses inside the function).
fn jump_tables(ov: &LevelOverlay, base: u32, words: &[u32]) -> HashMap<usize, Vec<u32>> {
    let end = base + 4 * words.len() as u32;
    let mut out = HashMap::new();
    for (i, a) in address_refs(words) {
        if a < OVERLAY_BASE || (a >= base && a < end) || a % 4 != 0 { continue; }
        let mut t = Vec::new();
        while let Some(v) = ov.u32(a + 4 * t.len() as u32) {
            if v < base || v >= end || v % 4 != 0 { break; }
            t.push(v);
        }
        if t.len() >= 2 { out.insert(i, t); }
    }
    out
}

/// Equal data, with words that are main-RAM addresses on both sides taken as the same (relocated) pointer.
fn same_data(p: &[u8], q: &[u8]) -> bool {
    let ptr = |w: u32| (0x10_0000..0x200_0000).contains(&w);
    p.len() == q.len()
        && p.chunks(4).zip(q.chunks(4)).all(|(x, y)| {
            if x.len() == 4 {
                let (a, b) = (u32::from_le_bytes(x.try_into().unwrap()), u32::from_le_bytes(y.try_into().unwrap()));
                a == b || (ptr(a) && ptr(b))
            } else {
                x == y
            }
        })
}

fn show_bytes(b: &[u8]) -> String {
    if b.len() == 4 {
        let v = u32::from_le_bytes(b.try_into().unwrap());
        let f = f32::from_bits(v);
        if f.is_finite() && f != 0.0 && (1e-6..1e7).contains(&f.abs()) { return format!("{v:08x} ({f})"); }
        return format!("{v:08x}");
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Rows from the addresses cited in the `.rs` files under `dir`.
fn cited(dir: &Path, levels: &HashMap<u32, Level>, rows: &mut BTreeMap<(u32, u32), Row>) {
    fn walk(d: &Path, files: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() { walk(&p, files); } else if p.extension().is_some_and(|x| x == "rs") { files.push(p); }
        }
    }
    let mut files = Vec::new();
    walk(dir, &mut files);
    files.sort();
    for p in files {
        let Ok(s) = std::fs::read_to_string(&p) else { continue };
        for line in s.lines() {
            let l = line.to_ascii_lowercase();
            let l00 = line.contains("L00");
            let b = l.as_bytes();
            let mut i = 0;
            while i + 2 < b.len() {
                if b[i] == b'0' && b[i + 1] == b'x' && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
                    let mut j = i + 2;
                    while j < b.len() && b[j].is_ascii_hexdigit() { j += 1; }
                    if (5..=8).contains(&(j - i - 2)) && (j == b.len() || !b[j].is_ascii_alphanumeric()) {
                        if let Ok(v) = u32::from_str_radix(&l[i + 2..j], 16) {
                            if levels[&1].graph.strong.contains(&v) {
                                rows.entry((1, v)).or_insert(Row { reference: 1, addr: v, name: String::new(), source: "cited" });
                            } else if l00 && levels[&0].graph.strong.contains(&v) {
                                rows.entry((0, v)).or_insert(Row { reference: 0, addr: v, name: String::new(), source: "cited" });
                            }
                        }
                    }
                    i = j;
                    continue;
                }
                i += 1;
            }
        }
    }
}

/// The level word index `k` of the reference instruction `i`, through the alignment.
fn build_map(n_ref: usize, pairs: &[(usize, usize)]) -> Vec<Option<usize>> {
    let mut m = vec![None; n_ref];
    for &(i, j) in pairs { m[i] = Some(j); }
    m
}

/// One side of a hunk: at most `max` instructions of `range`, disassembled.
fn side_lines(h: &mut String, side: char, range: std::ops::Range<usize>, words: &[u32], base: u32, lv: &Level, max: usize) {
    let n = range.len();
    for k in range.take(max) {
        let pc = base + 4 * k as u32;
        let mut t = mips::disasm(words[k], pc);
        if op(words[k]) == 3 {
            if let Some(nm) = mips::target(words[k], pc).and_then(|x| lv.names.get(&x)) { let _ = write!(t, "  ; {nm}"); }
        }
        let _ = writeln!(h, "  {side} {pc:08x}  {t}");
    }
    if n > max { let _ = writeln!(h, "  {side} … {} more", n - max); }
}

/// The largest jump table (at least 16 cases) of a function: (forming word index, targets).
fn main_table(t: &HashMap<usize, Vec<u32>>) -> Option<(usize, &Vec<u32>)> {
    t.iter().filter(|(_, v)| v.len() >= 16).max_by_key(|(i, v)| (v.len(), usize::MAX - **i)).map(|(i, v)| (*i, v))
}

/// A switch's shape: the default target (the most common one, when it is shared by two or more cases), the
/// case bodies as word ranges (from each distinct target to the next, the last to the end) and each target's
/// cases.
struct Switch {
    default: Option<u32>,
    first: usize,
    ranges: BTreeMap<u32, std::ops::Range<usize>>,
    owners: BTreeMap<u32, Vec<usize>>,
    targets: Vec<u32>,
}

impl Switch {
    fn new(base: u32, len: usize, targets: &[u32]) -> Switch {
        let mut owners: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
        for (k, &t) in targets.iter().enumerate() { owners.entry(t).or_default().push(k); }
        let default = owners.iter().max_by_key(|(t, v)| (v.len(), u32::MAX - **t)).filter(|(_, v)| v.len() >= 2).map(|(t, _)| *t);
        let ts: Vec<u32> = owners.keys().copied().collect();
        let ix = |a: u32| ((a - base) / 4) as usize;
        let ranges = ts.iter().enumerate().map(|(n, &t)| (t, ix(t)..ts.get(n + 1).map_or(len, |&u| ix(u)))).collect();
        Switch { default, first: ts.first().map_or(len, |&t| ix(t)), ranges, owners, targets: targets.to_vec() }
    }
    fn handles(&self, k: usize) -> Option<u32> { self.targets.get(k).copied().filter(|&t| Some(t) != self.default) }
}

fn cases(ks: &[usize]) -> String { ks.iter().map(|k| format!("{k:#x}")).collect::<Vec<_>>().join(",") }

/// Compares the reference function `f` (on `rl`) with `g` (on `tl`): alignment, hunks, checks (module doc).
///
/// A function with a large switch on both sides is compared case by case: the code before the first case, the
/// default bodies, and each reference case body with the level body its cases go to; bodies only one side has
/// are the switch's case-set difference (`case_only`), not hunks.
fn compare(rl: &Level, f: u32, tl: &Level, g: u32, max_lines: usize, loose: bool) -> Cell { compare_n(rl, f, tl, g, max_lines, loose, None) }

/// [`compare`] over at most `n` words on each side.
fn compare_n(rl: &Level, f: u32, tl: &Level, g: u32, max_lines: usize, loose: bool, n: Option<usize>) -> Cell {
    let (Some(a), Some(b)) = (rl.words(f), tl.words(g)) else { return Cell { status: '?', ..Cell::default() } };
    let (a, b) = match n { Some(n) => (&a[..a.len().min(n)], &b[..b.len().min(n)]), None => (a, b) };
    let (na, nb) = if loose { (norm_loose(a), norm_loose(b)) } else { (norm(a), norm(b)) };
    let mut c = Cell { counterpart: Some(g), ..Cell::default() };
    let (ta, tb) = (jump_tables(&rl.ov, f, a), jump_tables(&tl.ov, g, b));
    let pc_a = |i: usize| f + 4 * i as u32;
    let pc_b = |j: usize| g + 4 * j as u32;
    // Segments to align: (reference range, level range, label).
    let mut segs: Vec<(std::ops::Range<usize>, std::ops::Range<usize>, String)> = Vec::new();
    let mut main: Option<(usize, usize, Switch, Switch)> = None;
    if let (Some((ia, va)), Some((ib, vb))) = (main_table(&ta), main_table(&tb)) {
        let (sa, sb) = (Switch::new(f, a.len(), va), Switch::new(g, b.len(), vb));
        segs.push((0..sa.first, 0..sb.first, String::new()));
        let mut used: BTreeSet<u32> = BTreeSet::new();
        let mut pair: BTreeMap<u32, u32> = BTreeMap::new();
        if let (Some(da), Some(db)) = (sa.default, sb.default) {
            pair.insert(da, db);
            used.insert(db);
        }
        for (&t, ks) in &sa.owners {
            if Some(t) == sa.default { continue; }
            let mut votes: BTreeMap<u32, usize> = BTreeMap::new();
            for &k in ks { if let Some(u) = sb.handles(k).filter(|u| !used.contains(u)) { *votes.entry(u).or_default() += 1; } }
            if let Some((&u, _)) = votes.iter().max_by_key(|(u, n)| (**n, u32::MAX - **u)) {
                pair.insert(t, u);
                used.insert(u);
            }
        }
        for (&t, &u) in &pair {
            let lab = if Some(t) == sa.default { " [default]".to_string() } else { format!(" [case {}]", cases(&sa.owners[&t])) };
            segs.push((sa.ranges[&t].clone(), sb.ranges[&u].clone(), lab));
        }
        for (&t, ks) in &sa.owners {
            if pair.contains_key(&t) { continue; }
            let r = &sa.ranges[&t];
            let other: Vec<usize> = ks.iter().copied().filter(|&k| sb.handles(k).is_some()).collect();
            c.case_only.push(format!(
                "  ref-only body 0x{:x} ({} words): cases {}{}",
                pc_a(r.start),
                r.len(),
                cases(ks),
                if other.is_empty() { String::new() } else { format!(" (the level handles {} elsewhere)", cases(&other)) }
            ));
        }
        for (&u, ks) in &sb.owners {
            if used.contains(&u) { continue; }
            let r = &sb.ranges[&u];
            c.case_only.push(format!("  lvl-only body 0x{:x} ({} words): cases {}", pc_b(r.start), r.len(), cases(ks)));
            c.lvl_bad.extend(r.clone());
        }
        // Cases both handle that go to bodies which are not a pair.
        let mut split = Vec::new();
        for k in 0..sa.targets.len().max(sb.targets.len()) {
            if let (Some(t), Some(u)) = (sa.handles(k), sb.handles(k)) {
                if pair.get(&t) != Some(&u) { split.push(k); }
            }
        }
        if !split.is_empty() { c.other.push(format!("  ~ cases {} go to bodies that are not each other's pair", cases(&split))); }
        main = Some((ia, ib, sa, sb));
    } else {
        segs.push((0..a.len(), 0..b.len(), String::new()));
    }
    // Align each segment; hunks are the gaps.
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for (ra, rb, lab) in &segs {
        let p: Vec<(usize, usize)> = align(&na[ra.clone()], &nb[rb.clone()]).into_iter().map(|(i, j)| (i + ra.start, j + rb.start)).collect();
        let mut prev = (ra.start, rb.start);
        let mut ends = p.clone();
        ends.push((ra.end, rb.end));
        for &(i, j) in &ends {
            if i > prev.0 || j > prev.1 {
                let (ha, hb) = (prev.0..i, prev.1..j);
                c.removed += ha.len();
                c.added += hb.len();
                c.lvl_bad.extend(hb.clone());
                let mut h = format!("  @@ ref 0x{:x} -{} | lvl 0x{:x} +{}{lab}\n", pc_a(ha.start), ha.len(), pc_b(hb.start), hb.len());
                // Two aligned instructions before the hunk, as context.
                if max_lines > 0 { side_lines(&mut h, ' ', ha.start.saturating_sub(2).max(ra.start)..ha.start, a, f, rl, 2); }
                side_lines(&mut h, '-', ha, a, f, rl, max_lines);
                side_lines(&mut h, '+', hb, b, g, tl, max_lines);
                c.hunks.push(h);
            }
            prev = (i + 1, j + 1);
        }
        pairs.extend(p);
    }
    let map = build_map(a.len(), &pairs);
    // Checks on aligned pairs.
    let (ra, rb): (HashMap<usize, u32>, HashMap<usize, u32>) = (address_refs(a).into_iter().collect(), address_refs(b).into_iter().collect());
    let fend = f + 4 * a.len() as u32;
    let gend = g + 4 * b.len() as u32;
    let inside_a = |x: u32| x >= f && x < fend;
    let lvl_of = |x: u32| -> Option<u32> { map.get(((x - f) / 4) as usize).copied().flatten().map(|j| g + 4 * j as u32) };
    let mut noted: Vec<usize> = Vec::new();
    for &(i, j) in &pairs {
        let (n0, k0) = (c.other.len(), c.consts.len());
        let (wa, wb) = (a[i], b[j]);
        let line = |what: String| format!("  ~ ref {:08x} {} | lvl {:08x} {}: {what}", pc_a(i), mips::disasm(wa, pc_a(i)), pc_b(j), mips::disasm(wb, pc_b(j)));
        if let (Some(x), Some(y)) = (mips::target(wa, pc_a(i)), mips::target(wb, pc_b(j))) {
            let call = op(wa) == 3 || (op(wa) == 2 && !inside_a(x));
            if !call {
                if inside_a(x) && y >= g && y < gend && lvl_of(x).is_some_and(|m| m != y) { c.other.push(line("branch lands elsewhere".into())); }
            } else if x < OVERLAY_BASE || y < OVERLAY_BASE {
                if x != y { c.other.push(line("other boot-ELF callee".into())); }
            } else {
                c.calls.push((x, y));
            }
            if c.other.len() > n0 { noted.push(j); }
            continue;
        }
        match (ra.get(&i), rb.get(&j)) {
            (Some(&x), Some(&y)) => {
                if x < OVERLAY_BASE || y < OVERLAY_BASE {
                    if x != y { c.other.push(line(format!("boot address 0x{x:x} vs 0x{y:x}"))); }
                } else if main.as_ref().is_some_and(|m| (m.0, m.1) == (i, j)) {
                    // The main switch: compared case by case above.
                } else if let (Some(t1), Some(t2)) = (ta.get(&i), tb.get(&j)) {
                    let mut bad = Vec::new();
                    for k in 0..t1.len().max(t2.len()) {
                        match (t1.get(k), t2.get(k)) {
                            (Some(&p), Some(&q)) => { if lvl_of(p) != Some(q) { bad.push(format!("{k:#x}")); } }
                            (Some(_), None) => bad.push(format!("{k:#x}(ref only)")),
                            (None, Some(_)) => bad.push(format!("{k:#x}(lvl only)")),
                            (None, None) => {}
                        }
                    }
                    if !bad.is_empty() { c.other.push(line(format!("jump table {} / {} cases, cases to other code: {}", t1.len(), t2.len(), bad.join(" ")))); }
                } else if let Some(n) = load_width(wa).filter(|_| !rl.in_text(x) && !tl.in_text(y)) {
                    // (A "load" from `.text` is a `lui` the mask followed past its real use: not data.)
                    if let (Some(p), Some(q)) = (rl.ov.read(x, n), tl.ov.read(y, n)) {
                        if !same_data(p, q) { c.consts.push(line(format!("[0x{x:x}] {} vs [0x{y:x}] {}", show_bytes(p), show_bytes(q)))); }
                    }
                }
            }
            (None, None) => {}
            _ => c.other.push(line("address formed on one side only".into())),
        }
        if c.other.len() > n0 || c.consts.len() > k0 { noted.push(j); }
    }
    c.noted.extend(noted);
    c.map = map;
    c
}

/// Shingles (runs of four normalised words) of a function.
fn shingles(words: &[u32]) -> HashSet<u64> {
    let n = norm(words);
    n.windows(4).map(|w| w.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &x| (h ^ x as u64).wrapping_mul(0x100_0000_01b3))).collect()
}

/// Shared shingles over the larger set (0..1; two functions under 4 words compare equal words).
fn similarity(a: &[u32], b: &[u32]) -> f32 {
    if a.len() < 4 || b.len() < 4 { return if norm(a) == norm(b) { 1.0 } else { 0.0 }; }
    let (x, y) = (shingles(a), shingles(b));
    x.intersection(&y).count() as f32 / x.len().max(y.len()).max(1) as f32
}

/// A level's shingle index over its strong function starts.
struct Fuzzy {
    post: HashMap<u64, Vec<u32>>,
    size: HashMap<u32, usize>,
}

impl Fuzzy {
    fn new(l: &Level) -> Fuzzy {
        let mut post: HashMap<u64, Vec<u32>> = HashMap::new();
        let mut size = HashMap::new();
        for &g in &l.graph.strong {
            let Some(w) = l.words(g) else { continue };
            let s = shingles(w);
            size.insert(g, s.len());
            for h in s { post.entry(h).or_default().push(g); }
        }
        Fuzzy { post, size }
    }

    /// The clearly most similar function to `words` (score = shared shingles / the larger set), with its score.
    fn best(&self, words: &[u32]) -> Option<(u32, f32)> {
        let s = shingles(words);
        if s.is_empty() { return None; }
        let mut hits: HashMap<u32, usize> = HashMap::new();
        for h in &s {
            if let Some(v) = self.post.get(h).filter(|v| v.len() <= 64) { for &g in v { *hits.entry(g).or_default() += 1; } }
        }
        let mut sc: Vec<(f32, u32)> = hits.into_iter().map(|(g, n)| (n as f32 / s.len().max(self.size[&g]) as f32, g)).collect();
        sc.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
        let (b0, g0) = *sc.first()?;
        let b1 = sc.get(1).map_or(0.0, |x| x.0);
        (b0 >= 0.25 && b1 < 0.8 * b0).then_some((g0, b0))
    }
}

pub fn run(extracted: &Path, work: &Path, repo: &Path, o: &Options) -> Result<Summary> {
    let mut levels: HashMap<u32, Level> = HashMap::new();
    for l in 0..19u32 {
        let b = std::fs::read(extracted.join(format!("levels/{l:02}/overlay.bin"))).with_context(|| format!("level {l:02} overlay"))?;
        let ov = LevelOverlay::parse(&b).map_err(|e| anyhow::anyhow!("level {l:02}: {e:?}"))?;
        let graph = Graph::build(&ov);
        levels.insert(l, Level { ov, graph, names: names(work, &format!("level{l:02}.elf")) });
    }
    // Rows.
    let mut rows: BTreeMap<(u32, u32), Row> = BTreeMap::new();
    for d in &o.cite { cited(&repo.join(d), &levels, &mut rows); }
    for &(r, a) in &o.fns { rows.entry((r, a)).or_insert(Row { reference: r, addr: a, name: String::new(), source: "given" }); }
    for _ in 0..o.callee_depth {
        let add: Vec<(u32, u32)> = rows
            .keys()
            .flat_map(|&(r, a)| {
                levels[&r].graph.callees.get(&a).into_iter().flatten().filter(|(_, e)| matches!(e, Edge::Call | Edge::Tail)).map(move |&(c, _)| (r, c))
            })
            .filter(|&(r, c)| c >= OVERLAY_BASE && levels[&r].graph.strong.contains(&c) && !rows.contains_key(&(r, c)))
            .collect();
        for (r, c) in add { rows.entry((r, c)).or_insert(Row { reference: r, addr: c, name: String::new(), source: "callee" }); }
    }
    for row in rows.values_mut() {
        row.name = levels[&row.reference].names.get(&row.addr).cloned().unwrap_or_else(|| format!("FUN_{:08x}", row.addr));
    }
    let keys: Vec<(u32, u32)> = rows.keys().copied().collect();
    let key_ix: HashMap<(u32, u32), usize> = keys.iter().enumerate().map(|(i, &k)| (k, i)).collect();
    // cells[row][level]
    let mut cells: Vec<Vec<Cell>> = vec![vec![Cell::default(); 19]; keys.len()];
    for t in 0..19u32 {
        let tl = &levels[&t];
        let relocs: HashMap<u32, Relocation> = REFS.iter().map(|&r| (r, Relocation::new(&levels[&r].ov, &tl.ov))).collect();
        // Level → reference: a candidate that is an exact copy of another reference function is that function's.
        let back: HashMap<u32, Relocation> = REFS.iter().filter(|&&r| r != t).map(|&r| (r, Relocation::new(&tl.ov, &levels[&r].ov))).collect();
        let other_copy = |r: u32, a: u32, g: u32| {
            back.get(&r).is_some_and(|b| {
                let cs = b.copies(g);
                !cs.contains(&a) && cs.iter().any(|x| levels[&r].graph.strong.contains(x))
            })
        };
        let mut done: HashMap<usize, Cell> = HashMap::new();
        // The reference itself.
        for (ix, &(r, a)) in keys.iter().enumerate() {
            if r == t {
                let n = levels[&r].words(a).map_or(0, <[u32]>::len);
                done.insert(ix, Cell { counterpart: Some(a), how: "ref".into(), status: '.', map: (0..n).map(Some).collect(), ..Cell::default() });
            }
        }
        // 1. Exact masked copies (one strong start, or one start).
        for (ix, &(r, a)) in keys.iter().enumerate() {
            if done.contains_key(&ix) { continue; }
            let cs: Vec<u32> = relocs[&r].copies(a).into_iter().filter(|g| tl.graph.starts.contains(g)).collect();
            let strong: Vec<u32> = cs.iter().copied().filter(|g| tl.graph.strong.contains(g)).collect();
            let pick = match (strong.as_slice(), cs.as_slice()) {
                ([g], _) | ([], [g]) => Some(*g),
                _ => None,
            };
            if let Some(g) = pick {
                // Compared over the span the copy is proven equal on (the relocator's extent: a longer strong-start
                // extent can run into a function only reached by a pointer, which differs per level).
                let mut c = compare_n(&levels[&r], a, tl, g, o.max_lines, false, levels[&r].ov.extent(a));
                c.how = "copy".into();
                done.insert(ix, c);
            }
        }
        // 2. Call sites of callers with a known counterpart (rows first, then exact copies of other callers).
        loop {
            let mut found: Vec<(usize, u32)> = Vec::new();
            for (ix, &(r, a)) in keys.iter().enumerate() {
                if done.contains_key(&ix) { continue; }
                let rl = &levels[&r];
                let mut votes: BTreeMap<u32, usize> = BTreeMap::new();
                for &cf in rl.graph.callers.get(&a).into_iter().flatten() {
                    let Some(cw) = rl.words(cf) else { continue };
                    let (cg, map): (u32, Vec<Option<usize>>) = if let Some(c) = key_ix.get(&(r, cf)).and_then(|i| done.get(i)) {
                        let Some(g) = c.counterpart else { continue };
                        (g, c.map.clone())
                    } else if let Some(g) = relocs[&r].func(cf).filter(|g| tl.graph.starts.contains(g)) {
                        (g, (0..cw.len()).map(Some).collect())
                    } else {
                        continue;
                    };
                    let Some(gw) = tl.words(cg) else { continue };
                    for (k, &w) in cw.iter().enumerate() {
                        if !matches!(op(w), 2 | 3) || mips::target(w, cf + 4 * k as u32) != Some(a) { continue; }
                        let Some(Some(k2)) = map.get(k) else { continue };
                        let Some(&w2) = gw.get(*k2) else { continue };
                        if op(w2) != op(w) { continue; }
                        if let Some(x) = mips::target(w2, cg + 4 * *k2 as u32).filter(|x| tl.graph.starts.contains(x)) { *votes.entry(x).or_default() += 1; }
                    }
                }
                // The best-voted candidate named by two or more call sites, or one that also looks like the
                // function (a single `jal` aligned inside a large differing caller can name another function).
                let aw = rl.words(a).unwrap_or(&[]);
                let mut vs: Vec<(u32, usize)> = votes.into_iter().collect();
                vs.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
                if let Some(&(g, _)) = vs.iter().find(|(g, n)| !other_copy(r, a, *g) && (*n >= 2 || tl.words(*g).is_some_and(|gw| similarity(aw, gw) >= 0.3))) { found.push((ix, g)); }
            }
            if found.is_empty() { break; }
            for (ix, g) in found {
                let (r, a) = keys[ix];
                let mut c = compare(&levels[&r], a, tl, g, o.max_lines, false);
                c.how = "call site".into();
                done.insert(ix, c);
            }
        }
        // 3. The callers of its callees' counterparts, each callee weighted by how few functions call it there.
        loop {
            let mut found: Vec<(usize, u32)> = Vec::new();
            for (ix, &(r, a)) in keys.iter().enumerate() {
                if done.contains_key(&ix) { continue; }
                let rl = &levels[&r];
                let mut score: BTreeMap<u32, f32> = BTreeMap::new();
                for &(cf, e) in rl.graph.callees.get(&a).into_iter().flatten() {
                    if !matches!(e, Edge::Call) { continue; }
                    let cg = if cf < OVERLAY_BASE {
                        Some(cf)
                    } else if let Some(c) = key_ix.get(&(r, cf)).and_then(|i| done.get(i)) {
                        c.counterpart
                    } else {
                        relocs[&r].func(cf)
                    };
                    let Some(callers) = cg.and_then(|x| tl.graph.callers.get(&x)) else { continue };
                    let w = 1.0 / callers.len() as f32;
                    for &x in callers { if tl.graph.strong.contains(&x) { *score.entry(x).or_default() += w; } }
                }
                let mut v: Vec<(u32, f32)> = score.into_iter().collect();
                v.sort_by(|x, y| y.1.total_cmp(&x.1).then(x.0.cmp(&y.0)));
                // Among the candidates the callees point at (score ≥ 0.25), the clearly most similar body.
                let aw = rl.words(a).unwrap_or(&[]);
                let mut sims: Vec<(f32, u32)> = v.iter().filter(|x| x.1 >= 0.25).filter_map(|&(g, _)| Some((similarity(aw, tl.words(g)?), g))).collect();
                sims.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
                let Some(&(s0, g)) = sims.first() else { continue };
                let s1 = sims.get(1).map_or(0.0, |x| x.0);
                if s0 >= 0.1 && s0 >= 1.3 * s1 && !other_copy(r, a, g) { found.push((ix, g)); }
            }
            if found.is_empty() { break; }
            for (ix, g) in found {
                let (r, a) = keys[ix];
                let mut c = compare(&levels[&r], a, tl, g, o.max_lines, false);
                c.how = "callees".into();
                done.insert(ix, c);
            }
        }
        // 4. The most similar function.
        let fuzzy = Fuzzy::new(tl);
        for (ix, &(r, a)) in keys.iter().enumerate() {
            if done.contains_key(&ix) { continue; }
            // A weak match (under 0.5) must also call one of the function's callees (boot, or an overlay callee's
            // exact copy): particle and moby helpers look alike.
            let shares_callee = |g: u32| {
                let cs = |gr: &Graph, f: u32| -> Vec<u32> { gr.callees.get(&f).into_iter().flatten().filter(|(_, e)| matches!(e, Edge::Call)).map(|&(c, _)| c).collect() };
                let theirs = cs(&tl.graph, g);
                cs(&levels[&r].graph, a).into_iter().any(|c| if c < OVERLAY_BASE { theirs.contains(&c) } else { relocs[&r].func(c).is_some_and(|x| theirs.contains(&x)) })
            };
            let hit = levels[&r].words(a).and_then(|w| fuzzy.best(w)).filter(|&(g, sc)| !other_copy(r, a, g) && (sc >= 0.5 || shares_callee(g)));
            let c = match hit {
                Some((g, s)) => {
                    let mut c = compare(&levels[&r], a, tl, g, o.max_lines, false);
                    c.how = format!("similar {s:.2}");
                    c
                }
                None => Cell { status: '?', how: "none".into(), ..Cell::default() },
            };
            done.insert(ix, c);
        }
        for (ix, c) in done { cells[ix][t as usize] = c; }
    }
    // Own status, then the callee checks (to a fixpoint through the rows).
    for (ix, row) in cells.iter_mut().enumerate() {
        for (t, c) in row.iter_mut().enumerate() {
            if c.status == '.' || c.status == '?' { continue; }
            c.status = if !c.hunks.is_empty() || !c.other.is_empty() { 'D' } else if !c.consts.is_empty() { 'k' } else if !c.case_only.is_empty() { 'S' } else { '=' };
            let _ = (ix, t);
        }
    }
    // The superset check: the level's differing words that level 00's copy (the superset build) does not have
    // either are the level's own code (`U`); a difference level 00 covers is `s`.
    for (ix, &(r, _)) in keys.iter().enumerate() {
        let f0 = if r == 0 { None } else { cells[ix][0].counterpart };
        for t in 0..19usize {
            let c = &cells[ix][t];
            if !matches!(c.status, 'D' | 'S' | 'k') || t as u32 == r || (r == 1 && t == 0) { continue; }
            let Some(g) = c.counterpart else { continue };
            let tl = &levels[&(t as u32)];
            // Differing words, register-blind, against level 01 and level 00; a flagged check counts when the
            // same word is flagged against both.
            // Differing words, register-blind, against level 01 and level 00 (aligned in order, or found anywhere
            // in either copy as a run of three: a build that compiled fewer cases orders the blocks of a state
            // chain differently); a flagged check counts when the same word is flagged against both.
            let Some(b) = tl.words(g) else { continue };
            let refs: Vec<&[u32]> = if r == 0 { vec![levels[&0].words(keys[ix].1).unwrap_or(&[])] } else if let Some(f0) = f0 { vec![levels[&1].words(keys[ix].1).unwrap_or(&[]), levels[&0].words(f0).unwrap_or(&[])] } else { continue };
            let grams: HashSet<[u32; 3]> = refs.iter().flat_map(|w| norm_loose(w).windows(3).map(|x| [x[0], x[1], x[2]]).collect::<Vec<_>>()).collect();
            let lb = norm_loose(b);
            let words1: HashSet<u32> = refs.iter().flat_map(|w| norm_loose(w)).collect();
            let lo_masked: HashSet<u32> = words1.iter().filter(|&&w| op(w) == 9).map(|w| w & 0xffff_0000).collect();
            let covered = |j: usize| (j.saturating_sub(2)..=j).any(|k| k + 3 <= lb.len() && grams.contains(&[lb[k], lb[k + 1], lb[k + 2]]));
            let (unaligned, flagged): (BTreeSet<usize>, BTreeSet<usize>) = if r == 0 {
                let l0 = compare(&levels[&0], keys[ix].1, tl, g, 0, true);
                (l0.lvl_bad.iter().copied().filter(|&j| !covered(j)).collect(), c.noted.clone())
            } else {
                let f0 = f0.unwrap();
                let l1 = compare(&levels[&1], keys[ix].1, tl, g, 0, true);
                let l0 = compare(&levels[&0], f0, tl, g, 0, true);
                let s0 = compare(&levels[&0], f0, tl, g, 0, false);
                (l1.lvl_bad.intersection(&l0.lvl_bad).copied().filter(|&j| !covered(j)).collect(), c.noted.intersection(&s0.noted).copied().collect())
            };
            let bad: BTreeSet<usize> = unaligned.union(&flagged).copied().collect();
            let sw = main_table(&jump_tables(&tl.ov, g, b)).map(|(_, v)| Switch::new(g, b.len(), v));
            let mut runs: Vec<String> = Vec::new();
            let v: Vec<usize> = bad.into_iter().collect();
            let mut k = 0;
            while k < v.len() {
                let mut e = k;
                while e + 1 < v.len() && v[e + 1] == v[e] + 1 { e += 1; }
                let run = v[k]..v[e] + 1;
                k = e + 1;
                if run.clone().all(|j| b[j] == 0) { continue; }
                // The main switch's bound check (`sltiu` / `slti` against the table length): the case set.
                let bound = |j: usize| sw.as_ref().is_some_and(|s| matches!(op(b[j]), 0x0a | 0x0b) && (b[j] & 0xffff) as usize == s.targets.len());
                if run.clone().all(bound) { continue; }
                // One or two words whose instructions the reference copies have elsewhere: scheduling, not logic.
                if run.len() <= 2 && run.clone().all(|j| words1.contains(&lb[j]) && !flagged.contains(&j)) { continue; }
                // One or two `addiu rX, rY, imm` (rY not `$zero`) that the reference copies have with another
                // immediate: the `%lo` half of an address whose `lui` the mask lost track of (a register join).
                if run.len() <= 2 && run.clone().all(|j| op(b[j]) == 9 && (b[j] >> 21) & 31 != 0 && lo_masked.contains(&(lb[j] & 0xffff_0000))) { continue; }
                let lab = sw.as_ref().and_then(|s| s.ranges.iter().find(|(_, r)| r.contains(&run.start)).map(|(t, _)| format!(" [case {}]", cases(&s.owners[t])))).unwrap_or_default();
                let mut h = format!("  !! lvl 0x{:x} +{}{lab}\n", g + 4 * run.start as u32, run.len());
                side_lines(&mut h, '+', run, b, g, tl, o.max_lines);
                runs.push(h);
            }
            let c = &mut cells[ix][t];
            if !runs.is_empty() {
                c.status = 'U';
                c.unique = runs;
            } else if c.status == 'D' {
                c.status = 's';
            }
        }
    }
    let same_cache: HashMap<(usize, u32, u32), bool> = {
        let mut m = HashMap::new();
        for t in 0..19u32 {
            let tl = &levels[&t];
            let relocs: HashMap<u32, Relocation> = REFS.iter().map(|&r| (r, Relocation::new(&levels[&r].ov, &tl.ov))).collect();
            for (ix, row) in cells.iter().enumerate() {
                let r = keys[ix].0;
                for &(x, y) in &row[t as usize].calls {
                    if key_ix.contains_key(&(r, x)) { continue; }
                    m.entry((t as usize, x, y)).or_insert_with(|| if r == t { x == y } else { relocs[&r].same_code(x, y) });
                }
            }
        }
        m
    };
    loop {
        let mut changed = false;
        for ix in 0..cells.len() {
            let r = keys[ix].0;
            #[allow(clippy::needless_range_loop)] // `t` also indexes the callee rows' cells.
            for t in 0..19usize {
                let c = &cells[ix][t];
                if !matches!(c.status, '=' | 'k' | 'S' | 's' | 'U' | 'D') { continue; }
                let mut diffs = Vec::new();
                for &(x, y) in &c.calls {
                    let bad = match key_ix.get(&(r, x)) {
                        Some(&cx) => {
                            let cc = &cells[cx][t];
                            if cc.counterpart != Some(y) { Some(format!("0x{x:x} → 0x{y:x}: not the callee's counterpart ({:?})", cc.counterpart.map(|v| format!("{v:x}")))) } else if matches!(cc.status, '=' | '.') { None } else { Some(format!("0x{x:x} {} → 0x{y:x} ({})", rows[&(r, x)].name, cc.status)) }
                        }
                        None => (!same_cache.get(&(t, x, y)).copied().unwrap_or(false)).then(|| format!("0x{x:x} → 0x{y:x} (not a row; code differs)")),
                    };
                    if let Some(b) = bad { if !diffs.contains(&b) { diffs.push(b); } }
                }
                if !diffs.is_empty() {
                    let c = &mut cells[ix][t];
                    c.callee_diffs = diffs;
                    if c.status == '=' { c.status = 'c'; changed = true; }
                }
            }
        }
        if !changed { break; }
    }
    // Outputs.
    std::fs::create_dir_all(&o.out)?;
    let mut tsv = String::from("ref\taddr\tname\twords\tsource\treads_level");
    for t in 0..19 { let _ = write!(tsv, "\tL{t:02}"); }
    tsv.push('\n');
    let mut matrix = format!("{:<8} {:<10} {:<34} 0000000000111111111\n{:<8} {:<10} {:<34} 0123456789012345678\n", "", "", "", "ref", "addr", "name");
    let mut diffs = String::new();
    let mut counts: BTreeMap<char, usize> = BTreeMap::new();
    let mut own = 0;
    // A level-00 row that is the level-00 copy of a level-01 row is that row's L00 column already.
    let l00_of_l01: HashSet<u32> = keys.iter().enumerate().filter(|(_, k)| k.0 == 1).filter_map(|(ix, _)| cells[ix][0].counterpart).collect();
    let mut shown = 0;
    for (ix, &(r, a)) in keys.iter().enumerate() {
        if r == 0 && l00_of_l01.contains(&a) { continue; }
        shown += 1;
        let row = &rows[&(r, a)];
        let rl = &levels[&r];
        let words = rl.words(a).unwrap_or(&[]);
        let reads_level = address_refs(words).iter().any(|&(_, x)| x == LEVEL_NUMBER);
        let _ = write!(tsv, "L{r:02}\t{a:08x}\t{}\t{}\t{}\t{}", row.name, words.len(), row.source, if reads_level { "yes" } else { "" });
        let mut line = String::new();
        for (t, c) in cells[ix].iter().enumerate() {
            *counts.entry(c.status).or_default() += 1;
            line.push(c.status);
            match c.status {
                '.' => tsv.push_str("\t."),
                '=' => { let _ = write!(tsv, "\t= {:x}", c.counterpart.unwrap_or(0)); }
                '?' => tsv.push_str("\t?"),
                s => { let _ = write!(tsv, "\t{s} {:x} -{}/+{} h{} k{} o{} s{} c{}", c.counterpart.unwrap_or(0), c.removed, c.added, c.hunks.len(), c.consts.len(), c.other.len(), c.case_only.len(), c.callee_diffs.len()); }
            }
            let _ = t;
        }
        tsv.push('\n');
        let _ = writeln!(matrix, "L{r:02}      {a:08x}   {:<34} {line}", row.name.chars().take(34).collect::<String>());
        if cells[ix].iter().any(|c| matches!(c.status, 'D' | 'k' | 'U')) { own += 1; }
        if cells[ix].iter().all(|c| matches!(c.status, '.' | '=')) { continue; }
        let _ = writeln!(diffs, "=== L{r:02} 0x{a:08x} {} ({} words, {}) {line}", row.name, words.len(), row.source);
        for (t, c) in cells[ix].iter().enumerate() {
            if matches!(c.status, '.' | '=') { continue; }
            let _ = writeln!(
                diffs,
                "--- L{t:02} {} {} ({}) -{} +{}",
                c.status,
                c.counterpart.map_or("-".into(), |g| format!("0x{g:08x} {}", levels[&(t as u32)].names.get(&g).map_or("", String::as_str))),
                c.how,
                c.removed,
                c.added
            );
            if !c.unique.is_empty() {
                let _ = writeln!(diffs, "  level code neither level 01 nor level 00 has:");
                for x in &c.unique { diffs.push_str(x); }
                let _ = writeln!(diffs, "  against level 01:");
            }
            for x in &c.case_only { let _ = writeln!(diffs, "{x}"); }
            for h in &c.hunks { diffs.push_str(h); }
            for x in c.other.iter().chain(&c.consts) { let _ = writeln!(diffs, "{x}"); }
            for x in &c.callee_diffs { let _ = writeln!(diffs, "  > callee {x}"); }
        }
    }
    let legend = "legend: . reference  = identical  c only a callee differs  S a switch with other cases, the shared cases identical  k same code, other constants  s differs from level 01, all of it level-00 (superset) code  U the level has code neither level 01 nor level 00 has  D differs (no level-00 copy to check)  ? no counterpart\n";
    std::fs::write(o.out.join(format!("{}.tsv", o.name)), tsv)?;
    std::fs::write(o.out.join(format!("{}_matrix.txt", o.name)), format!("{legend}{matrix}"))?;
    std::fs::write(o.out.join(format!("{}_diffs.txt", o.name)), format!("{legend}{diffs}"))?;
    Ok(Summary { rows: shown, counts, own_diff_rows: own, out: o.out.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alignment_keeps_common_runs_and_masks_branch_offsets() {
        // beq +3 vs beq +4 (an inserted instruction below it) align; the insertion is the only gap.
        let a = [0x1040_0003, 0x2402_0001, 0x2403_0002, 0x03e0_0008];
        let b = [0x1040_0004, 0x2402_0001, 0x2404_0005, 0x2403_0002, 0x03e0_0008];
        let p = align(&norm(&a), &norm(&b));
        assert_eq!(p, vec![(0, 0), (1, 1), (2, 3), (3, 4)]);
        // Stack-frame offsets: `sq s0, 0x10(sp)` and `sq s0, 0x40(sp)` align.
        assert_eq!(norm(&[0x7fb0_0010]), norm(&[0x7fb0_0040]));
    }

    #[test]
    fn register_blind_words_keep_opcodes_and_immediates() {
        // addiu v0, zero, 0x16 and addiu v1, zero, 0x16 are one word; 0x17 is another.
        assert_eq!(norm_loose(&[0x2402_0016]), norm_loose(&[0x2403_0016]));
        assert_ne!(norm_loose(&[0x2402_0016]), norm_loose(&[0x2402_0017]));
        // mul.s $f0, $f1, $f0 and mul.s $f2, $f3, $f4: one word; add.s is another.
        assert_eq!(norm_loose(&[0x4600_0802]), norm_loose(&[0x4604_1882]));
        assert_ne!(norm_loose(&[0x4600_0802]), norm_loose(&[0x4600_0800]));
    }

    #[test]
    fn data_compare_takes_relocated_pointers_as_equal() {
        let w = |v: u32| v.to_le_bytes();
        assert!(same_data(&w(0x0016_cc00), &w(0x0016_c780)), "two main-RAM addresses");
        assert!(!same_data(&w(0x3f80_0000), &w(0x4000_0000)), "1.0 vs 2.0");
        assert!(!same_data(&w(0x24), &w(0x324)), "small integers");
    }

    #[test]
    fn switch_shape_default_and_bodies() {
        // Cases 0 and 2 share the default at +0x40; case 1 at +0x10, case 3 at +0x20; 32 words.
        let base = 0x1000;
        let sw = Switch::new(base, 32, &[base + 0x40, base + 0x10, base + 0x40, base + 0x20]);
        assert_eq!(sw.default, Some(base + 0x40));
        assert_eq!((sw.handles(0), sw.handles(1), sw.handles(3)), (None, Some(base + 0x10), Some(base + 0x20)));
        assert_eq!(sw.first, 4);
        assert_eq!((sw.ranges[&(base + 0x10)].clone(), sw.ranges[&(base + 0x20)].clone(), sw.ranges[&(base + 0x40)].clone()), (4..8, 8..16, 16..32));
    }
}
