# Release sources & attribution

Generated dictionary data is derived from **Wiktionary** via
[kaikki.org / wiktextract](https://kaikki.org/) and is licensed
**CC BY-SA 4.0**. The build pipeline itself is GPL-3.0-or-later (see `LICENSE`).

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
