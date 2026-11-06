//! A team project's Tasks list after a join (ORC-282, ORC-716), in the app's real window against the stand-in team
//! server (`orchester_api::test_support::team_server`), whose tasks the list reads from its `/api/v2`: Kontur's
//! checkout is linked to the team's Kontur on a server in the list this computer is not signed in to, and its Tasks
//! list says the team's tasks cannot be read. Joining the server with a code reads them: meanwhile the list shows the
//! reading's skeleton, and once the server answers, the list, its counts and the board show them without a restart. A
//! refusal says team data is unavailable, and Try again reads again. A checkout linked on the link page
//! (`LinkKind::Download`) shows the server's tasks too.

use std::collections::HashMap;

use orchester_api::{
  Credential, MemberRole,
  client::{PlainHttp, ServerAddress},
  test_support::team_server::{CODE, NAME, TasksAnswer, TeamServer},
};
use orchester_app::{
  routes::{JoinTarget, ProjectPage, Route, TaskList, TaskListTab},
  test_support::{Window, project_id},
};
use orchester_core::{id::ServerId, project::ProjectKey, time::Timestamp};
use orchester_store::{ProjectDb, SharingLink};
use orchester_workspace::sharing::{CodeRequest, Trust};

/// The team's Kontur on the stand-in server.
const SLUG: &str = "kontur";

/// Kontur's window on its Tasks list, All, with the team server.
struct Joining {
  window: Window,
  server: TeamServer,
}

impl Joining {
  /// Kontur linked to the team's Kontur with nothing of it here, on a server this computer is not signed in to; its
  /// list open, saying why the team's tasks cannot be read.
  fn linked(tasks: TasksAnswer) -> Self {
    let mut joining = Self::started(tasks);
    let services = joining.window.launch.services().expect("the session started").clone();
    let address = ServerAddress::parse(&joining.server.url()).expect("the server's address");
    let server = joining
      .window
      .data
      .runtime
      .block_on(services.servers.add(&address, NAME.to_owned(), PlainHttp::NotAllowed))
      .expect("the server is in the list");
    let link = SharingLink {
      server,
      project: joining.server.project(SLUG).expect("the team shares Kontur"),
      key: ProjectKey::new("KON").expect("a key"),
      role: Some(MemberRole::Member),
      joined_at: Timestamp::now(),
      cursor: 0,
      last_synced_at: Timestamp::now(),
      removed_at: None,
      address: Some(format!("{}/{SLUG}", joining.server.url())),
    };
    ProjectDb::open(&joining.window.data.dir.path().join("repo-kontur"))
      .expect("Kontur's database")
      .start_sharing(&link, &[], (&[], &HashMap::new()))
      .expect("linked with nothing cached");
    joining.window.navigation().go(tasks_route());
    joining.window.until("why the team's tasks cannot be read", |window| {
      laid_out(window, "tasks-team-unavailable")
    });
    joining
  }

  /// Kontur, local and without tasks, its list open.
  fn local(tasks: TasksAnswer) -> Self {
    let mut joining = Self::started(tasks);
    joining.window.navigation().go(tasks_route());
    joining
      .window
      .until("the empty list", |window| laid_out(window, "tasks-empty"));
    joining
  }

  /// Kontur's window, and the team server sharing its Kontur with tasks answered as `tasks`.
  fn started(tasks: TasksAnswer) -> Self {
    let server = TeamServer::start();
    server.share(&[("Kontur", SLUG, None)]);
    server.answer_tasks(tasks);
    Self {
      window: Window::open(),
      server,
    }
  }

  /// Joins the team server with its code, as Settings › Orchester does.
  fn join(&self) -> ServerId {
    let services = self.window.launch.services().expect("the session started");
    let address = ServerAddress::parse(&self.server.url()).expect("the server's address");
    let request = CodeRequest {
      code: Credential::new(CODE.to_owned()),
      username: None,
      display_name: None,
      proof: None,
    };
    self
      .window
      .data
      .runtime
      .block_on(
        services
          .servers
          .join_with_code(&address, Trust::System(PlainHttp::NotAllowed), request),
      )
      .expect("the code joins the server")
  }

  /// Waits for the list to show the server's three tasks.
  fn until_filled(&mut self) {
    self
      .window
      .until_within(std::time::Duration::from_secs(30), "the tasks", |window| {
        window
          .texts("tasks-view-all-count")
          .is_some_and(|count| count.concat() == "3")
      });
  }
}

fn laid_out(window: &mut Window, id: &str) -> bool {
  window
    .tree
    .get_element_by_id_mut(id)
    .is_some_and(|element| element.bounds().is_some())
}

#[test]
fn tasks_download_after_a_join_the_list_waits_for_the_server_then_fills_without_a_restart() {
  let mut joining = Joining::linked(TasksAnswer::Hold);
  joining.join();
  joining
    .window
    .until("the reading", |window| laid_out(window, "tasks-loading"));
  assert!(
    !laid_out(&mut joining.window, "tasks-empty"),
    "not an empty project meanwhile"
  );
  joining.server.answer_tasks(TasksAnswer::Serve);
  joining.until_filled();
  for key in ["KON-1", "KON-2", "KON-3"] {
    let row = format!("task-key-{key}");
    joining.window.until(&row, |window| laid_out(window, &row));
  }
  // Needs you is worked out from this computer's runs: none of the team's tasks has one that waits.
  assert_eq!(
    joining.window.text("tasks-view-needs_you-count"),
    "0",
    "Needs you counts this computer's waiting runs"
  );
  joining
    .window
    .until("the layout switch", |window| laid_out(window, "tasks-layout-board"));
  joining.window.press("tasks-layout-board");
  for key in ["KON-1", "KON-2", "KON-3"] {
    let card = format!("task-card-{key}");
    joining.window.until(&card, |window| laid_out(window, &card));
  }
  assert!(joining.server.authority_reads() > 0, "the list read the server");
}

#[test]
fn tasks_download_a_refusal_says_team_data_is_unavailable_and_try_again_reads_again() {
  let mut joining = Joining::linked(TasksAnswer::Fail);
  joining.join();
  joining.window.until("the refusal", |window| {
    window
      .texts("tasks-team-unavailable-title")
      .is_some_and(|title| title.concat() == "Team data is unavailable")
  });
  joining.server.answer_tasks(TasksAnswer::Serve);
  joining.window.press("tasks-team-try-again");
  joining.until_filled();
  assert!(!laid_out(&mut joining.window, "tasks-team-unavailable"));
}

#[test]
fn tasks_download_a_checkout_linked_on_the_link_page_shows_the_servers_tasks() {
  let mut joining = Joining::local(TasksAnswer::Serve);
  let server = joining.join();
  let target = JoinTarget::new(server, SLUG).expect("a join target");
  joining
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Join(target)));
  joining
    .window
    .until("the link page", |window| laid_out(window, "sharing-link-confirm"));
  joining.window.press("sharing-link-confirm");
  joining.window.until("the first download's step", |window| {
    window.navigation().route_now() == Route::FirstDownload(project_id())
  });
  joining.window.navigation().go(tasks_route());
  joining.until_filled();
  for key in ["KON-1", "KON-2", "KON-3"] {
    let row = format!("task-key-{key}");
    joining.window.until(&row, |window| laid_out(window, &row));
  }
}

fn tasks_route() -> Route {
  Route::Project(project_id(), ProjectPage::Tasks(TaskList::new(TaskListTab::All)))
}
