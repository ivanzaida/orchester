//! Several QA apps at once (`start_qa_app`, Project settings › Limits › *QA apps at once*): with two allowed, two
//! runs' apps run together, each from its own QA folder with its own discovery file, and a third run's start waits,
//! noted in its transcript with the runs that hold the slots, until one of them stops. A slot's leftover cleanup
//! leaves alone the app another slot holds, even when its own record names that app's QA folder.
//! Windows only, as `qa_app`: the stand-in script (`support::qa`) is Windows PowerShell.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test qa_app_slots qa_app`.
#![cfg(windows)]

mod support;

use std::{fs, num::NonZeroU32};

use orchester_core::{connection::stream::RunnerProblem, id::IdempotencyKey, run::RunStatus};

use self::support::{
  is_running,
  qa::{allow_qa_apps, approve, calls, gone, holder, noted, qa_project, started_pid, verify_folder, verify_step},
  queue_another_task, scheduler, wait_for,
};

#[tokio::test(flavor = "multi_thread")]
async fn qa_app_two_slots_run_two_apps_at_once_and_a_third_waits_until_one_stops() {
  let project = qa_project(verify_step("Start the QA app. Hold the QA app."));
  let root = project.repo.path();
  allow_qa_apps(root, 2);
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let tasks = [project.task, queue_another_task(root), queue_another_task(root)];
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .set_agent_slots(NonZeroU32::new(3).expect("three"))
    .await
    .expect("three agents at once");

  let mut runs = Vec::new();
  let mut apps = Vec::new();
  for task in &tasks[..2] {
    let run = scheduler
      .start_run(IdempotencyKey::generate(), *task)
      .await
      .expect("run");
    approve(&scheduler, root, &mut events, run).await;
    apps.push(started_pid(root, &mut events, run).await);
    runs.push(run);
  }
  assert_ne!(apps[0], apps[1]);
  assert!(apps.iter().all(|app| is_running(app)), "both runs' QA apps run at once");
  for run in &runs {
    assert!(
      calls(root, *run)
        .iter()
        .any(|call| call == "call_run_app ok: app ready"),
      "each run reaches its own app through its own discovery file: {:?}",
      calls(root, *run)
    );
  }
  assert_ne!(verify_folder(root, runs[0]), verify_folder(root, runs[1]));

  let third = scheduler
    .start_run(IdempotencyKey::generate(), tasks[2])
    .await
    .expect("third run");
  approve(&scheduler, root, &mut events, third).await;
  let waiting = RunnerProblem::WaitingForQaSlot {
    holders: runs.iter().map(|run| holder(root, *run)).collect(),
  };
  noted(root, &mut events, third, waiting).await;
  assert!(
    !calls(root, third).iter().any(|call| call.starts_with("start_qa_app")),
    "the third start waits while both slots are held"
  );

  fs::remove_file(&hold).expect("release");
  let third_app = started_pid(root, &mut events, third).await;
  assert!(
    !apps.iter().all(|app| is_running(app)),
    "the third app started only once one of the first two stopped"
  );
  for run in runs.iter().chain([&third]) {
    let ended = wait_for(root, &mut events, *run, |run| run.status().is_terminal()).await;
    assert_eq!(ended.status(), &RunStatus::Succeeded);
  }
  assert!(
    calls(root, third)
      .iter()
      .any(|call| call.starts_with("start_qa_app ok") && call.contains("waited")),
    "{:?}",
    calls(root, third)
  );
  gone(&third_app, "the third app, after its turn ended").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn qa_app_a_slots_leftover_cleanup_leaves_the_app_another_slot_holds() {
  let project = qa_project(verify_step("Start the QA app. Hold the QA app."));
  let root = project.repo.path();
  allow_qa_apps(root, 2);
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let second_task = queue_another_task(root);
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .set_agent_slots(NonZeroU32::new(2).expect("two"))
    .await
    .expect("two agents at once");

  let first = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("first run");
  approve(&scheduler, root, &mut events, first).await;
  let first_app = started_pid(root, &mut events, first).await;
  // Slot 1's record names the first run's QA folder, as if that step had held slot 1 before and its runner died (its
  // run created at the Unix epoch, so every program there counts); the first run's app holds slot 0 now.
  let named = holder(root, first);
  fs::write(
    root.join(".orchester").join("qa-slots").join("slot-1.holder"),
    format!(
      "0\n{}\n{}\n{}",
      named.task,
      named.run,
      verify_folder(root, first).display()
    ),
  )
  .expect("slot 1's record");

  let second = scheduler
    .start_run(IdempotencyKey::generate(), second_task)
    .await
    .expect("second run");
  approve(&scheduler, root, &mut events, second).await;
  let second_app = started_pid(root, &mut events, second).await;
  assert!(is_running(&second_app), "the second run's app took slot 1");
  assert!(
    is_running(&first_app),
    "slot 1's cleanup left alone the app slot 0 holds from the folder its record names"
  );
  fs::remove_file(&hold).expect("release");
  gone(&first_app, "the first app, after its turn ended").await;
  gone(&second_app, "the second app, after its turn ended").await;
}
