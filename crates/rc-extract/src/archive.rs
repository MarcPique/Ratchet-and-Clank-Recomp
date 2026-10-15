//! The Tier 0 archive plan: which files to write, from which bytes of the image, with which expected hash.
//! The file list itself comes from `rc_formats::disc::Disc::archive_files` (the lump rules of the retired C++
//! extractor's `unpack`, which the Tier 0 layout keeps).

use crate::build_db::TableEntry;
use crate::{Code, Error};
use rc_formats::disc::{Disc, DiscFile};
use std::collections::HashMap;
use std::io::{Read, Seek};

/// One file to copy (or hash).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    /// Archive-relative, `/`-separated (`global/mpegs/073.bin`).
    pub path: String,
    /// Byte offset in the image (2048-byte sectors, so = file offset).
    pub offset: u64,
    pub bytes: u64,
    /// Expected SHA-1 from the build's table.
    pub sha1: Option<[u8; 20]>,
}

/// PAL FMVs in `global/mpegs/` (docs/plan/cutscenes_transitions.md §5: PAL copies of the in-level story,
/// transition, extras and attract movies). 1,333 MiB on SCUS_971.99.
const PAL_MPEGS: [std::ops::RangeInclusive<u32>; 4] = [21..=39, 52..=62, 75..=79, 84..=87];

/// True for the PAL copies the NTSC-U game never reads, skipped by `--ntsc-only`: `levels/NN/gameplay_pal.bin`,
/// `levels/NN/scene/KK_pal.bin`, `global/credits_images_pal/*` and the PAL FMVs.
pub fn is_pal_copy(path: &str) -> bool {
    if let Some(rest) = path.strip_prefix("levels/") {
        return rest.ends_with("/gameplay_pal.bin") || (rest.contains("/scene/") && rest.ends_with("_pal.bin"));
    }
    if path.starts_with("global/credits_images_pal/") || path == "global/credits_images_pal.bin" { return true; }
    if let Some(n) = path.strip_prefix("global/mpegs/").and_then(|r| r.strip_suffix(".bin")).and_then(|n| n.parse::<u32>().ok()) {
        return PAL_MPEGS.iter().any(|r| r.contains(&n));
    }
    false
}

/// The disc's archive files (`Disc::archive_files`), as jobs.
pub fn disc_jobs<R: Read + Seek>(disc: &Disc<R>) -> Result<Vec<Job>, Error> {
    let files = disc.archive_files().map_err(|e| Error::new(Code::CannotRead, format!("cannot read the disc's table of contents: {e}")))?;
    Ok(files.into_iter().map(|DiscFile { path, offset, bytes }| Job { path, offset, bytes, sha1: None }).collect())
}

/// Attaches the table's hashes to the disc plan. The plan comes from the disc's own ToC, so a disagreement in the
/// file list or a size means a damaged image (code 40). `ntsc_only` then drops the PAL copies.
pub fn with_table(jobs: Vec<Job>, table: &[TableEntry], ntsc_only: bool) -> Result<Vec<Job>, Error> {
    let by_path: HashMap<&str, &TableEntry> = table.iter().map(|e| (e.path.as_str(), e)).collect();
    let mut problems = Vec::new();
    let mut out = Vec::with_capacity(jobs.len());
    for mut j in jobs {
        match by_path.get(j.path.as_str()) {
            None => problems.push(format!("{} is not in the build table", j.path)),
            Some(e) if e.size != j.bytes => problems.push(format!("{}: {} bytes on this disc, {} expected", j.path, j.bytes, e.size)),
            Some(e) => j.sha1 = Some(e.sha1),
        }
        out.push(j);
    }
    let planned: std::collections::HashSet<&str> = out.iter().map(|j| j.path.as_str()).collect();
    problems.extend(table.iter().filter(|e| !planned.contains(e.path.as_str())).map(|e| format!("{} is missing from this disc's table of contents", e.path)));
    if !problems.is_empty() {
        let n = problems.len();
        problems.truncate(5);
        return Err(Error::new(Code::VerifyFailed, format!(
            "the disc's table of contents does not match this build ({n} differences: {}{}); the image is damaged",
            problems.join("; "), if n > 5 { "; …" } else { "" })));
    }
    if ntsc_only { out.retain(|j| !is_pal_copy(&j.path)); }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pal_copies() {
        for p in ["levels/01/gameplay_pal.bin", "levels/05/scene/03_pal.bin", "global/credits_images_pal/007.bin", "global/mpegs/021.bin",
                  "global/mpegs/039.bin", "global/mpegs/052.bin", "global/mpegs/062.bin", "global/mpegs/075.bin", "global/mpegs/087.bin"] {
            assert!(is_pal_copy(p), "{p}");
        }
        for p in ["levels/01/gameplay_ntsc.bin", "levels/05/scene/03_ntsc.bin", "levels/05/speech/03_fr.bin", "global/credits_images_ntsc/007.bin",
                  "global/mpegs/020.bin", "global/mpegs/040.bin", "global/mpegs/051.bin", "global/mpegs/063.bin", "global/mpegs/074.bin",
                  "global/mpegs/080.bin", "global/help_audio/030.bin", "toc.bin", "boot/SCUS_971.99"] {
            assert!(!is_pal_copy(p), "{p}");
        }
    }

    #[test]
    fn table_join() {
        let job = |p: &str, n: u64| Job { path: p.into(), offset: 0, bytes: n, sha1: None };
        let entry = |p: &str, n: u64| TableEntry { path: p.into(), size: n, sha1: [n as u8; 20] };
        let table = [entry("toc.bin", 3), entry("levels/01/gameplay_pal.bin", 4)];
        let jobs = with_table(vec![job("toc.bin", 3), job("levels/01/gameplay_pal.bin", 4)], &table, false).unwrap();
        assert_eq!(jobs[1].sha1, Some([4; 20]));
        assert_eq!(with_table(jobs.clone(), &table, true).unwrap().len(), 1);
        let e = with_table(vec![job("toc.bin", 5)], &table, false).unwrap_err();
        assert_eq!(e.code, Code::VerifyFailed);
        assert!(e.message.contains("2 differences"), "{}", e.message);
    }
}
