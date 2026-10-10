use orchester_core::{
  resource::{AgentProfile, Draft},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::harness::Agents;

#[test]
fn agents_loading_is_visible_before_the_read_finishes() {
  let mut screen = Agents::new(true);
  screen.go();
  screen.window.navigation().go(orchester_app::routes::Route::AllProjects);
  screen.window.pass();
  screen.window.navigation().go(orchester_app::routes::Route::Project(
    super::support::project_id(),
    orchester_app::routes::ProjectPage::Agents,
  ));
  screen.window.pass();
  assert!(screen.exists("agents-loading"));
  screen.until("loaded", |s| s.exists("agents-new"));
}

#[test]
fn agents_empty_with_and_without_connections() {
  for connection in [false, true] {
    let mut screen = Agents::new(connection);
    screen.go();
    assert!(screen.text("agents-empty").contains("No agent profiles yet"));
    assert_eq!(screen.exists("agents-open-connections"), !connection);
    assert_eq!(screen.exists("agents-empty-new"), connection);
  }
}
#[test]
fn agents_list_and_published_editor() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  assert!(screen.text("agent-save-status").contains("Published v1"));
  screen.click("agent-tab-permissions");
  assert!(screen.exists("agent-repository"));
  assert!(screen.exists("agent-enforcement"));
  screen.click("agent-tab-used");
  assert!(screen.text("agent-editor").contains("not used in a pipeline"));
}
#[test]
fn agents_draft_and_stale_draft_states() {
  let mut screen = Agents::new(true);
  let id = screen.profile(false);
  let mut db = ProjectDb::open(&screen.root).expect("db");
  let draft = db.draft::<AgentProfile>(id).expect("read").expect("draft");
  orchester_workspace::agents::data::publish(&mut db, id, Timestamp::now()).expect("publish");
  db.save_draft(
    id,
    &Draft::new(super::harness::changed(draft.body()), None, Timestamp::now()),
  )
  .expect("stale draft");
  screen.open(id);
  assert!(screen.text("agent-save-status").contains("Draft based on v0"));
  assert!(screen.exists("agent-keep-draft"));
  screen.click("agent-keep-draft");
  screen.until("question armed", |s| s.exists("agent-confirm-confirm"));
  screen.window.tree.tick_futures();
  screen.window.pass();
  screen.click("agent-confirm-confirm");
  screen.until("draft kept", |s| s.text("screen-agents").contains("Draft saved"));
}
#[test]
fn agents_failed_load_can_retry() {
  let mut screen = Agents::new(true);
  screen.profile(false);
  let path = screen.root.join(".orchester/orchester.db");
  let db = rusqlite::Connection::open(path).expect("db");
  let body: String = db
    .query_row("SELECT body FROM resource_drafts", [], |row| row.get(0))
    .expect("body");
  db.execute("UPDATE resource_drafts SET body = 'broken'", [])
    .expect("corrupt fixture");
  screen.window.navigation().go(orchester_app::routes::Route::Project(
    super::support::project_id(),
    orchester_app::routes::ProjectPage::Agents,
  ));
  screen.until("failure", |s| s.exists("agents-load-error"));
  assert!(!screen.exists("agents-empty"));
  db.execute("UPDATE resource_drafts SET body = ?1", [&body])
    .expect("restore fixture");
  screen.click("agents-load-error-retry");
  screen.until("retried", |s| s.exists("agents-new"));
}
