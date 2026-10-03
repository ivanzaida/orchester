use orchester_core::{
  id::{ClaimId, UserId},
  run::TaskPhase,
  sharing::{CLAIM_STALE_MS, Claim, ClaimEnd, ClaimError, ClaimKind, TaskRevision, claim_end},
  time::Timestamp,
};
fn at(ms: i64) -> Timestamp {
  Timestamp::from_unix_millis(ms)
}

#[test]
fn s1b_claim_transitions_and_explicit_stale_boundary() {
  let alice = UserId::generate();
  let bob = UserId::generate();
  let id = ClaimId::generate();
  let manual = Claim::take(None, id, alice, ClaimKind::Manual, at(100), false).expect("free claim");
  assert!(!manual.stale(at(100 + CLAIM_STALE_MS - 1)));
  assert!(manual.stale(at(100 + CLAIM_STALE_MS)));
  assert_eq!(
    Claim::take(Some(&manual), ClaimId::generate(), bob, ClaimKind::Run, at(200), false),
    Err(ClaimError::Held)
  );
  let upgraded = Claim::take(
    Some(&manual),
    ClaimId::generate(),
    alice,
    ClaimKind::Run,
    at(200),
    false,
  )
  .expect("upgrade");
  assert_eq!(upgraded.id(), id);
  assert_eq!(upgraded.kind(), ClaimKind::Run);
  assert_eq!(upgraded.since(), at(100));
  assert_eq!(
    Claim::take(
      Some(&upgraded),
      ClaimId::generate(),
      alice,
      ClaimKind::Manual,
      at(300),
      false
    )
    .expect("idempotent"),
    upgraded
  );
  assert_eq!(
    upgraded.take_over(id, ClaimId::generate(), bob, at(100 + CLAIM_STALE_MS - 1)),
    Err(ClaimError::NotStale)
  );
  let takeover = upgraded
    .take_over(id, ClaimId::generate(), bob, at(100 + CLAIM_STALE_MS))
    .expect("stale takeover");
  assert_ne!(takeover.id(), id);
  assert_eq!(takeover.taken_over_from(), Some(alice));
  assert_eq!(
    takeover.take_over(id, ClaimId::generate(), alice, at(100 + 2 * CLAIM_STALE_MS)),
    Err(ClaimError::Changed)
  );
  let mut refreshed = takeover;
  assert_eq!(refreshed.heartbeat(alice, at(500)), Err(ClaimError::NotHolder));
  refreshed
    .heartbeat(bob, at(100 + 2 * CLAIM_STALE_MS))
    .expect("heartbeat");
  assert!(!refreshed.stale(at(100 + 2 * CLAIM_STALE_MS)));
}

#[test]
fn s1b_claim_release_and_terminal_phases() {
  let user = UserId::generate();
  let claim = Claim::take(None, ClaimId::generate(), user, ClaimKind::Run, at(0), false).expect("claim");
  assert_eq!(
    claim.check_release(UserId::generate(), None),
    Err(ClaimError::NotHolder)
  );
  for phase in [TaskPhase::Running, TaskPhase::Waiting] {
    assert_eq!(claim.check_release(user, Some(phase)), Err(ClaimError::AgentWorking));
  }
  for phase in [
    TaskPhase::Paused,
    TaskPhase::StoppedAtLimit,
    TaskPhase::Succeeded,
    TaskPhase::Failed,
  ] {
    assert!(claim.check_release(user, Some(phase)).is_ok());
    assert_eq!(claim_end(phase), ClaimEnd::Keep);
  }
  assert_eq!(claim_end(TaskPhase::Cancelled), ClaimEnd::Release);
  assert_eq!(claim_end(TaskPhase::Merged), ClaimEnd::Complete);
  assert_eq!(
    Claim::take(None, ClaimId::generate(), user, ClaimKind::Run, at(0), true),
    Err(ClaimError::Merged)
  );
}

#[test]
fn s1b_revision_rejects_invalid_and_overflow() {
  assert!(TaskRevision::try_from(0).is_err());
  assert!(TaskRevision::try_from(u64::MAX).is_err());
  assert_eq!(TaskRevision::FIRST.next().expect("next").get(), 2);
  assert!(
    TaskRevision::try_from(i64::MAX as u64)
      .expect("largest")
      .next()
      .is_err()
  );
}
