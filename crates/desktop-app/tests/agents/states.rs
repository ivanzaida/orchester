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
  assert!(screen.text("agent-draft-bar-status").contains("Published v1"));
  // One page since ORC-107: the role, connection, permissions, enforcement and uses are all on it.
  assert!(screen.exists("agent-repository"));
  assert!(screen.exists("agent-enforcement"));
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
  assert!(screen.text("agent-draft-bar-status").contains("Draft based on v0"));
  assert!(!screen.exists("agent-keep-draft"));
  screen.click("agent-publish");
  screen.until("question armed", |s| s.exists("agent-confirm-confirm"));
  assert!(screen.window.link.input_gate().protecting());
  lurq::app::events::with_synthetic_input(|| screen.click("agent-confirm-keep"));
  assert!(
    screen.exists("agent-confirm-confirm"),
    "synthetic Keep cannot answer a stale-base decision"
  );
  screen.click("agent-confirm-keep");
  assert!(screen.listed().profile(id).expect("profile").state.has_draft());
  screen.click("agent-publish");
  screen.until("question armed again", |s| s.exists("agent-confirm-confirm"));
  screen.window.tree.tick_futures();
  screen.window.pass();
  screen.click("agent-confirm-confirm");
  screen.until("published", |s| {
    s.exists("agent-draft-bar-status") && s.text("agent-draft-bar-status").contains("Published v2")
  });
  assert_eq!(
    db.latest_version::<AgentProfile>(id).expect("read").expect("v2").body(),
    &super::harness::changed(draft.body())
  );
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
#[test]
fn agents_profile_form_fits_the_initial_viewport_and_published_card_has_no_draft() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  let mut bounds = |id: &str| {
    screen
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} laid out"))
  };
  let label = bounds("agent-role-label");
  let name = bounds("agent-role-box");
  let instructions = bounds("agent-instructions-box");
  assert!(label.width > 60.0, "name label has room: {label:?}");
  // Under the hub's head and tabs, the page's crumb, head and draft bar (01b).
  assert!(name.y < 420.0, "name starts near the top: {name:?}");
  assert!(
    instructions.y + instructions.height < 872.0,
    "instructions fit above the status bar: {instructions:?}"
  );
  let bar = screen.text("agent-draft-bar");
  assert!(bar.contains("Published v1"), "{bar}");
  assert!(!bar.contains("Draft v2"), "{bar}");
}

#[test]
fn agents_failed_refetch_replaces_retained_data_and_retry_restores_it() {
  let mut screen = Agents::new(true);
  let id = screen.profile(false);
  screen.go();
  assert!(screen.exists(&format!("agent-{id}")));
  let db = rusqlite::Connection::open(screen.root.join(".orchester/orchester.db")).expect("fixture db");
  let body: String = db
    .query_row("SELECT body FROM resource_drafts", [], |row| row.get(0))
    .expect("body");
  db.execute("UPDATE resource_drafts SET body = 'broken'", [])
    .expect("corrupt disposable fixture");
  let feed = screen.window.projects().feed(super::support::project_id());
  feed.outside.update(|counts| counts.profiles += 1);
  screen.until("failed refetch", |s| s.exists("agents-load-error"));
  assert!(!screen.exists(&format!("agent-{id}")), "stale profile is hidden");
  assert!(!screen.exists("agents-new"));
  assert!(screen.text("agents-load-error").contains("could not"));
  db.execute("UPDATE resource_drafts SET body = ?1", [&body])
    .expect("restore fixture");
  screen.click("agents-load-error-retry");
  screen.until("retry restored profiles", |s| s.exists(&format!("agent-{id}")));
  assert!(!screen.exists("agents-load-error"));
}

#[test]
fn agents_failed_refetch_and_retry_preserve_unsaved_editor_fields() {
  let mut screen = Agents::new(true);
  let id = screen.profile(false);
  screen.open(id);
  let db = rusqlite::Connection::open(screen.root.join(".orchester/orchester.db")).expect("fixture db");
  let body: String = db
    .query_row("SELECT body FROM resource_drafts", [], |row| row.get(0))
    .expect("body");
  db.execute("UPDATE resource_drafts SET body = 'broken'", [])
    .expect("corrupt disposable fixture");
  screen
    .window
    .projects()
    .feed(super::support::project_id())
    .outside
    .update(|counts| counts.profiles += 1);
  screen.until("failed editor refetch", |s| s.exists("agents-load-error"));
  // Edits made while the profiles cannot be read wait: nothing is saved over what the page could not read.
  screen.set("agent-responsibility", "My unsaved responsibility");
  screen.set("agent-instructions", "My unsaved instructions");
  assert!(screen.exists("agent-repository"), "the page stays mounted");
  screen.click("agent-publish");
  assert!(!screen.exists("agent-saving"), "publishing retained data is disabled");
  db.execute("UPDATE resource_drafts SET body = ?1", [&body])
    .expect("restore fixture");
  screen.click("agents-load-error-retry");
  screen.until("editor read recovered", |s| !s.exists("agents-load-error"));
  for (id, expected) in [
    ("agent-responsibility", "My unsaved responsibility"),
    ("agent-instructions", "My unsaved instructions"),
  ] {
    assert_eq!(screen.value(id), expected);
  }
  screen.until("preserved fields saved on their own", |s| {
    let profile = s.listed().profile(id).expect("profile").profile.clone();
    profile.responsibilities() == "My unsaved responsibility"
      && profile.instructions().as_str() == "My unsaved instructions"
  });
}
