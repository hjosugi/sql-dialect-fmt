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
use sql_dialect_fmt_formatter::{
    CommaStyle, FormatOptions, KeywordCase, LineEnding, LogicalOperatorNewline, SelectItemLayout,
};
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
    /// Spaces per indentation level. `tab_width` is accepted as an alias (sql-formatter spelling).
    #[serde(alias = "tab_width")]
    pub indent_width: Option<usize>,
    /// Upper-case SQL keywords.
    pub uppercase_keywords: Option<bool>,
    /// Keyword casing policy.
    #[serde(default, deserialize_with = "deserialize_keyword_case")]
    pub keyword_case: Option<KeywordCase>,
    /// Casing for built-in data-type words in type positions.
    #[serde(default, deserialize_with = "deserialize_data_type_case")]
    pub data_type_case: Option<KeywordCase>,
    /// Casing for function names.
    #[serde(default, deserialize_with = "deserialize_function_case")]
    pub function_case: Option<KeywordCase>,
    /// Casing for unquoted identifiers.
    #[serde(default, deserialize_with = "deserialize_identifier_case")]
    pub identifier_case: Option<KeywordCase>,
    /// Where `AND`/`OR` sit when a boolean expression wraps: `before` or `after`.
    #[serde(default, deserialize_with = "deserialize_logical_operator_newline")]
    pub logical_operator_newline: Option<LogicalOperatorNewline>,
    /// Pack binary operators without surrounding spaces.
    pub dense_operators: Option<bool>,
    /// Indent with tab characters instead of spaces.
    pub use_tabs: Option<bool>,
    /// Place the statement-terminating `;` on its own line.
    pub newline_before_semicolon: Option<bool>,
    /// Number of blank lines to force between top-level statements.
    pub lines_between_queries: Option<usize>,
    /// Flat width cap for parenthesized lists before they wrap.
    pub expression_width: Option<usize>,
    /// Output line-ending policy.
    #[serde(default, deserialize_with = "deserialize_line_ending")]
    pub line_ending: Option<LineEnding>,
    /// Top-level `SELECT` list layout.
    #[serde(default, deserialize_with = "deserialize_select_item_layout")]
    pub select_item_layout: Option<SelectItemLayout>,
    /// Placement of commas in wrapped lists.
    #[serde(default, deserialize_with = "deserialize_comma_style")]
    pub comma_style: Option<CommaStyle>,
    /// SQL dialect to parse and format.
    #[serde(default, deserialize_with = "deserialize_dialect")]
    pub dialect: Option<Dialect>,
    /// Placeholder values applied after formatting (`--param` overrides/extend these).
    #[serde(default)]
    pub params: Vec<String>,
    /// Glob patterns skipped during recursive directory discovery.
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// Parse a dialect name (canonical or alias) into a [`Dialect`].
///
/// Accepts every dialect [`Dialect::ALL`] exposes plus common aliases (`oracle`, `tsql`,
/// `postgres`, `presto`, …). See [`Dialect::from_name`].
pub fn parse_dialect(value: &str) -> Result<Dialect, String> {
    Dialect::from_name(value).ok_or_else(|| {
        let names: Vec<&str> = Dialect::ALL.iter().map(|d| d.canonical_name()).collect();
        format!(
            "dialect expects one of: {}; got {value:?}",
            names.join(", ")
        )
    })
}

pub fn keyword_case_from_str(value: &str, field: &str) -> Result<KeywordCase, String> {
    match value.to_ascii_lowercase().as_str() {
        "upper" => Ok(KeywordCase::Upper),
        "lower" => Ok(KeywordCase::Lower),
        "preserve" => Ok(KeywordCase::Preserve),
        _ => Err(format!(
            "{field} expects one of: upper, lower, preserve; got {value:?}"
        )),
    }
}

pub fn parse_keyword_case(value: &str) -> Result<KeywordCase, String> {
    keyword_case_from_str(value, "keyword_case")
}

pub fn parse_data_type_case(value: &str) -> Result<KeywordCase, String> {
    keyword_case_from_str(value, "data_type_case")
}

pub fn parse_function_case(value: &str) -> Result<KeywordCase, String> {
    keyword_case_from_str(value, "function_case")
}

pub fn parse_identifier_case(value: &str) -> Result<KeywordCase, String> {
    keyword_case_from_str(value, "identifier_case")
}

pub fn parse_logical_operator_newline(value: &str) -> Result<LogicalOperatorNewline, String> {
    match value.to_ascii_lowercase().as_str() {
        "before" => Ok(LogicalOperatorNewline::Before),
        "after" => Ok(LogicalOperatorNewline::After),
        _ => Err(format!(
            "logical_operator_newline expects one of: before, after; got {value:?}"
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

pub fn parse_select_item_layout(value: &str) -> Result<SelectItemLayout, String> {
    match value.to_ascii_lowercase().as_str() {
        "auto" => Ok(SelectItemLayout::Auto),
        "vertical" => Ok(SelectItemLayout::Vertical),
        _ => Err(format!(
            "select_item_layout expects one of: auto, vertical; got {value:?}"
        )),
    }
}

pub fn parse_comma_style(value: &str) -> Result<CommaStyle, String> {
    match value.to_ascii_lowercase().as_str() {
        "trailing" => Ok(CommaStyle::Trailing),
        "leading" => Ok(CommaStyle::Leading),
        _ => Err(format!(
            "comma_style expects one of: trailing, leading; got {value:?}"
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

fn deserialize_data_type_case<'de, D>(deserializer: D) -> Result<Option<KeywordCase>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(|value| parse_data_type_case(&value).map_err(serde::de::Error::custom))
        .transpose()
}

fn deserialize_function_case<'de, D>(deserializer: D) -> Result<Option<KeywordCase>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(|value| parse_function_case(&value).map_err(serde::de::Error::custom))
        .transpose()
}

fn deserialize_identifier_case<'de, D>(deserializer: D) -> Result<Option<KeywordCase>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(|value| parse_identifier_case(&value).map_err(serde::de::Error::custom))
        .transpose()
}

fn deserialize_logical_operator_newline<'de, D>(
    deserializer: D,
) -> Result<Option<LogicalOperatorNewline>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(|value| parse_logical_operator_newline(&value).map_err(serde::de::Error::custom))
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

fn deserialize_select_item_layout<'de, D>(
    deserializer: D,
) -> Result<Option<SelectItemLayout>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(|value| parse_select_item_layout(&value).map_err(serde::de::Error::custom))
        .transpose()
}

fn deserialize_comma_style<'de, D>(deserializer: D) -> Result<Option<CommaStyle>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(|value| parse_comma_style(&value).map_err(serde::de::Error::custom))
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
        if let Some(data_type_case) = self.data_type_case {
            *options = (*options).with_data_type_case(data_type_case);
        }
        if let Some(function_case) = self.function_case {
            *options = (*options).with_function_case(function_case);
        }
        if let Some(identifier_case) = self.identifier_case {
            *options = (*options).with_identifier_case(identifier_case);
        }
        if let Some(logical_operator_newline) = self.logical_operator_newline {
            *options = (*options).with_logical_operator_newline(logical_operator_newline);
        }
        if let Some(dense_operators) = self.dense_operators {
            *options = (*options).with_dense_operators(dense_operators);
        }
        if let Some(use_tabs) = self.use_tabs {
            *options = (*options).with_use_tabs(use_tabs);
        }
        if let Some(newline_before_semicolon) = self.newline_before_semicolon {
            *options = (*options).with_newline_before_semicolon(newline_before_semicolon);
        }
        if let Some(lines_between_queries) = self.lines_between_queries {
            *options = (*options).with_lines_between_queries(Some(lines_between_queries));
        }
        if let Some(expression_width) = self.expression_width {
            *options = (*options).with_expression_width(Some(expression_width));
        }
        if let Some(line_ending) = self.line_ending {
            options.line_ending = line_ending;
        }
        if let Some(select_item_layout) = self.select_item_layout {
            options.select_item_layout = select_item_layout;
        }
        if let Some(comma_style) = self.comma_style {
            options.comma_style = comma_style;
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
            "line_width = 80\nindent_width = 2\nuppercase_keywords = false\nkeyword_case = \"lower\"\ndata_type_case = \"upper\"\nfunction_case = \"lower\"\nidentifier_case = \"lower\"\nlogical_operator_newline = \"after\"\ndense_operators = true\nuse_tabs = true\nnewline_before_semicolon = true\nlines_between_queries = 2\nexpression_width = 40\nline_ending = \"crlf\"\nselect_item_layout = \"vertical\"\ncomma_style = \"leading\"\ndialect = \"databricks\"\nparams = [\"'bar'\"]\nexclude = [\"target/**\"]\n",
        )
        .expect("valid");
        assert_eq!(cfg.line_width, Some(80));
        assert_eq!(cfg.indent_width, Some(2));
        assert_eq!(cfg.uppercase_keywords, Some(false));
        assert_eq!(cfg.keyword_case, Some(KeywordCase::Lower));
        assert_eq!(cfg.data_type_case, Some(KeywordCase::Upper));
        assert_eq!(cfg.function_case, Some(KeywordCase::Lower));
        assert_eq!(cfg.identifier_case, Some(KeywordCase::Lower));
        assert_eq!(
            cfg.logical_operator_newline,
            Some(LogicalOperatorNewline::After)
        );
        assert_eq!(cfg.dense_operators, Some(true));
        assert_eq!(cfg.use_tabs, Some(true));
        assert_eq!(cfg.newline_before_semicolon, Some(true));
        assert_eq!(cfg.lines_between_queries, Some(2));
        assert_eq!(cfg.expression_width, Some(40));
        assert_eq!(cfg.line_ending, Some(LineEnding::Crlf));
        assert_eq!(cfg.select_item_layout, Some(SelectItemLayout::Vertical));
        assert_eq!(cfg.comma_style, Some(CommaStyle::Leading));
        assert_eq!(cfg.dialect, Some(Dialect::Databricks));
        assert_eq!(cfg.params, vec!["'bar'"]);
        assert_eq!(cfg.exclude, vec!["target/**"]);
    }

    #[test]
    fn tab_width_is_an_alias_for_indent_width() {
        let cfg = Config::parse("tab_width = 4\n").expect("valid");
        assert_eq!(cfg.indent_width, Some(4));
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
        assert_eq!(cfg.data_type_case, None);
        assert_eq!(cfg.function_case, None);
        assert_eq!(cfg.identifier_case, None);
        assert_eq!(cfg.logical_operator_newline, None);
        assert_eq!(cfg.dense_operators, None);
        assert_eq!(cfg.use_tabs, None);
        assert_eq!(cfg.newline_before_semicolon, None);
        assert_eq!(cfg.lines_between_queries, None);
        assert_eq!(cfg.expression_width, None);
        assert_eq!(cfg.line_ending, None);
        assert_eq!(cfg.select_item_layout, None);
        assert_eq!(cfg.comma_style, None);
        assert_eq!(cfg.dialect, None);
        assert!(cfg.params.is_empty());
        assert!(cfg.exclude.is_empty());
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(Config::parse("nonsense_key = 4\n").is_err());
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
        assert_eq!(options.indent_width, 2);
    }

    #[test]
    fn every_dialect_name_parses() {
        for dialect in Dialect::ALL {
            assert_eq!(
                parse_dialect(dialect.canonical_name()).as_ref(),
                Ok(dialect),
                "{}",
                dialect.canonical_name()
            );
        }
        // Aliases people actually type.
        assert_eq!(parse_dialect("oracle").as_ref(), Ok(&Dialect::PlSql));
        assert_eq!(parse_dialect("tsql").as_ref(), Ok(&Dialect::TransactSql));
    }

    #[test]
    fn invalid_dialect_is_rejected() {
        assert!(Config::parse("dialect = \"nonsense\"\n").is_err());
        assert!(parse_dialect("nonsense").is_err());
    }

    #[test]
    fn invalid_keyword_case_and_line_ending_are_rejected() {
        assert!(Config::parse("keyword_case = \"title\"\n").is_err());
        assert!(Config::parse("line_ending = \"native\"\n").is_err());
        assert!(parse_keyword_case("title").is_err());
        assert!(parse_line_ending("native").is_err());
        assert!(Config::parse("logical_operator_newline = \"middle\"\n").is_err());
        assert!(parse_logical_operator_newline("middle").is_err());
    }
}
