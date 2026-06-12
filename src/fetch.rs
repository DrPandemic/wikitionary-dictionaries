//! Stage 0 — Acquire.
//!
//! Downloads the raw wiktextract dump for a language's Wiktionary edition to
//! `data/<code>/`, showing a byte progress bar with an ETA. The compressed file
//! is kept as-is; the build stage stream-decompresses it (we never write the
//! ~6.2 GB plaintext to disk).

use std::fs::{self, File};
use std::io;

use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};

use crate::lang::LangSpec;

pub fn run(lang: &LangSpec) -> Result<()> {
    let url = lang.dump_url();
    let dest = lang.dump_path();
    fs::create_dir_all(dest.parent().expect("dump path has a parent"))
        .with_context(|| format!("creating {}", dest.parent().unwrap().display()))?;

    println!("Fetching {} dump\n  {}", lang.label, url);

    let resp = ureq::get(&url)
        .call()
        .with_context(|| format!("requesting {url}"))?;

    let total: u64 = resp
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let pb = byte_bar(total);
    let mut reader = pb.wrap_read(resp.into_reader());

    // Write to a temp file first so an interrupted download never looks complete.
    let tmp = dest.with_extension("gz.partial");
    let mut out = File::create(&tmp).with_context(|| format!("creating {}", tmp.display()))?;
    let copied = io::copy(&mut reader, &mut out).context("downloading dump")?;
    out.sync_all().ok();
    pb.finish_and_clear();

    if total != 0 && copied != total {
        bail!("short download: got {copied} of {total} bytes");
    }
    fs::rename(&tmp, &dest)
        .with_context(|| format!("moving {} -> {}", tmp.display(), dest.display()))?;

    println!("Saved {} ({} bytes) to {}", lang.label, copied, dest.display());
    Ok(())
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
