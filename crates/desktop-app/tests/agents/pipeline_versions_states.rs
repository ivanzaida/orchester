//! The states of *Versions* in a local pipeline's panel that `pipeline_versions` does not reach (`Pipelines · v2` P12,
//! PV2-11): the list that cannot be read and reads again, the list of a pipeline never published, runs pinned to an
//! older version, *Restore* while a save runs, the question before *Restore* cancelled, what *Back* says, and leaving
//! while a version is shown. `cargo test -p orchester-app --test agents pipeline_versions_states`.

use std::path::PathBuf;

use orchester_app::routes::PipelinesFilter;
use orchester_core::time::Timestamp;
use orchester_store::ProjectDb;

use super::{
  active::{pipeline, running},
  harness::Agents,
  pipeline_hub::{open, project, view},
  pipeline_versions::{BUTTON, CANCEL, DESCRIPTION, RESTORE, SAVE, open_list, publish_second, two_versions, versions},
};

const VIEWING: &str = "pipeline-hub-version-viewing";
const BACK: &str = "pipeline-hub-version-back";
const DISCARD: &str = "pipeline-hub-leave-discard";
const KEEP: &str = "pipeline-hub-leave-cancel";

fn database(agents: &Agents) -> PathBuf {
  agents.root.join(".orchester/orchester.db")
}

/// Opens version `number` of the open pipeline from the list and waits for the read-only view.
fn view_version(agents: &mut Agents, number: u32) {
  open_list(agents);
  agents.click(&format!("pipeline-hub-version-{number}"));
  agents.until("the version is shown", |s| s.exists(VIEWING));
}

/// What *Back* says: the button's label, which is its only text.
fn back_label(agents: &mut Agents) -> String {
  agents.text("pipeline-hub-version-back-label")
}

#[test]
fn pipeline_versions_list_that_cannot_be_read_says_so_and_try_again_reads_it_again() {
  let (mut agents, _) = two_versions();
  let connection = rusqlite::Connection::open(database(&agents)).expect("database");
  let schema: u32 = connection
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("schema");
  connection
    .pragma_update(None, "user_version", 99999)
    .expect("a schema this app does not know");

  agents.click(BUTTON);
  agents.until("the error", |s| s.exists("pipeline-hub-versions-error"));
  assert!(agents.exists("pipeline-hub-versions-retry"));
  assert!(
    !agents.exists("pipeline-hub-version-2"),
    "no row stands for a version that was not read"
  );

  // Trying again while it still cannot be read says so again; once it can, the versions are there.
  agents.click("pipeline-hub-versions-retry");
  agents.until("it cannot be read again", |s| s.exists("pipeline-hub-versions-error"));
  connection
    .pragma_update(None, "user_version", schema)
    .expect("the schema again");
  agents.press_until("pipeline-hub-versions-retry", "the versions", |s| {
    s.exists("pipeline-hub-version-2") && s.exists("pipeline-hub-version-1")
  });
  assert!(!agents.exists("pipeline-hub-versions-error"));
}

#[test]
fn pipeline_versions_list_counts_the_runs_that_started_on_an_older_version() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let snapshot = pipeline(&mut db, profile);
  let id = snapshot.pipeline().resource;
  running(&mut db, snapshot.clone(), Timestamp::now());
  running(&mut db, snapshot, Timestamp::now());
  drop(db);
  publish_second(&agents, id, "Second version");
  open(&mut agents, PipelinesFilter::All, Some(id));
  agents.until("the panel", |s| s.exists(BUTTON));

  open_list(&mut agents);
  let first = agents.text("pipeline-hub-version-1-meta");
  assert!(first.ends_with(" · 2 runs on it"), "{first}");
  let current = agents.text("pipeline-hub-version-2-meta");
  assert!(current.ends_with(" · used by 2 tasks"), "{current}");
}

#[test]
fn pipeline_versions_restore_waits_for_a_save_that_is_under_way() {
  let (mut agents, ready) = two_versions();
  let path = database(&agents);
  let mut connection = rusqlite::Connection::open(path).expect("database");
  let lock = connection
    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
    .expect("a write that holds the database");
  agents.set(DESCRIPTION, "Pending work");
  agents.click(SAVE);
  agents.until("a pass", |_| true);
  view_version(&mut agents, 1);

  agents.click(RESTORE);
  agents.until("a pass", |_| true);
  assert!(
    agents.exists(VIEWING),
    "Restore does nothing while the save is under way"
  );
  assert!(!agents.exists("pipeline-hub-version-replace"));

  lock.commit().expect("the write ends");
  agents.until("the save ends", |s| versions(s, ready).len() == 3);
  agents.press_until(RESTORE, "the version is in the editor", |s| !s.exists(VIEWING));
  agents.until("the editor", |s| s.exists(SAVE));
  assert_eq!(agents.value(DESCRIPTION), "", "v1 has no description");
}

#[test]
fn pipeline_versions_cancelling_the_question_before_restore_leaves_the_edit_as_it_was() {
  let (mut agents, ready) = two_versions();
  agents.set(DESCRIPTION, "Not saved yet");
  agents.until("held", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes")
  });
  view_version(&mut agents, 1);
  agents.click(RESTORE);
  agents.until("the question", |s| s.exists("pipeline-hub-version-replace"));

  agents.click("pipeline-hub-version-replace-cancel");
  agents.until("the question is gone", |s| !s.exists("pipeline-hub-version-replace"));
  assert!(agents.exists(VIEWING), "the version stays shown");
  agents.click(BACK);
  agents.until("the edit", |s| s.exists(SAVE));
  assert_eq!(agents.value(DESCRIPTION), "Not saved yet", "the edit was not touched");
  assert_eq!(versions(&agents, ready).len(), 2);
}

#[test]
fn pipeline_versions_the_head_names_the_version_that_is_shown() {
  let (mut agents, _) = two_versions();
  let before = agents.text("team-hub-panel-version");
  assert!(before.starts_with("v2 · published "), "{before}");

  view_version(&mut agents, 1);
  let shown = agents.text("team-hub-panel-version");
  assert!(shown.starts_with("Viewing v1 · published "), "{shown}");
  assert_eq!(
    agents.text("pipeline-hub-version-viewing-title"),
    "Read-only",
    "the head names the version; the notice says only that it is read-only"
  );

  agents.click(BACK);
  agents.until("the edit", |s| s.exists(SAVE));
  assert_eq!(agents.text("team-hub-panel-version"), before);
}

#[test]
fn pipeline_versions_the_notice_says_what_restore_does_and_back_says_where_it_goes() {
  let (mut agents, _) = two_versions();
  view_version(&mut agents, 1);
  assert_eq!(
    agents.text("pipeline-hub-version-viewing-body"),
    "Restore puts v1 in the editor as unsaved changes · Save makes v3"
  );
  assert_eq!(
    back_label(&mut agents),
    "Back to v2",
    "the latest version, with no changes held"
  );
  assert!(!agents.exists("pipeline-hub-version-kept"), "no changes to keep");

  agents.click(BACK);
  agents.until("the edit", |s| s.exists(SAVE));
  agents.set(DESCRIPTION, "Not saved yet");
  agents.until("held", |s| s.exists(CANCEL));
  view_version(&mut agents, 1);
  assert_eq!(back_label(&mut agents), "Back to your changes");
  assert_eq!(
    agents.text("pipeline-hub-version-kept"),
    "Your unsaved changes are kept"
  );
}

#[test]
fn pipeline_versions_leaving_while_a_version_is_shown_asks_only_when_changes_would_be_lost() {
  let (mut agents, [ready, draft, _]) = project();
  publish_second(&agents, ready, "Second version");
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(BUTTON));

  // No changes: another pipeline opens at once, and coming back shows the pipeline, not the version.
  view_version(&mut agents, 1);
  agents.click(&format!("pipeline-hub-row-{draft}"));
  agents.until("the other pipeline", |s| {
    s.text("team-hub-panel-title") == "Dependency update"
  });
  assert!(!agents.exists(DISCARD), "a version that is only looked at asks nothing");
  assert!(!agents.exists(VIEWING));
  agents.click(&format!("pipeline-hub-row-{ready}"));
  agents.until("the first pipeline", |s| {
    s.text("team-hub-panel-title") == "Feature delivery"
  });
  assert!(
    !agents.exists(VIEWING),
    "the view of a version does not outlast its panel"
  );
}

#[test]
fn pipeline_versions_leaving_with_unsaved_changes_while_a_version_is_shown_asks_first() {
  let (mut agents, [ready, draft, _]) = project();
  publish_second(&agents, ready, "Second version");
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(BUTTON));
  agents.set(DESCRIPTION, "Not saved yet");
  agents.until("held", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes")
  });
  view_version(&mut agents, 1);

  // Another pipeline: the question comes, *Keep editing* leaves the version shown, *Discard changes* goes.
  agents.click(&format!("pipeline-hub-row-{draft}"));
  agents.until("the question", |s| s.exists(DISCARD));
  assert_eq!(
    agents.window.navigation().route_now(),
    view(PipelinesFilter::All, Some(ready)),
    "nothing moved while it asks"
  );
  agents.click(KEEP);
  agents.until("kept", |s| !s.exists(DISCARD));
  assert!(agents.exists(VIEWING), "Keep editing leaves everything as it was");

  // A tab: the same question; discarding goes there (empty: a pipeline holds no draft) with no version shown.
  agents.click("pipeline-hub-filter-drafts");
  agents.until("the question again", |s| s.exists(DISCARD));
  agents.click(DISCARD);
  agents.until("the tab", |s| {
    s.window.navigation().route_now() == view(PipelinesFilter::Drafts, None)
  });
  assert!(!agents.exists(VIEWING));
}
