//! Each place an answer is given, with the real runner and the fake Codex: the item, one of the count and one of the
//! pill leave in the press's frame, and the runner's record lets them go for good.

use orchester_app::routes::{OverviewTab, ProjectPage, Route, TaskTab};
use orchester_core::{
  connection::stream::AgentEvent,
  question::GivenAnswer,
  run::{Answerer, DecidedBy, DenialNote},
  store::ActivityStore,
};
use orchester_ipc::Command;
use orchester_store::ProjectDb;
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
  page.wait("Approve once", |window| {
    window.tree.get_element_by_id(&approve).is_some()
  });
  let label = if away {
    "panel in place, another tab"
  } else {
    "panel in place"
  };
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

/// *Deny with a message* in the panel (ORC-29): the item leaves as the denial is sent, the runner keeps the words with
/// who denied, and the agent gets them.
#[test]
fn denied_with_a_message_in_the_panel_leaves_at_once_and_tells_the_agent() {
  const WORDS: &str = "Keep the payloads as they are.";
  let mut page = Page::live();
  page.start(APPROVAL);
  let item = page.wait_for("the agent's request", is_approval);
  let run = item.run.map(|(run, _)| run).expect("the request's run");
  let card = expand_in_panel(&mut page, APPROVAL, "cards-approval-");
  let (opener, field) = (format!("{card}-deny-message"), format!("{card}-message"));
  page.wait("Deny with a message", |window| {
    window.tree.get_element_by_id(&opener).is_some()
  });
  page.click(&opener);
  page.wait("the editor", |window| window.tree.get_element_by_id(&field).is_some());
  page
    .window
    .tree
    .get_element_by_id_mut(&field)
    .and_then(|element| element.as_text_input())
    .expect("the message field")
    .set_value(WORDS);
  page.window.pass();
  page.window.pass();
  let send = format!("{card}-message-send");
  let answered = page.answer("panel, deny with a message", &item, |page| page.click(&send));
  assert!(answered.at_once, "the item leaves in the press's frame");
  let decided = ProjectDb::open(&page.repo)
    .expect("open")
    .decided_tool_approvals(run)
    .expect("decided");
  assert!(
    decided.iter().any(|decided| {
      decided.decided_by == Some(DecidedBy::PersonDeny) && decided.note.as_ref().map(DenialNote::as_str) == Some(WORDS)
    }),
    "{decided:?}"
  );
  let repo = page.repo.clone();
  page.wait("the agent to get the words", |_| {
    ProjectDb::open(&repo)
      .expect("open")
      .activity(run, 0)
      .expect("activity")
      .iter()
      .any(|entry| matches!(&entry.event, AgentEvent::Raw { line, .. } if line.starts_with("steered: ") && line.ends_with(WORDS)))
  });
  page.end();
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
  // The fake asks two questions; each takes an option.
  for option in ["0-1", "1-0"] {
    page.click(&format!("{card}-option-{option}"));
    page.window.pass();
  }
  let answered = page.answer("conversation card", &item, |page| page.click(&format!("{card}-send")));
  assert!(answered.at_once, "the item leaves in the press's frame");
  page.end();
}

#[test]
fn answered_on_the_summary_plan_leaves_at_once() {
  let mut page = Page::live();
  page.start(PLAN);
  let item = page.wait_for("the plan", is_plan);
  page.wait("Approve plan", |window| {
    window.tree.get_element_by_id("plan-approve").is_some()
  });
  let answered = page.answer("summary decision bar (plan)", &item, |page| page.click("plan-approve"));
  assert!(answered.at_once, "the plan leaves in the press's frame");
  page.end();
}

#[test]
fn answered_after_retries_on_the_summary_leaves_at_once() {
  let mut page = Page::live();
  page.start(FAILS);
  let item = page.wait_for("the decision after retries", is_decision);
  page.wait("Stop the run", |window| {
    window.tree.get_element_by_id("task-stop-run").is_some()
  });
  let answered = page.answer("summary decision bar (stuck)", &item, |page| {
    page.click("task-stop-run")
  });
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
  page
    .window
    .set("composer-input", "Split the limiter into its own module.");
  let answered = page.answer("composer (plan's changes)", &item, |page| {
    page.click("composer-send-button")
  });
  assert!(answered.at_once, "the plan leaves in the press's frame");
  page.end();
}

/// The Orchestrator chat's tools answer as `answer_question` does: through the project's runner, as automation, with
/// each question's first option.
fn answer_as_the_orchestrator(page: &Page, item: &WaitingItem) {
  let WaitingKind::Choice(choice) = &item.kind else {
    panic!("a question with suggested answers");
  };
  let asked = ProjectDb::open(&page.repo)
    .expect("the project's database")
    .waiting_questions()
    .expect("the waiting questions")
    .into_iter()
    .find(|asked| *asked.request.id() == *choice.request)
    .expect("the question waits");
  let answers = asked
    .request
    .questions()
    .iter()
    .map(|question| GivenAnswer {
      choices: vec![question.options()[0].label().to_owned()],
      text: None,
    })
    .collect();
  let command = Command::AnswerQuestion {
    run: asked.request.run(),
    request: asked.request.id().to_owned(),
    answers,
    by: Answerer::Automation,
  };
  let (services, open) = (
    page.window.launch.services().expect("the session started").clone(),
    page.open_project(),
  );
  let runners = services.runners.clone();
  services.runtime.spawn(async move {
    if let Err(error) = runners.control_run_in(&open, command).await {
      panic!("the runner did not take the answer: {error}");
    }
  });
}

/// The Orchestrator answers while the Overview shows the project, or while another tab is the active one. Its tools
/// answer off the window's thread, so the item leaves once their mark reaches the feed, not in a press's frame.
fn orchestrator(away: bool) {
  let mut page = Page::live();
  page.start(CHOICE);
  let item = page.wait_for("the agent's question", is_choice);
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
  page.answer(label, &item, |page| answer_as_the_orchestrator(page, &item));
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
fn answered_by_the_orchestrator_leaves_the_overview() {
  orchestrator(false);
}

#[test]
fn answered_by_the_orchestrator_for_another_tab_leaves() {
  orchestrator(true);
}
