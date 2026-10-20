//! `start_qa_app` end to end against `orchester-fake-codex`: the runner starts a QA copy through the worktree's
//! `tools/qa-app.ps1` (`support::qa`'s stand-in), keeps it for the invocation so `call_run_app` without a pid reaches
//! it, and stops it when the agent's turn ends or waits for a person. A project that allows one QA app at once: a
//! second run's start waits, noted in its transcript with the run that holds the slot, until the first app stops. A
//! script that names no app has what it started ended. Several QA apps at once: `qa_app_slots`.
//!
//! The stand-in is a Windows PowerShell script; on macOS the runner runs the script with `pwsh`, which is not part of
//! the test setup, so these tests are Windows only.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test qa_app qa_app`.
#![cfg(windows)]

mod support;

use std::{fs, path::Path};

use orchester_core::{
  connection::stream::RunnerProblem,
  id::{IdempotencyKey, RunId, TaskId},
  run::{Answerer, ApprovalAnswer, DecidedBy, RunStatus},
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc::UnboundedReceiver;

use self::support::{
  git, is_running, pending_approval,
  qa::{allow_qa_apps, approve, calls, gone, holder, noted, qa_project, reported_pid, started_pid, verify_step},
  queue_another_task, scheduler, wait_for,
};

#[tokio::test(flavor = "multi_thread")]
async fn qa_app_one_slot_second_run_waits_then_gets_it_and_apps_stop_at_turn_end() {
  let project = qa_project(verify_step("Start the QA app. Hold the QA app."));
  let root = project.repo.path();
  allow_qa_apps(root, 1);
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let second_task = queue_another_task(root);
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .set_agent_slots(std::num::NonZeroU32::new(2).expect("two"))
    .await
    .expect("two agents at once");

  let first = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("first run");
  approve(&scheduler, root, &mut events, first).await;
  let first_app = started_pid(root, &mut events, first).await;
  assert!(is_running(&first_app), "the first run's QA app runs");
  assert!(
    calls(root, first)
      .iter()
      .any(|call| call == "call_run_app ok: app ready"),
    "call_run_app without a pid reaches the step's QA app: {:?}",
    calls(root, first)
  );

  let second = scheduler
    .start_run(IdempotencyKey::generate(), second_task)
    .await
    .expect("second run");
  approve(&scheduler, root, &mut events, second).await;
  let waiting = RunnerProblem::WaitingForQaSlot {
    holders: vec![holder(root, first)],
  };
  noted(root, &mut events, second, waiting).await;
  assert!(
    !calls(root, second).iter().any(|call| call.starts_with("start_qa_app")),
    "the second start waits while the first app holds the slot"
  );

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, first, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let second_app = started_pid(root, &mut events, second).await;
  // The slot is freed only once the first app stopped.
  assert!(!is_running(&first_app), "the first app stopped when its turn ended");
  let run = wait_for(root, &mut events, second, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let answered = calls(root, second);
  assert!(
    answered
      .iter()
      .any(|call| call.starts_with("start_qa_app ok") && call.contains("waited")),
    "{answered:?}"
  );
  gone(&second_app, "the second app, after its turn ended").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn qa_app_stops_while_its_turn_waits_for_a_person() {
  let project = qa_project(verify_step("Start the QA app. Then read outside."));
  let root = project.repo.path();
  let deps = root.join("deps-a");
  fs::create_dir_all(&deps).expect("deps");
  fs::write(deps.join("notes.txt"), "outside line\n").expect("notes");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let app = started_pid(root, &mut events, id).await;
  // The read outside the worktree asks: while the person decides, the app is stopped and the slot free.
  gone(&app, "the QA app while its turn waits for a person").await;
  approve(&scheduler, root, &mut events, id).await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let answered = calls(root, id);
  assert!(
    answered
      .iter()
      .any(|call| call.starts_with("call_run_app error") && call.contains("waited for a person")),
    "the agent is told its app was stopped and starts it again: {answered:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn qa_app_a_start_a_person_just_approved_runs_at_once() {
  let project = qa_project(verify_step("Start the QA app. Then read outside. Then start again."));
  let root = project.repo.path();
  let deps = root.join("deps-a");
  fs::create_dir_all(&deps).expect("deps");
  fs::write(
    deps.join("notes.txt"),
    "outside line
",
  )
  .expect("notes");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  // The first start, the read outside, then the second start, each answered by the person; the agent calls the
  // second start at once after the read's answer, and its start runs as soon as the person approves it.
  for _ in 0..3 {
    approve(&scheduler, root, &mut events, id).await;
  }
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let starts: Vec<String> = calls(root, id)
    .into_iter()
    .filter(|call| call.starts_with("start_qa_app"))
    .collect();
  assert_eq!(starts.len(), 2, "{starts:?}");
  assert!(
    starts.iter().all(|call| call.starts_with("start_qa_app ok")),
    "{starts:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn qa_app_a_script_that_names_no_app_has_what_it_started_ended() {
  let project = qa_project(verify_step("Start the QA app. QA scenario nopid."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let app = reported_pid(root, "run-1-verify", "app").await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let answered = calls(root, id);
  assert!(
    answered
      .iter()
      .any(|call| call.starts_with("start_qa_app error") && call.contains("without naming a started app")),
    "{answered:?}"
  );
  gone(&app, "the app a script started without naming it").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn qa_app_a_script_changed_after_its_approval_does_not_run() {
  let project = qa_project(verify_step("Start the QA app."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  let request = pending_approval(root, &mut events, id).await;
  let target = request.target.clone().unwrap_or_default();
  assert!(
    target.starts_with("tools/qa-app.ps1 ") && target.contains(" · scenario tasks"),
    "{target}"
  );
  // The script changes while the person reads the request.
  let script = root
    .join(".orchester/worktrees")
    .join(id.to_string())
    .join("tools")
    .join("qa-app.ps1");
  let mut text = fs::read_to_string(&script).expect("the script");
  text.push_str(
    "# changed after the approval
",
  );
  fs::write(&script, text).expect("change the script");
  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let answered = calls(root, id);
  assert!(
    answered
      .iter()
      .any(|call| call.starts_with("start_qa_app error") && call.contains("not the script this start's approval named")),
    "{answered:?}"
  );
}

/// Runs `task` to its end, answering its `start_qa_app` request, if it asks, with `answer` by `by` first; whether it
/// asked.
async fn run_answering(
  (scheduler, events): (&Scheduler, &mut UnboundedReceiver<Event>),
  root: &Path,
  task: TaskId,
  answer: Option<ApprovalAnswer>,
) -> RunId {
  let id = scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("run");
  if let Some(answer) = answer {
    let request = pending_approval(root, events, id).await;
    assert_eq!(request.action, "start_qa_app");
    scheduler
      .decide_tool_approval(id, request.id, answer, Answerer::Person)
      .await
      .expect("approved");
  }
  let run = wait_for(root, events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert!(
    calls(root, id).iter().any(|call| call.starts_with("start_qa_app ok")),
    "{:?}",
    calls(root, id)
  );
  id
}

/// How `run`'s requests were answered, oldest first.
fn answered_by(root: &Path, run: RunId) -> Vec<Option<DecidedBy>> {
  ProjectDb::open(root)
    .expect("open")
    .decided_tool_approvals(run)
    .expect("decided")
    .into_iter()
    .map(|decided| decided.decided_by)
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn qa_app_a_project_rule_approves_it_in_every_run_while_the_script_is_unchanged() {
  let project = qa_project(verify_step("Start the QA app."));
  let root = project.repo.path();
  let tasks: Vec<_> = (0..3).map(|_| queue_another_task(root)).collect();
  let (scheduler, mut events) = scheduler(root);

  // Automation never makes a rule of the project.
  let first = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("first run");
  let request = pending_approval(root, &mut events, first).await;
  let refused = scheduler
    .decide_tool_approval(
      first,
      request.id.clone(),
      ApprovalAnswer::SimilarProject,
      Answerer::Automation,
    )
    .await;
  assert_eq!(refused, Err(Rejection::AutomationWider));
  let db = || ProjectDb::open(root).expect("open");
  assert!(db().project_approval_rules().expect("rules").is_empty());
  scheduler
    .decide_tool_approval(first, request.id, ApprovalAnswer::SimilarProject, Answerer::Person)
    .await
    .expect("approved for every run");
  wait_for(root, &mut events, first, |run| run.status().is_terminal()).await;
  let rules = db().project_approval_rules().expect("rules");
  assert_eq!(rules.len(), 1);
  assert_eq!(rules[0].made_in, first);

  // Another run's start is approved by the rule, without asking.
  let second = run_answering((&scheduler, &mut events), root, tasks[0], None).await;
  assert_eq!(answered_by(root, second), vec![Some(DecidedBy::SimilarRule)]);
  assert_eq!(db().project_approval_rules().expect("rules")[0].answered, 1);

  // A changed script asks again.
  let script = root.join("tools").join("qa-app.ps1");
  let original = fs::read_to_string(&script).expect("script");
  fs::write(&script, format!("{original}# changed\n")).expect("change the script");
  git(root, &["commit", "--quiet", "-am", "QA script changed"]);
  let third = run_answering((&scheduler, &mut events), root, tasks[1], Some(ApprovalAnswer::Once)).await;
  assert_eq!(answered_by(root, third), vec![Some(DecidedBy::Person)]);

  // The approved script again, with the rule revoked: it asks again.
  fs::write(&script, original).expect("restore the script");
  git(root, &["commit", "--quiet", "-am", "QA script restored"]);
  let rule = db().project_approval_rules().expect("rules")[0].id;
  assert!(
    db()
      .revoke_project_approval_rule(rule, Timestamp::now())
      .expect("revoked")
  );
  let fourth = run_answering((&scheduler, &mut events), root, tasks[2], Some(ApprovalAnswer::Once)).await;
  assert_eq!(answered_by(root, fourth), vec![Some(DecidedBy::Person)]);
}
