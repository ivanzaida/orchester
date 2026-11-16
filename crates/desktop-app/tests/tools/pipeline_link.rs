//! *Open pipeline* on a tool's Access tab, beside a step that calls the tool: in a local project it opens the Pipelines
//! hub with the pipeline's panel. A team project's link (the pipeline's own page) is tested on the task form
//! (`task_form pipeline_link`): here it needs the team's own tool and pipeline, with their cached versions.
//! `cargo test -p orchester-app --test tools pipeline_link`.

use orchester_app::routes::{PipelinesFilter, PipelinesView, ProjectPage, Route, ToolTab};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Pipeline, PromptText, Slug, Step, StepKey, StepKind},
  time::Timestamp,
};

use super::{harness::Tools, support::project_id};

/// A published tool and the published pipeline *Delivery* whose one step calls it; their ids.
fn called_tool(t: &Tools) -> (ResourceId, ResourceId) {
  let tool = t.seed("Run checks", true);
  let step = Step {
    key: StepKey::new("checks").expect("key"),
    name: Name::new("Run checks").expect("name"),
    kind: StepKind::Tool { tool: Some(tool) },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let mut db = t.db();
  let pipeline = db
    .create_published(
      Slug::new("delivery").expect("slug"),
      Name::new("Delivery").expect("name"),
      &Pipeline::new(vec![step]).expect("pipeline"),
      Timestamp::now(),
    )
    .expect("pipeline created")
    .0
    .id();
  (tool, pipeline)
}

fn open_from_access(t: &mut Tools, tool: ResourceId) {
  t.go(ProjectPage::Tool(tool, ToolTab::Access));
  t.wait_for("tool-access-open-0");
  t.click("tool-access-open-0");
}

#[test]
fn tools_pipeline_link_opens_a_local_projects_pipeline_in_the_hub() {
  let mut t = Tools::new();
  let (tool, pipeline) = called_tool(&t);
  // The project is read as local once its Pipelines page is the hub: until then a link opens the pipeline's page.
  t.window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Pipelines));
  t.wait_for("screen-pipeline-hub");
  open_from_access(&mut t, tool);
  assert_eq!(
    t.window.navigation().route_now(),
    Route::Project(
      project_id(),
      ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(pipeline)))
    )
  );
  t.wait_for("pipeline-hub-panel");
  assert!(t.text("pipeline-hub-panel").contains("Delivery"));
}
