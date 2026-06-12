//! Builds StarDict verb-conjugation companion dictionaries from Wiktionary data.
//!
//! See `PLAN.md` for the design. The pipeline runs in stages, each exposed as a
//! subcommand so it can run end-to-end or one stage at a time:
//!
//!   fetch    — download the kaikki bulk JSONL extract for a language
//!   build    — filter verbs, normalize/group forms, emit a StarDict
//!   package  — tar + zstd the StarDict into a release asset
//!
//! All stages are stubs in Phase 0; the CLI shape is what's wired up.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "verbdict", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Download the kaikki bulk JSONL extract for a language.
    Fetch {
        /// Language code (only `fr` is supported initially).
        lang: String,
    },
    /// Filter verbs, group forms into the conjugation grid, emit a StarDict.
    Build {
        /// Language code.
        lang: String,
    },
    /// Tar + zstd the built StarDict into a release asset.
    Package {
        /// Language code.
        lang: String,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Fetch { lang } => todo!("Phase 2: fetch kaikki extract for {lang}"),
        Command::Build { lang } => todo!("Phase 2: build StarDict for {lang}"),
        Command::Package { lang } => todo!("Phase 3: package release asset for {lang}"),
    }
}
