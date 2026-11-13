//! The steps of a pipeline being made in the Pipelines hub (`Pipelines · v2` P14c `g09dL`, P15 `nPaMj`, P15b `uDfc0`;
//! PV2-22 with PV2-23): *Add Deliver step* in its flow adds Deliver and opens it in the new pipeline's panel, by the id
//! the hub shows it by, never a stored one; and a copy whose source has a step that cannot run names that step in the
//! bar's reason, which opens it in the panel to fix before *Create v1*. Nothing is stored before *Create v1*.
//! `cargo test -p orchester-app --test agents pipeline_new_steps`.

use orchester_app::routes::{PipelineView, PipelinesFilter, ProjectPage, Route};
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, ResourceKind, VersionNumber},
  store::ResourceStore,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_deliver::{ADD, HEAD},
  pipeline_hub::{open, pipeline, pipeline_of},
  pipeline_step_agent::{FIELD, item},
  support::project_id,
};

const PANEL: &str = "pipeline-new-panel";
const NAME: &str = "pipeline-new-name";
const HINT: &str = "team-hub-actions-hint";
const QUESTION: &str = "pipeline-hub-leave-title";
const DELIVER_CARD: &str = "pipeline-hub-flow-deliver";

/// How many pipelines the project stores.
fn stored(agents: &Agents) -> usize {
  ProjectDb::open(&agents.root)
    .expect("database")
    .resources(ResourceKind::Pipeline)
    .expect("read")
    .len()
}

/// The pipeline the route opens a view of, and the view.
fn opened(agents: &Agents) -> (ResourceId, PipelineView) {
  match agents.window.navigation().route_now() {
    Route::Project(_, ProjectPage::Pipeline(id, view)) => (id, view),
    other => panic!("a pipeline's view: {other:?}"),
  }
}

#[test]
fn pipeline_new_add_deliver_step_edits_deliver_in_the_new_pipelines_panel() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let quick = pipeline(&agents, "Quick fix", Some(builder));
  open(&mut agents, PipelinesFilter::All, Some(quick));
  agents.click("pipeline-hub-new");
  agents.until("the new pipeline in the panel", |s| s.exists(PANEL));
  agents.set(NAME, "Dependency updates");
  agents.until("Add Deliver step in the starter's flow", |s| s.exists(ADD));

  agents.click(ADD);
  agents.until("the Deliver step in the panel", |s| s.exists(HEAD));
  let (made, view) = opened(&agents);
  assert_eq!(view, PipelineView::Deliver);
  assert_ne!(made, quick);
  assert!(
    ProjectDb::open(&agents.root)
      .expect("database")
      .resource(made)
      .expect("read")
      .is_none(),
    "the Deliver step is the new pipeline's, which is not stored"
  );
  assert!(!agents.exists(QUESTION), "opening its Deliver step asks nothing");
  assert!(agents.exists("pipeline-hub-row-new"), "it is still listed first");
  assert_eq!(agents.text("pipeline-hub-step-back-name"), "Dependency updates");

  agents.click("pipeline-hub-step-back");
  agents.until("the new pipeline", |s| s.exists(NAME));
  assert_eq!(agents.value(NAME), "Dependency updates", "what was typed stays");
  assert!(agents.exists(DELIVER_CARD), "the flow ends in the Deliver card");
  assert!(!agents.exists(ADD));

  agents.click(DELIVER_CARD);
  agents.until("Deliver again", |s| s.exists(HEAD));
  assert_eq!(opened(&agents), (made, PipelineView::Deliver));
  assert_eq!(stored(&agents), 1, "nothing is stored before Create v1");
}

#[test]
fn pipeline_new_a_copy_whose_step_cannot_run_opens_it_from_the_reason_to_fix_it() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let broken = pipeline_of(&agents, "Broken", [Some(builder), Some(ResourceId::generate())]);
  let build = ProjectDb::open(&agents.root)
    .expect("database")
    .latest_version::<Pipeline>(broken)
    .expect("read")
    .expect("a version")
    .body()
    .steps()[1]
    .key
    .clone();
  open(&mut agents, PipelinesFilter::All, Some(broken));
  agents
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::CopyPipeline(broken)));
  agents.until("the copy in the panel", |s| s.exists(PANEL));
  assert_eq!(agents.value(NAME), "Broken copy");
  assert_eq!(
    agents.text(HINT).split_whitespace().collect::<Vec<_>>().join(" "),
    "Choose a profile for 02 Build · nothing is saved yet"
  );
  let link = format!("pipeline-new-reason-{build}");
  assert_eq!(agents.text(&link), "02 Build", "the step is named as a link");

  agents.click(&link);
  agents.until("the step in the panel", |s| s.exists("pipeline-hub-step-head"));
  let (made, view) = opened(&agents);
  assert_eq!(view, PipelineView::Step(build));
  assert_ne!(made, broken, "the copy's step, not the source's");
  assert!(!agents.exists(QUESTION));

  agents.click(FIELD);
  agents.until("the picker", |s| s.exists(&item(0)));
  let mut place = 0;
  while agents.text(&format!("{}-name", item(place))) != "Builder" {
    place += 1;
    assert!(agents.exists(&item(place)), "Builder in the picker");
  }
  agents.click(&item(place));
  agents.until("chosen", |s| s.text("pipeline-hub-step-agent-name") == "Builder");

  agents.click("pipeline-hub-step-back");
  agents.until("the copy", |s| s.exists(NAME));
  assert_eq!(agents.text(HINT), "Saves Broken copy as v1", "the step is fixed");
  agents.click("pipeline-new-create");
  agents.until("the copy created", |s| {
    !s.exists(PANEL) && s.exists("pipeline-hub-panel")
  });
  assert_eq!(stored(&agents), 2);
  let source = ProjectDb::open(&agents.root)
    .expect("database")
    .latest_version::<Pipeline>(broken)
    .expect("read")
    .expect("a version");
  assert_eq!(source.number(), VersionNumber::FIRST, "the source is not touched");
}
