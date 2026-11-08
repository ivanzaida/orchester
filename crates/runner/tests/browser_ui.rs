//! The `browser` UI driver end to end against `orchester-fake-codex`, with `orchester-test-command`'s stand-in web
//! server as the service and its fixture page (`support/fake_server.rs`, `support/fake_web_tools.rs`), in the real
//! Chrome, Edge, Brave or Chromium of this computer as the project's browser: `ui_open` opens the service's page where
//! its `ui_start` says, in a tab of the run's own; the snapshot is the page's accessibility tree with refs of DOM
//! nodes; typing, choosing, a key and a click go by ref; `ui_wait` waits for an address, a text, a selector and idle;
//! screenshots are saved in the run's storage; `ui_logs` reads the console error and the failed request; requests and
//! a navigation to another origin are blocked and reported; a ref of no element says so; and the tab is gone once the
//! step ended. Where no browser is found that test says so and checks nothing. A browser that cannot be used answers
//! `ui_open` with what to install. Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test
//! browser_ui browser_ui`.

mod support;

use std::path::Path;

use orchester_core::{
  id::{IdempotencyKey, RunId},
  run::RunStatus,
  service::{ReadyCheck, Service, UiDriver, UiStart},
};
use orchester_ipc::browser::ComputerBrowser;
use orchester_runner::execution::Scheduler;
use serde_json::Value;

use self::support::{
  StepSpec, TestProject,
  qa::{approve, calls},
  scheduler,
  services::{publish, service, service_project, started_port},
  wait_for,
};

/// Whether this computer has a browser; where it has none, the test says so and checks nothing.
#[allow(
  clippy::print_stderr,
  reason = "says that nothing was checked where no browser is installed"
)]
fn browser_installed() -> bool {
  match orchester_exec::browser::find() {
    Ok(_) => true,
    Err(unavailable) => {
      eprintln!("skipped, no browser to test with: {unavailable}");
      false
    }
  }
}

fn implement(instructions: &'static str) -> Vec<StepSpec> {
  vec![StepSpec::new("implement", "Implement", instructions, &[])]
}

/// The stand-in web server as the service `web`, ready once its port opens, driven by the browser driver from its
/// fixture page.
fn web_service() -> Service {
  let arguments = ["listen".to_owned(), "{{port}}".to_owned()];
  service("web", &arguments, ReadyCheck::PortOpen, 20)
    .with_ui_driver(UiDriver::Browser)
    .with_ui_start(UiStart::new("{{url}}/app").expect("a place"))
}

/// A run of the published web service: its reported calls, its scheduler, its id, and its project, which holds its
/// storage while it is kept.
struct WebRun {
  answered: Vec<String>,
  scheduler: Scheduler,
  id: RunId,
  _project: TestProject,
}

/// Runs `instructions` against the published web service with the scheduler `prepare` gives.
async fn run_web(instructions: &'static str, prepare: impl FnOnce(Scheduler) -> Scheduler) -> WebRun {
  let project = service_project(implement(instructions), Vec::new());
  let root = project.repo.path();
  publish(root, &[web_service()]);
  let (scheduler, mut events) = scheduler(root);
  let scheduler = prepare(scheduler);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  WebRun {
    answered: calls(root, id),
    scheduler,
    id,
    _project: project,
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

/// The answer of the last call of `tool` that succeeded.
fn last(answered: &[String], tool: &str) -> Value {
  let prefix = format!("{tool} ok: ");
  let line = answered
    .iter()
    .rev()
    .find_map(|call| call.strip_prefix(&prefix))
    .unwrap_or_else(|| panic!("no answer of {tool}: {answered:#?}"));
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

/// The texts of `answer`'s list `field`.
fn texts<'a>(answer: &'a Value, field: &str) -> Vec<&'a str> {
  answer[field]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .collect()
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
async fn browser_ui_drives_the_fixture_page_by_refs_and_blocks_other_origins() {
  if !browser_installed() {
    return;
  }
  let profile = tempfile::tempdir().expect("temporary folder");
  let folder = profile.path().to_path_buf();
  let web = run_web("Start the service web. Then drive the web page.", |scheduler| {
    scheduler.with_browser_profile(folder)
  })
  .await;
  let answered = &web.answered;
  let port = started_port(
    answered
      .iter()
      .find(|call| call.starts_with("start_service ok"))
      .expect("the service started"),
  );

  opened(&answer(answered, "ui_open", 0), port);
  assert_eq!(answer(answered, "ui_wait", 0)["met"], "url");
  snapshot(&answer(answered, "ui_snapshot", 0)["tree"]);
  let found = answer(answered, "ui_snapshot", 1);
  assert_eq!(found["matches"][0]["role"], "button", "{found}");
  assert_eq!(found["matches"][0]["name"], "Save", "{found}");
  let actions: Vec<Value> = (0..5)
    .map(|index| answer(answered, "ui_act", index)["acted"].clone())
    .collect();
  assert_eq!(actions, ["type", "select", "key", "click", "click"]);
  assert_eq!(answer(answered, "ui_wait", 1)["met"], "text");
  assert_eq!(answer(answered, "ui_wait", 2)["met"], "selector");
  assert_eq!(answer(answered, "ui_wait", 3)["met"], "idle");
  screenshots(
    &answer(answered, "ui_screenshot", 0),
    &answer(answered, "ui_screenshot", 1),
  );
  logs(&answer(answered, "ui_logs", 0));
  let away = texts(&last(answered, "ui_logs"), "lines").join("\n");
  assert!(
    away.contains("blocked GET https://example.com/away (Document)"),
    "the navigation to another origin is blocked and reported: {away}"
  );
  let gone = error(answered, "ui_act", 0);
  assert!(gone.contains("b999999 is no longer in the page"), "{gone}");
  let tabs = web.scheduler.browser().tabs(web.id).await.expect("tabs");
  assert!(tabs.is_empty(), "the tab is gone once its step ended: {tabs:?}");
  web.scheduler.browser().stop().await;
}

/// `ui_open`: the service's page where its `ui_start` says, in a tab of the run's own that opens only the service's
/// origin; the image of another origin was blocked as the page loaded.
fn opened(opened: &Value, port: u16) {
  assert_eq!(opened["driver"], "browser", "{opened}");
  assert_eq!(opened["service"], "web");
  assert_eq!(opened["url"], format!("http://127.0.0.1:{port}/app"));
  assert_eq!(opened["title"], "Fixture app");
  assert!(opened["tab"].is_string(), "{opened}");
  assert_eq!(texts(opened, "allowed_origins"), [format!("http://127.0.0.1:{port}")]);
  let blocked = texts(opened, "blocked");
  assert!(
    blocked
      .iter()
      .any(|line| line.starts_with("blocked GET https://example.com/pixel.png (Image)")),
    "the image of another origin is blocked and said: {opened}"
  );
}

/// The page's tree: the document, its field, list, button and link, each by a ref of its DOM node.
fn snapshot(tree: &Value) {
  assert_eq!(tree["role"], "document", "{tree}");
  assert_eq!(tree["name"], "Fixture app");
  let field = element(tree, "textbox", "Title").expect("the field");
  assert!(
    field["ref"]
      .as_str()
      .is_some_and(|reference| reference.starts_with('b')),
    "{field}"
  );
  assert!(element(tree, "combobox", "Priority").is_some(), "{tree}");
  assert!(element(tree, "button", "Save").is_some(), "{tree}");
  assert!(element(tree, "link", "Elsewhere").is_some(), "{tree}");
  assert!(element(tree, "text", "Status: idle").is_some(), "{tree}");
}

/// The screenshots: PNGs in the run's storage, the page's and Save's, which is smaller.
fn screenshots(page: &Value, save: &Value) {
  let size = |shot: &Value| {
    let path = shot["screenshot"].as_str().expect("a path");
    assert!(
      path.contains("/.orchester/runs/") && path.contains("/screenshots/") && path.ends_with(".png"),
      "saved in the run's own storage: {path}"
    );
    let image = image::open(Path::new(path)).expect("a PNG");
    (image.width(), image.height())
  };
  let (page, save) = (size(page), size(save));
  assert!(page.0 > save.0 && page.1 > save.1, "the page {page:?}, Save {save:?}");
}

/// The log after Save: its console error, its request answered 404 and the request to another origin, blocked; a
/// blocked request is said once, by the guard, not again as a failed request.
fn logs(logged: &Value) {
  let lines = texts(logged, "lines");
  let has = |part: &str| lines.iter().any(|line| line.contains(part));
  assert!(has("console error: save logged an error on purpose"), "{lines:#?}");
  assert!(has("/missing: 404"), "{lines:#?}");
  assert!(has("blocked GET https://example.com/track ("), "{lines:#?}");
  assert!(!has("ERR_BLOCKED_BY_CLIENT"), "{lines:#?}");
}

/// A browser that cannot be used — here a program chosen in Settings › Browser that is gone — answers `ui_open` with
/// why and what to install.
#[tokio::test(flavor = "multi_thread")]
async fn browser_ui_without_a_browser_says_what_to_install() {
  let data = tempfile::tempdir().expect("temporary folder");
  let (profile, gone) = (data.path().join("profile"), data.path().join("gone").join("chrome"));
  let web = run_web("Start the service web. Then open its UI.", |scheduler| {
    let scheduler = scheduler.with_browser_profile(profile);
    scheduler.browser().set_computer(ComputerBrowser {
      program: Ok(Some(gone)),
      profiles: Err("the browser has a profile folder of its own".to_owned()),
    });
    scheduler
  })
  .await;
  let refused = error(&web.answered, "ui_open", 0);
  assert!(
    refused.starts_with("browser_unavailable: the program chosen in Settings › Browser cannot be used")
      && refused.contains("install Microsoft Edge or Google Chrome"),
    "{refused}"
  );
}
