use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{InstructionScope, Pipeline, PipelineEdit, StepKey, Version},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{harness::Instructions, preview::seed_preview};

/// Publishes the next version of pipeline `id` over `published`, whose step `removed` is replaced by a copy named
/// Review: its key.
fn replaced(db: &mut ProjectDb, (id, published): (ResourceId, &Version<Pipeline>), removed: &StepKey) -> StepKey {
  let mut edit = PipelineEdit::new(published.body().clone());
  let replacement = edit.duplicate_step(removed).expect("existing step");
  assert!(edit.update(&replacement, |step| step.name = Name::new("Review").expect("name")));
  assert!(edit.remove_step(removed));
  let body = edit.validate().expect("valid pipeline");
  db.publish_pipeline_edit(id, &body, published.number(), None, Timestamp::now())
    .expect("v2");
  replacement
}

#[test]
fn instructions_removed_step_rule_stays_listed_selectable_and_can_be_reassigned() {
  let mut screen = Instructions::new();
  let pipeline = seed_preview(&screen);
  let mut db = ProjectDb::open(&screen.root).expect("private fixture");
  let published = db
    .latest_version::<Pipeline>(pipeline)
    .expect("pipeline")
    .expect("published");
  let removed = published.body().steps()[0].key.clone();
  // The rule is written while v1 has its step; v2 removes the step (a pipeline holds no draft, D-032).
  let orphan = screen.rule(
    "Z removed step",
    "Removed step text",
    InstructionScope::Step {
      pipeline,
      step: removed.clone(),
    },
    true,
  );
  let replacement = replaced(&mut db, (pipeline, &published), &removed);
  let current = screen.rule(
    "A current step",
    "Current step text",
    InstructionScope::Step {
      pipeline,
      step: replacement.clone(),
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
      .is_none(),
    "the latest version has no such step"
  );
  screen.list();
  assert!(screen.exists(&format!("instruction-{current}")));
  assert!(
    screen.exists(&format!("instruction-{orphan}")),
    "removed-step rule remains in the library"
  );
  assert!(screen.text("instruction-library").contains("Z removed step"));
  let hint = screen.text(&format!("instruction-{orphan}"));
  assert!(hint.contains("its step was removed"), "{hint}");
  assert!(
    !hint.contains("pipeline was deleted"),
    "the pipeline still exists: {hint}"
  );
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
  screen.until("the page took the save", |s| {
    let status = s.text("instruction-save-status");
    status.contains("· saved") && !status.contains("Saving")
  });
  screen.click("instruction-crumb");
  screen.until("the list", |s| s.exists(&format!("instruction-{orphan}")));
  assert_eq!(screen.entry(current).body.text().as_str(), "Current step text");
}

#[test]
fn instructions_step_library_omits_empty_steps_but_keeps_empty_pipeline_headings() {
  use orchester_core::resource::Slug;

  let mut screen = Instructions::new();
  let pipeline = seed_preview(&screen);
  let mut db = ProjectDb::open(&screen.root).expect("private fixture");
  let published = db
    .latest_version::<Pipeline>(pipeline)
    .expect("pipeline")
    .expect("published");
  let mut edit = PipelineEdit::new(published.body().clone());
  let implement = published.body().steps()[0].key.clone();
  let review = edit.duplicate_step(&implement).expect("existing step");
  assert!(edit.update(&review, |step| step.name = Name::new("Review").expect("name")));
  let empty = edit.duplicate_step(&review).expect("existing step");
  assert!(edit.update(&empty, |step| step.name = Name::new("Run checks").expect("name")));
  let body = edit.validate().expect("valid pipeline");
  db.publish_pipeline_edit(pipeline, &body, published.number(), None, Timestamp::now())
    .expect("v2");
  db.create_published(
    Slug::new("quick-fix").expect("slug"),
    Name::new("Quick fix").expect("name"),
    &body,
    Timestamp::now(),
  )
  .expect("empty pipeline fixture");
  let first = screen.rule(
    "Implement rule",
    "Implement text",
    InstructionScope::Step {
      pipeline,
      step: implement,
    },
    true,
  );
  let second = screen.rule(
    "Review rule",
    "Review text",
    InstructionScope::Step { pipeline, step: review },
    false,
  );
  screen.list();
  screen.until("step library", |s| s.exists(&format!("instruction-{first}")));
  let library = screen.text("instruction-library");
  assert!(library.contains("01 Implement"));
  assert!(library.contains("02 Review"));
  assert!(!library.contains("03 Run checks"));
  // Each step is named twice, by its row and by its rule's hint (`umbc1`): no empty Quick fix step rows.
  assert_eq!(library.matches("01 Implement").count(), 2, "{library}");
  assert_eq!(library.matches("02 Review").count(), 2, "{library}");
  assert!(library.contains("Feature delivery"));
  assert!(library.contains("2 overrides"));
  assert!(library.contains("Quick fix"));
  assert!(library.contains("No overrides"));
  assert!(screen.exists(&format!("instruction-{second}")));
  screen.click(&format!("instruction-{second}"));
  screen.until("draft rule selected", |s| s.text("instruction-prompt") == "Review text");
}
