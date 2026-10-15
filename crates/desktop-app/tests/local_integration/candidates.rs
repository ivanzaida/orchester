use std::fs;

use orchester_app::routes::Route;

use super::{
  domain_support::{Fixture, git},
  integration_support::Page,
};

fn restoration() -> Page {
  let fixture = Fixture::new(false);
  fs::write(fixture.task_root.join("local.txt"), "task local change\n").expect("task");
  git(&fixture.task_root, &["commit", "-am", "second changed file"]);
  fs::write(fixture.root().join("task.txt"), "preserved task edit\n").expect("local");
  fs::write(fixture.root().join("local.txt"), "preserved local edit\n").expect("local");
  let mut session = fixture.open();
  session.preserve("Private restoration fixture").expect("preserve");
  session.sync().expect("real Check");
  let exact = session.operation().checked().expect("checked").clone();
  // Authorized private domain regression setup, never app MCP consent or a real human outcome.
  session.approve_and_merge(&exact).expect("private merge setup");
  session.restore().expect("two restoration conflicts");
  assert_eq!(session.restore_conflicts().expect("files").len(), 2);
  drop(session);
  let mut page = Page::new(fixture);
  page.ready("local-destination");
  page
}

#[test]
fn local_integration_partial_destination_only_keep_discard_file_switch_and_leave_are_guarded() {
  let mut page = restoration();
  let before = page.value("local-result-editor");
  page.set("local-destination", "../unfinished/");
  assert!(
    page.has("local-unsaved"),
    "raw invalid destination alone is a candidate"
  );
  page.choose("local-conflict-picker", "task.txt");
  page.window.navigation().go(Route::AllProjects);
  page.window.pass();
  assert_eq!(page.window.navigation().route_now(), page.route);
  page.click("local-keep-editing");
  assert_eq!(page.value("local-destination"), "../unfinished/");
  assert_eq!(page.value("local-result-editor"), before);
  page.click("local-discard-editor");
  assert_eq!(page.value("local-destination"), "");
  assert!(!page.has("local-unsaved"));
}

#[test]
fn local_integration_staging_only_candidate_keep_discard_and_pause_preserve_actual_choice() {
  let mut page = restoration();
  page.choose("local-staging-choice", "Stage");
  assert!(
    page.has("local-unsaved"),
    "staging alone blocks leave and file switching"
  );
  page.window.navigation().go(Route::AllProjects);
  page.window.pass();
  assert_eq!(page.window.navigation().route_now(), page.route);
  page.click("local-keep-editing");
  assert!(page.has("local-unsaved"));
  page.click("local-discard-editor");
  assert!(!page.has("local-unsaved"));
  page.choose("local-staging-choice", "Stage");
  page.click("local-save-pause");
  page.until("saved raw staging candidate", |window| {
    window.tree.get_element_by_id("local-unsaved").is_none()
  });
  let mut session = page.fixture.open();
  session.reconcile().expect("resume");
  let file = session.restore_conflicts().expect("still unresolved").remove(0);
  let expected = orchester_exec::local_git::file_hash(page.root(), &file.path).expect("hash");
  assert_eq!(
    session
      .editor_draft(&file, expected.as_ref())
      .expect("saved")
      .expect("candidate")
      .placement(),
    orchester_core::integration::local::Placement::Stage
  );
}

#[test]
fn local_integration_read_refusal_keeps_raw_candidate_and_leave_guard_until_its_source_returns() {
  let mut page = restoration();
  page.set("local-destination", "../still unfinished/");
  let name = refuse_metadata(&mut page);
  page.ready("local-read-error");
  assert!(page.has("local-unsaved"));
  assert!(page.has("local-save-pause"));
  page.click("local-keep-editing");
  page.window.navigation().go(Route::AllProjects);
  page.window.pass();
  assert_eq!(
    page.window.navigation().route_now(),
    page.route,
    "a read error cannot drop the editor's leave guard"
  );
  fs::remove_file(page.root().join(&name)).expect("remove only our private error fixture");
  let project = orchester_store::ProjectDb::open(page.root())
    .expect("db")
    .project()
    .id();
  page.window.projects().feed(project).saved();
  page.ready("local-destination");
  assert_eq!(page.value("local-destination"), "../still unfinished/");
  assert!(page.has("local-unsaved"));
}

#[test]
fn local_integration_pending_raw_save_locks_text_destination_and_staging_and_leaves_only_after_save() {
  let mut page = restoration();
  let text = page.value("local-result-editor");
  page.set("local-destination", "captured-result.bin");
  page.choose("local-staging-choice", "Stage");
  page.window.navigation().go(Route::AllProjects);
  page.window.pass();
  page.click("local-save-pause");
  page.window.tree.tick_futures();
  page.window.pass();
  assert!(page.has("local-working"), "the captured save is actually pending");
  page.type_into("local-destination", "must not replace captured");
  page.type_into("local-result-editor", "must not replace captured text");
  assert_eq!(
    page.value("local-destination"),
    "captured-result.bin",
    "actual pending input rejects edits"
  );
  assert_eq!(page.value("local-result-editor"), text);
  page.choose("local-staging-choice", "Leave unstaged");
  page.until("pending save completes before requested departure", |window| {
    window.navigation().route_now() == Route::AllProjects
  });
  let mut session = page.fixture.open();
  session.reconcile().expect("resume actual unresolved source");
  let file = session.restore_conflicts().expect("files").remove(0);
  let expected = orchester_exec::local_git::file_hash(page.root(), &file.path).expect("hash");
  let saved = session
    .editor_draft(&file, expected.as_ref())
    .expect("draft")
    .expect("retained");
  assert_eq!(saved.destination(), "captured-result.bin");
  assert_eq!(saved.text(), text);
  assert_eq!(saved.placement(), orchester_core::integration::local::Placement::Stage);
}
fn refuse_metadata(page: &mut Page) -> String {
  use orchester_core::{
    id::SecretId,
    secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
    store::SecretStore,
    time::Timestamp,
  };
  use orchester_store::ProjectDb;

  use super::integration_support::PRIVATE_VALUE;
  let name = format!("notes-{PRIVATE_VALUE}.txt");
  fs::write(page.root().join(&name), "private ordinary bytes").expect("private file");
  let mut db = ProjectDb::open(page.root()).expect("db");
  db.create_secret(&Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("PRIVATE_FIXTURE").expect("name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Development,
    },
    Timestamp::now(),
  ))
  .expect("private metadata");
  let project = db.project().id();
  page.window.projects().feed(project).saved();
  page.ready("local-read-error");
  name
}

#[test]
fn local_integration_read_refusal_can_save_raw_candidate_and_pause_without_exposing_metadata_or_merging_again() {
  let mut page = restoration();
  page.set("local-destination", "../raw unfinished/");
  page.choose("local-staging-choice", "Stage");
  let name = refuse_metadata(&mut page);
  let head = git(page.root(), &["rev-parse", "HEAD"]);
  page.window.navigation().go(Route::AllProjects);
  page.window.pass();
  page.click("local-save-pause");
  page.until(
    "refused projection still saves actual draft before departure",
    |window| window.navigation().route_now() == Route::AllProjects,
  );
  let mut session = page.fixture.open();
  session.reconcile().expect("owned restoration resume");
  let file = session.restore_conflicts().expect("still unresolved").remove(0);
  let expected = orchester_exec::local_git::file_hash(page.root(), &file.path).expect("hash");
  let draft = session
    .editor_draft(&file, expected.as_ref())
    .expect("draft")
    .expect("saved raw candidate");
  assert_eq!(draft.destination(), "../raw unfinished/");
  assert_eq!(draft.placement(), orchester_core::integration::local::Placement::Stage);
  assert_eq!(git(page.root(), &["rev-parse", "HEAD"]), head);
  assert_eq!(
    fs::read(page.root().join(name)).expect("unselected original"),
    b"private ordinary bytes"
  );
}
