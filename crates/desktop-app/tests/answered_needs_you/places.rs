//! Each place an answer is given, with the real runner and the fake Codex: the item, one of the count and one of the
//! pill leave in the press's frame, and the runner's record lets them go for good.

use orchester_app::routes::{OverviewTab, ProjectPage, Route, TaskTab};
use orchester_core::run::{Answerer, ApprovalAnswer};
use orchester_ipc::Command;
use orchester_workspace::tasks::needs_you::{WaitingItem, WaitingKind};

use super::{Page, ids_in, is_approval, is_choice, is_decision, is_plan};

const APPROVAL: &str = "Add serde to the webhook payloads";
const CHOICE: &str = "Pick where the docs button goes";
const PLAN: &str = "Rate-limit messages from GitHub";
const FAILS: &str = "Export invoices as PDF";

/// Opens the panel and expands the item of the task titled `title` around its card; the card's id.
pub(super) fn expand_in_panel(page: &mut Page, title: &str, prefix: &str) -> String {
  page.click("needs-you-pill");
  page.wait("the panel", |window| window.texts("needs-you-panel").is_some());
  let items = page.ids("needs-you-panel", "needs-you-item-");
  let item = items
    .into_iter()
    .find(|id| {
      page
        .window
        .texts(&format!("{id}-task"))
        .is_some_and(|texts| texts.join(" ").ends_with(title))
    })
    .unwrap_or_else(|| panic!("an item of {title}"));
  page.click(&format!("{item}-action"));
  page.wait("the card in place", |window| {
    window.tree.get_element_by_id(&item).is_some_and(|element| {
      let mut found = Vec::new();
      ids_in(element, prefix, &mut found);
      !found.is_empty()
    })
  });
  page.ids(&item, prefix).remove(0)
}

fn panel_in_place(away: bool) {
  let mut page = Page::live();
  page.start(APPROVAL);
  let item = page.wait_for("the agent's request", is_approval);
  if away {
    page.away();
  }
  let card = expand_in_panel(&mut page, APPROVAL, "cards-approval-");
  let approve = format!("{card}-approve");
  page.wait("Approve once", |window| window.tree.get_element_by_id(&approve).is_some());
  let label = if away { "panel in place, another tab" } else { "panel in place" };
  let answered = page.answer(label, &item, |page| page.click(&approve));
  assert!(answered.at_once, "the item leaves in the press's frame");
  page.end();
}

#[test]
fn answered_in_the_panel_leaves_at_once() {
  panel_in_place(false);
}

#[test]
fn answered_in_the_panel_for_another_tab_leaves_at_once() {
  panel_in_place(true);
}

#[test]
fn answered_on_the_conversation_card_leaves_at_once() {
  let mut page = Page::live();
  page.start(CHOICE);
  let item = page.wait_for("the agent's question", is_choice);
  page.go(CHOICE, TaskTab::Conversation);
  let mut card = Vec::new();
  page.wait("the card's options", |window| {
    card.clear();
    if let Some(chat) = window.tree.get_element_by_id("conversation-chat") {
      ids_in(chat, "cards-choice-", &mut card);
    }
    card.iter().any(|id| id.ends_with("-option-0-1"))
  });
  card.sort_by_key(String::len);
  let card = card.remove(0);
  page.click(&format!("{card}-option-0-1"));
  page.window.pass();
  let answered = page.answer("conversation card", &item, |page| page.click(&format!("{card}-send")));
  assert!(answered.at_once, "the item leaves in the press's frame");
  page.end();
}

#[test]
fn decided_on_the_summary_leaves_at_once() {
  let mut page = Page::live();
  page.start(PLAN);
  let item = page.wait_for("the plan", is_plan);
  page.wait("Approve plan", |window| window.tree.get_element_by_id("plan-approve").is_some());
  let answered = page.answer("summary decision bar (plan)", &item, |page| page.click("plan-approve"));
  assert!(answered.at_once, "the plan leaves in the press's frame");
  page.end();
}

#[test]
fn decided_after_retries_on_the_summary_leaves_at_once() {
  let mut page = Page::live();
  page.start(FAILS);
  let item = page.wait_for("the decision after retries", is_decision);
  page.wait("Stop the run", |window| window.tree.get_element_by_id("task-stop-run").is_some());
  let answered = page.answer("summary decision bar (stuck)", &item, |page| page.click("task-stop-run"));
  assert!(answered.at_once, "the decision leaves in the press's frame");
  page.end();
}

#[test]
fn answered_in_the_composer_leaves_at_once() {
  let mut page = Page::live();
  page.start(PLAN);
  let item = page.wait_for("the plan", is_plan);
  page.go(PLAN, TaskTab::Conversation);
  page.wait("the composer on the plan", |window| {
    window.tree.get_element_by_id("composer-input").is_some() && window.texts("composer-context").is_some()
  });
  page.window.set("composer-input", "Split the limiter into its own module.");
  let answered = page.answer("composer (plan's changes)", &item, |page| page.click("composer-send-button"));
  assert!(answered.at_once, "the plan leaves in the press's frame");
  page.end();
}

/// The Orchestrator chat's tools decide as `decide_approval` does: through the project's runner, as automation.
fn decide_as_the_orchestrator(page: &Page, item: &WaitingItem) {
  let WaitingKind::Approval(shown) = &item.kind else {
    panic!("an approval");
  };
  let command = Command::DecideToolApproval {
    run: shown.approval.run,
    approval: shown.approval.id.clone(),
    answer: ApprovalAnswer::Once,
    by: Answerer::Automation,
  };
  let (services, open) = (
    page.window.launch.services().expect("the session started").clone(),
    page.open_project(),
  );
  let runners = services.runners.clone();
  services.runtime.spawn(async move {
    if let Err(error) = runners.control_run_in(&open, command).await {
      panic!("the runner did not take the decision: {error}");
    }
  });
}

/// The Orchestrator answers while the Overview shows the project, or while another tab is the active one. Its tools
/// answer off the window's thread, so the item leaves once their mark reaches the feed, not in a press's frame.
fn orchestrator(away: bool) {
  let mut page = Page::live();
  page.start(APPROVAL);
  let item = page.wait_for("the agent's request", is_approval);
  if away {
    page.away();
  } else {
    let overview = Route::Project(page.project, ProjectPage::Overview(OverviewTab::default()));
    page.window.navigation().go(overview);
    page.wait("the Overview's Needs you", |window| {
      window.texts("overview-needs-count").is_some()
    });
  }
  let label = if away {
    "Orchestrator chat, another tab"
  } else {
    "Orchestrator chat, Overview card"
  };
  page.answer(label, &item, |page| decide_as_the_orchestrator(page, &item));
  if !away {
    assert_eq!(
      page.window.text("overview-needs-count"),
      page.count().to_string(),
      "the Overview counts what every Needs you counts"
    );
  }
  page.end();
}

#[test]
fn decided_by_the_orchestrator_leaves_the_overview() {
  orchestrator(false);
}

#[test]
fn decided_by_the_orchestrator_for_another_tab_leaves() {
  orchestrator(true);
}
