//! Stage 0 — Acquire.
//!
//! Downloads the raw wiktextract dump for a language's Wiktionary edition to
//! `data/<edition>/`, showing a byte progress bar with an ETA. The compressed
//! file is kept as-is; the build stage stream-decompresses it (we never write
//! the multi-GB plaintext to disk).
//!
//! The download is **resumable**: it streams into a `.partial` file and renames
//! it into place only on success, so a half-finished download is never mistaken
//! for a complete one. Re-running `fetch` continues a leftover `.partial` from
//! where it stopped via an HTTP `Range` request (these dumps are big and the
//! server is slow), falling back to a fresh download if the server ignores the
//! range or the partial is unusable.

use std::fs::{self, OpenOptions};
use std::io;

use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};

use crate::lang::LangSpec;

pub fn run(lang: &LangSpec) -> Result<()> {
    let url = lang.dump_url();
    let dest = lang.dump_path();
    fs::create_dir_all(dest.parent().expect("dump path has a parent"))
        .with_context(|| format!("creating {}", dest.parent().unwrap().display()))?;

    // Stream into `<dest>.partial`; rename into place only once complete.
    let tmp = dest.with_extension("gz.partial");
    let have = fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);

    println!("Fetching {} dump\n  {}", lang.label, url);
    if have > 0 {
        println!("  resuming from {have} bytes already downloaded");
    }

    // Ask the server to continue from where the partial left off.
    let req = ureq::get(&url);
    let req = if have > 0 {
        req.set("Range", &format!("bytes={have}-"))
    } else {
        req
    };
    let resp = match req.call() {
        Ok(resp) => resp,
        // 416 Range Not Satisfiable: our partial is already at/past the server's
        // size — most likely already complete. Start over cleanly to be safe.
        Err(ureq::Error::Status(416, _)) if have > 0 => {
            fs::remove_file(&tmp).ok();
            return run(lang);
        }
        Err(err) => return Err(err).with_context(|| format!("requesting {url}")),
    };

    // 206 = the server honored the range and is sending only the remaining bytes;
    // anything else (typically 200) means it's sending the whole file afresh.
    let resuming = resp.status() == 206 && have > 0;
    let already = if resuming { have } else { 0 };
    let remaining: u64 = resp
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    // Prefer the authoritative total from `Content-Range: bytes a-b/total`.
    let total = resp
        .header("Content-Range")
        .and_then(content_range_total)
        .unwrap_or_else(|| already + remaining);

    let pb = byte_bar(total);
    pb.set_position(already);
    let mut reader = pb.wrap_read(resp.into_reader());

    // Append when resuming a honored range; otherwise truncate and write fresh.
    let mut out = OpenOptions::new()
        .create(true)
        .write(true)
        .append(resuming)
        .truncate(!resuming)
        .open(&tmp)
        .with_context(|| format!("opening {}", tmp.display()))?;
    let written = io::copy(&mut reader, &mut out).context("downloading dump")?;
    out.sync_all().ok();
    pb.finish_and_clear();

    let copied = already + written;
    if total != 0 && copied != total {
        // Leave the `.partial` in place so the next run resumes rather than
        // restarts.
        bail!("short download: got {copied} of {total} bytes (re-run `fetch` to resume)");
    }
    fs::rename(&tmp, &dest)
        .with_context(|| format!("moving {} -> {}", tmp.display(), dest.display()))?;

    println!("Saved {} ({} bytes) to {}", lang.label, copied, dest.display());
    Ok(())
}

/// Parse the total size out of a `Content-Range: bytes <start>-<end>/<total>`
/// header (the part after `/`). `None` if it's absent or `*` (unknown).
fn content_range_total(header: &str) -> Option<u64> {
    header.rsplit('/').next()?.trim().parse().ok()
}

/// A byte-denominated progress bar styled with percent, throughput, and ETA.
fn byte_bar(total: u64) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::with_template(
            "{percent:>3}% [{bar:40}] {bytes}/{total_bytes} {bytes_per_sec} ({eta})",
        )
        .expect("valid template")
        .progress_chars("=> "),
    );
    pb
}
