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

## License

Code: GPL-3.0-or-later. Generated dictionary data is Wiktionary-derived and
licensed CC BY-SA 4.0; each release carries attribution and the source snapshot
date.
