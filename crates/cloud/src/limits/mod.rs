//! The cloud's rate limits (`docs/architecture/server.md`, "Rate limits of the cloud"). Every refusal answers
//! `429 rate_limited` with `Retry-After`.

mod bounded;
mod quota;
mod sign_in;

use std::net::IpAddr;

use orchester_core::id::CloudDeviceId;
pub use quota::Quota;
pub use sign_in::{Attempt, SignInLimit};

use crate::{
  addresses::{rate_address, rate_network},
  clock::{HOUR_MS, MINUTE_MS},
  error::CloudError,
};

/// Keys each limiter map tracks at most, about 10 MB per map when full; beyond them the oldest keys are evicted.
const MAX_KEYS: usize = 100_000;

pub(crate) struct Limits {
  pub sign_in: SignInLimit,
  email_per_address: Quota<IpAddr>,
  email_per_network: Quota<IpAddr>,
  email_per_email: Quota<String>,
  identity_tokens: Quota<CloudDeviceId>,
  link_failures: Quota<IpAddr>,
  link_failures_per_network: Quota<IpAddr>,
  link_failures_per_link: Quota<Vec<u8>>,
}

impl Limits {
  pub fn new() -> Self {
    Self {
      sign_in: SignInLimit::new(MAX_KEYS),
      email_per_address: Quota::new(20, HOUR_MS, MAX_KEYS),
      email_per_network: Quota::new(100, HOUR_MS, MAX_KEYS),
      email_per_email: Quota::new(5, HOUR_MS, MAX_KEYS),
      identity_tokens: Quota::new(60, HOUR_MS, MAX_KEYS),
      link_failures: Quota::new(20, 15 * MINUTE_MS, MAX_KEYS),
      link_failures_per_network: Quota::new(100, 15 * MINUTE_MS, MAX_KEYS),
      link_failures_per_link: Quota::new(5, HOUR_MS, MAX_KEYS),
    }
  }

  /// A request that sends a confirmation or reset email: 5 an hour per email, 20 an hour per client address and 100
  /// per IPv6 /48. The address is counted first, so guessing many emails from one address stops there.
  pub fn email(&self, email: &str, address: IpAddr, now: i64) -> Result<(), CloudError> {
    self.email_per_address.take(rate_address(address), now)?;
    if let Some(network) = rate_network(address) {
      self.email_per_network.take(network, now)?;
    }
    self.email_per_email.take(email.to_owned(), now)
  }

  /// 60 identity tokens an hour per device.
  pub fn identity_token(&self, device: CloudDeviceId, now: i64) -> Result<(), CloudError> {
    self.identity_tokens.take(device, now)
  }

  /// Failed confirmation or reset finishes: 20 per address and 100 per IPv6 /48 in 15 minutes, and 5 an hour per
  /// link (`link` is its token's hash), whichever addresses they come from (review 2, N1).
  pub fn link_finish(&self, address: IpAddr, link: &[u8], now: i64) -> Result<(), CloudError> {
    self.link_failures.check(&rate_address(address), now)?;
    if let Some(network) = rate_network(address) {
      self.link_failures_per_network.check(&network, now)?;
    }
    // Reserved before the password check, as sign-in reserves its place, so parallel guesses cannot pass a link's 5
    // (review 3); `link_succeeded` takes the reservation back.
    self.link_failures_per_link.take(link.to_vec(), now)
  }

  /// A failed finish; the link's own count already holds it from `link_finish`.
  pub fn link_failed(&self, address: IpAddr, now: i64) {
    self.link_failures.record(rate_address(address), now);
    if let Some(network) = rate_network(address) {
      self.link_failures_per_network.record(network, now);
    }
  }

  /// A finish that succeeded: its reservation, made at `reserved_at`, does not count against the link.
  pub fn link_succeeded(&self, link: &[u8], reserved_at: i64) {
    self.link_failures_per_link.forgive(&link.to_vec(), reserved_at);
  }
}
