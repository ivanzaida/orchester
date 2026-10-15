//! `start_qa_app` end to end against `orchester-fake-codex`: the runner starts a QA copy through the worktree's
//! `tools/qa-app.ps1` (`support::qa`'s stand-in), keeps it for the invocation so `call_run_app` without a pid reaches
//! it, and stops it when the agent's turn ends or waits for a person. One QA slot: a second run's start waits, noted
//! in its transcript, until the first app stops. A script that names no app has what it started ended.
//!
//! The stand-in is a Windows PowerShell script; on macOS the runner runs the script with `pwsh`, which is not part of
//! the test setup, so these tests are Windows only.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test qa_app qa_app`.
#![cfg(windows)]

mod support;

use std::fs;

use orchester_core::{connection::stream::RunnerProblem, id::IdempotencyKey, run::RunStatus};

use self::support::{
  is_running,
  qa::{approve, calls, gone, noted, qa_project, reported_pid, started_pid, verify_step},
  queue_another_task, scheduler, wait_for,
};

#[tokio::test(flavor = "multi_thread")]
async fn qa_app_one_slot_second_run_waits_then_gets_it_and_apps_stop_at_turn_end() {
  let project = qa_project(verify_step("Start the QA app. Hold the QA app."));
  let root = project.repo.path();
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
  noted(root, &mut events, second, RunnerProblem::WaitingForQaSlot).await;
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
  let request = support::pending_approval(root, &mut events, id).await;
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
    .decide_tool_approval(
      id,
      request.id,
      orchester_core::run::ApprovalAnswer::Once,
      orchester_core::run::Answerer::Person,
    )
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
