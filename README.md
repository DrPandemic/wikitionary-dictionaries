# wikitionary-dictionaries

Build pipeline that turns Wiktionary data (via
[kaikki.org / wiktextract](https://kaikki.org/)) into compact **StarDict
dictionaries**, published as release assets so any StarDict reader can install
them. Two products share one pipeline:

- **Verb-conjugation companions** — full conjugation tables, to sit alongside a
  reading dictionary that on its own lacks them (French is the first target).
- **Monolingual definition dictionaries** — full headword/definition entries for
  languages the upstream [`xxyzz/wiktionary_stardict`](https://github.com/xxyzz/wiktionary_stardict)
  releases don't cover (Italian is the first target). See [PLAN.md](PLAN.md).

More languages follow.

## Building

The pipeline runs in three stages, each a subcommand taking a dictionary **id**
(`fr-conj`, `it-it`, `it-conj`, `en-conj`):

```sh
cargo run --release -- fetch it-it      # download the kaikki dump to data/itwiktionary/
cargo run --release -- build it-it      # render entries → StarDict in data/it/it-it/
cargo run --release -- package it-it    # tar + zstd → release assets in data/it/release/
```

A dictionary's id selects both the source edition and the product. The raw dump
is keyed by **edition** (`data/<edition>/`), so dictionaries sharing an edition
download it once: `it-conj` and `en-conj` are both built from the one
`enwiktionary` dump (it carries full per-person conjugation grids for every
language, which the native `itwiktionary` edition does not).

`fetch` streams into a `.partial` file and is **resumable** — these dumps are
multi-GB and the server is slow, so if a download is interrupted just re-run the
same `fetch` and it continues from where it stopped (via an HTTP `Range`
request). A `.partial` is only renamed into place once fully downloaded, so a
truncated file is never built from.

`build` accepts `--lemmas-only` to drop inflected forms (a lean definitions
build). `package` emits two assets per dictionary: `-plain` (uncompressed
`.dict`, smaller download) and `-dictzip` (`.dict.dz`, small download *and* small
on disk), each with a `.sha256` sidecar.

## Releasing

Releases are published with the [`gh`](https://cli.github.com/) CLI. irondict
downloads assets from **`releases/latest/download/<asset>`** (an unversioned
redirect), so **the release tagged *latest* must carry every product's assets** —
publishing an Italian-only release would 404 the existing French download. Always
re-upload the other product's assets alongside the new ones.

1. Build & package every product whose assets the release will serve (so they are
   all present under `data/*/release/`).
2. Record provenance in [`SOURCES.md`](SOURCES.md): the kaikki snapshot date
   (the dump's `Last-Modified` header — confirm `content-length` matches the
   local copy), headword count, attribution, and each asset's SHA-256. This file
   doubles as the release notes.
3. Tag and publish, listing **all** products' assets:

   ```sh
   git tag -a v0.2.0 -m "v0.2.0 — Italian monolingual definitions (it-it)"
   git push origin v0.2.0
   gh release create v0.2.0 \
     --title "v0.2.0 — Italian monolingual definitions" \
     --notes-file SOURCES.md \
     data/it/release/it-it-*.tar.zst data/it/release/it-it-*.tar.zst.sha256 \
     data/fr/release/fr-conj-*.tar.zst data/fr/release/fr-conj-*.tar.zst.sha256
   ```

4. Verify the redirect irondict relies on resolves for each asset:

   ```sh
   curl -sIL -o /dev/null -w '%{http_code}\n' \
     https://github.com/DrPandemic/wikitionary-dictionaries/releases/latest/download/it-it-dictzip.tar.zst
   ```

## License

Code: GPL-3.0-or-later. Generated dictionary data is Wiktionary-derived and
licensed CC BY-SA 4.0; each release carries attribution and the source snapshot
date.
