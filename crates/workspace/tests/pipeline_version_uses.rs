//! What the versions list of a pipeline says each version is held by (`Pipelines · v2` P12): the current version, the
//! tasks that use the pipeline; an older one, the runs that started on it, which a newer version never takes from them.
//! `cargo test -p orchester-workspace --test pipeline_version_uses`.

mod support;

use orchester_core::{
  resource::{Draft, Pipeline, VersionNumber},
  store::ResourceStore,
};
use orchester_workspace::pipelines::history::{Use, VersionHistory};

use self::support::{Fixture, at};

/// Publishes the next version of `pipeline`, which differs from the latest by its description.
fn publish_next(fixture: &mut Fixture, pipeline: orchester_core::id::ResourceId, description: &str, millis: i64) {
  let latest = fixture
    .db
    .latest_version::<Pipeline>(pipeline)
    .expect("read")
    .expect("a published pipeline has a latest version");
  let body = latest.body().clone().with_description(description.into());
  fixture
    .db
    .save_draft(pipeline, &Draft::new(body, Some(latest.number()), at(millis)))
    .expect("save draft");
  fixture
    .db
    .publish_draft::<Pipeline>(pipeline, at(millis + 1))
    .expect("publish");
}

#[test]
fn the_current_version_counts_tasks_and_an_older_one_the_runs_that_started_on_it() {
  let mut fixture = Fixture::new();
  let (first, second) = (fixture.task("First", at(10)), fixture.task("Second", at(11)));
  let pipeline = fixture.new_run_for(first).snapshot.pipeline().resource;
  fixture.pending_run(first, at(12));
  fixture.pending_run(second, at(13));
  publish_next(&mut fixture, pipeline, "Second version", 100);
  publish_next(&mut fixture, pipeline, "Third version", 200);

  let history = VersionHistory::load(&fixture.db, pipeline).expect("history");
  let number = |n: u32| VersionNumber::new(n.try_into().expect("above zero"));
  assert_eq!(
    history.use_of(number(3), 13),
    Some(Use::Tasks(13)),
    "the current version"
  );
  assert_eq!(history.use_of(number(3), 0), Some(Use::Tasks(0)), "no task uses it");
  assert_eq!(
    history.use_of(number(2), 13),
    Some(Use::Runs(0)),
    "no run started on it"
  );
  assert_eq!(
    history.use_of(number(1), 13),
    Some(Use::Runs(2)),
    "both runs started on the first"
  );
  assert_eq!(
    history.use_of(number(4), 13),
    None,
    "a version that was never published"
  );
}

#[test]
fn a_version_that_was_published_after_the_runs_started_leaves_them_on_theirs() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Task", at(10));
  let pipeline = fixture.new_run_for(task).snapshot.pipeline().resource;
  fixture.pending_run(task, at(12));

  let before = VersionHistory::load(&fixture.db, pipeline).expect("history");
  assert_eq!(before.use_of(VersionNumber::FIRST, 1), Some(Use::Tasks(1)));

  publish_next(&mut fixture, pipeline, "Next", 100);
  let after = VersionHistory::load(&fixture.db, pipeline).expect("history");
  assert_eq!(after.use_of(VersionNumber::FIRST, 1), Some(Use::Runs(1)));
}
