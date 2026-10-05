//! When a failed model request is tried again (`docs/architecture/native-agent.md` §3.1): rate limits,
//! overload, server errors and transport failures, with exponential backoff and jitter (2 s, 4 s, 8 s,
//! 16 s, 32 s by default), honouring the server's `retry-after`. Other failures are final.

use std::{
  collections::hash_map::RandomState,
  hash::{BuildHasher, Hasher},
  time::Duration,
};

use orchester_core::native::model::ModelError;

use crate::endpoint::Timing;

/// Retries after the first attempt, one per backoff step.
pub(crate) const MAX_RETRIES: u32 = 5;
/// A `retry-after` longer than this is not waited for: the request fails with it instead.
const MAX_WAIT: Duration = Duration::from_secs(300);
/// The largest share of a backoff step added as jitter, in percent.
const JITTER_PERCENT: u64 = 25;

/// Whether `error` may pass if the same request is sent again.
pub(crate) const fn retryable(error: &ModelError) -> bool {
  matches!(
    error,
    ModelError::RateLimited { .. } | ModelError::Overloaded | ModelError::Server(_) | ModelError::Transport(_)
  )
}

/// How long to wait before retry `retry` (from 1) after `error`, or `None` when the request is not
/// tried again.
pub(crate) fn wait(timing: Timing, retry: u32, error: &ModelError) -> Option<Duration> {
  if !retryable(error) || retry > MAX_RETRIES {
    return None;
  }
  if let ModelError::RateLimited {
    retry_after: Some(after),
  } = error
  {
    return (*after <= MAX_WAIT).then_some(*after);
  }
  let step = timing.first_backoff.saturating_mul(1 << (retry - 1));
  Some(step + jitter(step))
}

/// Up to [`JITTER_PERCENT`] of `step`, so that clients that failed together do not retry together.
fn jitter(step: Duration) -> Duration {
  // std has no random numbers; a fresh `RandomState` is seeded from the OS once per thread and
  // advanced for each instance, which is random enough to spread retries.
  let random = RandomState::new().build_hasher().finish();
  let millis = u64::try_from(step.as_millis()).unwrap_or(u64::MAX);
  let range = millis.saturating_mul(JITTER_PERCENT) / 100;
  Duration::from_millis(random % (range + 1))
}
