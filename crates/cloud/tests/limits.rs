//! The cloud's limiters and its password hashing pool are bounded (review 1, M6); no database needed.

use std::{net::IpAddr, time::Duration};

use orchester_api::{Credential, cloud::CloudErrorCode};
use orchester_cloud::{
  error::CloudError,
  limits::{Quota, SignInLimit},
  passwords::Passwords,
};

const NOW: i64 = 1_791_000_000_000;
const MINUTE: i64 = 60 * 1000;

fn busy<T>(result: &Result<T, CloudError>) -> bool {
  matches!(result, Err(CloudError::Api(CloudErrorCode::Busy)))
}

#[test]
fn limits_sign_in_map_is_bounded_and_pruned() {
  let limit = SignInLimit::new(2);
  let address: IpAddr = "203.0.113.7".parse().expect("address");
  limit
    .sign_in("a@example.test".into(), address, NOW)
    .expect("first")
    .failed(NOW);
  limit
    .sign_in("b@example.test".into(), address, NOW)
    .expect("second")
    .failed(NOW);
  assert!(
    busy(&limit.sign_in("c@example.test".into(), address, NOW)),
    "a third email does not grow the map"
  );
  let known = limit.sign_in("a@example.test".into(), address, NOW + 1);
  assert!(known.is_ok(), "an email counted already goes on");
  drop(known);
  // Once their failures are 15 minutes old, the next minute's pruning makes room.
  let later = NOW + 16 * MINUTE;
  assert!(limit.sign_in("c@example.test".into(), address, later).is_ok());
}

#[test]
fn limits_quota_is_bounded() {
  let quota = Quota::new(5, 60 * MINUTE, 2);
  quota.take(1, NOW).expect("first key");
  quota.take(2, NOW).expect("second key");
  assert!(busy(&quota.take(3, NOW)), "a third key does not grow the map");
  assert!(quota.take(1, NOW).is_ok(), "a key counted already goes on");
  assert!(busy(&quota.check(&3, NOW)));
  assert!(quota.take(3, NOW + 61 * MINUTE).is_ok(), "pruned after its window");
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
