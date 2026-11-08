//! Archive in the Agents hub's panel and what it leaves on the task page (`Agents · MCP server · admin · archive` A23
//! `Pnpbe`, the stopped run's task page A22 `pUu78`, ORC-838; `docs/design/team-configuration.md`, decision 11): the
//! panel's … menu holds *Archive*, which opens a confirmation that lists what names the server and the runs that stop,
//! read from the stand-in team server; opening it is anyone's, confirming it a person's alone; and a run an archive
//! stopped says which resource and who archived it. On the QA states `team-archive` and `team-archived`.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_hub_archive`

use lurq::app::events::with_synthetic_input;
use orchester_api::test_support::team_server::TeamServer;
use orchester_app::test_support::Window;
use orchester_core::id::{ProjectId, ResourceId};

use super::{hub_edit::on_pencil_until, not_ready::on_task, team::State};

const MORE: &str = "team-hub-more";
const ARCHIVE: &str = "team-hub-archive";
const CONFIRM: &str = "team-hub-archive-confirm";
const RUNS: &str = "team-hub-archive-runs-list";

/// The window on `pencil-design` in the state `team-archive`, once its … menu shows.
fn on_archive() -> (Window, TeamServer, (ProjectId, ResourceId)) {
  on_pencil_until(("team-archive", State::TeamArchive), MORE)
}

/// The confirmation open, as an MCP client may open it: a press on the menu and on *Archive* changes nothing.
fn opened(window: &mut Window) {
  with_synthetic_input(|| window.press(MORE));
  window.until("the menu", |window| window.texts(ARCHIVE).is_some());
  with_synthetic_input(|| window.press(ARCHIVE));
  window.until("what uses it", |window| window.texts(RUNS).is_some());
}

/// Whether the stand-in holds `pencil-design` archived.
fn archived_on(server: &TeamServer) -> bool {
  server
    .resources("kontur")
    .iter()
    .any(|(resource, _)| resource["name"] == "pencil-design" && resource["archived"] == true)
}

#[test]
fn team_qa_hub_archive_confirmation_lists_what_names_the_server_and_the_runs_that_stop() {
  let (mut window, server, _) = on_archive();
  opened(&mut window);
  assert_eq!(window.text("team-hub-archive-title"), "Archive pencil-design?");
  assert_eq!(
    window.text("team-hub-archive-body"),
    "pencil-design leaves every list and no new run can use it. Its versions stay readable, and an admin can restore it."
  );
  assert_eq!(window.text("team-hub-archive-used-heading"), "USED BY · 1");
  assert_eq!(window.text("team-hub-archive-used-0-name"), "Builder · agent profile");
  assert_eq!(window.text("team-hub-archive-used-0-version"), "v1");
  assert_eq!(window.text("team-hub-archive-runs-heading"), "RUNS THAT WILL STOP · 2");
  assert_eq!(window.text("team-hub-archive-run-0-name"), "KON-12");
  assert_eq!(window.text("team-hub-archive-run-0-holder"), "Bohdan Koval");
  assert_eq!(window.text("team-hub-archive-run-1-name"), "KON-21");
  assert_eq!(window.text("team-hub-archive-run-1-holder"), "olha");
  assert_eq!(
    window.text("team-hub-archive-note"),
    "Builder stays not ready on every computer until an admin removes pencil-design from it or restores it."
  );
  assert_eq!(window.text(CONFIRM), "Archive and stop 2 runs");
  assert!(!archived_on(&server), "listing what uses it archives nothing");
}

#[test]
fn team_qa_hub_archive_a_person_confirming_archives_the_resource_on_the_team_server() {
  let (mut window, server, _) = on_archive();
  opened(&mut window);
  window.press(CONFIRM);
  window.until("the archive on the server", |_| archived_on(&server));
  window.until("the confirmation closed", |window| {
    window.texts("team-hub-archive-title").is_none()
  });
}

#[test]
fn team_qa_hub_archive_automation_can_open_the_confirmation_but_not_confirm_it() {
  let (mut window, server, _) = on_archive();
  opened(&mut window);
  with_synthetic_input(|| window.press(CONFIRM));
  window.until("why nothing was archived", |window| {
    window.texts("team-hub-archive-confirm-person-only").is_some()
  });
  assert!(!archived_on(&server), "a press by automation archives nothing");
  assert!(
    window.texts("team-hub-archive-title").is_some(),
    "the confirmation stays open"
  );
}

#[test]
fn team_qa_hub_archive_cancel_keeps_everything_as_it_is() {
  let (mut window, server, _) = on_archive();
  opened(&mut window);
  window.press("team-hub-archive-cancel");
  window.until("the confirmation closed", |window| {
    window.texts("team-hub-archive-title").is_none()
  });
  assert!(!archived_on(&server));
}

#[test]
fn team_qa_hub_archive_without_knowing_what_uses_it_nothing_is_archived() {
  let (mut window, server, _) = on_archive();
  server.set_down(true);
  opened_without_lists(&mut window);
  window.until("the failure", |window| {
    window.texts("team-hub-archive-read-failed").is_some()
  });
  assert!(window.texts(RUNS).is_none(), "no list is shown for a read that failed");
  window.press(CONFIRM);
  assert!(!archived_on(&server), "the action waits for what it stops");
}

/// [`opened`] for a read that fails, which has no list to wait for.
fn opened_without_lists(window: &mut Window) {
  with_synthetic_input(|| window.press(MORE));
  window.until("the menu", |window| window.texts(ARCHIVE).is_some());
  with_synthetic_input(|| window.press(ARCHIVE));
  window.until("the confirmation", |window| {
    window.texts("team-hub-archive-title").is_some()
  });
}

#[test]
fn team_qa_hub_archive_a_stopped_run_names_the_resource_and_who_archived_it() {
  let (mut window, _server, _) = on_task(("team-archived", State::TeamArchived), "KON-12");
  window.until("the card", |window| window.texts("task-archived").is_some());
  assert_eq!(
    window.text("task-archived-title"),
    "Stopped: pencil-design was archived"
  );
  assert!(
    window.text("task-archived-meta").starts_with("Run 8 · "),
    "{}",
    window.text("task-archived-meta")
  );
  let body = window.text("task-archived-body");
  assert!(
    body.starts_with("dana-k archived the team MCP server pencil-design at "),
    "{body}"
  );
  assert!(
    body.ends_with("the worktree, changes and conversation are kept."),
    "{body}"
  );
  let resource = window.text("task-archived-resource-need");
  assert!(resource.starts_with("MCP server · archived by dana-k · "), "{resource}");
  assert!(
    window.text("task-status").contains("pencil-design was archived"),
    "the status line says why: {}",
    window.text("task-status")
  );
  // Builder still names it, which the team server says: it cannot run until it is changed or the server restored.
  window.until("the profile that still uses it", |window| {
    window.texts("task-archived-profile-0").is_some()
  });
  assert_eq!(window.text("task-archived-profile-0-what"), "Builder v1");
  assert_eq!(
    window.text("task-archived-profile-0-need"),
    "still uses pencil-design · not ready anywhere until changed"
  );
}
