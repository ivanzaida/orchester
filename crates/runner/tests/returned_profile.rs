//! Returning a run's work to a step with another agent profile, for that return only (ORC-808), through the scheduler
//! with the fake CLI: the returned step runs on the chosen profile's model and the history names it; the run's snapshot
//! keeps the pipeline's agent; a profile the run cannot read, or with another model than its version has, is refused
//! and the run stays as it was.
mod support;

use std::path::Path;

use orchester_core::{
  id::{IdempotencyKey, ResourceId, RunId, VersionId},
  name::Name,
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, ResourceKind, Slug, VersionNumber},
  run::{Answerer, AttemptReason, ChosenProfile, PinnedVersion, ReviewNote, Run, RunStatus, StepExecutor},
  store::{ResourceStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::{Rejection, WorkReturn};
use orchester_store::ProjectDb;
use support::{Setup, project, scheduler, threads, wait_for, wait_for_idle};

const OTHER_MODEL: &str = "gpt-5.4";

fn read(root: &Path, id: RunId) -> Run {
  ProjectDb::open(root)
    .expect("open")
    .run(id)
    .expect("read")
    .expect("run")
}

/// A second published profile, "Fixer", on the Builder's connection with another model, as the project's agents list
/// would offer it.
fn publish_fixer(root: &Path) -> ChosenProfile {
  let mut db = ProjectDb::open(root).expect("open");
  let builder = db
    .resources(ResourceKind::AgentProfile)
    .expect("profiles")
    .remove(0)
    .id();
  let body = db
    .latest_version::<AgentProfile>(builder)
    .expect("the builder")
    .expect("published")
    .body()
    .clone();
  let fixer = AgentProfile::new(
    Name::new("Fixer").expect("name"),
    String::new(),
    PromptText::new("Fix things.").expect("prompt"),
    body.connection(),
    ModelId::new(OTHER_MODEL).expect("model"),
    ProviderSettings::default(),
  )
  .with_permissions(body.permissions().clone());
  let now = Timestamp::now();
  let id = db
    .create_resource(
      Slug::new("fixer").expect("slug"),
      Name::new("Fixer").expect("name"),
      &fixer,
      now,
    )
    .expect("profile")
    .id();
  let version = db.publish_draft::<AgentProfile>(id, now).expect("publish");
  ChosenProfile {
    profile: PinnedVersion::of(id, &version),
    model: fixer.model().clone(),
  }
}

fn returned(root: &Path, run: RunId, profile: Option<ChosenProfile>) -> WorkReturn {
  WorkReturn {
    run,
    step: orchester_core::resource::StepKey::new("implement").expect("a step key"),
    note: ReviewNote::new("Rename the function.").expect("a note"),
    conflicts: None,
    profile,
    by: Answerer::Person,
    expected: read(root, run).last_sequence(),
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn returned_with_a_profile_runs_the_step_on_its_model_and_leaves_the_pipeline_unchanged() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let first = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  let snapshot = first.snapshot().clone();
  let chosen = publish_fixer(root);
  scheduler
    .return_ended(returned(root, id, Some(chosen.clone())))
    .await
    .expect("returned");
  let done = wait_for(root, &mut events, id, |run| {
    run.status() == &RunStatus::Succeeded && run.attempts().len() == 2
  })
  .await;
  wait_for_idle(&scheduler).await;
  assert!(matches!(
    done.current_attempt().expect("attempt").reason(),
    AttemptReason::ReturnedAfterEnd { profile: Some(kept), .. } if *kept == chosen
  ));
  assert_eq!(done.snapshot(), &snapshot, "the run keeps the pipeline's agents");
  let db = ProjectDb::open(root).expect("open");
  let threads = threads(&db, id);
  assert_eq!(threads.len(), 2, "the step started again");
  assert_ne!(threads[0]["model"], OTHER_MODEL, "the pipeline's agent ran first");
  assert_eq!(
    threads[1]["model"], OTHER_MODEL,
    "the return ran on the chosen profile's model"
  );
  let returned = done.current_attempt().expect("attempt").executions()[0].id();
  assert_eq!(
    done
      .executing_agent(returned)
      .map(|(profile, model)| (profile.resource, model.as_str())),
    Some((chosen.profile.resource, OTHER_MODEL))
  );
  let StepExecutor::Agent { profile, .. } = done.snapshot().steps()[0].executor() else {
    panic!("an agent step");
  };
  assert_ne!(profile.resource, chosen.profile.resource);
}

#[tokio::test(flavor = "multi_thread")]
async fn returned_refuses_a_profile_the_run_cannot_run_and_stays_as_it_was() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  let ended = read(root, id);
  let chosen = publish_fixer(root);
  let unknown = ChosenProfile {
    profile: PinnedVersion {
      resource: ResourceId::generate(),
      version: VersionId::generate(),
      number: VersionNumber::FIRST,
    },
    model: chosen.model.clone(),
  };
  let another_model = ChosenProfile {
    model: ModelId::new("some-other-model").expect("model"),
    ..chosen
  };
  for profile in [unknown, another_model] {
    assert_eq!(
      scheduler.return_ended(returned(root, id, Some(profile))).await,
      Err(Rejection::ProfileNotUsable)
    );
    assert_eq!(read(root, id), ended);
  }
}
