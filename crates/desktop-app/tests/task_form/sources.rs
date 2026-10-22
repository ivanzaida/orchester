//! Context › Sources (`zcFQJ`): what a task gets from the project and its pipeline, read-only; a new task keeps the
//! sources a person confirmed in place until Create stores them with it; a stored task's are saved as they change.

use std::path::Path;

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::source::SourcePath;

use crate::harness::Form;

const PATH: &str = "task-form-sources-add-path";

fn source(path: &Path) -> SourcePath {
  SourcePath::new(&path.to_string_lossy()).expect("an absolute path")
}

/// A spec outside the project.
fn spec() -> (tempfile::TempDir, std::path::PathBuf) {
  let folder = tempfile::tempdir().expect("a folder");
  let spec = folder.path().join("delivery-log.md");
  std::fs::write(&spec, "# Delivery log\n").expect("a file");
  (folder, spec)
}

/// Chooses the published pipeline whose label starts with `name`.
fn choose(form: &mut Form, name: &str) {
  let mut select = form
    .window
    .tree
    .get_element_by_id_mut("task-form-pipeline-input")
    .and_then(|element| element.as_select())
    .expect("the pipeline select");
  let index = select
    .labels()
    .iter()
    .position(|option| option.starts_with(name))
    .unwrap_or_else(|| panic!("an option for {name}"));
  select.commit(index);
  form.window.pass();
}

/// Adds `path` through the in-place question, as a person does, once the section has read what the task inherits.
fn add(form: &mut Form, path: &Path) {
  form.window.until("the sources read", |window| {
    window.tree.get_element_by_id(PATH).is_some()
  });
  form.set(PATH, &path.to_string_lossy());
  form.window.until("the check", |window| {
    window
      .texts("task-form-sources-add-hint-text")
      .is_some_and(|hint| hint.join("") == "File · readable.")
  });
  form.click("task-form-sources-add-add");
  form.window.until("the question", |window| {
    window
      .tree
      .get_element_by_id("task-form-sources-question-confirm")
      .is_some()
  });
  assert!(form.window.link.input_gate().protecting());
  // The question must live through a future pass before a person's press is taken.
  form.window.tree.tick_futures();
  form.window.pass();
  form.click("task-form-sources-question-confirm");
}

#[test]
fn form_sources_a_new_task_keeps_them_until_create_stores_them() {
  let mut form = Form::seeded("sources");
  form.new_task();
  form.window.until("the project's sources", |window| {
    window.tree.get_element_by_id("task-form-project-source-2").is_some()
  });
  assert_eq!(form.text("task-form-sources-project-count"), "3");
  assert!(form.exists("task-form-project-source-0-lock"));
  assert!(!form.exists("task-form-project-source-0-remove"));
  assert_eq!(form.text("task-form-project-source-2-status"), "Missing");
  assert!(!form.exists("task-form-sources-pipeline"));
  choose(&mut form, "Feature delivery");
  form.window.until("the pipeline's sources", |window| {
    window.tree.get_element_by_id("task-form-pipeline-source-1").is_some()
  });
  assert!(
    form
      .text("task-form-pipeline-source-1-meta")
      .contains("Review in Feature delivery")
  );
  let (_folder, spec) = spec();
  add(&mut form, &spec);
  form.window.until("the task's own source", |window| {
    window.tree.get_element_by_id("task-form-source-0").is_some()
  });
  assert!(!form.window.link.input_gate().protecting());
  assert_eq!(form.text("task-form-source-0-meta"), "File · this task");
  assert!(form.text("task-form-sources-summary").contains("starts with 6 sources"));
  form.set("task-form-title-input", "Add a delivery log");
  form.click("task-form-create");
  form.window.until("Tasks opens", |window| {
    matches!(
      window.navigation().route_now(),
      Route::Project(_, ProjectPage::Tasks(_))
    )
  });
  let created = form.task("KON-2").expect("Create stored the task");
  assert_eq!(form.db().task_sources(created.id()).expect("read"), [source(&spec)]);
}

#[test]
fn form_sources_create_and_run_stores_them_before_the_run_is_asked_to_start() {
  let mut form = Form::seeded("sources");
  form.new_task();
  let (_folder, spec) = spec();
  add(&mut form, &spec);
  form.window.until("the task's own source", |window| {
    window.tree.get_element_by_id("task-form-source-0").is_some()
  });
  form.set("task-form-title-input", "Add a delivery log");
  form.set("task-form-brief-input", "Record every delivery attempt.");
  form.click("task-form-add-criterion");
  form.set("task-form-criterion-1-input", "Every attempt is stored.");
  choose(&mut form, "Quick fix");
  form.click("task-form-run");
  form.window.until("the run's refusal", |window| {
    window
      .texts("task-form-status-label")
      .is_some_and(|status| status.join("") == "Queued · run not started")
  });
  let queued = form.task("KON-2").expect("the task is stored");
  assert_eq!(form.db().task_sources(queued.id()).expect("read"), [source(&spec)]);
}

#[test]
fn form_sources_retry_run_stores_the_ones_create_and_run_could_not_before_it_asks_again() {
  let mut form = Form::seeded("sources");
  form.new_task();
  let (_folder, spec) = spec();
  add(&mut form, &spec);
  form.window.until("the task's own source", |window| {
    window.tree.get_element_by_id("task-form-source-0").is_some()
  });
  let database = rusqlite::Connection::open(form.repo.join(".orchester").join("orchester.db")).expect("opens");
  database
    .execute_batch(
      "CREATE TRIGGER refuse_sources BEFORE INSERT ON task_sources BEGIN SELECT RAISE(FAIL, 'refused'); END",
    )
    .expect("the store refuses task sources");
  form.set("task-form-title-input", "Add a delivery log");
  form.set("task-form-brief-input", "Record every delivery attempt.");
  form.click("task-form-add-criterion");
  form.set("task-form-criterion-1-input", "Every attempt is stored.");
  choose(&mut form, "Quick fix");
  form.click("task-form-run");
  form.window.until("queued without its sources", |window| {
    window
      .texts("task-form-status-label")
      .is_some_and(|status| status.join("") == "Queued · run not started")
  });
  let queued = form.task("KON-2").expect("the task is stored");
  assert!(form.db().task_sources(queued.id()).expect("read").is_empty());
  database
    .execute_batch("DROP TRIGGER refuse_sources")
    .expect("the store takes them again");
  form.click("task-form-retry");
  let repo = form.repo.clone();
  form.window.until("stored before the run is asked for", |_| {
    orchester_store::ProjectDb::open(&repo)
      .and_then(|db| db.task_sources(queued.id()))
      .is_ok_and(|stored| stored == [source(&spec)])
  });
}

#[test]
fn form_sources_a_stored_task_saves_them_as_they_change() {
  let mut form = Form::seeded("sources");
  let task = form.task("KON-1").expect("the seeded draft").id();
  form.edit("KON-1");
  form.window.until("the task's own source", |window| {
    window.tree.get_element_by_id("task-form-source-0").is_some()
  });
  assert_eq!(form.text("task-form-source-0-meta"), "File · this task");
  form.click("task-form-source-0-remove");
  form.window.until("removed", |window| {
    window.tree.get_element_by_id("task-form-source-0").is_none()
  });
  assert!(form.db().task_sources(task).expect("read").is_empty());
  let (_folder, spec) = spec();
  add(&mut form, &spec);
  form.window.until("stored", |window| {
    window.tree.get_element_by_id("task-form-source-0").is_some()
  });
  assert_eq!(form.db().task_sources(task).expect("read"), [source(&spec)]);
  assert_eq!(form.status(), "Draft saved locally", "sources are no unsaved change");
}

/// Lists a relative path among the project's sources, which no read takes: the store calls it corrupt.
fn unreadable(form: &Form, broken: bool) {
  let sql = if broken {
    "INSERT INTO project_sources (position, path) VALUES (99, 'docs')"
  } else {
    "DELETE FROM project_sources WHERE path = 'docs'"
  };
  let changed = rusqlite::Connection::open(form.repo.join(".orchester").join("orchester.db"))
    .expect("the project database opens")
    .execute(sql, [])
    .expect("the sources change");
  assert_eq!(changed, 1);
}

#[test]
fn form_sources_a_failed_read_says_why_with_try_again_never_the_list_read_before() {
  let mut form = Form::seeded("sources");
  let task = form.task("KON-1").expect("the seeded draft").id();
  form.edit("KON-1");
  form.window.until("the task's own source", |window| {
    window.tree.get_element_by_id("task-form-source-0").is_some()
  });
  unreadable(&form, true);
  // Removing one stores it, then reads the sources again, which fails now.
  form.click("task-form-source-0-remove");
  form.window.until("the failure", |window| {
    window.tree.get_element_by_id("task-form-sources-load-error").is_some()
  });
  assert!(form.db().task_sources(task).expect("read").is_empty());
  assert!(
    form
      .text("task-form-sources-load-error")
      .starts_with("The sources couldn't be read")
  );
  assert!(!form.exists("task-form-source-0"), "the list read before is gone");
  assert!(!form.exists(PATH), "nothing is added to a list not read");
  assert!(!form.exists("task-form-sources-summary"));
  unreadable(&form, false);
  form.click("task-form-sources-load-error-retry");
  form
    .window
    .until("read again", |window| window.tree.get_element_by_id(PATH).is_some());
  assert!(!form.exists("task-form-sources-load-error"));
  assert!(!form.exists("task-form-source-0"));
  assert_eq!(form.text("task-form-sources-project-count"), "3");
}
