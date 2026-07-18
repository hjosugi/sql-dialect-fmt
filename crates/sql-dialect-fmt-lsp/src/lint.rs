//! Thin LSP adapter over the `sql-dialect-fmt-lint` rule engine.
//!
//! The rules, options, and suppression comments live in `sql_dialect_fmt_lint`; this module only
//! converts its byte-ranged [`sql_dialect_fmt_lint::LintDiagnostic`]s into `lsp_types::Diagnostic`s
//! using the negotiated position encoding, and re-exports the types LSP consumers already use.

use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Range};
use sql_dialect_fmt_lint::LintSeverity;
use sql_dialect_fmt_parser::Dialect;
use sql_dialect_fmt_text::LineIndex;

use crate::{lsp_position, PositionEncoding};

pub use sql_dialect_fmt_lint::{LintCode, LintOptions};

/// The [`LintCode`] carried by an LSP diagnostic, when it is one of ours.
pub fn diagnostic_lint_code(diagnostic: &Diagnostic) -> Option<LintCode> {
    match diagnostic.code.as_ref()? {
        NumberOrString::String(value) => LintCode::from_code(value),
        NumberOrString::Number(_) => None,
    }
}

pub(crate) fn diagnostics_with_encoding(
    text: &str,
    dialect: Dialect,
    index: &LineIndex<'_>,
    options: LintOptions,
    encoding: PositionEncoding,
) -> Vec<Diagnostic> {
    sql_dialect_fmt_lint::lint_with_dialect(text, dialect, options)
        .into_iter()
        .map(|finding| Diagnostic {
            range: Range::new(
                lsp_position(index, finding.range.start, encoding),
                lsp_position(index, finding.range.end, encoding),
            ),
            severity: Some(match finding.severity {
                LintSeverity::Warning => DiagnosticSeverity::WARNING,
                LintSeverity::Error => DiagnosticSeverity::ERROR,
            }),
            code: Some(NumberOrString::String(finding.code.as_str().to_string())),
            source: Some("sql-dialect-fmt".to_string()),
            message: finding.message,
            ..Default::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lint_findings_convert_to_lsp_diagnostics() {
        let text = "SELECT * FROM t;";
        let index = LineIndex::new(text);
        let diagnostics = diagnostics_with_encoding(
            text,
            Dialect::Snowflake,
            &index,
            LintOptions::default(),
            PositionEncoding::Utf16,
        );
        let wildcard = diagnostics
            .iter()
            .find(|diagnostic| diagnostic_lint_code(diagnostic) == Some(LintCode::SelectWildcard))
            .expect("SDF001 diagnostic");
        assert_eq!(wildcard.severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(wildcard.source.as_deref(), Some("sql-dialect-fmt"));
        let col = text.find('*').unwrap() as u32;
        assert_eq!(wildcard.range.start, lsp_types::Position::new(0, col));
        assert_eq!(wildcard.range.end, lsp_types::Position::new(0, col + 1));
    }

    #[test]
    fn suppression_comments_apply_before_conversion() {
        let text = "-- sql-dialect-fmt: disable-next-line SDF001\nSELECT * FROM t;";
        let index = LineIndex::new(text);
        assert!(diagnostics_with_encoding(
            text,
            Dialect::Snowflake,
            &index,
            LintOptions::default(),
            PositionEncoding::Utf16,
        )
        .is_empty());
    }
}
