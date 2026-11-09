//! A team project's Tasks list and board read from its server through the session's lurq queries (ORC-716), on the QA
//! app's `team-member` and `team-outage` states with the stand-in team server's `/api/v2`: a member's list without New
//! task and with the line naming the Admins, an Admin's with New task, the views counted by the server, a change the
//! server commits refreshing the mounted list, the board without quick add, the outage in place of the rows, the
//! sources keyed by the session and not the person, and a local project left as it was. ORC-727: the team's line on
//! the board only, the sidebar counting the server's Active tasks, the status bar saying the server answers, and
//! Sharing's *Your role* saying what the role does.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_tasks`

use orchester_api::test_support::team_server::{Standing, TeamServer};
use orchester_app::{
  routes::{ProjectPage, ProjectSettingsTab, Route, TaskList, TaskListTab},
  test_support::{Data, Window, project_id},
};

use super::{on_the_tasks_list, prepared, tasks_route, team::State};

const ROW: &str = "task-row-KON-12";
const LOCK: &str = "tasks-team-read-only-text";
const ADMIN_LINE: &str = "tasks-team-admin-line-text";
const UNAVAILABLE: &str = "tasks-team-unavailable-title";

/// The list on `state`, once the server's tasks show.
fn listed(state: State) -> (Window, TeamServer) {
  let (mut window, server) = on_the_tasks_list(state);
  window.until("the server's tasks", |window| window.texts(ROW).is_some());
  (window, server)
}

fn count(window: &mut Window, tab: TaskListTab) -> String {
  window.text(&format!("tasks-view-{}-count", tab.name()))
}

/// The board, where the team's line shows (01b `zv7b6`), and the role it names.
fn role_on_the_board(window: &mut Window) -> String {
  window.press("tasks-layout-board");
  window.until("the team's line", |window| window.texts("tasks-team-role").is_some());
  window.text("tasks-team-role")
}

#[test]
fn team_qa_tasks_a_member_reads_the_servers_tasks_without_new_task() {
  let (mut window, server) = listed(State::Member);
  assert!(server.authority_reads() > 0, "the list read the server");
  assert!(window.texts("tasks-new-task").is_none(), "a member creates no task");
  window.until("the Admins named", |window| {
    window.texts(LOCK).is_some_and(|text| text.concat().contains("dana-k"))
  });
  assert_eq!(
    window.text(LOCK),
    "You read, run, claim and close what you hold. Only Admins create, cancel, reopen or move tasks: dana-k (Admin) and olha (Server admin)."
  );
  let line = window.text("tasks-team-line");
  assert!(
    line.starts_with("Team project on ") && line.ends_with(" · Read from the server · You're a member"),
    "the list names the team and the role (T7 `lCoNt`): {line}"
  );
  assert!(
    window.texts("tasks-team-settings").is_none(),
    "the list's line has no Sharing settings"
  );
  assert!(window.texts(ADMIN_LINE).is_none(), "a member has no Admin line");
  assert_eq!(count(&mut window, TaskListTab::All), "24");
  assert_eq!(count(&mut window, TaskListTab::Active), "18");
  assert_eq!(count(&mut window, TaskListTab::Backlog), "3");
  assert_eq!(count(&mut window, TaskListTab::Done), "3");
  assert_eq!(role_on_the_board(&mut window), "You're a member");
  assert_eq!(window.text("tasks-team-from-server"), "Read from the server");
}

/// The sidebar counts the server's Active tasks, and the status bar says nothing of a server whose reads answer.
#[test]
fn team_qa_tasks_the_sidebar_and_the_status_bar_follow_the_servers_reads() {
  let (mut window, _server) = listed(State::Member);
  window.until("the server's Active count", |window| {
    window.texts("nav-tasks").is_some_and(|texts| texts == ["Tasks", "18"])
  });
  window.until("the server answering", |window| window.texts("status-server").is_none());
}

#[test]
fn team_qa_tasks_an_admin_has_new_task_and_names_the_admins() {
  let (window, server) = prepared(State::Member);
  server.stand_as(Standing::Admin);
  window.navigation().go(tasks_route());
  let mut window = window;
  window.until("the server's tasks", |window| window.texts(ROW).is_some());
  assert!(window.texts("tasks-new-task").is_some(), "an Admin creates tasks");
  assert!(window.texts(LOCK).is_none());
  window.until("the Admins named", |window| {
    window
      .texts(ADMIN_LINE)
      .is_some_and(|text| text.concat().contains("dana-k"))
  });
  assert_eq!(
    window.text(ADMIN_LINE),
    "Admins of this team: bohdan (Admin), dana-k (Admin) and olha (Server admin). Closing a task ends the claim on it."
  );
  let line = window.text("tasks-team-line");
  assert!(
    line.starts_with("Team project on ") && line.ends_with(" · Read from the server · You're an Admin"),
    "T8 `vdSWS`: {line}"
  );
  assert_eq!(role_on_the_board(&mut window), "You're an Admin");
}

/// The `team-member`, `team-admin` and `team-server-admin` states: the board's line and Sharing's *Your role* name the
/// role, and *Your role* says what it does (07e `BLTsP`).
#[test]
fn team_qa_tasks_the_role_states_name_the_role_on_the_board_and_in_sharing() {
  for (state, line, role, does) in [
    (
      State::Member,
      "You're a member",
      "Member",
      "Reads, runs and claims team work".to_owned(),
    ),
    (
      State::Admin,
      "You're an Admin",
      "Admin",
      "Changes team tasks, modules, tags and configuration".to_owned(),
    ),
    (
      State::ServerAdmin,
      "You're a Server admin",
      "Server admin",
      "Administers {host} and every workspace on it; includes Admin".to_owned(),
    ),
  ] {
    let (mut window, server) = listed(state);
    assert_eq!(
      window.texts("tasks-new-task").is_some(),
      state != State::Member,
      "{state:?}"
    );
    assert_eq!(role_on_the_board(&mut window), line);
    window.navigation().go(Route::Project(
      project_id(),
      ProjectPage::Settings(ProjectSettingsTab::Sharing),
    ));
    window.until("Your role", |window| window.texts("sharing-role-value").is_some());
    assert_eq!(window.text("sharing-role-value"), role, "{state:?}");
    let host = server.url().trim_start_matches("http://").to_owned();
    assert_eq!(
      window.text("sharing-role-hint"),
      does.replace("{host}", &host),
      "{state:?}"
    );
  }
}

/// `team-outage-later`: the tasks show, then the server stops answering and they are no longer shown as current.
#[test]
fn team_qa_tasks_an_outage_after_the_tasks_were_read_takes_them_away() {
  let (mut window, server) = listed(State::OutageLater);
  window.until("the outage", |window| window.texts(UNAVAILABLE).is_some());
  assert!(window.texts(ROW).is_none(), "nothing read before shows as current");
  assert!(window.texts("tasks-views").is_none());
  server.authority_unavailable(false);
  window.press("tasks-team-try-again");
  window.until("the server's tasks", |window| window.texts(ROW).is_some());
}

#[test]
fn team_qa_tasks_a_change_the_server_commits_refreshes_the_mounted_list() {
  let (mut window, server) = listed(State::Member);
  let reads = server.authority_reads();
  server.retitle(12, "Retry webhook sends with backoff");
  window.until("the new title", |window| {
    window
      .texts("task-title-text-KON-12")
      .is_some_and(|title| title.concat() == "Retry webhook sends with backoff")
  });
  assert!(server.authority_reads() > reads, "the invalidation read the list again");
}

#[test]
fn team_qa_tasks_a_members_board_has_no_quick_add() {
  let (mut window, _server) = listed(State::Member);
  window.press("tasks-layout-board");
  window.until("the board", |window| window.texts("tasks-board").is_some());
  assert!(
    window.texts("tasks-board-quick-add").is_none(),
    "a member adds no draft"
  );
  assert!(window.text(LOCK).contains("tasks and cards"), "{}", window.text(LOCK));
}

#[test]
fn team_qa_tasks_a_members_row_menu_offers_only_what_their_claim_allows() {
  let (mut window, _server) = listed(State::Member);
  // The row's actions show under the pointer once the row has its place, which can take a pass after it shows.
  window.until("the row's menu", |window| {
    let placed = window
      .tree
      .get_element_by_id_mut(ROW)
      .is_some_and(|row| row.bounds().is_some());
    if placed {
      window.hover(ROW);
    }
    window.texts("task-more-KON-12").is_some()
  });
  window.press("task-more-KON-12");
  window.until("its rows", |window| window.texts("task-more-KON-12-panel").is_some());
  assert!(window.texts("task-more-KON-12-open").is_some());
  assert!(window.texts("task-more-KON-12-copy").is_some());
  // KON-12 is the member's own: they close or block it (decision D4); Cancel, Reopen and Delete are not theirs.
  assert!(window.texts("task-more-KON-12-done").is_some());
  for change in ["cancel", "reopen-todo", "reopen-backlog", "delete"] {
    let id = format!("task-more-KON-12-{change}");
    assert!(window.texts(&id).is_none(), "a member has no {change}");
  }
}

#[test]
fn team_qa_tasks_an_outage_says_team_data_is_unavailable_instead_of_the_rows() {
  let (mut window, server) = on_the_tasks_list(State::Outage);
  window.until("the outage", |window| window.texts(UNAVAILABLE).is_some());
  assert_eq!(window.text(UNAVAILABLE), "Team data is unavailable");
  // The server by its host, as `l6EMj` names orchester.acme.dev.
  let host = server.url().trim_start_matches("http://").to_owned();
  // The card shows as soon as the reads fail; the server's name joins it when the project's link is read, a moment
  // later.
  let hasnt_answered = format!("{host} hasn't answered since");
  window.until("the server's name in the card", |window| {
    window.text("tasks-team-unavailable").contains(&hasnt_answered)
  });
  assert!(window.texts(ROW).is_none(), "no task is shown as current");
  assert!(window.texts("tasks-views").is_none());
  assert!(window.texts("tasks-new-task").is_none());
  window.until("the server not answering", |window| {
    window
      .texts("status-server")
      .is_some_and(|texts| texts.concat().contains("Acme offline"))
  });
}

#[test]
fn team_qa_tasks_try_again_reads_the_tasks_once_the_server_answers() {
  let (mut window, server) = on_the_tasks_list(State::Outage);
  window.until("the outage", |window| window.texts(UNAVAILABLE).is_some());
  server.authority_unavailable(false);
  window.press("tasks-team-try-again");
  window.until("the server's tasks", |window| window.texts(ROW).is_some());
}

#[test]
fn team_qa_tasks_a_source_is_kept_by_the_session_and_never_names_the_person() {
  let (window, _server) = prepared(State::Member);
  let services = window.launch.services().expect("the session started").clone();
  let root = window.data.repo("repo-kontur");
  let source = window
    .data
    .runtime
    .block_on(services.team.source(&root))
    .expect("the project's source")
    .expect("a team project");
  let key = source.key();
  assert_eq!(key.generation, services.team.generation());
  let shown = format!("{source:?}");
  assert!(!shown.contains("orc_"), "no token: {shown}");
  assert!(!shown.contains("bohdan"), "no person: {shown}");
  let client = services.team.client();
  services.team.renew();
  assert!(services.team.client() != client, "a new generation has its own client");
  let again = window
    .data
    .runtime
    .block_on(services.team.source(&root))
    .expect("the project's source")
    .expect("a team project");
  assert_eq!(again.key().generation, key.generation + 1);
  assert_eq!((again.key().server, again.key().project), (key.server, key.project));
}

#[test]
fn team_qa_tasks_a_local_project_reads_its_own_tasks() {
  let data = Data::new();
  let launch = data.launch();
  let kontur = data.add_kontur(launch.workspace().expect("the session started"));
  let services = launch.services().expect("the session started").clone();
  let source = data.runtime.block_on(services.team.source(&kontur.repo_root));
  assert_eq!(source.expect("a local project has no team failure"), None);
  let mut window = Window::of(data, launch);
  window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Tasks(TaskList::new(TaskListTab::All)),
  ));
  window.until("the empty project", |window| window.texts("tasks-empty").is_some());
  assert!(window.texts("tasks-team-line").is_none());
  assert!(window.texts("tasks-new-task").is_some() || window.texts("tasks-empty-new-task").is_some());
}
