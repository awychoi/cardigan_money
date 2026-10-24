use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};

#[derive(Default)]
pub struct RateLimiter {
    calls: HashMap<&'static str, Vec<Instant>>,
}

impl RateLimiter {
    /// Sliding-window limiter. Keys are static command names, never user input.
    pub fn check(&mut self, key: &'static str, max: usize, window: Duration) -> AppResult<()> {
        let now = Instant::now();
        let entry = self.calls.entry(key).or_default();
        entry.retain(|at| now.duration_since(*at) < window);
        if entry.len() >= max {
            return Err(AppError::RateLimited);
        }
        entry.push(now);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_limit_within_window() {
        let mut limiter = RateLimiter::default();
        for _ in 0..3 {
            limiter.check("unlock", 3, Duration::from_secs(60)).unwrap();
        }
        assert!(matches!(
            limiter.check("unlock", 3, Duration::from_secs(60)),
            Err(AppError::RateLimited)
        ));
        // Different key is independent.
        limiter.check("backup", 1, Duration::from_secs(60)).unwrap();
    }
}
