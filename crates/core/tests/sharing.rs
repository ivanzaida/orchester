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
  assert_eq!(upgraded.heartbeat_at(), at(200));
  let refreshed = Claim::take(
    Some(&upgraded),
    ClaimId::generate(),
    alice,
    ClaimKind::Manual,
    at(300),
    false,
  )
  .expect("same holder refresh");
  assert_eq!(refreshed.id(), upgraded.id());
  assert_eq!(refreshed.kind(), ClaimKind::Run);
  assert_eq!(refreshed.heartbeat_at(), at(300));
  assert_eq!(
    upgraded.take_over(id, ClaimId::generate(), bob, at(200 + CLAIM_STALE_MS - 1)),
    Err(ClaimError::NotStale)
  );
  let takeover = upgraded
    .take_over(id, ClaimId::generate(), bob, at(200 + CLAIM_STALE_MS))
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

/// The rule the desktop ends its claims by (ORC-284): only a merged run completes the claim and only a cancelled one
/// releases it; every other phase of a task's latest run keeps it, so the heartbeat keeps it fresh.
#[test]
fn claim_lifecycle_only_merged_and_cancelled_runs_end_their_claim() {
  let kept = [
    TaskPhase::Draft,
    TaskPhase::Queued,
    TaskPhase::Running,
    TaskPhase::Waiting,
    TaskPhase::Stuck,
    TaskPhase::Paused,
    TaskPhase::StoppedAtLimit,
    TaskPhase::Interrupted,
    TaskPhase::Succeeded,
    TaskPhase::Failed,
  ];
  for phase in kept {
    assert_eq!(claim_end(phase), ClaimEnd::Keep, "{phase:?}");
  }
  assert_eq!(claim_end(TaskPhase::Merged), ClaimEnd::Complete);
  assert_eq!(claim_end(TaskPhase::Cancelled), ClaimEnd::Release);
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

#[test]
fn s2c_similar_titles_share_most_words_regardless_of_case() {
  let server = [
    "Fix reconnect after sleep",
    "Switcher keyboard shortcuts",
    "Fix flaky sync test",
    "Runner health indicator",
    "Token usage",
  ];
  let similar = |title: &str| orchester_core::sharing::similar_titles(title, server.iter().copied());
  assert_eq!(similar("Add switcher KEYBOARD shortcuts"), [1]);
  assert_eq!(similar("Retry the flaky sync test"), [2]);
  assert_eq!(similar("Runner health indicator in the status bar"), [3]);
  // One word in common is chance; so is a small share of the shorter title.
  assert_eq!(similar("Token usage export to CSV"), [4]);
  assert!(similar("Notes for the legacy account migration").is_empty());
  assert!(similar("Fix the docs").is_empty());
  // The larger share first, ties in the order given.
  let ordered = orchester_core::sharing::similar_titles(
    "flaky sync test retry",
    ["flaky sync bug", "sync test", "flaky sync test retry"],
  );
  assert_eq!(ordered, [1, 2, 0]);
}
