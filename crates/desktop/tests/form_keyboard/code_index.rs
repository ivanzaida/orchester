//! Project settings › Code index (ORC-38 `i220W`, `LC2V0`, `RkMap`, `h6yMk`): the setting saves
//! when its switch flips and the list goes while it is off; the kept indexes with their state,
//! *Rebuild* and *Retry*; ripgrep not found with *Check again*. The project's agents get a `PATH`
//! and a `%LOCALAPPDATA%` of an empty folder, so no ripgrep of the machine is found; *Install
//! ripgrep* is never pressed.

use std::{
  collections::BTreeMap,
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_core::{
  code_index::CodeIndexSetting,
  resource::{CommandEnvironment, VariableName},
  revision::Revision,
  store::SettingsStore,
};
use orchester_exec::ripgrep::LOCAL_APP_DATA;
use orchester_index::CodebaseIndexes;
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::init_repository,
};

const TAB: &str = "settings-tab-code-index";
const SWITCH: &str = "settings-code-index-switch";
const LIST: &str = "settings-code-index-list";
const CHECK: &str = "settings-code-index-check";
const NOTICE: &str = "settings-code-index-ripgrep-notice";
const OFF: &str = "settings-code-index-off";
/// A revision the repository does not have: its build fails.
const MISSING: &str = "1111111111111111111111111111111111111111";

/// The project's agents search an empty folder for programs.
fn without_ripgrep(repo: &Path) {
  let empty = repo.join(".orchester").join("empty");
  fs::create_dir_all(&empty).expect("an empty folder");
  let empty = empty.to_str().expect("a Unicode path").to_owned();
  let environment = CommandEnvironment::new(BTreeMap::from([
    (VariableName::new("PATH").expect("name"), empty.clone()),
    (VariableName::new(LOCAL_APP_DATA).expect("name"), empty),
  ]))
  .expect("valid variables");
  ProjectDb::open(repo)
    .expect("open")
    .save_project_environment(&environment)
    .expect("save the project's environment");
}

fn head(repo: &Path) -> Revision {
  let output = Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(["rev-parse", "HEAD"])
    .output()
    .expect("run git");
  Revision::new(String::from_utf8_lossy(&output.stdout).trim()).expect("a revision")
}

fn code_index_tab(desktop: &mut Desktop) {
  desktop.wait_for("nav-settings");
  desktop.click("nav-settings");
  desktop.wait_for(TAB);
  desktop.click(TAB);
  desktop.wait_for(SWITCH);
}

fn manifest(repo: &Path, revision: &Revision) -> PathBuf {
  repo
    .join(".orchester")
    .join("index")
    .join(revision.as_str())
    .join("manifest.json")
}

#[test]
fn code_index_tab_lists_a_ready_and_a_failed_index_and_rebuilds_them() {
  let mut desktop = Desktop::with_runner(PathBuf::from("no-runner.exe"), init_repository, |repo| {
    without_ripgrep(repo);
    let indexes = CodebaseIndexes::new(repo);
    indexes.rebuild(&head(repo)).expect("the index of HEAD builds");
    let missing = Revision::new(MISSING).expect("a revision");
    assert!(indexes.rebuild(&missing).is_err(), "a missing revision cannot be indexed");
  });
  let revision = head(desktop.repo());
  let short = &revision.as_str()[..7];
  let (ready, failed) = (
    format!("settings-code-index-row-{short}"),
    "settings-code-index-row-1111111".to_owned(),
  );
  code_index_tab(&mut desktop);
  desktop.wait_for(&ready);
  desktop.wait_for(&failed);

  assert!(shows(&mut desktop, &ready, short));
  assert!(shows(&mut desktop, &ready, "Ready"));
  assert!(shows(&mut desktop, &ready, "1 files · "));
  assert!(shows(&mut desktop, &ready, "Rebuild"));
  assert!(shows(&mut desktop, &failed, "Failed"));
  assert!(shows(&mut desktop, &failed, "Retry"));
  assert!(shows(&mut desktop, "settings-code-index-total", "2 indexes · "));
  assert!(shows(&mut desktop, "settings-code-index", "2 indexes kept · 1 failed"));

  // Rebuild builds the ready index again; Retry tries the failed one again.
  let built = fs::metadata(manifest(desktop.repo(), &revision))
    .and_then(|metadata| metadata.modified())
    .expect("the index's manifest");
  desktop.press(&format!("settings-code-index-button-{short}"));
  desktop.wait_until("the index is built again", |desktop| {
    fs::metadata(manifest(desktop.repo(), &revision))
      .and_then(|metadata| metadata.modified())
      .is_ok_and(|modified| modified != built)
  });
  desktop.press("settings-code-index-button-1111111");
  desktop.wait_for(&ready);
  assert!(shows(&mut desktop, &failed, "Failed"));
}

#[test]
fn code_index_tab_without_an_index_or_ripgrep_says_what_agents_do() {
  let mut desktop = Desktop::with_seeded_project(without_ripgrep);
  code_index_tab(&mut desktop);
  desktop.wait_for("settings-code-index-empty");
  desktop.wait_for(NOTICE);

  assert!(shows(&mut desktop, SWITCH, "On · agents search the index"));
  assert!(shows(&mut desktop, "settings-code-index", "No index yet"));
  assert!(shows(&mut desktop, "settings-code-index-total", "None kept"));
  assert!(shows(&mut desktop, "settings-code-index-ripgrep-state", "Not found"));
  assert!(shows(&mut desktop, NOTICE, "Agents fall back to slower search"));
  // The form: the switch, Install ripgrep when this system has its installer, Check again.
  let stops = desktop.stops(3, false);
  assert_eq!(stops.first(), Some(&Some(SWITCH.to_owned())));
  assert!(stops.contains(&Some(CHECK.to_owned())), "{stops:?}");

  // Check again looks again and finds nothing again.
  desktop.click(CHECK);
  desktop.wait_for(NOTICE);
  assert!(shows(&mut desktop, "settings-code-index-ripgrep-state", "Not found"));
}

#[test]
fn code_index_switch_saves_off_and_on() {
  let mut desktop = Desktop::with_seeded_project(without_ripgrep);
  code_index_tab(&mut desktop);
  desktop.wait_for(LIST);

  desktop.click(SWITCH);
  desktop.wait_for(OFF);
  assert_eq!(
    ProjectDb::open(desktop.repo()).expect("open").code_index().expect("read"),
    CodeIndexSetting::Off
  );
  assert!(shows(&mut desktop, SWITCH, "Off · agents get no index"));
  assert!(shows(&mut desktop, OFF, "Agents get no index and no index skill"));
  assert!(desktop.tree.get_element_by_id(LIST).is_none());
  assert!(desktop.tree.get_element_by_id(NOTICE).is_none());
  assert!(shows(&mut desktop, "settings-save-status", "All changes saved"));

  desktop.click(SWITCH);
  desktop.wait_for(LIST);
  assert_eq!(
    ProjectDb::open(desktop.repo()).expect("open").code_index().expect("read"),
    CodeIndexSetting::On
  );
}
