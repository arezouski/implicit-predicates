//! The command line tool: generates the stages and the checks of every
//! predicate defined in a directory and writes them, formatted with rustfmt,
//! or checks that the files already there are up to date.
//!
//! ```text
//! indirect-predicates-gen <pred-dir> --stages <dir> --checks <dir> [--check]
//! ```
//!
//! The header of every file written gives the command that regenerates it:
//! this one, with the same arguments and without `--check`. The paths must
//! be relative to the directory the command is run from, so that the header
//! does not depend on where the files are; absolute paths are refused. The comparison
//! of `--check` includes the header, so it must be run with the arguments
//! the files were written with.

use std::ffi::OsStr;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use indirect_predicates_gen::generate;

const USAGE: &str =
    "usage: indirect-predicates-gen <pred-dir> --stages <dir> --checks <dir> [--check]";

/// The parsed command line.
struct Options {
    definitions: PathBuf,
    stages: PathBuf,
    checks: PathBuf,
    check: bool,
}

/// A file the tool writes, or compares with in check mode.
struct Output {
    path: PathBuf,
    contents: String,
}

fn main() -> ExitCode {
    let options = match parse_args(std::env::args_os().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(messages) => {
            for message in messages {
                eprintln!("{message}");
            }
            ExitCode::FAILURE
        }
    }
}

fn parse_args(args: impl Iterator<Item = std::ffi::OsString>) -> Result<Options, String> {
    let (mut definitions, mut stages, mut checks, mut check) = (None, None, None, false);
    let mut args = args;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{name} needs a directory"))
        };
        match arg.to_str() {
            Some("--stages") => stages = Some(value("--stages")?),
            Some("--checks") => checks = Some(value("--checks")?),
            Some("--check") => check = true,
            Some(flag) if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ if definitions.is_none() => definitions = Some(PathBuf::from(arg)),
            _ => return Err("only one directory of definitions is allowed".to_string()),
        }
    }
    let options = Options {
        definitions: definitions.ok_or("missing the directory of definitions")?,
        stages: stages.ok_or("missing --stages")?,
        checks: checks.ok_or("missing --checks")?,
        check,
    };
    // The paths go into the header of the generated files.
    for path in [&options.definitions, &options.stages, &options.checks] {
        if path.is_absolute() {
            return Err(format!(
                "{} is an absolute path: pass paths relative to the directory the command \
                 is run from, so the generated header is reproducible",
                path.display()
            ));
        }
    }
    Ok(options)
}

/// Generates and formats everything first, so that nothing is written when
/// any definition fails; then writes or compares.
fn run(options: &Options) -> Result<(), Vec<String>> {
    let outputs = outputs(options).map_err(|message| vec![message])?;
    if options.check {
        let stale: Vec<String> = outputs.iter().filter_map(compare).collect();
        if stale.is_empty() { Ok(()) } else { Err(stale) }
    } else {
        for output in &outputs {
            write(output).map_err(|message| vec![message])?;
        }
        Ok(())
    }
}

/// The formatted files of every predicate in the directory of definitions.
fn outputs(options: &Options) -> Result<Vec<Output>, String> {
    let dir = &options.definitions;
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut files = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
            .path();
        if path.extension() == Some(OsStr::new("pred")) {
            files.push(path);
        }
    }
    files.sort();
    if files.is_empty() {
        return Err(format!("no .pred files in {}", dir.display()));
    }
    // The header names the file relative to the parent of the directory,
    // so that it does not depend on where the tool runs.
    let label_dir = dir.file_name().map(Path::new).unwrap_or(Path::new(""));
    let regenerate = regeneration_command(options);
    let mut names: Vec<(String, PathBuf)> = Vec::new();
    let mut outputs = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let label = label_dir.join(path.file_name().expect("a file"));
        let generated =
            generate(&source, &label.to_string_lossy(), &regenerate).map_err(|mut e| {
                e.file = path.display().to_string();
                e.to_string()
            })?;
        for g in generated {
            if let Some((_, other)) = names.iter().find(|(name, _)| *name == g.name) {
                return Err(format!(
                    "{}: the predicate `{}` is also defined in {}",
                    path.display(),
                    g.name,
                    other.display()
                ));
            }
            names.push((g.name.clone(), path.clone()));
            let file = format!("{}.rs", g.name);
            outputs.push(Output {
                path: options.stages.join(&file),
                contents: rustfmt(&g.stages)?,
            });
            // A predicate without checks has no checks file.
            if !g.checks.is_empty() {
                outputs.push(Output {
                    path: options.checks.join(&file),
                    contents: rustfmt(&g.checks)?,
                });
            }
        }
    }
    Ok(outputs)
}

/// The text of the header that tells how to regenerate the files: this
/// command, with the arguments it was given except `--check`, to be run from
/// the directory that relative paths are relative to.
fn regeneration_command(options: &Options) -> String {
    let mut command = String::from(env!("CARGO_BIN_NAME"));
    let arguments = [
        (None, &options.definitions),
        (Some("--stages"), &options.stages),
        (Some("--checks"), &options.checks),
    ];
    for (flag, path) in arguments {
        if let Some(flag) = flag {
            command.push(' ');
            command.push_str(flag);
        }
        command.push(' ');
        command.push_str(&quoted(&path.to_string_lossy()));
    }
    format!("run from the directory that the paths are relative to:\n{command}")
}

/// `argument` as a shell word: unchanged if it only has characters that a
/// shell does not interpret, otherwise in single quotes.
fn quoted(argument: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "_-./=+,:@%".contains(c);
    if !argument.is_empty() && argument.chars().all(plain) {
        argument.to_string()
    } else {
        format!("'{}'", argument.replace('\'', "'\\''"))
    }
}

/// Formats `code` with `rustfmt --edition 2024`, as `cargo fmt` does.
fn rustfmt(code: &str) -> Result<String, String> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| match e.kind() {
            ErrorKind::NotFound => {
                "cannot run rustfmt: it was not found; install it with `rustup component add rustfmt`"
                    .to_string()
            }
            _ => format!("cannot run rustfmt: {e}"),
        })?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    // Write from another thread so that a full stdout pipe cannot block.
    let input = code.to_string();
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child
        .wait_with_output()
        .map_err(|e| format!("cannot run rustfmt: {e}"))?;
    writer
        .join()
        .expect("the writer does not panic")
        .map_err(|e| format!("cannot write to rustfmt: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "rustfmt rejected the generated code (a bug in the generator):\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|_| "rustfmt wrote invalid UTF-8".to_string())
}

/// The message for a file that is missing or differs, if it does.
fn compare(output: &Output) -> Option<String> {
    match fs::read(&output.path) {
        Ok(contents) if contents == output.contents.as_bytes() => None,
        Ok(_) => Some(format!(
            "{} differs from the generated code",
            output.path.display()
        )),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            Some(format!("{} is missing", output.path.display()))
        }
        Err(e) => Some(format!("cannot read {}: {e}", output.path.display())),
    }
}

fn write(output: &Output) -> Result<(), String> {
    let path = &output.path;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    fs::write(path, &output.contents).map_err(|e| format!("cannot write {}: {e}", path.display()))
}
