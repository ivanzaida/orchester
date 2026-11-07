//! What the Agents page of a team project shows while it cannot yet tell the project from a local one, and while its
//! list is read again or the read failed (`Failure states · redesign` `SsC16`, ORC-831 review).
//! `cargo test -p orchester-app --test agents team_hub`.

use orchester_app::{
  project::hold::ReadHold,
  routes::{HubView, ProjectPage, Route},
};

use super::{
  harness::Agents,
  support::project_id,
  team_hub::{open, team},
};

fn hold(agents: &Agents) -> ReadHold {
  agents.window.launch.services().expect("services").hold.clone()
}

/// Makes the project's database unreadable (a newer schema); the version to put back.
fn break_database(agents: &Agents) -> (rusqlite::Connection, u32) {
  let db = rusqlite::Connection::open(agents.root.join(".orchester/orchester.db")).expect("fixture db");
  let version: u32 = db
    .query_row("PRAGMA user_version", [], |row| row.get(0))
    .expect("schema version");
  db.execute_batch("PRAGMA user_version = 99999").expect("future schema");
  (db, version)
}

fn mend_database((db, version): &(rusqlite::Connection, u32)) {
  db.execute_batch(&format!("PRAGMA user_version = {version}"))
    .expect("restore fixture");
}

#[test]
fn team_hub_waits_for_the_team_read_instead_of_showing_the_local_list() {
  let mut agents = Agents::new(true);
  let hold = hold(&agents);
  hold.set(true);
  team(&agents);
  agents
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Agents));
  agents.until("loading", |s| s.exists("team-hub-team-loading"));
  assert!(!agents.exists("agents-new") && !agents.exists("agents-loading") && !agents.exists("team-hub-list"));
  hold.set(false);
  agents.until("the hub", |s| s.exists("team-hub-list"));
  assert!(!agents.exists("agents-new") && !agents.exists("team-hub-team-loading"));
}

#[test]
fn team_hub_says_so_when_the_team_read_failed_and_reads_again_on_retry() {
  let mut agents = Agents::new(true);
  let hold = hold(&agents);
  hold.set(true);
  team(&agents);
  agents
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Agents));
  agents.until("loading", |s| s.exists("team-hub-team-loading"));
  // The database cannot be read when the held read goes on.
  let broken = break_database(&agents);
  hold.set(false);
  agents.until("the failure", |s| s.exists("team-hub-team-error"));
  assert!(!agents.exists("agents-new") && !agents.exists("team-hub-list"));
  assert!(agents.exists("team-hub-team-error-retry"));
  mend_database(&broken);
  agents.click("team-hub-team-error-retry");
  agents.until("the hub", |s| {
    s.exists("team-hub-list") && !s.exists("team-hub-team-error")
  });
}

#[test]
fn team_hub_marks_the_last_answer_while_it_is_read_again() {
  let mut agents = Agents::new(true);
  team(&agents);
  open(&mut agents, HubView::default());
  assert_eq!(agents.text("team-hub-filter-all-count"), "4");
  assert!(!agents.exists("team-hub-reading"));
  let hold = hold(&agents);
  hold.set(true);
  agents
    .window
    .projects()
    .feed(project_id())
    .outside
    .update(|counts| counts.profiles += 1);
  agents.until("the reading mark", |s| s.exists("team-hub-reading"));
  assert_eq!(agents.text("team-hub-reading-label"), "Reading…");
  assert!(!agents.exists("team-hub-filter-hint"));
  hold.set(false);
  agents.until("current again", |s| !s.exists("team-hub-reading"));
  assert!(agents.exists("team-hub-filter-hint") && agents.exists("team-hub-list"));
}

#[test]
fn team_hub_failed_read_again_shows_the_failure_and_no_stale_counts() {
  let mut agents = Agents::new(true);
  team(&agents);
  open(&mut agents, HubView::default());
  assert_eq!(agents.text("team-hub-filter-all-count"), "4");
  let broken = break_database(&agents);
  agents
    .window
    .projects()
    .feed(project_id())
    .outside
    .update(|counts| counts.profiles += 1);
  agents.until("the failure", |s| s.exists("team-hub-load-error"));
  assert!(!agents.exists("team-hub-list") && !agents.exists("team-hub-panel"));
  assert!(
    !agents.exists("team-hub-filter-all-count") && agents.exists("team-hub-filter-all"),
    "the segments stay and count nothing"
  );
  mend_database(&broken);
  agents.click("team-hub-load-error-retry");
  agents.until("the list", |s| s.exists("team-hub-list"));
  assert_eq!(agents.text("team-hub-filter-all-count"), "4");
}
