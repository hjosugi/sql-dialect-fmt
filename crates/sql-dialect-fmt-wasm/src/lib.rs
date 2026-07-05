//! Raw WebAssembly bindings for browser extension use.
//!
//! This crate deliberately avoids `wasm-bindgen` so the Chrome extension can load a single local
//! `.wasm` file without generated JavaScript glue. JavaScript owns input buffers allocated through
//! `sql_dialect_fmt_alloc`; this module owns the last formatted result until the next call or
//! `sql_dialect_fmt_clear_result`.

use std::{cell::RefCell, mem, ptr, slice, str};

use sql_dialect_fmt_formatter::{format, FormatOptions};

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
    clear_last_result();

    let bytes = slice::from_raw_parts(ptr as *const u8, len as usize);
    let Ok(source) = str::from_utf8(bytes) else {
        return 1;
    };

    let options = FormatOptions::default()
        .with_line_width(line_width.max(1) as usize)
        .with_indent_width(indent_width.clamp(1, 16) as usize)
        .with_uppercase_keywords(uppercase_keywords != 0);

    store_last_result(format(source, &options).into_bytes().into_boxed_slice());
    0
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
}
