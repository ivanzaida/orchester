//! The browser tools end to end through the invocation's relay (`orchester-fake-codex`, `orchester-fake-claude`, their
//! script in `support/fake_browser_tools.rs`), with the real browser on the pages of [`super::site`]: a read-only step
//! whose profile has *Browser* on reads, types, clicks, waits, scrolls, takes screenshots under `.tmp/browser/`, reads
//! the console and the network, goes back in history and closes its tab, on its own tabs only; `browser_eval` asks
//! even on a local page, an allowed site opens without asking and any other site asks. A step without *Browser* is
//! offered none of them.

use std::{fs, path::Path};

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  orchester_tools::browser::PAGE_DATA_NOTICE,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess,
    browser::{AllowedSite, AllowedSites, BrowserAccess, BrowserPermission},
  },
  run::{Answerer, ApprovalAnswer, RunStatus},
};
use orchester_store::ProjectDb;

use super::site;
use crate::{
  browser_installed,
  support::{Setup, TestProject, pending_approval, project, reported, scheduler, wait_for},
};

/// A read-only profile on `provider` whose *Browser* is `access`, allowing `docs.example.invalid`.
fn browsing(provider: AgentProvider, access: BrowserAccess) -> TestProject {
  let commands = match provider {
    AgentProvider::CodexCli => CommandPermission::AnyInWorktree,
    _ => CommandPermission::Allowed(AllowedCommands::default()),
  };
  let mut permissions = AgentPermissions::new(RepositoryAccess::ReadOnly, commands, NetworkAccess::On);
  let docs = AllowedSite::new("docs.example.invalid").expect("a site");
  permissions.set_browser(BrowserPermission::new(
    access,
    AllowedSites::new(vec![docs]).expect("sites"),
  ));
  project(&Setup {
    provider,
    permissions: Some(permissions),
    ..Setup::new("Use the browser tools.")
  })
}

/// The run's browser calls, `<tool> <ok|error>[ +image]: <first line>`, in order.
fn calls(root: &Path, run: RunId) -> Vec<String> {
  reported(&ProjectDb::open(root).expect("open"), run, "browser-call: ")
}

/// The lines of the answers of the run's calls of `tool`.
fn answers(root: &Path, run: RunId, tool: &str) -> Vec<String> {
  let prefix = format!("browser-answer: {tool} | ");
  reported(&ProjectDb::open(root).expect("open"), run, &prefix)
}

/// The call of `tool` at `index` among the run's calls of it.
fn call_of(calls: &[String], tool: &str, index: usize) -> String {
  calls
    .iter()
    .filter(|call| call.starts_with(&format!("{tool} ")))
    .nth(index)
    .cloned()
    .unwrap_or_else(|| panic!("no call {index} of {tool}: {calls:?}"))
}

/// Runs the fake's browser script in `project`, with another run's tab open, answering its two asks: `browser_eval`
/// approved, the site that is not allowed denied. The run.
async fn run_script(project: &TestProject) -> RunId {
  let root = project.repo.path();
  let profile = tempfile::tempdir().expect("temporary folder");
  let (scheduler, mut events) = scheduler(root);
  let scheduler = scheduler.with_browser_profile(profile.path().to_path_buf());
  let site = site::serve().await;
  let other = scheduler
    .browser()
    .open_tab(RunId::generate(), &format!("{site}other"))
    .await
    .expect("another run's tab");
  fs::write(root.join(".orchester").join("fake-browser-site"), &site).expect("site");
  fs::write(root.join(".orchester").join("fake-browser-other-tab"), other.as_str()).expect("tab");
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");

  let eval = pending_approval(root, &mut events, id).await;
  assert_eq!(
    (eval.action.as_str(), eval.target.as_deref()),
    ("browser_eval", Some("127.0.0.1")),
    "browser_eval asks on a local page too; {:?}",
    calls(root, id)
  );
  scheduler
    .decide_tool_approval(id, eval.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  let elsewhere = pending_approval(root, &mut events, id).await;
  assert_eq!(
    (elsewhere.action.as_str(), elsewhere.target.as_deref()),
    ("browser_open", Some("elsewhere.example.invalid")),
    "the allowed site opened without asking, the other one asks"
  );
  scheduler
    .decide_tool_approval(id, elsewhere.id, ApprovalAnswer::Deny, Answerer::Person)
    .await
    .expect("denied");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded, "{:?}", calls(root, id));
  scheduler.browser().stop().await;
  id
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_tools_work_on_the_runs_own_tabs_and_ask_where_the_policy_says() {
  if !browser_installed() {
    return;
  }
  let project = browsing(AgentProvider::CodexCli, BrowserAccess::On);
  let root = project.repo.path();
  let id = run_script(&project).await;
  let calls = calls(root, id);
  let listed = reported(&ProjectDb::open(root).expect("open"), id, "orchester-tools: ");
  assert!(
    listed[0].contains("browser_open") && listed[0].contains("browser_eval"),
    "{listed:?}"
  );

  assert!(
    call_of(&calls, "browser_tabs", 0).contains("ok: This run has no open tab"),
    "{calls:?}"
  );
  assert!(
    call_of(&calls, "browser_read_page", 0).contains("error: tab ")
      && call_of(&calls, "browser_read_page", 0).contains("is not one of this run's open tabs"),
    "another run's tab is out of reach: {calls:?}"
  );
  assert!(
    call_of(&calls, "browser_click", 0).contains("error: this run has no open tab"),
    "{calls:?}"
  );
  assert_eq!(
    call_of(&calls, "browser_open", 0),
    format!("browser_open ok: {PAGE_DATA_NOTICE}")
  );
  let opened = answers(root, id, "browser_open");
  assert!(
    opened[1].starts_with("Opened in tab ") && opened[1].contains("\"Form\""),
    "{opened:?}"
  );
  acted_on_the_page(root, id, &calls);
  took_screenshots(root, id, &calls);
  read_the_logs(root, id);
  went_back_and_followed_the_policy(root, id, &calls);
}

/// Read, typed, clicked, waited, read the text, refused a stale ref, pressed a key and scrolled.
fn acted_on_the_page(root: &Path, id: RunId, calls: &[String]) {
  let page = answers(root, id, "browser_read_page");
  assert!(
    page.iter().any(|line| line.contains("- heading \"Guest book\"")),
    "{page:?}"
  );
  assert!(
    page.iter().any(|line| line.contains("- textbox \"Name\" [e")),
    "{page:?}"
  );
  assert!(
    page.iter().any(|line| line.contains("- button \"Save\" [e")),
    "{page:?}"
  );
  assert!(
    call_of(calls, "browser_type", 0).contains("ok: Typed 3 characters into e"),
    "{calls:?}"
  );
  assert!(
    call_of(calls, "browser_click", 1).contains("ok: Clicked e"),
    "{calls:?}"
  );
  assert!(
    answers(root, id, "browser_click")
      .iter()
      .any(|line| line == "The page opened a dialog: a alert dialog was dismissed: Saved Ada."),
    "the alert was dismissed and reported"
  );
  assert!(
    call_of(calls, "browser_wait", 0).contains("ok: The text \"Saved Ada\" showed"),
    "{calls:?}"
  );
  assert!(
    answers(root, id, "browser_page_text")
      .iter()
      .any(|line| line == "Saved Ada"),
    "{:?}",
    answers(root, id, "browser_page_text")
  );
  assert!(
    call_of(calls, "browser_click", 2).contains("error: e") && call_of(calls, "browser_click", 2).contains("stale"),
    "a ref of an earlier read is stale: {calls:?}"
  );
  assert_eq!(call_of(calls, "browser_key", 0), "browser_key ok: Pressed Tab.");
  assert_eq!(
    call_of(calls, "browser_scroll", 0),
    "browser_scroll ok: Scrolled the page to x 0, y 300."
  );
  assert!(
    call_of(calls, "browser_scroll", 1).contains("ok: Scrolled e"),
    "{calls:?}"
  );
  assert!(
    call_of(calls, "browser_wait", 1).contains("ok: The element e"),
    "{calls:?}"
  );
}

/// Saved a PNG of the visible page, of one element and of the full page under `.tmp/browser/` in the worktree.
fn took_screenshots(root: &Path, id: RunId, calls: &[String]) {
  let worktree = root.join(".orchester").join("worktrees").join(id.to_string());
  for (index, what) in ["visible page", "element e", "full page"].into_iter().enumerate() {
    let shot = call_of(calls, "browser_screenshot", index);
    assert!(shot.contains(&format!("ok: PNG of the {what}")), "{shot}");
    assert!(!shot.contains("+image"), "Codex is not sent images");
    let saved = shot
      .split("saved: ")
      .nth(1)
      .and_then(|rest| rest.split(". ").next())
      .unwrap_or_else(|| panic!("{shot}"));
    assert!(
      saved.starts_with(".tmp/browser/screenshot-") && saved.ends_with(".png"),
      "{shot}"
    );
    let png = fs::read(worktree.join(saved)).expect("the screenshot is saved in the worktree");
    assert!(png.starts_with(b"\x89PNG"), "a PNG");
  }
}

/// Read the console, all of it and its errors only, and the requests, without headers or bodies.
fn read_the_logs(root: &Path, id: RunId) {
  let console = answers(root, id, "browser_console");
  let errors_only = console
    .rsplit(|line| line == PAGE_DATA_NOTICE)
    .next()
    .expect("the second call's answer");
  assert!(console.iter().any(|line| line == "[log] saved Ada"), "{console:?}");
  assert!(errors_only.iter().any(|line| line == "[error] oops Ada"), "{console:?}");
  assert!(
    !errors_only.iter().any(|line| line.starts_with("[log]")),
    "only errors: {errors_only:?}"
  );
  let network = answers(root, id, "browser_network");
  assert!(
    network
      .iter()
      .any(|line| line.starts_with("GET 200 Fetch") && line.ends_with("/api/save?name=Ada")),
    "{network:?}"
  );
}

/// Went back in history, ran out of time waiting, evaluated once approved, opened the allowed site unasked, was
/// denied another one, closed its tab and never saw another run's.
fn went_back_and_followed_the_policy(root: &Path, id: RunId, calls: &[String]) {
  assert!(call_of(calls, "browser_navigate", 0).contains("ok:"), "{calls:?}");
  assert!(
    answers(root, id, "browser_navigate")
      .iter()
      .any(|line| line.contains("shows \"Form\"")),
    "back from the later page"
  );
  assert_eq!(
    call_of(calls, "browser_wait", 2),
    "browser_wait error: The text \"never on the page\" did not appear within 1 s"
  );
  assert!(
    answers(root, id, "browser_eval").iter().any(|line| line == "\"Form\""),
    "{calls:?}"
  );
  assert!(
    call_of(calls, "browser_open", 2).contains("error: http://docs.example.invalid/ did not open"),
    "the allowed site ran without asking: {calls:?}"
  );
  assert!(
    call_of(calls, "browser_open", 3).starts_with("browser_open error"),
    "denied: {calls:?}"
  );
  assert!(
    call_of(calls, "browser_close", 0).contains("ok: Closed tab "),
    "{calls:?}"
  );
  let tabs = answers(root, id, "browser_tabs");
  assert!(
    tabs.iter().any(|line| line.contains("\"Form\"")),
    "the first tab stays: {tabs:?}"
  );
  assert!(
    !tabs.iter().any(|line| line.contains("Page other")),
    "never another run's tab: {tabs:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_tools_reach_claude_code_by_allow_rules_and_screenshots_come_back_as_images() {
  if !browser_installed() {
    return;
  }
  let project = browsing(AgentProvider::ClaudeCodeCli, BrowserAccess::On);
  let root = project.repo.path();
  let id = run_script(&project).await;
  let permissions = reported(&ProjectDb::open(root).expect("open"), id, "browser-permission: ");
  assert!(!permissions.is_empty());
  assert!(
    permissions.iter().all(|line| line.ends_with(" allow-by-rule")),
    "the runner asks about a browser call itself: {permissions:?}"
  );
  let shot = call_of(&calls(root, id), "browser_screenshot", 0);
  assert!(
    shot.starts_with("browser_screenshot ok +image: PNG of the visible page"),
    "{shot}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_tools_are_not_offered_to_a_step_without_the_browser() {
  let project = browsing(AgentProvider::CodexCli, BrowserAccess::Off);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let listed = reported(&ProjectDb::open(root).expect("open"), id, "orchester-tools: ");
  assert!(!listed.is_empty() && !listed[0].contains("browser_"), "{listed:?}");
  assert_eq!(
    calls(root, id),
    [
      "browser_tabs error: `browser_tabs` is not available in this step: its profile's Browser permission is off, or \
      the step has no network access"
    ]
  );
}
