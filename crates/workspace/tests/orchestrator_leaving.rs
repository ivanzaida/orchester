//! Before you leave (`orchestrator::leaving`, `docs/plan/orchestrator-autonomy/OA-19-…`): what would stop or weaken
//! the work a conversation leads to the end while nobody is at the computer, read when it is asked for. It names the
//! switch that is off, an Orchestrator with nothing to run on, the tasks that cannot start, a computer that may sleep,
//! and always that closing the window stops the work.
//! `cargo test -p orchester-workspace --test orchestrator_leaving leaving`.

mod support;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Ceilings, Chat, Subscription, TurnState, WatchMode, WatchTarget},
};
use orchester_exec::programs::Programs;
use orchester_workspace::orchestrator::leaving::{AppSet, Leaving, Weak, leaving_in};
use support::{Fixture, at};

fn names(leaving: &Leaving) -> Vec<&'static str> {
  leaving.weak.iter().map(Weak::name).collect()
}

#[test]
fn leaving_names_what_would_stop_the_work_while_nobody_is_there() {
  let mut fixture = Fixture::new();
  // A conversation that leads the whole project to the end.
  let chat = Chat::named(ChatId::generate(), "Leads Kontur", at(1));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Lead it to the end").expect("text");
  fixture
    .db
    .keep_chat_message(&chat, (person, &text), at(1))
    .expect("kept");
  fixture
    .db
    .end_chat_turn(person, &TurnState::Answered, at(2))
    .expect("ended");
  let lead = WatchMode::Lead {
    ceilings: Ceilings::default(),
  };
  let leading = Subscription::new(chat.id(), WatchTarget::Project, "", at(3)).in_mode(lead);
  fixture.db.start_subscription(&leading).expect("leading");
  // A task with no pipeline cannot start; one queued on the fixture's pipeline can.
  fixture.task("No pipeline yet", at(10));
  fixture.queued_task("Add login", at(20));
  let (root, programs) = (fixture.repo.path(), Programs::processes());

  let off = AppSet {
    drives: false,
    keeps_awake: false,
  };
  let left = leaving_in((root, &programs), &leading, off).expect("read");
  let told = names(&left);
  for expected in [
    "drive_off",
    "orchestrator_not_ready",
    "no_pipeline",
    "may_sleep",
    "stops_with_window",
  ] {
    assert!(told.contains(&expected), "{expected}: {told:?}");
  }
  assert_eq!(told.first(), Some(&"drive_off"), "the most stopping first: {told:?}");
  assert_eq!(told.last(), Some(&"stops_with_window"));
  let unstarted = left
    .weak
    .iter()
    .find(|weak| weak.name() == "no_pipeline")
    .expect("named");
  assert_eq!(unstarted.tasks(), ["ORC-1".to_owned()]);
  assert!(!left.ready());
  assert_eq!(left.not_checked.len(), 1, "what the app cannot tell is named");

  // The switches on: those two lines go; what the project itself lacks stays.
  let on = AppSet {
    drives: true,
    keeps_awake: true,
  };
  let left = leaving_in((root, &programs), &leading, on).expect("read");
  let told = names(&left);
  assert!(!told.contains(&"drive_off") && !told.contains(&"may_sleep"), "{told:?}");
  assert!(told.contains(&"no_pipeline") && !left.ready(), "{told:?}");
}

#[test]
fn leaving_is_ready_when_only_the_window_is_left_to_mind() {
  let only_window = Leaving {
    weak: vec![Weak::StopsWithWindow],
    not_checked: Vec::new(),
  };
  assert!(only_window.ready());
  let asleep = Leaving {
    weak: vec![Weak::MaySleep, Weak::StopsWithWindow],
    not_checked: Vec::new(),
  };
  assert!(!asleep.ready());
}
