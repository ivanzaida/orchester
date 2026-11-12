//! The editor follows its fields and its writes once, by what it shows now, however often it rendered:
//! a save is written once, and a field the editor locked since it was first shown does not change the version a save
//! publishes (a pipeline holds no draft, D-032; nothing saves on its own, PV2-32).
use std::time::{Duration, Instant};

use orchester_core::{id::ResourceId, resource::StepKind};

use crate::{pipelines_support::Editor, support::project_id};

/// How many pipeline writes the window was told of.
fn writes(editor: &Editor) -> u64 {
  editor
    .window
    .projects()
    .feed(project_id())
    .outside
    .get_untracked()
    .pipelines
}

/// Delivers what finishes within a second and a half, so a second write would have shown.
fn rest(editor: &mut Editor) {
  let until = Instant::now() + Duration::from_millis(1500);
  while Instant::now() < until {
    editor.arm();
  }
}

#[test]
fn pipelines_watch_save_after_many_renders_is_written_once() {
  let mut editor = Editor::team();
  let id = editor.empty("Renders");
  editor.open(id);
  for text in ["One", "Two", "Three", "Four", "Five"] {
    editor.set("pipeline-description", text);
    editor.arm();
  }
  let before = writes(&editor);
  editor.click("pipeline-save");
  editor.until("saved", |e| !e.exists("pipeline-save"));
  rest(&mut editor);
  assert_eq!(writes(&editor) - before, 1, "one press of Save writes once");
  assert!(!editor.exists("pipeline-write-error"));
  assert_eq!(editor.stored(id).description(), "Five");
}

/// Where the edit is tried while the weaker save's question locks the editor.
#[derive(Clone, Copy)]
enum Page {
  Step,
  Summary,
  Settings,
}

/// Shows `page` unlocked and renders it a few times, asks the weaker save's question, then tries an edit
/// while it locks the page: confirming saves what was asked about, not the locked edit. Gives the editor,
/// the pipeline and the field it edited.
fn locked_edit_while_asked(page: Page) -> (Editor, ResourceId, &'static str) {
  let mut editor = Editor::team();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-step-step");
  editor.click("step-gate");
  let field = match page {
    Page::Step => "step-budget",
    Page::Summary => {
      editor.click("pipeline-start");
      "pipeline-description"
    }
    Page::Settings => {
      editor.click("pipeline-settings");
      "pipeline-settings-description"
    }
  };
  let asked = if matches!(page, Page::Step) { "4,000" } else { "Asked" };
  for text in ["1", "2", asked] {
    editor.set(field, text);
    editor.arm();
  }
  editor.click("pipeline-save");
  editor.until("save question", |e| e.exists("pipeline-save-question-confirm"));
  editor.set(field, if matches!(page, Page::Step) { "9,000" } else { "Locked" });
  editor.arm();
  assert!(editor.gated());
  editor.click("pipeline-save-question-confirm");
  assert!(
    !editor.exists("pipeline-save-question-confirm"),
    "the locked {field} did not change what the question asked about"
  );
  editor.until("weaker version saved", |e| e.version(id) == 2);
  let body = editor.stored(id);
  let step = &body.steps()[0];
  assert!(!step.approval_gate);
  assert!(matches!(step.kind, StepKind::Tool { .. }));
  match page {
    Page::Step => assert_eq!(step.budget.map(|b| b.get()), Some(4000)),
    Page::Summary | Page::Settings => assert_eq!(body.description(), "Asked"),
  }
  (editor, id, field)
}

/// The latest version's budget of its only step, or its description.
fn saved(editor: &Editor, id: ResourceId, page: Page) -> String {
  let body = editor.stored(id);
  match page {
    Page::Step => body.steps()[0].budget.map_or_else(String::new, |b| b.get().to_string()),
    Page::Summary | Page::Settings => body.description().to_owned(),
  }
}

#[test]
fn pipelines_watch_fields_follow_again_once_the_question_is_answered() {
  for page in [Page::Step, Page::Summary, Page::Settings] {
    let (mut editor, id, field) = locked_edit_while_asked(page);
    let (typed, stored) = if matches!(page, Page::Step) {
      ("3,000", "3000")
    } else {
      ("Unlocked", "Unlocked")
    };
    editor.set(field, typed);
    editor.until("the unlocked edit", |e| e.exists("pipeline-save"));
    // v2 dropped the gate already: the next edit weakens nothing, so it saves without the question.
    editor.click("pipeline-save");
    editor.until("the unlocked edit saved", |e| saved(e, id, page) == stored);
    assert!(!editor.exists("pipeline-save-question-confirm"));
  }
}

#[test]
fn pipelines_watch_locked_step_field_keeps_the_asked_version() {
  locked_edit_while_asked(Page::Step);
}
#[test]
fn pipelines_watch_locked_summary_field_keeps_the_asked_version() {
  locked_edit_while_asked(Page::Summary);
}
#[test]
fn pipelines_watch_locked_settings_field_keeps_the_asked_version() {
  locked_edit_while_asked(Page::Settings);
}
