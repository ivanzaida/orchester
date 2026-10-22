//! The Orchestrator chat continues interrupted runs only behind the person's switch (`docs/design/
//! orchestrator-toolset.md`, decision 5), on the seeded Kontur's task states: with *Let the Orchestrator continue
//! interrupted runs* off, `resume_run` of a run a crash, the runner going away or *Quit now* interrupted is refused
//! (`person_only`) as before; with it on, the call goes to the runner to continue the run from its last checkpoint,
//! as the task page's Continue does, and the real runner continues it, kept as the chat's own action. Every limit, a
//! person's cancel, the runner failing to record a run and a gone worktree stay refused whatever the switch says. In a
//! turn an interrupted run's end started by itself, with *answer agents* on too, only that run is continued, and not
//! when it was interrupted again soon after the time before.

use orchester_core::{
  id::ChatMessageId,
  limits::ProjectLimits,
  orchestrator::{ActionOutcome, AgentWait, RunEnd, WaitKind},
  run::{ContinueTrigger, InterruptCause, Run, RunEventKind, RunLimit, RunStatus},
  store::{RunStore, SettingsStore, WorktreeStore},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;
use orchester_workspace::{app_mcp::ToolError, orchestrator::tools::ChatScope, workspace::OrchestratorConsent};
use serde_json::{Value, json};

use super::{
  Served,
  kontur::Seeded,
  orchestrator_chat::{actions, chat, chat_call, task_run},
};

/// The seeded task the seed interrupted with a crash, its worktree there.
const CRASHED: &str = "Import contacts from CSV";

pub(super) fn resume(run: &Run) -> Value {
  json!({ "project": "Kontur", "run": run.id().to_string() })
}

/// The refusal's code and the key of its first reason (the tests' words are the keys).
pub(super) fn refusal(error: &ToolError) -> (&str, &str) {
  match error {
    ToolError::Refused { code, reasons } => (code, reasons.first().map_or("", |reason| reason.key)),
    other => panic!("a refusal: {other:?}"),
  }
}

pub(super) fn allow(served: &Served, consent: OrchestratorConsent, allowed: bool) {
  served
    .services()
    .workspace
    .set_orchestrator_allows(consent, allowed)
    .expect("the person chose it");
}

/// The latest run of the seeded task titled `title`, interrupted now for `cause`.
fn interrupted(db: &mut ProjectDb, title: &str, cause: InterruptCause) -> Run {
  let (_, mut run) = task_run(db, title);
  let events = run.interrupt(cause, Timestamp::now()).expect("interrupted");
  db.append_events(&run, &events).expect("recorded");
  run
}

/// The crashed run, one the runner lost and one *Quit now* interrupted: each with its worktree.
fn interruptions(db: &mut ProjectDb) -> [(InterruptCause, Run); 3] {
  let (_, crashed) = task_run(db, CRASHED);
  assert_eq!(crashed.status(), &RunStatus::Interrupted(InterruptCause::Crashed));
  [
    (InterruptCause::Crashed, crashed),
    (
      InterruptCause::RunnerLost,
      interrupted(db, "Add retry to webhook sender", InterruptCause::RunnerLost),
    ),
    (
      InterruptCause::QuitNow,
      interrupted(db, "Sync contacts with HubSpot", InterruptCause::QuitNow),
    ),
  ]
}

#[test]
fn chat_continues_a_crash_a_lost_runner_and_quit_now_only_with_the_persons_switch() {
  let (mut served, seeded) = Served::seeded("task-states");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let scope = chat(&served, &mut db);
  let runs = interruptions(&mut db);
  for (cause, run) in &runs {
    let refused = chat_call(&mut served, &scope, "resume_run", resume(run)).expect_err("the person's");
    assert_eq!(
      refusal(&refused),
      ("person_only", "orchestrator.tools.resumePersonOnly"),
      "{cause:?} while the switch is off"
    );
  }
  allow(&served, OrchestratorConsent::ContinueInterrupted, true);
  for (cause, run) in &runs {
    // This session's runner cannot start: the call reached it, to continue the run from its last checkpoint.
    let asked = chat_call(&mut served, &scope, "resume_run", resume(run)).expect_err("no runner here");
    assert_eq!(refusal(&asked).0, "runner_unavailable", "{cause:?}: {asked:?}");
  }
  let kept: Vec<String> = actions(&db, &scope)
    .into_iter()
    .map(|action| match action.outcome {
      ActionOutcome::Refused { code, .. } => code,
      other => panic!("refused, not {other:?}"),
    })
    .collect();
  assert_eq!(
    kept,
    [
      "person_only",
      "person_only",
      "person_only",
      "runner_unavailable",
      "runner_unavailable",
      "runner_unavailable"
    ],
    "each call is kept in the conversation"
  );
}

#[test]
fn chat_never_continues_a_limit_a_cancel_a_runner_failure_or_a_gone_worktree() {
  let (mut served, seeded) = Served::seeded("task-states");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let scope = chat(&served, &mut db);
  let (_, mut timed) = task_run(&db, "Rate-limit messages from GitHub");
  let events = timed.stop_at_limit(RunLimit::Time, Timestamp::now()).expect("stopped");
  db.append_events(&timed, &events).expect("recorded");
  let failed = interrupted(
    &mut db,
    "Add serde to the webhook payloads",
    InterruptCause::RunnerFailed,
  );
  let (_, gone) = task_run(&db, CRASHED);
  db.save_worktree_removal(gone.id(), Timestamp::now())
    .expect("the person removed its worktree");
  let limit = "orchestrator.tools.resumePersonOnly";
  let refused = [
    ("Explain an expired GitHub token in the status bar", limit),
    ("Retry failed exports nightly", limit),
    ("Batch webhook deliveries", limit),
    ("Rate-limit messages from GitHub", limit),
    ("Retry Slack notifications", "orchestrator.tools.resumeCanceled"),
    (
      "Add serde to the webhook payloads",
      "orchestrator.tools.continueRunnerFailed",
    ),
    (CRASHED, "orchestrator.tools.continueWorktreeGone"),
  ];
  for allowed in [false, true] {
    allow(&served, OrchestratorConsent::ContinueInterrupted, allowed);
    allow(&served, OrchestratorConsent::AnswerAgents, allowed);
    for (title, key) in refused {
      let (_, run) = task_run(&db, title);
      let error = chat_call(&mut served, &scope, "resume_run", resume(&run)).expect_err("the person's");
      assert_eq!(refusal(&error), ("person_only", key), "{title}, switches on: {allowed}");
    }
  }
  assert_eq!(
    db.run(failed.id()).expect("read").expect("it").status(),
    &RunStatus::Interrupted(InterruptCause::RunnerFailed),
    "nothing continued"
  );
}

/// The turn an end of `run` (`kind`) started by itself in `scope`'s conversation, thinking now.
pub(super) fn turn_of_end(db: &mut ProjectDb, scope: &ChatScope, (seeded, run): (&Seeded, &Run), kind: RunEnd) {
  let wait = AgentWait {
    kind: WaitKind::Ended(kind),
    task: "KON".to_owned(),
    title: "An end".to_owned(),
    run: run.id(),
    number: run.number().get(),
    step: None,
    place: None,
    others: 0,
    agent: None,
    link: format!("/project/{}/tasks", seeded.project),
    own: false,
    automatic: true,
    continuing: None,
    merging: None,
  };
  let conversation = db.chat(scope.chat).expect("read").expect("the conversation");
  let key = format!("end:{}:test", run.id());
  db.keep_agent_wait(&conversation, (&key, &wait), Timestamp::now())
    .expect("kept");
  let seq = db
    .automatic_waits_without_turn()
    .expect("waits")
    .into_iter()
    .find(|kept| kept.key == key)
    .expect("it waits for its turn")
    .seq;
  assert!(
    db.start_wait_turn((scope.chat, seq), ChatMessageId::generate())
      .expect("started")
  );
}

#[test]
fn chat_turn_of_an_interruption_continues_only_its_run_once_and_not_again_soon() {
  let (mut served, seeded) = Served::seeded("task-states");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let own = ChatScope {
    on_its_own: true,
    ..chat(&served, &mut db)
  };
  let [(_, crashed), (_, lost), _] = interruptions(&mut db);
  turn_of_end(&mut db, &own, (&seeded, &crashed), RunEnd::Interrupted);
  allow(&served, OrchestratorConsent::ContinueInterrupted, true);
  // Without answering agents on, no turn is the person's to start by itself.
  let refused = chat_call(&mut served, &own, "resume_run", resume(&crashed)).expect_err("answer agents is off");
  assert_eq!(refusal(&refused).0, "person_only");
  allow(&served, OrchestratorConsent::AnswerAgents, true);
  let other = chat_call(&mut served, &own, "resume_run", resume(&lost)).expect_err("not this turn's run");
  assert_eq!(refusal(&other), ("person_only", "orchestrator.tools.resumePersonOnly"));
  let asked = chat_call(&mut served, &own, "resume_run", resume(&crashed)).expect_err("no runner here");
  assert_eq!(
    refusal(&asked).0,
    "runner_unavailable",
    "the turn's run goes to the runner"
  );
  // The seed's crash was hours ago. A person continued it on its Summary, it crashed at once, they continued it
  // again, and it crashed again at once: two interruptions within ten minutes.
  let mut again = db.run(crashed.id()).expect("read").expect("it");
  for _ in 0..2 {
    let events = again.continue_run(Timestamp::now()).expect("continued");
    db.append_events(&again, &events).expect("recorded");
    let events = again
      .interrupt(InterruptCause::Crashed, Timestamp::now())
      .expect("interrupted again");
    db.append_events(&again, &events).expect("recorded");
  }
  let soon = chat_call(&mut served, &own, "resume_run", resume(&again)).expect_err("left to the person");
  assert_eq!(
    refusal(&soon),
    ("interrupted_again", "orchestrator.tools.continueAgain")
  );
}

#[test]
fn chat_continues_an_interrupted_run_through_the_real_runner_as_its_own_action() {
  let runner = super::runner_programs::programs().runner.clone();
  let (folder, seeded) = Served::seed("task-states", None);
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  // The seed spends today's budget; a run continues only while the day has room.
  let limits = db.project_limits().expect("limits");
  let roomy = ProjectLimits::new(
    limits.default_run_budget(),
    TokenBudget::new(1_000_000_000).expect("a budget"),
    limits.default_time_limit(),
  )
  .expect("limits")
  .with_agent_slots(limits.agent_slots());
  db.save_project_limits(&roomy).expect("saved");
  let mut served = Served::start(folder, &seeded.app_database, runner);
  let scope = chat(&served, &mut db);
  let (_, crashed) = task_run(&db, CRASHED);
  allow(&served, OrchestratorConsent::ContinueInterrupted, true);
  let told = chat_call(&mut served, &scope, "resume_run", resume(&crashed)).expect("continued");
  assert_ne!(told["run"]["status"]["state"], "interrupted", "{told}");
  let action = actions(&db, &scope).pop().expect("kept");
  assert_eq!(action.outcome, ActionOutcome::Done);
  assert_eq!(
    action.resumed,
    Some(crashed.id()),
    "kept as the chat's own continuation"
  );
  let events = db.events(crashed.id()).expect("its history");
  assert!(
    events.iter().any(|event| matches!(
      event.kind(),
      RunEventKind::Continued {
        trigger: ContinueTrigger::User,
        ..
      }
    )),
    "continued from its last checkpoint, as the task page's Continue does"
  );
  // Once it goes on, the same call continues nothing a second time.
  chat_call(&mut served, &scope, "resume_run", resume(&crashed)).expect_err("nothing left to continue");
  assert_eq!(
    db.events(crashed.id())
      .expect("its history")
      .iter()
      .filter(|event| matches!(event.kind(), RunEventKind::Continued { .. }))
      .count(),
    1,
    "one continuation"
  );
}
