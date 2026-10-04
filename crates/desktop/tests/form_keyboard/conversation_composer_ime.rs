//! The composer and an input method (IME), headlessly, on a draft task's Conversation tab: while a
//! composition is shown, Enter and Ctrl+Enter only confirm it, so nothing is sent and no line is
//! added; once the input method commits it, Enter sends the whole message.

use lurq::app::events::ImeEvent;
use orchester_core::store::{ConversationStore, TaskStore};
use orchester_store::ProjectDb;

use crate::{
  conversation_composer::holds,
  harness::Desktop,
  task_run::{fill_new_task, init_repository, programs, seed},
};

/// What the person's messages to the project's only task say, as stored.
fn sent(desktop: &Desktop) -> Vec<String> {
  let db = ProjectDb::open(desktop.repo()).expect("open the project");
  let task = db.tasks().expect("tasks").remove(0);
  db.messages(task.id())
    .expect("messages")
    .iter()
    .map(|message| message.body().as_str().to_owned())
    .collect()
}

/// A draft task's Conversation tab, its composer's field focused.
fn draft_composer() -> Desktop {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  fill_new_task(&mut desktop, "Say hello.", "1400000");
  desktop.click("save-draft");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-tab-conversation");
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-input");
  desktop.click("conversation-input");
  desktop
}

#[test]
fn composer_enter_confirming_a_composition_sends_nothing() {
  let mut desktop = draft_composer();
  desktop.type_text("Hello");
  desktop.tree.ime(ImeEvent::Enabled);
  desktop.tree.ime(ImeEvent::Preedit {
    text: "世界".into(),
    cursor: Some((6, 6)),
  });
  desktop.pass();
  assert!(desktop.tree.is_composing());
  // Windows reports the Enter the input method takes as `Process`.
  desktop.key("Process", "Enter");
  // An Enter the input method passes on arrives marked as composing; so does Ctrl+Enter.
  desktop.enter();
  desktop
    .tree
    .key_down("Enter".into(), "Enter".into(), false, true, false);
  desktop.pass();
  for _ in 0..5 {
    desktop.tree.tick_futures();
    desktop.pass();
  }
  assert_eq!(desktop.value("conversation-input"), "Hello", "not sent, no line added");
  assert!(sent(&desktop).is_empty());

  desktop.tree.ime(ImeEvent::Commit("世界".into()));
  desktop.pass();
  assert!(!desktop.tree.is_composing());
  assert_eq!(desktop.value("conversation-input"), "Hello世界");
  desktop.enter();
  desktop.wait_until("queued for the start", |desktop| {
    holds(desktop, "conversation", "Queued · waiting for the run to start")
  });
  assert_eq!(sent(&desktop), ["Hello世界"]);
}
