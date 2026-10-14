//! A new conversation (`HvzWI`): its start, suggestions that fill the prompt and send nothing, and what keeps the chat
//! from sending until the Orchestrator has something to run on.

use orchester_app::routes::OrchestratorChat;

use super::harness::{Orchestrator, Setup};

#[test]
fn orchestrator_new_conversation_starts_with_suggestions_that_fill_the_prompt() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::Latest);
  screen.until("the start", |screen| screen.exists("orchestrator-start-title"));
  assert_eq!(screen.text("orchestrator-title"), "New conversation");
  assert_eq!(screen.text("orchestrator-meta"), "Not saved until you send");
  assert_eq!(screen.text("orchestrator-runs-on"), "Claude · Kontur / haiku");
  assert!(
    screen
      .text("orchestrator-footer")
      .contains("Conversations stay in History for Kontur")
  );
  assert!(
    screen
      .text("orchestrator-under-composer")
      .contains("Enter adds a line ·")
  );
  screen.click("orchestrator-suggestion-running");
  assert_eq!(
    screen.text("orchestrator-input"),
    "What's running, what's waiting and what's blocked right now?"
  );
  assert!(
    screen.db().chats().expect("chats").is_empty(),
    "a suggestion sends nothing"
  );
  // Enter alone adds a line; nothing is sent.
  screen.click("orchestrator-input");
  screen.key(("Enter", "Enter"), false);
  assert!(screen.db().chats().expect("chats").is_empty());
}

#[test]
fn orchestrator_without_a_connection_says_where_to_choose_one() {
  let mut screen = Orchestrator::new(Setup::NotChosen);
  screen.go(OrchestratorChat::New);
  screen.until("why it can't send", |screen| screen.exists("orchestrator-not-ready"));
  assert!(
    screen
      .text("orchestrator-not-ready")
      .contains("Choose what the Orchestrator runs on")
  );
  assert_eq!(screen.text("orchestrator-runs-on"), "Not set up");
  screen.send("Plan the webhook work.");
  for _ in 0..20 {
    screen.window.tree.tick_futures();
    screen.window.pass();
  }
  assert!(screen.db().chats().expect("chats").is_empty(), "nothing is sent");
  assert_eq!(
    screen.text("orchestrator-input"),
    "Plan the webhook work.",
    "the draft stays"
  );
  screen.click("orchestrator-open-settings");
  screen.until("its settings", |screen| screen.exists("orchestrator-settings-form"));
}

#[test]
fn orchestrator_draft_is_kept_when_the_runner_never_takes_it() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  screen.go(OrchestratorChat::New);
  screen.send("What is running?");
  screen.until("the failed send", |screen| screen.exists("orchestrator-send-problem"));
  assert!(
    screen
      .text("orchestrator-send-problem")
      .contains("Not sent · the runner didn't answer")
  );
  assert!(screen.text("orchestrator-send-problem").contains("Your draft is kept."));
  assert_eq!(screen.text("orchestrator-input"), "What is running?");
  assert!(
    screen.db().chats().expect("chats").is_empty(),
    "a message the runner never took is not kept"
  );
  assert!(screen.exists("orchestrator-send-retry"));
}
