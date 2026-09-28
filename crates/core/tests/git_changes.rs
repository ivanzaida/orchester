//! A repository's changes read with git, against real temporary repositories: the diff between
//! two commits (a modified, added, deleted, renamed and binary file), stored once per pair and
//! reused; a change too large to read; what a worktree holds beyond its commit, untracked files
//! included; and git's refusals.

use std::{
  cell::{Cell, RefCell},
  collections::HashMap,
  convert::Infallible,
  fs,
  io::ErrorKind,
  path::{Path, PathBuf},
  process,
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  diff::{
    Changes, Diff, FileChange, MAX_DIFF_FILES, RevisionDiff,
    git::{
      GitDiffError, RevisionChanges, RevisionDiffError, WorktreeRead, parse_numstat, read_revision_diff,
      read_worktree_changes,
    },
  },
  revision::Revision,
  store::RevisionDiffStore,
  time::Timestamp,
};
use tempfile::TempDir;

/// Runs programs as child processes and counts them.
#[derive(Default)]
struct Processes {
  ran: Cell<usize>,
}

impl CommandRunner for Processes {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    self.ran.set(self.ran.get() + 1);
    let mut child = process::Command::new(command.program());
    child.args(command.args()).envs(command.env().iter().cloned());
    if let Some(directory) = command.current_dir() {
      child.current_dir(directory);
    }
    let output = child.output().map_err(|error| match error.kind() {
      ErrorKind::NotFound => CommandError::NotFound,
      _ => CommandError::Failed(error.to_string()),
    })?;
    Ok(CommandOutput {
      code: output.status.code(),
      stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
      stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
  }
}

#[derive(Default)]
struct Diffs(RefCell<HashMap<(Revision, Revision), RevisionDiff>>);

impl RevisionDiffStore for Diffs {
  type Error = Infallible;

  fn save_revision_diff(&mut self, diff: &RevisionDiff) -> Result<(), Infallible> {
    let key = (diff.base.clone(), diff.head.clone());
    self.0.borrow_mut().insert(key, diff.clone());
    Ok(())
  }

  fn revision_diff(&self, base: &Revision, head: &Revision) -> Result<Option<RevisionDiff>, Infallible> {
    Ok(self.0.borrow().get(&(base.clone(), head.clone())).cloned())
  }
}

fn git() -> PathBuf {
  PathBuf::from("git")
}

fn now() -> Timestamp {
  Timestamp::from_unix_millis(1_700_000_000_000)
}

/// `git <args>` in `repo`, which must succeed; its output.
fn run_git(repo: &Path, args: &[&str]) -> String {
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
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A repository whose first commit has `README.md`, `notes.txt`, `old.rs` and `logo.png`.
fn repository() -> (TempDir, Revision) {
  let dir = TempDir::new().expect("temporary directory");
  let repo = dir.path();
  run_git(repo, &["init", "--quiet", "--initial-branch=main"]);
  for (key, value) in [
    ("user.name", "Orchester Test"),
    ("user.email", "test@orchester.invalid"),
    ("commit.gpgsign", "false"),
    ("core.autocrlf", "false"),
  ] {
    run_git(repo, &["config", key, value]);
  }
  fs::write(repo.join("README.md"), "# Kontur\n").expect("write");
  fs::write(repo.join("notes.txt"), "remove me\n").expect("write");
  fs::write(repo.join("old.rs"), "fn main() {\n  println!(\"moved\");\n}\n").expect("write");
  fs::write(repo.join("logo.png"), [0_u8, 1, 2, 3, 0]).expect("write");
  let base = commit(repo, "initial");
  (dir, base)
}

fn commit(repo: &Path, message: &str) -> Revision {
  run_git(repo, &["add", "--all"]);
  run_git(repo, &["commit", "--quiet", "-m", message]);
  Revision::new(&run_git(repo, &["rev-parse", "HEAD"])).expect("a revision")
}

/// Modifies, adds, deletes, renames and changes a binary file, and commits.
fn change(repo: &Path) -> Revision {
  fs::write(repo.join("README.md"), "# Kontur\n\nGitHub sign-in.\n").expect("write");
  fs::create_dir_all(repo.join("src/auth")).expect("create");
  fs::write(repo.join("src/auth/github.rs"), "pub fn sign_in() {}\n").expect("write");
  fs::remove_file(repo.join("notes.txt")).expect("remove");
  run_git(repo, &["mv", "old.rs", "main.rs"]);
  fs::write(repo.join("logo.png"), [0_u8, 9, 9, 0]).expect("write");
  commit(repo, "change")
}

#[test]
fn git_changes_between_two_commits_read_every_kind_of_change() {
  let (dir, base) = repository();
  let head = change(dir.path());
  let (runner, mut store) = (Processes::default(), Diffs::default());
  let read = read_revision_diff(&runner, &git(), dir.path(), &mut store, (&base, &head), now()).expect("read");
  let RevisionChanges::Read(diff) = read else {
    panic!("a small change is read: {read:?}");
  };
  assert_eq!((&diff.base, &diff.head, diff.read_at), (&base, &head, now()));
  let parsed = Diff::parse(&diff.text).expect("a unified diff");
  let changes: Vec<(&str, &FileChange, bool)> = parsed
    .files()
    .iter()
    .map(|file| (file.path(), file.change(), file.is_binary()))
    .collect();
  assert_eq!(
    changes,
    [
      ("README.md", &FileChange::Modified, false),
      ("logo.png", &FileChange::Modified, true),
      ("main.rs", &FileChange::Renamed { similarity: Some(100) }, false),
      ("notes.txt", &FileChange::Deleted, false),
      ("src/auth/github.rs", &FileChange::Added, false),
    ]
  );
  assert_eq!((parsed.additions(), parsed.deletions()), (3, 1));
  let renamed = &parsed.files()[2];
  assert_eq!(renamed.old_path(), Some("old.rs"));
}

#[test]
fn git_changes_between_two_commits_are_stored_and_reused() {
  let (dir, base) = repository();
  let head = change(dir.path());
  let (runner, mut store) = (Processes::default(), Diffs::default());
  let first = read_revision_diff(&runner, &git(), dir.path(), &mut store, (&base, &head), now()).expect("read");
  assert_eq!(runner.ran.get(), 2, "the size, then the diff");
  let later = Timestamp::from_unix_millis(now().unix_millis() + 1);
  let again = read_revision_diff(&runner, &git(), dir.path(), &mut store, (&base, &head), later).expect("read");
  assert_eq!(runner.ran.get(), 2, "the stored diff is reused without git");
  assert_eq!(first, again);
}

#[test]
fn git_changes_too_large_are_measured_not_read() {
  let (dir, base) = repository();
  for index in 0..=MAX_DIFF_FILES {
    fs::write(dir.path().join(format!("file-{index}.txt")), "line\n").expect("write");
  }
  let head = commit(dir.path(), "many files");
  let (runner, mut store) = (Processes::default(), Diffs::default());
  let read = read_revision_diff(&runner, &git(), dir.path(), &mut store, (&base, &head), now()).expect("read");
  let files = MAX_DIFF_FILES + 1;
  assert_eq!(
    read,
    RevisionChanges::TooLarge(Changes {
      files,
      additions: files,
      deletions: 0
    })
  );
  assert_eq!(runner.ran.get(), 1, "only the size was read");
  assert_eq!(store.revision_diff(&base, &head), Ok(None));
}

#[test]
fn git_changes_of_an_unknown_commit_are_git_failures() {
  let (dir, base) = repository();
  let missing = Revision::new(&"b".repeat(40)).expect("a revision");
  let (runner, mut store) = (Processes::default(), Diffs::default());
  let error = read_revision_diff(&runner, &git(), dir.path(), &mut store, (&base, &missing), now())
    .expect_err("the commit does not exist");
  assert!(
    matches!(error, RevisionDiffError::Git(GitDiffError::Failed(_))),
    "{error:?}"
  );
  let absent = read_revision_diff(
    &runner,
    &dir.path().join("no-git"),
    dir.path(),
    &mut store,
    (&base, &missing),
    now(),
  )
  .expect_err("no git program");
  assert!(
    matches!(
      absent,
      RevisionDiffError::Git(GitDiffError::Run(CommandError::NotFound))
    ),
    "{absent:?}"
  );
}

#[test]
fn git_changes_in_a_worktree_include_untracked_files() {
  let (dir, base) = repository();
  let repo = dir.path();
  let runner = Processes::default();
  let clean = read_worktree_changes(&runner, &git(), repo).expect("read");
  assert_eq!((clean.head, clean.read), (base.clone(), WorktreeRead::Clean));

  fs::write(repo.join("README.md"), "# Kontur\nnot committed\n").expect("write");
  run_git(repo, &["mv", "old.rs", "moved.rs"]);
  fs::create_dir_all(repo.join("docs")).expect("create");
  fs::write(repo.join("docs/plan with space.md"), "one\ntwo\n").expect("write");
  fs::write(repo.join("capture.bin"), [0_u8, 1, 0]).expect("write");
  fs::write(repo.join("empty.txt"), "").expect("write");
  let changed = read_worktree_changes(&runner, &git(), repo).expect("read");
  assert_eq!(changed.head, base);
  let WorktreeRead::Read(text) = changed.read else {
    panic!("the changes are read: {:?}", changed.read);
  };
  let parsed = Diff::parse(&text).expect("a unified diff");
  let changes: Vec<(&str, &FileChange, bool)> = parsed
    .files()
    .iter()
    .map(|file| (file.path(), file.change(), file.is_binary()))
    .collect();
  assert_eq!(
    changes,
    [
      ("README.md", &FileChange::Modified, false),
      ("moved.rs", &FileChange::Renamed { similarity: Some(100) }, false),
      ("capture.bin", &FileChange::Added, true),
      ("docs/plan with space.md", &FileChange::Added, false),
      ("empty.txt", &FileChange::Added, false),
    ]
  );
  assert_eq!((parsed.additions(), parsed.deletions()), (3, 0));
}

#[test]
fn git_changes_in_a_worktree_with_too_many_files_are_not_read() {
  let (dir, base) = repository();
  for index in 0..=MAX_DIFF_FILES {
    fs::write(dir.path().join(format!("untracked-{index}.txt")), "line\n").expect("write");
  }
  let runner = Processes::default();
  let changes = read_worktree_changes(&runner, &git(), dir.path()).expect("read");
  assert_eq!(changes.head, base);
  assert_eq!(
    changes.read,
    WorktreeRead::TooLarge(Changes {
      files: MAX_DIFF_FILES + 1,
      additions: 0,
      deletions: 0
    })
  );
  assert_eq!(
    runner.ran.get(),
    3,
    "HEAD, the tracked changes' size and the untracked files"
  );
}

#[test]
fn git_numstat_counts_renamed_and_binary_files() {
  let listed = "1\t0\tREADME.md\0-\t-\tlogo.png\0\
                0\t0\t\0old.rs\0main.rs\0";
  assert_eq!(
    parse_numstat(listed),
    Ok(Changes {
      files: 3,
      additions: 1,
      deletions: 0
    })
  );
  assert_eq!(parse_numstat(""), Ok(Changes::default()));
  assert!(matches!(
    parse_numstat("one\ttwo\tthree\0"),
    Err(GitDiffError::Unrecognized { .. })
  ));
}
