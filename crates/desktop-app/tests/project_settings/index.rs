//! Code-index setting, empty and failed lists, and a real rebuild in a test repository.
use std::{fs, process::Command};

use orchester_core::{revision::Revision, store::SettingsStore};
use orchester_workspace::{code_index, tools::form::parse_environment};

use super::harness::Settings;

#[test]
fn project_settings_code_index_saves_its_switch_and_checks_the_search_tool() {
  let mut page = Settings::new(false);
  let environment = parse_environment("PATH=\nLOCALAPPDATA=").expect("environment");
  page
    .db()
    .save_project_environment(&environment)
    .expect("no tools on project search path");
  page.tab("codeIndex", "settings-code-index");
  if !page.db().code_index().expect("setting").is_on() {
    page.click("settings-code-index");
    page.saved();
  }
  page.until("empty index panel", |p| p.exists("settings-no-indexes"));
  assert!(page.text("settings-no-indexes").contains("The first run"));
  page.until("ripgrep checked", |p| {
    p.text("settings-ripgrep-state").contains("not found")
  });
  assert!(page.exists("settings-ripgrep-install-panel"));
  page.click("settings-ripgrep-check");
  page.until("ripgrep rechecked", |p| {
    p.text("settings-ripgrep-state").contains("not found")
  });
  page.click("settings-code-index");
  page.until("index off", |p| p.exists("settings-index-off"));
  page.saved();
  assert!(!page.db().code_index().expect("setting").is_on());
  assert!(!page.exists("settings-index-panel"));
}

#[test]
fn project_settings_code_index_lists_ready_and_failed_revisions_and_rebuilds() {
  let mut page = Settings::new(false);
  git(&page.repo, &["init"]);
  fs::write(page.repo.join("lib.rs"), "fn main() {}\n").expect("source file");
  git(&page.repo, &["add", "lib.rs"]);
  git(
    &page.repo,
    &[
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "commit",
      "-m",
      "fixture",
    ],
  );
  let output = Command::new("git")
    .arg("-C")
    .arg(&page.repo)
    .args(["rev-parse", "HEAD"])
    .output()
    .expect("head");
  let revision = Revision::new(String::from_utf8_lossy(&output.stdout).trim()).expect("revision");
  code_index::rebuild_in(&page.repo, &revision).expect("build index");
  let missing = Revision::new("1111111111111111111111111111111111111111").expect("revision");
  assert!(code_index::rebuild_in(&page.repo, &missing).is_err());
  page.tab("codeIndex", "settings-code-index");
  if !page.db().code_index().expect("setting").is_on() {
    page.click("settings-code-index");
    page.saved();
  }
  let short = &revision.as_str()[..7];
  let ready = format!("settings-index-{short}");
  page.until("index list", |p| p.exists(&ready) && p.exists("settings-index-1111111"));
  assert!(page.text(&ready).contains("Ready"));
  assert!(page.text("settings-index-1111111").contains("Failed"));
  let manifest = page
    .repo
    .join(".orchester/index")
    .join(revision.as_str())
    .join("manifest.json");
  let before = fs::metadata(&manifest).expect("manifest").modified().expect("time");
  page.click(&format!("settings-index-rebuild-{short}"));
  page.until("index rebuilt", |_| {
    fs::metadata(&manifest)
      .and_then(|meta| meta.modified())
      .is_ok_and(|time| time != before)
  });
  page.until("failed retry", |p| {
    if p.exists("settings-index-rebuild-error") {
      return true;
    }
    // The preceding rebuild may still be publishing its completion; disabled clicks have no effect.
    p.click("settings-index-rebuild-1111111");
    false
  });
}
#[test]
fn project_settings_review_code_index_refreshes_runner_created_and_removed_indexes() {
  let mut page = Settings::new(false);
  git(&page.repo, &["init"]);
  page.tab("codeIndex", "settings-code-index");
  if !page.db().code_index().expect("setting").is_on() {
    page.click("settings-code-index");
    page.saved();
  }
  page.until("empty index list", |p| p.exists("settings-no-indexes"));
  let revision = Revision::new("1111111111111111111111111111111111111111").expect("revision");
  assert!(code_index::rebuild_in(&page.repo, &revision).is_err());
  let feed = page.window.projects().feed(crate::support::project_id());
  feed.changes.update(|revision| *revision += 1);
  page.until("runner-created index", |p| p.exists("settings-index-1111111"));
  assert!(page.text("settings-index-1111111").contains("Failed"));
  fs::remove_file(page.repo.join(".orchester/index/.failures").join(revision.as_str())).expect("remove fixture index");
  feed.changes.update(|revision| *revision += 1);
  page.until("runner-removed index", |p| p.exists("settings-no-indexes"));
}

pub(super) fn git(root: &std::path::Path, args: &[&str]) {
  let output = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(args)
    .output()
    .expect("git");
  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
