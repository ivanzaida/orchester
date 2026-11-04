//! The code viewer's side of a run's files asked of the runner: a listing comes back page by page and is read whole,
//! a name a shifted page repeats is kept once, and a run's version the runner cannot answer for is left out of
//! another page's version switch, as a worktree that is not there is, while the run's own page still fails.

use std::{fs, path::Path, process};

use orchester_core::{command::CommandRunner, id::RunId};
use orchester_exec::programs::Programs;
use orchester_ipc::run_files::{RunFilesReply, RunFilesRequest};
use orchester_workspace::files::{FilesError, RunFiles, Version, list_files, versions};
use parking_lot::Mutex;
use tempfile::TempDir;

/// Answers a listing in the pages it holds, by where each starts, and records what was asked.
struct Paged {
  pages: Vec<(u64, Vec<&'static str>, Option<u64>)>,
  asked: Mutex<Vec<u64>>,
}

impl RunFiles for Paged {
  fn ask(&self, _run: RunId, request: RunFilesRequest) -> Result<RunFilesReply, FilesError> {
    let RunFilesRequest::List { from } = request else {
      panic!("only the listing is asked here: {request:?}");
    };
    self.asked.lock().push(from);
    let (_, files, next) = self
      .pages
      .iter()
      .find(|(start, ..)| *start == from)
      .unwrap_or_else(|| panic!("no page starts at {from}"));
    Ok(RunFilesReply::Listed {
      files: files.iter().map(|file| (*file).to_owned()).collect(),
      next: *next,
    })
  }
}

/// A runner that cannot be reached.
struct Unreachable;

impl RunFiles for Unreachable {
  fn ask(&self, _run: RunId, _request: RunFilesRequest) -> Result<RunFilesReply, FilesError> {
    Err(FilesError::Runner("the runner could not be started".to_owned()))
  }
}

fn git(repo: &Path, args: &[&str]) {
  let output = process::Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

/// A repository on `main` with `README.md` committed.
fn repository() -> TempDir {
  let dir = tempfile::tempdir().expect("a temporary folder");
  let repo = dir.path();
  git(repo, &["init", "--quiet", "--initial-branch=main"]);
  git(repo, &["config", "user.name", "Orchester Test"]);
  git(repo, &["config", "user.email", "test@orchester.invalid"]);
  fs::write(repo.join("README.md"), "# Kontur\n").expect("README");
  git(repo, &["add", "."]);
  git(repo, &["commit", "--quiet", "-m", "first"]);
  dir
}

#[test]
fn run_files_a_listing_is_read_page_by_page_and_a_repeated_name_is_kept_once() {
  let dir = repository();
  let programs = Programs::processes();
  let runs = Paged {
    pages: vec![
      (0, vec!["src/main.rs", "notes.md"], Some(2)),
      // The worktree gained a file between the pages: the second starts with the first's last name again.
      (2, vec!["notes.md", "src/lib.rs"], Some(4)),
      (4, vec!["data.bin"], None),
    ],
    asked: Mutex::new(Vec::new()),
  };
  let list = list_files(
    (programs.runner() as &dyn CommandRunner, &runs),
    dir.path(),
    &Version::Run(RunId::generate()),
  )
  .expect("the run's files");
  assert_eq!(*runs.asked.lock(), [0, 2, 4]);
  assert_eq!(list.len(), 4);
  for file in ["src/main.rs", "notes.md", "src/lib.rs", "data.bin"] {
    assert!(list.contains(file), "{file} is listed");
  }
}

#[test]
fn run_files_a_page_that_does_not_move_on_fails_instead_of_asking_forever() {
  let dir = repository();
  let programs = Programs::processes();
  let runs = Paged {
    pages: vec![(0, vec!["a.rs"], Some(0))],
    asked: Mutex::new(Vec::new()),
  };
  let listed = list_files(
    (programs.runner() as &dyn CommandRunner, &runs),
    dir.path(),
    &Version::Run(RunId::generate()),
  );
  assert!(matches!(listed, Err(FilesError::Runner(_))), "{listed:?}");
}

#[test]
fn run_files_a_run_the_runner_cannot_answer_for_is_left_out_of_another_pages_switch() {
  let dir = repository();
  let programs = Programs::processes();
  let readers = (programs.runner() as &dyn CommandRunner, &Unreachable as &dyn RunFiles);
  let run = RunId::generate();
  let choices = versions(readers, dir.path(), (&Version::Checkout, Some(run)), None).expect("versions");
  let listed: Vec<&Version> = choices.choices.iter().map(|choice| &choice.version).collect();
  assert_eq!(listed, [&Version::Checkout, &Version::Base]);

  let on_the_run = versions(readers, dir.path(), (&Version::Run(run), Some(run)), None);
  assert!(matches!(on_the_run, Err(FilesError::Runner(_))), "{on_the_run:?}");
}
