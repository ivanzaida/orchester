//! Where a run's base branch comes from (`Base branch · redesign`, `ri5ru`): a task's own, else its module's, else
//! the branch checked out, for every combination, a task in no module included; how each source lands; and a branch
//! picked for a task or a module, a remote-only one creating its local tracking branch.
//! `cargo test -p orchester-core --test integration base_branch_source`.

use orchester_core::{
  id::ModuleId,
  integration::base::{
    BaseChoice, BranchPick, BranchPickError, Landing, RunBaseSource, TaskBase, branch_choices, resolve, take_branch,
  },
  run::BranchName,
};

use crate::support::{Processes, Repo, git_program};

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("valid branch")
}

#[test]
fn base_branch_source_resolves_every_task_and_module_combination() {
  let module = ModuleId::generate();
  let (staging, release) = (
    BaseChoice::Branch(branch("staging")),
    BaseChoice::Branch(branch("release")),
  );
  let task_branch = TaskBase::Branch(branch("release"));
  let cases = [
    (
      TaskBase::Inherit,
      None,
      BaseChoice::CheckedOut,
      RunBaseSource::CheckedOut,
    ),
    (
      TaskBase::Inherit,
      Some(&BaseChoice::CheckedOut),
      BaseChoice::CheckedOut,
      RunBaseSource::CheckedOut,
    ),
    (
      TaskBase::Inherit,
      Some(&staging),
      staging.clone(),
      RunBaseSource::Module { module },
    ),
    (
      TaskBase::CheckedOut,
      None,
      BaseChoice::CheckedOut,
      RunBaseSource::CheckedOut,
    ),
    (
      TaskBase::CheckedOut,
      Some(&BaseChoice::CheckedOut),
      BaseChoice::CheckedOut,
      RunBaseSource::CheckedOut,
    ),
    (
      TaskBase::CheckedOut,
      Some(&staging),
      BaseChoice::CheckedOut,
      RunBaseSource::CheckedOut,
    ),
    (task_branch.clone(), None, release.clone(), RunBaseSource::Task),
    (
      task_branch.clone(),
      Some(&BaseChoice::CheckedOut),
      release.clone(),
      RunBaseSource::Task,
    ),
    (task_branch, Some(&staging), release, RunBaseSource::Task),
  ];
  for (task, module_base, choice, source) in cases {
    assert_eq!(
      resolve(&task, module_base.map(|base| (module, base))),
      (choice, source),
      "task {task:?} in module {module_base:?}"
    );
  }
}

#[test]
fn base_branch_source_defaults_inherit_for_a_task_and_the_checked_out_branch_for_a_module() {
  assert_eq!(TaskBase::default(), TaskBase::Inherit);
  assert_eq!(BaseChoice::default(), BaseChoice::CheckedOut);
  assert_eq!(TaskBase::Branch(branch("staging")).branch(), Some(&branch("staging")));
  assert_eq!(TaskBase::CheckedOut.branch(), None);
}

#[test]
fn base_branch_source_lands_beside_the_checkout_only_for_a_branch_set() {
  let module = ModuleId::generate();
  for source in [
    RunBaseSource::Task,
    RunBaseSource::Module { module },
    RunBaseSource::Project,
  ] {
    assert_eq!(Landing::of(source), Landing::BesideOtherBranch, "{source:?}");
  }
  assert_eq!(Landing::of(RunBaseSource::CheckedOut), Landing::Checkout);
}

#[test]
fn base_branch_source_picking_a_remote_only_branch_creates_its_tracking_branch() {
  let repo = Repo::new();
  let head = repo.head();
  repo.git(&["remote", "add", "origin", "https://example.invalid/kontur.git"]);
  repo.git(&["update-ref", "refs/remotes/origin/release", &head]);
  let choices = branch_choices(&Processes, &git_program(), &repo.root).expect("choices");
  let remote = BranchPick::Remote(choices.remote.first().expect("origin/release").clone());
  let taken = take_branch(&Processes, &git_program(), (&repo.root, &remote)).expect("taken");
  assert_eq!(taken, branch("release"));
  assert_eq!(
    repo.git(&["rev-parse", "--abbrev-ref", "release@{upstream}"]),
    "origin/release"
  );
  assert_eq!(
    repo.git(&["symbolic-ref", "--short", "HEAD"]),
    "main",
    "the checkout stays"
  );
  assert_eq!(
    take_branch(&Processes, &git_program(), (&repo.root, &remote)),
    Ok(branch("release")),
    "a pick whose local branch exists by now takes it"
  );
  assert_eq!(
    take_branch(
      &Processes,
      &git_program(),
      (&repo.root, &BranchPick::Local(branch("main")))
    ),
    Ok(branch("main"))
  );
  assert_eq!(
    take_branch(
      &Processes,
      &git_program(),
      (&repo.root, &BranchPick::Local(branch("gone")))
    ),
    Err(BranchPickError::Gone(branch("gone")))
  );
}
