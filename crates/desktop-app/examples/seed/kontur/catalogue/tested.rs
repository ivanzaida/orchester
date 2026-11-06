//! The pipeline of `summary-tests` (`07 · Summary · Tests` `qN2FK`): Explore, Plan with plan approval, Implement, a
//! Test step whose failed tests go back to Implement up to twice, and Review. Its runs are recorded, not run.

use std::{collections::BTreeMap, num::NonZeroU32};

use orchester_core::{
  id::ConnectionId,
  resource::{FailureRoute, Pipeline, PromptText, RepositoryAccess, Step, StepKey, StepKind},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{Profiles, Published, SeedError, agent_step, name, pinned, publish};

/// How often the Test step sends the failed tests back before the run waits for a person.
const RETRIES: u32 = 2;

pub fn tested(db: &mut ProjectDb, connection: ConnectionId, now: Timestamp) -> Result<Published, SeedError> {
  let profiles = Profiles::publish(db, connection, now)?;
  let explore = agent_step(
    ("explore", "Explore"),
    profiles.explorer,
    RepositoryAccess::ReadOnly,
    &[],
  );
  let mut plan = agent_step(
    ("plan", "Plan"),
    profiles.planner,
    RepositoryAccess::ReadOnly,
    &["explore"],
  );
  plan.approval_gate = true;
  let implement = agent_step(
    ("implement", "Implement"),
    profiles.builder,
    RepositoryAccess::Write,
    &["plan"],
  );
  let test = Step {
    key: StepKey::new("test").expect("a valid step key"),
    name: name("Test"),
    kind: StepKind::Test,
    instructions: PromptText::new("").expect("an empty prompt is valid"),
    depends_on: [StepKey::new("implement").expect("a valid step key")].into(),
    approval_gate: false,
    on_failure: Some(FailureRoute {
      return_to: StepKey::new("implement").expect("a valid step key"),
      max_retries: NonZeroU32::new(RETRIES).expect("above zero"),
    }),
    budget: None,
  };
  let review = agent_step(
    ("review", "Review"),
    profiles.reviewer,
    RepositoryAccess::ReadOnly,
    &["test"],
  );
  let pipeline = Pipeline::new(vec![explore, plan, implement, test, review]).expect("the tested pipeline is valid");
  let (id, version) = publish(db, "Feature delivery", vec![pipeline], now)?;
  pinned(id, &version, &profiles, &BTreeMap::new())
}
