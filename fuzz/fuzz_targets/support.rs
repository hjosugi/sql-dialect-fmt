use sql_dialect_fmt_syntax::Dialect;

const MAX_INPUT_BYTES: usize = 64 * 1024;

pub fn sql_input(data: &[u8]) -> Option<(Dialect, &str)> {
    let (selector, body) = match data.split_first() {
        Some((first, rest)) => (*first, rest),
        None => (0, data),
    };
    if body.len() > MAX_INPUT_BYTES {
        return None;
    }
    let dialect = if selector & 1 == 0 {
        Dialect::Snowflake
    } else {
        Dialect::Databricks
    };
    let source = std::str::from_utf8(body).ok()?;
    Some((dialect, source))
}

#[allow(dead_code)]
pub fn assert_span_in_source(offset: usize, len: usize, source_len: usize) {
    let end = offset
        .checked_add(len)
        .expect("diagnostic span overflowed usize");
    assert!(
        end <= source_len,
        "diagnostic span {offset}..{end} exceeded input length {source_len}"
    );
}
