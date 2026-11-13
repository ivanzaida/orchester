//! The task form's links to its pipeline, *View pipeline ↗* on the plan line and *View pipeline* over the Sources
//! popover's pipeline group: in a local project they open the Pipelines hub with the pipeline's panel, in a team
//! project the pipeline's own page. `cargo test -p orchester-app --test task_form pipeline_link`.

use orchester_app::routes::{PipelineView, PipelinesFilter, ProjectPage, Route};

use crate::{harness::Form, support::team_link::share_as_owner};

/// Shares the form's project with a team, and waits until the app reads it so: its Pipelines page is the library.
fn shared(form: &mut Form) {
  share_as_owner(&form.window, &form.repo);
  form.go(ProjectPage::Pipelines);
  form.window.until("the team's pipelines library", |window| {
    window.tree.get_element_by_id("pipelines-body").is_some()
  });
}

/// Waits until the app reads the form's project as local: its Pipelines page is the hub. Until then a link opens the
/// pipeline's page.
fn local(form: &mut Form) {
  form.go(ProjectPage::Pipelines);
  form.window.until("the hub", |window| {
    window.tree.get_element_by_id("screen-pipeline-hub").is_some()
  });
}

/// New task with the pipeline whose title starts with `pipeline` chosen; its link `link` pressed.
fn press_link(form: &mut Form, pipeline: &str, link: &str) {
  form.new_task();
  form.choose_pipeline(pipeline);
  if link.starts_with("task-form-sources") {
    form.open("task-form-sources");
  }
  form
    .window
    .until(link, |window| window.tree.get_element_by_id(link).is_some());
  form.click(link);
}

/// The route is the hub's, on *All* with a pipeline chosen, and its panel names `name`.
fn in_hub(form: &mut Form, name: &str) {
  form.window.until("the hub's panel", |window| {
    window.tree.get_element_by_id("pipeline-hub-panel").is_some()
  });
  match form.window.navigation().route_now() {
    Route::Project(project, ProjectPage::PipelinesHub(view)) => {
      assert_eq!(project, form.project);
      assert_eq!(view.filter, PipelinesFilter::All);
      assert!(view.item.is_some(), "the pipeline is chosen");
    }
    route => panic!("the hub opens, not {route:?}"),
  }
  assert!(form.text("pipeline-hub-panel").contains(name));
}

/// The route is the pipeline's own page, on its Steps.
fn on_its_page(form: &mut Form) {
  assert!(
    matches!(
      form.window.navigation().route_now(),
      Route::Project(_, ProjectPage::Pipeline(_, PipelineView::Steps))
    ),
    "{:?}",
    form.window.navigation().route_now()
  );
  form.window.until("the pipeline's page", |window| {
    window.tree.get_element_by_id("pipeline-flow").is_some()
  });
}

#[test]
fn form_pipeline_link_on_the_plan_line_opens_the_hub_in_a_local_project() {
  let mut form = Form::seeded("live");
  local(&mut form);
  press_link(&mut form, "Quick fix v1", "task-form-view-pipeline");
  in_hub(&mut form, "Quick fix");
}

#[test]
fn form_pipeline_link_on_the_plan_line_opens_its_page_in_a_team_project() {
  let mut form = Form::seeded("live");
  shared(&mut form);
  press_link(&mut form, "Quick fix v1", "task-form-view-pipeline");
  on_its_page(&mut form);
}

#[test]
fn form_pipeline_link_in_sources_opens_the_hub_in_a_local_project() {
  let mut form = Form::seeded("sources");
  local(&mut form);
  press_link(&mut form, "Feature delivery", "task-form-sources-pipeline-link");
  in_hub(&mut form, "Feature delivery");
}

#[test]
fn form_pipeline_link_in_sources_opens_its_page_in_a_team_project() {
  let mut form = Form::seeded("sources");
  shared(&mut form);
  press_link(&mut form, "Feature delivery", "task-form-sources-pipeline-link");
  on_its_page(&mut form);
}
