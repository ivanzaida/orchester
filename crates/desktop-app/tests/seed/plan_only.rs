//! Plan-only mode's scenarios (ORC-646): the conversation the Orchestrator opens on keeps its *Plan only* switch on,
//! and holds a plan that waits, one approved and being carried out, or one replaced and one dismissed.

use orchester_core::{
  id::ChatId,
  orchestrator::{ChatPlan, EntryKind, PlanState, TurnMode},
};

use super::Seed;

/// The conversation `seed`'s Orchestrator opens on, which keeps its switch on.
fn opened(seed: &Seed) -> ChatId {
  let chat = seed.db().latest_chat().expect("read").expect("a conversation");
  assert!(seed.db().chat(chat).expect("read").expect("kept").plan_only());
  chat
}

/// The plans of that conversation, and the modes of its person's messages.
fn read(seed: &Seed) -> (Vec<ChatPlan>, Vec<TurnMode>) {
  let entries = seed.db().chat_entries(opened(seed)).expect("entries");
  let plans = entries
    .iter()
    .filter_map(|entry| match &entry.kind {
      EntryKind::Plan(plan) => Some((**plan).clone()),
      _ => None,
    })
    .collect();
  let modes = entries
    .iter()
    .filter_map(|entry| match &entry.kind {
      EntryKind::Person { mode, .. } => Some(*mode),
      _ => None,
    })
    .collect();
  (plans, modes)
}

#[test]
fn seed_orchestrator_plan_scenarios_leave_each_state_of_a_plan() {
  let (plans, _) = read(&Seed::of("orchestrator-plan-only"));
  assert!(plans.is_empty(), "nothing asked yet");

  let (plans, modes) = read(&Seed::of("orchestrator-plan-answer"));
  assert!(matches!(&plans[..], [plan] if plan.waits() && plan.steps().len() == 4));
  assert_eq!(modes.last(), Some(&TurnMode::Plans));

  let (plans, modes) = read(&Seed::of("orchestrator-plan-approved"));
  assert!(matches!(plans[0].state(), PlanState::Approved { .. }));
  assert_eq!(modes.last(), Some(&TurnMode::CarriesOut));

  let (plans, _) = read(&Seed::of("orchestrator-plan-changed"));
  let states: Vec<PlanState> = plans.iter().map(ChatPlan::state).collect();
  assert!(
    matches!(
      &states[..],
      [
        PlanState::Replaced { .. },
        PlanState::Replaced { .. },
        PlanState::Dismissed { .. }
      ]
    ),
    "{states:?}"
  );
}
