//! What keeps a merge from reaching the person's work in ways git would allow: `merge.autoStash`,
//! ignored files and folders of untracked files in its way, the checkout moving while the merge
//! commit is made, a fast-forward that stops part way, the checkout's own hooks folder, and a folder
//! left where the temporary worktree goes.

use std::{fs, path::Path};

use orchester_core::{
  integration::{MergeError, MergeKind, merge_run},
  run::BranchName,
};

use crate::{
  merge::repo_with_run_work,
  support::{Ending, Processes, RUN_BRANCH, Repo, git_in, git_program, now, run, task},
};

/// Writes the hook `name` of the repository at `hooks` as a shell script.
fn hook(hooks: &Path, name: &str, script: &str) {
  fs::create_dir_all(hooks).expect("hooks folder");
  let path = hooks.join(name);
  fs::write(&path, format!("#!/bin/sh\n{script}")).expect("hook");
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable hook");
  }
}

/// A command a hook runs in the checkout at `root`, not in the worktree git runs the hook for.
fn in_checkout(root: &Path, command: &str) -> String {
  let root = root.to_str().expect("a Unicode temporary path");
  format!("unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX\ngit -C '{root}' {command}\n")
}

fn merge(repo: &Repo, base: &str) -> Result<orchester_core::integration::MergeOutcome, MergeError> {
  merge_run(
    &Processes,
    &git_program(),
    &repo.root,
    (&run(base, Some("main"), Ending::Succeeded), &task()),
    now(),
  )
}

/// Git lists no temporary worktree and the git folder holds none.
fn assert_no_temporary(repo: &Repo) {
  let listed = repo.git(&["worktree", "list", "--porcelain"]);
  assert!(!listed.contains("orchester-merge"), "{listed}");
  let names: Vec<String> = fs::read_dir(repo.root.join(".git"))
    .expect("the git folder")
    .map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned())
    .collect();
  assert!(
    names.iter().all(|name| !name.starts_with("orchester-merge")),
    "{names:?}"
  );
}

#[test]
fn merge_never_stashes_the_persons_work_with_merge_auto_stash_on() {
  let repo = Repo::new();
  let base = repo.head();
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  repo.commit(&worktree, "README.md", "the run's line\n", "Implement");
  repo.git(&["config", "merge.autoStash", "true"]);
  fs::write(repo.root.join("README.md"), "the person's line\n").expect("edit");
  let refused = merge(&repo, &base);
  assert_eq!(
    refused,
    Err(MergeError::UncommittedInTheWay {
      files: vec!["README.md".to_owned()]
    })
  );
  assert_eq!(repo.head(), base);
  assert_eq!(repo.git(&["stash", "list"]), "", "nothing was stashed");
  assert_eq!(
    fs::read_to_string(repo.root.join("README.md")).expect("kept"),
    "the person's line\n"
  );

  fs::write(repo.root.join("README.md"), "hello\n").expect("as committed");
  repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  fs::write(repo.root.join("notes.txt"), "untracked\n").expect("untracked");
  let merged = merge(&repo, &base).expect("merged beside the untracked file");
  assert_eq!(merged.merge.kind, MergeKind::MergeCommit);
  assert_eq!(repo.git(&["stash", "list"]), "");
  assert!(repo.root.join("notes.txt").is_file());
  assert_no_temporary(&repo);
}

#[test]
fn merge_refuses_an_ignored_file_in_its_way() {
  let repo = Repo::new();
  let base = repo.head();
  fs::write(repo.root.join(".git/info/exclude"), "/.orchester/\n*.log\n").expect("ignore logs");
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  fs::write(worktree.join("build.log"), "the run's\n").expect("log");
  git_in(&worktree, &["add", "--force", "build.log"]);
  git_in(&worktree, &["commit", "--quiet", "-m", "Implement"]);
  fs::write(repo.root.join("build.log"), "the person's\n").expect("ignored file");
  let refused = merge(&repo, &base);
  assert_eq!(
    refused,
    Err(MergeError::UncommittedInTheWay {
      files: vec!["build.log".to_owned()]
    })
  );
  assert_eq!(
    fs::read_to_string(repo.root.join("build.log")).expect("kept"),
    "the person's\n"
  );
}

#[test]
fn merge_refuses_to_replace_a_folder_that_holds_untracked_files() {
  let repo = Repo::new();
  fs::create_dir(repo.root.join("docs")).expect("folder");
  let base = repo.commit(&repo.root, "docs/guide.md", "guide\n", "a folder");
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  git_in(&worktree, &["rm", "--quiet", "-r", "docs"]);
  repo.commit(&worktree, "docs", "a file now\n", "Implement: docs becomes a file");
  fs::write(repo.root.join("docs/draft.md"), "untracked\n").expect("untracked in the folder");
  let refused = merge(&repo, &base);
  assert_eq!(
    refused,
    Err(MergeError::UncommittedInTheWay {
      files: vec!["docs".to_owned()]
    })
  );
  assert!(repo.root.join("docs/draft.md").is_file());
  assert_eq!(repo.head(), base);
}

#[test]
fn merge_refuses_a_checkout_switched_to_another_branch_while_the_merge_commit_was_made() {
  let (repo, base, _) = repo_with_run_work();
  let moved = repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  hook(
    &repo.root.join(".git/hooks"),
    "pre-merge-commit",
    &in_checkout(&repo.root, "switch --quiet -c elsewhere"),
  );
  let refused = merge(&repo, &base);
  assert_eq!(
    refused,
    Err(MergeError::WrongBranch {
      expected: BranchName::new("main").expect("valid"),
      current: Some(BranchName::new("elsewhere").expect("valid")),
    })
  );
  assert_eq!(repo.git(&["rev-parse", "main"]), moved, "main was not moved");
  assert_eq!(repo.head(), moved);
  assert_no_temporary(&repo);
}

#[test]
fn merge_refuses_a_base_branch_that_moved_while_the_merge_commit_was_made() {
  let (repo, base, _) = repo_with_run_work();
  repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  hook(
    &repo.root.join(".git/hooks"),
    "pre-merge-commit",
    &in_checkout(&repo.root, "commit --quiet --allow-empty -m meanwhile"),
  );
  let refused = merge(&repo, &base);
  assert_eq!(
    refused,
    Err(MergeError::BaseMoved {
      branch: BranchName::new("main").expect("valid")
    })
  );
  assert_eq!(repo.git(&["log", "-1", "--format=%s"]), "meanwhile");
  assert!(!repo.root.join("feature.txt").exists(), "the run's work did not arrive");
  assert_no_temporary(&repo);
}

#[test]
fn merge_runs_the_hooks_of_the_checkouts_own_hooks_folder() {
  let (repo, base, _) = repo_with_run_work();
  let moved = repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  // As husky's `.husky/_`: relative, and not in the commits, so not in the temporary worktree.
  fs::write(repo.root.join(".git/info/exclude"), "/.orchester/\n/.hooks/\n").expect("ignore hooks");
  repo.git(&["config", "core.hooksPath", ".hooks"]);
  hook(
    &repo.root.join(".hooks"),
    "pre-merge-commit",
    "echo 'refused by the local hooks' >&2\nexit 1\n",
  );
  let refused = merge(&repo, &base);
  let Err(MergeError::Refused(said)) = refused else {
    panic!("the checkout's hook refuses the merge: {refused:?}");
  };
  assert!(said.contains("refused by the local hooks"), "{said}");
  assert_eq!(repo.head(), moved);
  assert_no_temporary(&repo);
}

#[test]
fn merge_deletes_a_folder_git_does_not_know_where_its_temporary_worktree_goes() {
  let (repo, base, _) = repo_with_run_work();
  repo.commit(&repo.root, "other.txt", "mine\n", "the user's own work");
  let finished = run(&base, Some("main"), Ending::Succeeded);
  let common = repo.git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]);
  let left = format!("{common}/orchester-merge-{}", finished.id());
  fs::create_dir_all(format!("{left}/nested")).expect("a folder git does not know");
  fs::write(format!("{left}/nested/half.txt"), "half\n").expect("a file in it");
  let merged = merge_run(&Processes, &git_program(), &repo.root, (&finished, &task()), now()).expect("merged");
  assert_eq!(merged.merge.kind, MergeKind::MergeCommit);
  assert_eq!(merged.left, None);
  assert_no_temporary(&repo);
}

/// Changes `a.txt` and `held.txt` on the run's branch and turns the file `docs` into a folder,
/// keeps `notes.txt` of the person's beside them, and fast-forwards while git cannot replace one
/// of the run's files: `held.txt` held open on Windows, a new file in a folder without write
/// permission elsewhere.
#[test]
fn merge_puts_back_what_a_fast_forward_that_stopped_part_way_wrote() {
  let repo = Repo::new();
  fs::create_dir(repo.root.join("locked")).expect("folder");
  repo.commit(&repo.root, "a.txt", "base a\n", "a");
  repo.commit(&repo.root, "docs", "base docs\n", "docs");
  repo.commit(&repo.root, "held.txt", "base held\n", "held");
  let base = repo.commit(&repo.root, "locked/kept.txt", "kept\n", "locked");
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  repo.commit(&worktree, "a.txt", "run a\n", "Implement: a");
  git_in(&worktree, &["rm", "--quiet", "docs"]);
  fs::create_dir(worktree.join("docs")).expect("docs becomes a folder");
  repo.commit(&worktree, "docs/guide.md", "run guide\n", "Implement: docs");
  repo.commit(&worktree, "held.txt", "run held\n", "Implement: held");
  repo.commit(&worktree, "locked/new.txt", "run new\n", "Implement: new");
  repo.commit(&worktree, "z.txt", "run z\n", "Implement: z");
  fs::write(repo.root.join("notes.txt"), "the person's\n").expect("untracked");
  let refused = {
    let _blocked = Blocked::new(&repo.root);
    merge(&repo, &base)
  };
  assert!(matches!(refused, Err(MergeError::Refused(_))), "{refused:?}");
  assert_eq!(repo.head(), base);
  for (file, content) in [
    ("a.txt", "base a\n"),
    ("held.txt", "base held\n"),
    ("docs", "base docs\n"),
  ] {
    assert_eq!(
      fs::read_to_string(repo.root.join(file)).expect("there"),
      content,
      "{file}"
    );
  }
  assert!(!repo.root.join("z.txt").exists(), "a file the base did not have went");
  assert!(!repo.root.join("locked/new.txt").exists());
  assert_eq!(
    repo.git(&["status", "--porcelain", "--untracked-files=all"]),
    "?? notes.txt"
  );
}

#[test]
fn merge_puts_back_the_index_and_files_when_git_cannot_move_the_branch() {
  let repo = Repo::new();
  let base = repo.commit(&repo.root, "a.txt", "base a\n", "a");
  let worktree = repo.worktree("run", RUN_BRANCH, &base);
  repo.commit(&worktree, "a.txt", "run a\n", "Implement: a");
  repo.commit(&worktree, "new.txt", "run new\n", "Implement: new");
  fs::write(repo.root.join("notes.txt"), "the person's\n").expect("untracked");
  // A stale lock of the branch's ref: git writes the index and the files, then cannot move main.
  let lock = repo.root.join(".git/refs/heads/main.lock");
  fs::write(&lock, "").expect("stale lock");
  let refused = merge(&repo, &base);
  fs::remove_file(&lock).expect("lock gone");
  assert!(matches!(refused, Err(MergeError::Refused(_))), "{refused:?}");
  assert_eq!(repo.head(), base);
  assert_eq!(fs::read_to_string(repo.root.join("a.txt")).expect("there"), "base a\n");
  assert!(!repo.root.join("new.txt").exists());
  assert_eq!(
    repo.git(&["status", "--porcelain", "--untracked-files=all"]),
    "?? notes.txt",
    "the index is the base's again"
  );
}

/// What keeps git from writing one of the run's files, while it lives.
struct Blocked {
  #[cfg(windows)]
  _held: fs::File,
  #[cfg(unix)]
  locked: std::path::PathBuf,
}

impl Blocked {
  #[cfg(windows)]
  fn new(root: &Path) -> Self {
    use std::os::windows::fs::OpenOptionsExt;
    // Readers only: git can neither replace nor delete the file while it is open.
    const FILE_SHARE_READ: u32 = 1;
    let held = fs::OpenOptions::new()
      .read(true)
      .share_mode(FILE_SHARE_READ)
      .open(root.join("held.txt"))
      .expect("held open");
    Self { _held: held }
  }

  #[cfg(unix)]
  fn new(root: &Path) -> Self {
    use std::os::unix::fs::PermissionsExt;
    let locked = root.join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).expect("read-only folder");
    Self { locked }
  }
}

#[cfg(unix)]
impl Drop for Blocked {
  fn drop(&mut self) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&self.locked, fs::Permissions::from_mode(0o755)).expect("writable again");
  }
}
