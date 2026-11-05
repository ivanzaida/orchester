//! The Builder's todo list in KON-12's run of the `conversation` scenario (`Live todo list · redesign` `Ucmyu`): at
//! work on Implement, it keeps seven items, three done and one in progress (07 `Y6ouq`), as the runner records the
//! latest update of its agent's `TodoWrite`. Pinned over the composer while the run goes on; folded into Implement's
//! divider while no runner looks after it.

use orchester_core::{
  connection::stream::{AgentEvent, TodoItem, TodoState},
  run::Invocation,
  store::ActivityStore,
  time::Timestamp,
};

use crate::kontur::{Kontur, SeedError};

/// The Builder's list on Implement, as it works (07 `Y6ouq`).
const BUILDER: [(&str, TodoState); 7] = [
  ("Read the webhook sender and its tests", TodoState::Done),
  ("Add RetryPolicy with exponential backoff", TodoState::Done),
  ("Cover the backoff with unit tests", TodoState::Done),
  ("Wire the policy into the sender", TodoState::InProgress),
  ("Keep 4xx responses from retrying", TodoState::Pending),
  ("Log each attempt with its delay", TodoState::Pending),
  ("Run cargo test for the webhooks crate", TodoState::Pending),
];

/// Records the Builder's list on `working`, the invocation at work, as of `at`.
pub fn seed(kontur: &mut Kontur, working: &Invocation, at: Timestamp) -> Result<(), SeedError> {
  let items = BUILDER
    .iter()
    .map(|(text, state)| TodoItem {
      text: (*text).to_owned(),
      state: *state,
    })
    .collect();
  kontur
    .db
    .append_activity(working, &[(at, AgentEvent::TodoList { items })])?;
  Ok(())
}
