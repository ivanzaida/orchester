//! Delays and failures use only this test's private project, through real input and persistence.
use lurq::app::events::MouseButton;
use orchester_app::root::Start;
use orchester_core::{
  name::Name,
  resource::{AgentTargets, InstructionScope, Pipeline, Slug},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::instructions::editor;

use super::{
  harness::{Instructions, body},
  preview::seed_preview,
};

fn type_value(screen: &mut Instructions, id: &str, value: &str) {
  screen.window.tree.get_element_by_id_mut(id).expect("input").focus();
  screen.key("a", "KeyA", false, true);
  for character in value.chars() {
    screen.key(&character.to_string(), "KeyX", false, false);
  }
}
fn hold(screen: &Instructions) -> orchester_app::project::hold::ReadHold {
  let Start::Ready(ready) = screen.window.launch.start() else {
    panic!("started fixture")
  };
  ready.services.hold
}
fn fail_saves(screen: &Instructions) -> rusqlite::Connection {
  let db = rusqlite::Connection::open(screen.root.join(".orchester/orchester.db")).expect("private db");
  db.execute_batch(
    "CREATE TRIGGER private_insert_failure BEFORE INSERT ON resource_drafts BEGIN SELECT RAISE(FAIL, 'private save failure'); END;
     CREATE TRIGGER private_update_failure BEFORE UPDATE ON resource_drafts BEGIN SELECT RAISE(FAIL, 'private save failure'); END;"
  ).expect("private triggers");
  db
}
fn allow_saves(db: &rusqlite::Connection) {
  db.execute_batch("DROP TRIGGER private_insert_failure; DROP TRIGGER private_update_failure;")
    .expect("remove fixture triggers");
}
fn rejected_save(screen: &mut Instructions) -> (orchester_core::id::ResourceId, rusqlite::Connection) {
  let id = screen.rule("Agreement", "Stored draft", InstructionScope::Project, false);
  screen.go();
  let db = fail_saves(screen);
  type_value(screen, "instruction-prompt", "Rejected edit");
  screen.until("rejected save", |s| s.exists("instruction-write-error"));
  assert!(screen.entry(id).state.published().is_none());
  (id, db)
}
#[test]
fn instructions_retry_after_failed_save_and_revert_never_publishes() {
  let mut screen = Instructions::new();
  let (id, db) = rejected_save(&mut screen);
  allow_saves(&db);
  type_value(&mut screen, "instruction-prompt", "Stored draft");
  assert_eq!(screen.text("instruction-prompt"), "Stored draft");
  assert!(screen.exists("instruction-write-error"));
  screen.click("instruction-retry");
  screen.until("same save retried", |s| !s.exists("instruction-write-error"));
  assert_eq!(screen.entry(id).body.text().as_str(), "Stored draft");
  assert!(screen.entry(id).state.published().is_none());
  assert!(screen.entry(id).state.has_draft());
}
#[test]
fn instructions_failed_reload_retry_remains_a_read_without_publication() {
  let mut screen = Instructions::new();
  let (id, db) = rejected_save(&mut screen);
  allow_saves(&db);
  type_value(&mut screen, "instruction-prompt", "Stored draft");
  let stored: String = db
    .query_row(
      "SELECT body FROM resource_drafts WHERE resource_id=?1",
      [id.to_string()],
      |row| row.get(0),
    )
    .expect("stored body");
  db.execute(
    "UPDATE resource_drafts SET body='broken' WHERE resource_id=?1",
    [id.to_string()],
  )
  .expect("private read failure");
  screen.click("instruction-reload");
  screen.until("reload rejected", |s| s.exists("instruction-write-error"));
  db.execute(
    "UPDATE resource_drafts SET body=?1 WHERE resource_id=?2",
    (&stored, id.to_string()),
  )
  .expect("restore private body");
  let before = screen.entry(id);
  screen.click("instruction-retry");
  screen.until("reload retried", |s| !s.exists("instruction-write-error"));
  assert_eq!(screen.entry(id), before, "Retry of Reload must perform no store write");
  assert!(screen.entry(id).state.published().is_none());
}
#[test]
fn instructions_delayed_reload_preserves_post_request_text_picker_targets_and_timing() {
  let mut screen = Instructions::new();
  seed_preview(&screen);
  let id = screen.rule("Agreement", "Original", InstructionScope::Project, true);
  screen.go();
  screen.until("picker loaded", |s| s.exists("instruction-variable-insert"));
  editor::save_in(
    &screen.root,
    &screen.entry(id),
    body("Outside", InstructionScope::Project),
  )
  .expect("outside candidate");
  type_value(&mut screen, "instruction-prompt", "Before reload");
  screen.until("outside conflict", |s| s.exists("instruction-write-error"));
  let hold = hold(&screen);
  hold.set(true);
  screen.click("instruction-reload");
  assert!(screen.text("instruction-save-status").contains("Saving"));
  type_value(&mut screen, "instruction-prompt", "After request ");
  screen.click("instruction-variable-insert");
  screen.choose("instruction-targets", "Builder");
  screen.choose("instruction-event", "Start & resume");
  let retained = screen.text("instruction-prompt");
  assert!(retained.contains("After request"));
  assert!(retained.contains("{{task.title}}"));
  hold.set(false);
  screen.until("post-request candidate saved", |s| {
    s.entry(id).body.text().as_str() == retained
      && matches!(s.entry(id).body.targets(), AgentTargets::Roles(_))
      && s.entry(id).body.events().len() == 2
      && !s.exists("instruction-write-error")
  });
  assert_eq!(screen.text("instruction-prompt"), retained);
  assert_eq!(screen.entry(id).state.published().expect("original version").get(), 1);
}
#[test]
fn instructions_delayed_first_create_locks_name_prompt_open_menu_picker_targets_and_timing() {
  let mut screen = Instructions::new();
  seed_preview(&screen);
  screen.go();
  screen.click("instructions-new");
  screen.until("new form", |s| s.exists("instruction-name"));
  screen.set("instruction-name", "Created agreement");
  screen.set("instruction-prompt", "Captured candidate");
  screen.until("picker", |s| s.exists("instruction-variable-insert"));
  // Open the timing menu before the pending operation so its retained options are exercised.
  let bounds = screen
    .window
    .tree
    .get_element_by_id_mut("instruction-event")
    .and_then(|e| e.bounds())
    .expect("event bounds");
  let (x, y) = (bounds.x + 5.0, bounds.y + 5.0);
  screen.window.tree.mouse_move(x, y);
  screen.window.tree.mouse_down(x, y, MouseButton::Left);
  screen.window.tree.mouse_up(x, y, MouseButton::Left);
  screen.window.pass();
  let db = rusqlite::Connection::open(screen.root.join(".orchester/orchester.db")).expect("private db");
  db.execute_batch("BEGIN IMMEDIATE").expect("delay private create");
  screen.click("instruction-create");
  assert!(screen.text("instruction-save-status").contains("Saving"));
  type_value(&mut screen, "instruction-name", "Later name");
  type_value(&mut screen, "instruction-prompt", "Later prompt");
  screen
    .window
    .tree
    .get_element_by_id_mut("instruction-targets")
    .and_then(|e| e.as_select())
    .expect("locked targets")
    .commit(1);
  screen
    .window
    .tree
    .get_element_by_id_mut("instruction-event")
    .and_then(|e| e.as_select())
    .expect("locked timing")
    .commit(1);
  screen.window.pass();
  screen.key("ArrowDown", "ArrowDown", false, false);
  screen.key("Enter", "Enter", false, false);
  screen.click("instruction-variable-insert");
  assert_eq!(screen.text("instruction-name"), "Created agreement");
  assert_eq!(screen.text("instruction-prompt"), "Captured candidate");
  db.execute_batch("COMMIT").expect("release private create");
  screen.until("created", |s| s.exists("instruction-publish"));
  let entry = &screen.listed().entries[0];
  assert_eq!(&*entry.name, "Created agreement");
  assert_eq!(entry.body.text().as_str(), "Captured candidate");
  assert_eq!(entry.body.targets(), &AgentTargets::All);
  assert_eq!(entry.body.events().len(), 1);
  assert!(entry.state.published().is_none());
}
#[test]
fn instructions_delayed_autosave_acknowledges_baseline_without_resetting_newer_input() {
  let mut screen = Instructions::new();
  let id = screen.rule("Agreement", "Published", InstructionScope::Project, true);
  screen.go();
  let db = rusqlite::Connection::open(screen.root.join(".orchester/orchester.db")).expect("private db");
  db.execute_batch("BEGIN IMMEDIATE").expect("delay private save");
  type_value(&mut screen, "instruction-prompt", "First candidate");
  screen.until("save pending", |s| s.text("instruction-save-status").contains("Saving"));
  type_value(&mut screen, "instruction-prompt", "Newer candidate");
  assert_eq!(screen.text("instruction-prompt"), "Newer candidate");
  db.execute_batch("COMMIT").expect("release private save");
  screen.until("newer saved", |s| {
    s.entry(id).body.text().as_str() == "Newer candidate"
      && s.text("instruction-save-status").contains("Draft v2 saved")
  });
  let db = ProjectDb::open(&screen.root).expect("private db");
  assert_eq!(
    db.latest_version::<orchester_core::resource::Instruction>(id)
      .expect("latest")
      .expect("published")
      .body()
      .text()
      .as_str(),
    "Published"
  );
}

#[test]
fn instructions_delayed_step_create_keeps_selected_pipeline_and_step_locked() {
  let mut screen = Instructions::new();
  let pipeline = seed_preview(&screen);
  let mut store = ProjectDb::open(&screen.root).expect("private db");
  let version = store
    .latest_version::<Pipeline>(pipeline)
    .expect("version")
    .expect("pipeline");
  let other = store
    .create_resource(
      Slug::new("other").expect("slug"),
      Name::new("Other pipeline").expect("name"),
      version.body(),
      Timestamp::now(),
    )
    .expect("second private pipeline")
    .id();
  store
    .publish_draft::<Pipeline>(other, Timestamp::now())
    .expect("published fixture");
  screen.go();
  screen.click("instruction-tab-3");
  screen.click("instructions-new");
  screen.until("step form", |s| s.exists("instruction-step"));
  screen.set("instruction-name", "Step agreement");
  screen.choose("instruction-pipeline", "Feature delivery");
  screen.choose("instruction-step", "Implement");
  screen.set("instruction-prompt", "Step candidate");
  let db = rusqlite::Connection::open(screen.root.join(".orchester/orchester.db")).expect("private db");
  db.execute_batch("BEGIN IMMEDIATE").expect("delay private create");
  screen.click("instruction-create");
  assert!(screen.text("instruction-save-status").contains("Saving"));
  screen
    .window
    .tree
    .get_element_by_id_mut("instruction-pipeline")
    .and_then(|e| e.as_select())
    .expect("locked pipeline")
    .commit(1);
  screen
    .window
    .tree
    .get_element_by_id_mut("instruction-step")
    .and_then(|e| e.as_select())
    .expect("locked step")
    .commit(0);
  screen.window.pass();
  db.execute_batch("COMMIT").expect("release private create");
  screen.until("step created", |s| s.exists("instruction-publish"));
  let entry = screen.listed().entries[0].clone();
  assert!(matches!(entry.body.scope(), InstructionScope::Step { pipeline: selected, .. } if *selected == pipeline));
  assert!(entry.state.published().is_none());
}

#[test]
fn instructions_delayed_reload_discards_pre_request_text_but_keeps_later_timing() {
  let mut screen = Instructions::new();
  let id = screen.rule("Agreement", "Original", InstructionScope::Project, true);
  screen.go();
  editor::save_in(
    &screen.root,
    &screen.entry(id),
    body("Outside", InstructionScope::Project),
  )
  .expect("outside");
  type_value(&mut screen, "instruction-prompt", "Discard this before-request text");
  screen.until("conflict", |s| s.exists("instruction-write-error"));
  let hold = hold(&screen);
  hold.set(true);
  screen.click("instruction-reload");
  screen.choose("instruction-event", "Agent resume");
  hold.set(false);
  screen.until("merged reload", |s| {
    s.entry(id).body.text().as_str() == "Outside"
      && s
        .entry(id)
        .body
        .events()
        .contains(&orchester_core::resource::InjectionEvent::Resume)
      && !s.exists("instruction-write-error")
  });
  assert_eq!(screen.text("instruction-prompt"), "Outside");
  assert_eq!(screen.entry(id).state.published().expect("original version").get(), 1);
}
#[test]
fn instructions_delayed_reload_keeps_invalid_post_request_raw_text_without_false_save() {
  let mut screen = Instructions::new();
  let id = screen.rule("Agreement", "Original", InstructionScope::Project, true);
  screen.go();
  editor::save_in(
    &screen.root,
    &screen.entry(id),
    body("Outside", InstructionScope::Project),
  )
  .expect("outside");
  type_value(&mut screen, "instruction-prompt", "Discard before");
  screen.until("conflict", |s| s.exists("instruction-write-error"));
  let hold = hold(&screen);
  hold.set(true);
  screen.click("instruction-reload");
  type_value(&mut screen, "instruction-prompt", "Invalid after {{secret.value}}");
  hold.set(false);
  screen.until("read complete", |s| !s.exists("instruction-write-error"));
  screen.pump();
  assert_eq!(screen.text("instruction-prompt"), "Invalid after {{secret.value}}");
  assert!(screen.exists("instruction-text-error"));
  assert_eq!(screen.entry(id).body.text().as_str(), "Outside");
  assert_eq!(screen.entry(id).state.published().expect("original version").get(), 1);
}

#[test]
fn instructions_create_completion_keeps_new_rule_identity_while_catalog_refresh_waits() {
  let mut screen = Instructions::new();
  let old = screen.rule("Existing agreement", "Existing text", InstructionScope::Project, true);
  screen.go();
  screen.click("instructions-new");
  screen.set("instruction-name", "New agreement");
  screen.set("instruction-prompt", "Created text");
  let hold = hold(&screen);
  hold.set(true);
  screen.click("instruction-create");
  screen.until("new resource committed", |s| {
    s.listed().entries.len() == 2 && s.exists("instruction-publish")
  });
  assert_eq!(screen.text("instruction-prompt"), "Created text");
  screen.click("instruction-publish");
  screen.until("new resource published", |s| {
    s.listed()
      .entries
      .iter()
      .any(|e| &*e.name == "New agreement" && e.state.published().is_some())
  });
  assert_eq!(screen.entry(old).state.published().expect("old version").get(), 1);
  assert_eq!(screen.entry(old).body.text().as_str(), "Existing text");
  hold.set(false);
}
