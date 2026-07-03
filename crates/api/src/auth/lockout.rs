//! Per-account login lockout
//!
//! Defense-in-depth alongside the IP rate limits on the login endpoints
//! (REST `/oauth/login` and the GraphQL `loginUser` mutation): after a few
//! failed attempts for one email, lock that email out for a window so a single
//! account can't be brute-forced even from rotating IPs. In-memory and
//! per-instance (resets on restart; not shared across replicas) — a deliberate,
//! good-enough choice given the IP limiter already caps total volume. The
//! tradeoff is a possible temporary lockout DoS against a known email; the short
//! window bounds it.

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

const MAX_FAILED_ATTEMPTS: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(15 * 60);
/// Cap the map so a flood of distinct emails can't grow it unbounded.
const MAX_TRACKED: usize = 10_000;

struct AttemptState {
    failures: u32,
    locked_until: Option<Instant>,
}

static ATTEMPTS: LazyLock<Mutex<HashMap<String, AttemptState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn key(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

/// True if this email is currently locked out.
pub fn is_locked(email: &str) -> bool {
    let mut map = ATTEMPTS.lock();
    if let Some(st) = map.get_mut(&key(email)) {
        match st.locked_until {
            Some(until) if Instant::now() < until => return true,
            Some(_) => {
                // Window elapsed — reset so the user gets a fresh start.
                st.failures = 0;
                st.locked_until = None;
            }
            None => {}
        }
    }
    false
}

pub fn record_failure(email: &str) {
    let mut map = ATTEMPTS.lock();
    if map.len() >= MAX_TRACKED {
        let now = Instant::now();
        map.retain(|_, st| st.locked_until.is_some_and(|u| u > now));
    }
    let st = map.entry(key(email)).or_insert(AttemptState {
        failures: 0,
        locked_until: None,
    });
    st.failures += 1;
    if st.failures >= MAX_FAILED_ATTEMPTS {
        st.locked_until = Some(Instant::now() + LOCKOUT);
    }
}

pub fn record_success(email: &str) {
    ATTEMPTS.lock().remove(&key(email));
}

#[cfg(test)]
mod tests {
    use super::{is_locked, record_failure, record_success, MAX_FAILED_ATTEMPTS};

    // `ATTEMPTS` is a process-global; each test uses a unique address so the
    // suite stays order- and parallelism-independent.

    #[test]
    fn locks_at_the_threshold_and_clears_on_success() {
        let email = "lockout-unit-threshold@test.dev";
        assert!(!is_locked(email));
        for _ in 0..MAX_FAILED_ATTEMPTS - 1 {
            record_failure(email);
            assert!(!is_locked(email), "must not lock before the threshold");
        }
        record_failure(email); // crosses MAX_FAILED_ATTEMPTS
        assert!(is_locked(email), "must lock once the threshold is reached");

        record_success(email);
        assert!(!is_locked(email), "a successful login clears the lock");
    }

    #[test]
    fn tracks_addresses_independently_and_case_insensitively() {
        let locked = "lockout-unit-locked@test.dev";
        let other = "lockout-unit-other@test.dev";
        for _ in 0..MAX_FAILED_ATTEMPTS {
            record_failure(locked);
        }
        assert!(is_locked(locked));
        assert!(!is_locked(other), "an unrelated address is unaffected");
        assert!(
            is_locked("LOCKOUT-UNIT-LOCKED@TEST.DEV"),
            "the same address in a different case shares the bucket"
        );
        record_success(locked);
    }
}
