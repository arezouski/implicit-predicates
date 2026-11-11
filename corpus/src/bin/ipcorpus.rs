//! Command-line tool for corpus files: `stats`, `verify` and `extract`.
//!
//! - `stats` prints the statistics of all the files together as Markdown
//!   tables; with `--per-file`, it prints instead one CSV row per file with
//!   its calls and zero calls by predicate and implicit count.
//! - `verify` checks every stored sign exactly. It exits with status 1 if a
//!   sign disagrees, and with status 3 if no sign disagrees but some calls
//!   could not be checked (they use LPI or TPI points), unless
//!   `--allow-skipped` is given.
//! - `extract` writes a small corpus with at most N stored calls per stratum.
//!
//! Usage errors and unreadable files exit with status 2.

use implicit_predicates_corpus::stats::Stats;
use implicit_predicates_corpus::verify::verify;
use implicit_predicates_corpus::{Corpus, CorpusError};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::process::ExitCode;

const USAGE: &str = "usage:
  ipcorpus stats [--per-file] <file>...
  ipcorpus verify [--allow-skipped] <file>...
  ipcorpus extract --max-calls <N> <input> <output>";

/// Number of mismatches printed per file.
const MISMATCHES_SHOWN: usize = 10;

/// Exit status of `verify` when calls were skipped and none disagrees.
const SKIPPED: u8 = 3;

fn load(path: &str) -> Result<Corpus, String> {
    let file = File::open(path).map_err(|e| format!("{path}: {e}"))?;
    Corpus::read(BufReader::new(file)).map_err(|e| format!("{path}: {e}"))
}

/// Splits a leading `flag` off `args`: whether it was there, and the rest.
fn option<'a>(args: &'a [String], flag: &str) -> (bool, &'a [String]) {
    match args.split_first() {
        Some((first, rest)) if first == flag => (true, rest),
        _ => (false, args),
    }
}

fn stats(args: &[String]) -> Result<ExitCode, String> {
    let (per_file, files) = option(args, "--per-file");
    if files.is_empty() {
        return Err(USAGE.to_string());
    }
    if per_file {
        println!("file,model,{}", Stats::columns().join(","));
    }
    let mut total = Stats::default();
    for path in files {
        let corpus = load(path)?;
        let stats = Stats::from_corpus(&corpus);
        if per_file {
            let values: Vec<String> = stats.values().iter().map(u64::to_string).collect();
            println!("{path},{},{}", corpus.model, values.join(","));
        } else {
            total.merge(&stats);
        }
    }
    if !per_file {
        print!("{}", total.to_markdown());
    }
    Ok(ExitCode::SUCCESS)
}

fn verify_files(args: &[String]) -> Result<ExitCode, String> {
    let (allow_skipped, files) = option(args, "--allow-skipped");
    if files.is_empty() {
        return Err(USAGE.to_string());
    }
    let (mut failed, mut skipped) = (false, false);
    for path in files {
        let v = verify(&load(path)?);
        println!(
            "{path}: checked {}, skipped {}, mismatches {}",
            v.checked,
            v.skipped,
            v.mismatches.len()
        );
        for m in v.mismatches.iter().take(MISMATCHES_SHOWN) {
            println!(
                "  call {}: recorded {}, exact {}",
                m.call, m.recorded, m.exact
            );
        }
        failed |= !v.mismatches.is_empty();
        skipped |= v.skipped > 0;
    }
    Ok(if failed {
        ExitCode::FAILURE
    } else if skipped && !allow_skipped {
        eprintln!("some calls were not checked; pass --allow-skipped to accept that");
        ExitCode::from(SKIPPED)
    } else {
        ExitCode::SUCCESS
    })
}

fn extract(args: &[String]) -> Result<ExitCode, String> {
    let [flag, n, input, output] = args else {
        return Err(USAGE.to_string());
    };
    if flag != "--max-calls" {
        return Err(USAGE.to_string());
    }
    let n: usize = n.parse().map_err(|_| format!("invalid count: {n}"))?;
    let small = load(input)?.extract(n);
    let file = File::create(output).map_err(|e| format!("{output}: {e}"))?;
    small
        .write(BufWriter::new(file))
        .map_err(|e: CorpusError| format!("{output}: {e}"))?;
    println!(
        "kept {} calls and {} points",
        small.calls.len(),
        small.points.len()
    );
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.split_first() {
        Some((cmd, rest)) if cmd == "stats" => stats(rest),
        Some((cmd, rest)) if cmd == "verify" => verify_files(rest),
        Some((cmd, rest)) if cmd == "extract" => extract(rest),
        _ => Err(USAGE.to_string()),
    };
    result.unwrap_or_else(|message| {
        eprintln!("{message}");
        ExitCode::from(2)
    })
}
