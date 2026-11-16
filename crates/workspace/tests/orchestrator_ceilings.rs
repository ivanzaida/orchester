//! What the work a subscription leads spent (`orchestrator::ceilings::spent`, OA-09), which its budget is held
//! against: its conversation's own turns since it leads, and the runs, started since, of the tasks whose events go to
//! it; not what another conversation spent, nor what was spent before.
//! `cargo test -p orchester-workspace --test orchestrator_ceilings ceilings`.

mod support;

use orchester_core::{
  id::{AgentAccountId, ChatId, ChatMessageId},
  orchestrator::{Chat, Subscription, WatchTarget},
  run::TokenUsage,
  tokens::TokensUsed,
};
use orchester_workspace::orchestrator::ceilings::spent;
use support::{Fixture, at};

/// A turn of `chat` spent 30 counted tokens at `at(when)`.
fn turn_spent(fixture: &mut Fixture, chat: ChatId, when: i64) {
  let usage = TokenUsage {
    input: 10,
    cached_input: 0,
    output: 20,
  };
  fixture
    .db
    .record_chat_usage(
      (chat, ChatMessageId::generate()),
      (AgentAccountId::generate(), usage),
      at(when),
    )
    .expect("usage");
}

#[test]
fn ceilings_count_what_a_subscriptions_own_conversation_spent_since_it_leads() {
  let mut fixture = Fixture::new();
  let leading = fixture.everything(at(100));
  let other = Chat::named(ChatId::generate(), "Another", at(100));
  let task = fixture.task("Add login", at(40));
  fixture.started_run(task, at(150));
  turn_spent(&mut fixture, leading.id(), 50);
  turn_spent(&mut fixture, leading.id(), 120);
  turn_spent(&mut fixture, leading.id(), 180);
  turn_spent(&mut fixture, other.id(), 130);

  let subscriptions = fixture.db.active_subscriptions().expect("subscriptions");
  let subscription = subscriptions
    .of(WatchTarget::Project)
    .expect("the whole project is led");
  let one_turn = TokenUsage {
    input: 10,
    cached_input: 0,
    output: 20,
  }
  .counted_tokens();
  let mut two_turns = TokensUsed::ZERO;
  two_turns += one_turn;
  two_turns += one_turn;
  assert_eq!(
    spent(&fixture.db, &subscriptions, subscription).expect("read"),
    two_turns,
    "its two turns since it leads; the run it leads spent nothing yet"
  );

  // A task another conversation leads by a more specific subscription is not this one's work.
  let taken = Subscription::new(other.id(), WatchTarget::Task(task), "", at(160));
  fixture.db.start_subscription(&taken).expect("watching");
  let subscriptions = fixture.db.active_subscriptions().expect("subscriptions");
  let theirs = subscriptions.of(WatchTarget::Task(task)).expect("led");
  assert_eq!(
    spent(&fixture.db, &subscriptions, theirs).expect("read"),
    TokensUsed::ZERO,
    "the other conversation's turn was before it led the task, and the run started before too"
  );
}
