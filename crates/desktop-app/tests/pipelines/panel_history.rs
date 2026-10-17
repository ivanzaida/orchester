//! The editor's panels and the version it views are the route's: each one a person opens is an entry of the
//! tab's history, Back and Forward step through them without asking about the edits, and closing one leaves
//! no entry behind.
use orchester_app::routes::{PipelinePanel, ProjectPage, Route};
use orchester_core::{
  id::ResourceId,
  resource::{StepKey, VersionNumber},
};

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

fn viewing(editor: &Editor, id: ResourceId, panel: PipelinePanel) {
  assert_eq!(
    editor.window.navigation().route_now(),
    Route::Project(
      project_id(),
      ProjectPage::PipelineVersion(id, VersionNumber::FIRST, panel)
    )
  );
}

#[test]
fn pipelines_a_viewed_version_is_in_the_route_and_back_returns_to_the_draft() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-step-step");
  editor.set("step-name", "Checked");
  editor.click("pipeline-history");
  editor.until("versions", |e| e.exists("pipeline-version-1"));
  editor.click("pipeline-version-1");
  viewing(&editor, id, PipelinePanel::History);
  assert!(editor.exists("pipeline-back-draft"), "a past version is viewed");
  editor.click("pipeline-step-step");
  viewing(&editor, id, step());
  assert!(editor.exists("pipeline-back-draft"), "the version's step is shown");

  back(&mut editor);
  viewing(&editor, id, PipelinePanel::History);
  assert!(editor.exists("pipeline-back-draft"));
  back(&mut editor);
  at(&editor, id, PipelinePanel::History);
  assert!(
    !editor.exists("pipeline-back-draft"),
    "Back before the pick shows the draft's history"
  );
  back(&mut editor);
  at(&editor, id, step());
  assert!(!editor.exists("pipeline-back-draft"));
  assert_eq!(editor.value("step-name"), "Checked", "the draft's edit shows again");
  editor.set("step-name", "Checked again");
  assert_eq!(editor.value("step-name"), "Checked again", "the step is editable");

  forward(&mut editor);
  at(&editor, id, PipelinePanel::History);
  assert!(!editor.exists("pipeline-back-draft"));
  forward(&mut editor);
  viewing(&editor, id, PipelinePanel::History);
  editor.until("the version read again", |e| e.exists("pipeline-back-draft"));
  editor.click("pipeline-back-draft");
  at(&editor, id, PipelinePanel::Pipeline);
  assert!(!editor.exists("pipeline-back-draft"));
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
