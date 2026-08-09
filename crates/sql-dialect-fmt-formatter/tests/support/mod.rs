use sql_dialect_fmt_formatter::{FormatOptions, LineEnding, SelectItemLayout};

/// Stable layout profile for the long-running structural golden matrices.
///
/// Product defaults have their own focused contract tests. Keeping historical goldens on an
/// explicit adaptive profile lets those matrices continue to pinpoint formatter-structure changes
/// instead of rewriting hundreds of unrelated expectations whenever the product default changes.
pub fn adaptive_options() -> FormatOptions {
    FormatOptions::default()
        .with_line_width(100)
        .with_indent_width(4)
        .with_line_ending(LineEnding::Lf)
        .with_select_item_layout(SelectItemLayout::Auto)
}
