// `sql-dialect-fmt` — the command-line SQL dialect formatter.
//
// Reads SQL from files, directories (recursed for `*.sql`), or stdin (no paths or `-`), formats it,
// and either prints to stdout, rewrites the files (`--write`), checks formatting (`--check`), or
// lints it (`--lint`). Formatting is encoding-aware: a UTF-8 BOM and UTF-16 inputs round-trip, and
// bytes that are not valid text pass through untouched. The formatter never panics and never drops
// content — input it cannot parse is returned unchanged, and the parse diagnostics are surfaced to
// stderr so a malformed file is reported rather than silently passed through.
//
// Configuration knobs come from three layers, lowest priority first:
//   1. the formatter's built-in defaults,
//   2. the nearest `sql-dialect-fmt.toml` discovered by walking up from each input (or the CWD),
//   3. explicit CLI flags.
//
// Exit codes:
//   * `0` — success (formatted to stdout/written, `--check` found nothing to do, or `--lint`
//     found no findings),
//   * `1` — `--check`: at least one input would be reformatted; `--lint`: at least one finding,
//   * `2` — a parse error, an I/O error, or a usage error.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use rayon::prelude::*;
use sql_dialect_fmt_encoding::DecodedText;
use sql_dialect_fmt_formatter::{format_range, FormatOptions, KeywordCase, LineEnding};
use sql_dialect_fmt_lint::LintOptions;
use sql_dialect_fmt_parser::{Dialect, ParseError};
use sql_dialect_fmt_text::{LineColumn, LineIndex};

use sql_dialect_fmt_config::{self as config, Config};

/// Process exit codes, kept in one place so their meaning is documented and consistent. Success
/// (`0`) is expressed via [`ExitCode::SUCCESS`]; the two non-zero codes are named here.
const EXIT_CHECK_FAILED: u8 = 1;
const EXIT_LINT_FINDINGS: u8 = 1;
const EXIT_ERROR: u8 = 2;
const DEFAULT_EXCLUDE_PATTERNS: &[&str] = &[
    ".git",
    ".git/**",
    "**/.git",
    "**/.git/**",
    "node_modules",
    "node_modules/**",
    "**/node_modules",
    "**/node_modules/**",
    "target",
    "target/**",
    "**/target",
    "**/target/**",
];

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
    /// Lint the inputs instead of formatting them.
    lint: bool,
    /// Show a unified diff when `--check` finds unformatted input.
    diff: bool,
    /// File path context for stdin, used for config discovery and diagnostics.
    stdin_filepath: Option<PathBuf>,
    /// Reformat only the statements intersecting this byte range (stdin mode only).
    range: Option<(usize, usize)>,
    /// Ignore any `sql-dialect-fmt.toml` and use defaults + CLI flags only.
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
    keyword_case: Option<KeywordCase>,
    line_ending: Option<LineEnding>,
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
            *options = (*options).with_uppercase_keywords(uppercase_keywords);
        }
        if let Some(keyword_case) = self.keyword_case {
            *options = (*options).with_keyword_case(keyword_case);
        }
        if let Some(line_ending) = self.line_ending {
            options.line_ending = line_ending;
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

    if args.lint {
        return run_lint(&args);
    }
    if args.paths.is_empty() || args.paths.iter().any(|path| is_stdin_path(path)) {
        return run_stdin(&args);
    }
    run_paths(&args)
}

/// `--lint`: report lint findings as `path:line:col: SDFxxx message` instead of formatting.
///
/// The dialect follows the same defaults → `sql-dialect-fmt.toml` → CLI flag layering as
/// formatting; lint rules run with their defaults, and per-line suppression comments
/// (`-- sql-dialect-fmt: disable-next-line SDFxxx`) are honored. Exits `1` when any finding is
/// reported and `0` otherwise.
fn run_lint(args: &Args) -> Result<ExitCode, String> {
    if args.paths.is_empty() || args.paths.iter().any(|path| is_stdin_path(path)) {
        return run_lint_stdin(args);
    }

    let files = collect_files(args)?;
    let options = OptionsResolver::new(args);
    let mut stdout = io::stdout().lock();
    let mut findings = 0usize;
    for file in &files {
        let options = options.for_file(file)?;
        let source =
            fs::read(file).map_err(|err| format!("failed to read {}: {err}", file.display()))?;
        let decoded = DecodedText::decode(&source);
        // Non-text input (e.g. binary) has nothing to lint; skip it like formatting passes it
        // through.
        let Some(text) = decoded.as_str() else {
            continue;
        };
        findings += write_lint_findings(&mut stdout, &file.display().to_string(), text, &options)?;
    }
    Ok(lint_exit_code(findings))
}

fn run_lint_stdin(args: &Args) -> Result<ExitCode, String> {
    let options = options_for(args, args.stdin_filepath.as_deref())?;
    let mut source = Vec::new();
    io::stdin()
        .read_to_end(&mut source)
        .map_err(|err| format!("failed to read stdin: {err}"))?;
    let decoded = DecodedText::decode(&source);
    let Some(text) = decoded.as_str() else {
        return Ok(ExitCode::SUCCESS);
    };
    let label = args
        .stdin_filepath
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<stdin>".to_string());
    let findings = write_lint_findings(&mut io::stdout().lock(), &label, text, &options)?;
    Ok(lint_exit_code(findings))
}

/// Lint `text` and print one `label:line:col: SDFxxx message` line per finding (1-based
/// line/column). Returns the number of findings printed.
fn write_lint_findings(
    stdout: &mut impl Write,
    label: &str,
    text: &str,
    options: &FormatOptions,
) -> Result<usize, String> {
    let findings =
        sql_dialect_fmt_lint::lint_with_dialect(text, options.dialect, LintOptions::default());
    let index = LineIndex::new(text);
    for finding in &findings {
        let position = index.line_column(finding.range.start);
        writeln!(
            stdout,
            "{label}:{}:{}: {} {}",
            position.line,
            position.column,
            finding.code.as_str(),
            finding.message
        )
        .map_err(|err| format!("failed to write stdout: {err}"))?;
    }
    Ok(findings.len())
}

fn lint_exit_code(findings: usize) -> ExitCode {
    if findings > 0 {
        ExitCode::from(EXIT_LINT_FINDINGS)
    } else {
        ExitCode::SUCCESS
    }
}

/// Resolve the effective options for a single input `path` (or stdin when `path` is `None`).
///
/// Layers: defaults → nearest `sql-dialect-fmt.toml` (unless `--no-config`) → CLI overrides.
fn options_for(args: &Args, path: Option<&Path>) -> Result<FormatOptions, String> {
    options_for_start(args, path.unwrap_or_else(|| Path::new(".")))
}

fn options_for_start(args: &Args, start: &Path) -> Result<FormatOptions, String> {
    let mut options = FormatOptions::default();
    if !args.no_config {
        if let Some(config_path) = config::discover(start) {
            Config::load(&config_path)?.apply_to(&mut options);
        }
    }
    args.overrides.apply_to(&mut options);
    validate_options(&options)?;
    Ok(options)
}

fn validate_options(options: &FormatOptions) -> Result<(), String> {
    if options.line_width == 0 {
        return Err("line_width must be greater than 0".to_string());
    }
    if options.indent_width == 0 {
        return Err("indent_width must be greater than 0".to_string());
    }
    Ok(())
}

struct OptionsResolver<'a> {
    args: &'a Args,
    by_dir: Mutex<HashMap<PathBuf, Result<FormatOptions, String>>>,
}

impl<'a> OptionsResolver<'a> {
    fn new(args: &'a Args) -> Self {
        Self {
            args,
            by_dir: Mutex::new(HashMap::new()),
        }
    }

    fn for_file(&self, file: &Path) -> Result<FormatOptions, String> {
        let dir = file
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut cache = self.by_dir.lock().expect("config cache poisoned");
        if let Some(cached) = cache.get(&dir) {
            return cached.clone();
        }
        let resolved = options_for_start(self.args, &dir);
        cache.insert(dir, resolved.clone());
        resolved
    }
}

/// No path arguments: format stdin to stdout (or `--check` it).
fn run_stdin(args: &Args) -> Result<ExitCode, String> {
    let stdin_path = args.stdin_filepath.as_deref();
    let options = options_for(args, stdin_path)?;
    let mut source = Vec::new();
    io::stdin()
        .read_to_end(&mut source)
        .map_err(|err| format!("failed to read stdin: {err}"))?;

    if let Some((start, end)) = args.range {
        return run_stdin_range(&source, &options, start, end);
    }

    // Surface parse problems on stderr, but keep going (the formatter passes content through).
    let decoded = DecodedText::decode(&source);
    let formatted = format_decoded_with_diagnostics(&decoded, &options);
    let parse_errors = collect_parse_error_messages(&formatted.parse_errors, stdin_path);
    for error in &parse_errors {
        eprintln!("{error}");
    }

    if args.check {
        if formatted.bytes != source {
            if args.diff {
                let label = stdin_display_name(args);
                write_diff(&mut io::stdout().lock(), &label, &source, &formatted.bytes)?;
            }
            eprintln!(
                "sql-dialect-fmt: {} is not formatted",
                stdin_display_name(args)
            );
            return Ok(ExitCode::from(EXIT_CHECK_FAILED));
        }
        return Ok(ExitCode::SUCCESS);
    }
    io::stdout()
        .write_all(&formatted.bytes)
        .map_err(|err| format!("failed to write stdout: {err}"))?;
    Ok(ExitCode::SUCCESS)
}

/// Reformat only the statements intersecting `[start, end)` (byte offsets into the decoded text) and
/// write the whole document to stdout with the rest kept verbatim. Non-text input (e.g. binary or a
/// range that touches no statement) round-trips unchanged.
fn run_stdin_range(
    source: &[u8],
    options: &FormatOptions,
    start: usize,
    end: usize,
) -> Result<ExitCode, String> {
    let decoded = DecodedText::decode(source);
    let spliced = decoded.as_str().and_then(|text| {
        let edit = format_range(text, start..end, options)?;
        let mut out = String::with_capacity(text.len() + edit.new_text.len());
        out.push_str(&text[..edit.range.start]);
        out.push_str(&edit.new_text);
        out.push_str(&text[edit.range.end..]);
        Some(out)
    });
    let bytes = match spliced {
        Some(text) => decoded.map_text(|_| text).encode(),
        None => decoded.encode(),
    };
    io::stdout()
        .write_all(&bytes)
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
    diff_stdout: Option<String>,
    changed: bool,
    written: bool,
    parse_errors: Vec<String>,
}

fn run_paths(args: &Args) -> Result<ExitCode, String> {
    let files = collect_files(args)?;
    let options = OptionsResolver::new(args);
    let outcomes = files
        .par_iter()
        .map(|file| process_file(args, &options, file))
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
                if let Some(diff) = &outcome.diff_stdout {
                    stdout
                        .write_all(diff.as_bytes())
                        .map_err(|err| format!("failed to write stdout: {err}"))?;
                }
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

fn process_file(
    args: &Args,
    options: &OptionsResolver<'_>,
    file: &Path,
) -> Result<FileOutcome, String> {
    let options = options.for_file(file)?;
    let source =
        fs::read(file).map_err(|err| format!("failed to read {}: {err}", file.display()))?;
    let decoded = DecodedText::decode(&source);
    let formatted = format_decoded_with_diagnostics(&decoded, &options);
    let parse_errors = collect_parse_error_messages(&formatted.parse_errors, Some(file));
    let changed = formatted.bytes != source;
    let mut written = false;

    if args.write && changed {
        fs::write(file, &formatted.bytes)
            .map_err(|err| format!("failed to write {}: {err}", file.display()))?;
        written = true;
    }

    let diff_stdout = if args.check && args.diff && changed {
        Some(unified_diff(
            &file.display().to_string(),
            &source,
            &formatted.bytes,
        ))
    } else {
        None
    };

    Ok(FileOutcome {
        formatted_stdout: if args.write || args.check {
            Vec::new()
        } else {
            formatted.bytes
        },
        diff_stdout,
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
fn collect_files(args: &Args) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for path in &args.paths {
        if path.is_dir() {
            let exclusions = Exclusions::for_input(args, path)?;
            collect_dir(path, &exclusions, &mut out, &mut seen)?;
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
    exclusions: &Exclusions,
    out: &mut Vec<PathBuf>,
    seen: &mut std::collections::BTreeSet<PathBuf>,
) -> Result<(), String> {
    let mut walker = WalkBuilder::new(dir);
    walker.standard_filters(true);
    let root = dir.to_path_buf();
    let exclusions = exclusions.clone();
    walker.filter_entry(move |entry| !exclusions.matches(&root, entry.path()));

    let mut files = Vec::new();
    for entry in walker.build() {
        let entry = entry.map_err(|err| format!("failed to walk {}: {err}", dir.display()))?;
        let path = entry.path();
        if path == dir {
            continue;
        }
        if entry.file_type().is_some_and(|kind| kind.is_file()) && is_sql_file(path) {
            files.push(path.to_path_buf());
        }
    }
    files.sort();
    for file in files {
        push_unique(file, out, seen);
    }
    Ok(())
}

#[derive(Clone)]
struct Exclusions {
    globset: GlobSet,
}

impl Exclusions {
    fn for_input(args: &Args, input: &Path) -> Result<Self, String> {
        let mut patterns = DEFAULT_EXCLUDE_PATTERNS
            .iter()
            .map(|pattern| (*pattern).to_string())
            .collect::<Vec<_>>();

        if !args.no_config {
            if let Some(config_path) = config::discover(input) {
                patterns.extend(Config::load(&config_path)?.exclude);
            }
        }

        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            builder.add(
                Glob::new(&pattern)
                    .map_err(|err| format!("invalid exclude pattern {pattern:?}: {err}"))?,
            );
        }
        let globset = builder
            .build()
            .map_err(|err| format!("invalid exclude patterns: {err}"))?;
        Ok(Self { globset })
    }

    fn matches(&self, root: &Path, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(root) else {
            return false;
        };
        if relative.as_os_str().is_empty() {
            return false;
        }
        self.globset.is_match(relative)
    }
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

fn is_stdin_path(path: &Path) -> bool {
    path == Path::new("-")
}

fn stdin_display_name(args: &Args) -> String {
    args.stdin_filepath
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "stdin".to_string())
}

fn collect_parse_error_messages(errors: &[ParseError], file: Option<&Path>) -> Vec<String> {
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
            let position = error.line_column.unwrap_or_else(|| LineColumn::new(1, 1));
            format!(
                "sql-dialect-fmt: parse error in {where_}:{}:{}: {}",
                position.line, position.column, error.message
            )
        })
        .collect()
}

/// Format `bytes` while preserving its encoding (BOM/UTF-16) and passing through any non-text bytes.
#[cfg(test)]
fn format_bytes(bytes: &[u8], options: &FormatOptions) -> Vec<u8> {
    let decoded = DecodedText::decode(bytes);
    format_decoded(&decoded, options)
}

#[cfg(test)]
fn format_decoded(decoded: &DecodedText, options: &FormatOptions) -> Vec<u8> {
    format_decoded_with_diagnostics(decoded, options).bytes
}

struct FormattedBytes {
    bytes: Vec<u8>,
    parse_errors: Vec<ParseError>,
}

fn format_decoded_with_diagnostics(
    decoded: &DecodedText,
    options: &FormatOptions,
) -> FormattedBytes {
    let Some(text) = decoded.as_str() else {
        return FormattedBytes {
            bytes: decoded.encode(),
            parse_errors: Vec::new(),
        };
    };
    let result = sql_dialect_fmt_formatter::format_with_diagnostics(text, options);
    let parse_errors = result.parse_errors;
    let formatted = result.formatted;
    FormattedBytes {
        bytes: decoded.map_text(|_| formatted).encode(),
        parse_errors,
    }
}

fn write_diff(
    stdout: &mut impl Write,
    label: &str,
    original: &[u8],
    formatted: &[u8],
) -> Result<(), String> {
    stdout
        .write_all(unified_diff(label, original, formatted).as_bytes())
        .map_err(|err| format!("failed to write stdout: {err}"))
}

fn unified_diff(label: &str, original: &[u8], formatted: &[u8]) -> String {
    let original_decoded = sql_dialect_fmt_encoding::DecodedText::decode(original);
    let formatted_decoded = sql_dialect_fmt_encoding::DecodedText::decode(formatted);
    let original_text = original_decoded.as_str().unwrap_or("");
    let formatted_text = formatted_decoded.as_str().unwrap_or("");
    unified_text_diff(label, original_text, formatted_text)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DiffLine<'a> {
    text: &'a str,
    has_newline: bool,
}

fn unified_text_diff(label: &str, original: &str, formatted: &str) -> String {
    let original_lines = diff_lines(original);
    let formatted_lines = diff_lines(formatted);
    let mut prefix = 0;
    while prefix < original_lines.len()
        && prefix < formatted_lines.len()
        && original_lines[prefix] == formatted_lines[prefix]
    {
        prefix += 1;
    }

    let mut suffix = 0;
    while suffix < original_lines.len().saturating_sub(prefix)
        && suffix < formatted_lines.len().saturating_sub(prefix)
        && original_lines[original_lines.len() - 1 - suffix]
            == formatted_lines[formatted_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }

    const CONTEXT_LINES: usize = 3;
    let original_change_end = original_lines.len() - suffix;
    let formatted_change_end = formatted_lines.len() - suffix;
    let context_start = prefix.saturating_sub(CONTEXT_LINES);
    let suffix_context = suffix.min(CONTEXT_LINES);
    let original_context_end = original_change_end + suffix_context;
    let formatted_context_end = formatted_change_end + suffix_context;
    let original_count = original_context_end - context_start;
    let formatted_count = formatted_context_end - context_start;

    let mut out = String::new();
    out.push_str("--- ");
    out.push_str(label);
    out.push('\n');
    out.push_str("+++ ");
    out.push_str(label);
    out.push('\n');
    out.push_str("@@ -");
    out.push_str(&diff_range(context_start, original_count));
    out.push_str(" +");
    out.push_str(&diff_range(context_start, formatted_count));
    out.push_str(" @@\n");

    for line in &original_lines[context_start..prefix] {
        push_diff_line(&mut out, ' ', line);
    }
    for line in &original_lines[prefix..original_change_end] {
        push_diff_line(&mut out, '-', line);
    }
    for line in &formatted_lines[prefix..formatted_change_end] {
        push_diff_line(&mut out, '+', line);
    }
    for line in &original_lines[original_change_end..original_context_end] {
        push_diff_line(&mut out, ' ', line);
    }
    out
}

fn diff_lines(text: &str) -> Vec<DiffLine<'_>> {
    text.split_inclusive('\n')
        .map(|line| DiffLine {
            text: line.strip_suffix('\n').unwrap_or(line),
            has_newline: line.ends_with('\n'),
        })
        .collect()
}

fn diff_range(context_start: usize, count: usize) -> String {
    if count == 0 {
        "0,0".to_string()
    } else {
        format!("{},{}", context_start + 1, count)
    }
}

fn push_diff_line(out: &mut String, marker: char, line: &DiffLine<'_>) {
    out.push(marker);
    out.push_str(line.text);
    out.push('\n');
    if !line.has_newline {
        out.push_str("\\ No newline at end of file\n");
    }
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
    let mut lint = false;
    let mut diff = false;
    let mut stdin_filepath = None;
    let mut range = None;
    let mut no_config = false;
    let mut overrides = Overrides::default();
    let mut args = raw.into_iter();

    while let Some(arg) = args.next() {
        match arg.to_string_lossy().as_ref() {
            "--write" | "-w" => write = true,
            "--check" => check = true,
            "--lint" => lint = true,
            "--diff" => diff = true,
            "--no-config" => no_config = true,
            "--no-uppercase" => overrides.uppercase_keywords = Some(false),
            "--uppercase" => overrides.uppercase_keywords = Some(true),
            "--keyword-case" => {
                overrides.keyword_case = Some(take_keyword_case(&mut args, "--keyword-case")?)
            }
            "--line-ending" => {
                overrides.line_ending = Some(take_line_ending(&mut args, "--line-ending")?)
            }
            "--dialect" => overrides.dialect = Some(take_dialect(&mut args, "--dialect")?),
            "--line-width" => overrides.line_width = Some(take_usize(&mut args, "--line-width")?),
            "--indent-width" => {
                overrides.indent_width = Some(take_usize(&mut args, "--indent-width")?)
            }
            "--stdin-filepath" => stdin_filepath = Some(take_path(&mut args, "--stdin-filepath")?),
            "--range" => range = Some(take_range(&mut args, "--range")?),
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
                if let Some((flag, value)) = other.split_once('=') {
                    match flag {
                        "--line-width" => overrides.line_width = Some(parse_usize(flag, value)?),
                        "--indent-width" => {
                            overrides.indent_width = Some(parse_usize(flag, value)?)
                        }
                        "--dialect" => overrides.dialect = Some(parse_dialect_flag(value)?),
                        "--keyword-case" => {
                            overrides.keyword_case = Some(parse_keyword_case_flag(value)?)
                        }
                        "--line-ending" => {
                            overrides.line_ending = Some(parse_line_ending_flag(value)?)
                        }
                        "--stdin-filepath" => stdin_filepath = Some(parse_path_flag(flag, value)?),
                        "--range" => range = Some(parse_range(flag, value)?),
                        _ => return Err(format!("unknown option {flag}\n\n{}", usage())),
                    }
                } else {
                    return Err(format!("unknown option {other}\n\n{}", usage()));
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
    if lint && (write || check) {
        return Err("--lint cannot be combined with --write or --check".to_string());
    }
    if lint && diff {
        return Err("--lint cannot be combined with --diff".to_string());
    }
    if diff && !check {
        return Err("--diff requires --check".to_string());
    }
    let stdin_path_count = paths.iter().filter(|path| is_stdin_path(path)).count();
    if stdin_path_count > 1 {
        return Err("stdin input '-' may only be used once".to_string());
    }
    if stdin_path_count == 1 && paths.len() > 1 {
        return Err("stdin input '-' cannot be combined with file or directory paths".to_string());
    }
    if stdin_filepath.is_some() && !paths.is_empty() && stdin_path_count == 0 {
        return Err("--stdin-filepath requires stdin input (no paths or '-')".to_string());
    }
    if range.is_some() {
        if write || check || lint {
            return Err("--range cannot be combined with --write, --check, or --lint".to_string());
        }
        if paths.len() - stdin_path_count > 0 {
            return Err("--range requires stdin input (no file paths)".to_string());
        }
    }
    Ok(Parsed::Run(Args {
        paths,
        write,
        check,
        lint,
        diff,
        stdin_filepath,
        range,
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

fn take_keyword_case<I: Iterator<Item = OsString>>(
    args: &mut I,
    flag: &str,
) -> Result<KeywordCase, String> {
    let value = args
        .next()
        .ok_or_else(|| format!("{flag} requires a keyword case"))?;
    parse_keyword_case_flag(value.to_string_lossy().as_ref())
}

fn take_line_ending<I: Iterator<Item = OsString>>(
    args: &mut I,
    flag: &str,
) -> Result<LineEnding, String> {
    let value = args
        .next()
        .ok_or_else(|| format!("{flag} requires a line ending"))?;
    parse_line_ending_flag(value.to_string_lossy().as_ref())
}

fn take_path<I: Iterator<Item = OsString>>(args: &mut I, flag: &str) -> Result<PathBuf, String> {
    let value = args
        .next()
        .ok_or_else(|| format!("{flag} requires a path"))?;
    if value.as_os_str().is_empty() {
        return Err(format!("{flag} requires a non-empty path"));
    }
    Ok(PathBuf::from(value))
}

fn take_range<I: Iterator<Item = OsString>>(
    args: &mut I,
    flag: &str,
) -> Result<(usize, usize), String> {
    let value = args
        .next()
        .ok_or_else(|| format!("{flag} requires START:END byte offsets"))?;
    parse_range(flag, value.to_string_lossy().as_ref())
}

fn parse_range(flag: &str, value: &str) -> Result<(usize, usize), String> {
    let malformed = || format!("{flag} expects START:END byte offsets, got {value:?}");
    let (start, end) = value.split_once(':').ok_or_else(malformed)?;
    let start = start.parse::<usize>().map_err(|_| malformed())?;
    let end = end.parse::<usize>().map_err(|_| malformed())?;
    if start > end {
        return Err(format!("{flag} START must not exceed END, got {value:?}"));
    }
    Ok((start, end))
}

fn parse_dialect_flag(value: &str) -> Result<Dialect, String> {
    config::parse_dialect(value)
}

fn parse_keyword_case_flag(value: &str) -> Result<KeywordCase, String> {
    config::parse_keyword_case(value)
}

fn parse_line_ending_flag(value: &str) -> Result<LineEnding, String> {
    config::parse_line_ending(value)
}

fn parse_usize(flag: &str, value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{flag} expects a positive integer, got {value:?}"))?;
    if parsed == 0 {
        return Err(format!("{flag} expects a positive integer, got {value:?}"));
    }
    Ok(parsed)
}

fn parse_path_flag(flag: &str, value: &str) -> Result<PathBuf, String> {
    if value.is_empty() {
        return Err(format!("{flag} requires a non-empty path"));
    }
    Ok(PathBuf::from(value))
}

fn usage() -> String {
    "\
sql-dialect-fmt — an opinionated SQL dialect formatter

USAGE:
    sql-dialect-fmt [OPTIONS] [PATHS...]

    PATHS may be files or directories. Directories are searched recursively for
    *.sql files. With no PATHS or with PATHS set to -, reads SQL from stdin and
    writes the formatted result to stdout.

    Configuration is read from the nearest sql-dialect-fmt.toml found by walking up from
    each input (or --stdin-filepath/current directory for stdin). CLI flags override the
    config file.

OPTIONS:
    -w, --write           Format files in place
        --check           Exit non-zero if any input is not already formatted (no writes)
        --lint            Lint instead of formatting: print findings as
                           path:line:col: SDFxxx message and exit 1 when any exist.
                           Rules: SELECT * (SDF001), large IN list (SDF002), unsupported
                           embedded language (SDF003), DELETE/UPDATE without WHERE
                           (SDF004/SDF005), comma join (SDF006), ORDER BY ordinal (SDF007).
                           Suppress per line with -- sql-dialect-fmt: disable-next-line SDFxxx
        --diff            With --check, print a unified diff for unformatted input
        --stdin-filepath PATH
                           File path context for stdin config discovery and diagnostics
        --range START:END Reformat only statements intersecting the byte range [START, END)
                           (stdin only; prints the whole document to stdout)
        --line-width N    Target line width (default 100)
        --indent-width N  Spaces per indent level (default 4)
        --dialect NAME    SQL dialect: snowflake or databricks (default snowflake)
        --keyword-case NAME
                           Keyword case: upper, lower, or preserve (default upper)
        --line-ending NAME
                           Output line endings: auto, lf, or crlf (default lf)
        --uppercase       Upper-case SQL keywords (the default)
        --no-uppercase    Do not upper-case SQL keywords
        --no-config       Ignore any sql-dialect-fmt.toml; use defaults and flags only
    -h, --help            Print this help
    -V, --version         Print version

EXIT CODES:
    0   success
    1   --check: at least one input would be reformatted; --lint: at least one finding
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
            "--keyword-case",
            "lower",
            "--line-ending",
            "crlf",
            "a.sql",
        ]);
        assert_eq!(args.overrides.line_width, Some(80));
        assert_eq!(args.overrides.dialect, Some(Dialect::Databricks));
        assert_eq!(args.overrides.uppercase_keywords, Some(false));
        assert_eq!(args.overrides.keyword_case, Some(KeywordCase::Lower));
        assert_eq!(args.overrides.line_ending, Some(LineEnding::Crlf));
        assert_eq!(args.paths, vec![PathBuf::from("a.sql")]);
    }

    #[test]
    fn parses_eq_style_options() {
        let args = run_args(&[
            "--line-width=70",
            "--indent-width=2",
            "--dialect=snowflake",
            "--keyword-case=preserve",
            "--line-ending=auto",
        ]);
        assert_eq!(args.overrides.line_width, Some(70));
        assert_eq!(args.overrides.indent_width, Some(2));
        assert_eq!(args.overrides.dialect, Some(Dialect::Snowflake));
        assert_eq!(args.overrides.keyword_case, Some(KeywordCase::Preserve));
        assert_eq!(args.overrides.line_ending, Some(LineEnding::Auto));
    }

    #[test]
    fn parses_stdin_filepath() {
        let args = run_args(&["--stdin-filepath", "src/query.sql"]);
        assert_eq!(args.stdin_filepath, Some(PathBuf::from("src/query.sql")));
        assert!(args.paths.is_empty());
    }

    #[test]
    fn parses_diff_with_check() {
        let args = run_args(&["--check", "--diff", "a.sql"]);
        assert!(args.check);
        assert!(args.diff);
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
    fn lone_dash_requests_stdin() {
        let args = run_args(&["-"]);
        assert_eq!(args.paths.len(), 1);
        assert!(is_stdin_path(&args.paths[0]));
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
    fn zero_width_args_error() {
        assert!(parse_args(["--line-width", "0"].map(Into::into)).is_err());
        assert!(parse_args(["--indent-width=0"].map(Into::into)).is_err());
    }

    #[test]
    fn diff_requires_check() {
        assert!(parse_args(["--diff", "a.sql"].map(Into::into)).is_err());
    }

    #[test]
    fn stdin_filepath_requires_stdin_input() {
        assert!(
            parse_args(["--stdin-filepath", "src/query.sql", "a.sql"].map(Into::into)).is_err()
        );
    }

    #[test]
    fn stdin_dash_cannot_be_mixed_with_paths() {
        assert!(parse_args(["-", "a.sql"].map(Into::into)).is_err());
    }

    #[test]
    fn invalid_dialect_arg_errors() {
        assert!(parse_args(["--dialect", "oracle"].map(Into::into)).is_err());
        assert!(parse_args(["--dialect"].map(Into::into)).is_err());
    }

    #[test]
    fn invalid_keyword_case_and_line_ending_args_error() {
        assert!(parse_args(["--keyword-case", "title"].map(Into::into)).is_err());
        assert!(parse_args(["--line-ending", "native"].map(Into::into)).is_err());
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
