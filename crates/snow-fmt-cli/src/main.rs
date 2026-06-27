// `sql-dialect-fmt` — the command-line SQL dialect formatter.
//
// Reads SQL from files, directories (recursed for `*.sql`), or stdin, formats it, and either
// prints to stdout, rewrites the files (`--write`), or checks formatting (`--check`). Formatting
// is encoding-aware: a UTF-8 BOM and UTF-16 inputs round-trip, and bytes that are not valid text
// pass through untouched. The formatter never panics and never drops content — input it cannot
// parse is returned unchanged, and the parse diagnostics are surfaced to stderr so a malformed
// file is reported rather than silently passed through.
//
// Configuration knobs come from three layers, lowest priority first:
//   1. the formatter's built-in defaults,
//   2. the nearest `snow-fmt.toml` discovered by walking up from each input (or the CWD),
//   3. explicit CLI flags.
//
// Exit codes:
//   * `0` — success (formatted to stdout/written, or `--check` found nothing to do),
//   * `1` — `--check` only: at least one input would be reformatted,
//   * `2` — a parse error, an I/O error, or a usage error.

mod config;

use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rayon::prelude::*;
use snow_fmt_formatter::FormatOptions;
use snow_fmt_parser::Dialect;

use config::Config;

/// Process exit codes, kept in one place so their meaning is documented and consistent. Success
/// (`0`) is expressed via [`ExitCode::SUCCESS`]; the two non-zero codes are named here.
const EXIT_CHECK_FAILED: u8 = 1;
const EXIT_ERROR: u8 = 2;

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1)) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("sql-dialect-fmt: {err}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

#[derive(Debug)]
struct Args {
    /// Paths given on the command line (files and/or directories).
    paths: Vec<PathBuf>,
    write: bool,
    check: bool,
    /// Ignore any `snow-fmt.toml` and use defaults + CLI flags only.
    no_config: bool,
    /// CLI-flag overrides, layered on top of any config file. `None` means "not set on the CLI".
    overrides: Overrides,
}

/// CLI flags that override config-file / default values. Only `Some` fields take effect.
#[derive(Clone, Copy, Debug, Default)]
struct Overrides {
    line_width: Option<usize>,
    indent_width: Option<usize>,
    uppercase_keywords: Option<bool>,
    dialect: Option<Dialect>,
}

impl Overrides {
    fn apply_to(&self, options: &mut FormatOptions) {
        if let Some(line_width) = self.line_width {
            options.line_width = line_width;
        }
        if let Some(indent_width) = self.indent_width {
            options.indent_width = indent_width;
        }
        if let Some(uppercase_keywords) = self.uppercase_keywords {
            options.uppercase_keywords = uppercase_keywords;
        }
        if let Some(dialect) = self.dialect {
            options.dialect = dialect;
        }
    }
}

fn run<I: IntoIterator<Item = OsString>>(raw: I) -> Result<ExitCode, String> {
    let args = match parse_args(raw)? {
        Parsed::Run(args) => args,
        Parsed::Help => {
            print!("{}", usage());
            return Ok(ExitCode::SUCCESS);
        }
        Parsed::Version => {
            println!("sql-dialect-fmt {}", env!("CARGO_PKG_VERSION"));
            return Ok(ExitCode::SUCCESS);
        }
    };

    if args.paths.is_empty() {
        return run_stdin(&args);
    }
    run_paths(&args)
}

/// Resolve the effective options for a single input `path` (or stdin when `path` is `None`).
///
/// Layers: defaults → nearest `snow-fmt.toml` (unless `--no-config`) → CLI overrides.
fn options_for(args: &Args, path: Option<&Path>) -> Result<FormatOptions, String> {
    let mut options = FormatOptions::default();
    if !args.no_config {
        // For stdin, anchor discovery at the current directory.
        let start = path.unwrap_or_else(|| Path::new("."));
        if let Some(config_path) = config::discover(start) {
            Config::load(&config_path)?.apply_to(&mut options);
        }
    }
    args.overrides.apply_to(&mut options);
    Ok(options)
}

/// No path arguments: format stdin to stdout (or `--check` it).
fn run_stdin(args: &Args) -> Result<ExitCode, String> {
    let options = options_for(args, None)?;
    let mut source = Vec::new();
    io::stdin()
        .read_to_end(&mut source)
        .map_err(|err| format!("failed to read stdin: {err}"))?;

    // Surface parse problems on stderr, but keep going (the formatter passes content through).
    report_parse_errors(&source, None, options.dialect);
    let formatted = format_bytes(&source, &options);

    if args.check {
        if formatted != source {
            eprintln!("sql-dialect-fmt: stdin is not formatted");
            return Ok(ExitCode::from(EXIT_CHECK_FAILED));
        }
        return Ok(ExitCode::SUCCESS);
    }
    io::stdout()
        .write_all(&formatted)
        .map_err(|err| format!("failed to write stdout: {err}"))?;
    Ok(ExitCode::SUCCESS)
}

/// Outcome of processing one file, accumulated into the run-wide summary.
#[derive(Clone, Copy, Default)]
struct Summary {
    /// Files inspected.
    total: usize,
    /// Files that were already formatted (no change needed).
    unchanged: usize,
    /// Files rewritten (`--write`).
    written: usize,
    /// Files that would change (`--check`), reported but not written.
    would_change: usize,
    /// Files that produced parse errors (still processed losslessly).
    with_errors: usize,
}

struct FileOutcome {
    formatted_stdout: Vec<u8>,
    changed: bool,
    written: bool,
    parse_errors: Vec<String>,
}

fn run_paths(args: &Args) -> Result<ExitCode, String> {
    let files = collect_files(&args.paths)?;
    let outcomes = files
        .par_iter()
        .map(|file| process_file(args, file))
        .collect::<Result<Vec<_>, _>>()?;

    let mut summary = Summary::default();
    let mut stdout = io::stdout().lock();

    for (file, outcome) in files.iter().zip(outcomes) {
        for error in &outcome.parse_errors {
            eprintln!("{error}");
        }
        if !outcome.parse_errors.is_empty() {
            summary.with_errors += 1;
        }
        summary.total += 1;

        if args.check {
            if outcome.changed {
                eprintln!("{} is not formatted", file.display());
                summary.would_change += 1;
            } else {
                summary.unchanged += 1;
            }
        } else if args.write {
            if outcome.written {
                summary.written += 1;
            } else {
                summary.unchanged += 1;
            }
        } else {
            stdout
                .write_all(&outcome.formatted_stdout)
                .map_err(|err| format!("failed to write stdout: {err}"))?;
        }
    }

    // A per-run summary on stderr, but only for the modes where it is meaningful (writing or
    // checking many files). Streaming to stdout stays clean for piping.
    if args.write {
        eprintln!(
            "sql-dialect-fmt: {} file(s); {} reformatted, {} unchanged{}",
            summary.total,
            summary.written,
            summary.unchanged,
            errors_suffix(summary.with_errors),
        );
    } else if args.check {
        if summary.would_change == 0 {
            eprintln!(
                "sql-dialect-fmt: {} file(s) already formatted{}",
                summary.total,
                errors_suffix(summary.with_errors),
            );
        } else {
            eprintln!(
                "sql-dialect-fmt: {} of {} file(s) would be reformatted{}",
                summary.would_change,
                summary.total,
                errors_suffix(summary.with_errors),
            );
        }
    }

    if args.check && summary.would_change > 0 {
        return Ok(ExitCode::from(EXIT_CHECK_FAILED));
    }
    Ok(ExitCode::SUCCESS)
}

fn process_file(args: &Args, file: &Path) -> Result<FileOutcome, String> {
    let options = options_for(args, Some(file))?;
    let source =
        fs::read(file).map_err(|err| format!("failed to read {}: {err}", file.display()))?;
    let parse_errors = collect_parse_error_messages(&source, Some(file), options.dialect);
    let formatted = format_bytes(&source, &options);
    let changed = formatted != source;
    let mut written = false;

    if args.write && changed {
        fs::write(file, &formatted)
            .map_err(|err| format!("failed to write {}: {err}", file.display()))?;
        written = true;
    }

    Ok(FileOutcome {
        formatted_stdout: if args.write || args.check {
            Vec::new()
        } else {
            formatted
        },
        changed,
        written,
        parse_errors,
    })
}

fn errors_suffix(with_errors: usize) -> String {
    if with_errors == 0 {
        String::new()
    } else {
        format!(" ({with_errors} with parse errors)")
    }
}

/// Expand the command-line paths into a flat, de-duplicated list of files to format.
///
/// Directories are recursed for `*.sql` files (case-insensitive extension); explicitly named
/// files are taken as-is regardless of extension. Order is deterministic: command-line order is
/// preserved, and files discovered under a directory are sorted by path.
fn collect_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for path in paths {
        if path.is_dir() {
            collect_dir(path, &mut out, &mut seen)?;
        } else if path.is_file() {
            push_unique(path.clone(), &mut out, &mut seen);
        } else {
            return Err(format!("no such file or directory: {}", path.display()));
        }
    }
    Ok(out)
}

fn collect_dir(
    dir: &Path,
    out: &mut Vec<PathBuf>,
    seen: &mut std::collections::BTreeSet<PathBuf>,
) -> Result<(), String> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|err| format!("failed to read directory {}: {err}", dir.display()))?
        .map(|entry| {
            entry
                .map(|e| e.path())
                .map_err(|err| format!("failed to read entry in {}: {err}", dir.display()))
        })
        .collect::<Result<_, _>>()?;
    entries.sort();

    for entry in entries {
        if entry.is_dir() {
            collect_dir(&entry, out, seen)?;
        } else if is_sql_file(&entry) {
            push_unique(entry, out, seen);
        }
    }
    Ok(())
}

fn push_unique(
    path: PathBuf,
    out: &mut Vec<PathBuf>,
    seen: &mut std::collections::BTreeSet<PathBuf>,
) {
    if seen.insert(path.clone()) {
        out.push(path);
    }
}

fn is_sql_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("sql"))
}

/// Parse `source` and print any diagnostics to stderr. Returns `true` if there were errors.
///
/// This does **not** abort processing: the formatter still round-trips the content losslessly.
/// It exists purely so malformed input is *visible* instead of silently passing through. Opaque
/// (non-text) bytes are skipped — there is nothing to parse.
fn report_parse_errors(source: &[u8], file: Option<&Path>, dialect: Dialect) -> bool {
    let messages = collect_parse_error_messages(source, file, dialect);
    for message in &messages {
        eprintln!("{message}");
    }
    !messages.is_empty()
}

fn collect_parse_error_messages(
    source: &[u8],
    file: Option<&Path>,
    dialect: Dialect,
) -> Vec<String> {
    let decoded = snow_fmt_encoding::DecodedText::decode(source);
    let Some(text) = decoded.as_str() else {
        return Vec::new();
    };
    let parse = snow_fmt_parser::parse_with_dialect(text, dialect);
    let errors = parse.errors();
    if errors.is_empty() {
        return Vec::new();
    }

    let where_ = match file {
        Some(path) => path.display().to_string(),
        None => "<stdin>".to_string(),
    };
    errors
        .iter()
        .map(|error| {
            let (line, col) = line_col(text, error.offset);
            format!(
                "sql-dialect-fmt: parse error in {where_}:{line}:{col}: {}",
                error.message
            )
        })
        .collect()
}

/// Translate a byte offset into 1-based line and column numbers (columns counted in `char`s).
fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let mut line = 1usize;
    let mut col = 1usize;
    for ch in text[..offset].chars() {
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
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
    let mut paths = Vec::new();
    let mut write = false;
    let mut check = false;
    let mut no_config = false;
    let mut overrides = Overrides::default();
    let mut args = raw.into_iter();

    while let Some(arg) = args.next() {
        match arg.to_string_lossy().as_ref() {
            "--write" | "-w" => write = true,
            "--check" => check = true,
            "--no-config" => no_config = true,
            "--no-uppercase" => overrides.uppercase_keywords = Some(false),
            "--uppercase" => overrides.uppercase_keywords = Some(true),
            "--dialect" => overrides.dialect = Some(take_dialect(&mut args, "--dialect")?),
            "--line-width" => overrides.line_width = Some(take_usize(&mut args, "--line-width")?),
            "--indent-width" => {
                overrides.indent_width = Some(take_usize(&mut args, "--indent-width")?)
            }
            "-h" | "--help" => return Ok(Parsed::Help),
            "-V" | "--version" => return Ok(Parsed::Version),
            "--" => {
                // Everything after `--` is a path, even if it looks like a flag.
                for rest in args.by_ref() {
                    paths.push(PathBuf::from(rest));
                }
                break;
            }
            other if other.starts_with("--") && other.contains('=') => {
                // `--line-width=80` style.
                let (flag, value) = other.split_once('=').expect("contains '='");
                match flag {
                    "--line-width" => overrides.line_width = Some(parse_usize(flag, value)?),
                    "--indent-width" => overrides.indent_width = Some(parse_usize(flag, value)?),
                    "--dialect" => overrides.dialect = Some(parse_dialect_flag(value)?),
                    _ => return Err(format!("unknown option {flag}\n\n{}", usage())),
                }
            }
            other if other.starts_with('-') && other != "-" => {
                return Err(format!("unknown option {other}\n\n{}", usage()));
            }
            // `-` (a lone dash) and bare words are paths.
            _ => paths.push(PathBuf::from(arg)),
        }
    }

    if write && check {
        return Err("--write and --check are mutually exclusive".to_string());
    }
    Ok(Parsed::Run(Args {
        paths,
        write,
        check,
        no_config,
        overrides,
    }))
}

fn take_usize<I: Iterator<Item = OsString>>(args: &mut I, flag: &str) -> Result<usize, String> {
    let value = args
        .next()
        .ok_or_else(|| format!("{flag} requires a number"))?;
    parse_usize(flag, value.to_string_lossy().as_ref())
}

fn take_dialect<I: Iterator<Item = OsString>>(args: &mut I, flag: &str) -> Result<Dialect, String> {
    let value = args
        .next()
        .ok_or_else(|| format!("{flag} requires a dialect"))?;
    parse_dialect_flag(value.to_string_lossy().as_ref())
}

fn parse_dialect_flag(value: &str) -> Result<Dialect, String> {
    config::parse_dialect(value)
}

fn parse_usize(flag: &str, value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("{flag} expects a non-negative integer, got {value:?}"))
}

fn usage() -> String {
    "\
sql-dialect-fmt — an opinionated SQL dialect formatter

USAGE:
    sql-dialect-fmt [OPTIONS] [PATHS...]

    PATHS may be files or directories. Directories are searched recursively for
    *.sql files. With no PATHS, reads SQL from stdin and writes the formatted
    result to stdout.

    Configuration is read from the nearest snow-fmt.toml found by walking up from
    each input (or the current directory). CLI flags override the config file.

OPTIONS:
    -w, --write           Format files in place
        --check           Exit non-zero if any input is not already formatted (no writes)
        --line-width N    Target line width (default 100)
        --indent-width N  Spaces per indent level (default 4)
        --dialect NAME    SQL dialect: snowflake or databricks (default snowflake)
        --uppercase       Upper-case SQL keywords (the default)
        --no-uppercase    Do not upper-case SQL keywords
        --no-config       Ignore any snow-fmt.toml; use defaults and flags only
    -h, --help            Print this help
    -V, --version         Print version

EXIT CODES:
    0   success
    1   --check: at least one input would be reformatted
    2   parse error, I/O error, or bad usage
"
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(bytes: &[u8]) -> Vec<u8> {
        format_bytes(bytes, &FormatOptions::default())
    }

    fn run_args(args: &[&str]) -> Args {
        match parse_args(args.iter().map(OsString::from)).expect("valid args") {
            Parsed::Run(args) => args,
            _ => panic!("expected Run"),
        }
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
        let args = run_args(&[
            "--line-width",
            "80",
            "--dialect",
            "databricks",
            "--no-uppercase",
            "a.sql",
        ]);
        assert_eq!(args.overrides.line_width, Some(80));
        assert_eq!(args.overrides.dialect, Some(Dialect::Databricks));
        assert_eq!(args.overrides.uppercase_keywords, Some(false));
        assert_eq!(args.paths, vec![PathBuf::from("a.sql")]);
    }

    #[test]
    fn parses_eq_style_options() {
        let args = run_args(&["--line-width=70", "--indent-width=2", "--dialect=snowflake"]);
        assert_eq!(args.overrides.line_width, Some(70));
        assert_eq!(args.overrides.indent_width, Some(2));
        assert_eq!(args.overrides.dialect, Some(Dialect::Snowflake));
    }

    #[test]
    fn double_dash_treats_rest_as_paths() {
        let args = run_args(&["--", "--check", "-w"]);
        assert!(!args.check);
        assert!(!args.write);
        assert_eq!(
            args.paths,
            vec![PathBuf::from("--check"), PathBuf::from("-w")]
        );
    }

    #[test]
    fn lone_dash_is_a_path() {
        let args = run_args(&["-"]);
        assert_eq!(args.paths, vec![PathBuf::from("-")]);
    }

    #[test]
    fn unknown_option_errors() {
        assert!(parse_args(["--frobnicate"].map(Into::into)).is_err());
    }

    #[test]
    fn missing_numeric_arg_errors() {
        assert!(parse_args(["--line-width"].map(Into::into)).is_err());
    }

    #[test]
    fn non_numeric_arg_errors() {
        assert!(parse_args(["--line-width", "wide"].map(Into::into)).is_err());
    }

    #[test]
    fn invalid_dialect_arg_errors() {
        assert!(parse_args(["--dialect", "oracle"].map(Into::into)).is_err());
        assert!(parse_args(["--dialect"].map(Into::into)).is_err());
    }

    #[test]
    fn line_col_maps_offsets() {
        let text = "abc\ndefg\nhi";
        assert_eq!(line_col(text, 0), (1, 1));
        assert_eq!(line_col(text, 4), (2, 1)); // first char of line 2
        assert_eq!(line_col(text, 6), (2, 3));
        // Out-of-range offsets clamp to the end rather than panicking.
        assert_eq!(line_col(text, 999), (3, 3));
    }

    #[test]
    fn overrides_layer_over_defaults() {
        let mut options = FormatOptions::default();
        let overrides = Overrides {
            line_width: Some(42),
            dialect: Some(Dialect::Databricks),
            ..Overrides::default()
        };
        overrides.apply_to(&mut options);
        assert_eq!(options.line_width, 42);
        assert_eq!(options.dialect, Dialect::Databricks);
        assert_eq!(options.indent_width, 4);
    }
}
