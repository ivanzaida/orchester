//! Pull request diffs per head revision, their text in content-addressed blob files.

use std::{fs, path::Path};

use orchester_core::{
  connection::GitHubHost,
  diff::RevisionDiff,
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  repository::GitHubRepo,
  revision::Revision,
  store::PullRequestDiffStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use tempfile::TempDir;

const DIFF: &str =
  "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n";

fn project_repo() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

fn kontur() -> GitHubRepo {
  GitHubRepo::new(GitHubHost::github_com(), "ivanzaida", "kontur").expect("valid repository")
}

fn revision(digit: char) -> Revision {
  Revision::new(&digit.to_string().repeat(40)).expect("valid revision")
}

fn diff(head: char, text: &str, synced_at: i64) -> RevisionDiff {
  RevisionDiff {
    base: revision('b'),
    head: revision(head),
    text: text.to_owned(),
    read_at: Timestamp::from_unix_millis(synced_at),
  }
}

/// The blob files under `.orchester/blobs`.
fn blob_files(repo: &Path) -> Vec<std::path::PathBuf> {
  let blobs = repo.join(".orchester").join("blobs");
  let Ok(directories) = fs::read_dir(&blobs) else {
    return Vec::new();
  };
  directories
    .flat_map(|directory| fs::read_dir(directory.expect("entry").path()).expect("blob directory"))
    .map(|file| file.expect("entry").path())
    .collect()
}

#[test]
fn diff_round_trips_through_a_compressed_blob() {
  let (repo, mut db) = project_repo();
  let stored = diff('1', DIFF, 10);
  db.save_pull_request_diff(&kontur(), 42, &stored).expect("save");
  assert_eq!(
    db.pull_request_diff(&kontur(), 42, &revision('1')).expect("read"),
    Some(stored.clone())
  );
  let files = blob_files(repo.path());
  assert_eq!(files.len(), 1);
  // Named by the SHA-256 of the text, under its first two hex digits, and compressed.
  let name = files[0].file_name().expect("name").to_string_lossy().into_owned();
  let parent = files[0].parent().and_then(Path::file_name).expect("parent");
  assert_eq!((name.len(), &name[..2]), (64, parent.to_str().expect("utf-8")));
  assert_ne!(fs::read(&files[0]).expect("blob"), DIFF.as_bytes());
  // Reopening reads the same diff.
  drop(db);
  let reopened = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(
    reopened.pull_request_diff(&kontur(), 42, &revision('1')).expect("read"),
    Some(stored)
  );
}

#[test]
fn equal_texts_share_one_blob() {
  let (repo, mut db) = project_repo();
  db.save_pull_request_diff(&kontur(), 42, &diff('1', DIFF, 10))
    .expect("save");
  db.save_pull_request_diff(&kontur(), 43, &diff('2', DIFF, 20))
    .expect("save");
  assert_eq!(blob_files(repo.path()).len(), 1);
}

#[test]
fn latest_diff_is_the_most_recently_synced_head() {
  let (_repo, mut db) = project_repo();
  assert_eq!(db.latest_pull_request_diff(&kontur(), 42).expect("read"), None);
  db.save_pull_request_diff(&kontur(), 42, &diff('1', DIFF, 10))
    .expect("save");
  db.save_pull_request_diff(&kontur(), 42, &diff('2', "", 20))
    .expect("save");
  let latest = db.latest_pull_request_diff(&kontur(), 42).expect("read");
  assert_eq!(latest.map(|diff| diff.head), Some(revision('2')));
  // Saving a head again marks it synced again.
  db.save_pull_request_diff(&kontur(), 42, &diff('1', DIFF, 30))
    .expect("save");
  let latest = db.latest_pull_request_diff(&kontur(), 42).expect("read");
  assert_eq!(latest.map(|diff| diff.head), Some(revision('1')));
  assert_eq!(db.latest_pull_request_diff(&kontur(), 7).expect("read"), None);
}

#[test]
fn a_damaged_blob_is_reported_not_returned() {
  let (repo, mut db) = project_repo();
  db.save_pull_request_diff(&kontur(), 42, &diff('1', DIFF, 10))
    .expect("save");
  let file = blob_files(repo.path()).remove(0);
  let other = zstd::encode_all("something else".as_bytes(), 3).expect("compress");
  fs::write(&file, other).expect("overwrite");
  assert!(matches!(
    db.pull_request_diff(&kontur(), 42, &revision('1')),
    Err(StoreError::Corrupt { what: "blob", .. })
  ));
}
