//! Randomness for engines running on `wasm32-unknown-unknown`.
//!
//! The worker instantiates engines with **no JavaScript imports**, so the
//! usual `getrandom` → `crypto.getRandomValues` bridge is not available.
//! Instead all randomness flows through a ChaCha20 CSPRNG held in a
//! thread-local:
//!
//! * Callers can (and for security sensitive tools **must**) pass
//!   `payload.entropy` – base64 of ≥16 bytes from `crypto.getRandomValues`.
//!   [`crate::protocol::run_request`] mixes it into the pool before dispatch.
//! * Every request additionally stirs the pool with a SHA-256 of the request
//!   bytes and an incrementing counter, so output differs between calls even
//!   without caller entropy (this is *not* cryptographically secure).
//! * The `getrandom` crate (all versions used by dependencies) is routed to
//!   this pool through its `custom` backend, so `rand`, `uuid`, `aes-gcm`,
//!   `lopdf`, `zip` etc. work transparently.
//!
//! Security sensitive actions should call [`require_secure`] which fails with
//! `E_ENTROPY_REQUIRED` unless caller entropy was supplied.

use crate::b64;
use crate::error::{EngineError, EngineResult};
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};
use sha2::{Digest, Sha256};
use std::cell::RefCell;

struct Pool {
    rng: ChaCha20Rng,
    seeded_by_caller: bool,
    counter: u64,
}

thread_local! {
    static POOL: RefCell<Pool> = RefCell::new(Pool {
        rng: ChaCha20Rng::from_seed([0x5Au8; 32]),
        seeded_by_caller: false,
        counter: 0,
    });
}

/// Minimum entropy bytes accepted from the caller.
pub const MIN_ENTROPY_BYTES: usize = 16;

/// Re-seeds the pool from caller supplied base64 entropy.
pub fn seed_from_base64(entropy_b64: &str) -> EngineResult<()> {
    let bytes = b64::decode(entropy_b64)
        .map_err(|_| EngineError::invalid("`entropy` must be base64 encoded random bytes"))?;
    if bytes.len() < MIN_ENTROPY_BYTES {
        return Err(EngineError::invalid(format!(
            "`entropy` must contain at least {MIN_ENTROPY_BYTES} bytes (got {})",
            bytes.len()
        )));
    }
    POOL.with(|p| {
        let mut p = p.borrow_mut();
        let mut h = Sha256::new();
        h.update(b"dev-toolbox-entropy");
        h.update(&bytes);
        let mut old = [0u8; 32];
        p.rng.fill_bytes(&mut old);
        h.update(old);
        let seed: [u8; 32] = h.finalize().into();
        p.rng = ChaCha20Rng::from_seed(seed);
        p.seeded_by_caller = true;
    });
    Ok(())
}

/// Mixes arbitrary bytes (e.g. the request body) into the pool.
pub fn stir(data: &[u8]) {
    POOL.with(|p| {
        let mut p = p.borrow_mut();
        p.counter = p.counter.wrapping_add(1);
        let mut h = Sha256::new();
        h.update(b"dev-toolbox-stir");
        h.update(p.counter.to_le_bytes());
        h.update(data);
        let mut old = [0u8; 32];
        p.rng.fill_bytes(&mut old);
        h.update(old);
        let seed: [u8; 32] = h.finalize().into();
        p.rng = ChaCha20Rng::from_seed(seed);
    });
}

/// Fills `dest` with random bytes from the pool.
pub fn fill_bytes(dest: &mut [u8]) {
    POOL.with(|p| p.borrow_mut().rng.fill_bytes(dest));
}

/// Returns a random `u64`.
pub fn next_u64() -> u64 {
    POOL.with(|p| p.borrow_mut().rng.next_u64())
}

/// Returns a random `u32`.
pub fn next_u32() -> u32 {
    POOL.with(|p| p.borrow_mut().rng.next_u32())
}

/// Uniform float in `[0, 1)`.
pub fn next_f64() -> f64 {
    (next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

/// Uniform integer in `[0, n)` without modulo bias. `n` must be > 0.
pub fn below(n: u64) -> u64 {
    debug_assert!(n > 0);
    if n == 0 {
        return 0;
    }
    let zone = u64::MAX - (u64::MAX % n);
    loop {
        let v = next_u64();
        if v < zone {
            return v % n;
        }
    }
}

/// Uniform integer in the inclusive range `[lo, hi]`.
pub fn range_inclusive(lo: i64, hi: i64) -> i64 {
    if lo >= hi {
        return lo;
    }
    let span = (hi as i128 - lo as i128 + 1) as u64;
    lo.wrapping_add(below(span) as i64)
}

/// Fisher–Yates shuffle.
pub fn shuffle<T>(items: &mut [T]) {
    for i in (1..items.len()).rev() {
        let j = below(i as u64 + 1) as usize;
        items.swap(i, j);
    }
}

/// Picks a random element.
pub fn choose<T>(items: &[T]) -> Option<&T> {
    if items.is_empty() {
        None
    } else {
        items.get(below(items.len() as u64) as usize)
    }
}

/// Picks a random element, reporting an empty set as an input error instead of panicking.
pub fn pick<'a, T>(items: &'a [T], what: &str) -> crate::error::EngineResult<&'a T> {
    choose(items).ok_or_else(|| crate::error::EngineError::empty(what))
}

/// Whether the pool has been seeded with caller supplied entropy during this
/// engine lifetime.
pub fn has_caller_entropy() -> bool {
    POOL.with(|p| p.borrow().seeded_by_caller)
}

/// Fails with `E_ENTROPY_REQUIRED` unless caller entropy is available.
pub fn require_secure() -> EngineResult<()> {
    if has_caller_entropy() {
        Ok(())
    } else {
        Err(EngineError::entropy_required())
    }
}

// ---------------------------------------------------------------------------
// getrandom backends
// ---------------------------------------------------------------------------

/// Backend for `getrandom` 0.3 / 0.4 (selected with
/// `--cfg getrandom_backend="custom"`, see `engines/.cargo/config.toml`).
///
/// # Safety
/// `dest` must be valid for `len` writable bytes. Called only by `getrandom`.
#[no_mangle]
pub unsafe extern "Rust" fn __getrandom_v03_custom(
    dest: *mut u8,
    len: usize,
) -> Result<(), core::num::NonZeroU32> {
    if !dest.is_null() && len > 0 {
        // SAFETY: `dest`/`len` validity is guaranteed by the `getrandom` caller contract.
        let buf = unsafe { std::slice::from_raw_parts_mut(dest, len) };
        fill_bytes(buf);
    }
    Ok(())
}

fn getrandom02_backend(buf: &mut [u8]) -> Result<(), getrandom02::Error> {
    fill_bytes(buf);
    Ok(())
}
getrandom02::register_custom_getrandom!(getrandom02_backend);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeding_and_determinism() {
        let e = b64::encode(&[7u8; 32]);
        seed_from_base64(&e).unwrap();
        let a = next_u64();
        // same entropy mixed with the (now different) pool state -> different
        seed_from_base64(&e).unwrap();
        let b = next_u64();
        assert_ne!(a, b);
        assert!(has_caller_entropy());
        assert!(require_secure().is_ok());
    }

    #[test]
    fn rejects_short_entropy() {
        assert!(seed_from_base64(&b64::encode(&[1u8; 8])).is_err());
        assert!(seed_from_base64("not base64 !!").is_err());
    }

    #[test]
    fn helpers_are_in_range() {
        for _ in 0..1000 {
            let f = next_f64();
            assert!((0.0..1.0).contains(&f));
            let r = range_inclusive(-3, 3);
            assert!((-3..=3).contains(&r));
            assert!(below(7) < 7);
        }
        assert_eq!(range_inclusive(5, 5), 5);
        assert_eq!(range_inclusive(5, 1), 5);
        let mut v: Vec<u32> = (0..50).collect();
        shuffle(&mut v);
        let mut sorted = v.clone();
        sorted.sort();
        assert_eq!(sorted, (0..50).collect::<Vec<_>>());
        assert!(choose::<u32>(&[]).is_none());
        assert!(choose(&[1, 2, 3]).is_some());
    }

    #[test]
    fn getrandom_backend_works() {
        let mut buf = [0u8; 32];
        getrandom02::getrandom(&mut buf).unwrap();
        assert!(buf.iter().any(|&b| b != 0));
        let mut buf2 = [0u8; 8];
        unsafe { __getrandom_v03_custom(buf2.as_mut_ptr(), buf2.len()).unwrap() };
        unsafe { __getrandom_v03_custom(std::ptr::null_mut(), 0).unwrap() };
    }
}
