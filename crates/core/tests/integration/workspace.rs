//! Warm workspaces against real git (ORC-114): a switch keeps only the warm folders set to keep, uncommitted
//! leftovers refuse it until kept in a branch or discarded, and giving a workspace back frees its branch.

use std::fs;

use orchester_core::{
  integration::workspace::{SwitchTo, WorkspaceGitError, Workspaces},
  run::BranchName,
  workspaces::{WarmChoice, WarmFolder, WarmPattern, WorkspaceNumber, artifacts_folder},
};

use super::support::{Processes, Repo, git_in, git_program};

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("a branch")
}

fn folder(pattern: &str, choice: WarmChoice) -> WarmFolder {
  WarmFolder::new(WarmPattern::new(pattern).expect("a pattern"), choice)
}

/// A repository that ignores build output, with two run branches.
fn repo() -> Repo {
  let repo = Repo::new();
  repo.commit(
    &repo.root,
    ".gitignore",
    "target/\nnode_modules/\ndist/\n.tmp/\n",
    "ignore build output",
  );
  repo.git(&["branch", "orchester/orc-1-run-1"]);
  repo.git(&["branch", "orchester/orc-2-run-1"]);
  repo
}

#[test]
fn workspace_clean_keeps_only_keep_folders_on_a_switch() {
  let repo = repo();
  let program = git_program();
  let workspaces = Workspaces::new(&Processes, &program, &repo.root);
  let number = WorkspaceNumber::FIRST;
  let path = workspaces.prepare(number, "main").expect("a workspace");
  assert_eq!(path, number.path(&repo.root));
  workspaces
    .switch(&path, SwitchTo::Branch(&branch("orchester/orc-1-run-1")), (&[], false))
    .expect("switch");
  for built in [
    "target/debug/app",
    "packages/web/node_modules/x/index.js",
    "dist/app.js",
    ".tmp/scratch",
  ] {
    let file = path.join(built);
    fs::create_dir_all(file.parent().expect("a parent")).expect("folders");
    fs::write(&file, "built").expect("a build file");
  }
  fs::create_dir_all(path.join("packages/web")).expect("a package");
  let folders = [
    folder("target", WarmChoice::Keep),
    folder("packages/*/node_modules", WarmChoice::Keep),
    folder("dist", WarmChoice::Clean),
  ];
  workspaces.detach(&path).expect("give back");
  workspaces
    .switch(
      &path,
      SwitchTo::Branch(&branch("orchester/orc-2-run-1")),
      (&folders, false),
    )
    .expect("switch to another run");
  assert!(path.join("target/debug/app").is_file());
  assert!(path.join("packages/web/node_modules/x/index.js").is_file());
  assert!(!path.join("dist").exists(), "a folder set to clean every run goes");
  assert!(!path.join(".tmp").exists(), "everything else ignored goes");
  assert_eq!(
    git_in(&path, &["symbolic-ref", "--short", "HEAD"]),
    "orchester/orc-2-run-1"
  );
}

#[test]
fn workspace_uncommitted_leftovers_refuse_a_switch_until_kept_in_a_branch() {
  let repo = repo();
  let program = git_program();
  let workspaces = Workspaces::new(&Processes, &program, &repo.root);
  let path = workspaces.prepare(WorkspaceNumber::FIRST, "main").expect("a workspace");
  workspaces
    .switch(&path, SwitchTo::Branch(&branch("orchester/orc-1-run-1")), (&[], false))
    .expect("switch");
  fs::write(path.join("half-done.rs"), "fn main() {}").expect("a leftover");
  let refused = workspaces.switch(&path, SwitchTo::Branch(&branch("orchester/orc-2-run-1")), (&[], false));
  assert_eq!(
    refused,
    Err(WorkspaceGitError::Uncommitted(vec!["half-done.rs".to_owned()]))
  );
  let rescue = branch("orchester/rescue/orc-1-run-1");
  workspaces
    .rescue(&path, &rescue, "kept from a stopped run")
    .expect("rescue");
  assert!(repo.branch_exists(rescue.as_str()));
  assert_eq!(
    git_in(&repo.root, &["show", "orchester/rescue/orc-1-run-1:half-done.rs"]),
    "fn main() {}"
  );
  workspaces
    .switch(&path, SwitchTo::Branch(&branch("orchester/orc-2-run-1")), (&[], false))
    .expect("the switch goes on");
}

#[test]
fn workspace_discard_throws_the_leftovers_away_and_keeps_warm_folders() {
  let repo = repo();
  let program = git_program();
  let workspaces = Workspaces::new(&Processes, &program, &repo.root);
  let path = workspaces.prepare(WorkspaceNumber::FIRST, "main").expect("a workspace");
  fs::write(path.join("README.md"), "changed").expect("a change");
  fs::write(path.join("new.txt"), "new").expect("a new file");
  fs::create_dir_all(path.join("target")).expect("build output");
  fs::write(path.join("target/app"), "built").expect("built");
  workspaces.discard(&path).expect("discard");
  assert!(workspaces.uncommitted(&path).expect("status").is_empty());
  assert!(path.join("target/app").is_file());
}

#[test]
fn workspace_carry_keeps_the_same_runs_changes_and_frees_its_branch_elsewhere() {
  let repo = repo();
  let program = git_program();
  let workspaces = Workspaces::new(&Processes, &program, &repo.root);
  let run = branch("orchester/orc-1-run-1");
  let first = workspaces.prepare(WorkspaceNumber::FIRST, "main").expect("a workspace");
  workspaces
    .switch(&first, SwitchTo::Branch(&run), (&[], false))
    .expect("switch");
  // A runner that stopped left the branch checked out in workspace 1; the run goes on in workspace 2.
  let second = workspaces
    .prepare(WorkspaceNumber::new(2).expect("two"), "main")
    .expect("a second workspace");
  workspaces
    .switch(&second, SwitchTo::Branch(&run), (&[], false))
    .expect("its branch moves");
  assert_eq!(git_in(&second, &["symbolic-ref", "--short", "HEAD"]), run.as_str());
  fs::write(second.join("work.rs"), "wip").expect("work in progress");
  workspaces
    .switch(&second, SwitchTo::Branch(&run), (&[], true))
    .expect("the same run goes on with its changes");
  assert!(second.join("work.rs").is_file());
  let committed = workspaces.commit(&second, "saved").expect("commit");
  assert!(committed.is_some());
}

#[test]
fn workspace_artifacts_survive_a_clean() {
  let repo = repo();
  let program = git_program();
  let workspaces = Workspaces::new(&Processes, &program, &repo.root);
  let path = workspaces.prepare(WorkspaceNumber::FIRST, "main").expect("a workspace");
  let run = orchester_core::id::RunId::generate();
  let artifacts = artifacts_folder(&repo.root, run);
  fs::create_dir_all(&artifacts).expect("the run storage");
  fs::write(artifacts.join("screenshot.png"), "png").expect("an artifact");
  fs::create_dir_all(path.join("target")).expect("build output");
  fs::write(path.join("target/app"), "built").expect("built");
  workspaces.clean_warm(&path).expect("clean");
  assert!(!path.join("target").exists());
  assert!(artifacts.join("screenshot.png").is_file());
}
