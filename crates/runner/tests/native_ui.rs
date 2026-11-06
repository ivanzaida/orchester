//! The `native` UI driver end to end against `orchester-fake-codex`, with `orchester-test-window` as the service
//! (`support/test_window.rs`, `support/fake_native_tools.rs`): a published service with the native driver offers the
//! `ui_*` tools; `ui_open` finds the fixture's window among its service's processes and moves it off-screen without
//! activating it; the snapshot is its UI Automation tree with refs, roles, names, states and values; typing sets the
//! field's value and a click invokes Save through their patterns; an element without the pattern answers unsupported,
//! and a key is never pressed; the screenshot is the window's own rendering, of its size and colours; the log is the
//! service's output. A service whose process the run may not reach — its program outside the worktree — has its
//! windows refused, and a window the run did not start is never among them.
//!
//! Windows only: the macOS driver needs a Mac with Accessibility and Screen Recording granted, and its pure parts are
//! tested in `orchester-exec`'s `tests/native_ui.rs`. Needs `git`. Run with `cargo test -p orchester-runner --features
//! test-agent --test native_ui native_ui`.
#![cfg(windows)]

mod support;

use std::{
  path::Path,
  process::{Child, Command, Stdio},
};

use orchester_core::{
  id::IdempotencyKey,
  resource::{CommandArg, CommandProgram},
  run::RunStatus,
  service::{LogPattern, Readiness, ReadyCheck, ReadyTimeout, Service, ServiceName, UiDriver},
};
use serde_json::Value;

use self::support::{
  StepSpec,
  qa::{approve, calls},
  scheduler,
  services::{publish, service_project},
  wait_for,
};

/// The fixture window, as cargo built it.
const FIXTURE: &str = env!("CARGO_BIN_EXE_orchester-test-window");
/// The fixture's background colour, which a screenshot shows below its controls.
const BACKGROUND: [u8; 3] = [0x24, 0x8A, 0x3D];

fn implement(instructions: &'static str) -> Vec<StepSpec> {
  vec![StepSpec::new("implement", "Implement", instructions, &[])]
}

/// The service `name` running the fixture `program` with `arguments`, ready once it shows its window, with the native
/// driver.
fn window_service(name: &str, program: &str, arguments: &[&str]) -> Service {
  let ready = ReadyCheck::LogLine(LogPattern::new("window ready").expect("a pattern"));
  let readiness = Readiness::new(ready, ReadyTimeout::from_secs(30).expect("in range"));
  let arguments = arguments
    .iter()
    .map(|argument| CommandArg::new(argument).expect("an argument"))
    .collect();
  Service::new(
    ServiceName::new(name).expect("a name"),
    CommandProgram::new(program).expect("a program"),
    readiness,
  )
  .with_arguments(arguments)
  .with_ui_driver(UiDriver::Native)
}

/// The JSON answer of the `index`th call of `tool` that succeeded.
fn answer(answered: &[String], tool: &str, index: usize) -> Value {
  let prefix = format!("{tool} ok: ");
  let line = answered
    .iter()
    .filter_map(|call| call.strip_prefix(&prefix))
    .nth(index)
    .unwrap_or_else(|| panic!("no answer {index} of {tool}: {answered:#?}"));
  serde_json::from_str(line).unwrap_or_else(|_| panic!("{tool} answered no JSON: {line}"))
}

/// The error of the `index`th call of `tool` that failed.
fn error<'a>(answered: &'a [String], tool: &str, index: usize) -> &'a str {
  let prefix = format!("{tool} error: ");
  answered
    .iter()
    .filter_map(|call| call.strip_prefix(&prefix))
    .nth(index)
    .unwrap_or_else(|| panic!("no error {index} of {tool}: {answered:#?}"))
}

/// The first element of `tree` with `role`, whose name starts with `name`.
fn element<'a>(tree: &'a Value, role: &str, name: &str) -> Option<&'a Value> {
  if tree["role"] == role && tree["name"].as_str().is_some_and(|own| own.starts_with(name)) {
    return Some(tree);
  }
  tree["children"]
    .as_array()
    .into_iter()
    .flatten()
    .find_map(|child| element(child, role, name))
}

fn states(element: &Value) -> Vec<&str> {
  element["states"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn native_ui_drives_the_fixture_window_through_ui_automation_patterns() {
  let project = service_project(
    implement("Start the service desktop. Copy the native fixture. Then drive the native window."),
    Vec::new(),
  );
  let root = project.repo.path();
  publish(
    root,
    &[window_service("desktop", "fixture/orchester-test-window.exe", &[])],
  );
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let answered = calls(root, id);

  let opened = answer(&answered, "ui_open", 0);
  assert_eq!(opened["driver"], "native");
  let window = &opened["windows"][0];
  assert_eq!(window["title"], "Orchester native fixture");
  assert_eq!(
    window["off_screen"], true,
    "ui_open moved the window off every monitor: {window}"
  );
  let first = answer(&answered, "ui_snapshot", 0);
  first_snapshot(&first["tree"]);

  for index in 0..3 {
    let acted = answer(&answered, "ui_act", index);
    assert!(acted["ref"].is_string(), "{acted}");
  }
  let waited = answer(&answered, "ui_wait", 0);
  assert_eq!(waited["met"], "element");
  assert_eq!(waited["matches"][0]["name"], "Saved: Fix login", "{waited}");
  unsupported(&answered);
  second_snapshot(&answer(&answered, "ui_snapshot", 1)["tree"]);
  screenshot(&answer(&answered, "ui_screenshot", 0), window);
  let lines = answer(&answered, "ui_logs", 0)["lines"].clone();
  assert_eq!(lines, serde_json::json!(["window ready", "saved Fix login"]));
  assert_eq!(answer(&answered, "ui_wait", 1)["met"], "idle");
}

/// The fixture's tree before anything was done: its window, its controls and how they stand.
fn first_snapshot(tree: &Value) {
  assert_eq!(tree["role"], "window", "{tree}");
  assert_eq!(tree["name"], "Orchester native fixture");
  assert!(tree["ref"].as_str().is_some_and(|reference| reference.starts_with('u')));
  assert!(element(tree, "button", "Save").is_some(), "{tree}");
  assert!(element(tree, "text", "Status: idle").is_some(), "{tree}");
  let field = element(tree, "edit", "").expect("the field");
  assert!(field.get("value").is_none(), "the field is empty: {field}");
  let remember = element(tree, "checkbox", "Remember me").expect("the checkbox");
  assert!(states(remember).contains(&"unchecked"), "{remember}");
}

/// The fixture's tree after typing, Save and the checkbox.
fn second_snapshot(tree: &Value) {
  assert!(element(tree, "text", "Saved: Fix login").is_some(), "{tree}");
  let field = element(tree, "edit", "").expect("the field");
  assert_eq!(field["value"], "Fix login");
  let remember = element(tree, "checkbox", "Remember me").expect("the checkbox");
  assert!(states(remember).contains(&"checked"), "{remember}");
}

/// A click on the label, which has no pattern for it, and a key, which the driver never presses.
fn unsupported(answered: &[String]) {
  let label = error(answered, "ui_act", 0);
  assert!(
    label.starts_with("unsupported for this element: click needs the Invoke, Toggle, SelectionItem or ExpandCollapse")
      && label.contains("(text)")
      && label.contains("never clicks a point"),
    "{label}"
  );
  let key = error(answered, "ui_act", 1);
  assert!(key.contains("presses no keys"), "{key}");
}

/// The screenshot: a PNG in the run's storage of the window's own size, its background below the controls.
fn screenshot(shot: &Value, window: &Value) {
  let path = shot["screenshot"].as_str().expect("a path");
  assert!(
    path.contains("/.orchester/runs/") && path.contains("/screenshots/app-") && path.ends_with(".png"),
    "saved in the run's own storage: {path}"
  );
  let image = image::open(Path::new(path)).expect("a PNG").to_rgba8();
  let bounds = &window["bounds"];
  assert_eq!(
    (u64::from(image.width()), u64::from(image.height())),
    (
      bounds["width"].as_u64().expect("a width"),
      bounds["height"].as_u64().expect("a height")
    ),
    "only the window, never the screen"
  );
  let pixel = image.get_pixel(image.width() / 2, image.height() - 30);
  assert_eq!(pixel.0[..3], BACKGROUND, "the window drew itself, off-screen as it is");
}

/// A window of the fixture that the test starts itself, beyond the desktop: no run started it.
struct Foreign(Child);

impl Drop for Foreign {
  #[allow(clippy::print_stderr, reason = "a window the test could not end is said, not hidden")]
  fn drop(&mut self) {
    if let Err(error) = self.0.kill().and_then(|()| self.0.wait().map(drop)) {
      eprintln!("the test's own fixture window was not ended: {error}");
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn native_ui_refuses_windows_of_processes_the_run_may_not_reach() {
  let foreign = Foreign(
    Command::new(FIXTURE)
      .arg("off-screen")
      .stdout(Stdio::null())
      .spawn()
      .expect("the test's own window"),
  );
  let project = service_project(implement("Start the service outside. Then open its UI."), Vec::new());
  let root = project.repo.path();
  // Started by the run, but from cargo's build folder, outside the run's worktree.
  publish(root, &[window_service("outside", FIXTURE, &["off-screen"])]);
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let answered = calls(root, id);
  assert!(answer(&answered, "start_service", 0)["port"].is_u64());
  let refused = error(&answered, "ui_open", 0);
  assert!(
    refused.starts_with("refused: no window of the service `outside` belongs to a process this run may reach")
      && refused.contains("(orchester-test-window.exe): its executable is outside this run's worktree"),
    "{refused}"
  );
  assert!(
    !refused.contains(&format!("pid {} ", foreign.0.id())),
    "the test's own window is never one of the service's: {refused}"
  );
  assert!(
    !answered.iter().any(|call| call.starts_with("ui_open ok")),
    "{answered:#?}"
  );
}
