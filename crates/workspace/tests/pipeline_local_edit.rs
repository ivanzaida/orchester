//! What *Save* does with the edit the Pipelines page of a local project holds (`Pipelines · v2` P1): it stores the edit
//! as the draft and publishes it as the next version at once, keeps the draft and says why when it cannot publish,
//! asks for what a person decides, and leaves what runs started on alone; *Cancel* drops the edit and stores nothing.
use std::sync::Arc;

use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    CommandAction, CommandProgram, Pipeline, PipelineEdit, Slug, StepKey, StepKind, Tool, ToolAction, VersionNumber,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  messages::Message,
  pipelines::{
    data::OpenedPipeline,
    local_edit::{Held, Saved, save},
  },
};

struct Fixture {
  folder: tempfile::TempDir,
  db: ProjectDb,
  pipeline: ResourceId,
  step: StepKey,
}

/// A project with a published tool and a pipeline of one step that runs it, stored as its draft only.
fn fixture() -> Fixture {
  let folder = tempfile::tempdir().expect("the fixture is valid");
  std::fs::create_dir(folder.path().join(".git")).expect("the fixture is valid");
  let mut db = ProjectDb::create(
    folder.path(),
    Project::new(
      ProjectId::generate(),
      ProjectKey::new("TEST").expect("the fixture is valid"),
      Name::new("Test").expect("the fixture is valid"),
    ),
  )
  .expect("the fixture is valid");
  let now = Timestamp::now();
  let tool = Tool::new(
    "",
    ToolAction::Command(CommandAction::new(
      CommandProgram::new("cargo").expect("the fixture is valid"),
      vec![],
    )),
    None,
  )
  .expect("the fixture is valid");
  let tool_id = db
    .create_resource(
      Slug::new("tool").expect("the fixture is valid"),
      Name::new("Tool").expect("the fixture is valid"),
      &tool,
      now,
    )
    .expect("the fixture is valid")
    .id();
  db.publish_draft::<Tool>(tool_id, now).expect("the fixture is valid");
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("the fixture is valid"));
  let step = edit.add_step(Name::new("Check").expect("the fixture is valid"), None);
  edit.update(&step, |step| step.kind = StepKind::Tool { tool: Some(tool_id) });
  let pipeline = db
    .create_resource(
      Slug::new("pipeline").expect("the fixture is valid"),
      Name::new("Pipeline").expect("the fixture is valid"),
      &edit.validate().expect("the fixture is valid"),
      now,
    )
    .expect("the fixture is valid")
    .id();
  Fixture {
    folder,
    db,
    pipeline,
    step,
  }
}

impl Fixture {
  fn opened(&self) -> OpenedPipeline {
    OpenedPipeline::load(&self.db, self.pipeline).expect("the fixture is valid")
  }

  /// Saves what a page that opened the pipeline and made `change` holds.
  fn save(&self, change: impl FnOnce(&mut PipelineEdit), allowed: &[Message]) -> Result<Saved, Message> {
    let mut held = Held::start(Arc::new(self.opened().body));
    held.change(change);
    save(self.folder.path(), self.pipeline, held.base(), held.edit(), allowed)
  }

  fn versions(&self) -> Vec<Pipeline> {
    let mut versions = self
      .db
      .versions::<Pipeline>(self.pipeline)
      .expect("the fixture is valid");
    versions.sort_by_key(|version| version.number());
    versions.iter().map(|version| version.body().clone()).collect()
  }
}

fn first_version() -> Message {
  Message::new("appMcp.widens.firstPipelineVersion")
}

#[test]
fn save_publishes_the_first_version_once_a_person_allows_it() {
  let pipeline = fixture();
  let describe = |edit: &mut PipelineEdit| edit.set_description("Check the change");

  let asked = pipeline.save(describe, &[]).unwrap();
  assert_eq!(asked, Saved::Confirm(vec![first_version()]));
  assert!(pipeline.versions().is_empty());
  assert_eq!(pipeline.opened().body.description(), "");

  let saved = pipeline.save(describe, &[first_version()]).unwrap();
  assert_eq!(saved, Saved::Published(VersionNumber::FIRST));
  let opened = pipeline.opened();
  assert_eq!(opened.latest.unwrap().body.description(), "Check the change");
  assert!(opened.draft_saved_at.is_none());
}

#[test]
fn save_makes_the_next_version_and_leaves_the_one_runs_started_on() {
  let pipeline = fixture();
  pipeline.save(|_| {}, &[first_version()]).unwrap();
  let before = pipeline.versions();
  assert_eq!(before.len(), 1);

  let saved = pipeline
    .save(|edit| edit.set_description("Now with a description"), &[])
    .unwrap();
  let Saved::Published(number) = saved else {
    panic!("expected a published version, got {saved:?}");
  };
  assert_eq!(number.get(), 2);
  let after = pipeline.versions();
  assert_eq!(after.len(), 2);
  assert_eq!(after[0], before[0], "the version a run started on is untouched");
  assert_eq!(after[1].description(), "Now with a description");
  assert!(pipeline.opened().draft_saved_at.is_none());
}

#[test]
fn save_keeps_the_edit_as_the_draft_and_says_why_when_it_cannot_publish() {
  let pipeline = fixture();
  pipeline.save(|_| {}, &[first_version()]).unwrap();

  let saved = pipeline
    .save(
      |edit| {
        edit.add_step(Name::new("Review").unwrap(), Some(&pipeline.step));
      },
      &[],
    )
    .unwrap();
  let Saved::Drafted(why) = saved else {
    panic!("expected a draft kept, got {saved:?}");
  };
  assert_eq!(why.key, "pipelines.problems.noProfile");
  assert_eq!(pipeline.versions().len(), 1, "nothing was published");
  let opened = pipeline.opened();
  assert!(opened.draft_saved_at.is_some());
  assert_eq!(opened.body.steps().len(), 2, "the stored draft is the edit");
  assert_eq!(opened.latest.unwrap().body.steps().len(), 1);
}

#[test]
fn save_that_breaks_a_rule_stores_nothing() {
  let pipeline = fixture();
  pipeline.save(|_| {}, &[first_version()]).unwrap();
  let stored = pipeline.opened();

  let refused = pipeline
    .save(
      |edit| {
        let step = edit.steps()[0].key.clone();
        edit.set_dependency(&step, &step, true);
      },
      &[],
    )
    .unwrap_err();
  assert_eq!(refused.key, "pipelines.problems.cycleAt");
  assert_eq!(pipeline.opened(), stored);
}

#[test]
fn save_that_changes_nothing_writes_nothing() {
  let pipeline = fixture();
  pipeline.save(|_| {}, &[first_version()]).unwrap();
  assert_eq!(pipeline.save(|_| {}, &[]).unwrap(), Saved::Unchanged);
  assert_eq!(pipeline.versions().len(), 1);
}

#[test]
fn save_of_an_edit_started_before_the_pipeline_changed_is_refused() {
  let pipeline = fixture();
  let mut held = Held::start(Arc::new(pipeline.opened().body));
  held.change(|edit| edit.set_description("Mine"));
  pipeline
    .save(|edit| edit.set_description("Another editor's"), &[first_version()])
    .unwrap();

  let allowed = [first_version()];
  let refused = save(
    pipeline.folder.path(),
    pipeline.pipeline,
    held.base(),
    held.edit(),
    &allowed,
  )
  .unwrap_err();
  assert_eq!(refused.key, "errors.draftChanged");
  assert_eq!(pipeline.versions().len(), 1);
}

#[test]
fn a_stored_draft_is_where_an_edit_starts_and_save_publishes_it() {
  let pipeline = fixture();
  assert!(pipeline.opened().draft_saved_at.is_some());
  let held = Held::start(Arc::new(pipeline.opened().body));
  assert!(!held.is_changed(), "opening a stored draft is not a change");

  let allowed = [first_version()];
  let saved = save(
    pipeline.folder.path(),
    pipeline.pipeline,
    held.base(),
    held.edit(),
    &allowed,
  )
  .unwrap();
  assert_eq!(saved, Saved::Published(VersionNumber::FIRST));
}

#[test]
fn cancel_drops_the_edit_and_the_stored_pipeline_is_what_the_page_shows_again() {
  let pipeline = fixture();
  pipeline.save(|_| {}, &[first_version()]).unwrap();
  let stored = pipeline.opened();

  let mut held = Held::start(Arc::new(stored.body.clone()));
  assert!(!held.is_changed());
  held.change(|edit| edit.set_description("Unsaved"));
  assert!(held.is_changed());
  held.change(|edit| edit.set_description(""));
  assert!(!held.is_changed(), "changes that cancel each other are none");
  held.change(|edit| edit.set_description("Unsaved again"));
  drop(held);

  assert_eq!(pipeline.opened(), stored, "an edit that was dropped stored nothing");
  let again = Held::start(Arc::new(pipeline.opened().body));
  assert_eq!(again.edit().description(), "");
}

#[test]
fn an_edit_back_to_the_latest_version_drops_the_stored_draft_and_does_not_wedge_save() {
  let pipeline = fixture();
  pipeline.save(|_| {}, &[first_version()]).unwrap();
  let latest = pipeline.opened().latest.unwrap().body;
  pipeline
    .save(
      |edit| {
        edit.add_step(Name::new("Review").unwrap(), Some(&pipeline.step));
      },
      &[],
    )
    .unwrap();
  let drafted = pipeline.opened();
  assert!(drafted.draft_saved_at.is_some());
  assert_ne!(drafted.body, latest);

  // The page opens on the stored draft; the person changes it back to the latest version and presses Save.
  let mut held = Held::start(Arc::new(drafted.body));
  held.change(|edit| *edit = PipelineEdit::new(latest.clone()));
  let saved = save(pipeline.folder.path(), pipeline.pipeline, held.base(), held.edit(), &[]).unwrap();
  assert_eq!(saved, Saved::Published(VersionNumber::FIRST));

  let opened = pipeline.opened();
  assert!(
    opened.draft_saved_at.is_none(),
    "the draft the person chose against is gone"
  );
  assert_eq!(opened.body, latest);
  assert_eq!(pipeline.versions().len(), 1, "no version was made");

  // What the page does next: it shows the latest version, and a Save on it neither fails nor writes.
  let again = Held::start(Arc::new(opened.body));
  assert_eq!(
    save(
      pipeline.folder.path(),
      pipeline.pipeline,
      again.base(),
      again.edit(),
      &[]
    )
    .unwrap(),
    Saved::Unchanged
  );
  assert_eq!(pipeline.versions().len(), 1);
}

/// A pipeline with two published versions: the first, and a second that has a description.
fn two_versions(pipeline: &Fixture) -> (Pipeline, Pipeline) {
  pipeline
    .save(|_| {}, &[first_version()])
    .expect("the first version is published");
  pipeline
    .save(|edit| edit.set_description("Second version"), &[])
    .expect("the second version is published");
  let versions = pipeline.versions();
  assert_eq!(versions.len(), 2);
  (versions[0].clone(), versions[1].clone())
}

#[test]
fn restore_puts_an_older_version_into_the_edit_and_save_makes_it_the_next_version() {
  let pipeline = fixture();
  let (first, second) = two_versions(&pipeline);
  assert_ne!(first, second);

  let mut held = Held::start(Arc::new(pipeline.opened().body));
  assert!(!held.is_changed());
  held.restore(&first);
  assert!(held.is_changed(), "the first version differs from the latest");
  assert_eq!(held.edit().validate().unwrap(), first);

  let saved = save(pipeline.folder.path(), pipeline.pipeline, held.base(), held.edit(), &[]).unwrap();
  assert_eq!(saved, Saved::Published(VersionNumber::new(3.try_into().unwrap())));
  let versions = pipeline.versions();
  assert_eq!(
    versions.len(),
    3,
    "the restored version is a new one; no version was changed"
  );
  assert_eq!(versions[2], first);
  assert_eq!(versions[1], second);
}

#[test]
fn restore_replaces_the_changes_the_edit_held() {
  let pipeline = fixture();
  let (first, _) = two_versions(&pipeline);

  let mut held = Held::start(Arc::new(pipeline.opened().body));
  held.change(|edit| edit.set_description("Unsaved words"));
  held.restore(&first);
  assert_eq!(held.edit().description(), first.description());
  assert_eq!(held.edit().validate().unwrap(), first);
}

#[test]
fn restoring_the_current_version_is_no_change_and_save_writes_nothing() {
  let pipeline = fixture();
  let (_, second) = two_versions(&pipeline);

  let mut held = Held::start(Arc::new(pipeline.opened().body));
  held.change(|edit| edit.set_description("Unsaved words"));
  assert!(held.is_changed());
  held.restore(&second);
  assert!(!held.is_changed(), "the current version is what the edit started from");

  let saved = save(pipeline.folder.path(), pipeline.pipeline, held.base(), held.edit(), &[]).unwrap();
  assert_eq!(saved, Saved::Unchanged);
  assert_eq!(pipeline.versions().len(), 2);
}

#[test]
fn save_that_cannot_publish_names_every_step_not_ready() {
  let pipeline = fixture();
  pipeline.save(|_| {}, &[first_version()]).unwrap();

  let saved = pipeline
    .save(
      |edit| {
        edit.add_step(Name::new("Review").unwrap(), Some(&pipeline.step));
        edit.add_step(Name::new("Merge").unwrap(), Some(&pipeline.step));
      },
      &[],
    )
    .unwrap();
  let Saved::Drafted(why) = saved else {
    panic!("expected a draft kept, got {saved:?}");
  };
  // In the order of the flow: each step added after Check comes right after it.
  assert_eq!(why.key, "pipelines.problems.stepsNotReady");
  assert_eq!(
    why.args,
    [("count", "2".to_owned()), ("steps", "Merge, Review".to_owned())]
  );
}

#[test]
fn unpublishable_says_what_keeps_an_edit_from_being_published() {
  use orchester_workspace::pipelines::{
    flow::FlowRead,
    local_edit::{Unpublishable, unpublishable},
  };

  let pipeline = fixture();
  let opened = pipeline.opened();
  let of = |edit: &PipelineEdit| unpublishable(edit, &FlowRead::of_edit(edit, &opened.assignables));
  let ready = PipelineEdit::new(opened.body.clone());
  assert_eq!(of(&ready), None, "its one step runs a published tool");

  let mut missing = ready.clone();
  missing.add_step(Name::new("Review").unwrap(), Some(&pipeline.step));
  assert_eq!(of(&missing), Some(Unpublishable::Steps(1)));

  let mut empty = ready;
  empty.remove_step(&pipeline.step);
  assert_eq!(of(&empty), Some(Unpublishable::NoSteps));
}
