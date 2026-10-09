//! What the seed refuses before it writes anything: a live scenario without the fake Codex, a repository
//! folder that holds anything, and a data folder whose application database exists.

use orchester_core::time::Timestamp;

use crate::kontur::{self, SeedError, SeedRequest, scenarios};

#[test]
fn seed_live_needs_the_fake_codex_and_writes_nothing_without_it() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let repo = folder.path().join("repo-live");
  let request = SeedRequest {
    data_dir: folder.path().join("bin"),
    repo: repo.clone(),
    scenario: scenarios::named("live").expect("live"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  assert!(matches!(kontur::seed(&request), Err(SeedError::FakeCodexRequired)));
  assert!(!repo.exists());
}

#[test]
fn seed_never_writes_into_a_folder_that_holds_anything() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let repo = folder.path().join("repo");
  std::fs::create_dir(&repo).expect("folder");
  std::fs::write(repo.join("notes.txt"), "mine").expect("a file");
  let request = SeedRequest {
    data_dir: folder.path().join("bin"),
    repo: repo.clone(),
    scenario: scenarios::named("empty").expect("empty"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  assert!(matches!(kontur::seed(&request), Err(SeedError::RepoNotEmpty(_))));
  assert!(!repo.join(".git").exists());
  assert_eq!(std::fs::read_to_string(repo.join("notes.txt")).expect("kept"), "mine");
}

#[test]
fn seed_never_registers_in_an_application_database_that_exists() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let bin = folder.path().join("bin");
  std::fs::create_dir(&bin).expect("folder");
  let existing = bin.join("orchester.db");
  std::fs::write(&existing, "an install's database").expect("a file");
  let repo = folder.path().join("repo");
  let request = SeedRequest {
    data_dir: bin,
    repo: repo.clone(),
    scenario: scenarios::named("empty").expect("empty"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  assert!(matches!(kontur::seed(&request), Err(SeedError::AppDatabaseExists(path)) if path == existing));
  assert!(!repo.exists(), "nothing was seeded");
  assert_eq!(
    std::fs::read_to_string(&existing).expect("kept"),
    "an install's database"
  );
}
