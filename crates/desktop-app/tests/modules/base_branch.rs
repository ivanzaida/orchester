//! A module's base branch (`Base branch · redesign` `ri5ru`): the Base branch chip of Edit module and its menu (02
//! `eyrnI`), and Project settings › General saying where base branches are set now, once with how the project's
//! base branch moved (04 `rBSzW`, 04b `NDTCw`). `cargo test -p orchester-app --test modules module_base_branch`.

use std::{path::Path, process::Command};

use orchester_app::routes::{ModuleListTab, ProjectPage, ProjectSettingsTab, Route};
use orchester_core::{
  integration::base::BaseChoice, run::BranchName, task::organization::WorkflowStatus,
};

use super::harness::{Seeded, details, module_with};

fn git(repo: &Path, args: &[&str]) -> String {
  let output = Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(args)
    .output()
    .expect("git runs");
  assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A module of two Todo tasks in a repository with `staging` beside `main`, and `origin/hotfix-1.9` only on the
/// remote; the window on its Edit module with the Base branch menu open.
fn editing() -> (Seeded, orchester_core::id::ModuleId) {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    picked = Some(module_with(db, details("Checkout", (0, 7)), &[(WorkflowStatus::Todo, 2)]).0);
  });
  let module = picked.expect("the module");
  git(&seeded.repo, &["branch", "staging"]);
  git(&seeded.repo, &["remote", "add", "origin", "https://example.invalid/kontur.git"]);
  git(&seeded.repo, &["update-ref", "refs/remotes/origin/hotfix-1.9", "HEAD"]);
  seeded.open(ProjectPage::EditModule(module), "module-base");
  seeded.click("module-base");
  seeded.until("the branches", |seeded| seeded.shows("module-base-note"));
  (seeded, module)
}

fn base(seeded: &Seeded, module: orchester_core::id::ModuleId) -> BaseChoice {
  seeded.db().module(module).expect("read").expect("the module").base().clone()
}

#[test]
fn module_base_branch_menu_offers_the_branches_and_the_form_saves_the_one_chosen() {
  let (mut seeded, module) = editing();
  assert_eq!(seeded.text("module-base-value"), "Checked-out branch");
  assert_eq!(
    seeded.text("module-base-checked-out"),
    "Checked-out branch main now · its tasks take the branch checked out when each run starts"
  );
  // The repository's local branches in git's order: `main`, the seeded runs' branches, `staging`.
  let mut branches = Vec::new();
  while seeded.shows(&format!("module-base-branch-{}-title", branches.len())) {
    branches.push(seeded.text(&format!("module-base-branch-{}-title", branches.len())));
  }
  assert_eq!(branches.first().map(String::as_str), Some("main"));
  let staging = branches
    .iter()
    .position(|branch| branch == "staging")
    .unwrap_or_else(|| panic!("staging is offered: {branches:?}"));
  assert_eq!(
    seeded.text("module-base-remote-0"),
    "origin/hotfix-1.9 Choosing it creates the local branch hotfix-1.9"
  );
  assert_eq!(
    seeded.text("module-base-note"),
    "Its 2 tasks without a base branch of their own run from it. Runs under way keep theirs."
  );
  seeded.click(&format!("module-base-branch-{staging}"));
  assert!(!seeded.shows("module-base-menu"), "a choice closes the menu");
  assert_eq!(seeded.text("module-base-value"), "staging");
  assert_eq!(base(&seeded, module), BaseChoice::CheckedOut, "nothing is saved before Save");
  seeded.click("module-editor-save");
  seeded.until("the module's page", |seeded| seeded.shows("module-summary"));
  let staging = BranchName::new("staging").expect("a branch");
  assert_eq!(base(&seeded, module), BaseChoice::Branch(staging));
}

#[test]
fn module_base_branch_found_by_name_and_a_remote_one_made_local() {
  let (mut seeded, module) = editing();
  seeded.set("module-base-find", "HOT");
  assert!(!seeded.shows("module-base-branch-0"), "no local branch matches");
  assert!(!seeded.shows("module-base-no-match"));
  seeded.click("module-base-remote-0");
  assert_eq!(seeded.text("module-base-value"), "hotfix-1.9");
  seeded.click("module-editor-save");
  seeded.until("the module's page", |seeded| seeded.shows("module-summary"));
  let hotfix = BranchName::new("hotfix-1.9").expect("a branch");
  assert_eq!(base(&seeded, module), BaseChoice::Branch(hotfix));
  assert_eq!(
    git(&seeded.repo, &["rev-parse", "--abbrev-ref", "hotfix-1.9@{upstream}"]),
    "origin/hotfix-1.9",
    "the choice made the local branch, tracking the remote one"
  );
}

#[test]
fn module_base_branch_general_says_where_it_is_set_and_once_how_it_moved() {
  let mut seeded = Seeded::new("modules-base-branch", |_| {});
  let general = ProjectPage::Settings(ProjectSettingsTab::General);
  seeded.open(general.clone(), "settings-base-branch-moved-text");
  assert_eq!(
    seeded.text("settings-base-branch-hint"),
    "Set on each task and module. A task without one takes its module's; without either, the branch checked out \
     when its run starts."
  );
  let moved = seeded.text("settings-base-branch-moved-text");
  assert!(moved.starts_with("Moved on "), "{moved}");
  assert!(
    moved.ends_with(
      ": main went to Billing exports and API rate limits, which had none. Webhooks v2 keeps its own; runs under \
       way keep theirs."
    ),
    "{moved}"
  );
  seeded.click("settings-base-branch-modules");
  assert_eq!(
    seeded.route(),
    Route::Project(seeded.project, ProjectPage::Modules(ModuleListTab::default()))
  );
  seeded.until("the settings closed forget the move", |seeded| {
    seeded.db().base_branch_move().expect("read").is_none()
  });
  seeded.open(general, "settings-base-branch-row");
  assert!(!seeded.shows("settings-base-branch-moved"), "the line shows once");
  assert!(seeded.shows("settings-base-branch-modules"));
}
