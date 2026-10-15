//! `verify`: re-hash a data folder against the built-in size/SHA-1 table of the disc named in `extract-info.json`.

use crate::archive::is_pal_copy;
use crate::build_db::Build;
use crate::extract::{build_table, fail_with};
use crate::json::{self, Value};
use crate::workers::{self, Ctx, CHUNK};
use crate::{Code, Emit, Error, Stage, Summary, DATA_FORMAT, INFO_FILE};
use std::fs::File;
use std::path::Path;
use std::sync::atomic::AtomicBool;

/// What `extract-info.json` says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractInfo {
    pub disc: String,
    pub data_format: i64,
    pub extractor_version: String,
    pub ntsc_only: bool,
    pub files: i64,
    pub bytes: i64,
}

pub fn read_info(out: &Path) -> Result<ExtractInfo, Error> {
    let p = out.join(INFO_FILE);
    let bad = |why: &str| Error::new(Code::VerifyFailed, format!("{}: {why}; the extraction is incomplete or this is not a Ratchet & Clank Recomp data folder", p.display()));
    let text = std::fs::read_to_string(&p).map_err(|e| bad(&e.to_string()))?;
    let obj = json::parse_object(&text).ok_or_else(|| bad("not a JSON object"))?;
    let s = |k: &str| match json::get(&obj, k) { Some(Value::Str(s)) => Ok(s.clone()), _ => Err(bad(&format!("no \"{k}\""))) };
    let n = |k: &str| match json::get(&obj, k) { Some(Value::Num(n)) => Ok(*n), _ => Err(bad(&format!("no \"{k}\""))) };
    let ntsc_only = match json::get(&obj, "ntsc_only") { Some(Value::Bool(b)) => *b, _ => return Err(bad("no \"ntsc_only\"")) };
    Ok(ExtractInfo { disc: s("disc")?, data_format: n("data_format")?, extractor_version: s("extractor_version")?, ntsc_only, files: n("files")?, bytes: n("bytes")? })
}

pub fn verify(out: &Path, builds: &[Build], threads: usize, cancel: Option<&AtomicBool>, emit: Emit) -> Result<(ExtractInfo, Summary), Error> {
    let info = read_info(out)?;
    if info.data_format != DATA_FORMAT as i64 {
        return Err(Error::new(Code::VerifyFailed, format!("data format {} in {INFO_FILE}; this extractor checks format {DATA_FORMAT}", info.data_format)));
    }
    let n = crate::build_db::normalize_serial(&info.disc);
    let build = builds.iter().find(|b| b.supported && b.table.is_some() && crate::build_db::normalize_serial(b.serial) == n)
        .ok_or_else(|| Error::new(Code::VerifyFailed, format!("{INFO_FILE} names disc {:?}, which has no file table", info.disc)))?;
    let mut table = build_table(build)?;
    if info.ntsc_only { table.retain(|e| !is_pal_copy(&e.path)); }
    let total = table.iter().map(|e| e.size).sum();
    let results = workers::run(
        table.len(), total, Stage::Verify, threads, cancel, emit,
        |i| table[i].path.clone(),
        || Ok(vec![0u8; CHUNK]),
        |buf: &mut Vec<u8>, i, ctx: &Ctx| {
            let e = &table[i];
            let p = out.join(&e.path);
            let mut f = match File::open(&p) { Ok(f) => f, Err(_) => return Ok(Some(format!("{}: missing", e.path))) };
            let len = f.metadata().map(|m| m.len()).unwrap_or(0);
            if len != e.size {
                ctx.add(e.size);
                return Ok(Some(format!("{}: {len} bytes, {} expected", e.path, e.size)));
            }
            match workers::stream(&mut f, e.size, buf, ctx, |err| Error::new(Code::VerifyFailed, err.to_string()), |_| Ok(())) {
                Ok(h) if h == e.sha1 => Ok(None),
                Ok(h) => Ok(Some(format!("{}: SHA-1 {} differs from the table", e.path, crate::sha1::hex(&h)))),
                Err(err) if err.code == Code::VerifyFailed => Ok(Some(format!("{}: cannot read ({})", e.path, err.message))),
                Err(err) => Err(err),
            }
        },
    )?;
    let problems: Vec<String> = results.into_iter().flatten().collect();
    if !problems.is_empty() { return Err(fail_with(&problems, "file(s) failed verification; re-extract from the disc image", emit)); }
    let summary = Summary { files: table.len() as u64, bytes: total };
    Ok((info, summary))
}
