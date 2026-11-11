//! What the Save bar of a local pipeline's panel says and does around a save (PV2-10, round 2): an edit put back to the
//! latest version, the version a save made, the hint over unsaved changes, a refusal that goes with the edit it
//! refused, the question about leaving while a save is under way and when a tab is switched or closed, and the words
//! of the question a source asks. `cargo test -p orchester-app --test agents pipeline_save_notes`.

use orchester_app::{
  navigation::press::OpenIn,
  routes::{PipelinesFilter, Route},
};
use orchester_core::{id::ResourceId, resource::Pipeline, store::ResourceStore};
use orchester_store::ProjectDb;
use orchester_workspace::pipelines::{
  data::OpenedPipeline,
  write::{Allowed, save_in},
};

use super::{
  harness::Agents,
  pipeline_hub::{open, project, view},
};

const SAVE: &str = "pipeline-hub-save";
const CANCEL: &str = "pipeline-hub-cancel";
const DESCRIPTION: &str = "pipeline-hub-description";
const HINT: &str = "team-hub-actions-hint";
const NOTICE: &str = "pipeline-hub-bar-notice";
const DISCARD: &str = "pipeline-hub-leave-discard";
const KEEP: &str = "pipeline-hub-leave-cancel";

/// How many versions the pipeline has, and whether a draft is stored.
fn counts(agents: &Agents, id: ResourceId) -> (usize, bool) {
  let db = ProjectDb::open(&agents.root).expect("database");
  (
    db.versions::<Pipeline>(id).expect("versions").len(),
    db.draft::<Pipeline>(id).expect("draft").is_some(),
  )
}

#[test]
fn pipeline_save_note_says_the_version_it_made_until_the_next_change_and_nothing_after_cancel() {
  let (mut agents, [ready, ..]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE));
  agents.set(DESCRIPTION, "One");
  agents.press_until(SAVE, "v2", |s| counts(s, ready).0 == 2);
  agents.until("the line", |s| s.exists(NOTICE));
  assert_eq!(agents.text(NOTICE), "Saved as v2 · the next run uses it");

  agents.set(DESCRIPTION, "Two");
  agents.until("it goes with the next change", |s| !s.exists(NOTICE));

  // Cancel puts back what was saved and says nothing.
  agents.click(CANCEL);
  agents.until("cancelled", |s| s.value(DESCRIPTION) == "One");
  assert!(!agents.exists(NOTICE));
}

#[test]
fn pipeline_save_note_the_field_loses_the_focus_after_save() {
  let (mut agents, [ready, ..]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE));
  agents.set(DESCRIPTION, "Focused");
  agents
    .window
    .tree
    .get_element_by_id_mut(DESCRIPTION)
    .expect("the field")
    .focus();
  agents.window.pass();
  let focused = |agents: &Agents| {
    agents
      .window
      .tree
      .focused_element()
      .is_some_and(|element| element.id() == Some(DESCRIPTION))
  };
  assert!(focused(&agents), "the field has the focus while it is typed in");

  agents.press_until(SAVE, "v2", |s| counts(s, ready).0 == 2);
  agents.until("the line", |s| s.exists(NOTICE));
  assert!(!focused(&agents), "Save took the focus off the field");
}

#[test]
fn pipeline_save_note_the_bar_says_unsaved_changes_while_the_edit_has_some() {
  let (mut agents, [ready, ..]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE));
  assert_eq!(
    agents.text(HINT),
    "Save makes v2 · runs keep the version they started on"
  );

  agents.set(DESCRIPTION, "Changed");
  agents.until("the hint", |s| {
    s.text(HINT) == "Unsaved changes · Save makes v2 · runs keep the version they started on"
  });
  agents.click(CANCEL);
  agents.until("back", |s| {
    s.text(HINT) == "Save makes v2 · runs keep the version they started on"
  });
}

#[test]
fn pipeline_save_note_a_refusal_goes_when_the_edit_changes() {
  let (mut agents, [ready, ..]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE));
  agents.set(DESCRIPTION, "Mine");
  let before = OpenedPipeline::load_in(&agents.root, ready).expect("pipeline");
  let theirs = before.body.clone().with_description("Theirs".into());
  save_in(&agents.root, &before, &theirs, &Allowed::default()).expect("their version");
  agents.click(SAVE);
  agents.until("the refusal", |s| s.exists("pipeline-hub-refused"));

  agents.set(DESCRIPTION, "Mine, changed");
  agents.until("the refusal is gone", |s| !s.exists("pipeline-hub-refused"));
  assert_eq!(agents.value(DESCRIPTION), "Mine, changed");
}

#[test]
fn pipeline_save_note_leaving_while_a_save_is_under_way_does_not_offer_to_discard() {
  let (mut agents, [ready, draft, _]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE));
  agents.set(DESCRIPTION, "Being saved");

  // Another connection holds the write lock, so the save waits for it.
  let lock = rusqlite::Connection::open(agents.root.join(".orchester/orchester.db")).expect("database");
  lock.execute_batch("BEGIN IMMEDIATE").expect("the write lock");
  agents.click(SAVE);
  agents.until("a pass", |_| true);
  agents.click(&format!("pipeline-hub-row-{draft}"));
  agents.until("the question", |s| {
    s.exists("pipeline-hub-leave-saving-cancel") || s.exists(DISCARD)
  });
  assert!(
    !agents.exists(DISCARD),
    "an edit that is being saved cannot be discarded"
  );
  assert!(
    agents.exists("pipeline-hub-leave-saving-cancel"),
    "it says it is saving"
  );
  assert_eq!(
    agents.window.navigation().route_now(),
    view(PipelinesFilter::All, Some(ready)),
    "nothing moved"
  );

  lock.execute_batch("ROLLBACK").expect("the lock goes");
  agents.until("the save ended and the question closed", |s| {
    counts(s, ready).0 == 2 && !s.exists("pipeline-hub-leave-saving-cancel")
  });
  assert!(!agents.exists(DISCARD));
}

/// The panel holds a description that is not saved.
fn editing() -> (Agents, ResourceId) {
  let (mut agents, [ready, ..]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE));
  agents.set(DESCRIPTION, "Not saved yet");
  agents.until("held", |s| s.text(HINT).starts_with("Unsaved changes"));
  (agents, ready)
}

#[test]
fn pipeline_save_note_switching_to_another_tab_asks_first() {
  let (mut agents, ready) = editing();
  let here = view(PipelinesFilter::All, Some(ready));
  let tab = agents.window.navigation().tabs_now().active().id();

  agents.window.navigation().open_tab();
  agents.until("the question", |s| s.exists(DISCARD));
  assert_eq!(
    agents.window.navigation().route_now(),
    here,
    "nothing moved while it asks"
  );
  agents.click(KEEP);
  agents.until("kept", |s| !s.exists(DISCARD));
  assert_eq!(agents.value(DESCRIPTION), "Not saved yet");

  agents.window.navigation().open_tab();
  agents.until("the question again", |s| s.exists(DISCARD));
  assert!(
    agents
      .text("pipeline-hub-leave-what")
      .starts_with("Opening anything else"),
    "another tab is opened, not this one closed"
  );
  agents.click(DISCARD);
  agents.until("the new tab", |s| s.window.navigation().tabs_now().active().id() != tab);
}

#[test]
fn pipeline_save_note_closing_the_tab_asks_first() {
  let (mut agents, ready) = editing();
  let here = view(PipelinesFilter::All, Some(ready));
  agents
    .window
    .navigation()
    .open(Route::AllProjects, OpenIn::BackgroundTab);
  agents.window.pass();
  let tabs = agents.window.navigation().tabs_now().tabs().len();

  agents.window.navigation().close_active();
  agents.until("the question", |s| s.exists(DISCARD));
  assert!(
    agents
      .text("pipeline-hub-leave-what")
      .starts_with("Closing the tab now puts Feature delivery back"),
    "it says close: {}",
    agents.text("pipeline-hub-leave-what")
  );
  assert_eq!(agents.window.navigation().route_now(), here);
  assert_eq!(
    agents.window.navigation().tabs_now().tabs().len(),
    tabs,
    "nothing closed while it asks"
  );
  agents.click(DISCARD);
  agents.until("closed", |s| s.window.navigation().tabs_now().tabs().len() == tabs - 1);
}

#[test]
fn pipeline_save_note_a_source_that_is_added_or_removed_asks_in_words_that_fit_both() {
  let (mut agents, [ready, ..]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE));
  let folder = tempfile::tempdir().expect("a folder");

  // Added.
  agents.stand_ins.choose(Some(folder.path().to_path_buf()));
  agents.press_until("pipeline-hub-sources-add-add", "the pill", |s| {
    s.exists("pipeline-hub-source-0")
  });
  agents.click(SAVE);
  agents.until("the question", |s| s.exists("pipeline-hub-confirm-confirm"));
  let added = agents.text("pipeline-hub-confirm");
  assert!(added.contains("Allow these changes to the pipeline?"), "{added}");
  assert!(added.contains("sources"), "{added}");
  assert!(added.contains("differ"), "it says they differ: {added}");
  assert!(
    !added.to_lowercase().contains("widen"),
    "it does not say it widens: {added}"
  );
  agents.press_until("pipeline-hub-confirm-confirm", "v2", |s| counts(s, ready).0 == 2);

  // Removed.
  agents.until("the pill", |s| s.exists("pipeline-hub-source-0-remove"));
  agents.click("pipeline-hub-source-0-remove");
  agents.until("gone", |s| !s.exists("pipeline-hub-source-0"));
  agents.click(SAVE);
  agents.until("the question", |s| s.exists("pipeline-hub-confirm-confirm"));
  let removed = agents.text("pipeline-hub-confirm");
  assert!(removed.contains("Allow these changes to the pipeline?"), "{removed}");
  assert!(removed.contains("sources"), "{removed}");
  assert!(
    !removed.to_lowercase().contains("widen"),
    "it does not say it widens: {removed}"
  );
  assert!(
    !removed.contains("added"),
    "it does not say something was added: {removed}"
  );
}

#[test]
fn pipeline_save_note_ready_describes_the_edit_while_the_panel_holds_one() {
  const LABEL: &str = "team-hub-readiness-label";
  let (mut agents, [ready, ..]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE));
  assert_eq!(agents.text(LABEL), "Ready?", "the version tasks use");

  // The panel shows the edit, so Ready? reads the edit as it stands, under the same title.
  agents.set(DESCRIPTION, "Changed");
  agents.until("the edit", |s| s.text(HINT).starts_with("Unsaved changes"));
  assert_eq!(agents.text(LABEL), "Ready?");
  assert_eq!(agents.text("team-hub-readiness-count"), "all set");
  agents.click(CANCEL);
  agents.until("the version again", |s| s.text(HINT).starts_with("Save makes"));
  assert_eq!(agents.text(LABEL), "Ready?");

  // After a save the panel shows the new version.
  agents.set(DESCRIPTION, "Changed again");
  agents.until("the edit", |s| s.text(HINT).starts_with("Unsaved changes"));
  agents.press_until(SAVE, "v2", |s| counts(s, ready).0 == 2);
  assert_eq!(agents.text(LABEL), "Ready?");
}
