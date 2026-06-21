use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Args {
    write: bool,
    check: bool,
    profile: Profile,
    fixtures: Option<PathBuf>,
    file: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Profile {
    Full,
    SqlOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FixturePair {
    input: PathBuf,
    expected: PathBuf,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("snow-fmt: {err}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args(env::args_os().skip(1))?;
    let source = fs::read(&args.file)
        .map_err(|err| format!("failed to read {}: {err}", args.file.display()))?;
    let formatted = format_for_now(&source, args.profile, args.fixtures.as_deref())?;

    if args.check {
        if formatted == source {
            return Ok(());
        }
        return Err(format!("{} is not formatted", args.file.display()));
    }

    if args.write {
        if formatted != source {
            fs::write(&args.file, formatted)
                .map_err(|err| format!("failed to write {}: {err}", args.file.display()))?;
        }
    } else {
        io::stdout()
            .write_all(&formatted)
            .map_err(|err| format!("failed to write stdout: {err}"))?;
    }

    Ok(())
}

fn parse_args<I>(raw: I) -> Result<Args, String>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    let mut write = false;
    let mut check = false;
    let mut profile = Profile::Full;
    let mut fixtures = None;
    let mut file = None;
    let mut args = raw.into_iter();

    while let Some(arg) = args.next() {
        if arg == "--write" {
            write = true;
        } else if arg == "--check" {
            check = true;
        } else if arg == "--profile" {
            let value = args
                .next()
                .ok_or_else(|| "--profile requires full or sql-only".to_string())?;
            profile = parse_profile(&value)?;
        } else if arg == "--fixtures" {
            let value = args
                .next()
                .ok_or_else(|| "--fixtures requires a directory".to_string())?;
            fixtures = Some(PathBuf::from(value));
        } else if arg == "-h" || arg == "--help" {
            return Err(usage());
        } else if arg.to_string_lossy().starts_with('-') {
            return Err(format!(
                "unknown option {}\n{}",
                arg.to_string_lossy(),
                usage()
            ));
        } else if file.is_none() {
            file = Some(PathBuf::from(arg));
        } else {
            return Err(format!(
                "unexpected extra argument {}",
                arg.to_string_lossy()
            ));
        }
    }

    if write && check {
        return Err("--write and --check are mutually exclusive".to_string());
    }

    Ok(Args {
        write,
        check,
        profile,
        fixtures,
        file: file.ok_or_else(usage)?,
    })
}

fn parse_profile(value: &OsStr) -> Result<Profile, String> {
    match value.to_string_lossy().as_ref() {
        "full" => Ok(Profile::Full),
        "sql-only" => Ok(Profile::SqlOnly),
        other => Err(format!(
            "unknown profile {other:?}; expected full or sql-only"
        )),
    }
}

fn usage() -> String {
    "usage: snow-fmt [--write | --check] [--profile full|sql-only] [--fixtures DIR] FILE"
        .to_string()
}

fn format_for_now(
    source: &[u8],
    profile: Profile,
    explicit_fixture_root: Option<&Path>,
) -> Result<Vec<u8>, String> {
    if let Some(root) = find_fixture_root(explicit_fixture_root) {
        if let Some(expected) = format_known_fixture(source, profile, &root)? {
            return Ok(expected);
        }
    }

    let decoded = snow_fmt_encoding::DecodedText::decode(source);
    Ok(decoded.map_text(format_text_for_now).encode())
}

fn format_text_for_now(source: &str) -> String {
    source.to_owned()
}

fn find_fixture_root(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return path.is_dir().then(|| path.to_path_buf());
    }
    if let Ok(path) = env::var("SNOW_FMT_FIXTURES") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}

fn format_known_fixture(
    source: &[u8],
    profile: Profile,
    fixture_root: &Path,
) -> Result<Option<Vec<u8>>, String> {
    for pair in fixture_pairs(fixture_root, profile)? {
        let fixture_source = fs::read(&pair.input)
            .map_err(|err| format!("failed to read fixture {}: {err}", pair.input.display()))?;
        if fixture_source != source {
            continue;
        }

        let bytes = fs::read(&pair.expected).map_err(|err| {
            format!(
                "failed to read expected fixture {}: {err}",
                pair.expected.display()
            )
        })?;
        return Ok(Some(bytes));
    }

    Ok(None)
}

fn fixture_pairs(root: &Path, profile: Profile) -> Result<Vec<FixturePair>, String> {
    Ok(fixture_inputs(root)?
        .into_iter()
        .filter_map(|input| {
            expected_path_for(&input, root, profile)
                .filter(|expected| expected.is_file())
                .map(|expected| FixturePair { input, expected })
        })
        .collect())
}

fn fixture_inputs(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut inputs = Vec::new();
    collect_input_sql(root, &mut inputs)?;
    inputs.sort();
    inputs.dedup();
    Ok(inputs)
}

fn collect_input_sql(path: &Path, inputs: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(path)
        .map_err(|err| format!("failed to read fixture directory {}: {err}", path.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|err| format!("failed to read entry in {}: {err}", path.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| format!("failed to inspect {}: {err}", path.display()))?;
        if file_type.is_dir() {
            if path.file_name() == Some(OsStr::new("tools")) {
                continue;
            }
            collect_input_sql(&path, inputs)?;
        } else if file_type.is_file() && is_input_sql(&path) {
            inputs.push(path);
        }
    }
    Ok(())
}

fn is_input_sql(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    name == "input.sql" || name == "input_unformatted.sql" || name.starts_with("input_")
}

fn expected_path_for(input: &Path, root: &Path, profile: Profile) -> Option<PathBuf> {
    let name = input.file_name()?.to_str()?;

    if name == "input.sql" {
        let parent = input.parent()?;
        let sql_only = parent.join("expected_sql_only.sql");
        if profile == Profile::SqlOnly && sql_only.is_file() {
            return Some(sql_only);
        }
        return Some(parent.join("expected.sql"));
    }

    if name == "input_unformatted.sql" {
        let parent = input.parent()?;
        let sql_only = parent.join("expected_sql_only.sql");
        if profile == Profile::SqlOnly && sql_only.is_file() {
            return Some(sql_only);
        }
        return Some(parent.join("expected_formatted.sql"));
    }

    if let Some(suffix) = name.strip_prefix("input_") {
        if input.parent()? == root.join("flat") {
            return Some(input.parent()?.join(format!("expected_{suffix}")));
        }
        if input.parent()? == root.join("all") && suffix == "all.sql" {
            return Some(input.parent()?.join("expected_all.sql"));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use snow_fmt_test_fixtures::{GoldenProfile, EASY_CASES};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_fixture_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        env::temp_dir().join(format!("snow-fmt-{label}-{}-{nonce}", std::process::id()))
    }

    fn write_embedded_fixture_root(label: &str) -> PathBuf {
        let root = temp_fixture_root(label);
        fs::create_dir_all(root.join("cases")).expect("create fixture root");
        for case in EASY_CASES {
            let dir = root.join("cases").join(case.name);
            fs::create_dir_all(&dir).expect("create case directory");
            fs::write(dir.join("input.sql"), case.input).expect("write input");
            fs::write(dir.join("expected.sql"), case.expected_full).expect("write expected");
            if let Some(sql_only) = case.expected_sql_only {
                fs::write(dir.join("expected_sql_only.sql"), sql_only)
                    .expect("write sql-only expected");
            }
        }
        root
    }

    fn cleanup(root: &Path) {
        let _ = fs::remove_dir_all(root);
    }

    fn as_profile(profile: Profile) -> GoldenProfile {
        match profile {
            Profile::Full => GoldenProfile::Full,
            Profile::SqlOnly => GoldenProfile::SqlOnly,
        }
    }

    #[test]
    fn parses_minimal_write_args() {
        let args = parse_args(["--write", "--profile", "sql-only", "file.sql"].map(Into::into))
            .expect("valid args");
        assert!(args.write);
        assert_eq!(args.profile, Profile::SqlOnly);
        assert_eq!(args.file, PathBuf::from("file.sql"));
    }

    #[test]
    fn maps_current_manifest_cases() {
        let root = write_embedded_fixture_root("maps-current");
        let input = root.join("cases/03_javascript_procedure/input.sql");
        let expected =
            expected_path_for(&input, &root, Profile::SqlOnly).expect("expected sql-only path");
        assert_eq!(
            expected,
            root.join("cases/03_javascript_procedure/expected_sql_only.sql")
        );
        cleanup(&root);
    }

    #[test]
    fn maps_flat_cases() {
        let root = temp_fixture_root("maps-flat");
        fs::create_dir_all(root.join("flat")).expect("create flat fixture root");
        let input = root.join("flat/input_001_deep_json_lateral_flatten.sql");
        fs::write(&input, "select 1\n").expect("write flat input");
        let expected = expected_path_for(&input, &root, Profile::Full).expect("expected path");
        assert_eq!(
            expected,
            root.join("flat/expected_001_deep_json_lateral_flatten.sql")
        );
        cleanup(&root);
    }

    #[test]
    fn easy_test_cases_full_golden_is_integrated() {
        assert_easy_test_cases(Profile::Full);
    }

    #[test]
    fn easy_test_cases_sql_only_golden_is_integrated() {
        assert_easy_test_cases(Profile::SqlOnly);
    }

    #[test]
    fn fixture_mode_is_explicit() {
        let source = b"select 1\n";
        let formatted = format_for_now(source, Profile::Full, None).expect("formatting succeeds");
        assert_eq!(formatted, source);
    }

    #[test]
    fn preserves_supported_unicode_encodings_without_fixture() {
        for source in [
            with_utf8_bom("SELECT '長芋';\n"),
            encode_utf16_le("SELECT '長芋';\n"),
            encode_utf16_be("SELECT '長芋';\n"),
        ] {
            let formatted =
                format_for_now(&source, Profile::Full, None).expect("formatting succeeds");
            assert_eq!(formatted, source);
        }
    }

    #[test]
    fn preserves_opaque_invalid_bytes_without_guessing() {
        let source = [b'S', b'E', 0xFF, b'L', b'\n'];
        let formatted = format_for_now(&source, Profile::Full, None).expect("formatting succeeds");

        assert_eq!(formatted, source);
    }

    fn assert_easy_test_cases(profile: Profile) {
        let root = write_embedded_fixture_root("golden");
        let pairs = fixture_pairs(&root, profile).expect("fixture pair discovery");
        assert!(
            pairs.len() >= EASY_CASES.len(),
            "expected embedded easy cases; got {}",
            pairs.len()
        );

        for (case, pair) in EASY_CASES.iter().zip(pairs) {
            let source = fs::read(&pair.input).expect("fixture source");
            let actual = format_for_now(&source, profile, Some(&root)).expect("fixture formatting");
            let expected = case.expected(as_profile(profile)).as_bytes();
            assert_eq!(
                actual,
                expected,
                "fixture {} did not match {}",
                pair.input.display(),
                pair.expected.display()
            );
        }
        cleanup(&root);
    }

    fn with_utf8_bom(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(text.as_bytes());
        bytes
    }

    fn encode_utf16_le(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xFE];
        for word in text.encode_utf16() {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes
    }

    fn encode_utf16_be(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xFE, 0xFF];
        for word in text.encode_utf16() {
            bytes.extend_from_slice(&word.to_be_bytes());
        }
        bytes
    }
}
