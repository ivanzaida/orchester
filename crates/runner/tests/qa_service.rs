//! The QA app as the built-in service `desktop-qa` (`docs/architecture/agent-testing.md`, "UI drivers"), end to end
//! against `orchester-fake-codex` and the stand-in QA script and app (`support::qa`): `start_service("desktop-qa")`
//! asks with the QA script's approval and starts the QA copy; the `ui_*` tools then see and use it through the `mcp`
//! driver, each call one of the app's lurq tools, and answer in the common shape; `service_status` and `service_logs`
//! of it say where it is and read its log. A start through `start_service` takes one of the service slots for QA
//! apps: with one allowed, another run's `start_qa_app` waits for it, and gets it once the first app stopped.
//!
//! Windows only, as `qa_app`: the stand-in script is Windows PowerShell.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test qa_service qa_service`.
#![cfg(windows)]

mod support;

use std::{fs, num::NonZeroU32, path::Path};

use orchester_core::{connection::stream::RunnerProblem, id::IdempotencyKey, run::RunStatus};
use serde_json::{Value, json};

use self::support::{
  PATIENCE, is_running, pending_approval,
  qa::{allow_qa_apps, approve, calls, gone, holder, noted, qa_project, started_pid, verify_folder, verify_step},
  queue_another_task, scheduler, wait_for,
};

/// The JSON answer of the `index`th call of `tool` in `answered`, which reported `<tool> ok: <answer>`.
fn answer(answered: &[String], tool: &str, index: usize) -> Value {
  let prefix = format!("{tool} ok: ");
  let line = answered
    .iter()
    .filter_map(|call| call.strip_prefix(&prefix))
    .nth(index)
    .unwrap_or_else(|| panic!("no answer {index} of {tool}: {answered:#?}"));
  serde_json::from_str(line).unwrap_or_else(|_| panic!("{tool} answered no JSON: {line}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn qa_service_starts_through_start_service_and_the_ui_tools_drive_it_through_the_mcp_driver() {
  let project = qa_project(verify_step(
    "Start the service desktop-qa. Then drive its UI. Hold the QA app.",
  ));
  let root = project.repo.path();
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  let request = pending_approval(root, &mut events, id).await;
  assert_eq!(request.action, "start_service");
  let target = request.target.clone().unwrap_or_default();
  assert!(
    target.starts_with("tools/qa-app.ps1 ") && target.contains(" · scenario tasks"),
    "the start names the QA script, as start_qa_app's does: {target}"
  );
  approve(&scheduler, root, &mut events, id).await;

  // The UI calls are made before the agent holds the app: wait for the last of them.
  let answered = tokio::time::timeout(PATIENCE, async {
    loop {
      let answered = calls(root, id);
      if answered.iter().any(|call| call.starts_with("service_logs")) {
        return answered;
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("the agent drives the UI");

  let started = answer(&answered, "start_service", 0);
  assert_eq!(started["service"], "desktop-qa");
  assert_eq!(started["driver"], "mcp");
  assert_eq!(started["scenario"], "tasks");
  let pid = started["pid"].as_u64().expect("a pid").to_string();
  assert!(is_running(&pid), "the QA app runs");
  ui_answers(&answered, &pid);
  lurq_calls(&verify_folder(root, id));

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  gone(&pid, "the QA service, after its turn ended").await;
}

/// The `ui_*` answers of "Then drive its UI" on the QA app `pid`, each in the common shape.
fn ui_answers(answered: &[String], pid: &str) {
  assert!(
    answered
      .iter()
      .any(|call| call.starts_with("ui_snapshot error") && call.contains("ui_open")),
    "a snapshot before ui_open is refused: {answered:#?}"
  );
  let opened = answer(answered, "ui_open", 0);
  assert_eq!(opened["service"], "desktop-qa");
  assert_eq!(opened["driver"], "mcp");
  assert_eq!(opened["pid"].as_u64().map(|pid| pid.to_string()).as_deref(), Some(pid));
  assert_eq!(opened["windows"][0]["title"], "Orchester QA");

  assert_eq!(
    answer(answered, "ui_snapshot", 0),
    json!({"tree": {"ref": "ref_1", "role": "window", "name": "Orchester QA", "children": [
      {"ref": "ref_2", "role": "button", "name": "Save"},
      {"ref": "ref_3", "role": "textbox", "name": "Title"},
      {"ref": "ref_4", "role": "combobox", "name": "Priority"},
    ]}, "truncated": false}),
    "the snapshot is a tree of refs, roles and names"
  );
  assert_eq!(
    answer(answered, "ui_snapshot", 1),
    json!({"matches": [{"ref": "ref_2", "role": "button", "name": "Save"}], "truncated": false})
  );
  let acted: Vec<Value> = (0..5).map(|index| answer(answered, "ui_act", index)).collect();
  assert_eq!(
    acted,
    [
      json!({"acted": "click", "ref": "ref_2"}),
      json!({"acted": "type", "ref": "ref_3"}),
      json!({"acted": "key", "ref": null}),
      json!({"acted": "scroll", "ref": "ref_3"}),
      json!({"acted": "select", "ref": "ref_4"}),
    ]
  );
  let shot = answer(answered, "ui_screenshot", 0);
  let path = shot["screenshot"].as_str().expect("a path");
  assert!(
    path.contains("/.orchester/runs/") && path.contains("/screenshots/app-") && path.ends_with(".png"),
    "saved in the run's own storage: {path}"
  );
  let image = image::open(path).expect("a PNG");
  assert_eq!((image.width(), image.height()), (2, 2));
  assert_eq!(
    answer(answered, "ui_logs", 0),
    json!({"lines": ["INFO app ready", "INFO saved the task"]})
  );
  assert_eq!(answer(answered, "ui_wait", 0), json!({"met": "idle"}));
  assert_eq!(
    answer(answered, "ui_wait", 1),
    json!({"met": "element", "matches": [{"ref": "ref_9", "role": "status", "name": "Saved"}]})
  );
  let status = answer(answered, "service_status", 0);
  assert_eq!(status["state"], "ready");
  assert_eq!(status["pid"].as_u64().map(|pid| pid.to_string()).as_deref(), Some(pid));
  assert_eq!(answer(answered, "service_logs", 0)["lines"][0], "INFO app ready");
}

/// Each `ui_*` call was one of the app's lurq tools, with its arguments, as the stand-in in the QA folder `folder`
/// noted them; the wait for an element looked until it showed.
fn lurq_calls(folder: &Path) {
  let lurq = fs::read_to_string(folder.join("mcp").join("lurq-calls")).expect("the app's calls");
  assert_eq!(
    lurq.lines().collect::<Vec<_>>(),
    [
      r#"lurq_windows {}"#,
      r#"lurq_inspect {"max_depth":40,"max_nodes":300}"#,
      r#"lurq_inspect {"max_depth":40,"max_nodes":300,"query":"Save"}"#,
      r#"lurq_act {"action":"invoke","ref":"ref_2"}"#,
      r#"lurq_interact {"action":"type","ref":"ref_3","text":"Fix login"}"#,
      r#"lurq_interact {"action":"key","key":"Enter"}"#,
      r#"lurq_interact {"action":"scroll_to","ref":"ref_3"}"#,
      r#"lurq_set_value {"ref":"ref_4","value":"High"}"#,
      r#"lurq_screenshot {}"#,
      r#"lurq_logs {"lines":5}"#,
      r#"lurq_wait {"timeout_ms":10000}"#,
      r#"lurq_inspect {"max_depth":40,"max_nodes":300,"query":"Saved"}"#,
      r#"lurq_inspect {"max_depth":40,"max_nodes":300,"query":"Saved"}"#,
      r#"lurq_logs {"lines":100}"#,
    ]
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn qa_service_takes_a_service_slot_another_runs_qa_app_waits_for() {
  let project = qa_project(verify_step("Start the service desktop-qa. Hold the QA app."));
  let root = project.repo.path();
  allow_qa_apps(root, 1);
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
  let first_app = tokio::time::timeout(PATIENCE, async {
    loop {
      let answered = calls(root, first);
      if answered.iter().any(|call| call.starts_with("start_service")) {
        return answer(&answered, "start_service", 0)["pid"]
          .as_u64()
          .expect("a pid")
          .to_string();
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("the QA service starts");
  assert!(is_running(&first_app), "the first run's QA app runs");

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
    !calls(root, second).iter().any(|call| call.starts_with("start_service")),
    "the second start waits while the first run's QA service holds the slot"
  );

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, first, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let run = wait_for(root, &mut events, second, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert!(!is_running(&first_app), "the first app stopped when its turn ended");
  let second_start = answer(&calls(root, second), "start_service", 0);
  assert!(
    second_start["waited_secs"].as_f64().is_some_and(|waited| waited > 0.0),
    "the second start waited for the slot: {second_start}"
  );
  let second_app = second_start["pid"].as_u64().expect("a pid").to_string();
  gone(&second_app, "the second app, after its turn ended").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn qa_service_start_qa_app_stays_its_alias_and_points_to_the_ui_tools() {
  let project = qa_project(verify_step("Start the QA app."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let app = started_pid(root, &mut events, id).await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let answered = calls(root, id);
  let started = answered
    .iter()
    .find(|call| call.starts_with("start_qa_app ok"))
    .expect("started");
  assert!(started.contains("ui_open(\"desktop-qa\")"), "{started}");
  assert!(
    answered.iter().any(|call| call == "call_run_app ok: app ready"),
    "{answered:#?}"
  );
  gone(&app, "the QA app, after its turn ended").await;
}
