//! Resolving a merge's conflicts by hand in the run's worktree, against real repositories: the base branch is merged
//! into the run's branch there with git's markers; the index is the truth of what is resolved; a binary file or one a
//! side deleted waits for a side to be kept; *Continue* commits only once every file is resolved; *Abort* puts the
//! worktree back; a worktree with uncommitted work, off the run's branch or past its checkpoint is not merged into.

use std::{fs, path::Path};

use orchester_core::{
  integration::resolve::{self, FileState, ResolveError, Side, Started, Undecided, holds_markers},
  revision::Revision,
  run::BranchName,
};

use crate::support::{Processes, RUN_BRANCH, Repo, git_in, git_program};

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("a branch name")
}

fn head(worktree: &Path) -> Revision {
  Revision::new(&git_in(worktree, &["rev-parse", "HEAD"])).expect("a commit")
}

/// A repository whose run worktree and `main` both changed `file` since the run started, as `(run's, main's)`, from
/// `first`; `None` deletes it on that side.
fn conflicting(file: &str, first: &[u8], (run, main): (Option<&[u8]>, Option<&[u8]>)) -> (Repo, std::path::PathBuf) {
  let repo = Repo::new();
  fs::write(repo.root.join(file), first).expect("write");
  git_in(&repo.root, &["add", "--all"]);
  git_in(&repo.root, &["commit", "--quiet", "-m", "the file"]);
  let base = repo.head();
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  let side = |dir: &Path, content: Option<&[u8]>, message: &str| {
    match content {
      Some(content) => fs::write(dir.join(file), content).expect("write"),
      None => fs::remove_file(dir.join(file)).expect("delete"),
    }
    git_in(dir, &["add", "--all"]);
    git_in(dir, &["commit", "--quiet", "-m", message]);
  };
  side(&worktree, run, "Implement: the run's change");
  side(&repo.root, main, "main's change");
  (repo, worktree)
}

/// Both appended a line to the end of `list.txt`.
fn appended() -> (Repo, std::path::PathBuf) {
  conflicting(
    "list.txt",
    b"first\n",
    (Some(b"first\nrun's\n"), Some(b"first\nmain's\n")),
  )
}

fn start(worktree: &Path) -> Result<Started, ResolveError> {
  resolve::start(
    &Processes,
    &git_program(),
    worktree,
    (&branch("main"), &branch(RUN_BRANCH), &head(worktree)),
  )
}

fn finish(worktree: &Path, expected: &Revision) -> Result<resolve::MergedIn, ResolveError> {
  resolve::finish(&Processes, &git_program(), worktree, (&branch("main"), expected))
}

fn states(worktree: &Path) -> Vec<(String, FileState)> {
  resolve::resolution(&Processes, &git_program(), worktree)
    .expect("read")
    .expect("a merge in progress")
    .files
    .into_iter()
    .map(|file| (file.path, file.state))
    .collect()
}

#[test]
fn resolve_merges_the_base_into_the_worktree_and_leaves_the_markers() {
  let (repo, worktree) = appended();
  let checkout = repo.head();
  assert_eq!(start(&worktree), Ok(Started::Conflicts(vec!["list.txt".to_owned()])));
  let text = fs::read_to_string(worktree.join("list.txt")).expect("the conflicted file");
  assert!(holds_markers(&text), "{text}");
  assert_eq!(states(&worktree), [("list.txt".to_owned(), FileState::Markers)]);
  // The person's checkout is not touched.
  assert_eq!(repo.head(), checkout);
  assert!(repo.is_clean());
  assert_eq!(start(&worktree), Err(ResolveError::InMerge));
}

#[test]
fn resolve_continue_refuses_while_markers_remain_and_commits_once_resolved() {
  let (repo, worktree) = appended();
  let expected = head(&worktree);
  start(&worktree).expect("merged with conflicts");
  assert_eq!(
    finish(&worktree, &expected),
    Err(ResolveError::MarkersLeft(vec!["list.txt".to_owned()]))
  );
  fs::write(worktree.join("list.txt"), "first\nmain's\nrun's\n").expect("resolved by hand");
  assert_eq!(
    states(&worktree),
    [("list.txt".to_owned(), FileState::Resolved)],
    "resolved by its text before it is added"
  );
  let merged = finish(&worktree, &expected).expect("committed");
  assert_eq!(merged.base.as_str(), repo.head());
  assert_eq!(merged.tip, head(&worktree));
  assert_eq!(resolve::resolution(&Processes, &git_program(), &worktree), Ok(None));
  assert_eq!(git_in(&worktree, &["status", "--porcelain"]), "");
  git_in(&worktree, &["merge-base", "--is-ancestor", &repo.head(), "HEAD"]);
  assert_eq!(
    git_in(&worktree, &["log", "-1", "--format=%s"]),
    format!("Merge main into {RUN_BRANCH}")
  );
}

#[test]
fn resolve_a_file_marked_resolved_is_resolved_whatever_its_text_holds() {
  let (_repo, worktree) = appended();
  let expected = head(&worktree);
  start(&worktree).expect("merged with conflicts");
  // A file about git's markers holds one as its content; once the person marks it resolved, the index decides.
  fs::write(
    worktree.join("list.txt"),
    "first\n<<<<<<< is how git marks a conflict\n",
  )
  .expect("written");
  assert_eq!(states(&worktree), [("list.txt".to_owned(), FileState::Markers)]);
  git_in(&worktree, &["add", "list.txt"]);
  assert_eq!(states(&worktree), [("list.txt".to_owned(), FileState::Resolved)]);
  finish(&worktree, &expected).expect("committed");
}

#[test]
fn resolve_a_binary_conflict_waits_for_a_side_to_be_kept() {
  let (_repo, worktree) = conflicting(
    "logo.bin",
    b"\x00\x01first",
    (Some(b"\x00\x01run's"), Some(b"\x00\x01main's")),
  );
  let expected = head(&worktree);
  start(&worktree).expect("merged with conflicts");
  assert_eq!(
    states(&worktree),
    [("logo.bin".to_owned(), FileState::Undecided(Undecided::Unmergeable))]
  );
  assert_eq!(
    finish(&worktree, &expected),
    Err(ResolveError::Undecided(vec!["logo.bin".to_owned()]))
  );
  resolve::decide(&Processes, &git_program(), &worktree, ("logo.bin", Side::Base)).expect("kept main's");
  assert_eq!(fs::read(worktree.join("logo.bin")).expect("kept"), b"\x00\x01main's");
  assert_eq!(states(&worktree), [("logo.bin".to_owned(), FileState::Resolved)]);
  finish(&worktree, &expected).expect("committed");
}

#[test]
fn resolve_a_file_main_deleted_keeps_the_runs_or_goes() {
  for (side, kept) in [(Side::Run, true), (Side::Base, false)] {
    let (_repo, worktree) = conflicting("notes.txt", b"first\n", (Some(b"first\nrun's\n"), None));
    let expected = head(&worktree);
    start(&worktree).expect("merged with conflicts");
    assert_eq!(
      states(&worktree),
      [("notes.txt".to_owned(), FileState::Undecided(Undecided::DeletedByBase))]
    );
    resolve::decide(&Processes, &git_program(), &worktree, ("notes.txt", side)).expect("decided");
    assert_eq!(states(&worktree), [("notes.txt".to_owned(), FileState::Resolved)]);
    finish(&worktree, &expected).expect("committed, a removed path too");
    assert_eq!(worktree.join("notes.txt").is_file(), kept, "{side:?}");
    assert_eq!(git_in(&worktree, &["status", "--porcelain"]), "");
  }
}

#[test]
fn resolve_a_file_the_run_deleted_waits_for_a_side() {
  let (_repo, worktree) = conflicting("notes.txt", b"first\n", (None, Some(b"first\nmain's\n")));
  start(&worktree).expect("merged with conflicts");
  assert_eq!(
    states(&worktree),
    [("notes.txt".to_owned(), FileState::Undecided(Undecided::DeletedByRun))]
  );
}

#[test]
fn resolve_abort_puts_the_worktree_back() {
  let (_repo, worktree) = appended();
  let before = head(&worktree);
  start(&worktree).expect("merged with conflicts");
  resolve::abort(&Processes, &git_program(), &worktree).expect("aborted");
  assert_eq!(head(&worktree), before);
  assert_eq!(git_in(&worktree, &["status", "--porcelain"]), "");
  assert_eq!(
    resolve::abort(&Processes, &git_program(), &worktree),
    Err(ResolveError::NotInMerge)
  );
}

#[test]
fn resolve_refuses_uncommitted_work_another_branch_and_a_branch_past_its_checkpoint() {
  let (_repo, worktree) = appended();
  let checkpoint = head(&worktree);
  fs::write(worktree.join("notes.txt"), "the agent's\n").expect("uncommitted work");
  assert_eq!(
    start(&worktree),
    Err(ResolveError::Uncommitted(vec!["notes.txt".to_owned()]))
  );
  git_in(&worktree, &["add", "--all"]);
  git_in(&worktree, &["commit", "--quiet", "-m", "not a checkpoint"]);
  assert_eq!(
    resolve::start(
      &Processes,
      &git_program(),
      &worktree,
      (&branch("main"), &branch(RUN_BRANCH), &checkpoint)
    ),
    Err(ResolveError::BranchMoved)
  );
  assert_eq!(
    resolve::start(
      &Processes,
      &git_program(),
      &worktree,
      (&branch("main"), &branch("someone-else"), &head(&worktree))
    ),
    Err(ResolveError::NotOnBranch)
  );
  assert_eq!(resolve::resolution(&Processes, &git_program(), &worktree), Ok(None));
}

#[test]
fn resolve_a_merge_without_conflicts_is_committed_at_once() {
  let repo = Repo::new();
  let base = repo.head();
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  repo.commit(&worktree, "feature.txt", "work\n", "Implement: add feature");
  repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  assert!(matches!(start(&worktree), Ok(Started::Merged(_))));
  assert!(worktree.join("other.txt").is_file());
}

#[test]
fn resolve_markers_are_told_at_the_start_of_a_line_only() {
  assert!(holds_markers("a\n<<<<<<< HEAD\nb\n=======\nc\n>>>>>>> main\n"));
  assert!(!holds_markers("Title\n=======\n\nText with <<<<<<< inside.\n"));
}
