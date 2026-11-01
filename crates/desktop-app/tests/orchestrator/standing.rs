//! Waits of the project's agents that truly wait: a task's run whose one step waits for a message, kept in the
//! conversations by the workspace's follower as the app keeps them, and answered on its task's page, after which the
//! conversation keeps it but its agent no longer waits.

use std::collections::BTreeMap;

use orchester_core::{
  id::{IdempotencyKey, ResourceId, StepExecutionId},
  orchestrator::EntryKind,
  resource::{AgentProfile, Pipeline, StepKind},
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot, WaitReason},
  store::{ResourceStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::orchestrator::waits::{MergeChecks, Switches, follow_in};

use super::harness::Orchestrator;

/// A run of a new task titled `title` on `pipeline`, its one step waiting for a message; the run and that step.
pub fn waiting(screen: &Orchestrator, pipeline: ResourceId, title: &str) -> (Run, StepExecutionId) {
  let mut db = screen.db();
  let version = db
    .latest_version::<Pipeline>(pipeline)
    .expect("read the pipeline")
    .expect("a published pipeline");
  let StepKind::Agent {
    profile: Some(builder), ..
  } = version.body().steps()[0].kind
  else {
    panic!("the pipeline's step has a profile");
  };
  let profile = db
    .latest_version::<AgentProfile>(builder)
    .expect("read the profile")
    .expect("a published profile");
  let profiles = BTreeMap::from([(builder, profile)]);
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &BTreeMap::new(),
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  let snapshot = RunSnapshot::build(pipeline, &version, published).expect("snapshot");
  let key = screen.draft_on(pipeline, title);
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  let new = NewRun {
    task: task.id(),
    snapshot,
    branch: BranchName::new(&format!("orchester/{}", key.to_lowercase())).expect("branch"),
    base: Revision::new(&"a".repeat(40)).expect("revision"),
    base_branch: None,
    base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
    budget: None,
    max_duration: None,
  };
  let now = Timestamp::now();
  let mut run = db.create_run(IdempotencyKey::generate(), new, now).expect("run");
  let events = run.start(now).expect("started");
  db.append_events(&run, &events).expect("kept");
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.start_step(execution, now).expect("step started");
  db.append_events(&run, &events).expect("kept");
  let events = run.wait_step(execution, WaitReason::Message, now).expect("waits");
  db.append_events(&run, &events).expect("kept");
  (run, execution)
}

/// The person answers `run`'s step on its task's page: it goes on, and no longer waits.
pub fn answered_on_task_page(screen: &Orchestrator, (run, execution): &mut (Run, StepExecutionId)) {
  let events = run.resume_step(*execution, Timestamp::now()).expect("resumed");
  screen.db().append_events(run, &events).expect("kept");
}

/// Keeps the waits that are new in the conversations, as the app's follower does, with *Let the Orchestrator answer
/// agents* off.
pub fn follow(screen: &Orchestrator) {
  let programs = Programs::processes();
  follow_in(
    (&screen.root, screen.project, &programs),
    &MergeChecks::default(),
    (Switches::default(), true),
  )
  .expect("followed");
}

/// The entry `run`'s wait was kept by in the project's latest conversation.
pub fn entry_of(screen: &Orchestrator, run: &Run) -> i64 {
  let db = screen.db();
  let chat = db.latest_chat().expect("read").expect("a conversation");
  db.chat_entries(chat)
    .expect("entries")
    .into_iter()
    .find_map(|entry| match entry.kind {
      EntryKind::Wait { wait, .. } if wait.run == run.id() => Some(entry.seq),
      _ => None,
    })
    .expect("the wait's entry")
}
