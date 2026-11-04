//! A team project's Tasks list and board read from its server through the session's lurq queries (ORC-716), on the QA
//! app's `team-member` and `team-outage` states with the stand-in team server's `/api/v2`: a member's list without New
//! task and with the line naming the Admins, an Admin's with New task, the views counted by the server, a change the
//! server commits refreshing the mounted list, the board without quick add, the outage in place of the rows, the
//! sources keyed by the session and not the person, and a local project left as it was.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_tasks`

use orchester_api::test_support::team_server::{Standing, TeamServer};
use orchester_app::{
  routes::{ProjectPage, Route, TaskList, TaskListTab},
  test_support::{Data, Window, project_id},
};

use super::{on_the_tasks_list, prepared, tasks_route, team::State};

const ROW: &str = "task-row-KON-12";
const LOCK: &str = "tasks-team-read-only-text";
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
    "Only Admins change team tasks, modules and tags: dana-k (Admin) and olha (Server admin). You read, run and claim."
  );
  assert_eq!(window.text("tasks-team-role"), "You're a member");
  assert_eq!(window.text("tasks-team-from-server"), "Read from the server");
  assert_eq!(count(&mut window, TaskListTab::All), "24");
  assert_eq!(count(&mut window, TaskListTab::Active), "18");
  assert_eq!(count(&mut window, TaskListTab::Backlog), "3");
  assert_eq!(count(&mut window, TaskListTab::Done), "3");
}

#[test]
fn team_qa_tasks_an_admin_has_new_task_and_no_lock_line() {
  let (window, server) = prepared(State::Member);
  server.stand_as(Standing::Admin);
  window.navigation().go(tasks_route());
  let mut window = window;
  window.until("the server's tasks", |window| window.texts(ROW).is_some());
  assert!(window.texts("tasks-new-task").is_some(), "an Admin creates tasks");
  assert!(window.texts(LOCK).is_none());
  assert_eq!(window.text("tasks-team-role"), "You're an Admin");
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
  assert!(window.text(LOCK).contains("and move cards"), "{}", window.text(LOCK));
}

#[test]
fn team_qa_tasks_an_outage_says_team_data_is_unavailable_instead_of_the_rows() {
  let (mut window, _server) = on_the_tasks_list(State::Outage);
  window.until("the outage", |window| window.texts(UNAVAILABLE).is_some());
  assert_eq!(window.text(UNAVAILABLE), "Team data is unavailable");
  assert!(window.texts(ROW).is_none(), "no task is shown as current");
  assert!(window.texts("tasks-views").is_none());
  assert!(window.texts("tasks-new-task").is_none());
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
