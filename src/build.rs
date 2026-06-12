//! Stages 1–3 (filter, normalize, group) plus the StarDict emit.
//!
//! Streams the gzipped wiktextract dump, keeps French verbs that carry a
//! `Conjugaison:` form source, groups their forms into the canonical grid, and
//! writes a StarDict set to `data/<code>/<id>/`. The plaintext is never
//! materialized — we decompress on the fly and progress against the *compressed*
//! bytes consumed, which gives a true ETA.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

use anyhow::{bail, Context, Result};
use flate2::read::MultiGzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;

use crate::lang::{data_dir, LangSpec};
use crate::model::Conjugation;
use crate::stardict::{self, DictMeta};

/// A dump line we care about. Unused fields are ignored by serde.
#[derive(Deserialize)]
struct DumpEntry {
    word: String,
    #[serde(default)]
    lang_code: String,
    #[serde(default)]
    pos: String,
    #[serde(default)]
    forms: Vec<DumpForm>,
}

#[derive(Deserialize)]
struct DumpForm {
    #[serde(default)]
    form: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    source: String,
}

pub fn run(lang: &LangSpec) -> Result<()> {
    let dump = lang.dump_path();
    if !dump.exists() {
        bail!(
            "dump not found at {} — run `fetch {}` first",
            dump.display(),
            lang.code
        );
    }

    let file = File::open(&dump).with_context(|| format!("opening {}", dump.display()))?;
    let total = file.metadata()?.len();

    println!("Building {} conjugation dictionary", lang.label);

    let pb = byte_bar(total);
    // Progress tracks the compressed bytes read off disk, *under* the decoder.
    let counted = pb.wrap_read(file);
    let reader = BufReader::with_capacity(1 << 20, MultiGzDecoder::new(counted));

    let mut verbs: Vec<Conjugation> = Vec::new();
    let mut by_infinitive: HashMap<String, usize> = HashMap::new();
    let mut matched = 0usize;

    for line in reader.lines() {
        let line = line.context("reading dump")?;
        // Cheap prefilter: skip the JSON parse unless a conjugation source is
        // present at all. Cuts the ~6 GB stream down to the verb lines.
        if !line.contains("\"Conjugaison:") {
            continue;
        }
        let entry: DumpEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.lang_code != lang.code || entry.pos != "verb" {
            continue;
        }

        let forms: Vec<(Vec<String>, String)> = entry
            .forms
            .into_iter()
            .filter(|f| f.source.starts_with("Conjugaison:"))
            .map(|f| (f.tags, f.form))
            .collect();
        if forms.is_empty() {
            continue;
        }

        let conj = Conjugation::from_forms(entry.word, &forms);
        if conj.sections.is_empty() {
            continue;
        }
        matched += 1;
        pb.set_message(format!("{matched} verbs"));

        // The same infinitive can appear under several etymologies; keep the
        // entry with the most forms.
        match by_infinitive.get(&conj.infinitive) {
            Some(&i) if verbs[i].total_forms() >= conj.total_forms() => {}
            Some(&i) => verbs[i] = conj,
            None => {
                by_infinitive.insert(conj.infinitive.clone(), verbs.len());
                verbs.push(conj);
            }
        }
    }
    pb.finish_and_clear();
    println!(
        "Filtered {} verb entries → {} unique infinitives",
        matched,
        verbs.len()
    );

    if verbs.is_empty() {
        bail!("no conjugation entries found — is this the right dump for `{}`?", lang.code);
    }

    // Stage 4 — render each verb to its HTML entry.
    let render = count_bar(verbs.len() as u64, "rendering");
    let entries: Vec<(String, String)> = verbs
        .iter()
        .map(|c| {
            render.inc(1);
            (c.infinitive.clone(), c.to_html())
        })
        .collect();
    render.finish_and_clear();

    let out_dir = data_dir(lang.code).join(lang.id);
    let bookname = format!("Conjugaison — {}", lang.label);
    let meta = DictMeta {
        bookname: &bookname,
        author: "Wiktionary contributors",
        description: "Verb conjugation tables from Wiktionary via kaikki.org / wiktextract (CC BY-SA 4.0).",
        date: "",
        sametypesequence: "h",
    };
    let written = stardict::write(&out_dir, lang.id, &meta, &entries)?;

    println!("Wrote {} entries to {}/{}.{{ifo,idx,dict}}", written, out_dir.display(), lang.id);
    Ok(())
}

/// A byte-denominated progress bar (compressed bytes of the dump) with ETA and a
/// running matched-verb count in the message slot.
fn byte_bar(total: u64) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::with_template(
            "{percent:>3}% [{bar:40}] {bytes}/{total_bytes} {bytes_per_sec} ({eta}) {msg}",
        )
        .expect("valid template")
        .progress_chars("=> "),
    );
    pb
}

/// A count-denominated progress bar with ETA, labelled by `what`.
fn count_bar(total: u64, what: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::with_template(&format!(
            "{{percent:>3}}% [{{bar:40}}] {{pos}}/{{len}} {what} ({{eta}})"
        ))
        .expect("valid template")
        .progress_chars("=> "),
    );
    pb
}
