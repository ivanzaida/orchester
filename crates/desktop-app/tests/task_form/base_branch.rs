//! The Base branch chip after Module and its menu (`Base branch · redesign`, 01 `eUFSd`): a task inherits its module's
//! base branch, named, by default; the menu offers the checked-out branch, the repository's local branches, which
//! *Find a branch* narrows, and the remote-only ones, whose local branch saving the task creates.

use std::{path::Path, process::Command};

use orchester_core::{
  integration::base::{BaseChoice, TaskBase},
  module::{ModuleDetails, ModuleState},
  name::Name,
  run::BranchName,
  task::organization::ChangeActor,
  time::Timestamp,
};

use crate::harness::Form;

fn git(repo: &Path, args: &[&str]) -> String {
  let output = Command::new("git")
    .args(args)
    .current_dir(repo)
    .output()
    .expect("git runs");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("a branch name")
}

/// Opens the chip's menu and waits until it lists the repository's branches.
fn open_menu(form: &mut Form) {
  form.open("task-form-base-branch");
  form.window.until("the branches are read", |window| {
    window.texts("task-form-base-branch-note").is_none()
      && window.tree.get_element_by_id("task-form-base-branch-local-0").is_some()
  });
}

/// The local branches the menu lists now, by their titles.
fn listed(form: &mut Form, kind: &str) -> Vec<String> {
  (0..)
    .map_while(|index| {
      form
        .window
        .texts(&format!("task-form-base-branch-{kind}-{index}-title"))
    })
    .map(|title| title.join(""))
    .collect()
}

fn save(form: &mut Form) {
  form.click("task-form-save");
  form.window.until("the change is saved", |window| {
    window
      .texts("task-form-status-label")
      .is_some_and(|status| status.join("") == "Draft saved locally")
  });
}

#[test]
fn task_base_branch_takes_a_local_branch_or_the_checked_out_one() {
  let mut form = Form::seeded("live");
  git(&form.repo, &["branch", "release/2.0"]);
  git(&form.repo, &["branch", "staging"]);
  let checked_out = git(&form.repo, &["branch", "--show-current"]);
  form.edit("KON-1");
  assert_eq!(form.text("task-form-base-branch-label"), "Base branch");
  assert_eq!(form.text("task-form-base-branch-value"), "Checked-out branch");
  open_menu(&mut form);
  assert_eq!(form.text("task-form-base-branch-inherit-title"), "From its module");
  assert_eq!(
    form.text("task-form-base-branch-checked-out-help"),
    format!("{checked_out} now · taken when the run starts")
  );
  let mut local = listed(&mut form, "local");
  local.sort();
  let mut expected = vec![checked_out, "release/2.0".to_owned(), "staging".to_owned()];
  expected.sort();
  assert_eq!(local, expected);
  form.set("task-form-base-branch-find", "REL");
  assert_eq!(listed(&mut form, "local"), vec!["release/2.0".to_owned()]);
  form.click("task-form-base-branch-local-0");
  assert!(!form.exists("task-form-base-branch-menu"), "a choice closes the menu");
  assert_eq!(form.text("task-form-base-branch-value"), "release/2.0");
  assert_eq!(form.status(), "Unsaved changes");
  save(&mut form);
  assert_eq!(
    form.task("KON-1").expect("KON-1").base(),
    &TaskBase::Branch(branch("release/2.0"))
  );

  open_menu(&mut form);
  form.click("task-form-base-branch-checked-out");
  assert_eq!(form.text("task-form-base-branch-value"), "Checked-out branch");
  save(&mut form);
  assert_eq!(form.task("KON-1").expect("KON-1").base(), &TaskBase::CheckedOut);
}

#[test]
fn task_base_branch_remote_only_creates_its_local_branch_when_saved() {
  let mut form = Form::seeded("live");
  let head = git(&form.repo, &["rev-parse", "HEAD"]);
  git(
    &form.repo,
    &["remote", "add", "origin", "https://example.invalid/kontur.git"],
  );
  git(&form.repo, &["update-ref", "refs/remotes/origin/hotfix-1.9", &head]);
  form.edit("KON-1");
  open_menu(&mut form);
  assert_eq!(listed(&mut form, "remote"), vec!["origin/hotfix-1.9".to_owned()]);
  assert_eq!(
    form.text("task-form-base-branch-remote-0-help"),
    "Choosing it creates the local branch hotfix-1.9"
  );
  form.click("task-form-base-branch-remote-0");
  assert_eq!(form.text("task-form-base-branch-value"), "hotfix-1.9");
  assert!(
    git(&form.repo, &["branch", "--list", "hotfix-1.9"]).is_empty(),
    "nothing is created before the task is saved"
  );
  save(&mut form);
  assert_eq!(git(&form.repo, &["branch", "--list", "hotfix-1.9"]), "hotfix-1.9");
  assert_eq!(
    git(&form.repo, &["rev-parse", "--abbrev-ref", "hotfix-1.9@{upstream}"]),
    "origin/hotfix-1.9"
  );
  assert_eq!(
    form.task("KON-1").expect("KON-1").base(),
    &TaskBase::Branch(branch("hotfix-1.9"))
  );
}

#[test]
fn task_base_branch_inherits_its_modules_named_by_default() {
  let mut form = Form::seeded("live");
  git(&form.repo, &["branch", "release/2.0"]);
  let mut db = form.db();
  let details = ModuleDetails::new(
    Name::new("Webhooks v2").expect("a name"),
    "",
    (None, None),
    ModuleState::Open,
  )
  .expect("details");
  let module = db.create_module(details, Timestamp::now()).expect("the module");
  db.set_module_base(
    module.id(),
    &BaseChoice::CheckedOut,
    BaseChoice::Branch(branch("release/2.0")),
  )
  .expect("its base branch");
  let task = form.task("KON-1").expect("KON-1");
  db.set_task_module(task.id(), Some(module.id()), ChangeActor::Person, Timestamp::now())
    .expect("in the module");
  drop(db);
  form.window.projects().feed(form.project).saved();
  form.edit("KON-1");
  assert_eq!(form.text("task-form-base-branch-value"), "release/2.0 · from module");
  open_menu(&mut form);
  assert_eq!(
    form.text("task-form-base-branch-inherit-title"),
    "From module · Webhooks v2"
  );
  assert_eq!(
    form.text("task-form-base-branch-inherit-help"),
    "release/2.0 · the module's base branch"
  );
  assert_eq!(form.task("KON-1").expect("KON-1").base(), &TaskBase::Inherit);
}
