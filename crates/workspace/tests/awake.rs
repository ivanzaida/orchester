//! When work is under way in a project (`orchester_workspace::awake`, `docs/plan/orchestrator-autonomy/OA-08-…`): an
//! agent of a run works, or a turn of the Orchestrator chat thinks; not while a run waits for a person, and not once
//! it ended.
//! `cargo test -p orchester-workspace --test awake awake`.

mod support;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, TurnState},
  run::WaitReason,
};
use orchester_workspace::awake::work_under_way;
use support::{Fixture, at};

#[test]
fn awake_work_is_under_way_while_an_agent_works_or_a_turn_thinks() {
  let mut fixture = Fixture::new();
  let root = fixture.repo.path().to_path_buf();
  assert!(!work_under_way(&root).expect("read"), "an empty project");

  // An agent works.
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  assert!(work_under_way(&root).expect("read"));
  // Its step waits for the person: nothing is lost by sleeping.
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.wait_step(execution, WaitReason::Message, at(30)).expect("waits");
  fixture.record(&run, &events);
  assert!(!work_under_way(&root).expect("read"));
  // It goes on, and ends.
  let events = run.resume_step(execution, at(40)).expect("resumed");
  fixture.record(&run, &events);
  assert!(work_under_way(&root).expect("read"));
  fixture.succeed(&mut run, at(50));
  assert!(!work_under_way(&root).expect("read"));

  // A turn of the Orchestrator thinks, then ends.
  let chat = Chat::named(ChatId::generate(), "Kontur", at(60));
  let message = ChatMessageId::generate();
  let text = MessageBody::new("What is left?").expect("text");
  fixture
    .db
    .keep_chat_message(&chat, (message, &text), at(60))
    .expect("kept");
  assert!(work_under_way(&root).expect("read"));
  fixture
    .db
    .end_chat_turn(message, &TurnState::Answered, at(70))
    .expect("ended");
  assert!(!work_under_way(&root).expect("read"));
}
