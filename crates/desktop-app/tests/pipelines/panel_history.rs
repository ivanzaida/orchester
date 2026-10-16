//! The editor's panels are the route's: each one a person opens is an entry of the tab's history, Back and
//! Forward step through them without asking about the edits, and closing one leaves no entry behind.
use orchester_app::routes::{PipelinePanel, ProjectPage, Route};
use orchester_core::{id::ResourceId, resource::StepKey};

use crate::{pipelines_support::Editor, support::project_id};

fn at(editor: &Editor, id: ResourceId, panel: PipelinePanel) {
  assert_eq!(
    editor.window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::Pipeline(id, panel))
  );
}

fn step() -> PipelinePanel {
  PipelinePanel::Step(StepKey::new("step").expect("a step key"))
}

fn back(editor: &mut Editor) {
  editor.window.navigation().back();
  editor.window.pass();
  editor.window.pass();
}

fn forward(editor: &mut Editor) {
  editor.window.navigation().forward();
  editor.window.pass();
  editor.window.pass();
}

#[test]
fn pipelines_panels_are_history_entries_back_and_forward_step_through() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  at(&editor, id, PipelinePanel::Pipeline);
  editor.click("pipeline-step-step");
  at(&editor, id, step());
  assert!(editor.exists("step-name"));
  editor.set("step-name", "Checked");
  editor.click("pipeline-history");
  at(&editor, id, PipelinePanel::History);
  editor.until("versions", |e| e.exists("pipeline-version-1"));
  editor.click("pipeline-settings");
  at(&editor, id, PipelinePanel::Settings);
  assert!(editor.exists("pipeline-settings-description"));

  back(&mut editor);
  at(&editor, id, PipelinePanel::History);
  editor.until("versions", |e| e.exists("pipeline-version-1"));
  back(&mut editor);
  at(&editor, id, step());
  assert!(
    !editor.exists("pipeline-leave-question"),
    "a panel of the editor is no departure"
  );
  assert_eq!(editor.value("step-name"), "Checked", "the step keeps its edit");
  back(&mut editor);
  at(&editor, id, PipelinePanel::Pipeline);
  assert!(editor.exists("pipeline-summary-name"));

  forward(&mut editor);
  at(&editor, id, step());
  assert!(editor.exists("step-name"));
  forward(&mut editor);
  at(&editor, id, PipelinePanel::History);
}

#[test]
fn pipelines_closing_a_panel_or_removing_its_step_leaves_no_entry() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.go(ProjectPage::Pipelines);
  editor.open(id);
  editor.click("pipeline-step-step");
  editor.click("pipeline-inspector-close");
  at(&editor, id, PipelinePanel::Pipeline);
  forward(&mut editor);
  at(&editor, id, PipelinePanel::Pipeline);
  editor.click("pipeline-step-step");
  editor.click("step-remove");
  editor.click("step-remove-confirm");
  at(&editor, id, PipelinePanel::Pipeline);
  assert!(!editor.exists("pipeline-step-step"));
  back(&mut editor);
  assert!(
    editor.exists("pipeline-leave-question"),
    "Back past the removed step's place leaves the editor and asks about the removal"
  );
}
