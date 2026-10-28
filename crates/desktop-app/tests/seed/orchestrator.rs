//! The `orchestrator` scenario: its waits stand as the conversation reads them, under the keys the app's follower
//! keeps them by, so following the project keeps nothing new; and the runs-on picker's sign-ins, with their checks and
//! plan reads, are in the application database.

use orchester_core::{account::AccountState, connection::invocation::Platform, id::ChatId};
use orchester_exec::programs::Programs;
use orchester_store::AppDb;
use orchester_workspace::orchestrator::{
  chat::{ChatView, EndMoved},
  waits::{MergeChecks, Switches, follow_in},
};

use super::Seed;

/// The conversation of `seed` titled `title`, as the screen reads it.
fn conversation(seed: &Seed, title: &str) -> ChatView {
  let chats = seed.db().chats().expect("the conversations");
  let chat: ChatId = chats
    .iter()
    .find(|listing| listing.chat.title() == title)
    .map(|listing| listing.chat.id())
    .unwrap_or_else(|| panic!("a conversation {title}"));
  ChatView::load_in(seed.repo(), chat).expect("read").expect("kept")
}

#[test]
fn seed_orchestrator_keeps_waits_that_stand_under_the_followers_keys() {
  let seed = Seed::of("orchestrator");
  let away = conversation(&seed, "Plan the webhook retry work");
  assert_eq!(
    away.standing.len(),
    4,
    "the digest's question and three interrupted runs stand"
  );
  let cards = conversation(&seed, "Every card");
  assert_eq!(
    cards.standing.len(),
    3,
    "the run ready to merge, the plan and the question being read stand"
  );
  assert_eq!(
    cards.moved.values().copied().collect::<Vec<_>>(),
    vec![EndMoved::Merged],
    "the end of the run merged since no longer waits: its line is neutral"
  );
  let followed = follow_in(
    (seed.repo(), seed.seeded.project, &Programs::processes()),
    &MergeChecks::default(),
    (Switches::default(), true),
  )
  .expect("followed");
  assert!(
    !followed.kept,
    "every wait and end is kept under the follower's own key"
  );
}

#[test]
fn seed_orchestrator_adds_the_pickers_sign_ins_with_their_plan_use() {
  let seed = Seed::of("orchestrator");
  let app = AppDb::open(&seed.seeded.app_database).expect("app database");
  let accounts = app.agent_accounts().expect("accounts");
  let state = |named: &str| {
    let account = accounts
      .iter()
      .find(|account| account.name().is_some_and(|name| name.as_str() == named))
      .unwrap_or_else(|| panic!("an account {named}"));
    let read = app.last_plan_read(account.id()).expect("plan read");
    (account.state(Platform::Windows), read.is_some())
  };
  assert_eq!(state("Work"), (AccountState::SignedIn, true));
  assert_eq!(state("Personal"), (AccountState::SignedIn, true));
  assert_eq!(state("Client"), (AccountState::NotSignedIn, false));
}
