//! The pipeline page has one *Save*, which publishes the next version (PV2-32, D-033): in a team project and in a
//! local one; unavailable with why while the edit cannot be saved; no *Restore as draft* or *Compare*; a rename alone
//! saves a local pipeline's name. A team pipeline's save waiting for the team server is [`super::team_save`].
use std::{
  num::NonZeroU32,
  time::{Duration, Instant},
};

use orchester_app::{
  routes::{ProjectPage, Route},
  ui::tooltip::TOOLTIP_DELAY,
};
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, ResourceKind, VersionNumber},
  store::ResourceStore,
};

use crate::{pipelines_support::Editor, support::project_id};

fn latest(editor: &Editor, id: ResourceId) -> (VersionNumber, Pipeline) {
  let version = editor
    .db()
    .latest_version::<Pipeline>(id)
    .expect("read")
    .expect("a version");
  (version.number(), version.body().clone())
}

fn two() -> VersionNumber {
  VersionNumber::new(NonZeroU32::new(2).expect("two"))
}

/// The words of the tooltip `tip` of the element `id`, once the pointer rests on it.
fn tooltip(editor: &mut Editor, id: &str, tip: &str) -> String {
  let bounds = editor
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|found| found.bounds())
    .expect("laid out");
  let scale = editor.window.tree.scale_factor();
  editor.window.tree.mouse_move(
    (bounds.x + bounds.width / 2.0) * scale,
    (bounds.y + bounds.height / 2.0) * scale,
  );
  editor.window.pass();
  editor
    .window
    .tree
    .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
  editor.window.pass();
  editor.text(tip)
}

/// Changes the description, checks that nothing saves on its own and that the one *Save* stands where *Save draft* and
/// *Publish vN* were, then saves: v2 is published.
fn one_save_publishes(editor: &mut Editor) {
  let id = editor.published();
  editor.open(id);
  assert!(editor.exists("pipeline-header-history"), "nothing to save yet");
  assert!(!editor.exists("pipeline-save"));

  editor.set("pipeline-description", "Changed.");
  editor.until("Save", |e| e.exists("pipeline-save"));
  assert_eq!(editor.text("pipeline-save"), "Save");
  assert!(!editor.exists("pipeline-publish"), "no second button publishes");
  assert!(!editor.exists("pipeline-header-history"));
  // What was autosave's rest passes: still nothing is saved.
  std::thread::sleep(Duration::from_millis(900));
  editor.arm();
  assert_eq!(latest(editor, id).0, VersionNumber::FIRST, "nothing saves on its own");

  editor.click("pipeline-save");
  editor.until("v2", |e| latest(e, id).0 == two());
  assert_eq!(latest(editor, id).1.description(), "Changed.");
  editor.until("saved", |e| e.exists("pipeline-header-history"));
  assert_eq!(editor.text("pipeline-save-status"), "Saved as v2");
  assert!(
    editor.db().draft::<Pipeline>(id).expect("read").is_none(),
    "a pipeline holds no draft"
  );
}

#[test]
fn pipelines_one_save_publishes_in_a_local_project() {
  one_save_publishes(&mut Editor::new());
}

#[test]
fn pipelines_one_save_publishes_in_a_team_project() {
  one_save_publishes(&mut Editor::team());
}

#[test]
fn pipelines_one_save_is_unavailable_with_why_while_a_step_is_not_ready() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-add-step");
  editor.until("Save", |e| e.exists("pipeline-save"));

  let why = tooltip(&mut editor, "pipeline-save", "pipeline-save-why-tooltip");
  assert!(why.ends_with("has no agent profile"), "{why}");
  editor.click("pipeline-save");
  editor.arm();
  assert_eq!(
    latest(&editor, id).0,
    VersionNumber::FIRST,
    "an unavailable Save stores nothing"
  );
}

#[test]
fn pipelines_one_save_a_viewed_version_has_no_restore_or_compare() {
  let mut editor = Editor::new();
  let id = editor.published();
  editor.open(id);
  editor.click("pipeline-history");
  editor.until("versions", |e| e.exists("pipeline-version-1"));
  editor.click("pipeline-version-1");
  editor.until("the viewed version", |e| e.exists("pipeline-viewing-banner"));

  for gone in [
    "pipeline-restore",
    "pipeline-compare",
    "pipeline-restore-question",
    "pipeline-save",
  ] {
    assert!(!editor.exists(gone), "{gone} is gone");
  }
  assert_eq!(editor.text("pipeline-back-draft"), "Back to editing");
  editor.click("pipeline-back-draft");
  editor.until("the edit", |e| !e.exists("pipeline-viewing-banner"));
}

#[test]
fn pipelines_one_save_a_rename_alone_saves_a_local_pipelines_name() {
  let mut editor = Editor::new();
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
    latest(&editor, id).0,
    VersionNumber::FIRST,
    "a rename alone publishes no version"
  );
  editor.until("saved", |e| e.exists("pipeline-header-history"));
}

/// Pipelines of the project, as stored.
fn pipelines(editor: &Editor) -> Vec<orchester_core::resource::Resource> {
  editor.db().resources(ResourceKind::Pipeline).expect("read")
}

/// The pipeline the hub's panel shows, by the route.
fn shown(editor: &Editor) -> Option<ResourceId> {
  match editor.window.navigation().route_now() {
    Route::Project(_, ProjectPage::PipelinesHub(view)) => view.item,
    _ => None,
  }
}

/// A local project's new pipeline, made in the hub's panel (`Pipelines · v2` P14), is stored only by *Create v1*, as
/// its v1, once every step is ready: a copy of a published pipeline.
#[test]
fn pipelines_one_save_create_v1_publishes_a_ready_pipeline_in_a_local_project() {
  let mut editor = Editor::new();
  let source = editor.published();
  editor.go(ProjectPage::NewPipeline);
  editor.until("New pipeline", |e| e.exists("pipeline-new-name"));
  assert!(editor.text("pipeline-new-create").contains("Create v1"));

  editor.set("pipeline-new-name", "Delivery copy");
  editor.click("pipeline-new-start-copy-pick");
  editor.until("the pipelines to copy", |e| {
    e.window
      .tree
      .get_element_by_id_mut("pipeline-new-copy")
      .and_then(|select| select.as_select())
      .is_some_and(|select| !select.labels().is_empty())
  });
  editor.choose("pipeline-new-copy", "Delivery · v1");
  assert_eq!(pipelines(&editor).len(), 1, "nothing is stored before Create v1");
  editor.click("pipeline-new-create");
  editor.until("the new pipeline's own panel", |e| {
    !e.exists("pipeline-new-panel") && e.exists("pipeline-hub-panel")
  });

  let id = shown(&editor).expect("the hub shows the pipeline");
  assert_ne!(id, source);
  let (number, body) = latest(&editor, id);
  assert_eq!(number, VersionNumber::FIRST, "created as v1");
  assert_eq!(body.steps().len(), latest(&editor, source).1.steps().len());
  assert!(editor.db().draft::<Pipeline>(id).expect("read").is_none());
}

/// A local project's new pipeline whose steps are not all ready cannot be created, and the bar says why; nothing is
/// stored: the starter's steps have no profile or tool in a project without the ones named for them.
#[test]
fn pipelines_one_save_create_v1_is_off_with_why_while_a_step_is_not_ready() {
  let mut editor = Editor::new();
  editor.go(ProjectPage::NewPipeline);
  editor.until("New pipeline", |e| e.exists("pipeline-new-name"));
  editor.set("pipeline-new-name", "Feature delivery");
  assert_eq!(
    editor
      .text("team-hub-actions-hint")
      .split_whitespace()
      .collect::<Vec<_>>()
      .join(" "),
    "Make 01 Research , 02 Plan and 4 more ready · nothing is saved yet"
  );
  editor.click("pipeline-new-create");
  editor.arm();

  assert!(pipelines(&editor).is_empty(), "nothing is stored");
  assert!(editor.exists("pipeline-new-name"), "the panel stays");
}

/// An old link to New pipeline with the empty start opens on the starter: an empty pipeline can never be saved
/// (D-040).
#[test]
fn pipelines_one_save_the_empty_start_is_not_offered() {
  // A team project: a local one's Pipelines is the hub, which has New of its own.
  let mut editor = Editor::team();
  editor
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::NewEmptyPipeline));
  editor.until("New pipeline", |e| e.exists("pipeline-start-0"));
  assert!(!editor.exists("pipeline-start-2"), "no empty pipeline is offered");

  editor.go(ProjectPage::Pipelines);
  editor.until("the empty list", |e| e.exists("pipelines-start"));
  assert!(!editor.exists("pipeline-empty-blank"), "no Empty pipeline row");
  assert!(!editor.exists("pipeline-empty-start"), "no Start empty");
}
