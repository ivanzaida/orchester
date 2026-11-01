//! A shared project's first download after a join (ORC-282), in the app's real window against the test-only loopback
//! responder: Kontur's checkout is linked to the responder's project with an empty cache, on a server in the list this
//! computer is not signed in to, and its Tasks list is open. Joining the server with a code downloads the project's
//! tasks at once: meanwhile the list shows the reading's skeleton, and once they came down the list, its counts, the
//! sidebar's count and the board show them without a restart. A failed download says why, and Try again downloads
//! again. A checkout linked on the link page (`LinkKind::Download`) shows its tasks too.

use std::collections::HashMap;

use orchester_api::{
  Credential, MemberRole,
  client::{PlainHttp, ServerAddress},
};
use orchester_app::{
  project::TasksState,
  routes::{JoinTarget, ProjectPage, Route, TaskList, TaskListTab},
  test_support::{Window, project_id},
};
use orchester_core::{id::ServerId, project::ProjectKey, time::Timestamp};
use orchester_store::{ProjectDb, SharingLink};
use orchester_workspace::sharing::{CodeRequest, Trust, project::FirstDownload};

use crate::responder::{KEY, Responder, SLUG, TasksAnswer};

/// The name the responder's server gives itself.
const SERVER: &str = "Kontur team";

/// Kontur's window on its Tasks list, All, with the responder.
struct Joining {
  window: Window,
  responder: Responder,
}

impl Joining {
  /// Kontur linked to the responder's project with an empty cache, its list open and empty.
  fn linked(tasks: TasksAnswer) -> Self {
    let mut joining = Self::local(tasks);
    let services = joining.window.launch.services().expect("the session started").clone();
    let address = ServerAddress::parse(joining.responder.url()).expect("the responder's address");
    let server = joining
      .window
      .data
      .runtime
      .block_on(services.servers.add(&address, SERVER.to_owned(), PlainHttp::NotAllowed))
      .expect("the server is in the list");
    let link = SharingLink {
      server,
      project: Responder::project(),
      key: ProjectKey::new(KEY).expect("a key"),
      role: Some(MemberRole::Admin),
      joined_at: Timestamp::now(),
      cursor: 0,
      last_synced_at: Timestamp::now(),
      removed_at: None,
      address: Some(joining.responder.address()),
    };
    ProjectDb::open(&joining.window.data.dir.path().join("repo-kontur"))
      .expect("Kontur's database")
      .start_sharing(&link, &[], (&[], &HashMap::new()))
      .expect("linked with an empty cache");
    joining.open_list();
    joining
  }

  /// Kontur, local and without tasks, its list open.
  fn local(tasks: TasksAnswer) -> Self {
    let mut joining = Self {
      window: Window::open(),
      responder: Responder::start(tasks),
    };
    joining.open_list();
    joining
  }

  fn open_list(&mut self) {
    self.window.navigation().go(tasks_route());
    self
      .window
      .until("the empty list", |window| laid_out(window, "tasks-empty"));
  }

  /// Joins the responder's server with a code, as Settings › Orchester does.
  fn join(&self) -> ServerId {
    let services = self.window.launch.services().expect("the session started");
    let address = ServerAddress::parse(self.responder.url()).expect("the responder's address");
    let request = CodeRequest {
      code: Credential::new("ABCD-EFGH".to_owned()),
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

  fn download(&self) -> Option<FirstDownload> {
    self.window.projects().feed(project_id()).download.get()
  }

  /// Waits for the list to show the responder's three tasks.
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
fn tasks_download_after_a_join_the_list_shows_downloading_then_fills_without_a_restart() {
  let mut joining = Joining::linked(TasksAnswer::Hold);
  assert!(
    !laid_out(&mut joining.window, "tasks-view-needs_you-count"),
    "an empty project counts no view"
  );
  joining.join();
  joining
    .window
    .until("the download", |window| laid_out(window, "tasks-loading"));
  assert_eq!(
    joining.download(),
    Some(FirstDownload::Downloading {
      server: SERVER.to_owned()
    }),
    "the list says what it waits for"
  );
  assert!(
    !laid_out(&mut joining.window, "tasks-empty"),
    "not an empty project meanwhile"
  );
  joining.responder.tasks(TasksAnswer::Serve);
  joining.until_filled();
  // The sync's own notice fills the list; the download ends with the reading the mark's settling starts after it.
  joining.window.until("the download's end", |window| {
    window.projects().feed(project_id()).download.get().is_none()
  });
  for key in ["KON-1", "KON-2", "KON-3"] {
    let row = format!("task-key-{key}");
    joining.window.until(&row, |window| laid_out(window, &row));
  }
  assert_eq!(joining.window.text("nav-tasks"), "Tasks 3", "the sidebar counts them");
  // Needs you is worked out from the tasks that came down: none has a run on this computer that waits.
  assert_eq!(
    joining.window.text("tasks-view-needs_you-count"),
    "0",
    "Needs you counts the downloaded tasks"
  );
  joining
    .window
    .until("the layout switch", |window| laid_out(window, "tasks-layout-board"));
  joining.window.press("tasks-layout-board");
  for key in ["KON-1", "KON-2", "KON-3"] {
    let card = format!("task-card-{key}");
    joining.window.until(&card, |window| laid_out(window, &card));
  }
  assert_eq!(joining.responder.tasks_asked(), 1, "downloaded once, at the join");
}

#[test]
fn tasks_download_a_failed_download_shows_why_with_try_again_that_downloads() {
  let mut joining = Joining::linked(TasksAnswer::Fail);
  joining.join();
  joining
    .window
    .until("the failure", |window| laid_out(window, "tasks-download-error"));
  let said = joining.window.text("tasks-download-error");
  assert!(said.contains("Kontur's tasks could not be downloaded"), "{said}");
  assert!(said.contains("Orchester joined Kontur team"), "{said}");
  assert!(said.contains("The server refused this."), "{said}");
  joining.responder.tasks(TasksAnswer::Serve);
  joining.window.press("tasks-download-error-retry");
  joining.until_filled();
  assert!(!laid_out(&mut joining.window, "tasks-download-error"));
}

#[test]
fn tasks_download_a_checkout_linked_on_the_link_page_shows_its_tasks() {
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
  assert!(
    matches!(
      joining.window.projects().feed(project_id()).tasks.get(),
      TasksState::Ready(tasks) if tasks.tasks.len() == 3
    ),
    "the feed holds what the list shows"
  );
}

fn tasks_route() -> Route {
  Route::Project(project_id(), ProjectPage::Tasks(TaskList::new(TaskListTab::All)))
}
