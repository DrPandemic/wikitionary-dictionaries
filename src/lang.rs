//! Per-language configuration.
//!
//! Each target maps to the Wiktionary edition whose dump we download, plus the
//! [`Product`] to build from it. French pulls `frwiktionary` for its conjugation
//! companion; Italian pulls `itwiktionary` for a full monolingual definition
//! dictionary.

use std::path::PathBuf;

/// What kind of dictionary we build from a language's dump.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Product {
    /// Verb conjugation tables (companion dictionary).
    Conjugation,
    /// Full monolingual headword/definition entries.
    Definitions,
}

/// A dictionary we can build: a source edition plus the product to extract.
pub struct LangSpec {
    /// Wiktextract `lang_code` to filter the dump by (e.g. `"fr"`).
    pub code: &'static str,
    /// kaikki edition slug whose raw dump we download (e.g. `"frwiktionary"`).
    pub edition: &'static str,
    /// Human-readable language name (e.g. `"Français"`).
    pub label: &'static str,
    /// StarDict id / release-asset stem (e.g. `"fr-conj"`, `"it-it"`). Used by the
    /// build / package stages.
    pub id: &'static str,
    /// Which dictionary to build from this edition.
    pub product: Product,
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

/// Dictionaries currently supported. French conjugation companion + Italian
/// monolingual definitions; more to follow.
static SUPPORTED: &[LangSpec] = &[
    LangSpec {
        code: "fr",
        edition: "frwiktionary",
        label: "Français",
        id: "fr-conj",
        product: Product::Conjugation,
    },
    LangSpec {
        code: "it",
        edition: "itwiktionary",
        label: "Italiano",
        id: "it-it",
        product: Product::Definitions,
    },
];
