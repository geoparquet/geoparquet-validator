//! A C interface: one call in, one JSON report out, so any language with a foreign-function
//! interface can use the validator in-process. See `include/geoparquet_validator.h`.

use std::ffi::{CStr, CString, c_char};

fn json(r: anyhow::Result<crate::checks::Report>) -> *mut c_char {
    let text = match r.and_then(|r| Ok(serde_json::to_string(&r)?)) {
        Ok(t) => t,
        Err(e) => serde_json::json!({ "error": format!("{e:#}") }).to_string(),
    };
    // a report never contains NUL, but never trust that with a null return
    CString::new(text)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Check a local path or an object-store URL. `max_rows` of 0 reads every row. Returns a JSON
/// report, or `{"error": "..."}`; free it with `gpv_free`. Returns NULL only on allocation failure.
///
/// # Safety
/// `target` must point to a valid NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpv_check(target: *const c_char, max_rows: u64) -> *mut c_char {
    if target.is_null() {
        return json(Err(anyhow::anyhow!("target is NULL")));
    }
    let target = unsafe { CStr::from_ptr(target) }
        .to_string_lossy()
        .into_owned();
    let max_rows = (max_rows > 0).then_some(max_rows as usize);
    json(crate::validate(&target, max_rows))
}

/// Check a file held in memory (`len` bytes at `data`); `name` only labels the report.
///
/// # Safety
/// `name` must be a valid NUL-terminated string and `data` must be readable for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpv_check_bytes(
    name: *const c_char,
    data: *const u8,
    len: usize,
    max_rows: u64,
) -> *mut c_char {
    if data.is_null() {
        return json(Err(anyhow::anyhow!("data is NULL")));
    }
    let name = if name.is_null() {
        "memory".to_string()
    } else {
        unsafe { CStr::from_ptr(name) }
            .to_string_lossy()
            .into_owned()
    };
    let bytes = unsafe { std::slice::from_raw_parts(data, len) }.to_vec();
    let max_rows = (max_rows > 0).then_some(max_rows as usize);
    json(crate::validate_bytes(&name, bytes, max_rows))
}

/// Free a string returned by `gpv_check` or `gpv_check_bytes`.
///
/// # Safety
/// `s` must have been returned by this library and not freed before; NULL is ignored.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpv_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// The library version, a static string; do not free it.
#[unsafe(no_mangle)]
pub extern "C" fn gpv_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}
