//! Reads in flight across invalidations: the server answers a held task read with what it read before a change, and
//! that answer is never returned as current; an invalidation of another scope does not cost a read again.

use super::{ALICE, Reader, TASKS, on_server, server::TeamServer, within};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_reads_an_invalidation_in_flight_never_makes_an_older_revision_current() {
  let server = TeamServer::start(&[ALICE], TASKS);
  let reader = Reader::of(&server, ALICE);
  let task = server.tasks()[0].id;
  reader.until_seen(&server).await;
  server.hold();
  let project = reader.project.clone();
  let read = tokio::spawn(async move { project.task(task).await });
  on_server(&server, |server| server.until_held(1)).await;
  server.retitle(task, "Sign in with passkeys");
  reader.until_seen(&server).await;
  server.release();

  let read = within(read).await.expect("the read").expect("a task");
  assert_eq!(read.value().task.content.title, "Sign in with passkeys");
  assert_eq!(read.value().task.revision.get(), 2);
  assert_eq!(
    server.task_reads(),
    2,
    "the answer from before the change was read again"
  );
  assert_eq!(read.taken().position().get(), server.cursor());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_reads_an_invalidation_of_another_scope_keeps_the_read() {
  let server = TeamServer::start(&[ALICE], TASKS);
  let reader = Reader::of(&server, ALICE);
  let tasks = server.tasks();
  let (task, other) = (tasks[0].id, tasks[1].id);
  server.hold();
  let project = reader.project.clone();
  let read = tokio::spawn(async move { project.task(task).await });
  on_server(&server, |server| server.until_held(1)).await;
  server.retitle(other, "Sign out everywhere");
  reader.until_seen(&server).await;
  server.release();

  let read = within(read).await.expect("the read").expect("a task");
  assert_eq!(read.value().task.content.title, "Sign in");
  assert_eq!(server.task_reads(), 1);
  assert!(
    read.taken().position().get() < server.cursor(),
    "taken where it was issued, before the other task's change"
  );
}
