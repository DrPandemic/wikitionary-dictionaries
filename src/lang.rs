//! Per-language configuration.
//!
//! Each target maps to the Wiktionary edition whose dump we download, plus the
//! [`Product`] to build from it. French pulls `frwiktionary` for its conjugation
//! companion; Italian pulls `itwiktionary` for a full monolingual definition
//! dictionary; English **and** Italian conjugation companions are both built
//! from the one `enwiktionary` dump (it carries full per-person conjugation
//! grids for every language, which the native editions don't).
//!
//! A spec is resolved by its **`id`** (the asset stem, e.g. `it-conj`), not the
//! bare language code, because one code can have several products (Italian has
//! both `it-it` definitions and `it-conj` conjugation).

use std::path::PathBuf;

use crate::model::ConjShape;

/// What kind of dictionary we build from a language's dump.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Product {
    /// Verb conjugation tables (companion dictionary). The [`ConjShape`] selects
    /// how forms are extracted and which language's grid/labels to render.
    Conjugation(ConjShape),
    /// Full monolingual headword/definition entries.
    Definitions,
}

/// A dictionary we can build: a source edition plus the product to extract.
pub struct LangSpec {
    /// Wiktextract `lang_code` to filter the dump by (e.g. `"fr"`).
    pub code: &'static str,
    /// kaikki edition slug whose raw dump we download (e.g. `"frwiktionary"`).
    /// Shared editions (several specs naming the same one) download once.
    pub edition: &'static str,
    /// Human-readable language name (e.g. `"Français"`).
    pub label: &'static str,
    /// StarDict id / release-asset stem (e.g. `"fr-conj"`, `"it-it"`). The
    /// resolution key, and the build/package output name.
    pub id: &'static str,
    /// Which dictionary to build from this edition.
    pub product: Product,
}

impl LangSpec {
    /// URL of the raw wiktextract dump for this language's edition. kaikki
    /// publishes the main English edition under `dictionary/`; the non-English
    /// editions under their own `<edition>` slug.
    pub fn dump_url(&self) -> String {
        let slug = match self.edition {
            "enwiktionary" => "dictionary",
            other => other,
        };
        format!("https://kaikki.org/{slug}/raw-wiktextract-data.jsonl.gz")
    }

    /// Local path the downloaded dump is stored at — keyed by **edition** so
    /// products sharing an edition (e.g. `en-conj` and `it-conj` on
    /// `enwiktionary`) reuse a single download.
    pub fn dump_path(&self) -> PathBuf {
        edition_dir(self.edition).join("raw-wiktextract-data.jsonl.gz")
    }
}

/// Working directory for a language's build/package artifacts (`./data/<code>`).
pub fn data_dir(code: &str) -> PathBuf {
    PathBuf::from("data").join(code)
}

/// Directory holding an edition's raw dump (`./data/<edition>`). Separate from
/// [`data_dir`] (keyed by `code`) so a shared dump isn't tied to one product.
pub fn edition_dir(edition: &str) -> PathBuf {
    PathBuf::from("data").join(edition)
}

/// Resolve a dictionary id (e.g. `fr-conj`, `it-it`, `it-conj`) to its spec, or
/// `None` if unsupported.
pub fn resolve(id: &str) -> Option<&'static LangSpec> {
    SUPPORTED.iter().find(|l| l.id == id)
}

/// The ids we can build, for help/error messages.
pub fn supported_ids() -> Vec<&'static str> {
    SUPPORTED.iter().map(|l| l.id).collect()
}

/// Dictionaries currently supported. French conjugation companion (frwiktionary),
/// Italian monolingual definitions (itwiktionary), and English + Italian
/// conjugation companions (both from enwiktionary's per-person grids).
static SUPPORTED: &[LangSpec] = &[
    LangSpec {
        code: "fr",
        edition: "frwiktionary",
        label: "Français",
        id: "fr-conj",
        product: Product::Conjugation(ConjShape::FrenchSource),
    },
    LangSpec {
        code: "it",
        edition: "itwiktionary",
        label: "Italiano",
        id: "it-it",
        product: Product::Definitions,
    },
    LangSpec {
        code: "it",
        edition: "enwiktionary",
        label: "Italiano",
        id: "it-conj",
        product: Product::Conjugation(ConjShape::Italian),
    },
    LangSpec {
        code: "en",
        edition: "enwiktionary",
        label: "English",
        id: "en-conj",
        product: Product::Conjugation(ConjShape::English),
    },
];
