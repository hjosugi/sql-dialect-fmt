//! End-to-end tests driving the real `snow-fmt` binary.
//!
//! These cover the productionized surface: multi-file and directory inputs, `snow-fmt.toml`
//! discovery and CLI override precedence, `--check` exit codes, stdin↔stdout, and the error UX
//! (parse errors surfaced to stderr, distinct exit codes, no crashes on bad input).

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use tempfile::TempDir;

/// Run the binary with `args`, optional `stdin`, in working directory `cwd`. Returns
/// (exit code, stdout, stderr).
fn run(cwd: &Path, args: &[&str], stdin: Option<&str>) -> (i32, String, String) {
    let bin = env!("CARGO_BIN_EXE_snow-fmt");
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn snow-fmt");
    {
        let mut child_stdin = child.stdin.take().expect("stdin");
        if let Some(input) = stdin {
            child_stdin
                .write_all(input.as_bytes())
                .expect("write stdin");
        }
        // Dropping closes stdin so the child sees EOF.
    }
    let output = child.wait_with_output().expect("wait");
    (
        output.status.code().expect("exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn write(dir: &Path, name: &str, contents: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir");
    }
    fs::write(&path, contents).expect("write file");
    path
}

#[test]
fn stdin_to_stdout_formats() {
    let tmp = TempDir::new().unwrap();
    let (code, out, _err) = run(tmp.path(), &[], Some("select a,b from t"));
    assert_eq!(code, 0);
    assert_eq!(out, "SELECT a, b\nFROM t;\n");
}

#[test]
fn version_and_help_succeed() {
    let tmp = TempDir::new().unwrap();
    let (code_v, out_v, _) = run(tmp.path(), &["--version"], None);
    assert_eq!(code_v, 0);
    assert!(out_v.contains("snow-fmt"));

    let (code_h, out_h, _) = run(tmp.path(), &["--help"], None);
    assert_eq!(code_h, 0);
    assert!(out_h.contains("USAGE"));
}

#[test]
fn multiple_files_stream_to_stdout() {
    let tmp = TempDir::new().unwrap();
    let a = write(tmp.path(), "a.sql", "select 1");
    let b = write(tmp.path(), "b.sql", "select 2");
    let (code, out, _err) = run(
        tmp.path(),
        &[a.to_str().unwrap(), b.to_str().unwrap()],
        None,
    );
    assert_eq!(code, 0);
    assert_eq!(out, "SELECT 1;\nSELECT 2;\n");
}

#[test]
fn directory_is_recursed_for_sql_files() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "top.sql", "select 1");
    write(tmp.path(), "nested/deep.sql", "select 2");
    write(tmp.path(), "ignore.txt", "select 3"); // not *.sql, skipped
    let (code, _out, err) = run(tmp.path(), &["--write", "."], None);
    assert_eq!(code, 0);
    // Two SQL files formatted; the .txt left alone.
    assert_eq!(
        fs::read_to_string(tmp.path().join("top.sql")).unwrap(),
        "SELECT 1;\n"
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("nested/deep.sql")).unwrap(),
        "SELECT 2;\n"
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("ignore.txt")).unwrap(),
        "select 3"
    );
    assert!(err.contains("2 file"), "summary missing: {err}");
}

#[test]
fn write_in_place_changes_files() {
    let tmp = TempDir::new().unwrap();
    let f = write(tmp.path(), "q.sql", "select a,b");
    let (code, out, _err) = run(tmp.path(), &["--write", f.to_str().unwrap()], None);
    assert_eq!(code, 0);
    assert!(out.is_empty(), "write mode should not print to stdout");
    assert_eq!(fs::read_to_string(&f).unwrap(), "SELECT a, b;\n");
}

#[test]
fn check_passes_for_formatted_file() {
    let tmp = TempDir::new().unwrap();
    let f = write(tmp.path(), "ok.sql", "SELECT 1;\n");
    let (code, _out, _err) = run(tmp.path(), &["--check", f.to_str().unwrap()], None);
    assert_eq!(code, 0);
}

#[test]
fn check_fails_for_unformatted_file() {
    let tmp = TempDir::new().unwrap();
    let f = write(tmp.path(), "bad.sql", "select 1");
    let (code, _out, err) = run(tmp.path(), &["--check", f.to_str().unwrap()], None);
    assert_eq!(code, 1, "exit code should signal would-reformat");
    assert!(err.contains("is not formatted"), "stderr: {err}");
    // --check must not modify the file.
    assert_eq!(fs::read_to_string(&f).unwrap(), "select 1");
}

#[test]
fn check_mixed_files_fails_and_names_offender() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "good.sql", "SELECT 1;\n");
    write(tmp.path(), "bad.sql", "select 2");
    let (code, _out, err) = run(tmp.path(), &["--check", "."], None);
    assert_eq!(code, 1);
    assert!(err.contains("bad.sql"), "should name the offender: {err}");
    assert!(!err.contains("good.sql is not formatted"));
}

#[test]
fn config_file_is_discovered_and_applied() {
    let tmp = TempDir::new().unwrap();
    // 80-wide config; with default 100 this stays on one line.
    write(
        tmp.path(),
        "snow-fmt.toml",
        "indent_width = 2\nuppercase_keywords = false\n",
    );
    let f = write(
        tmp.path(),
        "q.sql",
        "select case when a then 1 else 2 end from t",
    );
    let (code, out, _err) = run(tmp.path(), &[f.to_str().unwrap()], None);
    assert_eq!(code, 0);
    // uppercase_keywords=false from config => keywords stay lowercase.
    assert!(out.starts_with("select"), "config not applied: {out:?}");
    assert!(
        !out.contains("SELECT"),
        "keywords should stay lowercase: {out:?}"
    );
}

#[test]
fn config_is_discovered_walking_up_from_nested_file() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "snow-fmt.toml", "uppercase_keywords = false\n");
    let nested = write(tmp.path(), "a/b/c/q.sql", "select 1");
    let (code, out, _err) = run(tmp.path(), &[nested.to_str().unwrap()], None);
    assert_eq!(code, 0);
    assert_eq!(out, "select 1;\n");
}

#[test]
fn cli_flag_overrides_config() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "snow-fmt.toml", "uppercase_keywords = false\n");
    let f = write(tmp.path(), "q.sql", "select 1");
    // CLI --uppercase wins over the config's uppercase_keywords=false.
    let (code, out, _err) = run(tmp.path(), &["--uppercase", f.to_str().unwrap()], None);
    assert_eq!(code, 0);
    assert_eq!(out, "SELECT 1;\n");
}

#[test]
fn no_config_ignores_config_file() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "snow-fmt.toml", "uppercase_keywords = false\n");
    let f = write(tmp.path(), "q.sql", "select 1");
    let (code, out, _err) = run(tmp.path(), &["--no-config", f.to_str().unwrap()], None);
    assert_eq!(code, 0);
    // Defaults restored: keywords uppercased.
    assert_eq!(out, "SELECT 1;\n");
}

#[test]
fn invalid_config_is_reported_with_exit_2() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "snow-fmt.toml", "line_width = \"wide\"\n"); // wrong type
    let f = write(tmp.path(), "q.sql", "select 1");
    let (code, _out, err) = run(tmp.path(), &[f.to_str().unwrap()], None);
    assert_eq!(code, 2);
    assert!(err.contains("invalid config"), "stderr: {err}");
}

#[test]
fn parse_errors_surface_to_stderr() {
    let tmp = TempDir::new().unwrap();
    // A clearly malformed statement that the parser flags as an error.
    let (code, out, err) = run(tmp.path(), &[], Some("select from where 1"));
    // Output is still produced losslessly; the error is reported.
    assert!(!out.is_empty());
    assert_eq!(code, 0, "stdout mode succeeds even with parse errors");
    assert!(
        err.contains("parse error"),
        "expected parse error on stderr: {err}"
    );
    assert!(err.contains("<stdin>"), "should name the source: {err}");
}

#[test]
fn parse_error_in_file_names_the_file() {
    let tmp = TempDir::new().unwrap();
    let f = write(tmp.path(), "broken.sql", "select from where 1");
    let (_code, _out, err) = run(tmp.path(), &[f.to_str().unwrap()], None);
    assert!(err.contains("broken.sql"), "should name the file: {err}");
    assert!(err.contains("parse error"), "stderr: {err}");
}

#[test]
fn missing_path_is_an_io_error_exit_2() {
    let tmp = TempDir::new().unwrap();
    let (code, _out, err) = run(tmp.path(), &["does-not-exist.sql"], None);
    assert_eq!(code, 2);
    assert!(err.to_lowercase().contains("no such file"), "stderr: {err}");
}

#[test]
fn conflicting_modes_is_usage_error_exit_2() {
    let tmp = TempDir::new().unwrap();
    let (code, _out, err) = run(tmp.path(), &["--write", "--check"], Some("select 1"));
    assert_eq!(code, 2);
    assert!(err.contains("mutually exclusive"), "stderr: {err}");
}

#[test]
fn unknown_option_is_usage_error_exit_2() {
    let tmp = TempDir::new().unwrap();
    let (code, _out, err) = run(tmp.path(), &["--frobnicate"], None);
    assert_eq!(code, 2);
    assert!(err.contains("unknown option"), "stderr: {err}");
}

#[test]
fn opaque_bytes_pass_through_without_crash() {
    let tmp = TempDir::new().unwrap();
    // Invalid UTF-8 in a file: must round-trip untouched, no crash, no spurious parse error.
    let f = tmp.path().join("binary.sql");
    fs::write(&f, [b'S', b'E', 0xFF, b'L']).unwrap();
    let (code, _out, err) = run(tmp.path(), &["--check", f.to_str().unwrap()], None);
    // Opaque bytes are "unchanged", so --check passes and nothing is reported.
    assert_eq!(code, 0);
    assert!(!err.contains("parse error"), "stderr: {err}");
}

#[test]
fn already_formatted_directory_check_succeeds() {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), "a.sql", "SELECT 1;\n");
    write(tmp.path(), "b.sql", "SELECT 2;\n");
    let (code, _out, err) = run(tmp.path(), &["--check", "."], None);
    assert_eq!(code, 0);
    assert!(err.contains("already formatted"), "summary: {err}");
}
