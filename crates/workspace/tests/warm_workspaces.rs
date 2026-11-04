//! Warm workspaces as Project settings › Runs reads and changes them (ORC-114), with the real `orchester-runner`
//! sizing and changing their folders (`docs/architecture/remote-runners.md`, rule 5): a workspace's size, cleaning
//! its warm folders, and a stranded workspace's leftovers kept in a branch or discarded. Run with `cargo test -p
//! orchester-workspace --features test-support --test warm_workspaces warm_workspaces_`.

#[path = "support/runner_programs.rs"]
mod runner_programs;
mod support;

use std::{fs, num::NonZeroU32, path::Path, process::Command};

use orchester_core::{
  run::BranchName,
  store::WorkspaceStore,
  workspaces::{DiskSpace, DiskSpaceError, DiskUsage, GIGABYTE, Holder, Stranded, WorkspaceNumber},
};
use orchester_exec::disk_space::folder_size;
use orchester_ipc::SessionEnd;
use orchester_workspace::{
  activity::Activity,
  runner::{ExecutorFolders, QUIET_STOP_TIMEOUT, Runners},
  warm_workspaces::{
    self,
    actions::{self, Leftovers},
  },
  workspace::OpenProject,
};
use tokio::runtime::Handle;

use self::{
  runner_programs::programs,
  support::{Fixture, at},
};

struct Disk;

impl DiskSpace for Disk {
  fn usage(&self, _folder: &Path) -> Result<DiskUsage, DiskSpaceError> {
    Ok(DiskUsage {
      free: 500 * GIGABYTE,
      total: 2000 * GIGABYTE,
    })
  }
}

/// Runs git with `args` in `dir`; whether it succeeded.
fn git(dir: &Path, args: &[&str]) -> bool {
  Command::new("git")
    .args(args)
    .current_dir(dir)
    .output()
    .expect("run git")
    .status
    .success()
}

/// Makes the fixture's folder, whose stand-in `.git` git fills, a repository with one commit and workspace 1 a
/// worktree of it, with 10 000 bytes of ignored build output.
fn repository_with_workspace(root: &Path) -> WorkspaceNumber {
  for args in [
    &["init", "--quiet"][..],
    &["config", "user.email", "test@example.com"],
    &["config", "user.name", "Test"],
  ] {
    assert!(git(root, args), "git {args:?}");
  }
  fs::write(root.join(".gitignore"), ".orchester/\nbuild/\n").expect("the ignore file");
  fs::write(root.join("a.txt"), "one\n").expect("a file");
  assert!(git(root, &["add", "--all"]));
  assert!(git(root, &["commit", "--quiet", "-m", "start"]));
  let number = WorkspaceNumber::new(1).expect("above zero");
  assert!(git(
    root,
    &["worktree", "add", "--quiet", "--detach", &number.relative(), "HEAD"]
  ));
  let build = number.path(root).join("build");
  fs::create_dir_all(&build).expect("the build folder");
  fs::write(build.join("out.bin"), vec![0_u8; 10_000]).expect("build output");
  number
}

fn strand(fixture: &mut Fixture, number: WorkspaceNumber) {
  fixture
    .db
    .set_workspace_stranded(number, Some(Stranded { run: None, files: 1 }))
    .expect("stranded");
}

#[tokio::test(flavor = "multi_thread")]
async fn warm_workspaces_are_sized_cleaned_kept_and_discarded_by_the_runner() {
  let mut fixture = Fixture::new();
  let root = fixture.repo.path().to_path_buf();
  let number = repository_with_workspace(&root);
  let limit = NonZeroU32::new(2).expect("above zero");
  fixture
    .db
    .borrow_workspace(Holder::Upkeep, None, limit, at(1))
    .expect("borrow");
  fixture.db.release_workspace(number, Holder::Upkeep).expect("release");
  let project = OpenProject {
    project: fixture.db.project().clone(),
    repo_root: root.clone(),
  };
  let runners = Runners::new(
    programs().runner.clone(),
    Handle::current(),
    Activity::new(),
    SessionEnd::Clean,
  );
  let folders = ExecutorFolders::new(runners.clone(), project);
  let path = number.path(&root);

  let view = tokio::task::block_in_place(|| warm_workspaces::read(&root, &Disk, &folders)).expect("read");
  assert_eq!(view.rows.len(), 1);
  assert_eq!(
    view.rows[0].bytes,
    folder_size(&path).ok(),
    "the runner's size is the folder's own"
  );
  assert!(view.old_worktrees.is_empty());

  let freed = tokio::task::block_in_place(|| actions::clean(&root, &folders, &[number])).expect("clean");
  assert_eq!(freed, 10_000, "the build output went");
  assert!(!path.join("build").exists());
  assert!(path.join("a.txt").exists(), "committed files stay");

  fs::write(path.join("a.txt"), "two\n").expect("a change a stopped run left");
  strand(&mut fixture, number);
  let kept =
    tokio::task::block_in_place(|| actions::resolve_leftovers(&root, &folders, number, Leftovers::KeepInBranch))
      .expect("keep");
  let branch = BranchName::new("orchester/rescue/workspace-1").expect("a valid branch name");
  assert_eq!(kept, Some(branch));
  assert!(git(
    &root,
    &["rev-parse", "--verify", "--quiet", "orchester/rescue/workspace-1"]
  ));
  assert_eq!(
    fs::read_to_string(path.join("a.txt")).expect("read"),
    "two\n",
    "kept as it was"
  );

  fs::write(path.join("stray.txt"), "left\n").expect("another change");
  strand(&mut fixture, number);
  let discarded =
    tokio::task::block_in_place(|| actions::resolve_leftovers(&root, &folders, number, Leftovers::Discard))
      .expect("discard");
  assert_eq!(discarded, None);
  assert!(!path.join("stray.txt").exists(), "the leftovers went");
  let record = fixture.db.workspaces().expect("workspaces").remove(0);
  assert_eq!((record.stranded, record.holder), (None, None), "free again");
  runners.shutdown(Some(QUIET_STOP_TIMEOUT)).await;
}
