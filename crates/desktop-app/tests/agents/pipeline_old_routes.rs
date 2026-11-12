//! A local project's routes to the old Pipelines page open the Pipelines hub (PV2-14): the old page's Settings and
//! Versions tabs, its publish review and a published version show the pipeline's panel, a step's page opens the step
//! in the panel, each under the hub's own route; the old page is never shown. `cargo test -p orchester-app --test
//! agents pipeline_old_routes`.

use orchester_app::routes::{PipelineView, ProjectPage, Route, StepPage};
use orchester_core::resource::VersionNumber;

use super::{
  harness::Agents,
  pipeline_deliver::pipeline_route,
  pipeline_hub::setup,
  pipeline_step::{keys, named, route, step_route},
  support::project_id,
};

/// Goes to `page`, waits until the hub shows its panel under `shown`, and checks that the old page is not there.
fn lands(agents: &mut Agents, page: ProjectPage, shown: &Route, what: &str) {
  agents.window.navigation().go(Route::Project(project_id(), page));
  agents.until(what, |s| route(s) == *shown && s.exists("pipeline-hub-panel"));
  assert!(!agents.exists("screen-pipelines"), "{what}: never the old page");
}

#[test]
fn pipeline_old_routes_settings_opens_the_pipeline_s_panel() {
  let (mut agents, [ready, ..]) = setup();
  lands(
    &mut agents,
    ProjectPage::Pipeline(ready, PipelineView::Settings),
    &pipeline_route(ready),
    "Settings in the hub",
  );
  assert!(
    agents.exists("pipeline-hub-settings"),
    "the panel holds the pipeline's settings"
  );
}

#[test]
fn pipeline_old_routes_versions_opens_the_pipeline_s_panel() {
  let (mut agents, [ready, ..]) = setup();
  lands(
    &mut agents,
    ProjectPage::Pipeline(ready, PipelineView::Versions),
    &pipeline_route(ready),
    "Versions in the hub",
  );
}

#[test]
fn pipeline_old_routes_the_publish_review_opens_the_pipeline_s_panel() {
  let (mut agents, [ready, ..]) = setup();
  lands(
    &mut agents,
    ProjectPage::Pipeline(ready, PipelineView::Publish),
    &pipeline_route(ready),
    "the publish review in the hub",
  );
}

#[test]
fn pipeline_old_routes_a_published_version_opens_the_pipeline_s_panel() {
  let (mut agents, [ready, ..]) = setup();
  lands(
    &mut agents,
    ProjectPage::PipelineVersion(ready, VersionNumber::FIRST, PipelineView::Steps),
    &pipeline_route(ready),
    "a version in the hub",
  );
}

#[test]
fn pipeline_old_routes_a_step_s_page_opens_the_step_in_the_panel() {
  let (mut agents, [ready, ..]) = setup();
  let plan = keys(&agents, ready)[0].clone();
  lands(
    &mut agents,
    ProjectPage::Pipeline(ready, PipelineView::StepPage(plan.clone(), StepPage::Instructions)),
    &step_route(ready, &plan),
    "the step in the hub",
  );
  agents.until("the step's head", |s| named(s, "Plan"));
}
