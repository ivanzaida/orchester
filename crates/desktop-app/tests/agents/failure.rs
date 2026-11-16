//! The Agents page when its read or a save fails (`Failure states · redesign` `SsC16`): the card replaces only the
//! page's content, says what couldn't be read and why, tries again in place, shows the error and the tries in its
//! Details and says it still can't be read after three tries; a save that fails keeps the typed value in its field and
//! says so in the header.
use super::{harness::Agents, support::project_id};

/// Breaks the connections' settings so the hub can't read what its profiles run on; the settings to put back.
fn break_profiles(screen: &Agents) -> (rusqlite::Connection, String) {
  let db = rusqlite::Connection::open(screen.root.join(".orchester/orchester.db")).expect("fixture db");
  let settings: String = db
    .query_row("SELECT settings FROM connections", [], |row| row.get(0))
    .expect("settings");
  db.execute("UPDATE connections SET settings = 'broken'", [])
    .expect("corrupt disposable fixture");
  (db, settings)
}

fn failed_list(screen: &mut Agents) {
  screen.window.navigation().go(orchester_app::routes::Route::Project(
    project_id(),
    orchester_app::routes::ProjectPage::AgentsHub(orchester_app::routes::HubView::new(
      orchester_app::routes::HubFilter::Profiles,
      None,
    )),
  ));
  // A list already shown reads again once the profiles change.
  screen
    .window
    .projects()
    .feed(project_id())
    .outside
    .update(|counts| counts.profiles += 1);
  screen.until("the failure card", |s| s.exists("local-hub-load-error"));
}

/// Presses Try again and waits until the read under way ends again.
fn try_again(screen: &mut Agents, tries: &str) {
  screen.click("local-hub-load-error-retry");
  screen.until(tries, |s| {
    s.exists("local-hub-load-error-tries") && s.text("local-hub-load-error-tries").contains(tries)
  });
}

#[test]
fn failure_page_says_what_and_why_hides_the_count_and_tries_again_in_place() {
  let mut screen = Agents::new(true);
  screen.profile(true);
  screen.go();
  screen.until("counted", |s| s.text("local-hub-filter-profiles").contains('1'));
  let (db, body) = break_profiles(&screen);
  failed_list(&mut screen);
  let card = screen.text("local-hub-load-error");
  assert!(card.contains("The agents of this project couldn't be read"), "{card}");
  assert!(
    card.contains("couldn’t be read") || card.contains("couldn't be read"),
    "why in plain words: {card}"
  );
  assert!(card.contains("Runs keep going"), "what still works: {card}");
  assert!(screen.exists("local-hub-load-error-log"), "Open log");
  assert!(
    screen.exists("local-hub-filter") && screen.exists("local-hub-title"),
    "the head and filter stay"
  );
  screen.until("no stale count", |s| !s.text("local-hub-filter-profiles").contains('1'));
  db.execute("UPDATE connections SET settings = ?1", [&body])
    .expect("restore fixture");
  screen.click("local-hub-load-error-retry");
  screen.until("the profiles in its place", |s| {
    s.exists("local-hub-list") && !s.exists("local-hub-load-error")
  });
  screen.until("counted again", |s| s.text("local-hub-filter-profiles").contains('1'));
}

#[test]
fn failure_details_show_the_error_and_the_tries() {
  let mut screen = Agents::new(true);
  screen.profile(true);
  let _broken = break_profiles(&screen);
  failed_list(&mut screen);
  assert!(!screen.exists("local-hub-load-error-details"), "Details start closed");
  screen.click("local-hub-load-error-details-toggle");
  let details = screen.text("local-hub-load-error-details");
  assert!(
    details.contains("errors.storage · The data could not be read or written."),
    "{details}"
  );
  assert!(details.contains("Tried once · at"), "{details}");
  assert!(screen.exists("local-hub-load-error-copy"), "Copy details");
  try_again(&mut screen, "Tried 2 times");
}

#[test]
fn failure_still_fails_after_three_tries_and_leads_with_the_log() {
  let mut screen = Agents::new(true);
  screen.profile(true);
  let _broken = break_profiles(&screen);
  failed_list(&mut screen);
  screen.click("local-hub-load-error-details-toggle");
  assert!(screen.text("local-hub-load-error-title").contains("couldn't be read"));
  try_again(&mut screen, "Tried 2 times");
  try_again(&mut screen, "Tried 3 times");
  assert_eq!(screen.text("local-hub-load-error-title"), "Still can't be read");
  let bounds = |s: &mut Agents, id: &str| {
    s.window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.bounds())
      .unwrap_or_else(|| panic!("{id} laid out"))
  };
  let (log, retry) = (
    bounds(&mut screen, "local-hub-load-error-log"),
    bounds(&mut screen, "local-hub-load-error-retry"),
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
