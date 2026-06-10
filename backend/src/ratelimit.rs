use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Maximum number of failed login attempts allowed for a single account within
/// the window before further attempts are rejected.
const MAX_FAILURES: u32 = 5;
/// Sliding window length for counting failed attempts.
const WINDOW: Duration = Duration::from_secs(15 * 60);

struct Bucket {
    window_start: Instant,
    failures: u32,
}

/// In-memory, per-account login throttle to slow credential brute-forcing.
///
/// Keyed by (normalised) email so an attacker cannot exhaust the limit for
/// other users, and so the key is not attacker-spoofable (unlike a forwarded
/// client IP behind a reverse proxy). Suitable for a single app instance; for
/// horizontally-scaled deployments back this with a shared store (e.g. Redis).
pub struct LoginRateLimiter {
    buckets: Mutex<HashMap<String, Bucket>>,
}

impl LoginRateLimiter {
    pub fn new() -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Returns true if a login attempt for `key` is currently allowed.
    pub fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        let buckets = self.buckets.lock().unwrap();
        match buckets.get(key) {
            Some(b) if now.duration_since(b.window_start) < WINDOW => b.failures < MAX_FAILURES,
            _ => true,
        }
    }

    /// Record a failed login attempt for `key`.
    pub fn record_failure(&self, key: &str) {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().unwrap();
        // Opportunistically drop expired buckets to bound memory use under a
        // spray of random (non-existent) email addresses.
        buckets.retain(|_, b| now.duration_since(b.window_start) < WINDOW);
        let bucket = buckets.entry(key.to_string()).or_insert(Bucket {
            window_start: now,
            failures: 0,
        });
        if now.duration_since(bucket.window_start) >= WINDOW {
            bucket.window_start = now;
            bucket.failures = 0;
        }
        bucket.failures = bucket.failures.saturating_add(1);
    }

    /// Clear the counter for `key` after a successful authentication.
    pub fn reset(&self, key: &str) {
        let mut buckets = self.buckets.lock().unwrap();
        buckets.remove(key);
    }
}

impl Default for LoginRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}
