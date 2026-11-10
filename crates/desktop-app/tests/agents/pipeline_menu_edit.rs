//! The ⋯ menu of a pipeline's panel while the panel holds changes that are not saved (PV2-10 with PV2-12): *Rename* and
//! *Make default for New task* are direct actions that keep the edit; *Duplicate* and *Archive* leave the panel, so
//! they ask first, as every way out does, and *Keep editing* keeps what was typed. `cargo test -p orchester-app --test
//! agents pipeline_menu_edit`.

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::id::ResourceId;

use super::{
  harness::Agents,
  pipeline_hub::project,
  pipeline_hub_menu::{archived, key, names, open_menu},
  support::project_id,
};

const DISCARD: &str = "pipeline-hub-leave-discard";
const KEEP: &str = "pipeline-hub-leave-cancel";
const DESCRIPTION: &str = "pipeline-hub-description";

/// The panel of the ready pipeline with a description typed that is not saved, and its ⋯ menu open.
fn editing() -> (Agents, ResourceId) {
  let (mut agents, [ready, ..]) = project();
  open_menu(&mut agents, ready);
  agents.click("pipeline-hub-more");
  agents.until("the menu closed", |s| !s.exists("pipeline-hub-menu-archive"));
  agents.set(DESCRIPTION, "Not saved yet");
  agents.until("the change held", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes")
  });
  agents.click("pipeline-hub-more");
  agents.until("the menu", |s| s.exists("pipeline-hub-menu-archive"));
  (agents, ready)
}

#[test]
fn pipeline_menu_edit_rename_keeps_the_edit_and_save_keeps_the_new_name() {
  let (mut agents, ready) = editing();
  agents.click("pipeline-hub-menu-rename");
  agents.until("the field", |s| s.exists("pipeline-hub-rename"));
  assert!(!agents.exists(DISCARD), "a rename leaves nothing, so nothing is asked");

  agents.set("pipeline-hub-rename", "Feature flow");
  key(&mut agents, "Enter");
  agents.until("renamed", |s| {
    s.exists("team-hub-panel-title") && s.text("team-hub-panel-title") == "Feature flow"
  });
  assert_eq!(agents.value(DESCRIPTION), "Not saved yet", "the edit is still held");
  assert!(
    agents
      .text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes"),
    "and the row still says so"
  );

  agents.press_until("pipeline-hub-save", "saved", |s| s.text("pipeline-hub-chip-0") == "v2");
  assert_eq!(
    names(&agents, ready).0,
    "Feature flow",
    "Save kept the name the rename made"
  );
}

#[test]
fn pipeline_menu_edit_make_default_keeps_the_edit() {
  let (mut agents, ready) = editing();
  agents.click("pipeline-hub-menu-default");
  agents.until("the default chip", |s| {
    (0..4).any(|index| {
      s.exists(&format!("pipeline-hub-chip-{index}")) && s.text(&format!("pipeline-hub-chip-{index}")) == "Default"
    })
  });
  assert!(!agents.exists(DISCARD));
  assert_eq!(agents.value(DESCRIPTION), "Not saved yet");
  assert!(
    agents
      .text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes"),
    "the row still says the changes are not saved"
  );
}

#[test]
fn pipeline_menu_edit_duplicate_asks_first_and_keep_editing_keeps_what_was_typed() {
  let (mut agents, ready) = editing();
  let here = agents.window.navigation().route_now();
  agents.click("pipeline-hub-menu-duplicate");
  agents.until("the question", |s| s.exists(DISCARD));
  assert_eq!(
    agents.window.navigation().route_now(),
    here,
    "nothing moved while it asks"
  );

  agents.click(KEEP);
  agents.until("kept", |s| !s.exists(DISCARD));
  assert_eq!(agents.value(DESCRIPTION), "Not saved yet");

  agents.click("pipeline-hub-more");
  agents.until("the menu", |s| s.exists("pipeline-hub-menu-duplicate"));
  agents.click("pipeline-hub-menu-duplicate");
  agents.until("the question again", |s| s.exists(DISCARD));
  agents.click(DISCARD);
  agents.until("the copy page", |s| {
    s.window.navigation().route_now() == Route::Project(project_id(), ProjectPage::CopyPipeline(ready))
  });
}

#[test]
fn pipeline_menu_edit_archive_asks_about_the_changes_before_it_asks_to_archive() {
  let (mut agents, ready) = editing();
  agents.click("pipeline-hub-menu-archive");
  agents.until("the question", |s| s.exists(DISCARD));
  assert!(
    !agents.exists("team-hub-archive-confirm"),
    "the archive confirmation waits"
  );

  agents.click(KEEP);
  agents.until("kept", |s| !s.exists(DISCARD));
  assert!(
    !agents.exists("team-hub-archive-confirm"),
    "Keep editing archives nothing and asks nothing more"
  );
  assert_eq!(agents.value(DESCRIPTION), "Not saved yet");

  agents.click("pipeline-hub-more");
  agents.until("the menu", |s| s.exists("pipeline-hub-menu-archive"));
  agents.click("pipeline-hub-menu-archive");
  agents.until("the question again", |s| s.exists(DISCARD));
  agents.click(DISCARD);
  agents.until("the archive confirmation", |s| s.exists("team-hub-archive-confirm"));
  assert!(archived(&agents).is_empty(), "asking archives nothing");
  agents.press_until("team-hub-archive-confirm", "archived", |s| archived(s).contains(&ready));
}

#[test]
fn pipeline_menu_edit_archive_without_changes_asks_only_to_archive() {
  let (mut agents, [ready, ..]) = project();
  open_menu(&mut agents, ready);
  agents.click("pipeline-hub-menu-archive");
  agents.until("the archive confirmation", |s| s.exists("team-hub-archive-confirm"));
  assert!(!agents.exists(DISCARD), "nothing to lose, nothing asked");
}

#[test]
fn pipeline_menu_edit_an_archived_pipeline_has_no_save_bar() {
  let (mut agents, [ready, ..]) = project();
  open_menu(&mut agents, ready);
  agents.click("pipeline-hub-menu-archive");
  agents.until("the archive confirmation", |s| s.exists("team-hub-archive-confirm"));
  agents.press_until("team-hub-archive-confirm", "archived", |s| archived(s).contains(&ready));
  agents.click("pipeline-hub-filter-archived");
  agents.until("the archived panel", |s| s.exists("pipeline-hub-restore"));
  assert!(
    !agents.exists("pipeline-hub-save"),
    "an archived pipeline is not edited"
  );
  assert!(!agents.exists(DESCRIPTION));
}
