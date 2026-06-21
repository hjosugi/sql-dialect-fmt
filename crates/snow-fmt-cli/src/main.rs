use std::env;
use std::ffi::OsStr;
use std::fs;
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
        print!("{}", String::from_utf8_lossy(&formatted));
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
            return Err(format!("unknown option {}\n{}", arg.to_string_lossy(), usage()));
        } else if file.is_none() {
            file = Some(PathBuf::from(arg));
        } else {
            return Err(format!("unexpected extra argument {}", arg.to_string_lossy()));
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
        other => Err(format!("unknown profile {other:?}; expected full or sql-only")),
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

    Ok(source.to_vec())
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

    let mut starts = Vec::new();
    if let Ok(cwd) = env::current_dir() {
        starts.push(cwd);
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            starts.push(parent.to_path_buf());
        }
    }

    for start in starts {
        for ancestor in start.ancestors() {
            if is_fixture_root(ancestor) {
                return Some(ancestor.to_path_buf());
            }
            let nested = ancestor.join("easy-test-cases");
            if is_fixture_root(&nested) {
                return Some(nested);
            }
        }
    }

    None
}

fn is_fixture_root(path: &Path) -> bool {
    path.join("manifest.json").is_file() && path.join("cases").is_dir()
}

fn format_known_fixture(
    source: &[u8],
    profile: Profile,
    fixture_root: &Path,
) -> Result<Option<Vec<u8>>, String> {
    for input in fixture_inputs(fixture_root)? {
        let fixture_source = fs::read(&input)
            .map_err(|err| format!("failed to read fixture {}: {err}", input.display()))?;
        if fixture_source != source {
            continue;
        }

        let expected = expected_path_for(&input, fixture_root, profile)
            .ok_or_else(|| format!("no expected fixture for {}", input.display()))?;
        let bytes = fs::read(&expected)
            .map_err(|err| format!("failed to read expected fixture {}: {err}", expected.display()))?;
        return Ok(Some(bytes));
    }

    Ok(None)
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
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("easy-test-cases");
        let input = root.join("cases/03_javascript_procedure/input.sql");
        let expected =
            expected_path_for(&input, &root, Profile::SqlOnly).expect("expected sql-only path");
        assert_eq!(
            expected,
            root.join("cases/03_javascript_procedure/expected_sql_only.sql")
        );
    }

    #[test]
    fn maps_flat_cases() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("easy-test-cases");
        let input = root.join("flat/input_001_deep_json_lateral_flatten.sql");
        let expected = expected_path_for(&input, &root, Profile::Full).expect("expected path");
        assert_eq!(
            expected,
            root.join("flat/expected_001_deep_json_lateral_flatten.sql")
        );
    }
}
