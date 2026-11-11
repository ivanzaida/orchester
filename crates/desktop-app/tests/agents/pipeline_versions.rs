//! *Versions* in a local pipeline's head (`Pipelines · v2` P12, PV2-11): the list of published versions with what holds
//! each, a row opening that version read-only in the panel, and *Restore*, which puts it into the edit that *Save* then
//! keeps as the next version. `cargo test -p orchester-app --test agents pipeline_versions`.

use orchester_app::routes::PipelinesFilter;
use orchester_core::{
  id::ResourceId,
  resource::Pipeline,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_hub::{open, setup},
};

pub(super) const BUTTON: &str = "pipeline-hub-versions-button";
pub(super) const SAVE: &str = "pipeline-hub-save";
pub(super) const CANCEL: &str = "pipeline-hub-cancel";
pub(super) const DESCRIPTION: &str = "pipeline-hub-description";
pub(super) const RESTORE: &str = "pipeline-hub-version-restore";

/// The versions the database holds of `id`, oldest first.
pub(super) fn versions(agents: &Agents, id: ResourceId) -> Vec<Pipeline> {
  let db = ProjectDb::open(&agents.root).expect("database");
  let mut versions = db.versions::<Pipeline>(id).expect("versions");
  versions.sort_by_key(|version| version.number());
  versions.iter().map(|version| version.body().clone()).collect()
}

/// Publishes a second version of `id`, which has the description `description`, behind the page.
pub(super) fn publish_second(agents: &Agents, id: ResourceId, description: &str) {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let latest = db.latest_version::<Pipeline>(id).expect("read").expect("published");
  let body = latest.body().clone().with_description(description.into());
  db.publish_pipeline_edit(id, &body, latest.number(), None, Timestamp::now())
    .expect("published");
}

/// The panel of `ready` with a second version behind it, open.
pub(super) fn two_versions() -> (Agents, ResourceId) {
  let (mut agents, [ready, ..]) = setup();
  publish_second(&agents, ready, "Plan, then build");
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE) && s.exists(BUTTON));
  (agents, ready)
}

pub(super) fn open_list(agents: &mut Agents) {
  agents.click(BUTTON);
  agents.until("both versions", |s| {
    s.exists("pipeline-hub-version-2") && s.exists("pipeline-hub-version-1")
  });
}

#[test]
fn pipeline_versions_list_says_what_holds_each_version() {
  let (mut agents, ready) = two_versions();
  let list_open = agents.exists("pipeline-hub-versions");
  assert!(!list_open, "the list is closed until Versions is pressed");
  assert_eq!(agents.text(BUTTON), "Versions");

  open_list(&mut agents);
  assert_eq!(agents.text("pipeline-hub-versions-title"), "Versions");
  assert_eq!(agents.text("pipeline-hub-versions-name"), "Feature delivery");
  assert_eq!(agents.text("pipeline-hub-version-2-number"), "v2");
  assert_eq!(agents.text("pipeline-hub-version-2-current"), "current");
  assert!(
    agents.exists("pipeline-hub-version-2-check"),
    "the current version is checked"
  );
  assert!(!agents.exists("pipeline-hub-version-1-current"));
  assert!(!agents.exists("pipeline-hub-version-1-check"));
  let current = agents.text("pipeline-hub-version-2-meta");
  assert!(
    current.starts_with("Published ") && current.ends_with(" · used by 3 tasks"),
    "{current}"
  );
  let older = agents.text("pipeline-hub-version-1-meta");
  assert!(
    older.starts_with("Published ") && older.ends_with(" · no runs on it"),
    "{older}"
  );
  assert_eq!(
    agents.text("pipeline-hub-versions-foot"),
    "Runs keep the version they started on."
  );
  assert_eq!(versions(&agents, ready).len(), 2, "opening the list stores nothing");
}

#[test]
fn pipeline_versions_a_row_opens_that_version_read_only_and_back_to_editing_returns() {
  let (mut agents, ready) = two_versions();
  open_list(&mut agents);
  agents.click("pipeline-hub-version-1");

  agents.until("the version is shown", |s| s.exists("pipeline-hub-version-viewing"));
  assert!(
    !agents.exists("pipeline-hub-versions"),
    "choosing a row closes the list"
  );
  assert_eq!(
    agents.text("pipeline-hub-version-viewing-title"),
    "Read-only",
    "the head names the version; the notice says only that it is read-only"
  );
  assert_eq!(agents.text("pipeline-hub-version-description-value"), "No description");
  assert_eq!(agents.text("pipeline-hub-version-steps-count"), "2");
  assert!(
    !agents.exists(SAVE),
    "the Save bar belongs to the edit, which is not shown"
  );
  assert!(
    !agents.exists("pipeline-hub-description"),
    "a version has no field to change"
  );
  assert!(
    !agents.exists("pipeline-hub-edit-steps"),
    "a version's steps are not edited here"
  );
  assert!(agents.exists("pipeline-hub-flow-end"), "the version's flow is drawn");
  assert_eq!(versions(&agents, ready).len(), 2, "viewing stores nothing");

  agents.click("pipeline-hub-version-back");
  agents.until("the edit is shown again", |s| {
    s.exists(SAVE) && !s.exists("pipeline-hub-version-viewing")
  });
  assert_eq!(agents.value(DESCRIPTION), "Plan, then build");
}

#[test]
fn pipeline_versions_the_current_version_has_nothing_to_restore() {
  let (mut agents, _) = two_versions();
  open_list(&mut agents);
  agents.click("pipeline-hub-version-2");
  agents.until("the version is shown", |s| s.exists("pipeline-hub-version-viewing"));
  assert_eq!(
    agents.text("pipeline-hub-version-description-value"),
    "Plan, then build"
  );

  agents.click(RESTORE);
  agents.until("a pass", |_| true);
  assert!(
    agents.exists("pipeline-hub-version-viewing"),
    "pressing an unavailable Restore changes nothing"
  );
}

#[test]
fn pipeline_versions_restore_puts_the_version_into_the_edit_and_save_makes_it_the_next_version() {
  let (mut agents, ready) = two_versions();
  open_list(&mut agents);
  agents.click("pipeline-hub-version-1");
  agents.until("the version is shown", |s| s.exists(RESTORE));

  agents.click(RESTORE);
  agents.until("the edit holds v1", |s| {
    s.exists(SAVE) && !s.exists("pipeline-hub-version-viewing")
  });
  assert_eq!(agents.value(DESCRIPTION), "", "v1 has no description");
  agents.until("the row says it is not saved", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes")
  });
  assert_eq!(versions(&agents, ready).len(), 2, "nothing is stored before Save");

  agents.press_until(SAVE, "the next version", |s| versions(s, ready).len() == 3);
  let stored = versions(&agents, ready);
  assert_eq!(stored[2], stored[0], "v3 is v1 as it was");
  assert_eq!(stored[1].description(), "Plan, then build", "v2 is untouched");
}

#[test]
fn pipeline_versions_restore_puts_the_edit_back_to_the_stored_pipeline_on_cancel() {
  let (mut agents, ready) = two_versions();
  open_list(&mut agents);
  agents.click("pipeline-hub-version-1");
  agents.until("the version is shown", |s| s.exists(RESTORE));
  agents.click(RESTORE);
  agents.until("the edit holds v1", |s| s.exists(CANCEL));

  agents.click(CANCEL);
  agents.until("what is stored shows again", |s| {
    s.value(DESCRIPTION) == "Plan, then build"
  });
  assert_eq!(versions(&agents, ready).len(), 2);
}

#[test]
fn pipeline_versions_restore_asks_before_it_replaces_changes_that_are_not_saved() {
  let (mut agents, ready) = two_versions();
  agents.set(DESCRIPTION, "Not saved yet");
  agents.until("the row says it is not saved", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes")
  });
  open_list(&mut agents);
  agents.click("pipeline-hub-version-1");
  agents.until("the version is shown", |s| s.exists(RESTORE));

  agents.click(RESTORE);
  agents.until("the question", |s| s.exists("pipeline-hub-version-replace"));
  assert!(
    agents.exists("pipeline-hub-version-viewing"),
    "the version stays shown while it asks"
  );

  agents.click("pipeline-hub-version-replace-confirm");
  agents.until("the edit holds v1", |s| {
    s.exists(SAVE) && !s.exists("pipeline-hub-version-viewing")
  });
  assert_eq!(agents.value(DESCRIPTION), "", "what was typed was replaced by v1");
  assert!(!agents.exists("pipeline-hub-version-replace"));
}

#[test]
fn pipeline_versions_back_to_editing_keeps_the_changes_made_before_the_version_was_opened() {
  let (mut agents, ready) = two_versions();
  agents.set(DESCRIPTION, "Not saved yet");
  agents.until("the row says it is not saved", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes")
  });
  open_list(&mut agents);
  agents.click("pipeline-hub-version-1");
  agents.until("the version is shown", |s| s.exists("pipeline-hub-version-back"));

  agents.click("pipeline-hub-version-back");
  agents.until("the edit is shown", |s| s.exists(SAVE));
  assert_eq!(agents.value(DESCRIPTION), "Not saved yet");
}
