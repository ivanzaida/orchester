//! A watch on a team task ends on the team server's status (`orchestrator::team_status`, `orchestrator::watch_ends`):
//! this computer keeps a team task only as an identity and its runs, so the status is read from the server in the
//! background. A reading that cannot be made leaves the watch standing and says why in its listing; a local task's
//! watch ends on its stored row as before.

use std::time::Duration;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ModuleId, ProjectId, TaskId},
  module::{Module, ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{
    Chat, EntryKind, Subscription, TurnState, WatchTarget,
    watch::{EndReason, WatchSummary},
  },
  secret::{SecretValue, SecretVault, VaultKey},
  sharing::TaskRevision,
  store::{RunStore, TaskStore},
  task::{
    identity::TeamTaskRef,
    organization::{ChangeActor, WorkflowStatus},
  },
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::CachedModule;
use orchester_workspace::orchestrator::{
  team_status::{Refreshed, TeamReading, refresh},
  waits::{Followed, Switches, check_merges_in, follow_in},
  watching::WatchReads,
};
use parking_lot::Mutex;
use serde_json::json;

use crate::{
  Checkout,
  reads::Seen,
  team_runs::{Started, started},
  team_server::MEMBER,
  tools::server,
};

const NEVER_STALE: Duration = Duration::ZERO;

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

/// A conversation of the project that watches `target` from the start.
fn watching(checkout: &Checkout, target: impl FnOnce(&Checkout) -> WatchTarget) -> (ChatId, ProjectId) {
  let target = target(checkout);
  let mut db = checkout.db();
  let chat = Chat::named(ChatId::generate(), "Watching", at(10));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Watch it").expect("text");
  db.keep_chat_message(&chat, (person, &text), at(10)).expect("kept");
  db.end_chat_turn(person, &TurnState::Answered, at(12)).expect("ended");
  db.start_subscription(&Subscription::new(chat.id(), target, "", at(13)))
    .expect("watching");
  (chat.id(), db.project().id())
}

fn team_task(team: &Started) -> WatchTarget {
  let task = team.checkout.db().run(team.run).expect("read").expect("run").task();
  WatchTarget::Task(task)
}

/// Reads the watched team tasks from the server, as the app does every minute.
fn refreshed(checkout: &Checkout) -> Refreshed {
  checkout
    .runtime
    .block_on(refresh(&checkout.servers, checkout.root(), NEVER_STALE))
    .expect("refreshed")
}

fn follow(checkout: &Checkout, project: ProjectId) -> Followed {
  let programs = Programs::processes();
  let checks = check_merges_in((checkout.root(), &programs), &Default::default()).expect("checked");
  follow_in(
    (checkout.root(), project, &programs),
    &checks,
    (Switches { drives: false }, true),
  )
  .expect("followed")
}

fn ends(checkout: &Checkout, chat: ChatId) -> Vec<WatchSummary> {
  checkout
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::WatchEnded { summary, .. } => Some(*summary),
      _ => None,
    })
    .collect()
}

fn is_watched(checkout: &Checkout, chat: ChatId) -> bool {
  !checkout.db().chat_subscriptions(chat).expect("read").is_empty()
}

/// Why the listing of `chat`'s one subscription says it cannot read the team task's status, if it does.
fn cannot_read(checkout: &Checkout, chat: ChatId) -> Option<&'static str> {
  let reads = WatchReads::read(&checkout.db(), checkout.root()).expect("reads");
  let listing = reads.watching(chat);
  assert_eq!(listing.subscriptions.len(), 1, "{listing:?}");
  listing.subscriptions[0].unreadable.as_ref().map(|why| why.key)
}

fn server_says(team: &Started, workflow: &str) {
  team.seen.lock().task_fields = json!({ "workflow": workflow });
}

#[test]
fn team_mcp_a_watch_on_an_identity_only_team_task_ends_when_the_server_marks_it_done() {
  let team = started();
  let (chat, project) = watching(&team.checkout, |_| team_task(&team));

  // Nothing is read until a reading is made, and an open task keeps the watch.
  assert_eq!(TeamReading::of(team.checkout.root()), TeamReading::default());
  let first = refreshed(&team.checkout);
  assert!(first.polling, "a subscription needs the server's answer");
  assert_eq!(follow(&team.checkout, project), Followed::default());
  assert!(is_watched(&team.checkout, chat));
  assert_eq!(cannot_read(&team.checkout, chat), None);

  // The server closes the task; the next reading sees it and the follow ends the watch.
  server_says(&team, "done");
  assert!(refreshed(&team.checkout).changed);
  assert!(follow(&team.checkout, project).kept);
  assert!(!is_watched(&team.checkout, chat));
  let ended = ends(&team.checkout, chat);
  assert_eq!(ended.len(), 1, "{ended:?}");
  assert_eq!(ended[0].reason, EndReason::TaskDone);
  assert_eq!(ended[0].name, "KON-7");
  assert_eq!(ended[0].title.as_deref(), Some("Say hello"));

  // With nothing left to watch the app stops asking the server.
  assert!(!refreshed(&team.checkout).polling);
  assert_eq!(TeamReading::of(team.checkout.root()), TeamReading::default());
}

#[test]
fn team_mcp_a_canceled_team_task_ends_its_watch_as_canceled() {
  let team = started();
  let (chat, project) = watching(&team.checkout, |_| team_task(&team));
  server_says(&team, "canceled");
  refreshed(&team.checkout);
  assert!(follow(&team.checkout, project).kept);
  assert_eq!(ends(&team.checkout, chat)[0].reason, EndReason::TaskCanceled);
}

#[test]
fn team_mcp_a_watch_on_a_team_task_waits_and_says_why_while_signed_out_then_ends() {
  let team = started();
  let (chat, project) = watching(&team.checkout, |_| team_task(&team));
  let link = team.checkout.db().sharing().expect("link").expect("linked");
  server_says(&team, "done");

  team
    .checkout
    .vault
    .delete(&VaultKey::server_token(link.server))
    .expect("signed out");
  let signed_out = refreshed(&team.checkout);
  assert!(signed_out.changed && signed_out.polling);
  assert_eq!(follow(&team.checkout, project), Followed::default());
  assert!(
    is_watched(&team.checkout, chat),
    "the watch waits; it does not end or fail"
  );
  assert_eq!(cannot_read(&team.checkout, chat), Some("orchestrator.watch.unreadable"));
  // Asking again changes nothing and says the same.
  assert!(!refreshed(&team.checkout).changed);

  // Signed in again, the next reading sees Done: the watch ends and the listing has nothing to say.
  let token = SecretValue::new(MEMBER.to_owned()).expect("token");
  team
    .checkout
    .vault
    .store(&VaultKey::server_token(link.server), &token)
    .expect("signed in");
  let back = refreshed(&team.checkout);
  assert!(back.changed, "{back:?}");
  assert!(follow(&team.checkout, project).kept);
  assert!(!is_watched(&team.checkout, chat));
  assert_eq!(ends(&team.checkout, chat)[0].reason, EndReason::TaskDone);
}

#[test]
fn team_mcp_a_watch_on_a_team_task_waits_while_the_server_cannot_be_reached() {
  let team = started();
  let (chat, project) = watching(&team.checkout, |_| team_task(&team));
  server_says(&team, "done");

  team.server.set_down(true);
  assert!(refreshed(&team.checkout).changed);
  assert_eq!(follow(&team.checkout, project), Followed::default());
  assert!(is_watched(&team.checkout, chat));
  assert_eq!(cannot_read(&team.checkout, chat), Some("orchestrator.watch.unreadable"));

  team.server.set_down(false);
  refreshed(&team.checkout);
  assert!(follow(&team.checkout, project).kept);
  assert_eq!(ends(&team.checkout, chat)[0].reason, EndReason::TaskDone);
}

#[test]
fn team_mcp_a_young_reading_is_not_read_again() {
  let team = started();
  watching(&team.checkout, |_| team_task(&team));
  let servers = &team.checkout.servers;
  let root = team.checkout.root();
  assert!(refreshed(&team.checkout).polling);

  // The server changed, but a reading younger than the age asked for stands: the app's chat updates cost no request.
  server_says(&team, "done");
  let young = team
    .checkout
    .runtime
    .block_on(refresh(servers, root, Duration::from_secs(3600)))
    .expect("refreshed");
  assert_eq!(
    young,
    Refreshed {
      changed: false,
      polling: true
    }
  );
  assert!(refreshed(&team.checkout).changed);
}

#[test]
fn team_mcp_a_module_watch_takes_in_a_team_task_by_the_module_the_server_reports() {
  let team = started();
  let module = team.server.module;
  let (chat, project) = watching(&team.checkout, |checkout| module_watch(checkout, module));

  // The one task in the module is open on the server.
  refreshed(&team.checkout);
  assert_eq!(follow(&team.checkout, project), Followed::default());
  assert!(is_watched(&team.checkout, chat));

  server_says(&team, "done");
  refreshed(&team.checkout);
  assert!(follow(&team.checkout, project).kept);
  let ended = ends(&team.checkout, chat);
  assert_eq!(ended.len(), 1, "{ended:?}");
  assert_eq!(ended[0].reason, EndReason::ModuleClosed { done: 1, canceled: 0 });
}

#[test]
fn team_mcp_a_module_watch_does_not_end_on_a_team_task_in_another_module() {
  let team = started();
  let module = team.server.module;
  let (chat, project) = watching(&team.checkout, |checkout| module_watch(checkout, module));
  server_says(&team, "done");
  // The server reports the closed task in another module: the watched module has no closed work.
  team.seen.lock().task_fields = json!({ "workflow": "done", "module": ModuleId::generate() });
  refreshed(&team.checkout);
  assert_eq!(follow(&team.checkout, project), Followed::default());
  assert!(is_watched(&team.checkout, chat));
}

#[test]
fn team_mcp_a_module_watch_waits_and_says_why_while_a_team_task_cannot_be_read() {
  let team = started();
  let module = team.server.module;
  let (chat, project) = watching(&team.checkout, |checkout| module_watch(checkout, module));
  server_says(&team, "done");
  team.server.set_down(true);
  refreshed(&team.checkout);
  assert_eq!(follow(&team.checkout, project), Followed::default());
  assert!(is_watched(&team.checkout, chat));
  assert_eq!(cannot_read(&team.checkout, chat), Some("orchestrator.watch.unreadable"));
}

#[test]
fn team_mcp_a_local_task_watch_ends_on_its_stored_row_and_needs_no_reading() {
  let (server, _seen) = server();
  let checkout = Checkout::new(Some(&server), &["Say hello"]);
  let task = checkout.db().tasks().expect("tasks")[0].id();
  let (chat, project) = watching(&checkout, |_| WatchTarget::Task(task));

  // A local watch asks the server nothing.
  assert!(!refreshed(&checkout).polling);
  assert_eq!(follow(&checkout, project), Followed::default());
  assert_eq!(cannot_read(&checkout, chat), None);

  checkout
    .db()
    .close_task_organization(task, WorkflowStatus::Done, (ChangeActor::Person, Timestamp::now()))
    .expect("closed");
  assert!(follow(&checkout, project).kept);
  assert!(!is_watched(&checkout, chat));
  let ended = ends(&checkout, chat);
  assert_eq!(
    (ended[0].reason, ended[0].name.as_str()),
    (EndReason::TaskDone, "KON-1")
  );
}

/// Caches the server's module as the project's own (as the sync does) and watches it.
fn module_watch(checkout: &Checkout, module: ModuleId) -> WatchTarget {
  let details = ModuleDetails::new(
    Name::new("Webhooks").expect("name"),
    "",
    (None, None),
    ModuleState::Open,
  )
  .expect("details");
  checkout
    .db()
    .cache_shared_modules(&[CachedModule {
      module: Module::new(module, details, at(1)),
      revision: TaskRevision::FIRST,
    }])
    .expect("cached");
  WatchTarget::Module(module)
}

/// Records another team task of the server's project, which the server may or may not hold.
fn record_team_task(checkout: &Checkout, id: TaskId) {
  let mut db = checkout.db();
  let link = db.sharing().expect("link").expect("linked");
  let team = TeamTaskRef {
    server: link.server,
    project: link.project,
    task: id,
  };
  db.record_team_task(team, TaskRevision::FIRST).expect("identity");
}

#[test]
fn team_mcp_a_module_watch_waits_for_a_reading_even_when_its_local_tasks_are_done() {
  let (server, seen) = server();
  let checkout = Checkout::new(Some(&server), &["Say hello"]);
  record_team_task(&checkout, server.task);
  let (chat, project) = watching(&checkout, |checkout| module_watch(checkout, server.module));
  let local = checkout.db().tasks().expect("tasks")[0].id();
  let mut db = checkout.db();
  db.set_task_module(local, Some(server.module), ChangeActor::Person, at(20))
    .expect("in the module");
  db.close_task_organization(local, WorkflowStatus::Done, (ChangeActor::Person, at(21)))
    .expect("closed");
  drop(db);

  // Nothing was read from the server yet: the team task in the module may be open, and an ending is not undone.
  assert_eq!(follow(&checkout, project), Followed::default());
  assert!(is_watched(&checkout, chat));

  // The server says the team task is open: the watch goes on. Closed, it ends, on one list request and no task read.
  refreshed(&checkout);
  assert_eq!(follow(&checkout, project), Followed::default());
  server_task(&seen, "done");
  refreshed(&checkout);
  assert!(follow(&checkout, project).kept);
  assert_eq!(
    ends(&checkout, chat)[0].reason,
    EndReason::ModuleClosed { done: 2, canceled: 0 }
  );
  let seen = seen.lock();
  assert_eq!((seen.task_lists, seen.task_gets), (2, 0));
}

fn server_task(seen: &Mutex<Seen>, workflow: &str) {
  seen.lock().task_fields = json!({ "workflow": workflow });
}

#[test]
fn team_mcp_a_team_task_recorded_after_the_reading_keeps_a_module_watch_waiting_until_it_is_read() {
  let team = started();
  let (chat, project) = watching(&team.checkout, |checkout| module_watch(checkout, team.server.module));
  server_says(&team, "done");
  refreshed(&team.checkout);

  // A team task the project recorded since is not in the reading: whether it is in the module is not known.
  record_team_task(&team.checkout, TaskId::generate());
  assert_eq!(follow(&team.checkout, project), Followed::default());
  assert!(is_watched(&team.checkout, chat));

  // The next reading covers it (the server has no such task, so it is in no module) and the watch ends.
  refreshed(&team.checkout);
  assert!(follow(&team.checkout, project).kept);
  assert_eq!(
    ends(&team.checkout, chat)[0].reason,
    EndReason::ModuleClosed { done: 1, canceled: 0 }
  );
}

#[test]
fn team_mcp_a_module_watch_reads_the_module_list_page_by_page_and_no_task_alone() {
  let team = started();
  let (chat, project) = watching(&team.checkout, |checkout| module_watch(checkout, team.server.module));
  let (second, third) = (TaskId::generate(), TaskId::generate());
  record_team_task(&team.checkout, second);
  record_team_task(&team.checkout, third);
  let module = team.server.module;
  {
    let mut seen = team.seen.lock();
    seen.page_size = 1;
    seen.more_tasks = vec![
      json!({"id": second, "number": 8, "key": "KON-8", "workflow": "done", "module": module}),
      json!({"id": third, "number": 9, "key": "KON-9", "workflow": "todo", "module": module}),
    ];
  }
  server_says(&team, "done");

  let lists_before = team.seen.lock().task_lists;
  refreshed(&team.checkout);
  assert_eq!(follow(&team.checkout, project), Followed::default());
  assert!(is_watched(&team.checkout, chat));
  {
    let seen = team.seen.lock();
    assert_eq!(
      (seen.task_lists - lists_before, seen.task_gets),
      (3, 0),
      "three pages, no task read by itself"
    );
  }

  team.seen.lock().more_tasks[1] =
    json!({"id": third, "number": 9, "key": "KON-9", "workflow": "canceled", "module": module});
  refreshed(&team.checkout);
  assert!(follow(&team.checkout, project).kept);
  assert_eq!(
    ends(&team.checkout, chat)[0].reason,
    EndReason::ModuleClosed { done: 2, canceled: 1 }
  );
}

#[test]
fn team_mcp_one_team_task_the_server_fails_on_does_not_block_the_watches_after_it() {
  let team = started();
  let broken = TaskId::generate();
  record_team_task(&team.checkout, broken);
  team.seen.lock().broken_task = Some(broken);
  // The broken task's watch comes first, as the reading goes in the order the watches are kept.
  let (stuck, project) = watching(&team.checkout, |_| WatchTarget::Task(broken));
  let (healthy, _) = watching(&team.checkout, |_| team_task(&team));
  server_says(&team, "done");

  assert!(refreshed(&team.checkout).changed);
  assert!(follow(&team.checkout, project).kept);
  assert!(!is_watched(&team.checkout, healthy), "the healthy task's watch ended");
  assert_eq!(ends(&team.checkout, healthy)[0].reason, EndReason::TaskDone);
  assert!(is_watched(&team.checkout, stuck));
  assert_eq!(
    cannot_read(&team.checkout, stuck),
    Some("orchestrator.watch.unreadable")
  );
}

#[test]
fn team_mcp_a_missing_project_is_not_a_missing_task() {
  let team = started();
  let gone = TaskId::generate();
  record_team_task(&team.checkout, gone);
  let (chat, project) = watching(&team.checkout, |_| WatchTarget::Task(gone));

  // The server holds no such task and answers for the project: the watch is left to the person, with no note.
  refreshed(&team.checkout);
  assert_eq!(follow(&team.checkout, project), Followed::default());
  assert_eq!(cannot_read(&team.checkout, chat), None);

  // The same `404` for a project or a member that is gone says nothing of the task: the note shows.
  team.seen.lock().no_member = true;
  assert!(refreshed(&team.checkout).changed);
  assert_eq!(cannot_read(&team.checkout, chat), Some("orchestrator.watch.unreadable"));
  team.seen.lock().no_member = false;
  assert!(refreshed(&team.checkout).changed);
  assert_eq!(cannot_read(&team.checkout, chat), None);
}
