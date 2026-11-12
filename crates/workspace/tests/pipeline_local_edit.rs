//! What *Save* does with the edit the Pipelines page of a local project holds (`Pipelines · v2` P1): it publishes the
//! edit as the next version, refuses an edit that cannot be published and stores nothing (a pipeline holds no draft,
//! D-032), says why, asks for what a person decides, and leaves what runs started on alone; *Cancel* drops the edit
//! and stores nothing.
//! `cargo test -p orchester-workspace --test pipeline_local_edit _`.
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

/// A project with a published tool and a pipeline of one step that runs it, created as its first version.
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
  let (tool, _) = db
    .create_published(
      Slug::new("tool").expect("the fixture is valid"),
      Name::new("Tool").expect("the fixture is valid"),
      &tool,
      now,
    )
    .expect("the fixture is valid");
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("the fixture is valid"));
  let step = edit.add_step(Name::new("Check").expect("the fixture is valid"), None);
  edit.update(&step, |step| step.kind = StepKind::Tool { tool: Some(tool.id()) });
  let (pipeline, _) = db
    .published_pipeline_for_test(
      "pipeline",
      "Pipeline",
      &edit.validate().expect("the fixture is valid"),
      now,
    )
    .expect("the fixture is valid");
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

  /// Nothing was stored since the fixture: v1 only, and no draft.
  fn stores_only_the_first_version(&self) {
    assert_eq!(self.versions().len(), 1, "nothing was published");
    assert!(
      self.db.draft::<Pipeline>(self.pipeline).expect("read").is_none(),
      "a pipeline holds no draft"
    );
  }
}

#[test]
fn save_makes_the_next_version_and_leaves_the_one_runs_started_on() {
  let pipeline = fixture();
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
  assert_eq!(pipeline.opened().version, number);
}

#[test]
fn save_refuses_an_edit_that_cannot_be_published_says_why_and_stores_nothing() {
  let pipeline = fixture();
  let stored = pipeline.opened();

  let why = pipeline
    .save(
      |edit| {
        edit.add_step(Name::new("Review").unwrap(), Some(&pipeline.step));
      },
      &[],
    )
    .unwrap_err();
  assert_eq!(why.key, "pipelines.problems.noProfile");
  pipeline.stores_only_the_first_version();
  assert_eq!(pipeline.opened(), stored);
}

#[test]
fn save_that_changes_nothing_writes_nothing() {
  let pipeline = fixture();
  assert_eq!(pipeline.save(|_| {}, &[]).unwrap(), Saved::Unchanged);
  pipeline.stores_only_the_first_version();
}

#[test]
fn save_of_an_edit_started_before_the_pipeline_changed_is_refused() {
  let pipeline = fixture();
  let mut held = Held::start(Arc::new(pipeline.opened().body));
  held.change(|edit| edit.set_description("Mine"));
  pipeline
    .save(|edit| edit.set_description("Another editor's"), &[])
    .unwrap();

  let refused = save(pipeline.folder.path(), pipeline.pipeline, held.base(), held.edit(), &[]).unwrap_err();
  assert_eq!(refused.key, "errors.draftChanged");
  assert_eq!(pipeline.versions().len(), 2);
}

/// A pipeline with two published versions: the first, and a second that has a description.
fn two_versions(pipeline: &Fixture) -> (Pipeline, Pipeline) {
  pipeline
    .save(|edit| edit.set_description("Second version"), &[])
    .expect("the second version is published");
  let versions = pipeline.versions();
  assert_eq!(versions.len(), 2);
  (versions[0].clone(), versions[1].clone())
}

#[test]
fn save_that_cannot_publish_names_every_step_not_ready() {
  let pipeline = fixture();

  let why = pipeline
    .save(
      |edit| {
        edit.add_step(Name::new("Review").unwrap(), Some(&pipeline.step));
        edit.add_step(Name::new("Merge").unwrap(), Some(&pipeline.step));
      },
      &[],
    )
    .unwrap_err();
  // In the order of the flow: each step added after Check comes right after it.
  assert_eq!(why.key, "pipelines.problems.stepsNotReady");
  assert_eq!(
    why.args,
    [("count", "2".to_owned()), ("steps", "Merge, Review".to_owned())]
  );
  pipeline.stores_only_the_first_version();
}

#[test]
fn save_of_a_step_whose_profile_is_gone_is_refused_and_names_the_step_not_an_id() {
  let pipeline = fixture();
  let gone = ResourceId::generate();

  let runs_gone = |edit: &mut PipelineEdit| {
    edit.update(&pipeline.step, |step| {
      step.kind = StepKind::Agent {
        profile: Some(gone),
        repository: orchester_core::resource::RepositoryAccess::ReadOnly,
        model: None,
        permissions: orchester_core::permission::StepPermissions::default(),
      };
    });
  };
  let why = pipeline.save(runs_gone, &[]).unwrap_err();
  assert_eq!(
    why.key, "pipelines.problems.profileGone",
    "a profile this project does not have is gone, as Ready? says"
  );
  assert_eq!(why.args, [("step", "Check".to_owned())]);
  assert!(
    why.args.iter().all(|(_, value)| !value.contains(&gone.to_string())),
    "no raw id: {why:?}"
  );
  pipeline.stores_only_the_first_version();
}

#[test]
fn save_that_breaks_a_rule_stores_nothing() {
  let pipeline = fixture();
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
fn cancel_drops_the_edit_and_the_stored_pipeline_is_what_the_page_shows_again() {
  let pipeline = fixture();
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
fn unpublishable_says_what_keeps_an_edit_from_being_published() {
  use orchester_workspace::pipelines::{
    flow::FlowRead,
    local_edit::{Unpublishable, unpublishable},
  };

  let pipeline = fixture();
  let opened = pipeline.opened();
  let of =
    |edit: &PipelineEdit| unpublishable(edit, &FlowRead::of_edit(edit, &opened.assignables), &opened.assignables);
  let ready = PipelineEdit::new(opened.body.clone());
  assert_eq!(of(&ready), None, "its one step runs a published tool");

  let mut missing = ready.clone();
  missing.add_step(Name::new("Review").unwrap(), Some(&pipeline.step));
  assert_eq!(of(&missing), Some(Unpublishable::Steps));

  let mut empty = ready;
  empty.remove_step(&pipeline.step);
  assert_eq!(of(&empty), Some(Unpublishable::NoSteps));
}

/// A step that waits for only part of what starts the run with it is drawn as it is (P16): saving takes it, so the page
/// offers *Save* for it.
#[test]
fn unpublishable_and_save_take_steps_that_wait_for_only_part_of_a_parallel_group() {
  use orchester_workspace::pipelines::{
    flow::FlowRead,
    local_edit::{Unpublishable, unpublishable},
  };

  let pipeline = fixture();
  let opened = pipeline.opened();
  let mut branching = PipelineEdit::new(opened.body.clone());
  let kind = branching.step(&pipeline.step).expect("its one step").kind.clone();
  let lint = branching.add_step(Name::new("Lint").expect("a name"), Some(&pipeline.step));
  let notes = branching.add_step(Name::new("Notes").expect("a name"), Some(&pipeline.step));
  for key in [&lint, &notes] {
    branching.update(key, |step| step.kind = kind.clone());
  }
  // Notes starts the run beside the first step, while Lint waits for the first step only.
  branching.update(&notes, |step| step.depends_on.clear());
  assert!(branching.problems().is_empty(), "the steps break no rule of their own");

  let read = FlowRead::of_edit(&branching, &opened.assignables);
  assert!(read.flow.is_ok(), "the steps are drawn");
  let none: Option<Unpublishable> = None;
  assert_eq!(
    unpublishable(&branching, &read, &opened.assignables),
    none,
    "Save is offered"
  );
  let saved = save(pipeline.folder.path(), pipeline.pipeline, &opened.body, &branching, &[]).expect("saving takes it");
  assert_eq!(saved, Saved::Published(VersionNumber::new(2.try_into().unwrap())));
  let versions = pipeline.versions();
  assert_eq!(versions.len(), 2, "the steps are stored as the next version");
  assert_eq!(versions[1].steps(), branching.steps(), "as they were edited");
}

#[test]
fn publishing_tells_a_profile_not_published_yet_from_one_that_is_gone() {
  use orchester_workspace::pipelines::{
    data::{Assignables, DraftProfileChoice},
    write::refused_step,
  };

  let drafted = ResourceId::generate();
  let assignables = Assignables {
    draft_profiles: vec![DraftProfileChoice {
      id: drafted,
      name: "Planner".into(),
      model: orchester_core::resource::ModelId::new("test-model").unwrap(),
    }],
    ..Assignables::default()
  };
  let runs = |profile: ResourceId| {
    let mut edit = PipelineEdit::new(Pipeline::new(vec![]).unwrap());
    let key = edit.add_step(Name::new("Plan").unwrap(), None);
    edit.update(&key, |step| {
      step.kind = StepKind::Agent {
        profile: Some(profile),
        repository: orchester_core::resource::RepositoryAccess::ReadOnly,
        model: None,
        permissions: orchester_core::permission::StepPermissions::default(),
      };
    });
    edit.validate().unwrap()
  };

  let waiting = refused_step(runs(drafted).steps(), &assignables).expect("refused");
  assert_eq!(
    waiting.key, "runs.preflight.profileNotPublished",
    "a draft waits to be published"
  );
  let gone = refused_step(runs(ResourceId::generate()).steps(), &assignables).expect("refused");
  assert_eq!(gone.key, "pipelines.problems.profileGone");
  assert_eq!(
    gone.args,
    [("step", "Plan".to_owned())],
    "by the step's name, never an id"
  );
}
