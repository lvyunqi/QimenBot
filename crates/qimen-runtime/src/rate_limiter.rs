//! Token-bucket rate limiter for throttling plugin invocations.

use qimen_plugin_api::RateLimiterConfig;
use std::sync::Mutex;
use std::time::Instant;

/// A token-bucket rate limiter that refills at a configurable rate.
pub struct TokenBucketLimiter {
    inner: Mutex<TokenBucketState>,
    enabled: bool,
}

struct TokenBucketState {
    capacity: f64,
    rate: f64,
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucketLimiter {
    pub fn new(config: &RateLimiterConfig) -> Self {
        Self {
            inner: Mutex::new(TokenBucketState {
                capacity: config.capacity as f64,
                rate: config.rate,
                tokens: config.capacity as f64,
                last_refill: Instant::now(),
            }),
            enabled: config.enable,
        }
    }

    pub fn try_acquire(&self) -> bool {
        if !self.enabled {
            return true;
        }

        let mut state = match self.inner.lock() {
            Ok(guard) => guard,
            Err(_) => return true,
        };

        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        state.tokens = (state.tokens + elapsed * state.rate).min(state.capacity);
        state.last_refill = now;

        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_limiter_always_allows() {
        let limiter = TokenBucketLimiter::new(&RateLimiterConfig {
            enable: false,
            rate: 1.0,
            capacity: 1,
            timeout_secs: 0,
        });
        for _ in 0..100 {
            assert!(limiter.try_acquire());
        }
    }

    #[test]
    fn enabled_limiter_exhausts_tokens() {
        let limiter = TokenBucketLimiter::new(&RateLimiterConfig {
            enable: true,
            rate: 0.0,
            capacity: 3,
            timeout_secs: 0,
        });
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());
    }
}
