//! Raw WebAssembly bindings for browser extension use.
//!
//! This crate deliberately avoids `wasm-bindgen` so the Chrome extension can load a single local
//! `.wasm` file without generated JavaScript glue. JavaScript owns input buffers allocated through
//! `sql_dialect_fmt_alloc`; this module owns the last formatted result until the next call or
//! `sql_dialect_fmt_clear_result`.

use std::{cell::RefCell, mem, ptr, slice, str};

use sql_dialect_fmt_formatter::{format, Dialect, FormatOptions};

thread_local! {
    static LAST_RESULT: RefCell<Option<Box<[u8]>>> = const { RefCell::new(None) };
}

/// Allocate a writable byte buffer in Wasm memory.
///
/// JavaScript should copy UTF-8 bytes into the returned pointer, call `sql_dialect_fmt_format`, then free
/// this input buffer with `sql_dialect_fmt_dealloc(ptr, len)`.
#[no_mangle]
pub extern "C" fn sql_dialect_fmt_alloc(len: u32) -> u32 {
    let mut buffer = Vec::<u8>::with_capacity(len as usize);
    let ptr = buffer.as_mut_ptr();
    mem::forget(buffer);
    ptr as u32
}

/// Free a buffer previously allocated by `sql_dialect_fmt_alloc`.
///
/// # Safety
///
/// `ptr` must be a pointer returned by `sql_dialect_fmt_alloc` with the same `capacity`, and it must not
/// have already been freed. Passing any other pointer or capacity is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn sql_dialect_fmt_dealloc(ptr: u32, capacity: u32) {
    if ptr == 0 || capacity == 0 {
        return;
    }
    drop(Vec::from_raw_parts(ptr as *mut u8, 0, capacity as usize));
}

/// Format the UTF-8 SQL source stored at `ptr..ptr + len`.
///
/// Returns:
/// - `0`: success; read `sql_dialect_fmt_result_ptr()` and `sql_dialect_fmt_result_len()`.
/// - `1`: invalid UTF-8 input; no result is stored.
///
/// # Safety
///
/// `ptr` must point to `len` initialized bytes in Wasm memory for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn sql_dialect_fmt_format(
    ptr: u32,
    len: u32,
    line_width: u32,
    indent_width: u32,
    uppercase_keywords: u32,
) -> u32 {
    sql_dialect_fmt_format_with_dialect(ptr, len, line_width, indent_width, uppercase_keywords, 0)
}

/// Format the UTF-8 SQL source stored at `ptr..ptr + len` using an explicit dialect.
///
/// `dialect` values:
/// - `0`: Snowflake
/// - `1`: Databricks
///
/// Unknown values fall back to Snowflake for forwards-compatible callers.
///
/// # Safety
///
/// `ptr` must point to `len` initialized bytes in Wasm memory for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn sql_dialect_fmt_format_with_dialect(
    ptr: u32,
    len: u32,
    line_width: u32,
    indent_width: u32,
    uppercase_keywords: u32,
    dialect: u32,
) -> u32 {
    let bytes = slice::from_raw_parts(ptr as *const u8, len as usize);
    format_bytes(bytes, line_width, indent_width, uppercase_keywords, dialect)
}

/// The safe core of the format ABI: validate `bytes` as UTF-8, format with the decoded raw
/// options, and stash the result for the `result_ptr`/`result_len` accessors. Split out so the
/// exact option decoding (clamping, dialect fallback) is testable without Wasm linear memory.
fn format_bytes(
    bytes: &[u8],
    line_width: u32,
    indent_width: u32,
    uppercase_keywords: u32,
    dialect: u32,
) -> u32 {
    clear_last_result();

    let Ok(source) = str::from_utf8(bytes) else {
        return 1;
    };

    let options = FormatOptions::default()
        .with_line_width(line_width.max(1) as usize)
        .with_indent_width(indent_width.clamp(1, 16) as usize)
        .with_uppercase_keywords(uppercase_keywords != 0)
        .with_dialect(dialect_from_u32(dialect));

    store_last_result(format(source, &options).into_bytes().into_boxed_slice());
    0
}

fn dialect_from_u32(dialect: u32) -> Dialect {
    match dialect {
        1 => Dialect::Databricks,
        _ => Dialect::Snowflake,
    }
}

/// Pointer to the most recent formatted result.
///
/// # Safety
///
/// The returned pointer is valid only until the next `sql_dialect_fmt_format` or `sql_dialect_fmt_clear_result`
/// call. Callers must pair it with `sql_dialect_fmt_result_len` and copy the bytes before releasing it.
#[no_mangle]
pub unsafe extern "C" fn sql_dialect_fmt_result_ptr() -> u32 {
    last_result_ptr() as u32
}

/// Byte length of the most recent formatted result.
///
/// # Safety
///
/// The value describes the buffer returned by `sql_dialect_fmt_result_ptr` and is valid only until the next
/// `sql_dialect_fmt_format` or `sql_dialect_fmt_clear_result` call.
#[no_mangle]
pub unsafe extern "C" fn sql_dialect_fmt_result_len() -> u32 {
    last_result_len() as u32
}

/// Release the most recent formatted result, if any.
///
/// # Safety
///
/// Callers must not read from a previously returned result pointer after this function runs.
#[no_mangle]
pub unsafe extern "C" fn sql_dialect_fmt_clear_result() {
    clear_last_result();
}

fn clear_last_result() {
    LAST_RESULT.with(|last_result| {
        last_result.borrow_mut().take();
    });
}

fn store_last_result(result: Box<[u8]>) {
    LAST_RESULT.with(|last_result| {
        *last_result.borrow_mut() = Some(result);
    });
}

fn last_result_ptr() -> *const u8 {
    LAST_RESULT.with(|last_result| {
        last_result
            .borrow()
            .as_deref()
            .map_or(ptr::null(), |result| result.as_ptr())
    })
}

fn last_result_len() -> usize {
    LAST_RESULT.with(|last_result| last_result.borrow().as_deref().map_or(0, <[u8]>::len))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn last_result_bytes() -> Vec<u8> {
        let ptr = last_result_ptr();
        let len = last_result_len();

        if len == 0 {
            return Vec::new();
        }

        assert!(!ptr.is_null());
        unsafe { slice::from_raw_parts(ptr, len).to_vec() }
    }

    #[test]
    fn stores_result_bytes() {
        clear_last_result();

        store_last_result(b"select 1".to_vec().into_boxed_slice());

        assert_eq!(last_result_len(), 8);
        assert_eq!(last_result_bytes(), b"select 1");

        clear_last_result();
    }

    #[test]
    fn replacing_result_exposes_only_new_bytes() {
        clear_last_result();

        store_last_result(b"old result".to_vec().into_boxed_slice());
        store_last_result(b"new".to_vec().into_boxed_slice());

        assert_eq!(last_result_len(), 3);
        assert_eq!(last_result_bytes(), b"new");

        clear_last_result();
    }

    #[test]
    fn clear_result_removes_state_and_is_idempotent() {
        clear_last_result();
        store_last_result(b"temporary".to_vec().into_boxed_slice());

        clear_last_result();
        assert!(last_result_ptr().is_null());
        assert_eq!(last_result_len(), 0);
        assert_eq!(last_result_bytes(), b"");

        clear_last_result();
        assert!(last_result_ptr().is_null());
        assert_eq!(last_result_len(), 0);
    }

    /// Run [`format_bytes`] and copy the stored result out, mirroring how JavaScript reads the
    /// `result_ptr`/`result_len` pair after a successful call.
    fn format_to_string(
        source: &str,
        line_width: u32,
        indent_width: u32,
        uppercase_keywords: u32,
        dialect: u32,
    ) -> String {
        let status = format_bytes(
            source.as_bytes(),
            line_width,
            indent_width,
            uppercase_keywords,
            dialect,
        );
        assert_eq!(status, 0, "format_bytes({source:?}) failed");
        let result = String::from_utf8(last_result_bytes()).expect("UTF-8 result");
        clear_last_result();
        result
    }

    #[test]
    fn format_defaults_produce_uppercase_snowflake_output() {
        assert_eq!(
            format_to_string("select a,b from t", 80, 4, 1, 0),
            "SELECT a, b\nFROM t;\n"
        );
    }

    #[test]
    fn uppercase_flag_zero_preserves_source_keyword_case() {
        // `uppercase_keywords = 0` maps to KeywordCase::Preserve, not lowercasing.
        assert_eq!(
            format_to_string("select a from t", 80, 4, 0, 0),
            "select a\nfrom t;\n"
        );
        assert_eq!(
            format_to_string("SELECT a from t", 80, 4, 0, 0),
            "SELECT a\nfrom t;\n"
        );
    }

    #[test]
    fn line_width_controls_select_list_wrapping() {
        let source = "select aaaa, bbbb, cccc from t";
        assert_eq!(
            format_to_string(source, 100, 4, 1, 0),
            "SELECT aaaa, bbbb, cccc\nFROM t;\n"
        );
        assert_eq!(
            format_to_string(source, 10, 4, 1, 0),
            "SELECT\n    aaaa,\n    bbbb,\n    cccc\nFROM t;\n"
        );
    }

    #[test]
    fn indent_width_is_applied_and_clamped() {
        let source = "select aaaa, bbbb from t";
        assert_eq!(
            format_to_string(source, 10, 8, 1, 0),
            "SELECT\n        aaaa,\n        bbbb\nFROM t;\n"
        );
        // 0 clamps up to 1 space, out-of-range values clamp down to 16.
        assert_eq!(
            format_to_string(source, 10, 0, 1, 0),
            "SELECT\n aaaa,\n bbbb\nFROM t;\n"
        );
        assert_eq!(
            format_to_string(source, 10, 999, 1, 0),
            format_to_string(source, 10, 16, 1, 0)
        );
    }

    #[test]
    fn zero_line_width_is_clamped_instead_of_panicking() {
        assert_eq!(
            format_to_string("select a from t", 0, 4, 1, 0),
            format_to_string("select a from t", 1, 4, 1, 0)
        );
    }

    #[test]
    fn dialect_one_selects_databricks_and_unknown_values_fall_back_to_snowflake() {
        // `a <=> b` only parses under Databricks; Snowflake passes the statement through verbatim.
        let source = "select a <=> b from t";
        assert_eq!(
            format_to_string(source, 80, 4, 1, 1),
            "SELECT a <=> b\nFROM t;\n"
        );
        let snowflake = format_to_string(source, 80, 4, 1, 0);
        assert_eq!(format_to_string(source, 80, 4, 1, 999), snowflake);
        assert_eq!(dialect_from_u32(0), Dialect::Snowflake);
        assert_eq!(dialect_from_u32(1), Dialect::Databricks);
        assert_eq!(dialect_from_u32(u32::MAX), Dialect::Snowflake);
    }

    #[test]
    fn formatting_is_idempotent() {
        let first = format_to_string("select a,b from t where x=1", 80, 4, 1, 0);
        assert_eq!(format_to_string(&first, 80, 4, 1, 0), first);
    }

    #[test]
    fn unparseable_statements_pass_through_verbatim() {
        let source = "select from where";
        let result = format_to_string(source, 80, 4, 1, 0);
        assert!(
            result.contains(source),
            "broken input should survive verbatim, got {result:?}"
        );
    }

    #[test]
    fn invalid_utf8_input_reports_failure_and_stores_no_result() {
        // Leave a stale result behind to prove a failed call clears it.
        store_last_result(b"stale".to_vec().into_boxed_slice());

        let status = format_bytes(&[0x66, 0xFF, 0xFE], 80, 4, 1, 0);

        assert_eq!(status, 1);
        assert!(last_result_ptr().is_null());
        assert_eq!(last_result_len(), 0);
    }

    #[test]
    fn empty_input_formats_to_empty_output() {
        assert_eq!(format_bytes(b"", 80, 4, 1, 0), 0);
        assert_eq!(last_result_len(), 0);
        clear_last_result();
    }

    #[test]
    fn sequential_calls_replace_the_stored_result() {
        assert_eq!(format_bytes(b"select 1", 80, 4, 1, 0), 0);
        assert_eq!(last_result_bytes(), b"SELECT 1;\n");

        assert_eq!(format_bytes(b"select 2", 80, 4, 1, 0), 0);
        assert_eq!(last_result_bytes(), b"SELECT 2;\n");
        clear_last_result();
    }

    #[test]
    fn dealloc_ignores_null_and_zero_capacity_buffers() {
        // The guard clauses must make these calls no-ops on any target.
        unsafe {
            sql_dialect_fmt_dealloc(0, 0);
            sql_dialect_fmt_dealloc(0, 16);
            sql_dialect_fmt_dealloc(4, 0);
        }
    }
}
