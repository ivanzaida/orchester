//! The subscription as reads see it: a burst of invalidations comes as one batch that makes only the reads of its
//! scopes stale; a reconnect and a `reset` make every read stale and resume from the last cursor; a revoked membership
//! ends the stream, and a read held across it returns no data; a changed session ends the stream and the reads.

use orchester_api::v2::{QueryScope, TaskFilters};
use orchester_workspace::team_reads::TeamReadError;

use super::{ALICE, Reader, TASKS, TaskSort, on_server, request, server::TeamServer, within};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_reads_a_burst_coalesces_and_stales_only_its_scopes() {
  let server = TeamServer::start(&[ALICE], TASKS);
  let reader = Reader::of(&server, ALICE);
  let tasks = server.tasks();
  let (task, other) = (tasks[0].id, tasks[2].id);
  let read = within(reader.project.task(task)).await.expect("a task");
  let kept = within(reader.project.task(other)).await.expect("a task");
  let list = within(
    reader
      .project
      .task_page(request(TaskFilters::default(), TaskSort::Created, 10)),
  )
  .await
  .expect("a page");
  for title in ["Sign in 1", "Sign in 2", "Sign in 3"] {
    server.retitle(task, title);
  }
  reader.until_seen(&server).await;

  let changes = within(reader.project.changes()).await.expect("changes");
  assert!(!changes.is_reset());
  let last = reader.project.cursor().expect("a cursor");
  let scopes: Vec<_> = changes
    .scopes()
    .iter()
    .map(|(scope, cursor)| (*scope, *cursor))
    .collect();
  assert_eq!(
    scopes,
    [(QueryScope::TaskList {}, last), (QueryScope::Task { task }, last)]
  );
  assert!(read.is_stale(&changes));
  assert!(list.is_stale(&changes));
  assert!(!kept.is_stale(&changes), "another task's read stays current");
  let again = within(reader.project.task(task)).await.expect("a task");
  assert_eq!(again.value().task.content.title, "Sign in 3");
  assert!(!again.is_stale(&changes), "read after the changes it already holds");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_reads_reconnect_and_reset_make_every_read_stale() {
  let server = TeamServer::start(&[ALICE], TASKS);
  let reader = Reader::of(&server, ALICE);
  let tasks = server.tasks();
  let read = within(reader.project.task(tasks[1].id)).await.expect("a task");
  server.retitle(tasks[0].id, "Sign in again");
  reader.until_seen(&server).await;
  within(reader.project.changes()).await.expect("the retitle");

  server.drop_streams();
  on_server(&server, |server| server.until_subscribed(2)).await;
  let reconnected = within(reader.project.changes()).await.expect("changes");
  assert!(reconnected.is_reset(), "a reconnect makes every read stale");
  assert!(read.is_stale(&reconnected));
  let resumed = server.cursor().to_string();
  assert_eq!(
    server.subscribed(),
    [None, Some(resumed)],
    "resumed from the last cursor"
  );

  let fresh = within(reader.project.task(tasks[1].id)).await.expect("a task");
  assert!(!fresh.is_stale(&reconnected), "read in the new epoch");
  server.reset_next();
  server.drop_streams();
  on_server(&server, |server| server.until_subscribed(3)).await;
  let reset = within(reader.project.changes()).await.expect("changes");
  assert!(reset.is_reset());
  assert!(fresh.is_stale(&reset));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_reads_revoked_membership_ends_the_stream_and_holds_back_late_answers() {
  let server = TeamServer::start(&[ALICE], TASKS);
  let reader = Reader::of(&server, ALICE);
  let task = server.tasks()[0].id;
  within(reader.project.task(task)).await.expect("a member reads");
  server.hold();
  let project = reader.project.clone();
  let held = tokio::spawn(async move { project.task(task).await });
  on_server(&server, |server| server.until_held(1)).await;
  server.revoke(ALICE);

  assert_eq!(
    within(reader.follow).await.expect("followed"),
    TeamReadError::MembershipRevoked
  );
  server.release();
  let late = within(held).await.expect("the read");
  assert_eq!(
    late.err(),
    Some(TeamReadError::MembershipRevoked),
    "no data after revocation"
  );
  assert_eq!(
    within(reader.project.changes()).await.err(),
    Some(TeamReadError::MembershipRevoked)
  );
  assert_eq!(
    within(reader.project.task(task)).await.err(),
    Some(TeamReadError::MembershipRevoked)
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_reads_a_changed_session_ends_the_stream_and_the_reads() {
  let server = TeamServer::start(&[ALICE], TASKS);
  let reader = Reader::of(&server, ALICE);
  let task = server.tasks()[0].id;
  within(reader.project.task(task)).await.expect("a task");
  reader.generation.send_replace(1);
  assert_eq!(
    within(reader.follow).await.expect("followed"),
    TeamReadError::SessionChanged
  );
  assert_eq!(
    within(reader.project.task(task)).await.err(),
    Some(TeamReadError::SessionChanged)
  );
  assert_eq!(
    within(reader.project.changes()).await.err(),
    Some(TeamReadError::SessionChanged)
  );
}
