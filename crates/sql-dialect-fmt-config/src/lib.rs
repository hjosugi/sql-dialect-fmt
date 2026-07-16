//! `sql-dialect-fmt.toml` configuration files.
//!
//! Shared by the CLI and the language server so both discover and apply project configuration the
//! same way. A config file maps onto the formatter's [`FormatOptions`] plus the CLI-only `exclude`
//! file-discovery knob. Every key is optional, so a file may set only the knobs it cares about.
//! Discovery walks up the directory tree from a start point (an input file's parent, or the current
//! working directory) and uses the **nearest** `sql-dialect-fmt.toml` — the first one found on the
//! way up — mirroring how `rustfmt`, `prettier`, and friends scope project configuration. Explicit
//! overrides (CLI flags, or LSP editor settings) always win over formatter options from the file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};
use sql_dialect_fmt_formatter::{FormatOptions, KeywordCase, LineEnding};
use sql_dialect_fmt_parser::Dialect;

/// The file name the CLI looks for when walking up directories.
pub const CONFIG_FILE_NAME: &str = "sql-dialect-fmt.toml";

/// A parsed `sql-dialect-fmt.toml`. Every field is optional; absent fields fall back to the formatter
/// defaults (or to a CLI flag, which is layered on top afterwards).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Config {
    /// Target line width the printer keeps within where it can.
    pub line_width: Option<usize>,
    /// Spaces per indentation level.
    pub indent_width: Option<usize>,
    /// Upper-case SQL keywords.
    pub uppercase_keywords: Option<bool>,
    /// Keyword casing policy.
    #[serde(default, deserialize_with = "deserialize_keyword_case")]
    pub keyword_case: Option<KeywordCase>,
    /// Output line-ending policy.
    #[serde(default, deserialize_with = "deserialize_line_ending")]
    pub line_ending: Option<LineEnding>,
    /// SQL dialect to parse and format.
    #[serde(default, deserialize_with = "deserialize_dialect")]
    pub dialect: Option<Dialect>,
    /// Glob patterns skipped during recursive directory discovery.
    #[serde(default)]
    pub exclude: Vec<String>,
}

pub fn parse_dialect(value: &str) -> Result<Dialect, String> {
    match value.to_ascii_lowercase().as_str() {
        "snowflake" => Ok(Dialect::Snowflake),
        "databricks" => Ok(Dialect::Databricks),
        _ => Err(format!(
            "dialect expects one of: snowflake, databricks; got {value:?}"
        )),
    }
}

pub fn parse_keyword_case(value: &str) -> Result<KeywordCase, String> {
    match value.to_ascii_lowercase().as_str() {
        "upper" => Ok(KeywordCase::Upper),
        "lower" => Ok(KeywordCase::Lower),
        "preserve" => Ok(KeywordCase::Preserve),
        _ => Err(format!(
            "keyword_case expects one of: upper, lower, preserve; got {value:?}"
        )),
    }
}

pub fn parse_line_ending(value: &str) -> Result<LineEnding, String> {
    match value.to_ascii_lowercase().as_str() {
        "auto" => Ok(LineEnding::Auto),
        "lf" => Ok(LineEnding::Lf),
        "crlf" => Ok(LineEnding::Crlf),
        _ => Err(format!(
            "line_ending expects one of: auto, lf, crlf; got {value:?}"
        )),
    }
}

fn deserialize_dialect<'de, D>(deserializer: D) -> Result<Option<Dialect>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(|value| parse_dialect(&value).map_err(serde::de::Error::custom))
        .transpose()
}

fn deserialize_keyword_case<'de, D>(deserializer: D) -> Result<Option<KeywordCase>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(|value| parse_keyword_case(&value).map_err(serde::de::Error::custom))
        .transpose()
}

fn deserialize_line_ending<'de, D>(deserializer: D) -> Result<Option<LineEnding>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(|value| parse_line_ending(&value).map_err(serde::de::Error::custom))
        .transpose()
}

impl Config {
    /// Parse a config from TOML source text.
    pub fn parse(text: &str) -> Result<Config, String> {
        toml::from_str(text).map_err(|err| {
            // `toml`'s message already carries line/column; strip the trailing newline it adds.
            err.message().trim_end().to_string()
        })
    }

    /// Read and parse a config file, attributing any error to `path`.
    pub fn load(path: &Path) -> Result<Config, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        Config::parse(&text).map_err(|err| format!("invalid config {}: {err}", path.display()))
    }

    /// Layer this config onto `options`, overwriting only the fields the file actually set.
    pub fn apply_to(&self, options: &mut FormatOptions) {
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

/// Find the nearest `sql-dialect-fmt.toml` at or above `start`, returning its path (not its contents).
///
/// `start` may be a file or a directory; if it is a file we begin the walk at its parent. Returns
/// `None` when no config exists anywhere up to the filesystem root. Never panics on odd paths.
pub fn discover(start: &Path) -> Option<PathBuf> {
    let mut dir: PathBuf = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent().map(Path::to_path_buf).unwrap_or_default()
    };

    // An empty directory means "current directory"; normalize so the walk-up terminates.
    if dir.as_os_str().is_empty() {
        dir = PathBuf::from(".");
    }

    loop {
        let candidate = dir.join(CONFIG_FILE_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            // Reached a relative-path origin like "."; try the absolute CWD chain once more.
            return discover_from_cwd_if_relative(start);
        }
    }
}

/// When `start` was a relative path we may have walked up only to ".". Resolve the real working
/// directory and continue the walk so a config in an ancestor of the CWD is still found.
fn discover_from_cwd_if_relative(start: &Path) -> Option<PathBuf> {
    if start.is_absolute() {
        return None;
    }
    let cwd = std::env::current_dir().ok()?;
    let mut dir = if start.is_dir() {
        cwd.join(start)
    } else {
        match start.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => cwd.join(parent),
            _ => cwd,
        }
    };
    loop {
        let candidate = dir.join(CONFIG_FILE_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_keys() {
        let cfg = Config::parse(
            "line_width = 80\nindent_width = 2\nuppercase_keywords = false\nkeyword_case = \"lower\"\nline_ending = \"crlf\"\ndialect = \"databricks\"\nexclude = [\"target/**\"]\n",
        )
        .expect("valid");
        assert_eq!(cfg.line_width, Some(80));
        assert_eq!(cfg.indent_width, Some(2));
        assert_eq!(cfg.uppercase_keywords, Some(false));
        assert_eq!(cfg.keyword_case, Some(KeywordCase::Lower));
        assert_eq!(cfg.line_ending, Some(LineEnding::Crlf));
        assert_eq!(cfg.dialect, Some(Dialect::Databricks));
        assert_eq!(cfg.exclude, vec!["target/**"]);
    }

    #[test]
    fn empty_config_is_all_none() {
        assert_eq!(Config::parse("").expect("valid"), Config::default());
    }

    #[test]
    fn partial_config_leaves_other_fields_default() {
        let cfg = Config::parse("indent_width = 8\n").expect("valid");
        assert_eq!(cfg.indent_width, Some(8));
        assert_eq!(cfg.line_width, None);
        assert_eq!(cfg.uppercase_keywords, None);
        assert_eq!(cfg.keyword_case, None);
        assert_eq!(cfg.line_ending, None);
        assert_eq!(cfg.dialect, None);
        assert!(cfg.exclude.is_empty());
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(Config::parse("tab_width = 4\n").is_err());
    }

    #[test]
    fn malformed_toml_is_rejected() {
        assert!(Config::parse("line_width = \n").is_err());
    }

    #[test]
    fn apply_overrides_only_set_fields() {
        let mut options = FormatOptions::default();
        let cfg = Config::parse(
            "line_width = 60\ndialect = \"databricks\"\nkeyword_case = \"lower\"\nline_ending = \"crlf\"\n",
        )
        .expect("valid");
        cfg.apply_to(&mut options);
        assert_eq!(options.line_width, 60);
        assert_eq!(options.dialect, Dialect::Databricks);
        assert_eq!(options.keyword_case, KeywordCase::Lower);
        assert_eq!(options.line_ending, LineEnding::Crlf);
        // Untouched fields keep their defaults.
        assert_eq!(options.indent_width, 4);
    }

    #[test]
    fn invalid_dialect_is_rejected() {
        assert!(Config::parse("dialect = \"oracle\"\n").is_err());
        assert!(parse_dialect("oracle").is_err());
    }

    #[test]
    fn invalid_keyword_case_and_line_ending_are_rejected() {
        assert!(Config::parse("keyword_case = \"title\"\n").is_err());
        assert!(Config::parse("line_ending = \"native\"\n").is_err());
        assert!(parse_keyword_case("title").is_err());
        assert!(parse_line_ending("native").is_err());
    }
}
