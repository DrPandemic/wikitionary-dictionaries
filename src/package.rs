//! Stage 4 — package the built StarDict into release assets.
//!
//! Emits two `.tar.zst` assets so a release can carry both and the consumer
//! picks: a `-plain` one (uncompressed `.dict`, small download, large on disk)
//! and a `-dictzip` one (`.dict.dz`, small download *and* small on disk). The
//! `.ifo`/`.idx` are shared between them.
//!
//! Archive layout matches what StarDict installers expect: the files sit at the
//! archive **root** (no enclosing directory), since a reader looks for the
//! `.ifo` directly in the extracted folder. We keep `.dict` and `.dict.dz` in
//! *separate* archives — bundling both lets a reader pick the plain one and
//! ignore the dictzip, defeating the point.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};

use crate::lang::{data_dir, LangSpec};

/// zstd level for the plain-dict asset. (The dictzip asset is already
/// compressed, so this barely affects it.)
const ZSTD_LEVEL: i32 = 19;

pub fn run(lang: &LangSpec) -> Result<()> {
    let src = data_dir(lang.code).join(lang.id);
    let ifo = src.join(format!("{}.ifo", lang.id));
    let idx = src.join(format!("{}.idx", lang.id));
    let dict = src.join(format!("{}.dict", lang.id));
    let dz = src.join(format!("{}.dict.dz", lang.id));
    for p in [&ifo, &idx, &dict, &dz] {
        if !p.exists() {
            bail!("{} not found — run `build {}` first", p.display(), lang.code);
        }
    }

    let out = data_dir(lang.code).join("release");
    fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;

    println!("Packaging {} release assets", lang.label);

    // Plain: .ifo + .idx + uncompressed .dict.
    let plain = out.join(format!("{}-plain.tar.zst", lang.id));
    archive(&plain, &[&ifo, &idx, &dict])?;

    // Dictzip: .ifo + .idx + .dict.dz (small on disk after install).
    let dictzip = out.join(format!("{}-dictzip.tar.zst", lang.id));
    archive(&dictzip, &[&ifo, &idx, &dz])?;

    for asset in [&plain, &dictzip] {
        write_checksum(asset)?;
    }

    println!("Assets in {}", out.display());
    Ok(())
}

/// Write `members` into a zstd-compressed tar at `dest`, each entry stored at the
/// archive root under its own file name.
fn archive(dest: &Path, members: &[&PathBuf]) -> Result<()> {
    let label = file_name(dest);
    let spinner = spinner(&format!("writing {label}"));

    let file = File::create(dest).with_context(|| format!("creating {}", dest.display()))?;
    let encoder = zstd::Encoder::new(file, ZSTD_LEVEL).context("init zstd encoder")?;
    let mut builder = tar::Builder::new(encoder);
    for path in members {
        let name = file_name(path);
        builder
            .append_path_with_name(path, &name)
            .with_context(|| format!("adding {name} to {label}"))?;
    }
    // Finish the tar (writes its trailer) and hand back the zstd encoder to close.
    let encoder = builder.into_inner().context("finishing tar")?;
    encoder.finish().context("finishing zstd stream")?;

    spinner.finish_and_clear();
    let size = fs::metadata(dest)?.len();
    println!("  {label} ({:.1} MB)", size as f64 / 1_000_000.0);
    Ok(())
}

/// Write `<asset>.sha256` in `sha256sum` format (`<hex>  <filename>`).
fn write_checksum(asset: &Path) -> Result<()> {
    let mut file = File::open(asset).with_context(|| format!("opening {}", asset.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).context("reading asset for checksum")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let hex: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();

    let sidecar = asset.with_extension("zst.sha256");
    fs::write(&sidecar, format!("{hex}  {}\n", file_name(asset)))
        .with_context(|| format!("writing {}", sidecar.display()))?;
    Ok(())
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string()
}

fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::with_template("{spinner} {msg}").expect("valid template"));
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stardict::{self, DictMeta};

    /// Replays irondict's install pipeline end-to-end: our `package` archive →
    /// ruzstd decode → tar unpack → root `.ifo` discovery → `stardict` lookup.
    /// Proves the assets are consumable by the exact crates irondict uses.
    #[test]
    fn dictzip_asset_installs_through_irondict_chain() {
        let work = std::env::temp_dir().join(format!("wiktdict-pkg-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&work);
        let src = work.join("src");

        let entries: Vec<(String, String)> = (0..50)
            .map(|i| (format!("verbe{i:03}"), format!("<b>Indicatif présent</b><br>je verbe{i}<br>")))
            .collect();
        let meta = DictMeta {
            bookname: "Test",
            author: "",
            description: "",
            date: "",
            sametypesequence: "h",
        };
        stardict::write(&src, "t", &meta, &entries).unwrap();

        let asset = work.join("t-dictzip.tar.zst");
        archive(
            &asset,
            &[&src.join("t.ifo"), &src.join("t.idx"), &src.join("t.dict.dz")],
        )
        .unwrap();

        // irondict's extraction: ruzstd::StreamingDecoder -> tar::Archive::unpack.
        let out = work.join("out");
        fs::create_dir_all(&out).unwrap();
        let decoder = ruzstd::decoding::StreamingDecoder::new(File::open(&asset).unwrap()).unwrap();
        tar::Archive::new(decoder).unpack(&out).unwrap();

        // irondict's non-recursive `find_ifo`: a `.ifo` at the extracted root.
        let ifo = fs::read_dir(&out)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.extension().is_some_and(|x| x == "ifo"))
            .expect("a .ifo at the archive root");

        let mut sd = ::stardict::no_cache(ifo).unwrap();
        let got = ::stardict::StarDict::lookup(&mut sd, "verbe049").unwrap().unwrap();
        assert_eq!(got[0].segments[0].text, "<b>Indicatif présent</b><br>je verbe49<br>");

        let _ = fs::remove_dir_all(&work);
    }
}
