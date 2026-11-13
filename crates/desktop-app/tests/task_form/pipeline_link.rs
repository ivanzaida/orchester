//! The task form's links to its pipeline, *View pipeline ↗* on the plan line and *View pipeline* over the Sources
//! popover's pipeline group: they open the Pipelines hub with the pipeline's panel (a team project's too, whose hub
//! is `agents pipeline_hub_team`). `cargo test -p orchester-app --test task_form pipeline_link`.

use orchester_app::routes::{PipelinesFilter, ProjectPage, Route};

use crate::harness::Form;

/// Waits until the app reads the form's project: its Pipelines page is the hub.
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

#[test]
fn form_pipeline_link_on_the_plan_line_opens_the_hub() {
  let mut form = Form::seeded("live");
  local(&mut form);
  press_link(&mut form, "Quick fix v1", "task-form-view-pipeline");
  in_hub(&mut form, "Quick fix");
}

#[test]
fn form_pipeline_link_in_sources_opens_the_hub() {
  let mut form = Form::seeded("sources");
  local(&mut form);
  press_link(&mut form, "Feature delivery", "task-form-sources-pipeline-link");
  in_hub(&mut form, "Feature delivery");
}
