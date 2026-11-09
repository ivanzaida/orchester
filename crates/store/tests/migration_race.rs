//! Openers of one project database that meet a schema behind (the installed app's threads and the project's runner
//! process, on the first start after an update): each migration is applied once, and the openers that waited for the
//! write lock see the new version instead of applying it again. Run with `cargo test -p orchester-store --test
//! migration_race migration_race`.

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
  sync::{Arc, Barrier},
  thread,
  time::{Duration, Instant},
};

use orchester_core::id::ProjectId;
use orchester_store::ProjectDb;
use rusqlite::Connection;
use tempfile::TempDir;

/// The schema before the migration that creates `agent_events_by_run_kind` (a plain `CREATE INDEX`, which fails when
/// applied twice); the migrations are append only, so this stays the schema one of them is behind.
const BEHIND: u32 = 153;
const THREADS: usize = 8;
const PROCESSES: usize = 4;
const ROUNDS: usize = 5;
/// The environment variable that makes the test program act as one opener process, with the repository it opens.
const REPOSITORY: &str = "MIGRATION_RACE_REPOSITORY";
const OPENER: &str = "migration_race_opener_process";

fn database(repo: &Path) -> PathBuf {
  repo.join(".orchester").join("orchester.db")
}

/// A repository whose database is at schema `BEHIND`, with its project row.
fn repository_behind() -> TempDir {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let connection = ProjectDb::create_at_version(repo.path(), BEHIND).expect("an old database");
  connection
    .execute(
      "INSERT INTO project (id, key, name) VALUES (?1, 'ORC', 'Orchester')",
      [ProjectId::generate().to_string()],
    )
    .expect("the project row");
  assert_eq!(index_count(&connection), 0, "the schema is one migration behind");
  repo
}

fn index_count(connection: &Connection) -> i64 {
  connection
    .query_row(
      "SELECT COUNT(*) FROM sqlite_master WHERE name = 'agent_events_by_run_kind'",
      [],
      |row| row.get(0),
    )
    .expect("count")
}

/// What the test program does when it is started as an opener process: waits for the go-ahead file, then opens.
#[test]
fn migration_race_opener_process() {
  let Some(repository) = std::env::var_os(REPOSITORY).map(PathBuf::from) else {
    return;
  };
  let ready = repository.join("ready").join(std::process::id().to_string());
  fs::write(&ready, b"").expect("report ready");
  let go = repository.join("go");
  let deadline = Instant::now() + Duration::from_secs(60);
  while !go.exists() {
    assert!(Instant::now() < deadline, "the go-ahead never came");
    thread::sleep(Duration::from_millis(1));
  }
  ProjectDb::open(&repository).expect("an opener process opens the database");
}

fn spawn_openers(repo: &Path) -> Vec<std::process::Child> {
  fs::create_dir(repo.join("ready")).expect("ready folder");
  let exe = std::env::current_exe().expect("the test program");
  let children = (0..PROCESSES)
    .map(|_| {
      Command::new(&exe)
        .args(["--exact", OPENER, "--nocapture"])
        .env(REPOSITORY, repo)
        .spawn()
        .expect("start an opener process")
    })
    .collect();
  let deadline = Instant::now() + Duration::from_secs(60);
  while fs::read_dir(repo.join("ready")).expect("ready folder").count() < PROCESSES {
    assert!(Instant::now() < deadline, "the opener processes never got ready");
    thread::sleep(Duration::from_millis(5));
  }
  children
}

#[test]
fn migration_race_threads_and_processes_apply_a_migration_once() {
  for round in 0..ROUNDS {
    let repo = repository_behind();
    let children = spawn_openers(repo.path());
    let start = Arc::new(Barrier::new(THREADS + 1));
    let threads: Vec<_> = (0..THREADS)
      .map(|_| {
        let (start, path) = (Arc::clone(&start), repo.path().to_path_buf());
        thread::spawn(move || {
          start.wait();
          ProjectDb::open(&path).map(drop)
        })
      })
      .collect();
    start.wait();
    fs::write(repo.path().join("go"), b"").expect("let the opener processes go");
    for thread in threads {
      thread
        .join()
        .expect("an opener thread")
        .unwrap_or_else(|error| panic!("round {round}: a thread failed to open: {error}"));
    }
    for mut child in children {
      let status = child.wait().expect("an opener process ends");
      assert!(
        status.success(),
        "round {round}: an opener process failed to open: {status}"
      );
    }
    let connection = Connection::open(database(repo.path())).expect("open to check");
    assert_eq!(index_count(&connection), 1, "round {round}");
  }
}

#[test]
fn migration_race_a_newer_schema_is_still_refused_after_waiting() {
  let repo = repository_behind();
  let connection = Connection::open(database(repo.path())).expect("open to change");
  connection
    .pragma_update(None, "user_version", 100_000)
    .expect("a version of a newer build");
  drop(connection);
  assert!(
    ProjectDb::open(repo.path()).is_err(),
    "a database of a newer build is refused"
  );
}
