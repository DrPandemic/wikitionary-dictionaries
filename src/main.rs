//! Builds StarDict dictionaries from Wiktionary data (via kaikki/wiktextract).
//!
//! The pipeline runs in stages, each exposed as a
//! subcommand so it can run end-to-end or one stage at a time:
//!
//!   fetch    — download the kaikki bulk JSONL extract for a language
//!   build    — extract the language's product (conjugation or definitions),
//!              group and render entries, emit a StarDict
//!   package  — tar + zstd the StarDict into a release asset
//!
//! The product is selected per language in `lang.rs` (French → conjugation
//! companion, Italian → full monolingual definitions).

mod build;
mod dictzip;
mod fetch;
mod lang;
mod model;
mod model_def;
mod package;
mod stardict;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "wiktdict", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Download the kaikki raw wiktextract dump for a language.
    Fetch {
        /// Language code (`fr`, `it`).
        lang: String,
    },
    /// Extract the language's product, group and render entries, emit a StarDict.
    Build {
        /// Language code.
        lang: String,
        /// Drop inflected-form entries, keeping only lemmas (definitions builds).
        #[arg(long)]
        lemmas_only: bool,
    },
    /// Tar + zstd the built StarDict into a release asset.
    Package {
        /// Language code.
        lang: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Fetch { lang } => resolve(&lang).and_then(|l| fetch::run(l)),
        Command::Build { lang, lemmas_only } => {
            resolve(&lang).and_then(|l| build::run(l, lemmas_only))
        }
        Command::Package { lang } => resolve(&lang).and_then(|l| package::run(l)),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// Resolve a language code or fail with a helpful message.
fn resolve(code: &str) -> anyhow::Result<&'static lang::LangSpec> {
    lang::resolve(code)
        .ok_or_else(|| anyhow::anyhow!("unsupported language `{code}` (try `fr` or `it`)"))
}
