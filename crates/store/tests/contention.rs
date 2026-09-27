//! Opening and reading the project database while the other process (desktop or runner) holds it:
//! the read waits for a lock, and for the shared memory of the write-ahead log that a crashed
//! runner still maps, then succeeds once they are released. A real error comes back at once.

use std::{
  fs,
  path::{Path, PathBuf},
  sync::mpsc,
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
};
use orchester_store::{ProjectDb, StoreError};
use tempfile::TempDir;

/// An open that has not ended after this long waits: it gives up only after five seconds.
const STILL_WAITING: Duration = Duration::from_millis(300);
/// An open ends within this once what it waits for is released.
const PATIENCE: Duration = Duration::from_secs(30);
/// An error that comes back within this, long before an open gives up waiting, was not retried.
const AT_ONCE: Duration = Duration::from_secs(2);

fn project() -> TempDir {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  drop(ProjectDb::create(repo.path(), project).expect("create"));
  repo
}

fn database(repo: &Path) -> PathBuf {
  repo.join(".orchester").join("orchester.db")
}

/// Opens the project database of `repo` on a thread of its own, which reads the project's name.
fn read_elsewhere(repo: &Path) -> mpsc::Receiver<Result<String, StoreError>> {
  let (read, name) = mpsc::channel();
  let repo = repo.to_path_buf();
  thread::spawn(move || {
    let result = ProjectDb::open(&repo).map(|db| db.project().name().as_str().to_owned());
    read.send(result).expect("the test waits for the read");
  });
  name
}

/// Checks that the read is still waiting for `what`.
fn assert_waits(name: &mpsc::Receiver<Result<String, StoreError>>, what: &str) {
  match name.recv_timeout(STILL_WAITING) {
    Err(mpsc::RecvTimeoutError::Timeout) => {}
    ended => panic!("the read does not wait for {what}: {ended:?}"),
  }
}

#[test]
fn contention_a_read_waits_for_a_locked_database_and_then_succeeds() {
  let repo = project();
  let writer = rusqlite::Connection::open(database(repo.path())).expect("open");
  // In exclusive locking mode the writer keeps the database locked, for readers too, until it
  // closes.
  writer
    .pragma_update(None, "locking_mode", "EXCLUSIVE")
    .expect("exclusive");
  writer
    .execute_batch("BEGIN EXCLUSIVE; UPDATE project SET name = 'Kontur Two'; COMMIT;")
    .expect("write");
  let name = read_elsewhere(repo.path());
  assert_waits(&name, "the lock");
  drop(writer);
  let read = name.recv_timeout(PATIENCE).expect("the read ends in time");
  assert_eq!(read.expect("the read succeeds"), "Kontur Two");
}

/// A process that holds the file `path` mapped, as a runner that was killed still does for a
/// moment after it let go of its locks on the database.
#[cfg(windows)]
struct Mapped(std::process::Child);

#[cfg(windows)]
impl Mapped {
  fn map(path: &Path) -> Self {
    use std::{
      io::{BufRead, BufReader},
      process::{Command, Stdio},
    };
    let script = format!(
      "$ErrorActionPreference = 'Stop'; \
       $stream = [System.IO.File]::Open('{}', 'Open', 'ReadWrite', 'ReadWrite, Delete'); \
       $map = [System.IO.MemoryMappedFiles.MemoryMappedFile]::CreateFromFile($stream, [NullString]::Value, 0, 'ReadWrite', $null, 'None', $false); \
       $view = $map.CreateViewAccessor(); \
       [Console]::Out.WriteLine('mapped'); [Console]::Out.Flush(); \
       $null = [Console]::In.ReadLine(); \
       $view.Dispose(); $map.Dispose()",
      path.display()
    );
    let mut child = Command::new("powershell")
      .args(["-NoProfile", "-NonInteractive", "-Command", &script])
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .spawn()
      .expect("start powershell");
    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().expect("piped"))
      .read_line(&mut line)
      .expect("read powershell's output");
    assert_eq!(line.trim(), "mapped");
    Self(child)
  }

  /// Unmaps the file and waits for the process to end.
  fn unmap(mut self) {
    drop(self.0.stdin.take());
    let status = self.0.wait().expect("wait for powershell");
    assert!(status.success(), "powershell: {status}");
  }
}

#[cfg(windows)]
#[test]
fn contention_a_read_waits_for_the_shared_memory_a_killed_runner_still_maps() {
  let repo = project();
  // What a runner killed mid-write leaves behind: the log's shared memory, still mapped by the
  // process as it goes away.
  let shared_memory = repo.path().join(".orchester").join("orchester.db-shm");
  fs::write(&shared_memory, vec![0; 32 * 1024]).expect("write the shared memory");
  let mapped = Mapped::map(&shared_memory);
  let name = read_elsewhere(repo.path());
  assert_waits(&name, "the map to go");
  mapped.unmap();
  let read = name.recv_timeout(PATIENCE).expect("the read ends in time");
  assert_eq!(read.expect("the read succeeds"), "Kontur");
}

#[test]
fn contention_a_real_error_comes_back_at_once() {
  let damaged = project();
  fs::write(database(damaged.path()), vec![b'x'; 4096]).expect("overwrite the database");
  let started = Instant::now();
  let error = ProjectDb::open(damaged.path()).err().expect("not a database");
  assert!(started.elapsed() < AT_ONCE, "took {:?}", started.elapsed());
  assert!(
    matches!(&error, StoreError::Sqlite(rusqlite::Error::SqliteFailure(failure, _)) if failure.code == rusqlite::ErrorCode::NotADatabase),
    "{error}"
  );

  let newer = project();
  let connection = rusqlite::Connection::open(database(newer.path())).expect("open");
  connection
    .pragma_update(None, "user_version", 1000)
    .expect("set the version");
  drop(connection);
  let started = Instant::now();
  let error = ProjectDb::open(newer.path()).err().expect("written by a newer build");
  assert!(started.elapsed() < AT_ONCE, "took {:?}", started.elapsed());
  assert!(matches!(error, StoreError::SchemaTooNew { found: 1000, .. }), "{error}");
}
