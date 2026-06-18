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

use crate::lang::{data_dir, LangSpec, Product};
use crate::model::{ConjShape, Conjugation};
use crate::model_def::{self, Entry, PosSection, Sense};
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

/// A dump line for the definitions build. One kaikki object = one part-of-speech
/// section of a headword.
#[derive(Deserialize)]
struct DefEntry {
    word: String,
    #[serde(default)]
    lang_code: String,
    /// Italian part-of-speech label ("Sostantivo", "Verbo", …).
    #[serde(default)]
    pos_title: String,
    #[serde(default)]
    senses: Vec<DefSense>,
    #[serde(default)]
    sounds: Vec<DefSound>,
    /// Etymology text(s); itwiktionary fills the plural array, not the singular.
    #[serde(default)]
    etymology_texts: Vec<String>,
}

#[derive(Deserialize)]
struct DefSense {
    #[serde(default)]
    glosses: Vec<String>,
    #[serde(default)]
    examples: Vec<DefExample>,
    #[serde(default)]
    form_of: Vec<DefFormOf>,
}

#[derive(Deserialize)]
struct DefExample {
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct DefFormOf {
    #[serde(default)]
    word: String,
}

#[derive(Deserialize)]
struct DefSound {
    #[serde(default)]
    ipa: String,
    /// A regional/usage qualifier; we prefer an unqualified pronunciation.
    #[serde(default)]
    sense: String,
    #[serde(default)]
    raw_tags: Vec<String>,
}

/// Dispatch on the language's product: conjugation companion or full definitions.
///
/// `lemmas_only` drops inflected-form sections from a definitions build (no
/// effect on a conjugation build, which has no such sections).
pub fn run(lang: &LangSpec, lemmas_only: bool) -> Result<()> {
    match lang.product {
        Product::Conjugation(shape) => build_conjugation(lang, shape),
        Product::Definitions => build_definitions(lang, lemmas_only),
    }
}

/// Open the language's dump, returning the file and its size for the byte bar.
fn open_dump(lang: &LangSpec) -> Result<(File, u64)> {
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
    Ok((file, total))
}

fn build_conjugation(lang: &LangSpec, shape: ConjShape) -> Result<()> {
    let (file, total) = open_dump(lang)?;

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
        // Cheap prefilter to skip the JSON parse for non-verb lines. The French
        // source path keys on the `Conjugaison:` form source; the person-tagged
        // path (en/it from enwiktionary) keys on the verb POS marker.
        let keep = match shape {
            ConjShape::FrenchSource => line.contains("\"Conjugaison:"),
            _ => line.contains("\"pos\": \"verb\""),
        };
        if !keep {
            continue;
        }
        let entry: DumpEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.lang_code != lang.code || entry.pos != "verb" {
            continue;
        }

        // French keeps only the `Conjugaison:`-sourced forms (the grid); the
        // person-tagged path keeps every form and lets the grid match decide.
        let forms: Vec<(Vec<String>, String)> = entry
            .forms
            .into_iter()
            .filter(|f| shape != ConjShape::FrenchSource || f.source.starts_with("Conjugaison:"))
            .map(|f| (f.tags, f.form))
            .collect();
        if forms.is_empty() {
            continue;
        }

        let conj = match shape {
            ConjShape::FrenchSource => Conjugation::from_forms(entry.word, &forms),
            shape => Conjugation::from_person_tagged(
                entry.word,
                &forms,
                shape.grid().expect("person-tagged shape has a grid"),
            ),
        };
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
    let bookname = format!("{} — {}", shape.book_prefix(), lang.label);
    let meta = DictMeta {
        bookname: &bookname,
        author: "Wiktionary contributors",
        description: "Verb conjugation tables from Wiktionary via kaikki.org / wiktextract (CC BY-SA 4.0).",
        date: "",
        sametypesequence: "h",
    };
    let written = stardict::write(&out_dir, lang.id, &meta, &entries)?;

    println!(
        "Wrote {} entries to {}/{}.{{ifo,idx,dict,dict.dz}}",
        written,
        out_dir.display(),
        lang.id
    );
    Ok(())
}

/// Build the full monolingual definition dictionary: keep every `lang_code`
/// object, turn each into a part-of-speech section, group sections by headword,
/// render to HTML, and write a StarDict set.
fn build_definitions(lang: &LangSpec, lemmas_only: bool) -> Result<()> {
    let (file, total) = open_dump(lang)?;

    if lemmas_only {
        println!("Building {} definition dictionary (lemmas only)", lang.label);
    } else {
        println!("Building {} definition dictionary", lang.label);
    }

    let pb = byte_bar(total);
    let counted = pb.wrap_read(file);
    let reader = BufReader::with_capacity(1 << 20, MultiGzDecoder::new(counted));

    let mut entries: Vec<Entry> = Vec::new();
    let mut by_word: HashMap<String, usize> = HashMap::new();
    let mut sections = 0usize;

    for line in reader.lines() {
        let line = line.context("reading dump")?;
        // Cheap prefilter: the dump carries foreign-word entries too; skip any
        // line that can't be an Italian object before the JSON parse.
        if !line.contains("\"lang_code\"") {
            continue;
        }
        let entry: DefEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.lang_code != lang.code {
            continue;
        }

        let Some(section) = to_section(&entry) else {
            continue;
        };
        // A lemmas-only build skips inflected forms (verb forms + "forma flessa"),
        // keeping the ~75k rich lemma entries over the ~520k full set.
        if lemmas_only && section.is_inflected_form() {
            continue;
        }
        sections += 1;
        pb.set_message(format!("{sections} sections"));

        // Group every object sharing a headword into one multi-section entry.
        match by_word.get(&entry.word) {
            Some(&i) => entries[i].sections.push(section),
            None => {
                by_word.insert(entry.word.clone(), entries.len());
                entries.push(Entry { word: entry.word, sections: vec![section] });
            }
        }
    }
    pb.finish_and_clear();
    println!(
        "Filtered {} sections → {} unique headwords",
        sections,
        entries.len()
    );

    if entries.is_empty() {
        bail!("no definition entries found — is this the right dump for `{}`?", lang.code);
    }

    // Render each headword to its HTML entry.
    let render = count_bar(entries.len() as u64, "rendering");
    let rendered: Vec<(String, String)> = entries
        .iter()
        .map(|e| {
            render.inc(1);
            (e.word.clone(), e.to_html())
        })
        .collect();
    render.finish_and_clear();

    let out_dir = data_dir(lang.code).join(lang.id);
    let bookname = format!("Wiktionary — {}", lang.label);
    let meta = DictMeta {
        bookname: &bookname,
        author: "Wiktionary contributors",
        description: "Monolingual dictionary from Wiktionary via kaikki.org / wiktextract (CC BY-SA 4.0).",
        date: "",
        sametypesequence: "h",
    };
    let written = stardict::write(&out_dir, lang.id, &meta, &rendered)?;

    println!(
        "Wrote {} entries to {}/{}.{{ifo,idx,dict,dict.dz}}",
        written,
        out_dir.display(),
        lang.id
    );
    Ok(())
}

/// Turn one kaikki object into a [`PosSection`], or `None` when it has no
/// part-of-speech label or nothing worth rendering.
fn to_section(entry: &DefEntry) -> Option<PosSection> {
    if entry.pos_title.is_empty() {
        return None;
    }
    let senses: Vec<Sense> = entry
        .senses
        .iter()
        .filter_map(|s| {
            let examples: Vec<String> = s
                .examples
                .iter()
                .map(|e| e.text.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            let form_of: Vec<String> = s
                .form_of
                .iter()
                .map(|f| f.word.trim().to_string())
                .filter(|w| !w.is_empty())
                .collect();
            Sense::build(&entry.word, &s.glosses, examples, form_of)
        })
        .collect();

    let etymology = {
        let joined = entry
            .etymology_texts
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        (!joined.is_empty()).then_some(joined)
    };

    let section = PosSection {
        pos_title: entry.pos_title.clone(),
        ipa: pick_ipa(&entry.sounds),
        etymology,
        senses,
    };
    (!section.is_empty()).then_some(section)
}

/// Pick the header IPA: the first unqualified transcription (no regional/usage
/// qualifier), falling back to the first with any IPA at all.
fn pick_ipa(sounds: &[DefSound]) -> Option<String> {
    let unqualified = sounds
        .iter()
        .find(|s| !s.ipa.trim().is_empty() && s.sense.is_empty() && s.raw_tags.is_empty());
    let chosen = unqualified.or_else(|| sounds.iter().find(|s| !s.ipa.trim().is_empty()))?;
    model_def::normalize_ipa(&chosen.ipa)
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
