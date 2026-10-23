//! General's *Base branch* row (ORC-81) in the headless window: the checked-out branch by default, the repository's
//! local and remote-tracking branches offered, a choice saved in the project's database with the count of unfinished
//! runs that keep their base, a remote-tracking choice creating its local branch, a branch set that is gone shown as
//! the row's problem, and `get_project` reporting the setting.
//! `cargo test -p orchester-app --test project_settings project_settings_base_branch`.
use std::process::Command;

use orchester_core::{integration::base::BaseBranchSetting, run::BranchName};
use orchester_workspace::tasks::outside::Outside;
use serde_json::json;

use super::{domain::call, harness::Settings};

const SELECT: &str = "settings-base-branch-select";

fn git(page: &Settings, args: &[&str]) -> String {
  let output = Command::new("git")
    .arg("-C")
    .arg(&page.repo)
    .args(args)
    .output()
    .expect("git");
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// The project's folder as a repository on `main`, with `staging` and a remote-tracking `origin/release`; the row
/// reads it again.
fn repository(page: &mut Settings) {
  git(page, &["init", "--quiet", "--initial-branch=main"]);
  git(page, &["config", "user.name", "Orchester Test"]);
  git(page, &["config", "user.email", "test@orchester.invalid"]);
  git(page, &["config", "commit.gpgsign", "false"]);
  git(page, &["commit", "--quiet", "--allow-empty", "-m", "initial"]);
  git(page, &["branch", "staging"]);
  git(page, &["remote", "add", "origin", "https://example.invalid/kontur.git"]);
  git(page, &["update-ref", "refs/remotes/origin/release", "HEAD"]);
  page.outside(Outside::ProjectSettings);
  page.until("every branch read", |p| {
    labels(p)
      .iter()
      .any(|label| label == "origin/release · creates release")
  });
}

fn labels(page: &mut Settings) -> Vec<String> {
  page
    .window
    .tree
    .get_element_by_id_mut(SELECT)
    .and_then(|element| element.as_select())
    .map(|select| select.labels().iter().map(|label| label.to_string()).collect())
    .unwrap_or_default()
}

fn chosen(page: &mut Settings) -> Vec<String> {
  page
    .window
    .tree
    .get_element_by_id_mut(SELECT)
    .and_then(|element| element.as_select())
    .expect("the select")
    .selected_labels()
    .iter()
    .map(|label| label.to_string())
    .collect()
}

fn setting(page: &Settings) -> BaseBranchSetting {
  page.db().base_branch_setting().expect("setting")
}

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("branch")
}

#[test]
fn project_settings_base_branch_defaults_to_the_checked_out_branch_and_saves_a_branch() {
  let mut page = Settings::new(false);
  repository(&mut page);
  assert_eq!(
    labels(&mut page),
    [
      "Checked-out branch",
      "main",
      "staging",
      "origin/release · creates release"
    ]
  );
  assert_eq!(chosen(&mut page), ["Checked-out branch"]);
  assert_eq!(setting(&page), BaseBranchSetting::CheckedOut);
  assert!(page.text("settings-base-branch-setting-row").contains("Base branch"));
  assert!(
    page
      .text("settings-base-branch-hint")
      .contains("Kept on this computer only")
  );
  assert!(!page.exists("settings-base-branch-missing"));

  {
    let mut db = page.db();
    super::run_fixture::auto_run(&mut db);
  }
  page.choose(SELECT, "staging");
  page.until("staging saved", |p| {
    setting(p) == BaseBranchSetting::Branch(branch("staging"))
  });
  page.saved();
  page.until("the kept runs said", |p| p.exists("settings-base-branch-kept"));
  assert_eq!(
    page.text("settings-base-branch-kept"),
    "1 unfinished run keeps the base branch it started from."
  );
  assert_eq!(chosen(&mut page), ["staging"]);

  page.choose(SELECT, "Checked-out branch");
  page.until("the checked-out branch saved", |p| {
    setting(p) == BaseBranchSetting::CheckedOut
  });
  page.saved();
  assert_eq!(chosen(&mut page), ["Checked-out branch"]);
}

#[test]
fn project_settings_base_branch_a_remote_tracking_choice_creates_its_local_branch() {
  let mut page = Settings::new(false);
  repository(&mut page);
  page.choose(SELECT, "origin/release · creates release");
  page.until("release saved", |p| {
    setting(p) == BaseBranchSetting::Branch(branch("release"))
  });
  page.saved();
  assert_eq!(
    git(&page, &["rev-parse", "--abbrev-ref", "release@{upstream}"]),
    "origin/release"
  );
  assert_eq!(
    git(&page, &["symbolic-ref", "--short", "HEAD"]),
    "main",
    "the checkout stays"
  );
  page.until("release listed as local", |p| {
    labels(p) == ["Checked-out branch", "main", "release", "staging"]
  });
  page.until("release chosen", |p| chosen(p) == ["release"]);
  assert!(!page.exists("settings-base-branch-kept"), "no run keeps another base");
}

#[test]
fn project_settings_base_branch_a_branch_set_that_is_gone_is_the_rows_problem() {
  let mut page = Settings::new(false);
  repository(&mut page);
  page
    .db()
    .save_base_branch_setting(&BaseBranchSetting::Branch(branch("gone")))
    .expect("set");
  page.outside(Outside::ProjectSettings);
  page.until("the problem shown", |p| p.exists("settings-base-branch-missing"));
  assert_eq!(
    page.text("settings-base-branch-missing"),
    "gone no longer exists in the repository, so new runs are refused. Choose another base branch."
  );
  assert_eq!(chosen(&mut page), ["gone · no longer exists"]);
  let read = call(&page, "get_project", json!({"project":"Kontur"})).expect("read");
  assert_eq!(
    read["project"]["base_branch"],
    json!({"branch": "gone", "missing": true})
  );

  page.choose(SELECT, "main");
  page.until("main saved", |p| {
    setting(p) == BaseBranchSetting::Branch(branch("main"))
  });
  page.until("the problem gone", |p| !p.exists("settings-base-branch-missing"));
  let read = call(&page, "get_project", json!({"project":"Kontur"})).expect("read");
  assert_eq!(
    read["project"]["base_branch"],
    json!({"branch": "main", "missing": false})
  );
}

#[test]
fn project_settings_base_branch_get_project_reports_the_checked_out_branch_by_default() {
  let page = Settings::new(false);
  let read = call(&page, "get_project", json!({"project":"Kontur"})).expect("read");
  assert_eq!(
    read["project"]["base_branch"],
    json!({"branch": null, "missing": false})
  );
}
