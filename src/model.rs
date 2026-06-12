//! Conjugation model and the locked tag→label table.
//!
//! A [`Conjugation`] is a verb infinitive plus its mood/tense [`Section`]s in
//! canonical display order. Sections are derived from kaikki `forms[]`: each
//! form carries a `tags` set that we match — as a whole, order-independent set —
//! against the 22 blocks below. Anything that does not match exactly is noise
//! (archaic/pronominal/empty-tag variants) and is dropped.

/// One mood/tense block with its ordered person forms (already display-ready).
pub struct Section {
    pub label: &'static str,
    pub forms: Vec<String>,
}

/// A verb's full conjugation grid, sections in canonical order.
pub struct Conjugation {
    pub infinitive: String,
    pub sections: Vec<Section>,
}

/// One of the 22 mood/tense blocks: its kaikki tag-set (stored **sorted**) and
/// the French section label shown to the reader.
struct Block {
    /// kaikki `tags`, sorted ascending so we can compare against a sorted input.
    tags: &'static [&'static str],
    label: &'static str,
}

/// The 22 blocks, in display order. Derived from the real frwiktionary dump
/// (verified against `lire`); see PLAN.md for the source table. Each `tags`
/// slice is kept sorted ascending — keep it that way (a debug assert checks it).
static BLOCKS: &[Block] = &[
    // Indicatif
    Block { tags: &["indicative", "present"], label: "Indicatif présent" },
    Block { tags: &["imperfect", "indicative"], label: "Indicatif imparfait" },
    Block { tags: &["indicative", "past"], label: "Indicatif passé simple" },
    Block { tags: &["future", "indicative"], label: "Indicatif futur simple" },
    Block { tags: &["indicative", "multiword-construction", "past"], label: "Indicatif passé composé" },
    Block { tags: &["indicative", "pluperfect"], label: "Indicatif plus-que-parfait" },
    Block { tags: &["anterior", "indicative", "past"], label: "Indicatif passé antérieur" },
    Block { tags: &["future", "indicative", "perfect"], label: "Indicatif futur antérieur" },
    // Subjonctif
    Block { tags: &["present", "subjunctive"], label: "Subjonctif présent" },
    Block { tags: &["imperfect", "subjunctive"], label: "Subjonctif imparfait" },
    Block { tags: &["past", "subjunctive"], label: "Subjonctif passé" },
    Block { tags: &["pluperfect", "subjunctive"], label: "Subjonctif plus-que-parfait" },
    // Conditionnel
    Block { tags: &["conditional", "present"], label: "Conditionnel présent" },
    Block { tags: &["conditional", "past"], label: "Conditionnel passé" },
    // Impératif
    Block { tags: &["imperative", "present"], label: "Impératif présent" },
    Block { tags: &["imperative", "past"], label: "Impératif passé" },
    // Infinitif
    Block { tags: &["infinitive", "present"], label: "Infinitif présent" },
    Block { tags: &["infinitive", "past"], label: "Infinitif passé" },
    // Gérondif
    Block { tags: &["gerund", "present"], label: "Gérondif présent" },
    Block { tags: &["gerund", "past"], label: "Gérondif passé" },
    // Participe
    Block { tags: &["participle", "present"], label: "Participe présent" },
    Block { tags: &["participle", "past"], label: "Participe passé" },
];

/// Index of the block whose tag-set exactly equals `tags`, ignoring order.
/// `None` for any tag-set that is not one of the 22 (i.e. noise to drop).
fn block_index(tags: &[String]) -> Option<usize> {
    let mut sorted: Vec<&str> = tags.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    BLOCKS.iter().position(|b| b.tags == sorted.as_slice())
}

impl Conjugation {
    /// Group `forms` (each a `(tags, form)` pair from the dump) into the canonical
    /// sections. Forms whose tag-set is not one of the 22 are dropped; form text
    /// is normalized; exact duplicates within a block are collapsed.
    pub fn from_forms(infinitive: String, forms: &[(Vec<String>, String)]) -> Conjugation {
        debug_assert!(
            BLOCKS.iter().all(|b| b.tags.windows(2).all(|w| w[0] <= w[1])),
            "BLOCKS tag-sets must be stored sorted",
        );

        let mut buckets: Vec<Vec<String>> = (0..BLOCKS.len()).map(|_| Vec::new()).collect();
        for (tags, form) in forms {
            if let Some(i) = block_index(tags) {
                let text = normalize_form(form);
                if !text.is_empty() && !buckets[i].contains(&text) {
                    buckets[i].push(text);
                }
            }
        }

        let sections = buckets
            .into_iter()
            .enumerate()
            .filter(|(_, forms)| !forms.is_empty())
            .map(|(i, forms)| Section { label: BLOCKS[i].label, forms })
            .collect();

        Conjugation { infinitive, sections }
    }

    /// Total person-forms across all sections — used to pick the richest entry
    /// when the same infinitive appears more than once in the dump.
    pub fn total_forms(&self) -> usize {
        self.sections.iter().map(|s| s.forms.len()).sum()
    }

    /// The HTML entry body for `sametypesequence=h`: each tense as a bold heading
    /// followed by its `<br>`-separated person forms. `<br>` is the only line
    /// break — no literal newlines — so the body stays compact.
    pub fn to_html(&self) -> String {
        let mut out = String::new();
        for sec in &self.sections {
            out.push_str("<b>");
            push_escaped(&mut out, sec.label);
            out.push_str("</b><br>");
            for form in &sec.forms {
                push_escaped(&mut out, form);
                out.push_str("<br>");
            }
        }
        out
    }
}

/// Clean a raw `form` for display: trim, and fold the typographic apostrophe
/// (U+2019) to a plain `'` so forms copy/search/compare consistently.
fn normalize_form(form: &str) -> String {
    form.trim().replace('\u{2019}', "'")
}

/// Append `s` to `out`, escaping the three HTML-significant characters.
fn push_escaped(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn blocks_are_sorted() {
        for b in BLOCKS {
            assert!(b.tags.windows(2).all(|w| w[0] <= w[1]), "{:?} not sorted", b.tags);
        }
    }

    #[test]
    fn matches_ignore_tag_order() {
        // passé simple vs passé composé differ only by the extra tag.
        assert_eq!(block_index(&tags(&["past", "indicative"])), Some(2));
        assert_eq!(
            block_index(&tags(&["past", "multiword-construction", "indicative"])),
            Some(4),
        );
    }

    #[test]
    fn noise_tagsets_are_dropped() {
        assert_eq!(block_index(&tags(&["pronominal", "present"])), None);
        assert_eq!(block_index(&tags(&[])), None);
    }

    #[test]
    fn groups_and_normalizes() {
        let forms = vec![
            (tags(&["indicative", "present"]), "je mange".into()),
            (tags(&["indicative", "present"]), "tu manges".into()),
            (tags(&["past", "indicative", "multiword-construction"]), "j\u{2019}ai mangé".into()),
            (tags(&["archaic"]), "noise".into()),
        ];
        let c = Conjugation::from_forms("manger".into(), &forms);
        assert_eq!(c.sections.len(), 2);
        assert_eq!(c.sections[0].label, "Indicatif présent");
        assert_eq!(c.sections[0].forms, vec!["je mange", "tu manges"]);
        // passé composé sorts after présent and keeps the straight apostrophe.
        assert_eq!(c.sections[1].label, "Indicatif passé composé");
        assert_eq!(c.sections[1].forms, vec!["j'ai mangé"]);
        assert_eq!(c.total_forms(), 3);
    }
}
