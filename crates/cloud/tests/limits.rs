//! The cloud's limiters and its password hashing pool are bounded (review 1, M6); no database needed.

use std::{net::IpAddr, time::Duration};

use orchester_api::{Credential, cloud::CloudErrorCode};
use orchester_cloud::{
  error::CloudError,
  for_tests::{Passwords, Quota, SignInLimit},
};

const NOW: i64 = 1_791_000_000_000;
const MINUTE: i64 = 60 * 1000;

fn busy<T>(result: &Result<T, CloudError>) -> bool {
  matches!(result, Err(CloudError::Api(CloudErrorCode::Busy)))
}

/// Review 2, N2: a full email map still admits new emails instead of refusing every new one (which locked everyone
/// out), and the per-address limit keeps holding.
#[test]
fn limits_full_email_map_admits_new_emails_and_the_address_limit_holds() {
  let limit = SignInLimit::new(3);
  let address: IpAddr = "203.0.113.7".parse().expect("address");
  for index in 0..20 {
    let attempt = limit.sign_in(format!("guess{index}@example.test"), address, NOW);
    attempt.expect("a new email gets a place").failed(NOW);
  }
  let blocked = limit.sign_in("someone@example.test".into(), address, NOW);
  assert!(
    matches!(blocked, Err(CloudError::RateLimited(_))),
    "the address is blocked"
  );
  let other: IpAddr = "198.51.100.9".parse().expect("address");
  assert!(
    limit.sign_in("someone@example.test".into(), other, NOW).is_ok(),
    "nobody else is locked out"
  );
}

/// Review 2, N2: an IPv6 client owns its whole /48 in practice, so failures from many of its /64s count together.
#[test]
fn limits_ipv6_failures_count_per_48_too() {
  let limit = SignInLimit::new(100_000);
  for index in 0..100u16 {
    let address: IpAddr = format!("2001:db8:7:{index:x}::1").parse().expect("address");
    let attempt = limit.sign_in(format!("guess{index}@example.test"), address, NOW);
    attempt.expect("one failure per /64").failed(NOW);
  }
  let neighbour: IpAddr = "2001:db8:7:ffff::1".parse().expect("address");
  let blocked = limit.sign_in("someone@example.test".into(), neighbour, NOW);
  assert!(matches!(blocked, Err(CloudError::RateLimited(_))), "the /48 is blocked");
  let elsewhere: IpAddr = "2001:db8:8::1".parse().expect("address");
  assert!(limit.sign_in("someone@example.test".into(), elsewhere, NOW).is_ok());
}

/// Review 3: a flood of new keys never evicts a key that is still counted, so it cannot reset a count early; the
/// map grows past its bound instead, and the new keys are still let in.
#[test]
fn limits_full_quota_keeps_counted_keys() {
  let quota = Quota::new(1, 60 * MINUTE, 2);
  quota.take(1, NOW).expect("first key");
  quota.take(2, NOW + 1).expect("second key");
  assert!(quota.take(3, NOW + 2).is_ok(), "a new key is not refused");
  assert!(quota.take(1, NOW + 3).is_err(), "the oldest count was not flushed");
  assert!(quota.take(2, NOW + 4).is_err());
}

/// Review 3: a blocked email keeps its 15-minute block through 100,000 new emails.
#[test]
fn limits_blocked_email_survives_a_flood_of_new_keys() {
  let limit = SignInLimit::new(100_000);
  let victim = "victim@example.test";
  for index in 0..5 {
    let address: IpAddr = format!("198.51.100.{index}").parse().expect("address");
    limit.sign_in(victim.into(), address, NOW).expect("a guess").failed(NOW);
  }
  let flood: IpAddr = "203.0.113.7".parse().expect("address");
  for index in 0..100_000 {
    let attempt = limit.sign_in(format!("flood{index}@example.test"), flood, NOW + 1);
    attempt.expect("a new email gets a place").succeeded();
  }
  let other: IpAddr = "192.0.2.1".parse().expect("address");
  let blocked = limit.sign_in(victim.into(), other, NOW + 2);
  assert!(matches!(blocked, Err(CloudError::RateLimited(_))), "the block holds");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn limits_hashing_waits_for_a_slot_only_so_long() {
  let passwords = Passwords::new(1, Duration::from_millis(1));
  let password = || Credential::new("a long enough password".into());
  let (first, second) = tokio::join!(passwords.hash(password()), passwords.hash(password()));
  assert!(
    first.is_ok() != second.is_ok(),
    "one hash runs, the other is refused instead of queueing"
  );
  assert!(busy(&first.and(second)));
}
