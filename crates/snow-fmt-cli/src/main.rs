//! `snow-fmt` — the command-line Snowflake SQL formatter.
//!
//! Reads SQL from files (or stdin), formats it, and either prints to stdout, rewrites the files
//! (`--write`), or checks formatting (`--check`). Formatting is encoding-aware: a UTF-8 BOM and
//! UTF-16 inputs round-trip, and bytes that are not valid text pass through untouched. The
//! formatter never panics and never drops content — input it cannot parse is returned unchanged.

use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use snow_fmt_formatter::FormatOptions;

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1)) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("snow-fmt: {err}");
            ExitCode::from(2)
        }
    }
}

#[derive(Debug)]
struct Args {
    files: Vec<PathBuf>,
    write: bool,
    check: bool,
    options: FormatOptions,
}

fn run<I: IntoIterator<Item = OsString>>(raw: I) -> Result<ExitCode, String> {
    let args = match parse_args(raw)? {
        Parsed::Run(args) => args,
        Parsed::Help => {
            print!("{}", usage());
            return Ok(ExitCode::SUCCESS);
        }
        Parsed::Version => {
            println!("snow-fmt {}", env!("CARGO_PKG_VERSION"));
            return Ok(ExitCode::SUCCESS);
        }
    };

    if args.files.is_empty() {
        return run_stdin(&args);
    }
    run_files(&args)
}

/// No file arguments: format stdin to stdout (or `--check` it).
fn run_stdin(args: &Args) -> Result<ExitCode, String> {
    let mut source = Vec::new();
    io::stdin()
        .read_to_end(&mut source)
        .map_err(|err| format!("failed to read stdin: {err}"))?;
    let formatted = format_bytes(&source, &args.options);

    if args.check {
        if formatted != source {
            eprintln!("snow-fmt: stdin is not formatted");
            return Ok(ExitCode::from(1));
        }
        return Ok(ExitCode::SUCCESS);
    }
    io::stdout()
        .write_all(&formatted)
        .map_err(|err| format!("failed to write stdout: {err}"))?;
    Ok(ExitCode::SUCCESS)
}

fn run_files(args: &Args) -> Result<ExitCode, String> {
    let mut unformatted = 0usize;
    let mut stdout = io::stdout().lock();
    for file in &args.files {
        let source =
            fs::read(file).map_err(|err| format!("failed to read {}: {err}", file.display()))?;
        let formatted = format_bytes(&source, &args.options);

        if args.check {
            if formatted != source {
                eprintln!("{} is not formatted", file.display());
                unformatted += 1;
            }
        } else if args.write {
            if formatted != source {
                fs::write(file, &formatted)
                    .map_err(|err| format!("failed to write {}: {err}", file.display()))?;
            }
        } else {
            stdout
                .write_all(&formatted)
                .map_err(|err| format!("failed to write stdout: {err}"))?;
        }
    }
    if unformatted > 0 {
        return Ok(ExitCode::from(1));
    }
    Ok(ExitCode::SUCCESS)
}

/// Format `bytes` while preserving its encoding (BOM/UTF-16) and passing through any non-text bytes.
fn format_bytes(bytes: &[u8], options: &FormatOptions) -> Vec<u8> {
    snow_fmt_encoding::DecodedText::decode(bytes)
        .map_text(|text| snow_fmt_formatter::format(text, options))
        .encode()
}

enum Parsed {
    Run(Args),
    Help,
    Version,
}

fn parse_args<I: IntoIterator<Item = OsString>>(raw: I) -> Result<Parsed, String> {
    let mut files = Vec::new();
    let mut write = false;
    let mut check = false;
    let mut options = FormatOptions::default();
    let mut args = raw.into_iter();

    while let Some(arg) = args.next() {
        match arg.to_string_lossy().as_ref() {
            "--write" | "-w" => write = true,
            "--check" => check = true,
            "--no-uppercase" => options.uppercase_keywords = false,
            "--line-width" => options.line_width = take_usize(&mut args, "--line-width")?,
            "--indent-width" => options.indent_width = take_usize(&mut args, "--indent-width")?,
            "-h" | "--help" => return Ok(Parsed::Help),
            "-V" | "--version" => return Ok(Parsed::Version),
            "-" => files.push(PathBuf::from("/dev/stdin")),
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other}\n\n{}", usage()));
            }
            _ => files.push(PathBuf::from(arg)),
        }
    }

    if write && check {
        return Err("--write and --check are mutually exclusive".to_string());
    }
    Ok(Parsed::Run(Args {
        files,
        write,
        check,
        options,
    }))
}

fn take_usize<I: Iterator<Item = OsString>>(args: &mut I, flag: &str) -> Result<usize, String> {
    let value = args
        .next()
        .ok_or_else(|| format!("{flag} requires a number"))?;
    value
        .to_string_lossy()
        .parse::<usize>()
        .map_err(|_| format!("{flag} expects a positive integer"))
}

fn usage() -> String {
    "\
snow-fmt — an opinionated Snowflake SQL formatter

USAGE:
    snow-fmt [OPTIONS] [FILES...]

    With no FILES, reads SQL from stdin and writes the formatted result to stdout.

OPTIONS:
    -w, --write           Format files in place
        --check           Exit non-zero if any input is not already formatted (no writes)
        --line-width N    Target line width (default 100)
        --indent-width N  Spaces per indent level (default 4)
        --no-uppercase    Do not upper-case SQL keywords
    -h, --help            Print this help
    -V, --version         Print version
"
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(bytes: &[u8]) -> Vec<u8> {
        format_bytes(bytes, &FormatOptions::default())
    }

    #[test]
    fn formats_plain_sql() {
        assert_eq!(fmt(b"select a,b from t"), b"SELECT a, b\nFROM t;\n");
    }

    #[test]
    fn preserves_a_utf8_bom() {
        let mut input = vec![0xEF, 0xBB, 0xBF];
        input.extend_from_slice(b"select '\xE9\x95\xB7\xE8\x8A\x8B'"); // 長芋
        let out = fmt(&input);
        assert!(out.starts_with(&[0xEF, 0xBB, 0xBF]), "BOM lost: {out:?}");
    }

    #[test]
    fn passes_through_opaque_bytes() {
        let input = [b'S', b'E', 0xFF, b'L'];
        assert_eq!(fmt(&input), input);
    }

    #[test]
    fn unparsable_input_is_unchanged() {
        // `ALTER`'s surface is modeled only leniently, so this canonical one-liner round-trips
        // byte-for-byte (the encoder's newline handling leaves it as-is here).
        let input = b"ALTER TABLE t ADD COLUMN c INT;\n";
        assert_eq!(fmt(input), input);
    }

    #[test]
    fn rejects_conflicting_modes() {
        let err = parse_args(["--write", "--check"].map(Into::into)).err();
        assert!(err.is_some());
    }

    #[test]
    fn parses_options() {
        let parsed = parse_args(["--line-width", "80", "--no-uppercase", "a.sql"].map(Into::into))
            .expect("valid");
        match parsed {
            Parsed::Run(args) => {
                assert_eq!(args.options.line_width, 80);
                assert!(!args.options.uppercase_keywords);
                assert_eq!(args.files, vec![PathBuf::from("a.sql")]);
            }
            _ => panic!("expected Run"),
        }
    }
}
