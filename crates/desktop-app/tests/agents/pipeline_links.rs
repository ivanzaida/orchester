//! A link from another page to a pipeline of a local project (N-013): the link names the pipeline under *All*, as an
//! Orchestrator card made before the pipeline was archived does, and the hub opens the tab that holds it with it
//! chosen. `cargo test -p orchester-app --test agents pipeline_links`.

use orchester_app::routes::PipelinesFilter;
use orchester_core::time::Timestamp;
use orchester_store::ProjectDb;

use super::pipeline_hub::{setup, view};

#[test]
fn pipeline_links_to_an_archived_pipeline_open_it_under_archived() {
  let (mut agents, [ready, ..]) = setup();
  let mut db = ProjectDb::open(&agents.root).expect("database");
  db.archive_local_resource(ready, Timestamp::now()).expect("archived");
  agents.window.navigation().go(view(PipelinesFilter::All, Some(ready)));
  let landed = view(PipelinesFilter::Archived, Some(ready));
  agents.until("the archived pipeline's panel under Archived", |s| {
    s.window.navigation().route_now() == landed && s.exists("pipeline-hub-restore")
  });
  assert_eq!(agents.text("team-hub-panel-title"), "Feature delivery");
  assert!(agents.exists(&format!("pipeline-hub-row-{ready}")));
}

#[test]
fn pipeline_links_to_a_pipeline_under_the_other_tab_open_the_tab_that_holds_it() {
  let (mut agents, [_, draft, _]) = setup();
  agents
    .window
    .navigation()
    .go(view(PipelinesFilter::Archived, Some(draft)));
  let landed = view(PipelinesFilter::All, Some(draft));
  agents.until("the pipeline's panel under All", |s| {
    s.window.navigation().route_now() == landed && s.exists("pipeline-hub-more")
  });
  assert_eq!(agents.text("team-hub-panel-title"), "Dependency update");
}
