//! `export`: Tier 2, the optional "usable formats" (docs/plan/launcher_extractor.md "Tier 2 as built").
//!
//! Reads the Tier 0 archive in a data folder through the same golden-tested `rc-formats` loaders the engine uses
//! and writes standard files for people and mod authors: PNG textures (+ JSON sidecars with the CLUT and GS
//! state), WAV audio (+ loop points), glTF 2.0 levels, collision and moby models, and JSON tables and text. The
//! game never reads any of it; it never needs the disc image. Encoders are in-crate ([`png`], [`wav`], [`gltf`],
//! [`jsonv`]), dependency-free and tested.
//!
//! Output (`--to`, default `<data>/exports/`); paths mirror the asset paths mods will override (docs/plan/mods.md):
//! ```text
//! export-info.json                         written last: kinds, levels, file counts
//! textures/levels/NN/<table>/<key>.png     + .json sidecar; tfrag/billboard mips as <key>.mipK.png
//! textures/levels/NN/{sky,particle,fx,hud}/…
//! audio/levels/NN/sound_bank/NNN.wav       + .json; audio/levels/NN/sound_bank.json = the bank index
//! audio/levels/NN/{music,speech}/*.wav     + .json
//! audio/global/…                           global bank, music and the global VAG folders
//! levels/NN/level.gltf + level.bin         tfrag, tie and shrub instances, moby placements, sky (scenes)
//! levels/NN/collision.gltf + .bin          collision mesh, one primitive per surface
//! levels/NN/*.json                         mobys, ties, shrubs, volumes, paths, grind paths, sound instances, …
//! models/levels/NN/mobys/CCCC.gltf + .bin  moby classes, skinned with every animation sequence
//! text/levels/NN/<lang>.json, text/global/all_text/<lang>.json
//! ```
//! Files are written as `<name>.partial` and renamed when complete; `export-info.json` is removed at the start and
//! written last, so its absence marks an incomplete export.

pub mod gltf;
pub mod jsonv;
pub mod png;
pub mod wav;

mod audio;
mod data;
mod geometry;
#[cfg(test)]
mod level_test;
mod models;
mod tables;
mod text;
mod textures;

use crate::workers::{self, Ctx};
use crate::{mib, Code, Emit, Error, Event, Stage, EXTRACTOR_VERSION, PARTIAL_SUFFIX};
use jsonv::{Obj, J};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

/// Version of the export layout (`format` in `export-info.json`).
pub const EXPORT_FORMAT: u32 = 1;
/// Default export folder inside the data folder.
pub const DEFAULT_DIR: &str = "exports";
/// Written last by a successful export.
pub const INFO_FILE: &str = "export-info.json";
/// Info lines naming skipped items are capped at this many.
const MAX_WARNING_LINES: usize = 50;

/// What to export (`--what`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Kinds {
    pub textures: bool,
    pub audio: bool,
    pub models: bool,
    pub levels: bool,
    pub collision: bool,
    pub text: bool,
}

impl Kinds {
    pub const NAMES: [&'static str; 6] = ["textures", "audio", "models", "levels", "collision", "text"];
    pub const ALL: Kinds = Kinds { textures: true, audio: true, models: true, levels: true, collision: true, text: true };

    /// `textures,audio,…` or `all`.
    pub fn parse(s: &str) -> Result<Kinds, String> {
        let mut k = Kinds::default();
        for w in s.split(',').map(str::trim).filter(|w| !w.is_empty()) {
            match w {
                "all" => k = Kinds::ALL,
                "textures" => k.textures = true,
                "audio" => k.audio = true,
                "models" => k.models = true,
                "levels" => k.levels = true,
                "collision" => k.collision = true,
                "text" => k.text = true,
                _ => return Err(format!("unknown export kind {w:?} (textures, audio, models, levels, collision, text, all)")),
            }
        }
        if k == Kinds::default() { return Err("--what names no kind".into()); }
        Ok(k)
    }

    pub fn names(&self) -> Vec<String> {
        let f = [self.textures, self.audio, self.models, self.levels, self.collision, self.text];
        Self::NAMES.iter().zip(f).filter(|(_, on)| *on).map(|(n, _)| n.to_string()).collect()
    }
}

pub struct Options<'a> {
    /// Output folder (created if missing).
    pub to: PathBuf,
    pub kinds: Kinds,
    /// Only this level (and no global data).
    pub level: Option<u32>,
    pub threads: usize,
    pub cancel: Option<&'a AtomicBool>,
}

/// Totals of an export.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Exported {
    pub files: u64,
    pub bytes: u64,
    /// Files and bytes per top-level folder (`textures`, `audio`, `levels`, `models`, `text`).
    pub by_folder: BTreeMap<String, (u64, u64)>,
    /// Items that could not be exported (named in info lines).
    pub skipped: usize,
    pub levels: Vec<u32>,
}

/// Where files go, with counters. Shared by the workers.
pub(crate) struct Out {
    root: PathBuf,
    stats: Mutex<BTreeMap<String, (u64, u64)>>,
    warnings: Mutex<Vec<String>>,
}

impl Out {
    fn new(root: &Path) -> Self { Out { root: root.to_path_buf(), stats: Mutex::default(), warnings: Mutex::default() } }

    /// Writes `rel` (forward slashes) as `<rel>.partial`, then renames it.
    pub(crate) fn write(&self, rel: &str, bytes: &[u8]) -> Result<(), Error> {
        let path = self.root.join(rel);
        if let Some(dir) = path.parent() { std::fs::create_dir_all(dir).map_err(|e| workers::write_error(dir, e))?; }
        let partial = crate::extract::partial_path(&path);
        std::fs::write(&partial, bytes).map_err(|e| workers::write_error(&partial, e))?;
        std::fs::rename(&partial, &path).map_err(|e| workers::write_error(&path, e))?;
        let top = rel.split('/').next().unwrap_or(rel).to_string();
        let mut s = self.stats.lock().unwrap_or_else(|e| e.into_inner());
        let e = s.entry(top).or_default();
        e.0 += 1;
        e.1 += bytes.len() as u64;
        Ok(())
    }

    pub(crate) fn json(&self, rel: &str, v: &J) -> Result<(), Error> { self.write(rel, v.pretty().as_bytes()) }

    /// An item that could not be exported; the export goes on.
    pub(crate) fn skip(&self, msg: String) { self.warnings.lock().unwrap_or_else(|e| e.into_inner()).push(msg); }
}

/// One unit of work: a level's kind, or a global kind.
#[derive(Clone, Debug)]
enum Job {
    Textures(u32),
    Audio(u32),
    Levels(u32),
    Collision(u32),
    Models(u32),
    Text(u32),
    GlobalAudio,
    GlobalText,
    GlobalTextures,
}

impl Job {
    fn name(&self) -> String {
        match self {
            Job::Textures(l) | Job::Levels(l) | Job::Collision(l) | Job::Models(l) => format!("levels/{l:02}/core_data.bin"),
            Job::Audio(l) => format!("levels/{l:02}/sound_bank.bin"),
            Job::Text(l) => format!("levels/{l:02}/gameplay_ntsc.bin"),
            Job::GlobalAudio => "global/sound_bank.bin".into(),
            Job::GlobalText => "global/all_text.bin".into(),
            Job::GlobalTextures => "global/hud_header.bin".into(),
        }
    }
}

/// The levels present in the archive (`levels/NN/level_header.bin`), ascending.
fn archive_levels(data: &Path) -> Vec<u32> {
    let mut v: Vec<u32> = std::fs::read_dir(data.join("levels"))
        .map(|rd| rd.flatten().filter_map(|e| e.file_name().to_str()?.parse().ok()).collect())
        .unwrap_or_default();
    v.retain(|l| data.join(format!("levels/{l:02}/level_header.bin")).is_file());
    v.sort_unstable();
    v
}

fn file_len(p: &Path) -> u64 { std::fs::metadata(p).map(|m| m.len()).unwrap_or(0) }

fn dir_len(p: &Path) -> u64 { std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| file_len(&e.path())).sum()).unwrap_or(0) }

/// Tier 0 bytes a job reads (the progress weight) and a generous estimate of what it writes.
fn job_weight(data: &Path, j: &Job) -> (u64, u64) {
    let lv = |l: u32, f: &str| data.join(format!("levels/{l:02}/{f}"));
    match *j {
        Job::Textures(l) => (file_len(&lv(l, "core_data.bin")) / 4 + file_len(&lv(l, "gs_ram.bin")), 3 * file_len(&lv(l, "gs_ram.bin")) + file_len(&lv(l, "core_data.bin")) / 2),
        Job::Levels(l) | Job::Models(l) => (file_len(&lv(l, "core_data.bin")) / 2, 2 * file_len(&lv(l, "core_data.bin"))),
        Job::Collision(l) => (file_len(&lv(l, "core_data.bin")) / 8, file_len(&lv(l, "core_data.bin")) / 2),
        Job::Text(l) => (file_len(&lv(l, "gameplay_ntsc.bin")), file_len(&lv(l, "gameplay_ntsc.bin"))),
        Job::Audio(l) => {
            let b = file_len(&lv(l, "sound_bank.bin")) + dir_len(&lv(l, "music")) + dir_len(&lv(l, "speech"));
            (b, 4 * b)
        }
        Job::GlobalAudio => {
            let b = audio::GLOBAL_VAG_DIRS.iter().map(|d| dir_len(&data.join("global").join(d))).sum::<u64>()
                + file_len(&data.join("global/sound_bank.bin")) + file_len(&data.join("global/music.bin"));
            (b, 4 * b)
        }
        Job::GlobalText => (file_len(&data.join("global/all_text.bin")), 2 * file_len(&data.join("global/all_text.bin"))),
        Job::GlobalTextures => (dir_len(&data.join("global/hud_banks")) + 1, 4 * dir_len(&data.join("global/hud_banks"))),
    }
}

fn run_job(data: &Path, out: &Out, kinds: Kinds, j: &Job) -> Result<(), Error> {
    match *j {
        Job::Textures(l) => textures::export_level(data, out, l),
        Job::Audio(l) => audio::export_level(data, out, l),
        Job::Levels(l) => { geometry::export_level(data, out, l)?; tables::export_level(data, out, l) }
        Job::Collision(l) => geometry::export_collision(data, out, l),
        Job::Models(l) => models::export_level(data, out, l),
        Job::Text(l) => text::export_level(data, out, l),
        Job::GlobalAudio => audio::export_global(data, out),
        Job::GlobalText => text::export_global(data, out),
        Job::GlobalTextures => textures::export_global(data, out, kinds),
    }
}

/// Exports from the Tier 0 archive in `data` into `opts.to`.
pub fn export(data: &Path, opts: &Options, emit: Emit) -> Result<Exported, Error> {
    if !data.join("toc.bin").is_file() {
        return Err(Error::new(Code::CannotRead, format!("{} is not a Ratchet & Clank Recomp data folder (no toc.bin); extract the disc first", data.display())));
    }
    let all_levels = archive_levels(data);
    let levels: Vec<u32> = match opts.level {
        Some(l) if all_levels.contains(&l) => vec![l],
        Some(l) => return Err(Error::new(Code::CannotRead, format!("level {l:02} is not in the archive at {}", data.display()))),
        None => all_levels,
    };
    let mut kinds = opts.kinds;
    // Levels and models reference the level's textures.
    kinds.textures |= kinds.levels || kinds.models;
    let mut jobs = Vec::new();
    for &l in &levels {
        if kinds.levels { jobs.push(Job::Levels(l)); }
        if kinds.models { jobs.push(Job::Models(l)); }
        if kinds.textures { jobs.push(Job::Textures(l)); }
        if kinds.collision { jobs.push(Job::Collision(l)); }
        if kinds.audio { jobs.push(Job::Audio(l)); }
        if kinds.text { jobs.push(Job::Text(l)); }
    }
    if opts.level.is_none() {
        if kinds.audio { jobs.push(Job::GlobalAudio); }
        if kinds.text { jobs.push(Job::GlobalText); }
        if kinds.textures { jobs.push(Job::GlobalTextures); }
    }
    let weights: Vec<(u64, u64)> = jobs.iter().map(|j| job_weight(data, j)).collect();
    let total: u64 = weights.iter().map(|w| w.0.max(1)).sum();

    std::fs::create_dir_all(&opts.to).map_err(|e| workers::write_error(&opts.to, e))?;
    let info = opts.to.join(INFO_FILE);
    if info.exists() { std::fs::remove_file(&info).map_err(|e| workers::write_error(&info, e))?; }
    let stale = remove_partials(&opts.to);
    if stale > 0 { emit(Event::Info(format!("removed {stale} unfinished file(s) of an interrupted export"))); }
    let need: u64 = weights.iter().map(|w| w.1).sum::<u64>() + crate::extract::SPACE_MARGIN;
    match crate::space::available_bytes(&opts.to) {
        Some(free) if free < need => {
            return Err(Error::new(Code::NoSpace, format!(
                "not enough free space at {} for the export: about {:.0} MiB needed, {:.0} MiB available", opts.to.display(), mib(need), mib(free))));
        }
        Some(_) => {}
        None => emit(Event::Info("free disk space unknown on this platform; not checked".into())),
    }
    emit(Event::Info(format!("exporting {} for {} level(s) to {}", kinds.names().join(", "), levels.len(), opts.to.display())));

    let out = Out::new(&opts.to);
    workers::run(
        jobs.len(), total, Stage::Export, opts.threads, opts.cancel, emit,
        |i| jobs[i].name(),
        || Ok(()),
        |_: &mut (), i, ctx: &Ctx| {
            run_job(data, &out, kinds, &jobs[i])?;
            ctx.add(weights[i].0.max(1));
            Ok(())
        },
    )?;

    let warnings = out.warnings.into_inner().unwrap_or_else(|e| e.into_inner());
    for w in warnings.iter().take(MAX_WARNING_LINES) { emit(Event::Info(format!("skipped: {w}"))); }
    if warnings.len() > MAX_WARNING_LINES { emit(Event::Info(format!("… and {} more skipped items", warnings.len() - MAX_WARNING_LINES))); }
    let by_folder = out.stats.into_inner().unwrap_or_else(|e| e.into_inner());
    let mut ex = Exported {
        files: by_folder.values().map(|v| v.0).sum(),
        bytes: by_folder.values().map(|v| v.1).sum(),
        by_folder,
        skipped: warnings.len(),
        levels: levels.clone(),
    };
    let disc = std::fs::read_to_string(data.join(crate::INFO_FILE)).ok()
        .and_then(|s| crate::json::parse_object(&s))
        .and_then(|o| match crate::json::get(&o, "disc") { Some(crate::json::Value::Str(s)) => Some(s.clone()), _ => None });
    let mut folders = Obj::new();
    for (k, (f, b)) in &ex.by_folder { folders.put(k, Obj::new().set("files", *f).set("bytes", *b)); }
    let doc = Obj::new()
        .set("format", EXPORT_FORMAT)
        .set("extractor_version", EXTRACTOR_VERSION)
        .set("disc", disc)
        .set("kinds", J::Arr(kinds.names().into_iter().map(J::from).collect()))
        .set("levels", J::Arr(levels.iter().map(|&l| J::from(l)).collect()))
        .set("global", opts.level.is_none())
        .set("files", ex.files)
        .set("bytes", ex.bytes)
        .set("folders", folders)
        .set("skipped", J::Arr(warnings.into_iter().map(J::from).collect()))
        .build();
    let tmp = crate::extract::partial_path(&info);
    std::fs::write(&tmp, doc.pretty()).map_err(|e| workers::write_error(&tmp, e))?;
    std::fs::rename(&tmp, &info).map_err(|e| workers::write_error(&info, e))?;
    ex.files += 1;
    Ok(ex)
}

/// Removes `*.partial` files a killed export left under `dir`.
pub fn remove_partials(dir: &Path) -> usize {
    let mut n = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if e.file_type().is_ok_and(|t| t.is_dir()) { stack.push(p); }
            else if p.to_string_lossy().ends_with(PARTIAL_SUFFIX) && std::fs::remove_file(&p).is_ok() { n += 1; }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_parse_lists_and_all() {
        assert_eq!(Kinds::parse("all").unwrap(), Kinds::ALL);
        let k = Kinds::parse("textures, audio").unwrap();
        assert!(k.textures && k.audio && !k.models && !k.levels && !k.collision && !k.text);
        assert_eq!(k.names(), ["textures", "audio"]);
        assert!(Kinds::parse("meshes").is_err());
        assert!(Kinds::parse("").is_err());
    }
}
