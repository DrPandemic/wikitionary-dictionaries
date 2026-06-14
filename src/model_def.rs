//! Definition model and its HTML renderer.
//!
//! An [`Entry`] is one headword with its part-of-speech [`PosSection`]s (a word
//! can be e.g. both an adjective and a noun, mirroring the multiple kaikki
//! objects that share a `word`). Each section carries its IPA, etymology, and a
//! list of [`Sense`]s (gloss + examples + any inflection cross-reference).
//!
//! [`Entry::to_html`] emits the exact HTML shape irondict's `html_to_blocks`
//! parser already consumes for the French Wiktionnaire: a `<h4>` part-of-speech
//! heading, a headword line carrying a `\…\` phonetic (lifted onto the grey
//! "pos · pron" line), `<ol><li>` senses with `<dd>` examples, and `bword://`
//! anchors for inflection links. No new irondict parser is needed.

/// One numbered sense: its definition gloss, example sentences, and the lemma
/// words it is an inflected form of (for `corro` → *correre*).
pub struct Sense {
    pub gloss: String,
    pub examples: Vec<String>,
    pub form_of: Vec<String>,
}

impl Sense {
    /// Build a sense from raw kaikki fields, returning `None` when the gloss is a
    /// headword-line artifact (e.g. `casa` → `"casa ( approfondimento) f sing"`)
    /// rather than a real definition. `glosses` is joined; meta-link markers are
    /// stripped.
    pub fn build(word: &str, glosses: &[String], examples: Vec<String>, form_of: Vec<String>) -> Option<Sense> {
        let gloss = clean_gloss(&glosses.join(" "));
        if gloss.is_empty() || is_headword_artifact(word, &gloss) {
            return None;
        }
        Some(Sense { gloss, examples, form_of })
    }
}

/// One part-of-speech section of an entry (one kaikki object).
pub struct PosSection {
    /// Italian part-of-speech label (`pos_title`: "Sostantivo", "Verbo", …).
    pub pos_title: String,
    /// IPA transcription without delimiters (e.g. `ˈbɛllo`), if any.
    pub ipa: Option<String>,
    /// Joined etymology text, if any.
    pub etymology: Option<String>,
    pub senses: Vec<Sense>,
}

impl PosSection {
    /// True when this section carries nothing worth rendering.
    pub fn is_empty(&self) -> bool {
        self.senses.is_empty() && self.etymology.is_none()
    }

    /// True when this section is an inflected form (a verb form like `corro` or a
    /// "forma flessa" noun/adjective) rather than a lemma. Italian Wiktionary
    /// labels these `Voce verbale` or `…, forma flessa`; the `--lemmas-only`
    /// build drops them, keeping only the ~75k rich lemma entries.
    pub fn is_inflected_form(&self) -> bool {
        let pos = self.pos_title.to_lowercase();
        pos == "voce verbale" || pos.contains("forma flessa")
    }
}

/// A headword with all its part-of-speech sections, in dump order.
pub struct Entry {
    pub word: String,
    pub sections: Vec<PosSection>,
}

impl Entry {
    /// The HTML entry body for `sametypesequence=h`, matching irondict's parser.
    pub fn to_html(&self) -> String {
        let mut out = String::new();
        for sec in &self.sections {
            out.push_str("<h4>");
            push_escaped(&mut out, &sec.pos_title);
            out.push_str("</h4>");

            // Headword line with a backslash phonetic, so irondict's `slash_pron`
            // lifts "<pos> · <ipa>" onto the grey header line and drops this line.
            // Emitted only when an IPA is present (no phonetic = nothing to lift).
            if let Some(ipa) = &sec.ipa {
                out.push_str("<p>");
                push_escaped(&mut out, &self.word);
                out.push_str(" \\");
                push_escaped(&mut out, ipa);
                out.push_str("\\</p>");
            }

            if !sec.senses.is_empty() {
                out.push_str("<ol>");
                for sense in &sec.senses {
                    out.push_str("<li>");
                    push_gloss(&mut out, &sense.gloss, &sense.form_of);
                    for ex in &sense.examples {
                        out.push_str("<dd>");
                        push_escaped(&mut out, ex);
                        out.push_str("</dd>");
                    }
                    out.push_str("</li>");
                }
                out.push_str("</ol>");
            }

            if let Some(etym) = &sec.etymology {
                out.push_str("<p>Etimologia: ");
                push_escaped(&mut out, etym);
                out.push_str("</p>");
            }
        }
        out
    }
}

/// Normalize a raw IPA value: trim and drop one pair of `/…/` delimiters, leaving
/// the bare transcription (irondict re-wraps it for display). `None` when empty.
pub fn normalize_ipa(raw: &str) -> Option<String> {
    let s = raw.trim();
    let s = s.strip_prefix('/').unwrap_or(s);
    let s = s.strip_suffix('/').unwrap_or(s);
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// Wiki meta-link markers Italian Wiktionary embeds in glosses — links to an
/// in-depth page, citations, or the taxonomy box, not part of the definition.
const META_MARKERS: &[&str] = &[
    " ( approfondimento)",
    " ( citazioni)",
    " ( tassonomia)",
];

/// Strip the [`META_MARKERS`] from a gloss, then trim.
fn clean_gloss(gloss: &str) -> String {
    let mut s = gloss.to_string();
    for marker in META_MARKERS {
        if s.contains(marker) {
            s = s.replace(marker, "");
        }
    }
    s.trim().to_string()
}

/// Whether a cleaned gloss is really a headword line — the word itself optionally
/// followed by only a gender/number code (`f sing`, `m pl`, `inv`) — not a
/// definition. Such "senses" repeat what the header already shows.
fn is_headword_artifact(word: &str, gloss: &str) -> bool {
    let gloss = gloss.to_lowercase();
    let word = word.trim().to_lowercase();
    let Some(rest) = gloss.strip_prefix(&word) else {
        return false;
    };
    rest.split_whitespace().all(is_gender_token)
}

/// A grammatical gender/number token that can trail a headword line.
fn is_gender_token(tok: &str) -> bool {
    matches!(tok, "m" | "f" | "sing" | "pl" | "inv" | "s")
}

/// Append a gloss, turning the first inflection lemma into a `bword://` link so a
/// form entry ("1ª persona… di *correre*") jumps to its lemma. The lemma's last
/// occurrence in the gloss is linked; if it doesn't appear, an arrow link is
/// appended.
fn push_gloss(out: &mut String, gloss: &str, form_of: &[String]) {
    let Some(lemma) = form_of.iter().find(|w| !w.trim().is_empty()) else {
        push_escaped(out, gloss);
        return;
    };
    if let Some(pos) = gloss.rfind(lemma.as_str()) {
        push_escaped(out, &gloss[..pos]);
        push_link(out, lemma);
        push_escaped(out, &gloss[pos + lemma.len()..]);
    } else {
        push_escaped(out, gloss);
        out.push_str(" → ");
        push_link(out, lemma);
    }
}

/// Append a `bword://` cross-reference anchor for `target`.
fn push_link(out: &mut String, target: &str) {
    out.push_str("<a href=\"bword://");
    push_escaped(out, target);
    out.push_str("\">");
    push_escaped(out, target);
    out.push_str("</a>");
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

    fn sense(word: &str, gloss: &str) -> Option<Sense> {
        Sense::build(word, &[gloss.to_string()], vec![], vec![])
    }

    #[test]
    fn drops_headword_artifacts() {
        // Fabricated to mirror the itwiktionary shape, not copied from any entry.
        assert!(sense("nimbo", "nimbo ( approfondimento) f sing").is_none());
        assert!(sense("nimbo", "nimbo").is_none());
        assert!(sense("nimbo", "nimbo m sing").is_none());
        // A real definition that merely starts with the headword is kept.
        assert!(sense("nimbo", "nimbo luminoso attorno al capo").is_some());
    }

    #[test]
    fn normalizes_ipa() {
        assert_eq!(normalize_ipa("/ˈkasa/"), Some("ˈkasa".to_string()));
        assert_eq!(normalize_ipa("  ˈkasa "), Some("ˈkasa".to_string()));
        assert_eq!(normalize_ipa("//"), None);
    }

    #[test]
    fn header_line_carries_backslash_phonetic() {
        let entry = Entry {
            word: "nimbo".into(),
            sections: vec![PosSection {
                pos_title: "Sostantivo".into(),
                ipa: Some("ˈnimbo".into()),
                etymology: None,
                senses: vec![Sense {
                    gloss: "alone luminoso".into(),
                    examples: vec![],
                    form_of: vec![],
                }],
            }],
        };
        let html = entry.to_html();
        assert!(html.contains("<h4>Sostantivo</h4>"));
        assert!(html.contains("<p>nimbo \\ˈnimbo\\</p>"));
        assert!(html.contains("<ol><li>alone luminoso</li></ol>"));
    }

    #[test]
    fn form_of_lemma_becomes_a_link() {
        let s = Sense {
            gloss: "prima persona di correre".into(),
            examples: vec![],
            form_of: vec!["correre".into()],
        };
        let entry = Entry {
            word: "corro".into(),
            sections: vec![PosSection {
                pos_title: "Voce verbale".into(),
                ipa: None,
                etymology: None,
                senses: vec![s],
            }],
        };
        let html = entry.to_html();
        assert!(
            html.contains("prima persona di <a href=\"bword://correre\">correre</a>"),
            "got: {html}"
        );
        // No phonetic → no headword line emitted.
        assert!(!html.contains("<p>corro"));
    }

    #[test]
    fn classifies_inflected_forms() {
        let section = |pos: &str| PosSection {
            pos_title: pos.into(),
            ipa: None,
            etymology: None,
            senses: vec![],
        };
        assert!(section("Voce verbale").is_inflected_form());
        assert!(section("Sostantivo, forma flessa").is_inflected_form());
        assert!(section("Aggettivo, forma flessa").is_inflected_form());
        // Lemmas are kept.
        assert!(!section("Sostantivo").is_inflected_form());
        assert!(!section("Verbo").is_inflected_form());
        assert!(!section("Locuzione nominale").is_inflected_form());
    }

    #[test]
    fn examples_render_as_dd() {
        let entry = Entry {
            word: "correre".into(),
            sections: vec![PosSection {
                pos_title: "Verbo".into(),
                ipa: Some("ˈkorrere".into()),
                etymology: Some("dal latino currere".into()),
                senses: vec![Sense {
                    gloss: "procedere velocemente".into(),
                    examples: vec!["Corse via.".into()],
                    form_of: vec![],
                }],
            }],
        };
        let html = entry.to_html();
        assert!(html.contains("<li>procedere velocemente<dd>Corse via.</dd></li>"));
        assert!(html.contains("<p>Etimologia: dal latino currere</p>"));
    }
}
