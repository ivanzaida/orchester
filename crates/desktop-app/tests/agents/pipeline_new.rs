//! New pipeline in the Pipelines hub of a local project (`Pipelines · v2` P14 `Rszkt`, P14a `d4f9Vi`, P14b `sVNK4`,
//! P14c `g09dL`; PV2-22): *New pipeline* opens an unsaved pipeline in the panel, listed first under *New*, its name with
//! the keys; *Start from* chooses the starter (its flow drawn, the steps with no profile amber), a copy or nothing;
//! *Create v1* stays off with what to fix, and nothing is stored before it; a name whose slug is taken says whose;
//! leaving asks first once something is typed. Creating a copy and *Duplicate* are in `pipelines::default` and
//! `pipelines::one_save`. `cargo test -p orchester-app --test agents pipeline_new`.

use orchester_app::routes::PipelinesFilter;
use orchester_core::{id::ResourceId, resource::ResourceKind, store::ResourceStore};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  local_hub_create::choose,
  pipeline_hub::{open, pipeline, view},
};

const PANEL: &str = "pipeline-new-panel";
const NAME: &str = "pipeline-new-name";
const HINT: &str = "team-hub-actions-hint";
const QUESTION: &str = "pipeline-hub-leave-title";
const DISCARD: &str = "pipeline-hub-leave-discard";
const KEEP: &str = "pipeline-hub-leave-cancel";

/// The hub of a project with a published *Builder* (which the starter's *Implement* runs) and pipeline *Quick fix*,
/// with *New pipeline* pressed.
fn making() -> (Agents, ResourceId) {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let quick = pipeline(&agents, "Quick fix", Some(builder));
  open(&mut agents, PipelinesFilter::All, Some(quick));
  agents.click("pipeline-hub-new");
  agents.until("the new pipeline in the panel", |s| s.exists(PANEL));
  (agents, quick)
}

/// How many pipelines the project stores.
fn stored(agents: &Agents) -> usize {
  ProjectDb::open(&agents.root)
    .expect("database")
    .resources(ResourceKind::Pipeline)
    .expect("read")
    .len()
}

/// What the bar's reason says, its pieces' words one space apart.
fn said(agents: &mut Agents) -> String {
  agents.text(HINT).split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn pipeline_new_opens_unsaved_in_the_panel_with_the_starters_flow() {
  let (mut agents, quick) = making();
  assert_eq!(agents.text("pipeline-hub-section-new-title"), "New");
  assert_eq!(agents.text("pipeline-hub-section-new-count"), "not saved");
  assert_eq!(
    agents.text("pipeline-hub-row-new-line"),
    "Starter · 6 steps · nothing saved"
  );
  assert!(
    agents.text("pipeline-hub-row-new").contains("New pipeline"),
    "named New pipeline while nothing is typed"
  );
  assert_eq!(agents.text("pipeline-new-where-what"), "new · nothing is saved yet");
  assert_eq!(
    agents.window.tree.focused_element().and_then(|element| element.id()),
    Some(NAME),
    "the name has the keys"
  );
  assert_eq!(
    agents.text("pipeline-hub-filter-all-count"),
    "1",
    "nothing new is counted"
  );
  assert!(
    agents.exists(&format!("pipeline-hub-row-{quick}")),
    "the stored pipelines stay listed"
  );

  // P14c: the starter's flow, unsaved, with the steps that have no profile amber.
  assert!(
    agents.exists("pipeline-hub-flow-step-plan-choose"),
    "Plan has no profile"
  );
  assert!(
    agents.exists("pipeline-hub-flow-step-review-choose"),
    "Review has no profile"
  );
  assert!(
    !agents.exists("pipeline-hub-flow-step-implement-choose"),
    "Implement runs Builder"
  );
  assert_eq!(
    said(&mut agents),
    "Name it and make 01 Research , 02 Plan and 3 more ready · nothing is saved yet"
  );
  assert_eq!(
    agents.text("pipeline-new-reason-plan"),
    "02 Plan",
    "a step named is a link"
  );

  agents.set(NAME, "Dependency updates");
  agents.until("the row named", |s| {
    s.text("pipeline-hub-row-new").contains("Dependency updates")
  });
  assert_eq!(
    said(&mut agents),
    "Make 01 Research , 02 Plan and 3 more ready · nothing is saved yet"
  );
  assert_eq!(agents.text("pipeline-new-slug"), "dependency-updates");
  agents.click("pipeline-new-create");
  assert_eq!(stored(&agents), 1, "Create v1 is off: nothing is stored");
  assert!(agents.exists(PANEL));
}

#[test]
fn pipeline_new_a_starters_step_opens_in_the_panel_and_back_keeps_what_was_typed() {
  let (mut agents, _) = making();
  agents.set(NAME, "Dependency updates");
  agents.click("pipeline-hub-flow-step-plan");
  agents.until("the step in the panel", |s| s.exists("pipeline-hub-step-head"));
  assert!(!agents.exists(QUESTION), "opening a step of it asks nothing");
  assert_eq!(agents.text("pipeline-hub-step-back-name"), "Dependency updates");
  assert!(agents.exists("pipeline-hub-row-new"), "it is still listed first");

  agents.click("pipeline-hub-step-back");
  agents.until("the pipeline", |s| s.exists(NAME));
  assert_eq!(agents.value(NAME), "Dependency updates", "what was typed stays");
  assert_eq!(stored(&agents), 1, "nothing is stored");
}

#[test]
fn pipeline_new_a_name_whose_slug_is_taken_says_whose_and_opening_it_asks_first() {
  let (mut agents, quick) = making();
  agents.set(NAME, "Quick fix");
  agents.until("the slug taken", |s| s.exists("pipeline-new-slug-open"));
  assert_eq!(agents.text("pipeline-new-slug-row-sub"), "Already used by Quick fix.");
  assert_eq!(agents.text("pipeline-new-slug"), "quick-fix");
  assert_eq!(agents.text("pipeline-new-slug-open"), "Open Quick fix");
  assert_eq!(agents.text(HINT), "Choose a name with a free slug");

  agents.click("pipeline-new-slug-open");
  agents.until("the question", |s| s.exists(QUESTION));
  assert_eq!(agents.text(QUESTION), "Leave this pipeline?");
  agents.click(DISCARD);
  agents.until("Quick fix", |s| {
    s.window.navigation().route_now() == view(PipelinesFilter::All, Some(quick)) && !s.exists(PANEL)
  });
  assert!(
    !agents.exists("pipeline-hub-row-new"),
    "the pipeline being made is gone"
  );
}

#[test]
fn pipeline_new_cancel_drops_it_at_once_until_something_is_typed() {
  let (mut agents, quick) = making();
  agents.click("pipeline-new-cancel");
  agents.until("the hub without it", |s| !s.exists(PANEL));
  assert!(!agents.exists(QUESTION), "nothing typed: nothing asked");
  assert!(!agents.exists("pipeline-hub-row-new"));

  agents.click("pipeline-hub-new");
  agents.until("another new pipeline", |s| s.exists(PANEL));
  assert_eq!(agents.value(NAME), "", "a new one starts empty");
  agents.set(NAME, "Dependency updates");
  agents.click("pipeline-new-cancel");
  agents.until("the question", |s| s.exists(QUESTION));
  agents.click(KEEP);
  agents.until("kept", |s| !s.exists(QUESTION));
  assert_eq!(agents.value(NAME), "Dependency updates");

  agents.click(&format!("pipeline-hub-row-{quick}"));
  agents.until("the question for another row", |s| s.exists(QUESTION));
  agents.click(DISCARD);
  agents.until("Quick fix", |s| {
    s.window.navigation().route_now() == view(PipelinesFilter::All, Some(quick)) && !s.exists(PANEL)
  });
  assert_eq!(stored(&agents), 1, "nothing was stored");
}

#[test]
fn pipeline_new_an_empty_pipeline_draws_no_steps_and_waits_for_one() {
  let (mut agents, _) = making();
  agents.set(NAME, "Release notes");
  agents.click("pipeline-new-start-empty-pick");
  agents.until("no steps", |s| s.exists("pipeline-hub-flow-empty"));
  assert!(!agents.exists("pipeline-hub-flow-step-plan"), "no cards");
  assert_eq!(agents.text(HINT), "Add a step first to Create v1");
  assert_eq!(agents.text("pipeline-hub-row-new-line"), "Empty · nothing saved");

  agents.click("pipeline-hub-add-step");
  agents.until("a step", |s| !s.exists("pipeline-hub-flow-empty"));
  assert!(
    agents.text(HINT).starts_with("Choose a profile for 01 "),
    "{}",
    agents.text(HINT)
  );
  assert_eq!(
    agents.text("pipeline-hub-row-new-line"),
    "Empty · 1 step · nothing saved"
  );
}

#[test]
fn pipeline_new_choosing_a_pipeline_to_copy_chooses_copy_and_says_what_v1_gets() {
  let (mut agents, quick) = making();
  assert_eq!(
    agents.text("pipeline-new-start-copy-line-0"),
    "Its latest version becomes v1."
  );
  agents.set(NAME, "Quick fix again");
  choose(&mut agents, "pipeline-new-copy", "Quick fix · v1");
  agents.until("Copy chosen", |s| {
    s.text("pipeline-new-start-copy-line-0") == "Published version → v1"
  });
  assert_eq!(agents.text("pipeline-new-start-copy-line-1"), "2 steps · Plan → Build");
  assert!(
    !agents.exists("pipeline-hub-flow-step-plan"),
    "a copy's steps are not drawn"
  );
  assert_eq!(
    agents.text("pipeline-hub-row-new-line"),
    "Copy of Quick fix · nothing saved"
  );
  assert_eq!(agents.text(HINT), "Saves Quick fix again as v1");
  assert_eq!(
    agents.text("pipeline-new-where-what"),
    "new · a copy of Quick fix v1 · nothing is saved yet"
  );
  assert_eq!(stored(&agents), 1, "copying {quick} stores nothing before Create v1");
}
