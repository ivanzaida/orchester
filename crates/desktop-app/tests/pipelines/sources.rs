//! A pipeline draft's sources: Pipeline settings › Sources (`Q10UQ`) beside the project's, *Add folder* checked in
//! place, and publishing a version that reads more asked in place; the step inspector's own sources (`O6piA3`).
use std::path::Path;

use orchester_core::{resource::Pipeline, source::SourcePath, store::ResourceStore};

use crate::pipelines_support::Editor;

fn source(path: &Path) -> SourcePath {
  SourcePath::new(&path.to_string_lossy()).expect("an absolute path")
}

fn folder(files: usize) -> tempfile::TempDir {
  let folder = tempfile::tempdir().expect("a folder");
  for file in 0..files {
    std::fs::write(folder.path().join(format!("{file}.md")), "docs").expect("a file");
  }
  folder
}

fn drafted(editor: &Editor, id: orchester_core::id::ResourceId) -> Option<Pipeline> {
  editor
    .db()
    .draft::<Pipeline>(id)
    .expect("the draft reads")
    .map(|draft| draft.body().clone())
}

#[test]
fn pipelines_sources_a_chosen_folder_joins_the_draft_and_publishing_it_asks() {
  let (mut editor, stand_ins) = Editor::with_stand_ins();
  let id = editor.published();
  let (docs, tokens) = (folder(1), folder(2));
  editor
    .db()
    .set_project_sources(&[source(docs.path())])
    .expect("the project's");
  editor.open(id);
  editor.click("pipeline-summary-settings");
  editor.until("the project's sources", |e| e.exists("pipeline-project-source-0"));
  assert_eq!(editor.text("pipeline-project-source-0-hint"), "Project");
  std::fs::create_dir_all(editor.repo().join("docs")).expect("a project folder");
  stand_ins.choose(Some(editor.repo().join("docs")));
  editor.click("pipeline-sources-add-add");
  editor.until("why it is not added", |e| e.exists("pipeline-sources-add-hint"));
  assert!(
    editor
      .text("pipeline-sources-add-hint-text")
      .contains("agents already read it in the run's worktree")
  );
  stand_ins.choose(Some(tokens.path().to_path_buf()));
  editor.click("pipeline-sources-add-add");
  editor.until("the draft lists it", |e| {
    drafted(e, id).is_some_and(|body| body.sources().pipeline() == [source(tokens.path())])
  });
  assert!(editor.exists("pipeline-source-0"), "it can be removed");
  assert!(!editor.exists("pipeline-sources-add-hint"));
  editor.click("pipeline-tab-steps");
  editor.click("pipeline-publish");
  editor.until("publish question", |e| e.exists("pipeline-publish-question-confirm"));
  editor.arm();
  assert!(editor.gated(), "reading more is a person's to publish");
  assert!(
    editor
      .text("pipeline-publish-question")
      .contains("The sources the pipeline or its steps list differ")
  );
  editor.click("pipeline-publish-question-confirm");
  editor.until("v2", |e| {
    e.db().latest_version::<Pipeline>(id).unwrap().unwrap().number().get() == 2
  });
  let published = editor.db().latest_version::<Pipeline>(id).unwrap().unwrap();
  assert_eq!(published.body().sources().pipeline(), [source(tokens.path())]);
}

#[test]
fn pipelines_sources_a_step_lists_its_own_beside_what_it_inherits() {
  let mut editor = Editor::new();
  let id = editor.empty("Review");
  let (docs, guide) = (folder(1), folder(1));
  editor
    .db()
    .set_project_sources(&[source(docs.path())])
    .expect("the project's");
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.click("step-open-sources");
  editor.until("the step's sources", |e| e.exists("step-sources-add-path"));
  editor.until("what it inherits", |e| e.exists("step-sources-project-0"));
  assert_eq!(editor.text("step-sources-title-meta"), "0 own · 1 inherited");
  editor.set("step-sources-add-path", &guide.path().to_string_lossy());
  editor.until("the check", |e| {
    e.exists("step-sources-add-hint-text") && e.text("step-sources-add-hint-text") == "Folder · 1 file · readable."
  });
  editor.click("step-sources-add-add");
  editor.until("listed", |e| e.exists("step-source-0"));
  assert_eq!(editor.text("step-source-0-hint"), "This step");
  assert_eq!(editor.text("step-sources-title-meta"), "1 own · 1 inherited");
  // The draft saves itself once the edits rest.
  let listed = |body: &Pipeline| -> Vec<SourcePath> {
    body
      .steps()
      .iter()
      .flat_map(|step| body.sources().step(&step.key).to_vec())
      .collect()
  };
  editor.until("the draft lists it", |e| {
    drafted(e, id).is_some_and(|body| listed(&body) == [source(guide.path())])
  });
  editor.click("step-source-0-remove");
  editor.until("taken off the draft", |e| {
    drafted(e, id).is_some_and(|body| !body.steps().is_empty() && listed(&body).is_empty())
  });
}

/// Lists a relative path among the project's sources, which no read takes: the store calls it corrupt.
fn unreadable(editor: &Editor, broken: bool) {
  let sql = if broken {
    "INSERT INTO project_sources (position, path) VALUES (99, 'docs')"
  } else {
    "DELETE FROM project_sources WHERE path = 'docs'"
  };
  let changed = rusqlite::Connection::open(editor.repo().join(".orchester").join("orchester.db"))
    .expect("the project database opens")
    .execute(sql, [])
    .expect("the sources change");
  assert_eq!(changed, 1);
}

#[test]
fn pipelines_sources_a_failed_read_of_the_projects_says_why_with_try_again_never_an_empty_list() {
  let mut editor = Editor::new();
  let id = editor.empty("Review");
  let docs = folder(1);
  editor
    .db()
    .set_project_sources(&[source(docs.path())])
    .expect("the project's");
  unreadable(&editor, true);
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.click("step-open-sources");
  editor.until("the failure", |e| e.exists("step-sources-load-error"));
  assert!(
    editor
      .text("step-sources-load-error")
      .starts_with("The project's sources couldn't be read")
  );
  assert!(
    !editor.exists("step-sources-add-path"),
    "nothing is checked against a list not read"
  );
  assert!(
    !editor.exists("step-sources-title-meta"),
    "what it inherits is not counted"
  );
  assert!(!editor.exists("step-sources-project"));
  unreadable(&editor, false);
  editor.click("step-sources-load-error-retry");
  editor.until("read again", |e| {
    e.exists("step-sources-add-path") && e.exists("step-sources-title-meta")
  });
  assert_eq!(editor.text("step-sources-title-meta"), "0 own · 1 inherited");
  assert!(!editor.exists("step-sources-load-error"));
}
