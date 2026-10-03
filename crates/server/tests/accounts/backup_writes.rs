use std::{
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use rusqlite::Connection;
use tokio::{sync::oneshot, task::JoinHandle};

use crate::support::TestServer;

struct Writer {
  stop: Arc<AtomicBool>,
  task: Option<JoinHandle<usize>>,
}

impl Writer {
  async fn start(path: PathBuf) -> Self {
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let (ready, started) = oneshot::channel();
    let task = tokio::task::spawn_blocking(move || {
      let db = Connection::open(path).expect("writer database");
      db.busy_timeout(Duration::from_secs(5)).expect("writer timeout");
      let mut ready = Some(ready);
      let mut writes = 0;
      while !stopping.load(Ordering::Acquire) {
        db.execute("UPDATE backup_counter SET value=value+1", [])
          .expect("continuous commit");
        writes += 1;
        if let Some(ready) = ready.take() {
          ready.send(()).expect("waiting for writer");
        }
      }
      writes
    });
    let writer = Self { stop, task: Some(task) };
    started.await.expect("writer started");
    writer
  }

  async fn finish(mut self) -> usize {
    self.stop.store(true, Ordering::Release);
    self.task.take().expect("writer task").await.expect("join writer")
  }
}

impl Drop for Writer {
  fn drop(&mut self) {
    // Also stop the blocking loop if the backup test times out or panics.
    self.stop.store(true, Ordering::Release);
  }
}

#[tokio::test]
async fn s1a_large_online_backup_completes_during_continuous_wal_commits() {
  let server = TestServer::start(|_| {}).await;
  let db = server.database();
  db.execute_batch(
    "CREATE TABLE backup_payload(data BLOB); INSERT INTO backup_payload VALUES(randomblob(8388608));
    CREATE TABLE backup_counter(value INTEGER); INSERT INTO backup_counter VALUES(0);",
  )
  .expect("large backup fixture");
  let pages: i64 = db
    .query_row("PRAGMA page_count", [], |row| row.get(0))
    .expect("page count");
  assert!(pages > 100);
  let writer = Writer::start(server.directory.path().join("orchester-server.db")).await;
  let path = server.directory.path().join("busy-snapshot.db");
  let result = tokio::time::timeout(Duration::from_secs(5), server.server.host_backup(path.clone())).await;
  let writes = writer.finish().await;
  result
    .expect("backup cannot restart indefinitely under writes")
    .expect("completed snapshot");
  assert!(writes > 1);
  let snapshot = Connection::open(path).expect("snapshot");
  let integrity: String = snapshot
    .query_row("PRAGMA integrity_check", [], |row| row.get(0))
    .expect("integrity");
  assert_eq!(integrity, "ok");
  let copied: i64 = snapshot
    .query_row("SELECT length(data) FROM backup_payload", [], |row| row.get(0))
    .expect("payload");
  assert_eq!(copied, 8388608);
  let at_backup: i64 = snapshot
    .query_row("SELECT value FROM backup_counter", [], |row| row.get(0))
    .expect("snapshot counter");
  assert!(at_backup > 0 && at_backup <= writes as i64);
  drop(snapshot);
  drop(db);
  server.finish().await;
}
