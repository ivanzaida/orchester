//! The browser tools' second script (`support/fake_browser_tools.rs`, `read_closely_and_act_elsewhere`), with the real
//! browser on the pages of [`super::site`]: `browser_read_page` filtered by its query, cut to the elements an agent can
//! act on and cut at `max_chars`, saying so; `browser_page_text` cut too; a reload, and back then forward; a
//! `confirm()` dismissed and reported as an `alert()` is; a thrown exception listed by `browser_console`. Then the same
//! form at `[::ffff:127.0.0.1]`, a host that is neither local nor allowed: opening and acting there ask, and Approve
//! similar on a click lets the next click on that host run without asking, while typing there still asks.

use std::{fs, path::Path};

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  orchester_tools::browser::PAGE_DATA_NOTICE,
  permission::browser::BrowserAccess,
  run::{Answerer, ApprovalAnswer, RunStatus},
};

use super::{
  site,
  tools::{answers, browsing, call_of, calls},
};
use crate::{
  browser_installed,
  support::{pending_approval, scheduler, wait_for},
};

/// The host the form is served at the second time, as the approvals name it: an IPv4-mapped IPv6 address of
/// loopback, which reaches the test's pages but is no local development host.
const ELSEWHERE: &str = "[::ffff:127.0.0.1]";

/// The answers of the run's calls of `tool`, in order, each without the notice it starts with; for the tools whose
/// every answer starts with [`PAGE_DATA_NOTICE`] and opens no dialog.
fn page_answers(root: &Path, run: RunId, tool: &str) -> Vec<Vec<String>> {
  let mut each: Vec<Vec<String>> = Vec::new();
  for line in answers(root, run, tool) {
    if line == PAGE_DATA_NOTICE {
      each.push(Vec::new());
    } else if let Some(answer) = each.last_mut() {
      answer.push(line);
    }
  }
  each
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_tools_read_pages_closely_and_ask_before_acting_on_a_site_not_allowed() {
  if !browser_installed() {
    return;
  }
  let project = browsing(AgentProvider::CodexCli, BrowserAccess::On);
  let root = project.repo.path();
  let profile = tempfile::tempdir().expect("temporary folder");
  let (scheduler, mut events) = scheduler(root);
  let scheduler = scheduler.with_browser_profile(profile.path().to_path_buf());
  let site = site::serve().await;
  let elsewhere = site.replace("127.0.0.1", ELSEWHERE);
  fs::write(root.join(".orchester").join("fake-browser-site"), &site).expect("site");
  fs::write(root.join(".orchester").join("fake-browser-elsewhere"), &elsewhere).expect("elsewhere");
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");

  for (tool, answer) in [
    ("browser_open", ApprovalAnswer::Once),
    ("browser_click", ApprovalAnswer::Similar),
    ("browser_type", ApprovalAnswer::Deny),
  ] {
    let asked = pending_approval(root, &mut events, id).await;
    assert_eq!(
      (asked.action.as_str(), asked.target.as_deref()),
      (tool, Some(ELSEWHERE)),
      "only these ask, in this order: {:?}",
      calls(root, id)
    );
    scheduler
      .decide_tool_approval(id, asked.id, answer, Answerer::Person)
      .await
      .expect("answered");
  }
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded, "{:?}", calls(root, id));
  scheduler.browser().stop().await;

  let calls = calls(root, id);
  read_the_form_closely(root, id);
  went_through_history(root, id, &calls);
  dismissed_the_confirm_and_listed_the_exception(root, id, &calls);
  acted_elsewhere_as_approved(&calls);
}

/// The query kept only the lines naming it, in any case; `interactive_only` only the field and the button, with refs;
/// `max_chars` cut the tree and the text, and each answer said what was left out and how to read more.
fn read_the_form_closely(root: &Path, id: RunId) {
  let reads = page_answers(root, id, "browser_read_page");
  let queried = &reads[0][1..];
  assert!(
    queried.iter().any(|line| line.starts_with("- button \"Save\" [e")),
    "{queried:?}"
  );
  assert!(queried.iter().any(|line| line == "- text \"Not saved\""), "{queried:?}");
  assert!(
    queried.iter().all(|line| line.to_lowercase().contains("save")),
    "only what the query names: {queried:?}"
  );

  let cut = &reads[1][1..];
  let (notice, shown) = cut.split_last().expect("a cut tree");
  assert!(
    notice.starts_with("[cut: ")
      && notice.ends_with(" more characters were left out; read more with query or interactive_only]"),
    "{cut:?}"
  );
  let kept: usize = shown.iter().map(|line| line.chars().count() + 1).sum();
  assert!(kept > 0 && kept <= 31, "at most 30 characters of the tree: {cut:?}");

  let interactive = &reads[2][1..];
  assert_eq!(interactive.len(), 2, "{interactive:?}");
  assert!(interactive[0].starts_with("- textbox \"Name\" [e"), "{interactive:?}");
  assert!(interactive[1].starts_with("- button \"Save\" [e"), "{interactive:?}");

  let texts = page_answers(root, id, "browser_page_text");
  let short = &texts[0][1..];
  assert_eq!(short.len(), 2, "{short:?}");
  assert_eq!(short[0], "Guest book");
  assert!(
    short[1].starts_with("[cut: ")
      && short[1].ends_with(" more characters were left out; read more with a larger max_chars]"),
    "{short:?}"
  );
}

/// The reload showed the form as it was before it was saved; back went to the form, forward to the later page again.
fn went_through_history(root: &Path, id: RunId, calls: &[String]) {
  assert!(
    call_of(calls, "browser_click", 0).contains("ok: Clicked e"),
    "{calls:?}"
  );
  let after_reload = &page_answers(root, id, "browser_page_text")[1];
  assert!(after_reload.iter().any(|line| line == "Not saved"), "{after_reload:?}");
  assert!(
    !after_reload.iter().any(|line| line.contains("Saved Ada")),
    "{after_reload:?}"
  );
  let shown: Vec<String> = answers(root, id, "browser_navigate")
    .into_iter()
    .filter_map(|line| {
      let (_, rest) = line.split_once(" shows ")?;
      let (title, url) = rest.rsplit_once(' ')?;
      Some(format!("{title} {}", url.rsplit('/').next()?))
    })
    .collect();
  assert_eq!(
    shown,
    ["\"Form\" form", "\"Form\" form", "\"Later\" later"],
    "reload, back, forward"
  );
}

/// The `confirm()` was dismissed, so the page wrote that nothing was deleted, and the click's answer said so; the error
/// it then threw is listed among the console's errors.
fn dismissed_the_confirm_and_listed_the_exception(root: &Path, id: RunId, calls: &[String]) {
  assert!(
    call_of(calls, "browser_click", 1).contains("ok: Clicked e"),
    "{calls:?}"
  );
  assert!(
    answers(root, id, "browser_click")
      .iter()
      .any(|line| line == "The page opened a dialog: a confirm dialog was dismissed: Delete Ada?."),
    "{:?}",
    answers(root, id, "browser_click")
  );
  let after = &page_answers(root, id, "browser_page_text")[2];
  assert!(after.iter().any(|line| line == "Kept"), "{after:?}");
  let console = answers(root, id, "browser_console");
  assert!(
    console
      .iter()
      .any(|line| line == "[exception] Error: boom after the question"),
    "{console:?}"
  );
}

/// Opening the form elsewhere ran once approved; the click approved with Approve similar ran, and so did the next
/// click there without asking; typing there was denied.
fn acted_elsewhere_as_approved(calls: &[String]) {
  let opened = call_of(calls, "browser_open", 3);
  assert!(opened.contains("ok:"), "{opened}");
  for index in [2, 3] {
    assert!(
      call_of(calls, "browser_click", index).contains("ok: Clicked e"),
      "{calls:?}"
    );
  }
  assert!(
    call_of(calls, "browser_type", 1).starts_with("browser_type error"),
    "denied: {calls:?}"
  );
}
