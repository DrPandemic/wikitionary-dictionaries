//! Minimal StarDict writer.
//!
//! Emits the three-file `.ifo` / `.idx` / `.dict` set for a single dictionary.
//! We target the 2.4.2 format with 32-bit big-endian offsets and an uncompressed
//! `.dict` — the simplest shape every StarDict reader accepts, and plenty for a
//! few-MB conjugation dictionary (the release asset is compressed by `package`).

use std::cmp::Ordering;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// `.ifo` metadata for the dictionary being written.
pub struct DictMeta<'a> {
    pub bookname: &'a str,
    pub author: &'a str,
    pub description: &'a str,
    pub date: &'a str,
    /// Type of every entry, e.g. `"h"` for HTML.
    pub sametypesequence: &'a str,
}

/// Write `<dir>/<stem>.{ifo,idx,dict,dict.dz}` from `entries` (`(headword,
/// body)`). Both a plain `.dict` and a seekable dictzip `.dict.dz` are emitted;
/// the `.idx` offsets address the uncompressed stream and serve both.
///
/// Entries are sorted with StarDict's collation so the `.idx` is valid for
/// readers that binary-search it. Returns the number of entries written.
pub fn write(dir: &Path, stem: &str, meta: &DictMeta, entries: &[(String, String)]) -> Result<usize> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

    let mut entries: Vec<&(String, String)> = entries.iter().collect();
    entries.sort_by(|a, b| stardict_cmp(&a.0, &b.0));

    let mut dict: Vec<u8> = Vec::new();
    let mut idx: Vec<u8> = Vec::new();
    for (word, body) in &entries {
        let offset = u32::try_from(dict.len()).context(".dict exceeds 4 GiB (u32 offsets)")?;
        let size = u32::try_from(body.len()).context("entry exceeds 4 GiB")?;
        dict.extend_from_slice(body.as_bytes());

        idx.extend_from_slice(word.as_bytes());
        idx.push(0);
        idx.extend_from_slice(&offset.to_be_bytes());
        idx.extend_from_slice(&size.to_be_bytes());
    }

    let base = dir.join(stem);
    fs::write(base.with_extension("dict"), &dict)
        .with_context(|| format!("writing {}.dict", stem))?;
    let dz = crate::dictzip::compress(&dict).context("dictzip-compressing .dict")?;
    fs::write(base.with_extension("dict.dz"), &dz)
        .with_context(|| format!("writing {}.dict.dz", stem))?;
    fs::write(base.with_extension("idx"), &idx)
        .with_context(|| format!("writing {}.idx", stem))?;

    let ifo = render_ifo(meta, entries.len(), idx.len());
    fs::write(base.with_extension("ifo"), ifo)
        .with_context(|| format!("writing {}.ifo", stem))?;

    Ok(entries.len())
}

/// The `.ifo` text. The first line is the required magic; the rest are
/// `key=value` fields the reader parses.
fn render_ifo(meta: &DictMeta, wordcount: usize, idxfilesize: usize) -> String {
    let mut s = String::new();
    s.push_str("StarDict's dict ifo file\n");
    s.push_str("version=2.4.2\n");
    s.push_str(&format!("bookname={}\n", meta.bookname));
    s.push_str(&format!("wordcount={wordcount}\n"));
    s.push_str(&format!("idxfilesize={idxfilesize}\n"));
    s.push_str(&format!("sametypesequence={}\n", meta.sametypesequence));
    if !meta.author.is_empty() {
        s.push_str(&format!("author={}\n", meta.author));
    }
    if !meta.description.is_empty() {
        s.push_str(&format!("description={}\n", meta.description));
    }
    if !meta.date.is_empty() {
        s.push_str(&format!("date={}\n", meta.date));
    }
    s
}

/// StarDict's headword collation: ASCII-case-insensitive, then a byte-wise
/// tiebreak (matching `g_ascii_strcasecmp` followed by `strcmp`).
fn stardict_cmp(a: &str, b: &str) -> Ordering {
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    for (x, y) in ab.iter().zip(bb) {
        let ord = x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase());
        if ord != Ordering::Equal {
            return ord;
        }
    }
    ab.len().cmp(&bb.len()).then_with(|| ab.cmp(bb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collation_is_case_insensitive_then_bytewise() {
        assert_eq!(stardict_cmp("abc", "ABD"), Ordering::Less);
        assert_eq!(stardict_cmp("aller", "aller"), Ordering::Equal);
        assert_eq!(stardict_cmp("Aller", "aller"), Ordering::Less); // tiebreak: 'A' < 'a'
        assert_eq!(stardict_cmp("manger", "mange"), Ordering::Greater);
    }

    /// End-to-end: write a dict, drop the plain `.dict` so the reader must use
    /// our `.dict.dz`, then look it up through the *exact* `stardict` crate
    /// irondict uses. This is the real validation of the dictzip encoder.
    #[test]
    fn dictzip_reads_back_through_stardict() {
        // Enough entries to span multiple dictzip chunks (> 58 KB of bodies).
        let entries: Vec<(String, String)> = (0..4000)
            .map(|i| (format!("verbe{i:05}"), format!("<b>Indicatif présent</b><br>je verbe{i}<br>")))
            .collect();
        let meta = DictMeta {
            bookname: "Test",
            author: "",
            description: "",
            date: "",
            sametypesequence: "h",
        };

        let dir = std::env::temp_dir().join(format!("wiktdict-dz-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        write(&dir, "t", &meta, &entries).unwrap();
        // Force the dictzip path: only `.dict.dz` remains.
        fs::remove_file(dir.join("t.dict")).unwrap();

        let mut sd = ::stardict::no_cache(dir.join("t.ifo")).unwrap();
        let got = ::stardict::StarDict::lookup(&mut sd, "verbe03999")
            .unwrap()
            .unwrap();
        assert_eq!(got[0].segments[0].text, "<b>Indicatif présent</b><br>je verbe3999<br>");
        // A word in an early chunk too, to exercise random access.
        let early = ::stardict::StarDict::lookup(&mut sd, "verbe00000").unwrap().unwrap();
        assert_eq!(early[0].segments[0].text, "<b>Indicatif présent</b><br>je verbe0<br>");

        let _ = fs::remove_dir_all(&dir);
    }
}
