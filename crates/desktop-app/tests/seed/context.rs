//! The context meter's and *Compact*'s scenarios: the Orchestrator's conversation reads back at each fullness, with a
//! compaction running, done or failed as its last entry; the Builder's session on KON-13's Implement, where its
//! question waits for the person, at each fullness.

use orchester_core::{
  connection::context_use::{ContextUse, Fullness},
  id::ChatId,
  orchestrator::{ChatEvent, CompactionState, EntryKind, TurnFailure},
  store::ActivityStore,
};
use orchester_workspace::{
  orchestrator::chat::{ChatView, context_in},
  tasks::context_uses::of_execution_in,
};

use super::{Seed, latest};

/// The conversation the Orchestrator of `seed` opens on: *Plan the webhook retry work*.
fn opened(seed: &Seed) -> ChatId {
  let chat = seed.db().latest_chat().expect("read").expect("a conversation");
  let view = ChatView::load_in(seed.repo(), chat).expect("read").expect("kept");
  assert_eq!(view.chat.title(), "Plan the webhook retry work");
  chat
}

/// The reading of the conversation `seed`'s Orchestrator opens on.
fn chat_reading(seed: &Seed) -> Option<ContextUse> {
  context_in(seed.repo(), opened(seed)).expect("read")
}

#[test]
fn seed_orchestrator_context_leaves_each_fullness_of_its_conversation() {
  let cases = [
    ("orchestrator-context-plenty", Fullness::Plenty, Some(21)),
    ("orchestrator-context-near-full", Fullness::NearFull, Some(86)),
    ("orchestrator-context-full", Fullness::Full, Some(100)),
    ("orchestrator-context-unknown", Fullness::Unknown, None),
  ];
  for (scenario, fullness, share) in cases {
    let reading = chat_reading(&Seed::of(scenario)).unwrap_or_else(|| panic!("{scenario}: a reading"));
    assert_eq!((reading.fullness(), reading.share()), (fullness, share), "{scenario}");
  }
}

/// The last entry of the conversation `seed`'s Orchestrator opens on, a compaction: from how many tokens, and how it
/// went.
fn compaction(seed: &Seed) -> (Option<u64>, CompactionState) {
  let entries = seed.db().chat_entries(opened(seed)).expect("entries");
  match entries.last().map(|entry| &entry.kind) {
    Some(EntryKind::Event(ChatEvent::Compaction { before, state })) => (*before, state.clone()),
    other => panic!("a compaction last: {other:?}"),
  }
}

#[test]
fn seed_orchestrator_compactions_run_end_and_fail() {
  let running = Seed::of("orchestrator-compacting");
  assert_eq!(compaction(&running), (Some(182_000), CompactionState::Running));
  assert!(running.db().chat_compacting(opened(&running)).expect("read"));
  let done = Seed::of("orchestrator-compacted");
  assert_eq!(
    compaction(&done),
    (Some(182_000), CompactionState::Done { after: Some(41_000) })
  );
  let after = chat_reading(&done).expect("the reading after it");
  assert_eq!(after.share(), Some(21), "the meter is back to 21%");
  let failed = Seed::of("orchestrator-compact-failed");
  assert!(matches!(
    compaction(&failed),
    (
      Some(182_000),
      CompactionState::Failed {
        reason: TurnFailure::Failed { .. }
      }
    )
  ));
  assert_eq!(
    chat_reading(&failed).map(|reading| reading.used()),
    Some(182_000),
    "the session is as it was"
  );
}

#[test]
fn seed_conversation_context_leaves_the_builders_session_at_each_fullness() {
  let cases = [
    ("conversation-context-plenty", Fullness::Plenty, 38),
    ("conversation-context-near-full", Fullness::NearFull, 86),
    ("conversation-context-full", Fullness::Full, 100),
  ];
  for (scenario, fullness, share) in cases {
    let seed = Seed::of(scenario);
    let db = seed.db();
    let run = latest(&db, "KON-13").expect("a run").expect("readable");
    assert!(run.waits_for_a_person(), "{scenario}: the Builder's question waits");
    let asking = db
      .invocations(run.id())
      .expect("invocations")
      .pop()
      .expect("the Builder's invocation");
    let reading = of_execution_in(seed.repo(), run.id(), asking.execution)
      .expect("read")
      .unwrap_or_else(|| panic!("{scenario}: a reading"));
    assert_eq!(
      (reading.fullness(), reading.share()),
      (fullness, Some(share)),
      "{scenario}"
    );
  }
}
