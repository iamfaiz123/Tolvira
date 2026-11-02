//! Shared runtime for every Dev Toolbox WebAssembly engine.
//!
//! The crate intentionally keeps the original FFI surface (`alloc_buffer`,
//! `dealloc_buffer`, `store_result`, `get_result_len`) so that the frontend
//! worker (`frontend/src/workers/engine.worker.ts`) and the main-thread
//! loader (`frontend/src/services/engine-loader.ts`) keep working unchanged.
//!
//! On top of that it provides:
//!
//! * [`error`] – a structured [`EngineError`] with stable error codes.
//! * [`protocol`] – the `{ action, payload }` → `{ success, data, error, error_code }`
//!   request/response envelope and the [`run_request`] dispatcher that every
//!   engine uses from its exported `execute` function.
//! * [`b64`] – serde helpers so binary fields (files) can be transported as
//!   base64 strings inside the JSON envelope.
//! * [`rng`] – a ChaCha20 CSPRNG that backs `getrandom` for all crates and can
//!   be seeded from caller supplied entropy (`payload.entropy`).
//! * [`limits`] – input size guards shared by the file based engines.

use std::cell::RefCell;
use std::mem;

pub mod b64;
pub mod error;
pub mod limits;
pub mod protocol;
pub mod rng;

pub use error::{EngineError, EngineResult};
pub use protocol::{run_request, EngineResponse};

thread_local! {
    static SHARED_BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Allocates a buffer of `size` bytes in WASM linear memory and returns the pointer.
pub fn alloc_buffer(size: usize) -> *mut u8 {
    // A zero sized allocation would return a dangling pointer that the JS
    // side cannot distinguish from a null pointer; always reserve >= 1 byte.
    let mut buf = Vec::with_capacity(size.max(1));
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    ptr
}

/// Deallocates a buffer previously created by [`alloc_buffer`].
///
/// # Safety
/// `ptr` must come from [`alloc_buffer`] with the same `size`.
pub unsafe fn dealloc_buffer(ptr: *mut u8, size: usize) {
    if !ptr.is_null() {
        // SAFETY: guaranteed by the caller contract documented above.
        drop(unsafe { Vec::from_raw_parts(ptr, 0, size.max(1)) });
    }
}

/// Stores output bytes into the thread-local buffer and returns its pointer.
pub fn store_result(bytes: Vec<u8>) -> *const u8 {
    SHARED_BUFFER.with(|buf| {
        let mut b = buf.borrow_mut();
        *b = bytes;
        b.as_ptr()
    })
}

/// Returns the byte length of the current result in the thread-local buffer.
pub fn get_result_len() -> usize {
    SHARED_BUFFER.with(|buf| buf.borrow().len())
}

/// Generates the four `extern "C"` entry points every engine must export.
///
/// ```ignore
/// engine_shared::export_engine!(handle_action, MyAction);
/// ```
///
/// `handle_action` must have the signature
/// `fn(MyAction) -> engine_shared::EngineResult<serde_json::Value>` and
/// `MyAction` must be a `serde::Deserialize` enum tagged with
/// `#[serde(tag = "action", content = "payload")]`.
#[macro_export]
macro_rules! export_engine {
    ($handler:path, $action:ty) => {
        /// Allocates `size` bytes inside WASM linear memory.
        #[no_mangle]
        pub extern "C" fn alloc(size: usize) -> *mut u8 {
            $crate::alloc_buffer(size)
        }

        /// Frees a buffer returned by `alloc`.
        ///
        /// # Safety
        /// `ptr`/`size` must match a previous `alloc` call.
        #[no_mangle]
        pub unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize) {
            $crate::dealloc_buffer(ptr, size);
        }

        /// Length in bytes of the last response written by `execute`.
        #[no_mangle]
        pub extern "C" fn result_len() -> usize {
            $crate::get_result_len()
        }

        /// Executes a JSON request (`{"action": "...", "payload": {...}}`) and
        /// returns a pointer to the JSON response. Read `result_len()` bytes.
        ///
        /// # Safety
        /// `ptr` must point to `len` readable bytes (normally written by JS
        /// into a buffer obtained from `alloc`).
        #[no_mangle]
        pub unsafe extern "C" fn execute(ptr: *const u8, len: usize) -> *const u8 {
            let input: &[u8] = if ptr.is_null() || len == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(ptr, len)
            };
            let response = $crate::run_request::<$action, _>(input, $handler);
            $crate::store_result(response)
        }

        /// Pure (non-FFI) entry point used by unit/integration tests and by
        /// native callers: takes the request JSON bytes and returns the
        /// response JSON bytes.
        pub fn process_request(input: &[u8]) -> Vec<u8> {
            $crate::run_request::<$action, _>(input, $handler)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_roundtrip() {
        let ptr = alloc_buffer(16);
        assert!(!ptr.is_null());
        unsafe { dealloc_buffer(ptr, 16) };
        let zero = alloc_buffer(0);
        assert!(!zero.is_null());
        unsafe { dealloc_buffer(zero, 0) };
    }

    #[test]
    fn result_buffer() {
        let p = store_result(b"hello".to_vec());
        assert!(!p.is_null());
        assert_eq!(get_result_len(), 5);
    }
}

/// Small helpers that make the "never panic" rule easy to follow.
pub mod util {
    use std::cmp::Ordering;

    /// Total ordering for partially ordered values (floats): `NaN` sorts as
    /// equal instead of panicking like `partial_cmp().unwrap()` would.
    pub fn cmp_partial<T: PartialOrd>(a: &T, b: &T) -> Ordering {
        a.partial_cmp(b).unwrap_or(Ordering::Equal)
    }

    /// Total ordering for `f64` values: `NaN` sorts last.
    pub fn cmp_f64(a: &f64, b: &f64) -> Ordering {
        a.total_cmp(b)
    }
}
