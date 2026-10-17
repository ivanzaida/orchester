//! What the agent of a step that waits for a message wrote last (ORC-42, `tasks::last_words`): the question it ended
//! its step with, so a wait is never unreadable; a step that wrote nothing that can be read is still listed.

mod support;

use orchester_core::{
  conversation::{AgentMessage, Message, MessageBody},
  resource::ModelId,
  run::WaitReason,
  store::{ActivityStore, ConversationStore},
};
use orchester_workspace::tasks::last_words::waiting_words;
use support::{Fixture, at};

#[test]
fn last_words_of_a_waiting_step_are_the_question_its_agent_asked() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  assert!(
    waiting_words(&fixture.db, &run).expect("read").is_empty(),
    "no step waits"
  );
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.wait_step(execution, WaitReason::Message, at(30)).expect("waits");
  fixture.record(&run, &events);
  assert_eq!(
    waiting_words(&fixture.db, &run).expect("read"),
    [(execution, None)],
    "a step that waits is listed though its agent wrote nothing that can be read"
  );
  let invocation = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at(25))
    .expect("invocation")
    .id;
  let asked = Message::from_agent(
    task,
    run.id(),
    (execution, invocation),
    AgentMessage::Question,
    MessageBody::new("Keep email sign-in?").expect("text"),
    at(28),
  );
  fixture.db.add_agent_message(&asked).expect("asked");
  let words = waiting_words(&fixture.db, &run).expect("read");
  let (waiting, said) = &words[0];
  let said = said.as_ref().expect("its question");
  assert_eq!(*waiting, execution);
  assert_eq!(
    (&*said.text, said.question, said.at),
    ("Keep email sign-in?", true, at(28))
  );
}
