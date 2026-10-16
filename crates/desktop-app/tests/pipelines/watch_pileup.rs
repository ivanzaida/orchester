//! The editor follows its fields and its writes once, by what it shows now, however often it rendered:
//! a save is written once, a rest of the fields autosaves once, and a field the editor locked since it
//! was first shown does not change the draft.
use std::time::{Duration, Instant};

use orchester_core::{
  resource::{Pipeline, StepKind},
  store::ResourceStore,
};

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

/// Delivers what finishes for longer than autosave's rest of the fields.
fn rest(editor: &mut Editor) {
  let until = Instant::now() + Duration::from_millis(1500);
  while Instant::now() < until {
    editor.arm();
  }
}

#[test]
fn pipelines_watch_save_after_many_renders_is_written_once() {
  let mut editor = Editor::new();
  let id = editor.empty("Renders");
  editor.open(id);
  for text in ["One", "Two", "Three", "Four", "Five"] {
    editor.set("pipeline-description", text);
    editor.arm();
  }
  let before = writes(&editor);
  editor.click("pipeline-save");
  editor.until("saved draft", |e| !e.exists("pipeline-save"));
  rest(&mut editor);
  assert_eq!(writes(&editor) - before, 1, "one press of Save writes once");
  assert!(!editor.exists("pipeline-write-error"));
  assert_eq!(
    editor.db().draft::<Pipeline>(id).unwrap().unwrap().body().description(),
    "Five"
  );
}

#[test]
fn pipelines_watch_autosave_writes_once_per_rest() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  let before = writes(&editor);
  for round in 1..=2u64 {
    for text in ["First", "Second", "Third"] {
      editor.set("pipeline-description", &format!("{text} {round}"));
      editor.arm();
    }
    let saved = format!("Third {round}");
    editor.until("automatic draft save", |e| {
      e.db()
        .draft::<Pipeline>(id)
        .unwrap()
        .is_some_and(|d| d.body().description() == saved)
    });
    rest(&mut editor);
    assert_eq!(writes(&editor) - before, round, "rest {round} writes once");
    assert!(!editor.exists("pipeline-write-error"));
  }
}

/// Where the edit is tried while the weaker save's question locks the editor.
#[derive(Clone, Copy)]
enum Page {
  Step,
  Summary,
  Settings,
}

/// Shows `page` unlocked and renders it a few times, asks the weaker save's question, then tries an edit
/// while it locks the page: confirming saves what was asked about, not the locked edit.
fn locked_edit_while_asked(page: Page) {
  let mut editor = Editor::new();
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
  editor.until("weaker draft saved", |e| {
    e.db().draft::<Pipeline>(id).expect("the draft can be read").is_some()
  });
  let draft = editor
    .db()
    .draft::<Pipeline>(id)
    .expect("the draft can be read")
    .expect("the weaker draft was saved");
  let step = &draft.body().steps()[0];
  assert!(!step.approval_gate);
  assert!(matches!(step.kind, StepKind::Tool { .. }));
  match page {
    Page::Step => assert_eq!(step.budget.map(|b| b.get()), Some(4000)),
    Page::Summary | Page::Settings => assert_eq!(draft.body().description(), "Asked"),
  }
}

#[test]
fn pipelines_watch_locked_step_field_keeps_the_asked_draft() {
  locked_edit_while_asked(Page::Step);
}
#[test]
fn pipelines_watch_locked_summary_field_keeps_the_asked_draft() {
  locked_edit_while_asked(Page::Summary);
}
#[test]
fn pipelines_watch_locked_settings_field_keeps_the_asked_draft() {
  locked_edit_while_asked(Page::Settings);
}
