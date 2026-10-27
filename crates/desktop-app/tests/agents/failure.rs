//! The Profiles tab when its read or a save fails (`Failure states · redesign` `SsC16`): the card replaces only the
//! tab's content, says what couldn't be read and why, tries again in place, shows the error and the tries in its
//! Details, says it still can't be read after three tries, and the tab shows no count meanwhile; a save that fails
//! keeps the typed value in its field and says so in the header.
use super::{harness::Agents, support::project_id};

/// Breaks the profiles' bodies so they can't be read; the body to put back.
fn break_profiles(screen: &Agents) -> (rusqlite::Connection, String) {
  let db = rusqlite::Connection::open(screen.root.join(".orchester/orchester.db")).expect("fixture db");
  let body: String = db
    .query_row("SELECT body FROM resource_drafts", [], |row| row.get(0))
    .expect("body");
  db.execute("UPDATE resource_drafts SET body = 'broken'", [])
    .expect("corrupt disposable fixture");
  (db, body)
}

fn failed_list(screen: &mut Agents) {
  screen.window.navigation().go(orchester_app::routes::Route::Project(
    project_id(),
    orchester_app::routes::ProjectPage::Agents,
  ));
  // A list already shown reads again once the profiles change.
  screen
    .window
    .projects()
    .feed(project_id())
    .outside
    .update(|counts| counts.profiles += 1);
  screen.until("the failure card", |s| s.exists("agents-load-error"));
}

/// Presses Try again and waits until the read under way ends again.
fn try_again(screen: &mut Agents, tries: &str) {
  screen.click("agents-load-error-retry");
  screen.until(tries, |s| s.exists("agents-load-error-tries") && s.text("agents-load-error-tries").contains(tries));
}

#[test]
fn failure_page_says_what_and_why_hides_the_count_and_tries_again_in_place() {
  let mut screen = Agents::new(true);
  screen.profile(false);
  screen.go();
  screen.until("counted", |s| s.text("agents-hub-tab-profiles").contains('1'));
  let (db, body) = break_profiles(&screen);
  failed_list(&mut screen);
  let card = screen.text("agents-load-error");
  assert!(card.contains("Agent profiles couldn’t be read"), "{card}");
  assert!(card.contains("The project's database couldn't be read"), "why in plain words: {card}");
  assert!(card.contains("Runs keep going"), "what still works: {card}");
  assert!(screen.exists("agents-load-error-log"), "Open log");
  assert!(screen.exists("agents-hub-tabs") && screen.exists("agents-hub-title"), "the head and tabs stay");
  screen.until("no stale count", |s| !s.text("agents-hub-tab-profiles").contains('1'));
  db.execute("UPDATE resource_drafts SET body = ?1", [&body])
    .expect("restore fixture");
  screen.click("agents-load-error-retry");
  screen.until("the profiles in its place", |s| s.exists("agents-new") && !s.exists("agents-load-error"));
  screen.until("counted again", |s| s.text("agents-hub-tab-profiles").contains('1'));
}

#[test]
fn failure_details_show_the_error_and_the_tries() {
  let mut screen = Agents::new(true);
  screen.profile(false);
  let _broken = break_profiles(&screen);
  failed_list(&mut screen);
  assert!(!screen.exists("agents-load-error-details"), "Details start closed");
  screen.click("agents-load-error-details-toggle");
  let details = screen.text("agents-load-error-details");
  assert!(details.contains("errors.storage · The data could not be read or written."), "{details}");
  assert!(details.contains("Tried once · at"), "{details}");
  assert!(screen.exists("agents-load-error-copy"), "Copy details");
  try_again(&mut screen, "Tried 2 times");
}

#[test]
fn failure_still_fails_after_three_tries_and_leads_with_the_log() {
  let mut screen = Agents::new(true);
  screen.profile(false);
  let _broken = break_profiles(&screen);
  failed_list(&mut screen);
  screen.click("agents-load-error-details-toggle");
  assert!(screen.text("agents-load-error-title").contains("couldn’t be read"));
  try_again(&mut screen, "Tried 2 times");
  try_again(&mut screen, "Tried 3 times");
  assert_eq!(screen.text("agents-load-error-title"), "Still can't be read");
  let bounds = |s: &mut Agents, id: &str| {
    s.window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.bounds())
      .unwrap_or_else(|| panic!("{id} laid out"))
  };
  let (log, retry) = (
    bounds(&mut screen, "agents-load-error-log"),
    bounds(&mut screen, "agents-load-error-retry"),
  );
  assert!(log.x < retry.x, "Open log leads: {log:?} {retry:?}");
}

#[test]
fn failure_of_a_save_keeps_the_typed_value_in_its_field_and_says_so_in_the_header() {
  let mut screen = Agents::new(true);
  let id = screen.profile(true);
  screen.open(id);
  let db = rusqlite::Connection::open(screen.root.join(".orchester/orchester.db")).expect("fixture db");
  // Every write of a draft fails from now on; reads keep working.
  db.execute_batch(
    "CREATE TRIGGER test_no_draft_insert BEFORE INSERT ON resource_drafts BEGIN SELECT RAISE(ABORT, 'test'); END;
     CREATE TRIGGER test_no_draft_update BEFORE UPDATE ON resource_drafts BEGIN SELECT RAISE(ABORT, 'test'); END;",
  )
  .expect("failing writes");
  screen.set("agent-responsibility", "Keep this value");
  screen.until("the header names it", |s| {
    s.exists("agent-save-status") && s.text("agent-save-status").contains("A change was not saved")
  });
  assert!(screen.text("agent-save-status").contains("Try again"));
  assert_eq!(screen.value("agent-responsibility"), "Keep this value");
  let field = screen.text("agent-responsibility-row");
  assert!(field.contains("Your value is kept here."), "the field's hint: {field}");
  db.execute_batch("DROP TRIGGER test_no_draft_insert; DROP TRIGGER test_no_draft_update;")
    .expect("writes work again");
  screen.click("agent-save-status-retry");
  screen.until("saved", |s| {
    s.listed().profile(id).expect("profile").profile.responsibilities() == "Keep this value"
  });
}
