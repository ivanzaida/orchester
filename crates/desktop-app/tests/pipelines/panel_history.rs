//! The page's tabs, its chosen step and the version it views are the route's: each place a person opens is an
//! entry of the tab's history, Back and Forward step through them without asking about the edits, and removing a
//! step leaves no entry behind.
use orchester_app::routes::{PipelineView, ProjectPage, Route, StepPage};
use orchester_core::{
  id::ResourceId,
  resource::{StepKey, VersionNumber},
};

use crate::{pipelines_support::Editor, support::project_id};

fn at(editor: &Editor, id: ResourceId, view: PipelineView) {
  assert_eq!(
    editor.window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::Pipeline(id, view))
  );
}

fn key() -> StepKey {
  StepKey::new("step").expect("a step key")
}

fn step() -> PipelineView {
  PipelineView::Step(key())
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
fn pipelines_tabs_and_steps_are_history_entries_back_and_forward_step_through() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  at(&editor, id, PipelineView::Steps);
  editor.click("pipeline-step-step");
  at(&editor, id, step());
  assert!(editor.exists("step-name"));
  editor.set("step-name", "Checked");
  editor.click("pipeline-history");
  at(&editor, id, PipelineView::Versions);
  editor.until("versions", |e| e.exists("pipeline-version-1"));
  editor.click("pipeline-settings");
  at(&editor, id, PipelineView::Settings);
  assert!(editor.exists("pipeline-settings-description"));

  back(&mut editor);
  at(&editor, id, PipelineView::Versions);
  editor.until("versions", |e| e.exists("pipeline-version-1"));
  back(&mut editor);
  at(&editor, id, step());
  assert!(
    !editor.exists("pipeline-leave-question"),
    "a tab of the page is no departure"
  );
  assert_eq!(editor.value("step-name"), "Checked", "the step keeps its edit");
  back(&mut editor);
  at(&editor, id, PipelineView::Steps);
  assert!(editor.exists("pipeline-summary-name"));

  forward(&mut editor);
  at(&editor, id, step());
  assert!(editor.exists("step-name"));
  forward(&mut editor);
  at(&editor, id, PipelineView::Versions);
}

#[test]
fn pipelines_a_steps_pages_are_places_of_their_own() {
  let mut editor = Editor::new();
  editor.profile();
  let id = editor.empty("Pages");
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.choose("step-executor", "Reviewer");
  let added = match editor.window.navigation().route_now() {
    Route::Project(_, ProjectPage::Pipeline(_, PipelineView::Step(key))) => key,
    other => panic!("the added step is chosen: {other:?}"),
  };
  editor.click("step-open-permissions");
  at(&editor, id, PipelineView::StepPage(added.clone(), StepPage::Permissions));
  assert!(editor.text("step-page-back").contains("01 New step"));
  assert!(editor.exists("step-ask-tools"));
  editor.click("step-page-back");
  at(&editor, id, PipelineView::Step(added.clone()));
  editor.click("step-open-sources");
  at(&editor, id, PipelineView::StepPage(added.clone(), StepPage::Sources));
  back(&mut editor);
  at(&editor, id, PipelineView::Step(added.clone()));
  editor.click("step-instructions-row");
  at(&editor, id, PipelineView::StepPage(added, StepPage::Instructions));
  assert!(editor.exists("step-instructions"));
}

fn viewing(editor: &Editor, id: ResourceId, view: PipelineView) {
  assert_eq!(
    editor.window.navigation().route_now(),
    Route::Project(
      project_id(),
      ProjectPage::PipelineVersion(id, VersionNumber::FIRST, view)
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
  viewing(&editor, id, PipelineView::Versions);
  assert!(editor.exists("pipeline-back-draft"), "a past version is viewed");
  editor.click("pipeline-step-step");
  viewing(&editor, id, step());
  assert!(editor.exists("pipeline-back-draft"), "the version's step is shown");

  back(&mut editor);
  viewing(&editor, id, PipelineView::Versions);
  assert!(editor.exists("pipeline-back-draft"));
  back(&mut editor);
  at(&editor, id, PipelineView::Versions);
  assert!(
    !editor.exists("pipeline-back-draft"),
    "Back before the pick shows the draft's Versions"
  );
  back(&mut editor);
  at(&editor, id, step());
  assert!(!editor.exists("pipeline-back-draft"));
  assert_eq!(editor.value("step-name"), "Checked", "the draft's edit shows again");
  editor.set("step-name", "Checked again");
  assert_eq!(editor.value("step-name"), "Checked again", "the step is editable");

  forward(&mut editor);
  at(&editor, id, PipelineView::Versions);
  assert!(!editor.exists("pipeline-back-draft"));
  forward(&mut editor);
  viewing(&editor, id, PipelineView::Versions);
  editor.until("the version read again", |e| e.exists("pipeline-back-draft"));
  editor.click("pipeline-back-draft");
  at(&editor, id, PipelineView::Steps);
  assert!(!editor.exists("pipeline-back-draft"));
}

#[test]
fn pipelines_removing_a_step_leaves_no_entry() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.go(ProjectPage::Pipelines);
  editor.open(id);
  editor.click("pipeline-step-step");
  editor.click("step-remove");
  editor.click("step-remove-confirm");
  at(&editor, id, PipelineView::Steps);
  assert!(!editor.exists("pipeline-step-step"));
  forward(&mut editor);
  at(&editor, id, PipelineView::Steps);
  back(&mut editor);
  assert!(
    editor.exists("pipeline-leave-question"),
    "Back past the removed step's place leaves the page and asks about the removal"
  );
}
