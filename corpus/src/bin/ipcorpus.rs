//! Command-line tool for corpus files: `stats`, `verify` and `extract`.

use indirect_predicates_corpus::stats::Stats;
use indirect_predicates_corpus::verify::verify;
use indirect_predicates_corpus::{Corpus, CorpusError};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::process::ExitCode;

const USAGE: &str = "usage:
  ipcorpus stats <file>...
  ipcorpus verify <file>...
  ipcorpus extract --max-calls <N> <input> <output>";

/// Number of mismatches printed per file.
const MISMATCHES_SHOWN: usize = 10;

fn load(path: &str) -> Result<Corpus, String> {
    let file = File::open(path).map_err(|e| format!("{path}: {e}"))?;
    Corpus::read(BufReader::new(file)).map_err(|e| format!("{path}: {e}"))
}

fn stats(files: &[String]) -> Result<ExitCode, String> {
    let mut total = Stats::default();
    for path in files {
        total.merge(&Stats::from_corpus(&load(path)?));
    }
    print!("{}", total.to_markdown());
    Ok(ExitCode::SUCCESS)
}

fn verify_files(files: &[String]) -> Result<ExitCode, String> {
    let mut failed = false;
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
    }
    Ok(if failed {
        ExitCode::FAILURE
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
        Some((cmd, rest)) if cmd == "stats" && !rest.is_empty() => stats(rest),
        Some((cmd, rest)) if cmd == "verify" && !rest.is_empty() => verify_files(rest),
        Some((cmd, rest)) if cmd == "extract" => extract(rest),
        _ => Err(USAGE.to_string()),
    };
    result.unwrap_or_else(|message| {
        eprintln!("{message}");
        ExitCode::from(2)
    })
}
