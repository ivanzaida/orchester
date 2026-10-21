//! The Orchestrator chat may continue runs the runner stopped because the project database stayed busy (ORC-75,
//! `docs/design/orchestrator-toolset.md`, decision 5), on the seeded Kontur's task states: `get_run` and `list_runs`
//! report why the runner failed; with *Let the Orchestrator continue interrupted runs* on, `resume_run` sends a run
//! of storage contention to the runner to continue from its last checkpoint, kept as the chat's own action, while a
//! run the database failed for real, or the runner for another reason, stays the person's; with the switch off, so
//! does contention. A turn the run's interruption started by itself continues it once, not when it was interrupted
//! again soon, and not while more than three runs of the project were stopped so within ten minutes.

use orchester_core::{
  orchestrator::{ActionOutcome, RunEnd},
  run::{InterruptCause, Run, RunnerFailure},
  store::RunStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{orchestrator::tools::ChatScope, workspace::OrchestratorConsent};
use serde_json::json;

use super::{
  Served,
  orchestrator_chat::{actions, chat, chat_call, task_run},
  orchestrator_interrupted::{allow, refusal, resume, turn_of_end},
};

const BUSY: &str = "Add retry to webhook sender";
const BROKEN: &str = "Sync contacts with HubSpot";
const OTHER: &str = "Add serde to the webhook payloads";
/// The seed's crashed run, continued and then stopped by the runner too.
const CRASHED: &str = "Import contacts from CSV";

/// Interrupts `run`, at work, now for the runner's failure of kind `failure`, as the runner records it.
fn stop(db: &mut ProjectDb, run: &mut Run, failure: RunnerFailure) {
  let events = run
    .interrupt(InterruptCause::RunnerFailed, Timestamp::now())
    .expect("interrupted");
  db.append_runner_failure(run, &events, failure)
    .expect("recorded with why");
}

/// The latest run of the seeded task titled `title`, at work, stopped now by the runner's failure of kind `failure`.
fn stopped(db: &mut ProjectDb, title: &str, failure: RunnerFailure) -> Run {
  let (_, mut run) = task_run(db, title);
  stop(db, &mut run, failure);
  run
}

#[test]
fn chat_continues_only_storage_contention_and_only_with_the_persons_switch() {
  let (mut served, seeded) = Served::seeded("task-states");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let scope = chat(&served, &mut db);
  let busy = stopped(&mut db, BUSY, RunnerFailure::StorageContention);
  let broken = stopped(&mut db, BROKEN, RunnerFailure::StorageFailure);
  let other = stopped(&mut db, OTHER, RunnerFailure::Other);
  let refused = chat_call(&mut served, &scope, "resume_run", resume(&busy)).expect_err("the switch is off");
  assert_eq!(refusal(&refused), ("person_only", "orchestrator.tools.resumePersonOnly"));
  allow(&served, OrchestratorConsent::ContinueInterrupted, true);
  for run in [&broken, &other] {
    let refused = chat_call(&mut served, &scope, "resume_run", resume(run)).expect_err("the person's");
    assert_eq!(
      refusal(&refused),
      ("person_only", "orchestrator.tools.continueRunnerFailed")
    );
  }
  // This session's runner cannot start: the call reached it, to continue the run from its last checkpoint.
  let asked = chat_call(&mut served, &scope, "resume_run", resume(&busy)).expect_err("no runner here");
  assert_eq!(refusal(&asked).0, "runner_unavailable", "{asked:?}");
  let kept: Vec<String> = actions(&db, &scope)
    .into_iter()
    .map(|action| match action.outcome {
      ActionOutcome::Refused { code, .. } => code,
      other => panic!("refused, not {other:?}"),
    })
    .collect();
  assert_eq!(
    kept,
    ["person_only", "person_only", "person_only", "runner_unavailable"],
    "each call is kept in the conversation"
  );
}

#[test]
fn chat_reads_why_the_runner_stopped_a_run() {
  let (mut served, seeded) = Served::seeded("task-states");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  for (title, failure, kind, words) in [
    (BUSY, RunnerFailure::StorageContention, "storage_contention", "database busy"),
    (BROKEN, RunnerFailure::StorageFailure, "storage_failure", "database unavailable"),
    (OTHER, RunnerFailure::Other, "other", "runner error"),
  ] {
    let run = stopped(&mut db, title, failure);
    let read = served.ok("get_run", &json!({ "project": "Kontur", "run": run.id().to_string() }));
    assert_eq!(read["status"]["reason"], "runner_failed", "{read}");
    assert_eq!(read["status"]["runner_failure"], kind, "{read}");
    let (task, _) = task_run(&db, title);
    let listed = served.ok(
      "list_runs",
      &json!({ "project": "Kontur", "task": task.key().to_string() }),
    );
    let latest = &listed["runs"][0];
    assert_eq!(latest["status"]["runner_failure"], kind, "{listed}");
    assert!(
      latest["detail"].as_str().is_some_and(|detail| detail.contains(words)),
      "{title}: {latest}"
    );
  }
  // A run the runner stopped before the kind was kept: unknown.
  let (_, mut crashed) = task_run(&db, CRASHED);
  let events = crashed.continue_run(Timestamp::now()).expect("continued");
  db.append_events(&crashed, &events).expect("recorded");
  let events = crashed
    .interrupt(InterruptCause::RunnerFailed, Timestamp::now())
    .expect("interrupted");
  db.append_events(&crashed, &events).expect("recorded without why");
  let read = served.ok("get_run", &json!({ "project": "Kontur", "run": crashed.id().to_string() }));
  assert!(read["status"]["runner_failure"].is_null(), "{read}");
}

/// A conversation of its own, whose turn `run`'s interruption started by itself.
fn turn_of(served: &Served, db: &mut ProjectDb, seeded: &super::kontur::Seeded, run: &Run) -> ChatScope {
  let own = ChatScope {
    on_its_own: true,
    ..chat(served, db)
  };
  turn_of_end(db, &own, (seeded, run), RunEnd::Interrupted);
  own
}

#[test]
fn chat_turn_of_contention_continues_once_not_again_soon_and_not_under_contention() {
  let (mut served, seeded) = Served::seeded("task-states");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  allow(&served, OrchestratorConsent::ContinueInterrupted, true);
  allow(&served, OrchestratorConsent::AnswerAgents, true);
  let mut busy = stopped(&mut db, BUSY, RunnerFailure::StorageContention);
  let own = turn_of(&served, &mut db, &seeded, &busy);
  let asked = chat_call(&mut served, &own, "resume_run", resume(&busy)).expect_err("no runner here");
  assert_eq!(refusal(&asked).0, "runner_unavailable", "continued once: {asked:?}");
  // A person continued it on its Summary and the runner stopped it so again at once, while the turn still thinks.
  let events = busy.continue_run(Timestamp::now()).expect("continued");
  db.append_events(&busy, &events).expect("recorded");
  stop(&mut db, &mut busy, RunnerFailure::StorageContention);
  let soon = chat_call(&mut served, &own, "resume_run", resume(&busy)).expect_err("left to the person");
  assert_eq!(
    refusal(&soon),
    ("interrupted_again", "orchestrator.tools.continueAgain")
  );
  // Three more runs of the project stopped so within ten minutes: four in all, more than three.
  let second = stopped(&mut db, BROKEN, RunnerFailure::StorageContention);
  let third = stopped(&mut db, OTHER, RunnerFailure::StorageContention);
  let own = turn_of(&served, &mut db, &seeded, &third);
  let three = chat_call(&mut served, &own, "resume_run", resume(&third)).expect_err("no runner here");
  assert_eq!(refusal(&three).0, "runner_unavailable", "three runs: not yet");
  let (_, mut fourth) = task_run(&db, CRASHED);
  let events = fourth.continue_run(Timestamp::now()).expect("continued");
  db.append_events(&fourth, &events).expect("recorded");
  stop(&mut db, &mut fourth, RunnerFailure::StorageContention);
  assert_eq!(
    db.contended_runs_since(Timestamp::from_unix_millis(0)).expect("count"),
    4
  );
  let own = turn_of(&served, &mut db, &seeded, &second);
  let braked = chat_call(&mut served, &own, "resume_run", resume(&second)).expect_err("under contention");
  assert_eq!(
    refusal(&braked),
    ("storage_contention", "orchestrator.tools.continueContention")
  );
  // Asked in the chat, the person's own request still continues it.
  let scope = chat(&served, &mut db);
  let asked = chat_call(&mut served, &scope, "resume_run", resume(&second)).expect_err("no runner here");
  assert_eq!(refusal(&asked).0, "runner_unavailable", "{asked:?}");
}
