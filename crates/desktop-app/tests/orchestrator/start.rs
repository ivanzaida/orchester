//! A new conversation (`03 New conversation` `c99rep`): its start, suggestions that fill the prompt and send nothing,
//! and, until the Orchestrator has something to run on, the composer saying so (`02 Not set up` `Wd5CL`).

use orchester_app::routes::OrchestratorChat;

use super::harness::{Orchestrator, Setup};

#[test]
fn orchestrator_new_conversation_starts_with_suggestions_that_fill_the_prompt() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::Latest);
  screen.until("the start", |screen| screen.exists("orchestrator-start-title"));
  assert_eq!(screen.text("orchestrator-title"), "New conversation");
  assert_eq!(screen.text("orchestrator-meta"), "");
  let runs_on = screen.text("orchestrator-runs-on");
  assert!(
    runs_on.starts_with("Claude Code · ") && runs_on.ends_with(" · haiku"),
    "{runs_on}"
  );
  assert!(
    screen.text("orchestrator-keys").ends_with(" Enter"),
    "the keys that send, in the bar"
  );
  assert!(
    !screen.exists("orchestrator-footer") && !screen.exists("orchestrator-under-composer"),
    "no keys line and no footer under the composer"
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
fn orchestrator_without_a_connection_says_so_in_the_composer() {
  let mut screen = Orchestrator::new(Setup::NotChosen);
  screen.go(OrchestratorChat::New);
  assert_eq!(screen.text("orchestrator-runs-on"), "Not set up");
  assert!(
    screen
      .text("orchestrator-input")
      .starts_with("The Orchestrator can't answer yet"),
    "{}",
    screen.text("orchestrator-input")
  );
  assert!(
    !screen.exists("orchestrator-send") && !screen.exists("orchestrator-not-ready-notice"),
    "no send and no notice over the composer"
  );
  screen.set("orchestrator-input", "Plan the webhook work.");
  screen
    .window
    .tree
    .get_element_by_id_mut("orchestrator-input")
    .expect("the message field")
    .focus();
  screen.key(("Enter", "Enter"), true);
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
  assert_eq!(screen.text("orchestrator-open-settings"), "Set up the Orchestrator");
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
