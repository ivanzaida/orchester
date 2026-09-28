//! Extra writable folders on the Permissions tab: a typed folder is checked before it is added
//! (the checkout is refused, a folder under `.orchester/` is fine), Enter in the field adds it, the
//! summary and the enforcement panel say what the CLI grants, and the build folder Project settings
//! › Environment sets is suggested until it is added.

use std::collections::BTreeMap;

use orchester_core::{
  resource::{AgentProfile, CommandEnvironment, ResourceKind, VariableName},
  store::{ResourceStore, SettingsStore},
};
use orchester_store::ProjectDb;

use crate::{
  agent_permissions::{MAX_STOPS, permissions_of, seeded, shows},
  harness::Desktop,
};

const FIELD: &str = "agent-new-folder";

fn set_folder(desktop: &mut Desktop, text: &str) {
  desktop
    .tree
    .get_element_by_id_mut(FIELD)
    .and_then(|element| element.as_text_input())
    .expect("the folder field")
    .set_value(text);
  desktop.pass();
}

/// The folders of `role`'s saved draft.
fn drafted_folders(desktop: &Desktop, role: &str) -> Vec<String> {
  let db = ProjectDb::open(desktop.repo()).expect("project database");
  let id = db
    .resources(ResourceKind::AgentProfile)
    .expect("profiles")
    .into_iter()
    .find(|resource| resource.name().as_str() == role)
    .expect("profile")
    .id();
  db.draft::<AgentProfile>(id)
    .expect("read")
    .map(|draft| {
      draft
        .body()
        .permissions()
        .writable_folders()
        .entries()
        .iter()
        .map(|folder| folder.as_str().to_owned())
        .collect()
    })
    .unwrap_or_default()
}

#[test]
fn writable_folders_add_a_checked_folder_and_refuse_the_checkout() {
  let mut desktop = seeded();
  permissions_of(&mut desktop, "Builder");
  assert!(shows(&mut desktop, "Extra writable folders"));
  assert!(shows(&mut desktop, "Agents write only inside their task worktree."));
  // Without CARGO_TARGET_DIR in Environment nothing is suggested.
  assert!(desktop.tree.get_element_by_id("agent-target-folder").is_none());
  desktop.tab_to(FIELD, MAX_STOPS);
  desktop.type_text("src");
  // Enter adds the folder, or says why it cannot be added; it does not save the draft.
  desktop.enter();
  assert!(shows(&mut desktop, "is part of your checkout."));
  assert!(desktop.tree.get_element_by_id("agent-folder-0").is_none());
  set_folder(&mut desktop, ".orchester/target");
  desktop.enter();
  desktop.wait_for("agent-folder-0");
  assert_eq!(desktop.value(FIELD), "");
  assert!(!shows(&mut desktop, "is part of your checkout."));
  assert!(shows(&mut desktop, "Unsaved changes"));
  assert!(shows(
    &mut desktop,
    "Edits files in the task worktree and 1 more folder · Any command in the worktree · Network on"
  ));
  assert!(shows(&mut desktop, "Extra writable folders · enforced"));
  assert!(shows(&mut desktop, "sandbox · writable roots"));
  desktop.press("agent-save-draft");
  let saved = |desktop: &mut Desktop| drafted_folders(desktop, "Builder") == [".orchester/target"];
  desktop.wait_until("the saved draft", saved);
  // Pressing the entry removes the folder again.
  desktop.press("agent-folder-0");
  desktop.settle("no folder", |tree| tree.get_element_by_id("agent-folder-0").is_none());
  assert!(!shows(&mut desktop, "Extra writable folders · enforced"));
}

#[test]
fn writable_folders_suggest_the_build_folder_environment_sets() {
  let mut desktop = seeded();
  let target = desktop.repo().join(".orchester").join("target");
  let target = target.to_str().expect("unicode").to_owned();
  let variables = BTreeMap::from([(VariableName::new("CARGO_TARGET_DIR").expect("name"), target.clone())]);
  ProjectDb::open(desktop.repo())
    .expect("project database")
    .save_project_environment(&CommandEnvironment::new(variables).expect("environment"))
    .expect("saved");
  // A profile that only reads writes nowhere, so nothing is suggested.
  permissions_of(&mut desktop, "Reviewer");
  assert!(shows(&mut desktop, "With Read-only they write nowhere."));
  assert!(desktop.tree.get_element_by_id("agent-target-folder").is_none());
  desktop.press("agent-permission-edit");
  desktop.wait_for("agent-target-folder");
  assert!(shows(&mut desktop, "Allow agents to write to CARGO_TARGET_DIR"));
  assert!(shows(
    &mut desktop,
    &format!("Environment sets CARGO_TARGET_DIR to {target}.")
  ));
  desktop.press("agent-add-target-folder");
  desktop.wait_for("agent-folder-0");
  desktop.settle("the suggestion to go", |tree| {
    tree.get_element_by_id("agent-target-folder").is_none()
  });
  // Claude Code lets its edit tools write there; its commands were never confined.
  assert!(shows(&mut desktop, "--add-dir for edit tools · commands not confined"));
  assert!(shows(
    &mut desktop,
    "Edits files in the task worktree and 1 more folder · 3 allowed commands · Network on"
  ));
}
