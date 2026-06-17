# Release sources & attribution

Generated dictionary data is derived from **Wiktionary** via
[kaikki.org / wiktextract](https://kaikki.org/) and is licensed
**[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)**. The build
pipeline itself is GPL-3.0-or-later (see `LICENSE`).

Each release records the upstream edition and the kaikki snapshot date the
assets were built from.

## it-it — Wiktionary — Italiano (monolingual definitions)

- **Source edition:** itwiktionary
  (<https://kaikki.org/itwiktionary/raw-wiktextract-data.jsonl.gz>)
- **kaikki snapshot:** 2026-06-11
- **Headwords:** 522,590 (lemmas + inflected forms)
- **License:** CC BY-SA 4.0 — © Wiktionary contributors

| Asset | Size | SHA-256 |
|---|---|---|
| `it-it-plain.tar.zst` | 3.6 MB | `3d4fd7a00cb36dbf16618621e543c49bbdaa7e092248a7fdc08f737083b3fcfb` |
| `it-it-dictzip.tar.zst` | 5.1 MB | `6b6bedc023542d1f37375e2014f8a5c7cc2b3fea89184cc59668eefcfb8dd790` |

`-plain` carries an uncompressed `.dict` (smaller download, larger on disk);
`-dictzip` carries a `.dict.dz` (small download *and* small on disk). Both share
the same `.ifo`/`.idx`.

## fr-conj — Conjugaison — Français (verb conjugation companion)

- **Source edition:** frwiktionary
  (<https://kaikki.org/frwiktionary/raw-wiktextract-data.jsonl.gz>)
- **kaikki snapshot:** 2026-06-11 (build commit date)
- **Headwords:** 35,468 (verb conjugation tables)
- **License:** CC BY-SA 4.0 — © Wiktionary contributors

| Asset | Size | SHA-256 |
|---|---|---|
| `fr-conj-plain.tar.zst` | 9.1 MB | `39b881b0ab19621910c87ba863be99e995926da68b3100d04509e69fc196cbc6` |
| `fr-conj-dictzip.tar.zst` | 12.1 MB | `1f479db25f67af91854efab814075893ee4c2506129a03d1c95d6f4fb6beb86d` |

`-plain` carries an uncompressed `.dict` (smaller download, larger on disk);
`-dictzip` carries a `.dict.dz` (small download *and* small on disk). Both share
the same `.ifo`/`.idx`.
