//! A pipeline draft whose step stops waiting for an approval-gated step it waited for widens what runs before a person
//! looks: the Orchestrator's publication card names it in the short form of its *Widens* row (`UKgrR`), *step Build
//! stops waiting for Plan*. `cargo test -p orchester-app --test orchestrator changes_waiting`.

use std::collections::BTreeSet;

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{Draft, Pipeline, PromptText, RepositoryAccess, Slug, Step, StepKey, StepKind},
  store::ResourceStore,
  time::Timestamp,
};

use super::{
  changes::call,
  harness::{Orchestrator, Setup},
  proposal::proposal_card,
};

/// A step `key` named `name` that `builder` runs, after `after`.
fn step(builder: ResourceId, (key, name): (&str, &str), after: &[&str], gate: bool) -> Step {
  Step {
    key: StepKey::new(key).expect("key"),
    name: Name::new(name).expect("name"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Read README.md.").expect("prompt"),
    depends_on: after
      .iter()
      .map(|key| StepKey::new(key).expect("key"))
      .collect::<BTreeSet<_>>(),
    approval_gate: gate,
    on_failure: None,
    budget: None,
  }
}

#[test]
fn orchestrator_changes_waiting_a_step_that_stops_waiting_for_an_approval_is_named() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let now = Timestamp::now();
  let mut db = screen.db();
  // v1: Plan waits for the person's approval, and Build runs after it.
  let gated = Pipeline::new(vec![
    step(builder, ("plan", "Plan"), &[], true),
    step(builder, ("build", "Build"), &["plan"], false),
  ])
  .expect("a pipeline");
  let delivery = db
    .create_resource(
      Slug::new("delivery").expect("slug"),
      Name::new("Delivery").expect("name"),
      &gated,
      now,
    )
    .expect("pipeline")
    .id();
  db.publish_draft::<Pipeline>(delivery, now).expect("v1");
  // The draft lets Build start beside Plan: it no longer waits for the approval.
  let loose = Pipeline::new(vec![
    step(builder, ("plan", "Plan"), &[], true),
    step(builder, ("build", "Build"), &[], false),
  ])
  .expect("a pipeline");
  let v1 = db
    .latest_version::<Pipeline>(delivery)
    .expect("read")
    .expect("published")
    .number();
  db.save_draft(delivery, &Draft::new(loose, Some(v1), now))
    .expect("draft");

  screen.go(OrchestratorChat::New);
  call(
    &mut screen,
    r#"Call publish_pipeline {"project":"Kontur","pipeline":"Delivery"}"#,
    1,
  );
  let card = proposal_card(&screen);
  screen.until("the card's facts", |screen| {
    screen.text(&format!("{card}-facts")).contains("Widens")
  });
  let facts = screen.text(&format!("{card}-facts"));
  assert!(
    facts.contains("step Build stops waiting for Plan"),
    "the short form of what widens: {facts}"
  );
}
