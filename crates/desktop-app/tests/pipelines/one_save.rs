//! The pipeline page has one *Save*, which publishes the next version (PV2-32, D-033): in a team project and in a
//! local one; unavailable with why while the edit cannot be saved; no *Restore as draft* or *Compare*; a rename alone
//! saves a local pipeline's name; and a team profile's conflict still offers *Open my draft on vN*. A team pipeline's
//! save waiting for the team server is [`super::team_save`].
use std::{
  num::NonZeroU32,
  time::{Duration, Instant},
};

use orchester_app::{
  routes::{AgentTab, ProjectPage, Route},
  ui::tooltip::TOOLTIP_DELAY,
};
use orchester_core::{
  blob::ContentHash,
  id::{ConnectionId, ResourceId, UserId, VersionId},
  name::Name,
  resource::{
    AgentProfile, Draft, ModelId, Pipeline, PromptText, ProviderSettings, ResourceBody, ResourceKind, VersionNumber,
  },
  sharing::SharedPerson,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{CachedVersion, ProjectDb};

use crate::{pipelines_support::Editor, support::project_id};

const HASH: &str = "5f1d6b0f0e2b9a8c7d6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c";

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

/// `body` as the server numbered it `number`, published by olha.
fn cached<B: ResourceBody + Clone>(number: VersionNumber, body: &B) -> CachedVersion<B> {
  CachedVersion {
    number,
    published_at: Timestamp::now(),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "olha".to_owned(),
    },
    server_body: serde_json::to_value(body).expect("a body"),
    hash: ContentHash::new(HASH).expect("hash"),
    references: Vec::new(),
    local_body: body.clone(),
    waits_here: false,
  }
}

/// The server numbers the first version of `id` as v1, then `mine`, published here from v1, meets olha's v2 `theirs`.
fn meets_a_teammate<B: ResourceBody + Clone>(db: &mut ProjectDb, id: ResourceId, mine: VersionId, theirs: &B) {
  db.cache_shared_version::<B>(id, &cached(two(), theirs))
    .expect("cached");
  db.record_publish_conflict(mine, two()).expect("recorded");
}

fn first_numbered<B: ResourceBody + Clone>(db: &mut ProjectDb, id: ResourceId) -> B {
  let first = db.latest_version::<B>(id).expect("read").expect("a version");
  let body = first.body().clone();
  db.confirm_publish::<B>(first.id(), &cached(VersionNumber::FIRST, &body))
    .expect("numbered");
  body
}

/// A profile whose role is `name`.
fn role(name: &str) -> AgentProfile {
  AgentProfile::new(
    Name::new(name).expect("a name"),
    String::new(),
    PromptText::new("").expect("a prompt"),
    ConnectionId::generate(),
    ModelId::new("test-model").expect("a model"),
    ProviderSettings::default(),
  )
}

#[test]
fn pipelines_one_save_a_team_profiles_conflict_still_opens_its_draft() {
  // No server answers: the versions made here wait in the outbox, as this conflict needs.
  let editor = Editor::team_without_server();
  editor.profile();
  let mut db = editor.db();
  let profile = db
    .resources(ResourceKind::AgentProfile)
    .expect("read")
    .into_iter()
    .find(|resource| resource.slug().as_str() == "reviewer")
    .expect("the profile")
    .id();
  first_numbered::<AgentProfile>(&mut db, profile);
  db.save_draft(
    profile,
    &Draft::new(role("Mine"), Some(VersionNumber::FIRST), Timestamp::now()),
  )
  .expect("a draft");
  let mine = db
    .publish_draft::<AgentProfile>(profile, Timestamp::now())
    .expect("published here");
  let theirs = role("Theirs");
  meets_a_teammate(&mut db, profile, mine.id(), &theirs);
  let mut editor = editor;
  editor.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Agent(profile, AgentTab::Profile),
  ));
  editor.until("the conflict", |e| e.exists("agent-conflict"));

  assert!(editor.exists("agent-open-draft"), "a profile keeps Open my draft on vN");
  assert!(!editor.exists("agent-take-back"));
}

/// Pipelines of the project, as stored.
fn pipelines(editor: &Editor) -> Vec<orchester_core::resource::Resource> {
  editor.db().resources(ResourceKind::Pipeline).expect("read")
}

/// A local project's New pipeline publishes v1 when every step is ready: a copy of a published pipeline.
#[test]
fn pipelines_one_save_create_v1_publishes_a_ready_pipeline_in_a_local_project() {
  let mut editor = Editor::new();
  let source = editor.published();
  editor.go(ProjectPage::NewPipeline);
  editor.until("New pipeline", |e| e.exists("pipeline-name"));
  assert!(editor.text("pipeline-create").contains("Create v1"));
  assert!(!editor.exists("pipeline-start-2"), "no empty pipeline is offered");

  editor.set("pipeline-name", "Delivery copy");
  editor.click("pipeline-start-1");
  editor.choose("pipeline-copy", "Delivery · v1");
  editor.click("pipeline-create");
  editor.until("the new pipeline's page", |e| e.exists("pipeline-add-step"));

  let id = editor.selected();
  assert_ne!(id, source);
  let (number, body) = latest(&editor, id);
  assert_eq!(number, VersionNumber::FIRST, "created as v1");
  assert_eq!(body.steps().len(), latest(&editor, source).1.steps().len());
  assert!(editor.db().draft::<Pipeline>(id).expect("read").is_none());
}

/// A local project's New pipeline whose steps are not all ready is refused with why, and nothing is stored: the
/// starter's first step has no profile in a project without one named for it.
#[test]
fn pipelines_one_save_create_v1_is_refused_with_why_while_a_step_is_not_ready() {
  let mut editor = Editor::new();
  editor.go(ProjectPage::NewPipeline);
  editor.until("New pipeline", |e| e.exists("pipeline-name"));
  editor.set("pipeline-name", "Feature delivery");
  editor.click("pipeline-create");
  editor.until("the refusal", |e| e.exists("pipeline-create-error"));

  assert!(
    editor.text("pipeline-create-error").ends_with("has no agent profile"),
    "{}",
    editor.text("pipeline-create-error")
  );
  assert!(pipelines(&editor).is_empty(), "nothing is stored");
  assert!(editor.exists("pipeline-name"), "the page stays");
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
