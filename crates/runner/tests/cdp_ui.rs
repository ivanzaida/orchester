//! The `cdp` UI driver end to end against `orchester-fake-codex`, with this computer's Chrome, Edge, Brave or Chromium,
//! headless, as the stand-in of an Electron app (`support/fake_cdp_tools.rs`): the service is a launcher in the
//! worktree (`orchester-test-command launch`) that starts the browser on the fixture window with
//! `--remote-debugging-port={{port}}`, as an Electron app's `npm start` does. `ui_open` attaches to the window through
//! that port once a process of the service's own tree listens on it; the snapshot is its accessibility tree with refs
//! of DOM nodes; typing and a click go by ref; `ui_wait` waits for idle, a text and an element; the screenshot is saved
//! in the run's storage; `ui_logs` reads its console; the service's process got the port in
//! `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` too, as a WebView2 app needs. A debugging port that a process of another
//! service's tree listens on is refused. Where no browser is found each test says so and checks nothing.
//!
//! Windows only: the driver asks the system's TCP table which process listens on the port, and on macOS it answers
//! that the native driver is the one to use (`orchester-core`'s `tests/service/launch.rs`). Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test cdp_ui cdp_ui`.
#![cfg(windows)]

mod support;

use std::path::{Path, PathBuf};

use orchester_core::{
  id::{IdempotencyKey, RunId},
  resource::{CommandArg, CommandProgram},
  run::RunStatus,
  service::{LogPattern, Readiness, ReadyCheck, ReadyTimeout, Service, ServiceName, UiDriver, cdp::WEBVIEW2_ARGUMENTS},
};
use orchester_store::ProjectDb;
use serde_json::Value;
use tempfile::TempDir;

use self::support::{
  StepSpec, TestProject,
  qa::{approve, calls},
  reported, scheduler,
  services::{publish, service, service_project, started_port},
  wait_for,
};

/// What the browser prints once its debugging port listens.
const LISTENING: &str = "DevTools listening on";

/// The fixture window: a field, Save, and a status Save fills, logging to the console as it does.
const WINDOW: &str = r#"<!doctype html>
<html><head><title>Fixture desktop app</title></head>
<body>
<label>Title <input id="title" aria-label="Title"></label>
<button id="save" onclick="save()">Save</button>
<p id="status">Status: idle</p>
<script>
function save() {
  const title = document.getElementById("title").value;
  document.getElementById("status").textContent = "Saved: " + title;
  console.log("saved " + title);
  console.error("save logged an error on purpose");
}
</script>
</body></html>
"#;

/// This computer's browser; where it has none, the test says so and checks nothing.
#[allow(
  clippy::print_stderr,
  reason = "says that nothing was checked where no browser is installed"
)]
fn browser() -> Option<PathBuf> {
  match orchester_exec::browser::find() {
    Ok(found) => Some(found.path().to_path_buf()),
    Err(unavailable) => {
      eprintln!("skipped, no browser to stand in for an Electron app: {unavailable}");
      None
    }
  }
}

/// The fixture window written into `folder`, as the address the browser opens, and a profile folder of its own.
fn fixture(folder: &Path) -> (String, PathBuf) {
  let window = folder.join("window.html");
  std::fs::write(&window, WINDOW).expect("the fixture window");
  let address = format!("file:///{}", window.to_string_lossy().replace('\\', "/"));
  (address, folder.join("profile"))
}

/// The arguments that start `browser` headless on `profile` with its debugging port on `port` (a placeholder) and
/// `window` open, as an Electron app's main window.
fn browser_arguments(browser: &Path, port: &str, (window, profile): &(String, PathBuf)) -> Vec<String> {
  vec![
    browser.to_string_lossy().into_owned(),
    "--headless=new".to_owned(),
    format!("--remote-debugging-port={port}"),
    format!("--user-data-dir={}", profile.to_string_lossy()),
    "--no-first-run".to_owned(),
    "--no-default-browser-check".to_owned(),
    "--disable-extensions".to_owned(),
    window.clone(),
  ]
}

fn implement(instructions: &'static str) -> Vec<StepSpec> {
  vec![StepSpec::new("implement", "Implement", instructions, &[])]
}

/// The service `desktop`: the launcher copied into the worktree says the WebView2 variable it got, then starts
/// `browser` on the fixture with its debugging port on the service's own; ready once the browser listens; the `cdp`
/// driver.
fn desktop_service(browser: &Path, fixture: &(String, PathBuf)) -> Service {
  let ready = ReadyCheck::LogLine(LogPattern::new(LISTENING).expect("a pattern"));
  let readiness = Readiness::new(ready, ReadyTimeout::from_secs(60).expect("in range"));
  let arguments = ["env", WEBVIEW2_ARGUMENTS, "launch"]
    .map(str::to_owned)
    .into_iter()
    .chain(browser_arguments(browser, "{{port}}", fixture))
    .map(|argument| CommandArg::new(&argument).expect("an argument"))
    .collect();
  let launcher = format!("fixture/orchester-test-command{}", std::env::consts::EXE_SUFFIX);
  Service::new(
    ServiceName::new("desktop").expect("a name"),
    CommandProgram::new(&launcher).expect("a program"),
    readiness,
  )
  .with_arguments(arguments)
  .with_ui_driver(UiDriver::Cdp)
}

/// A run of `instructions` against `services`: its reported calls, its id, and its project, which holds its storage
/// while it is kept.
struct AppRun {
  answered: Vec<String>,
  id: RunId,
  project: TestProject,
}

async fn run_app(instructions: &'static str, services: &[Service]) -> AppRun {
  let project = service_project(implement(instructions), Vec::new());
  let root = project.repo.path();
  publish(root, services);
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  AppRun {
    answered: calls(root, id),
    id,
    project,
  }
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

#[tokio::test(flavor = "multi_thread")]
async fn cdp_ui_drives_an_electron_style_window_through_its_debugging_port() {
  let Some(browser) = browser() else {
    return;
  };
  let data = TempDir::new().expect("temporary folder");
  let fixture = fixture(data.path());
  let app = run_app(
    "Copy the launcher fixture. Start the service desktop. Then drive the app window.",
    &[desktop_service(&browser, &fixture)],
  )
  .await;
  let answered = &app.answered;
  let port = started_port(
    answered
      .iter()
      .find(|call| call.starts_with("start_service ok"))
      .expect("the service started"),
  );

  let lines = reported(
    &ProjectDb::open(app.project.repo.path()).expect("open"),
    app.id,
    "orchester-answer| ",
  );
  let given = format!("out| {WEBVIEW2_ARGUMENTS}=--remote-debugging-port={port}");
  assert!(
    lines.iter().any(|line| line.ends_with(&given)),
    "the service's process got its port for a WebView2 app: {lines:#?}"
  );
  // The window may still be loading as ui_open attaches: its title and address are what the browser has then.
  let opened = answer(answered, "ui_open", 0);
  assert_eq!(opened["driver"], "cdp", "{opened}");
  assert_eq!(opened["service"], "desktop");
  assert_eq!(opened["port"], port);
  assert!(opened["pid"].is_u64(), "the pid listening on the port: {opened}");
  assert!(opened["window"].is_string() && opened["title"].is_string(), "{opened}");
  let waited = answer(answered, "ui_wait", 0);
  assert_eq!(waited["met"], "url");
  assert!(
    waited["url"].as_str().is_some_and(|url| url.ends_with("/window.html")),
    "{waited}"
  );
  assert_eq!(answer(answered, "ui_wait", 1)["met"], "idle");
  let tree = &answer(answered, "ui_snapshot", 0)["tree"];
  assert_eq!(tree["role"], "document", "{tree}");
  assert_eq!(tree["name"], "Fixture desktop app");
  let field = element(tree, "textbox", "Title").expect("the field");
  assert!(
    field["ref"]
      .as_str()
      .is_some_and(|reference| reference.starts_with('b')),
    "{field}"
  );
  assert!(element(tree, "button", "Save").is_some(), "{tree}");
  let actions: Vec<Value> = (0..2)
    .map(|index| answer(answered, "ui_act", index)["acted"].clone())
    .collect();
  assert_eq!(actions, ["type", "click"]);
  assert_eq!(answer(answered, "ui_wait", 2)["met"], "text");
  assert_eq!(answer(answered, "ui_wait", 3)["met"], "element");
  let shot = answer(answered, "ui_screenshot", 0);
  let path = shot["screenshot"].as_str().expect("a path");
  assert!(
    path.contains("/.orchester/runs/") && path.contains("/screenshots/") && path.ends_with(".png"),
    "saved in the run's own storage: {path}"
  );
  image::open(Path::new(path)).expect("a PNG");
  let logged = answer(answered, "ui_logs", 0);
  let console: Vec<&str> = logged["lines"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .collect();
  assert!(console.contains(&"console log: saved Fix login"), "{console:#?}");
  assert!(
    console.contains(&"console error: save logged an error on purpose"),
    "{console:#?}"
  );
}

/// The service `app` listens on nothing; `holder`, which needs it, starts `browser` with its debugging port on `app`'s
/// port. A process of `holder`'s tree, not `app`'s, listens there, so `ui_open("app")` is refused.
#[tokio::test(flavor = "multi_thread")]
async fn cdp_ui_refuses_a_debugging_port_another_tree_listens_on() {
  let Some(browser) = browser() else {
    return;
  };
  let data = TempDir::new().expect("temporary folder");
  let fixture = fixture(data.path());
  let ready = ReadyCheck::LogLine(LogPattern::new("^ready$").expect("a pattern"));
  let app =
    service("app", &["say", "ready", "sleep", "600"].map(str::to_owned), ready, 20).with_ui_driver(UiDriver::Cdp);
  let launched = ["launch".to_owned()]
    .into_iter()
    .chain(browser_arguments(&browser, "{{services.app.port}}", &fixture))
    .collect::<Vec<_>>();
  let listening = ReadyCheck::LogLine(LogPattern::new(LISTENING).expect("a pattern"));
  let holder = service("holder", &launched, listening, 60)
    .with_needs(vec![ServiceName::new("app").expect("a name")])
    .expect("needs");
  let run = run_app("Start the service holder. Then open the UI of app.", &[app, holder]).await;
  let refused = error(&run.answered, "ui_open", 0);
  assert!(
    refused.contains("refused: pid ")
      && refused.contains("of the service `app`")
      && refused.contains("not a process of the service's own tree"),
    "{refused}"
  );
}
