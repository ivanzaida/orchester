//! A team project's reads straight from its server (`orchester_workspace::team_reads`) against a fake team server on
//! loopback: the task list's filters, search, sort, pages and counts, modules with core's progress, an expired page
//! snapshot; reads invalidated in flight never make an older revision current ([`races`]); bursts coalesce, reconnects
//! and resets make every read stale, and a revoked membership or a changed session ends the stream and the reads
//! ([`stream`]).
//!
//! `cargo test -p orchester-workspace --test team_reads team_reads_`

mod http;
mod races;
mod server;
mod stream;

use std::{future::Future, time::Duration};

use orchester_api::{
  client::{Client, PlainHttp, ServerAddress},
  v2::{ModuleListRequest, PageRequest, QueryScope, SearchText, TaskFilters, TaskListRequest, TaskSort},
};
use orchester_core::{id::ServerId, secret::SecretValue, task::organization::WorkflowStatus};
use orchester_workspace::{
  sharing::sync::Clock,
  team_reads::{TeamProject, TeamReadError, TeamSession},
};
use server::TeamServer;
use tokio::{sync::watch, task::JoinHandle};

const ALICE: &str = "alice-token";
const BOB: &str = "bob-token";

/// The project's tasks: title, workflow status and priority. The first three are in its module.
const TASKS: &[(&str, &str, &str)] = &[
  ("Sign in", "in_review", "high"),
  ("Sign out", "done", "none"),
  ("Billing", "in_review", "urgent"),
  ("Audit log", "done", "low"),
  ("Search", "in_review", "none"),
  ("Signup form", "canceled", "none"),
];

/// Reconnects at once, so the tests do not wait out the backoff.
struct QuickClock;

impl Clock for QuickClock {
  async fn wait(&self, _: Duration) {
    tokio::time::sleep(Duration::from_millis(10)).await;
  }

  fn jitter(&self) -> f64 {
    0.0
  }
}

/// A member's reads of the server's project, its subscription followed on the runtime, and its session's generation.
struct Reader {
  project: TeamProject,
  generation: watch::Sender<u64>,
  follow: JoinHandle<TeamReadError>,
}

impl Reader {
  fn start(url: &str, token: &str, server: &TeamServer) -> Self {
    let client = Client::new(ServerAddress::parse(url).expect("address"), PlainHttp::NotAllowed).expect("client");
    let token = SecretValue::new(token.to_owned()).expect("token");
    let generation = watch::channel(0).0;
    let session = TeamSession::new(
      (client, token),
      (ServerId::generate(), server.project),
      generation.subscribe(),
    );
    let project = TeamProject::new(session);
    let following = project.clone();
    let follow = tokio::spawn(async move { following.follow(&QuickClock).await });
    Self {
      project,
      generation,
      follow,
    }
  }

  fn of(server: &TeamServer, token: &str) -> Self {
    Self::start(&server.url, token, server)
  }

  /// Waits until the subscription has seen the server's last batch.
  async fn until_seen(&self, server: &TeamServer) {
    let cursor = server.cursor();
    within(async {
      while self.project.cursor().is_none_or(|seen| seen.position().get() < cursor) {
        tokio::time::sleep(Duration::from_millis(10)).await;
      }
    })
    .await;
  }
}

/// `future`'s output, failing the test after five seconds.
async fn within<T>(future: impl Future<Output = T>) -> T {
  tokio::time::timeout(Duration::from_secs(5), future)
    .await
    .expect("in time")
}

/// Runs `wait` on the server, which blocks, off the runtime's workers.
async fn on_server(server: &TeamServer, wait: impl FnOnce(&TeamServer) + Send + 'static) {
  let server = server.clone();
  within(tokio::task::spawn_blocking(move || wait(&server)))
    .await
    .expect("the server's wait");
}

fn titles(page: &orchester_api::v2::Page<orchester_api::v2::TaskRead>) -> Vec<&str> {
  page.items.iter().map(|read| read.task.content.title.as_str()).collect()
}

fn request(filters: TaskFilters, sort: TaskSort, limit: u16) -> TaskListRequest {
  TaskListRequest {
    filters,
    sort,
    page: PageRequest::new(None, Some(limit)).expect("a page"),
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_reads_task_pages_filter_search_sort_and_share_one_snapshot() {
  let server = TeamServer::start(&[ALICE], TASKS);
  let reader = Reader::of(&server, ALICE);
  let in_review = TaskFilters {
    statuses: vec![WorkflowStatus::InReview],
    ..TaskFilters::default()
  };
  let first = within(
    reader
      .project
      .task_page(request(in_review.clone(), TaskSort::Created, 2)),
  )
  .await
  .expect("the first page");
  assert_eq!(first.scope(), QueryScope::TaskList {});
  assert_eq!(titles(first.value()), ["Sign in", "Billing"]);
  let next = first.value().next.clone().expect("a next page");
  let mut following = request(in_review, TaskSort::Created, 2);
  following.page = PageRequest::new(Some(next), Some(2)).expect("a page");
  let second = within(reader.project.task_page(following))
    .await
    .expect("the second page");
  assert_eq!(titles(second.value()), ["Search"]);
  assert_eq!(second.value().next, None, "the last page");
  assert_eq!(second.value().cursor, first.value().cursor, "one snapshot");
  assert_eq!(second.taken(), first.taken());

  let search = TaskFilters {
    search: Some(SearchText::new(" SIGN ").expect("search")),
    ..TaskFilters::default()
  };
  let found = within(reader.project.task_page(request(search, TaskSort::Created, 50))).await;
  assert_eq!(
    titles(found.expect("found").value()),
    ["Sign in", "Sign out", "Signup form"]
  );
  let by_title = within(
    reader
      .project
      .task_page(request(TaskFilters::default(), TaskSort::Title, 3)),
  )
  .await;
  assert_eq!(
    titles(by_title.expect("by title").value()),
    ["Audit log", "Billing", "Search"]
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_reads_board_counts_follow_the_filters() {
  let server = TeamServer::start(&[ALICE], TASKS);
  let reader = Reader::of(&server, ALICE);
  let all = within(reader.project.board_counts(TaskFilters::default()))
    .await
    .expect("counts");
  let counts = all.value();
  assert_eq!(
    (counts.in_review, counts.done, counts.canceled, counts.todo),
    (3, 2, 1, 0)
  );
  assert_eq!(all.scope(), QueryScope::TaskList {});
  let module = TaskFilters {
    modules: vec![Some(server.module())],
    ..TaskFilters::default()
  };
  let in_module = within(reader.project.board_counts(module)).await.expect("counts");
  assert_eq!((in_module.value().in_review, in_module.value().done), (2, 1));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_reads_modules_count_progress_by_core() {
  let server = TeamServer::start(&[ALICE], TASKS);
  let reader = Reader::of(&server, ALICE);
  let modules = within(reader.project.module_page(ModuleListRequest::default()))
    .await
    .expect("modules");
  assert_eq!(modules.scope(), QueryScope::ModuleList {});
  let summary = &modules.value().items[0];
  let progress = summary.progress.to_core();
  assert_eq!((progress.scope, progress.done, progress.remaining()), (3, 1, 2));
  assert_eq!(progress.percent(), Some(33));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_reads_an_expired_page_snapshot_is_typed() {
  let server = TeamServer::start(&[ALICE], TASKS);
  let reader = Reader::of(&server, ALICE);
  let first = within(
    reader
      .project
      .task_page(request(TaskFilters::default(), TaskSort::Created, 2)),
  )
  .await
  .expect("the first page");
  server.expire_pages();
  let mut following = request(TaskFilters::default(), TaskSort::Created, 2);
  following.page = PageRequest::new(first.value().next.clone(), Some(2)).expect("a page");
  let expired = within(reader.project.task_page(following)).await;
  assert_eq!(expired.err(), Some(TeamReadError::CursorExpired));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_reads_an_unknown_token_signs_out_and_an_unreachable_server_fails_reads() {
  let server = TeamServer::start(&[ALICE], TASKS);
  let stranger = Reader::of(&server, BOB);
  assert_eq!(
    within(stranger.follow).await.expect("followed"),
    TeamReadError::SignedOut
  );
  let read = within(stranger.project.capabilities()).await;
  assert_eq!(read.err(), Some(TeamReadError::SignedOut));

  let closed = std::net::TcpListener::bind("127.0.0.1:0").expect("listen");
  let url = format!("http://{}", closed.local_addr().expect("address"));
  drop(closed);
  let unreachable = Reader::start(&url, ALICE, &server);
  let read = within(unreachable.project.metadata()).await;
  assert_eq!(read.err(), Some(TeamReadError::Unavailable { retry_after: None }));
  unreachable.follow.abort();
}
