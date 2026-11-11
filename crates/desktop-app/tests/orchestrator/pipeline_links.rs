//! A card that publishes a pipeline's draft opens the draft (*Open draft*): in a local project in the Pipelines hub's
//! panel. A team project's link (the pipeline's own page) is tested on the task form (`task_form pipeline_link`): the
//! Orchestrator publishes only the team's pipelines there. `cargo test -p orchester-app --test orchestrator
//! pipeline_links`.

use orchester_app::routes::{OrchestratorChat, PipelinesFilter, PipelinesView, ProjectPage, Route};
use orchester_core::id::ResourceId;

use super::{
  changes::call,
  harness::{Orchestrator, Setup},
  proposal::proposal_card,
};

const PUBLISH_PIPELINE: &str = r#"Call publish_pipeline {"project":"Kontur","pipeline":"Delivery"}"#;

/// Kontur with the pipeline *Delivery* and a draft of it; the pipeline's id.
fn with_draft(screen: &Orchestrator) -> ResourceId {
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  delivery
}

/// The agent proposes publishing *Delivery*'s draft; the card waits, and its *Open draft* is pressed.
fn open_the_draft(screen: &mut Orchestrator) {
  screen.go(OrchestratorChat::New);
  call(screen, PUBLISH_PIPELINE, 1);
  let open = format!("{}-open-0", proposal_card(screen));
  screen.until("Open draft", |screen| screen.exists(&open));
  screen.click(&open);
}

#[test]
fn orchestrator_pipeline_links_open_the_hub_in_a_local_project() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let delivery = with_draft(&screen);
  open_the_draft(&mut screen);
  assert_eq!(
    screen.route(),
    Route::Project(
      screen.project,
      ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(delivery)))
    )
  );
  screen.until("the pipeline's panel", |screen| screen.exists("pipeline-hub-panel"));
  assert!(screen.text("pipeline-hub-panel").contains("Delivery"));
}
