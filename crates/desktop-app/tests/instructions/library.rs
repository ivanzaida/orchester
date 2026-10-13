use orchester_core::{
  name::Name,
  resource::{InstructionScope, Pipeline, PipelineEdit},
  store::ResourceStore,
};
use orchester_store::ProjectDb;
use orchester_workspace::drafts::save_draft_in;

use super::{harness::Instructions, preview::seed_preview};

#[test]
fn instructions_removed_step_rule_stays_listed_selectable_and_can_be_reassigned() {
  let mut screen = Instructions::new();
  let pipeline = seed_preview(&screen);
  let db = ProjectDb::open(&screen.root).expect("private fixture");
  let published = db
    .latest_version::<Pipeline>(pipeline)
    .expect("pipeline")
    .expect("published");
  let removed = published.body().steps()[0].key.clone();
  let mut draft = PipelineEdit::new(published.body().clone());
  let replacement = draft.duplicate_step(&removed).expect("existing step");
  assert!(draft.update(&replacement, |step| step.name = Name::new("Review").expect("name")));
  assert!(draft.remove_step(&removed));
  save_draft_in(&screen.root, pipeline, draft.validate().expect("valid pipeline")).expect("saved draft");
  let current = screen.rule(
    "A current step",
    "Current step text",
    InstructionScope::Step {
      pipeline,
      step: replacement.clone(),
    },
    true,
  );
  let orphan = screen.rule(
    "Z removed step",
    "Removed step text",
    InstructionScope::Step {
      pipeline,
      step: removed.clone(),
    },
    true,
  );
  let listed = screen.listed();
  let choice = listed.pipeline(pipeline).expect("pipeline remains");
  assert!(choice.steps.iter().all(|step| step.key != removed));
  assert_eq!(choice.steps[0].key, replacement);
  assert!(
    db.latest_version::<Pipeline>(pipeline)
      .expect("pipeline")
      .expect("published")
      .body()
      .step(&removed)
      .is_some()
  );
  screen.go();
  screen.click("instruction-tab-3");
  screen.until("current rule selected", |s| {
    s.text("instruction-prompt") == "Current step text"
  });
  assert!(screen.exists(&format!("instruction-{current}")));
  assert!(
    screen.exists(&format!("instruction-{orphan}")),
    "removed-step rule remains in the library"
  );
  assert!(screen.text("instruction-library").contains("Z removed step"));
  screen.click(&format!("instruction-{orphan}"));
  screen.until("removed rule selected", |s| {
    s.text("instruction-prompt") == "Removed step text"
  });
  screen.choose("instruction-step", "01 Review");
  screen.until("scope reassigned", |s| {
    matches!(s.entry(orphan).body.scope(), InstructionScope::Step { pipeline: id, step } if *id == pipeline && *step == replacement)
  });
  assert!(screen.entry(orphan).state.has_draft());
  assert_eq!(screen.entry(orphan).state.published().expect("published").get(), 1);
  assert!(screen.exists(&format!("instruction-{orphan}")));
  assert_eq!(screen.entry(current).body.text().as_str(), "Current step text");
}
