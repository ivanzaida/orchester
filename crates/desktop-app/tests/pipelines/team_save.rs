//! A team pipeline's *Save* waits for the team server (PV2-34, D-042, D-043), against the stand-in team server: while
//! the request is out the page says *Saving…*, keeps the edit and asks before leaving; once the server took it the
//! version is stored; a refusal or a server that cannot be reached leaves the page as it was before the press, the edit
//! unsaved, with why where the save's refusals show, and stores nothing; a rename goes to the server too, and when only
//! the name was taken the page says so and keeps the steps to save.
use std::time::Duration;

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{resource::Pipeline, store::ResourceStore};

use crate::{
  pipelines_support::Editor,
  support::{project_id, team_link::STAND_IN_SLUG},
};

/// A team project's pipeline open with its description changed, not saved yet.
fn changed() -> (Editor, orchester_core::id::ResourceId) {
  let mut editor = Editor::team();
  let id = editor.published();
  editor.open(id);
  editor.set("pipeline-description", "Changed.");
  editor.until("Save", |e| e.exists("pipeline-save"));
  (editor, id)
}

fn team(editor: &Editor) -> &TeamServer {
  editor.team.as_ref().expect("a team project")
}

/// The number and description of `id`'s latest version here.
fn latest(editor: &Editor, id: orchester_core::id::ResourceId) -> (u32, String) {
  (editor.version(id), editor.stored(id).description().to_owned())
}

/// How many versions the team server holds of `id`, and its name there.
fn on_server(editor: &Editor, id: orchester_core::id::ResourceId) -> (usize, String) {
  team(editor)
    .resources(STAND_IN_SLUG)
    .into_iter()
    .find(|(resource, _)| resource["id"] == id.to_string())
    .map(|(resource, versions)| (versions.len(), resource["name"].as_str().unwrap_or_default().to_owned()))
    .expect("the team server holds the pipeline")
}

#[test]
fn pipelines_team_save_shows_saving_until_the_server_answers() {
  let (mut editor, id) = changed();
  team(&editor).slow_resource_writes(Duration::from_millis(1500));

  editor.click("pipeline-save");
  editor.until("saving", |e| e.text("pipeline-save-status") == "Saving…");
  assert_eq!(
    latest(&editor, id),
    (1, String::new()),
    "nothing is stored while the server answers"
  );
  assert_eq!(
    editor.value("pipeline-description"),
    "Changed.",
    "the edit stays in view"
  );
  editor
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Tasks(Default::default())));
  editor.until("the leave question", |e| e.exists("pipeline-leave-question"));

  editor.click("pipeline-leave-keep");
  editor.until("v2", |e| e.version(id) == 2);
  editor.until("saved", |e| e.text("pipeline-save-status") == "Saved as v2");
  assert_eq!(latest(&editor, id).1, "Changed.");
  assert_eq!(on_server(&editor, id).0, 2);
}

#[test]
fn pipelines_team_save_a_refusal_keeps_the_edit_with_why() {
  let (mut editor, id) = changed();
  team(&editor).reject_content(STAND_IN_SLUG, "body");

  editor.click("pipeline-save");
  editor.until("the refusal", |e| e.exists("pipeline-write-error"));

  let said = editor.text("pipeline-write-error");
  assert!(said.contains("does not accept its content"), "{said}");
  assert_eq!(
    editor.value("pipeline-description"),
    "Changed.",
    "the edit stays as typed"
  );
  assert!(editor.exists("pipeline-save"), "the edit is still not saved");
  assert_eq!(editor.text("pipeline-save-status"), "Unsaved changes");
  assert_eq!(latest(&editor, id), (1, String::new()), "nothing is stored");
  assert_eq!(on_server(&editor, id).0, 1);
}

#[test]
fn pipelines_team_save_an_unreachable_server_keeps_the_edit_with_why() {
  let (mut editor, id) = changed();
  team(&editor).set_down(true);

  editor.click("pipeline-save");
  editor.until("the refusal", |e| e.exists("pipeline-write-error"));

  let said = editor.text("pipeline-write-error");
  assert!(
    said.contains("The team server can't be reached, so nothing was saved"),
    "{said}"
  );
  assert_eq!(editor.value("pipeline-description"), "Changed.");
  assert!(editor.exists("pipeline-save"));
  assert_eq!(latest(&editor, id), (1, String::new()));
}

#[test]
fn pipelines_team_save_a_rename_goes_to_the_team_server() {
  let mut editor = Editor::team();
  let id = editor.published();
  editor.open(id);

  editor.set("pipeline-summary-name", "Gates");
  editor.until("Save", |e| e.exists("pipeline-save"));
  editor.click("pipeline-save");
  editor.until("renamed", |e| {
    e.db()
      .resource(id)
      .expect("read")
      .expect("the pipeline")
      .name()
      .as_str()
      == "Gates"
  });

  assert_eq!(
    on_server(&editor, id),
    (1, "Gates".to_owned()),
    "a rename alone publishes no version"
  );
  editor.until("saved", |e| e.exists("pipeline-header-history"));
}

#[test]
fn pipelines_team_save_says_when_only_the_new_name_was_saved() {
  let (mut editor, id) = changed();
  editor.set("pipeline-summary-name", "Gates");
  // The server checks the content of versions only: the rename, sent first, is taken, and the steps are not.
  team(&editor).reject_content(STAND_IN_SLUG, "body");

  editor.click("pipeline-save");
  editor.until("what was saved", |e| e.exists("pipeline-write-error"));

  let said = editor.text("pipeline-write-error");
  assert!(said.contains("does not accept its content"), "{said}");
  assert!(
    said.contains("The new name was saved for the team; the steps were not."),
    "{said}"
  );
  assert_eq!(on_server(&editor, id), (1, "Gates".to_owned()));
  assert_eq!(latest(&editor, id), (1, String::new()), "the steps are not stored");
  assert_eq!(
    editor.value("pipeline-description"),
    "Changed.",
    "the steps stay to save"
  );
  assert!(editor.exists("pipeline-save"));
  assert!(
    editor.db().latest_version::<Pipeline>(id).expect("read").is_some(),
    "the pipeline is kept"
  );
}

#[test]
fn pipelines_team_save_a_member_reads_the_pipeline_and_adds_no_step() {
  let mut editor = Editor::team();
  let id = editor.published();
  // The person is a member from here on: a member cannot make a team pipeline, so the admin made it first.
  editor
    .db()
    .record_sharing_state(Some(orchester_core::sharing::MemberRole::Member), None, None)
    .expect("the role");

  editor.go(ProjectPage::Pipeline(id, Default::default()));
  editor.until("the flow", |e| e.exists("pipeline-flow"));

  assert!(
    !editor.exists("pipeline-add-step"),
    "a member's flow offers no step to add"
  );
  assert!(!editor.exists("pipeline-save"));
  assert!(
    !editor.exists("pipeline-problem-strip"),
    "what blocks saving is not a member's to fix"
  );
  assert_eq!(latest(&editor, id), (1, String::new()));
}
