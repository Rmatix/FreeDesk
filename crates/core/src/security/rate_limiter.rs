//! In-memory sliding window rate limiter against brute-force connection attempts.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct RateLimiter {
    max_attempts: usize,
    window_duration: Duration,
    attempts: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl RateLimiter {
    pub fn new(max_attempts: usize, window_duration: Duration) -> Self {
        Self {
            max_attempts,
            window_duration,
            attempts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if key is allowed to attempt action; if allowed, records this attempt.
    pub fn check_and_record(&self, key: &str) -> bool {
        let mut map = self.attempts.lock().unwrap();
        let now = Instant::now();

        // Evict any stale keys in the entire map if size is growing
        if map.len() > 10_000 {
            let win = self.window_duration;
            map.retain(|_, timestamps| {
                timestamps.retain(|&t| now.duration_since(t) < win);
                !timestamps.is_empty()
            });
        }

        let entry = map.entry(key.to_string()).or_default();

        // Evict expired timestamps for this key
        entry.retain(|&timestamp| now.duration_since(timestamp) < self.window_duration);

        if entry.len() >= self.max_attempts {
            false
        } else {
            entry.push(now);
            true
        }
    }

    /// Reset attempts for a specific key (e.g., upon successful authentication).
    pub fn reset(&self, key: &str) {
        let mut map = self.attempts.lock().unwrap();
        map.remove(key);
    }
}
