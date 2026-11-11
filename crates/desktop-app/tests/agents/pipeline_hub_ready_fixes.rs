//! Every problem the Pipelines hub names has a way to its fix, in the same words wherever it is named (PV2-17): steps a
//! line cannot draw still name the step the list line names; a card whose profile lost its connection stands in amber
//! with *Open profile*; while the panel holds an edit, what the published version runs and the edit does not is left
//! out of *Ready?*, and a row about that version names it.
//! `cargo test -p orchester-app --test agents pipeline_hub_ready_fixes`.

use orchester_app::routes::{HubFilter, HubView, PipelineView, PipelinesFilter, ProjectPage, Route};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Pipeline, StepKey},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_hub::{open, pipeline_of},
  pipeline_hub_ready::{body, lost, step_at, store},
  support::project_id,
};

const PICKER: &str = "pipeline-hub-step-agent-picker";

#[test]
fn pipeline_hub_ready_fixes_steps_a_line_cannot_draw_still_name_the_step_the_list_line_names() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  // Build runs a profile that is gone: a stored pipeline's steps all run something (D-032).
  let mut steps = body([Some(builder), Some(ResourceId::generate())]).into_steps();
  let mut third = steps[0].clone();
  third.key = StepKey::new("third").expect("key");
  third.name = Name::new("Third").expect("name");
  // *Build* waits for *Plan* while *Third* starts the run too: the steps branch.
  let first = steps[0].key.clone();
  steps[1].depends_on.insert(first);
  third.depends_on.clear();
  steps.push(third);
  let branching = store(&agents, "Branching", &Pipeline::new(steps).expect("a pipeline"));
  let build = step_at(&agents, branching, 1);
  open(&mut agents, PipelinesFilter::All, Some(branching));

  let line = agents.text(&format!("pipeline-hub-row-{branching}-line"));
  assert_eq!(line, "2 things missing", "the layout and the step: {line}");
  assert_eq!(agents.text("pipeline-hub-chip-1-label"), "2 things missing");
  assert_eq!(agents.text("team-hub-readiness-count"), "2 missing");
  assert_eq!(
    agents.text("team-hub-ready-steps-words"),
    "These steps can't be drawn as one line"
  );
  let row = format!("team-hub-ready-step-{build}");
  assert_eq!(
    agents.text(&format!("{row}-label")),
    "Step 02 Build",
    "numbered as the line numbers it"
  );
  assert!(agents.exists(&format!("{row}-words")), "what the step lacks");
}

#[test]
fn pipeline_hub_ready_fixes_a_card_whose_profile_lost_its_connection_is_amber_and_opens_it() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let lost = lost(&agents, "Explorer");
  let stuck = pipeline_of(&agents, "Quick fix", [Some(builder), Some(lost)]);
  let build = step_at(&agents, stuck, 1);
  open(&mut agents, PipelinesFilter::All, Some(stuck));

  let card = format!("pipeline-hub-flow-step-{build}");
  assert_eq!(agents.text(&format!("{card}-runs")), "Explorer has no connection");
  assert_eq!(agents.text(&format!("{card}-fix-choose-words")), "Open profile");
  agents.click(&format!("{card}-fix"));
  agents.until("the profile", |s| {
    s.window.navigation().route_now()
      == Route::Project(
        project_id(),
        ProjectPage::AgentsHub(HubView::new(HubFilter::Profiles, Some(lost))),
      )
  });
}

/// A draft whose *Build* runs a profile that was archived since: gone here.
fn with_a_gone_profile(agents: &Agents) -> (ResourceId, StepKey) {
  let builder = agents.profile(true);
  let retired = agents.profile(true);
  let draft = pipeline_of(agents, "Dependency update", [Some(builder), Some(retired)]);
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(retired, Timestamp::now())
    .expect("archived");
  (draft, step_at(agents, draft, 1))
}

#[test]
fn pipeline_hub_ready_fixes_a_gone_profile_reads_as_the_card_does_and_chooses_another_in_the_picker() {
  let mut agents = Agents::new(true);
  let (draft, build) = with_a_gone_profile(&agents);
  open(&mut agents, PipelinesFilter::All, Some(draft));

  assert_eq!(
    agents.text(&format!("pipeline-hub-row-{draft}-line")),
    "02 Build runs a profile that is no longer available"
  );
  let row = format!("team-hub-ready-step-{build}");
  assert_eq!(
    agents.text(&format!("{row}-words")),
    agents.text(&format!("pipeline-hub-flow-step-{build}-runs")),
    "Ready? says it as the card does"
  );
  assert_eq!(agents.text(&format!("{row}-words")), "Profile no longer available");
  assert!(!agents.exists(&format!("{row}-open")), "no profile to open");
  assert_eq!(agents.text(&format!("{row}-choose-another")), "Choose another →");

  agents.click(&format!("{row}-choose-another"));
  agents.until("the step, its picker open", |s| s.exists(PICKER));
  assert_eq!(
    agents.window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::Pipeline(draft, PipelineView::Step(build)))
  );
}

#[test]
fn pipeline_hub_ready_fixes_a_card_whose_profile_is_gone_opens_its_step_with_the_picker_open() {
  let mut agents = Agents::new(true);
  let (draft, build) = with_a_gone_profile(&agents);
  let plan = step_at(&agents, draft, 0);
  open(&mut agents, PipelinesFilter::All, Some(draft));

  agents.click(&format!("pipeline-hub-flow-step-{build}"));
  agents.until("the step, its picker open", |s| s.exists(PICKER));

  // Another card opens its step as it always did: the picker stays closed.
  open(&mut agents, PipelinesFilter::All, Some(draft));
  agents.click(&format!("pipeline-hub-flow-step-{plan}"));
  agents.until("the plan", |s| s.exists("pipeline-hub-step-agent-name"));
  agents.window.pass();
  assert!(!agents.exists(PICKER), "only Choose another opens the picker");
}

#[test]
fn pipeline_hub_ready_fixes_an_archived_profile_of_a_published_pipeline_has_one_row_its_steps() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let retired = agents.profile(true);
  let id = pipeline_of(&agents, "Feature delivery", [Some(builder), Some(retired)]);
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(retired, Timestamp::now())
    .expect("archived");
  let build = step_at(&agents, id, 1);
  open(&mut agents, PipelinesFilter::All, Some(id));

  assert_eq!(
    agents.text("team-hub-readiness-count"),
    "1 missing",
    "one problem, one row"
  );
  let row = format!("team-hub-ready-step-{build}");
  assert_eq!(agents.text(&format!("{row}-words")), "Profile no longer available");
  assert_eq!(agents.text(&format!("{row}-choose-another")), "Choose another →");
  assert!(
    !agents.exists("team-hub-ready-undecided"),
    "not also the readiness check's own row about the archived profile"
  );
}

#[test]
fn pipeline_hub_ready_fixes_the_chip_and_the_line_count_what_ready_lists_for_a_published_pipeline() {
  let mut agents = Agents::new(true);
  let lost = lost(&agents, "Explorer");
  let retired = agents.profile(true);
  let id = pipeline_of(&agents, "Agents tour", [Some(lost), Some(retired)]);
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(retired, Timestamp::now())
    .expect("archived");
  let (plan, build) = (step_at(&agents, id, 0), step_at(&agents, id, 1));
  open(&mut agents, PipelinesFilter::All, Some(id));

  assert_eq!(agents.text("team-hub-readiness-count"), "2 missing");
  assert_eq!(
    agents.text(&format!("team-hub-ready-step-{plan}-words")),
    "Explorer has no connection"
  );
  assert_eq!(
    agents.text(&format!("team-hub-ready-step-{build}-words")),
    "Profile no longer available"
  );
  let said = "2 steps not ready";
  assert_eq!(
    agents.text(&format!("pipeline-hub-row-{id}-line")),
    said,
    "the line counts both"
  );
  assert_eq!(agents.text("pipeline-hub-chip-1-label"), said, "so does the chip");
}
