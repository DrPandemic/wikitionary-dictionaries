//! Per-language configuration.
//!
//! Each target language maps to the Wiktionary edition whose Conjugaison
//! namespace holds its conjugation tables. French tables live in fr.wiktionary,
//! so the French target pulls the `frwiktionary` extract.

use std::path::PathBuf;

/// A language we can build a conjugation dictionary for.
pub struct LangSpec {
    /// Wiktextract `lang_code` to filter the dump by (e.g. `"fr"`).
    pub code: &'static str,
    /// kaikki edition slug whose raw dump we download (e.g. `"frwiktionary"`).
    pub edition: &'static str,
    /// Human-readable language name (e.g. `"Français"`).
    pub label: &'static str,
    /// StarDict id / release-asset stem (e.g. `"fr-conj"`). Used by the build /
    /// package stages.
    pub id: &'static str,
}

impl LangSpec {
    /// URL of the raw wiktextract dump for this language's edition.
    pub fn dump_url(&self) -> String {
        format!(
            "https://kaikki.org/{}/raw-wiktextract-data.jsonl.gz",
            self.edition
        )
    }

    /// Local path the downloaded dump is stored at.
    pub fn dump_path(&self) -> PathBuf {
        data_dir(self.code).join("raw-wiktextract-data.jsonl.gz")
    }
}

/// Working directory for a language's pipeline artifacts (`./data/<code>`).
pub fn data_dir(code: &str) -> PathBuf {
    PathBuf::from("data").join(code)
}

/// Resolve a language code to its spec, or `None` if unsupported.
pub fn resolve(code: &str) -> Option<&'static LangSpec> {
    SUPPORTED.iter().find(|l| l.code == code)
}

/// Languages currently supported. French first; more to follow.
static SUPPORTED: &[LangSpec] = &[LangSpec {
    code: "fr",
    edition: "frwiktionary",
    label: "Français",
    id: "fr-conj",
}];
