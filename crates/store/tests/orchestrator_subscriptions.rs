//! ORC-758: what the project's Orchestrator conversations watch, kept in the project database: read per project and
//! per conversation, one leader per target even when two conversations act at once, a take over and a stop or end in
//! one change each, and the ended ones kept with when and why. Run with
//! `cargo test -p orchester-store --test orchestrator_subscriptions subscriptions`.

mod support;

use std::{fs, path::Path, sync::Barrier, thread};

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ModuleId, ProjectId, TaskId},
  orchestrator::{
    Chat, Subscription, TurnState, WatchTarget,
    watch::{EndReason, WatchEnd},
  },
  project::{Project, ProjectKey},
};
use orchester_store::{ChatDeletion, KeptSubscription, ProjectDb, WatchStart};
use support::{at, name};
use tempfile::TempDir;

fn project() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    name("Kontur"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

fn watch(chat: ChatId, target: WatchTarget, order: &str, since: i64) -> Subscription {
  Subscription::new(chat, target, order, at(since))
}

fn active(subscription: Subscription) -> KeptSubscription {
  KeptSubscription {
    subscription,
    ended: None,
  }
}

fn ended(subscription: Subscription, when: i64, why: WatchEnd) -> KeptSubscription {
  KeptSubscription {
    subscription,
    ended: Some((at(when), why)),
  }
}

#[test]
fn subscriptions_round_trip_per_project_and_per_conversation() {
  let (repo, mut db) = project();
  let (first, second) = (ChatId::generate(), ChatId::generate());
  let kept = [
    watch(first, WatchTarget::Project, "", 1),
    watch(
      second,
      WatchTarget::Module(ModuleId::generate()),
      "  merge each task  ",
      2,
    ),
    watch(first, WatchTarget::Task(TaskId::generate()), "review it", 3),
    watch(second, WatchTarget::WithoutModule, "", 4),
  ];
  for subscription in &kept {
    assert_eq!(db.start_subscription(subscription).expect("kept"), WatchStart::Started);
  }
  drop(db);

  let db = ProjectDb::open(repo.path()).expect("open");
  let all = db.active_subscriptions().expect("read");
  assert_eq!(
    all.as_slice(),
    kept.as_slice(),
    "every target, its order and since, in the order kept"
  );
  assert_eq!(
    all.as_slice()[0].order(),
    None,
    "a blank order takes the project's default"
  );
  assert_eq!(all.as_slice()[1].order(), Some("merge each task"));
  assert_eq!(
    db.chat_subscriptions(first).expect("read"),
    [kept[0].clone(), kept[2].clone()]
  );
  assert_eq!(
    db.chat_subscriptions(second).expect("read"),
    [kept[1].clone(), kept[3].clone()]
  );
  assert_eq!(db.chat_subscriptions(ChatId::generate()).expect("read"), []);
  let read_only = ProjectDb::open_read_only(repo.path()).expect("open to read");
  assert_eq!(
    read_only.active_subscriptions().expect("read"),
    all,
    "a reader that never writes reads them too"
  );
}

#[test]
fn subscriptions_keep_one_leader_of_a_target_when_two_conversations_start_at_once() {
  let (repo, db) = project();
  drop(db);
  let target = WatchTarget::Module(ModuleId::generate());
  let chats = [ChatId::generate(), ChatId::generate()];
  let barrier = Barrier::new(chats.len());
  let started: Vec<WatchStart> = thread::scope(|scope| {
    let handles: Vec<_> = chats
      .iter()
      .map(|&chat| {
        let barrier = &barrier;
        let root = repo.path();
        scope.spawn(move || {
          let mut db = ProjectDb::open(root).expect("open");
          barrier.wait();
          db.start_subscription(&watch(chat, target, "", 5)).expect("written")
        })
      })
      .collect();
    handles
      .into_iter()
      .map(|handle| handle.join().expect("thread"))
      .collect()
  });

  let db = ProjectDb::open(repo.path()).expect("open");
  let leaders = db.active_subscriptions().expect("read");
  let leader = leaders.of(target).expect("one leads").clone();
  assert_eq!(leaders.as_slice().len(), 1, "the other start kept nothing");
  let winner = chats
    .iter()
    .position(|&chat| chat == leader.chat())
    .expect("a racer leads");
  assert_eq!(started[winner], WatchStart::Started);
  assert_eq!(
    started[1 - winner],
    WatchStart::LedElsewhere(leader.clone()),
    "the later start is told who leads"
  );
  assert_eq!(
    ProjectDb::open(repo.path())
      .expect("open")
      .start_subscription(&watch(leader.chat(), target, "again", 9))
      .expect("written"),
    WatchStart::AlreadyHere
  );
  assert_eq!(
    db.active_subscriptions().expect("read"),
    leaders,
    "starting again changes nothing"
  );
  assert_one_leader_index(repo.path(), target_kind(target));
}

/// The table itself refuses a second leader of a target, whoever writes it.
fn assert_one_leader_index(root: &Path, kind: &str) {
  let connection = rusqlite::Connection::open(root.join(".orchester/orchester.db")).expect("open");
  let id: String = connection
    .query_row(
      "SELECT target_id FROM orchestrator_subscriptions WHERE target_kind = ?1 AND ended_at IS NULL",
      [kind],
      |row| row.get(0),
    )
    .expect("the leader");
  let second = connection.execute(
    "INSERT INTO orchestrator_subscriptions (chat_id, target_kind, target_id, order_text, since)
     VALUES (?1, ?2, ?3, '', 10)",
    rusqlite::params![ChatId::generate().to_string(), kind, id],
  );
  assert!(second.is_err(), "a second leader of the target is refused: {second:?}");
}

fn target_kind(target: WatchTarget) -> &'static str {
  match target {
    WatchTarget::Task(_) => "task",
    WatchTarget::Module(_) => "module",
    WatchTarget::WithoutModule => "without_module",
    WatchTarget::Project => "project",
  }
}

#[test]
fn subscriptions_take_over_ends_the_leader_and_keeps_the_new_one_in_one_change() {
  let (_repo, mut db) = project();
  let (theirs, mine) = (ChatId::generate(), ChatId::generate());
  let target = WatchTarget::Task(TaskId::generate());
  let led = watch(theirs, target, "fix it", 1);
  db.start_subscription(&led).expect("kept");

  let taking = watch(mine, target, "merge it", 5);
  assert_eq!(db.take_over_subscription(&taking).expect("taken"), Some(led.clone()));
  assert_eq!(db.active_subscriptions().expect("read").of(target), Some(&taking));
  assert_eq!(db.chat_subscriptions(theirs).expect("read"), []);
  assert_eq!(
    db.chat_subscription_history(theirs).expect("read"),
    [ended(led, 5, WatchEnd::TakenOver { by: mine })],
    "the taken one is kept, ended when the new one started"
  );

  let renewed = watch(mine, target, "merge it after review", 8);
  assert_eq!(
    db.take_over_subscription(&renewed).expect("renewed"),
    Some(taking.clone())
  );
  assert_eq!(
    db.chat_subscription_history(mine).expect("read"),
    [ended(taking, 8, WatchEnd::Renewed), active(renewed.clone())]
  );

  let free = WatchTarget::Project;
  assert_eq!(
    db.take_over_subscription(&watch(theirs, free, "", 9)).expect("taken"),
    None,
    "nobody led it"
  );
  assert_eq!(db.active_subscriptions().expect("read").as_slice().len(), 2);
}

#[test]
fn subscriptions_take_overs_at_once_leave_one_leader() {
  let (repo, db) = project();
  drop(db);
  let target = WatchTarget::WithoutModule;
  let chats: Vec<ChatId> = (0..4).map(|_| ChatId::generate()).collect();
  let barrier = Barrier::new(chats.len());
  thread::scope(|scope| {
    for (index, &chat) in chats.iter().enumerate() {
      let barrier = &barrier;
      let root = repo.path();
      scope.spawn(move || {
        let mut db = ProjectDb::open(root).expect("open");
        barrier.wait();
        let since = i64::try_from(index).expect("small") + 1;
        db.take_over_subscription(&watch(chat, target, "", since))
          .expect("taken");
      });
    }
  });

  let db = ProjectDb::open(repo.path()).expect("open");
  let leaders = db.active_subscriptions().expect("read");
  assert_eq!(leaders.as_slice().len(), 1, "one leader: {leaders:?}");
  let ended: usize = chats
    .iter()
    .map(|&chat| {
      db.chat_subscription_history(chat)
        .expect("read")
        .iter()
        .filter(|kept| kept.ended.is_some())
        .count()
    })
    .sum();
  assert_eq!(ended, chats.len() - 1, "every other take over ended the one before it");
}

#[test]
fn subscriptions_ending_keeps_when_and_why_and_frees_the_target() {
  let (_repo, mut db) = project();
  let (chat, other) = (ChatId::generate(), ChatId::generate());
  let task = WatchTarget::Task(TaskId::generate());
  let module = WatchTarget::Module(ModuleId::generate());
  let project_wide = WatchTarget::Project;
  for subscription in [
    watch(chat, task, "", 1),
    watch(chat, module, "", 2),
    watch(chat, project_wide, "", 3),
  ] {
    db.start_subscription(&subscription).expect("kept");
  }

  assert_eq!(
    db.stop_subscription(other, task, at(4)).expect("stop"),
    None,
    "only its leader stops it"
  );
  assert_eq!(
    db.end_subscription(chat, WatchTarget::WithoutModule, EndReason::TaskDone, at(4))
      .expect("end"),
    None,
    "nobody leads it"
  );
  let closed = EndReason::ModuleClosed { done: 3, canceled: 1 };
  assert_eq!(
    db.end_subscription(chat, task, EndReason::TaskCanceled, at(5))
      .expect("end"),
    Some(watch(chat, task, "", 1))
  );
  assert!(db.end_subscription(chat, module, closed, at(6)).expect("end").is_some());
  assert!(db.stop_subscription(chat, project_wide, at(7)).expect("stop").is_some());
  assert_eq!(db.active_subscriptions().expect("read").as_slice(), []);
  assert_eq!(
    db.chat_subscription_history(chat).expect("read"),
    [
      ended(watch(chat, task, "", 1), 5, WatchEnd::ByItself(EndReason::TaskCanceled)),
      ended(watch(chat, module, "", 2), 6, WatchEnd::ByItself(closed)),
      ended(watch(chat, project_wide, "", 3), 7, WatchEnd::Stopped),
    ]
  );
  assert_eq!(
    db.start_subscription(&watch(other, task, "", 8)).expect("kept"),
    WatchStart::Started,
    "an ended subscription leads nothing"
  );
}

#[test]
fn subscriptions_of_a_deleted_conversation_end_with_it() {
  let (_repo, mut db) = project();
  let (chat, message) = (ChatId::generate(), ChatMessageId::generate());
  let text = MessageBody::new("Watch the payments module").expect("text");
  db.keep_chat_message(&Chat::start(chat, &text, at(1)), (message, &text), at(1))
    .expect("kept");
  db.end_chat_turn(message, &TurnState::Answered, at(2))
    .expect("answered");
  let module = WatchTarget::Module(ModuleId::generate());
  let kept = watch(chat, module, "merge each task", 3);
  db.start_subscription(&kept).expect("kept");

  assert_eq!(db.delete_chat(chat).expect("deleted"), ChatDeletion::Deleted);
  assert_eq!(db.active_subscriptions().expect("read").as_slice(), []);
  let history = db.chat_subscription_history(chat).expect("read");
  assert_eq!(history.len(), 1);
  assert_eq!(history[0].subscription, kept);
  assert!(
    matches!(history[0].ended, Some((_, WatchEnd::ChatDeleted))),
    "kept, ended with its conversation: {history:?}"
  );
  let other = ChatId::generate();
  assert_eq!(
    db.start_subscription(&watch(other, module, "", 4)).expect("kept"),
    WatchStart::Started
  );
}
