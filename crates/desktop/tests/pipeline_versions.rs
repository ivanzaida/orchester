//! A pipeline's version history and restoring a published version as the draft: the versions
//! newest first with what changed from the one before and the runs that pinned them, and a
//! restored version stored as a draft on top of the latest version, or no draft at all when it
//! repeats the latest.

extern crate lurq_legacy as lurq;

// The project fixture of the workspace's tests.
#[path = "../../workspace/tests/support/mod.rs"]
mod support;

use std::sync::Arc;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Draft, DraftSaved, Pipeline, PipelineChange, ResourceKind, StepChange, VersionNumber},
  store::ResourceStore,
};
use orchester_desktop::{
  drafts::save_draft_in,
  pipelines::{data::OpenedPipeline, editor::EditorState, history::VersionHistory},
};

use self::support::{Fixture, at};

/// The fixture's pipeline, with v2 renaming its step and, when `draft`, a draft on top of v2
/// that waits for approval.
fn two_versions(fixture: &mut Fixture, draft: bool) -> ResourceId {
  let delivery = fixture.db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let v1 = fixture
    .db
    .latest_version::<Pipeline>(delivery)
    .expect("read")
    .expect("v1 is published");
  let mut steps = v1.body().steps().to_vec();
  steps[0].name = Name::new("Build").expect("valid name");
  let renamed = Pipeline::new(steps.clone()).expect("valid pipeline");
  fixture
    .db
    .save_draft(delivery, &Draft::new(renamed, Some(v1.number()), at(20)))
    .expect("save v2");
  let v2 = fixture
    .db
    .publish_draft::<Pipeline>(delivery, at(21))
    .expect("publish v2");
  if draft {
    steps[0].approval_gate = true;
    let gated = Pipeline::new(steps).expect("valid pipeline");
    fixture
      .db
      .save_draft(delivery, &Draft::new(gated, Some(v2.number()), at(22)))
      .expect("save draft v3");
  }
  delivery
}

fn version(number: u32) -> VersionNumber {
  VersionNumber::new(std::num::NonZeroU32::new(number).expect("versions start at 1"))
}

#[test]
fn versions_are_listed_newest_first_with_their_changes_and_runs() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add switcher", at(10));
  let mut first = fixture.started_run(task, at(11));
  fixture.succeed(&mut first, at(12));
  let delivery = two_versions(&mut fixture, true);

  let history = VersionHistory::load_in(fixture.repo.path(), delivery).expect("history");
  let numbers: Vec<u32> = history.versions().iter().map(|version| version.number.get()).collect();
  assert_eq!(numbers, [2, 1]);
  let latest = history.latest().expect("a latest version");
  assert_eq!(latest.number, version(2));
  assert_eq!(latest.published_at, at(21));
  assert!(latest.runs.is_empty());
  let from = latest.changes.as_ref().expect("v2 follows v1");
  assert_eq!(from.version, version(1));
  assert!(matches!(
    from.changes.as_slice(),
    [PipelineChange::Step { change: StepChange::Renamed { from }, .. }] if from.as_str() == "Implement"
  ));
  let v1 = history.version(version(1)).expect("v1");
  assert_eq!(v1.changes, None);
  assert_eq!(v1.runs, [first.number()]);
  assert_eq!(v1.body.steps()[0].name.as_str(), "Implement");
  // A draft is not a version.
  assert_eq!(history.version(version(3)), None);
}

#[test]
fn restoring_an_old_version_stores_it_as_a_draft_on_top_of_the_latest() {
  let mut fixture = Fixture::new();
  let delivery = two_versions(&mut fixture, true);
  let repo = fixture.repo.path();
  let history = VersionHistory::load_in(repo, delivery).expect("history");
  let v1 = history.version(version(1)).expect("v1").body.clone();

  let mut state = EditorState::new(Arc::new(OpenedPipeline::load_in(repo, delivery).expect("open")));
  assert_eq!(state.draft_number(), 3);
  state.restore(&v1);
  let (revision, draft) = state.draft_to_save(at(30)).expect("the restored steps are saved");
  assert_eq!(draft.body(), &*v1);
  assert_eq!(draft.based_on(), Some(version(2)));
  let outcome = save_draft_in(repo, delivery, &draft).expect("save");
  assert_eq!(outcome, DraftSaved::Stored);
  state.saved(revision, at(30), outcome);
  assert!(state.has_changes());
  assert_eq!(state.draft_number(), 3);
  assert!(state.can_publish());
  // Published versions are untouched.
  let after = VersionHistory::load_in(repo, delivery).expect("history");
  assert_eq!(after, history);
  let stored = fixture.db.draft::<Pipeline>(delivery).expect("read").expect("a draft");
  assert_eq!(stored.body(), &*v1);
}

#[test]
fn restoring_the_latest_version_over_a_draft_leaves_no_draft() {
  let mut fixture = Fixture::new();
  let delivery = two_versions(&mut fixture, true);
  let repo = fixture.repo.path();
  let history = VersionHistory::load_in(repo, delivery).expect("history");
  let latest = history.latest().expect("v2").body.clone();

  let mut state = EditorState::new(Arc::new(OpenedPipeline::load_in(repo, delivery).expect("open")));
  assert!(state.has_changes());
  state.restore(&latest);
  let (revision, draft) = state.draft_to_save(at(30)).expect("the restored steps are saved");
  let outcome = save_draft_in(repo, delivery, &draft).expect("save");
  assert_eq!(outcome, DraftSaved::Discarded);
  state.saved(revision, at(30), outcome);
  assert!(!state.has_changes());
  assert_eq!(fixture.db.draft::<Pipeline>(delivery).expect("read"), None);
  // Restoring what the editor already holds is no new revision.
  state.restore(&latest);
  assert_eq!(state.draft_to_save(at(31)), None);
}
