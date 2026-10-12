use std::fs;

use orchester_core::{
  command::Command,
  integration::local::{CheckedResult, LocalIntegrationStore},
  store::MergeStore,
};
use orchester_exec::local_git::LocalGit;
use orchester_store::ProjectDb;

use super::{
  domain_support::{Fixture, git},
  integration_support::{Page, contains},
};

#[test]
fn local_integration_retained_inspector_clears_acknowledgement_for_same_main_new_checked_result() {
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("real Check");
  let mut operation = session.operation().clone();
  let old = operation.checked().expect("checked").clone();
  drop(session);
  let mut page = Page::new(fixture);
  page.ready("local-confirm-scope");
  page.click("local-confirm-scope");
  let check = orchester_app::ui::icon::Icon::Check.glyph().to_string();
  assert!(contains(
    page
      .window
      .tree
      .get_element_by_id("local-confirm-scope")
      .expect("scope"),
    &check
  ));
  unchanged_refresh_keeps_scope(&mut page, &check);
  fs::write(page.fixture.task_root.join("task.txt"), "different exact result\n").expect("private change");
  git(
    &page.fixture.task_root,
    &["commit", "-am", "replace private checked result"],
  );
  let task = LocalGit::new(
    &page.fixture.task_root,
    &page.fixture.task_root.join(".orchester/local-integration"),
  )
  .expect("task");
  let source = task.snapshot().expect("new source");
  assert_eq!(old.base.head, git(page.root(), &["rev-parse", "HEAD"]));
  operation
    .begin_sync(&old.base)
    .expect("replacement starts another Check");
  operation.own_task(source.clone());
  operation.begin_check().expect("check");
  let check_command =
    Command::new(std::path::Path::new("git"), ["status", "--porcelain"]).with_current_dir(&page.fixture.task_root);
  assert_eq!(
    task
      .check_command(&check_command, std::time::Duration::from_secs(5))
      .expect("real private Check"),
    Some(0)
  );
  operation
    .checked_result(CheckedResult {
      base: old.base.clone(),
      result: source,
    })
    .expect("different checked result");
  let expected = operation.version();
  operation.advance_version();
  assert!(
    ProjectDb::open(page.root())
      .expect("db")
      .save_local_operation(&operation, Some(expected))
      .expect("save new identity")
  );
  let project = ProjectDb::open(page.root()).expect("db").project().id();
  page.window.projects().feed(project).saved();
  page.until("same retained inspector clears old checkbox", |window| {
    window
      .tree
      .get_element_by_id("local-confirm-scope")
      .is_some_and(|element| !contains(element, &check))
  });
  assert!(page.has("local-confirm-scope"));
  assert!(
    ProjectDb::open(page.root())
      .expect("db")
      .run_merge(page.run())
      .expect("no merge")
      .is_none()
  );
}
fn unchanged_refresh_keeps_scope(page: &mut Page, check: &str) {
  let project = ProjectDb::open(page.root()).expect("db").project().id();
  page.window.projects().feed(project).saved();
  page.window.pass();
  assert!(
    !page.has("local-confirm-scope"),
    "the old projection is actually hidden before replacement"
  );
  page.until(
    "unchanged reviewed result keeps its exact acknowledgement through remount",
    |window| {
      window
        .tree
        .get_element_by_id("local-confirm-scope")
        .is_some_and(|element| contains(element, check))
    },
  );
}
