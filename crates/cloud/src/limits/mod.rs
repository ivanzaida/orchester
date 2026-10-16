//! The cloud's rate limits (`docs/architecture/server.md`, "Rate limits of the cloud"). Every refusal answers
//! `429 rate_limited` with `Retry-After`.

mod bounded;
mod quota;
mod sign_in;

use std::net::IpAddr;

use orchester_core::id::{CloudDeviceId, TeamServerId};
pub use quota::Quota;
pub use sign_in::{Attempt, SignInLimit};
use uuid::Uuid;

use crate::{
  addresses::{rate_address, rate_network},
  clock::{DAY_MS, HOUR_MS, MINUTE_MS},
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
  invite_posts: Quota<IpAddr>,
  invite_posts_per_network: Quota<IpAddr>,
  invites_per_owner: Quota<Uuid>,
  invite_emails_per_server: Quota<(TeamServerId, String)>,
  invite_emails_per_email: Quota<String>,
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
      invite_posts: Quota::new(60, HOUR_MS, MAX_KEYS),
      invite_posts_per_network: Quota::new(300, HOUR_MS, MAX_KEYS),
      invites_per_owner: Quota::new(100, DAY_MS, MAX_KEYS),
      invite_emails_per_server: Quota::new(1, DAY_MS, MAX_KEYS),
      invite_emails_per_email: Quota::new(3, DAY_MS, MAX_KEYS),
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

  /// `POST /invites`: 60 an hour per client address and 300 per IPv6 /48, counted before the signature is checked.
  pub fn invite_post(&self, address: IpAddr, now: i64) -> Result<(), CloudError> {
    self.invite_posts.take(rate_address(address), now)?;
    match rate_network(address) {
      Some(network) => self.invite_posts_per_network.take(network, now),
      None => Ok(()),
    }
  }

  /// A new invite from a server `owner` owns: 100 a day for all the servers of one account, whatever their own limits
  /// say. Counted when the invite is kept, so revoking or expiring invites gives none back.
  pub fn invite_per_owner(&self, owner: Uuid, now: i64) -> Result<(), CloudError> {
    self.invites_per_owner.take(owner, now)
  }

  /// Whether an invite to `email` from `server` gets an email: at most one a day per server and address, and three a
  /// day per address, in buckets of their own, so invites never use up an address's confirmation and reset emails
  /// (security review 1, M2). An invite without its email is still kept and listed.
  pub fn invite_email(&self, server: TeamServerId, email: &str, now: i64) -> bool {
    self
      .invite_emails_per_server
      .take((server, email.to_owned()), now)
      .is_ok()
      && self.invite_emails_per_email.take(email.to_owned(), now).is_ok()
  }

  /// 60 identity tokens an hour per device.
  pub fn identity_token(&self, device: CloudDeviceId, now: i64) -> Result<(), CloudError> {
    self.identity_tokens.take(device, now)
  }

  /// Failed confirmation or reset finishes: 20 per address and 100 per IPv6 /48 in 15 minutes, and 5 an hour per
  /// link (`link` is its token's hash), whichever addresses they come from (review 2, N1).
  pub fn link_finish(&self, address: IpAddr, link: &[u8], now: i64) -> Result<(), CloudError> {
    self.link_inspect(address, now)?;
    // Reserved before the password check, as sign-in reserves its place, so parallel guesses cannot pass a link's 5
    // (review 3); `link_succeeded` takes the reservation back.
    self.link_failures_per_link.take(link.to_vec(), now)
  }

  /// Looking up a link without using it: refused while the address or its IPv6 /48 has too many failed links.
  pub fn link_inspect(&self, address: IpAddr, now: i64) -> Result<(), CloudError> {
    self.link_failures.check(&rate_address(address), now)?;
    if let Some(network) = rate_network(address) {
      self.link_failures_per_network.check(&network, now)?;
    }
    Ok(())
  }

  /// A failed finish or inspection; a finish's link count already holds it from `link_finish`.
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
