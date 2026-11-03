use orchester_core::{
  id::{ClaimId, UserId},
  sharing::{Claim, ClaimError, ClaimKind, ClaimStalenessThreshold, ClaimStalenessThresholdError},
  time::Timestamp,
};

#[test]
fn claim_threshold_bounds_default_and_serde() {
  assert_eq!(ClaimStalenessThreshold::default().seconds(), 900);
  for seconds in [300, 900, 86400] {
    let threshold = ClaimStalenessThreshold::new(seconds).expect("valid seconds");
    assert_eq!(threshold.millis(), seconds as i64 * 1000);
    let json = serde_json::to_string(&threshold).expect("serialize");
    assert_eq!(json, seconds.to_string());
    assert_eq!(
      serde_json::from_str::<ClaimStalenessThreshold>(&json).expect("round trip"),
      threshold
    );
  }
  for seconds in [0, 299, 86401, u64::MAX] {
    assert_eq!(ClaimStalenessThreshold::new(seconds), Err(ClaimStalenessThresholdError));
    assert!(serde_json::from_str::<ClaimStalenessThreshold>(&seconds.to_string()).is_err());
  }
  for json in ["-1", "300.5", "\"900\""] {
    assert!(serde_json::from_str::<ClaimStalenessThreshold>(json).is_err());
  }
}

#[test]
fn claim_threshold_uses_execution_heartbeat_at_exact_edge() {
  let holder = UserId::generate();
  let mut claim = Claim::take(None, ClaimId::generate(), holder, ClaimKind::Run, at(0), false).expect("claim");
  claim.heartbeat(holder, at(2000)).expect("heartbeat");
  for seconds in [300, 900, 86400] {
    let threshold = ClaimStalenessThreshold::new(seconds).expect("threshold");
    let edge = 2000 + threshold.millis();
    assert!(!claim.stale(at(1000), threshold));
    assert!(!claim.stale(at(edge - 1), threshold));
    assert!(claim.stale(at(edge), threshold));
    assert_eq!(
      claim.take_over(
        claim.id(),
        ClaimId::generate(),
        UserId::generate(),
        at(edge - 1),
        threshold
      ),
      Err(ClaimError::NotStale)
    );
    assert!(
      claim
        .take_over(claim.id(), ClaimId::generate(), UserId::generate(), at(edge), threshold)
        .is_ok()
    );
  }
}

fn at(ms: i64) -> Timestamp {
  Timestamp::from_unix_millis(ms)
}
