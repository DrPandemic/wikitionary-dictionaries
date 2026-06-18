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

/// How a language's conjugation is laid out in its source dump, selecting the
/// extraction + rendering path.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConjShape {
    /// `frwiktionary`: forms selected by `source: "Conjugaison:…"`, tagged by
    /// tense/mood only, with the subject pronoun already baked into the form
    /// (`"je parle"`). Rendered by [`Conjugation::from_forms`].
    FrenchSource,
    /// `enwiktionary` Italian: per-person tagged forms (`["first-person",
    /// "singular","present","indicative"]`) whose text is the bare verb form;
    /// the pronoun is synthesized on render. Uses [`ITALIAN_GRID`].
    Italian,
    /// `enwiktionary` English: principal parts only (English has no synthetic
    /// person grid). Uses [`ENGLISH_GRID`]. The rich periphrastic table would
    /// need synthesis (deferred).
    English,
}

impl ConjShape {
    /// The localized "Conjugation" word for the dictionary's bookname.
    pub fn book_prefix(self) -> &'static str {
        match self {
            ConjShape::FrenchSource => "Conjugaison",
            ConjShape::Italian => "Coniugazione",
            ConjShape::English => "Conjugation",
        }
    }

    /// The person-tagged grid for this shape, or `None` for the French
    /// source-tagged path.
    pub fn grid(self) -> Option<&'static PersonGrid> {
        match self {
            ConjShape::FrenchSource => None,
            ConjShape::Italian => Some(&ITALIAN_GRID),
            ConjShape::English => Some(&ENGLISH_GRID),
        }
    }
}

/// One of the 22 mood/tense blocks: its kaikki tag-set (stored **sorted**) and
/// the French section label shown to the reader.
struct Block {
    /// kaikki `tags`, sorted ascending so we can compare against a sorted input.
    tags: &'static [&'static str],
    label: &'static str,
}

/// The 22 blocks, in display order. Derived from the real frwiktionary dump
/// (verified against `lire`). Each `tags`
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

// --- Person-tagged grids (enwiktionary: Italian, English) ------------------

/// kaikki person/number tags. Split off a form's tag-set to find its person
/// slot; the remaining tags identify the mood/tense block.
const PERSON_NUMBER_TAGS: &[&str] = &[
    "first-person",
    "second-person",
    "third-person",
    "singular",
    "plural",
];

/// One mood/tense block in a person-tagged grid: the kaikki mood/tense tags
/// (person/number excluded), stored **sorted**, and the display label.
struct GridBlock {
    tags: &'static [&'static str],
    label: &'static str,
}

/// One person slot: the kaikki person/number tags identifying it (stored
/// **sorted**) and the subject pronoun synthesized onto the form. An empty
/// `tags` matches non-finite forms (participle/gerund), rendered bare.
struct PersonSlot {
    tags: &'static [&'static str],
    pronoun: &'static str,
}

/// A person-tagged conjugation layout: ordered mood/tense blocks and the person
/// slots (in display order) used to place and prefix each form.
pub struct PersonGrid {
    blocks: &'static [GridBlock],
    persons: &'static [PersonSlot],
    /// Strip interior stress accents from forms (Italian Wiktionary writes a
    /// pronunciation accent on every stressed vowel; standard orthography keeps
    /// it only on a word-final vowel). See [`strip_interior_stress`].
    strip_stress: bool,
}

impl Conjugation {
    /// Group person-tagged `forms` (enwiktionary shape) into the canonical
    /// sections of `grid`. Each form's tag-set is split into a person/number
    /// subset (→ pronoun + order) and a mood/tense subset (→ block); forms whose
    /// subsets don't both match the grid are dropped as noise. The subject
    /// pronoun is synthesized onto the bare form so the rendered HTML matches the
    /// `<b>heading</b><br>pronoun form` shape irondict's parser expects.
    pub fn from_person_tagged(
        infinitive: String,
        forms: &[(Vec<String>, String)],
        grid: &PersonGrid,
    ) -> Conjugation {
        debug_assert!(
            grid.blocks.iter().all(|b| b.tags.windows(2).all(|w| w[0] <= w[1])),
            "grid block tag-sets must be stored sorted",
        );

        // buckets[block] = forms paired with their person order, for sorting.
        let mut buckets: Vec<Vec<(usize, String)>> =
            (0..grid.blocks.len()).map(|_| Vec::new()).collect();

        for (tags, form) in forms {
            let (person_tags, block_tags): (Vec<&str>, Vec<&str>) = tags
                .iter()
                .map(String::as_str)
                .partition(|t| PERSON_NUMBER_TAGS.contains(t));
            let Some(block) = match_block(grid, &block_tags) else {
                continue;
            };
            let Some((order, pronoun)) = match_person(grid, &person_tags) else {
                continue;
            };
            let mut text = normalize_form(form);
            if grid.strip_stress {
                text = strip_interior_stress(&text);
            }
            if is_placeholder(&text) {
                continue;
            }
            // One canonical form per cell: keep the first form for each person
            // slot (Wiktionary lists the standard form first; later ones are
            // archaic/dialectal/poetic alternants tagged identically). This also
            // collapses exact duplicates.
            if buckets[block].iter().any(|(o, _)| *o == order) {
                continue;
            }
            let line = if pronoun.is_empty() {
                text
            } else {
                format!("{pronoun} {text}")
            };
            buckets[block].push((order, line));
        }

        let sections = buckets
            .into_iter()
            .enumerate()
            .filter(|(_, forms)| !forms.is_empty())
            .map(|(i, mut forms)| {
                forms.sort_by_key(|(order, _)| *order);
                Section {
                    label: grid.blocks[i].label,
                    forms: forms.into_iter().map(|(_, line)| line).collect(),
                }
            })
            .collect();

        Conjugation { infinitive, sections }
    }
}

/// Index of the grid block whose mood/tense tag-set equals `tags`, order-free.
fn match_block(grid: &PersonGrid, tags: &[&str]) -> Option<usize> {
    let mut sorted = tags.to_vec();
    sorted.sort_unstable();
    grid.blocks.iter().position(|b| b.tags == sorted.as_slice())
}

/// The `(display order, pronoun)` of the person slot matching `tags`, order-free.
fn match_person(grid: &PersonGrid, tags: &[&str]) -> Option<(usize, &'static str)> {
    let mut sorted = tags.to_vec();
    sorted.sort_unstable();
    grid.persons
        .iter()
        .position(|p| p.tags == sorted.as_slice())
        .map(|i| (i, grid.persons[i].pronoun))
}

/// Italian conjugation grid (kaikki tags from `enwiktionary`). Tag-sets are the
/// mood/tense tags with person/number removed, stored sorted.
///
/// Verified by enumerating every mood/tense tag-set over the real `enwiktionary`
/// Italian verbs: these 12 are the standard modern grid (each ~27k occurrences =
/// 6 persons × ~4.4k verbs). Passato remoto is `historic`; conditional/imperative
/// are bare. Everything else carries an extra qualifier tag — summary headword
/// forms (bare `["present"]`, `["subjunctive"]`, `["historic","past"]`), variants
/// (`archaic`/`poetic`/`rare`/`Traditional`/`dialectal`), the negative/formal
/// imperatives (`["imperative","negative"]`, `["formal",…,
/// "second-person-semantically"]`), and `auxiliary`/`canonical`/`table-tags`/
/// `inflection-template` rows — so all of it drops, leaving the clean table.
pub static ITALIAN_GRID: PersonGrid = PersonGrid {
    strip_stress: true,
    blocks: &[
        // Indicativo
        GridBlock { tags: &["indicative", "present"], label: "Indicativo presente" },
        GridBlock { tags: &["imperfect", "indicative"], label: "Indicativo imperfetto" },
        GridBlock { tags: &["historic", "indicative", "past"], label: "Indicativo passato remoto" },
        GridBlock { tags: &["future", "indicative"], label: "Indicativo futuro semplice" },
        // Congiuntivo
        GridBlock { tags: &["present", "subjunctive"], label: "Congiuntivo presente" },
        GridBlock { tags: &["imperfect", "subjunctive"], label: "Congiuntivo imperfetto" },
        // Condizionale
        GridBlock { tags: &["conditional"], label: "Condizionale presente" },
        // Imperativo
        GridBlock { tags: &["imperative"], label: "Imperativo" },
        // Forme nominali
        GridBlock { tags: &["infinitive"], label: "Infinito" },
        GridBlock { tags: &["gerund"], label: "Gerundio" },
        GridBlock { tags: &["participle", "present"], label: "Participio presente" },
        GridBlock { tags: &["participle", "past"], label: "Participio passato" },
    ],
    persons: &[
        PersonSlot { tags: &["first-person", "singular"], pronoun: "io" },
        PersonSlot { tags: &["second-person", "singular"], pronoun: "tu" },
        PersonSlot { tags: &["singular", "third-person"], pronoun: "egli" },
        PersonSlot { tags: &["first-person", "plural"], pronoun: "noi" },
        PersonSlot { tags: &["plural", "second-person"], pronoun: "voi" },
        PersonSlot { tags: &["plural", "third-person"], pronoun: "essi" },
        // Non-finite forms (infinitive/participle/gerund) carry no person tag.
        PersonSlot { tags: &[], pronoun: "" },
    ],
};

/// English conjugation grid (kaikki tags from `enwiktionary`). English stores
/// only principal parts; the periphrastic grid would need synthesis (deferred).
pub static ENGLISH_GRID: PersonGrid = PersonGrid {
    strip_stress: false,
    blocks: &[
        // Person/number tags are split off before matching, so the 3sg present
        // block is keyed on `present` alone (its person slot matches separately).
        GridBlock { tags: &["present"], label: "Third-person singular present" },
        GridBlock { tags: &["past"], label: "Simple past" },
        GridBlock { tags: &["participle", "past"], label: "Past participle" },
        GridBlock { tags: &["participle", "present"], label: "Present participle" },
    ],
    persons: &[
        // Principal parts render bare (no synthesized pronoun), whether the form
        // carried no person tags (past/participles) or the 3sg present pair.
        PersonSlot { tags: &[], pronoun: "" },
        PersonSlot { tags: &["singular", "third-person"], pronoun: "" },
    ],
};

/// Clean a raw `form` for display: trim, and fold the typographic apostrophe
/// (U+2019) to a plain `'` so forms copy/search/compare consistently.
fn normalize_form(form: &str) -> String {
    form.trim().replace('\u{2019}', "'")
}

/// A rendered form that carries no content: empty, or a dash placeholder kaikki
/// uses for a cell with no form (`-`, `–`, `—`).
fn is_placeholder(text: &str) -> bool {
    let t = text.trim();
    t.is_empty() || t.chars().all(|c| matches!(c, '-' | '\u{2013}' | '\u{2014}'))
}

/// The monosyllables that keep a written final accent in standard Italian
/// (disambiguating or by convention); every other monosyllable drops it.
const MONO_KEEP_ACCENT: &[&str] = &[
    "è", "dà", "là", "lì", "sì", "né", "sé", "più", "giù", "ciò", "può", "già",
];

/// Italian Wiktionary marks the stressed vowel of every form for pronunciation
/// (`dàre`, `diàmo`, `dièdi`), but standard orthography writes an accent only on
/// a **word-final** stressed vowel — and even then only on polysyllables
/// (`darò`, `città`, `perché`) or a few set monosyllables (`è`, `dà`). Fold every
/// interior accent away; keep a final accent only when the word is polysyllabic
/// or whitelisted (so `fù`/`và`/`sò` → `fu`/`va`/`so`, but `parlò`/`è` stay).
/// Acts per whitespace-separated word so a clitic doesn't shield an accent.
fn strip_interior_stress(form: &str) -> String {
    form.split(' ')
        .map(fold_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn fold_word(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    let mut out: String = chars
        .iter()
        .enumerate()
        .map(|(i, &c)| if i + 1 == n { c } else { unaccent_vowel(c) })
        .collect();
    if let Some(&last) = chars.last() {
        let folded = unaccent_vowel(last);
        let is_accented = folded != last;
        let keep = MONO_KEEP_ACCENT.contains(&word) || syllables(&out) >= 2;
        if is_accented && !keep {
            out.pop();
            out.push(folded);
        }
    }
    out
}

/// Rough Italian syllable count: maximal runs of vowels (a/e/i/o/u, accented or
/// not) each count as one. Good enough to tell monosyllables from the rest.
fn syllables(word: &str) -> usize {
    let mut count = 0;
    let mut in_vowel = false;
    for c in word.chars() {
        let v = matches!(unaccent_vowel(c.to_ascii_lowercase()), 'a' | 'e' | 'i' | 'o' | 'u');
        if v && !in_vowel {
            count += 1;
        }
        in_vowel = v;
    }
    count
}

/// Fold an Italian accented vowel to its base letter; pass anything else through.
fn unaccent_vowel(c: char) -> char {
    match c {
        'à' | 'á' => 'a',
        'è' | 'é' => 'e',
        'ì' | 'í' => 'i',
        'ò' | 'ó' => 'o',
        'ù' | 'ú' => 'u',
        other => other,
    }
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
    fn person_tagged_synthesizes_pronouns_and_orders() {
        // enwiktionary shape: per-person tags, bare form text. Tags deliberately
        // out of order to prove the set-match is order-free.
        let forms = vec![
            (tags(&["singular", "present", "indicative", "third-person"]), "parla".into()),
            (tags(&["first-person", "indicative", "present", "singular"]), "parlo".into()),
            (tags(&["second-person", "singular", "indicative", "present"]), "parli".into()),
            (tags(&["plural", "first-person", "present", "indicative"]), "parliamo".into()),
            // Non-finite: no person/number tags → rendered bare.
            (tags(&["participle", "past"]), "parlato".into()),
            // Noise: an unknown mood/tense tag-set is dropped.
            (tags(&["archaic", "present"]), "parlamente".into()),
        ];
        let c = Conjugation::from_person_tagged("parlare".into(), &forms, &ITALIAN_GRID);

        assert_eq!(c.sections.len(), 2);
        // Present indicative: pronouns synthesized, persons in io/tu/egli/noi order.
        assert_eq!(c.sections[0].label, "Indicativo presente");
        assert_eq!(
            c.sections[0].forms,
            vec!["io parlo", "tu parli", "egli parla", "noi parliamo"],
        );
        // Past participle: bare, no pronoun.
        assert_eq!(c.sections[1].label, "Participio passato");
        assert_eq!(c.sections[1].forms, vec!["parlato"]);
    }

    #[test]
    fn strips_interior_stress_keeps_final_accent() {
        // Italian Wiktionary's pronunciation accents (dàre/diàmo/dièdi) fold away;
        // an orthographic final accent on a polysyllable (darò) stays.
        assert_eq!(strip_interior_stress("dàre"), "dare");
        assert_eq!(strip_interior_stress("diàmo"), "diamo");
        assert_eq!(strip_interior_stress("dièdi"), "diedi");
        assert_eq!(strip_interior_stress("darò"), "darò");
        assert_eq!(strip_interior_stress("darà"), "darà");
        assert_eq!(strip_interior_stress("darèbbe"), "darebbe");
    }

    #[test]
    fn folds_bare_monosyllable_accents_but_keeps_disambiguators() {
        // Monosyllables drop the pronunciation accent (standard fu/va/so/fa)…
        assert_eq!(strip_interior_stress("fù"), "fu");
        assert_eq!(strip_interior_stress("và"), "va");
        assert_eq!(strip_interior_stress("sò"), "so");
        assert_eq!(strip_interior_stress("fà"), "fa");
        // …except the set monosyllables that keep it in orthography.
        assert_eq!(strip_interior_stress("è"), "è");
        assert_eq!(strip_interior_stress("dà"), "dà");
    }

    #[test]
    fn drops_placeholders_and_caps_one_form_per_cell() {
        let forms = vec![
            // Two variants for the same 1sg present cell: keep the first.
            (tags(&["first-person", "indicative", "present", "singular"]), "sono".into()),
            (tags(&["first-person", "indicative", "present", "singular"]), "sò".into()),
            // A dash placeholder for the present participle is dropped.
            (tags(&["participle", "present"]), "-".into()),
            (tags(&["participle", "present"]), "essente".into()),
        ];
        let c = Conjugation::from_person_tagged("essere".into(), &forms, &ITALIAN_GRID);
        assert_eq!(c.sections[0].label, "Indicativo presente");
        assert_eq!(c.sections[0].forms, vec!["io sono"]);
        assert_eq!(c.sections[1].label, "Participio presente");
        assert_eq!(c.sections[1].forms, vec!["essente"]);
    }

    #[test]
    fn italian_grid_folds_stress_and_dedups_variants() {
        // Two passato-remoto variants that differ only by an interior pronunciation
        // accent collapse to one after folding.
        let forms = vec![
            (tags(&["first-person", "indicative", "present", "plural"]), "diàmo".into()),
            (tags(&["first-person", "historic", "indicative", "past", "singular"]), "dièdi".into()),
            (tags(&["first-person", "historic", "indicative", "past", "singular"]), "diédi".into()),
        ];
        let c = Conjugation::from_person_tagged("dare".into(), &forms, &ITALIAN_GRID);
        assert_eq!(c.sections[0].label, "Indicativo presente");
        assert_eq!(c.sections[0].forms, vec!["noi diamo"]);
        assert_eq!(c.sections[1].label, "Indicativo passato remoto");
        assert_eq!(c.sections[1].forms, vec!["io diedi"]);
    }

    #[test]
    fn person_tagged_drops_unmatched_person_tagset() {
        // A present form whose person subset isn't a known slot (e.g. a stray
        // "impersonal") is dropped, not mis-placed.
        let forms = vec![
            (tags(&["first-person", "singular", "present", "indicative"]), "parlo".into()),
            (tags(&["impersonal", "present", "indicative"]), "si parla".into()),
        ];
        let c = Conjugation::from_person_tagged("parlare".into(), &forms, &ITALIAN_GRID);
        assert_eq!(c.sections.len(), 1);
        assert_eq!(c.sections[0].forms, vec!["io parlo"]);
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
