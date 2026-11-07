//! The conversation's end is where the person scrolls to (ORC-892): under a long answer, a `/watch-module` bubble and
//! its *Leading …* line, scrolling down reaches the very end, with nothing left under the last row, and *Jump to
//! latest* hides as soon as that end is in view, whether the wheel or the pill got there; rows measured only as the
//! scroll reaches them keep the chat at its end.

use orchester_core::{
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{
    ChatEvent, WatchTarget,
    watch::{NamedTarget, WatchCommand, WatchLine},
  },
  time::Timestamp,
};

use super::{
  harness::Orchestrator,
  scroll::{long_conversation, settle, wheel},
};

/// Keeps a `/watch-module` of a module *Webhooks v2* and its subscription line at the end of the conversation shown,
/// as the app writes them, and waits until they show: the id of the subscription line's row.
fn watch_tail(screen: &mut Orchestrator) -> String {
  let mut db = screen.db();
  let chat = db.chats().expect("chats").remove(0).chat;
  let name = Name::new("Webhooks v2").expect("name");
  let details = ModuleDetails::new(name, "", (None, None), ModuleState::Open).expect("details");
  let module = db.create_module(details, Timestamp::now()).expect("module");
  let target = NamedTarget {
    target: WatchTarget::Module(module.id()),
    name: Some("Webhooks v2".to_owned()),
  };
  let entries = [
    ChatEvent::WatchCommand {
      command: WatchCommand::Watch {
        targets: vec![target.clone()],
        order: "Answer agents from the brief.".to_owned(),
      },
    },
    ChatEvent::Watch {
      line: WatchLine::Watching {
        started: vec![target],
        already_here: Vec::new(),
        led_elsewhere: Vec::new(),
        gone: Vec::new(),
      },
    },
  ];
  db.keep_watch_entries(&chat, &entries, Timestamp::now()).expect("kept");
  let leading = db
    .chat_entries(chat.id())
    .expect("entries")
    .last()
    .expect("the line")
    .seq;
  drop(db);
  let root = screen.root.clone();
  screen
    .window
    .launch
    .services()
    .expect("the session started")
    .runners
    .chat_changed(&root);
  let leading = format!("orchestrator-watch-wait-{leading}");
  screen.until("the subscription line", |screen| {
    screen
      .text(&format!("{leading}-words"))
      .starts_with("Leading Webhooks v2")
  });
  settle(screen);
  leading
}

/// How far the turns' view ends below the foot of row `id`, once it is built.
fn foot_gap(screen: &mut Orchestrator, id: &str) -> Option<f32> {
  let view = screen.bounds("orchestrator-scroll");
  let row = screen.window.tree.get_element_by_id_mut(id)?.bounds()?;
  Some(view.y + view.height - (row.y + row.height))
}

/// Asserts the chat sits at its very end with row `id` last and *Jump to latest* hidden: one more turn of the wheel
/// down moves nothing.
fn at_true_end(screen: &mut Orchestrator, id: &str, when: &str) {
  assert!(
    !screen.exists("conversation-jump"),
    "{when}: the end is in view, Jump to latest hides"
  );
  let gap = foot_gap(screen, id).expect("the last row is built");
  wheel(screen, -120.0);
  settle(screen);
  let after = foot_gap(screen, id).expect("the last row is built");
  assert!(
    (after - gap).abs() < 1.0,
    "{when}: nothing is left to scroll under the last row: {gap} then {after}"
  );
  assert!(!screen.exists("conversation-jump"), "{when}: still at the end");
}

/// A conversation of `turns` long turns ending with the watch tail, scrolled up from its end: the subscription line's
/// row id.
fn scrolled_up(turns: usize) -> (Orchestrator, String) {
  let mut screen = long_conversation(turns);
  let leading = watch_tail(&mut screen);
  at_true_end(&mut screen, &leading, "opened at the end");
  wheel(&mut screen, 600.0);
  settle(&mut screen);
  assert!(screen.exists("conversation-jump"), "scrolled up, Jump to latest shows");
  (screen, leading)
}

#[test]
fn orchestrator_end_wheel_steps_reach_the_true_end_under_a_watch_tail() {
  let (mut screen, leading) = scrolled_up(2);
  for _ in 0..40 {
    if foot_gap(&mut screen, &leading).is_some_and(|gap| gap >= 0.0) {
      break;
    }
    wheel(&mut screen, -100.0);
  }
  settle(&mut screen);
  at_true_end(&mut screen, &leading, "the wheel brought the last row in view");
}

#[test]
fn orchestrator_end_rows_measured_at_the_end_keep_the_chat_there() {
  let (mut screen, leading) = scrolled_up(3);
  for _ in 0..10 {
    wheel(&mut screen, 5000.0);
  }
  settle(&mut screen);
  assert!(
    foot_gap(&mut screen, &leading).is_none(),
    "at the start, the last rows are not built"
  );
  // One turn of the wheel to the end as the list estimates it: the rows there are measured only as it gets there.
  wheel(&mut screen, -100_000.0);
  settle(&mut screen);
  at_true_end(&mut screen, &leading, "a wheel to the estimated end");
}

#[test]
fn orchestrator_end_jump_to_latest_reaches_the_true_end() {
  let (mut screen, leading) = scrolled_up(2);
  screen.click("conversation-jump");
  settle(&mut screen);
  at_true_end(&mut screen, &leading, "after Jump to latest");
}
