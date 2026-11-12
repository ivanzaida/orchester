//! The steps of a pipeline being made in the Pipelines hub (`Pipelines · v2` P14c `g09dL`, P15 `nPaMj`, P15b `uDfc0`;
//! PV2-22 with PV2-23): *Add Deliver step* in its flow adds Deliver and opens it in the new pipeline's panel, by the id
//! the hub shows it by, never a stored one. Nothing is stored before *Create v1*.
//! `cargo test -p orchester-app --test agents pipeline_new_steps`.

use orchester_app::routes::{PipelineView, PipelinesFilter, ProjectPage, Route};
use orchester_core::{id::ResourceId, resource::ResourceKind, store::ResourceStore};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_deliver::{ADD, HEAD},
  pipeline_hub::{open, pipeline},
};

const PANEL: &str = "pipeline-new-panel";
const NAME: &str = "pipeline-new-name";
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
