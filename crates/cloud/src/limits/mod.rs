//! The cloud's rate limits (`docs/architecture/server.md`, "Rate limits of the cloud"). Every refusal answers
//! `429 rate_limited` with `Retry-After`.

mod quota;
mod sign_in;

use std::net::IpAddr;

use orchester_core::id::CloudDeviceId;
pub use quota::Quota;
pub use sign_in::{Attempt, SignInLimit};

use crate::{
  addresses::rate_address,
  clock::{HOUR_MS, MINUTE_MS},
  error::CloudError,
};

/// Keys each limiter map tracks at most: about 10 MB per map when full.
const MAX_KEYS: usize = 100_000;

pub(crate) struct Limits {
  pub sign_in: SignInLimit,
  email_per_address: Quota<IpAddr>,
  email_per_email: Quota<String>,
  identity_tokens: Quota<CloudDeviceId>,
  link_failures: Quota<IpAddr>,
}

impl Limits {
  pub fn new() -> Self {
    Self {
      sign_in: SignInLimit::new(MAX_KEYS),
      email_per_address: Quota::new(20, HOUR_MS, MAX_KEYS),
      email_per_email: Quota::new(5, HOUR_MS, MAX_KEYS),
      identity_tokens: Quota::new(60, HOUR_MS, MAX_KEYS),
      link_failures: Quota::new(20, 15 * MINUTE_MS, MAX_KEYS),
    }
  }

  /// A request that sends a confirmation or reset email: 5 an hour per email, 20 an hour per client address. The
  /// address is counted first, so guessing many emails from one address stops there.
  pub fn email(&self, email: &str, address: IpAddr, now: i64) -> Result<(), CloudError> {
    self.email_per_address.take(rate_address(address), now)?;
    self.email_per_email.take(email.to_owned(), now)
  }

  /// 60 identity tokens an hour per device.
  pub fn identity_token(&self, device: CloudDeviceId, now: i64) -> Result<(), CloudError> {
    self.identity_tokens.take(device, now)
  }

  /// 20 failed confirmation or reset finishes per address in 15 minutes.
  pub fn link_finish(&self, address: IpAddr, now: i64) -> Result<(), CloudError> {
    self.link_failures.check(&rate_address(address), now)
  }

  pub fn link_failed(&self, address: IpAddr, now: i64) {
    self.link_failures.record(rate_address(address), now);
  }
}
