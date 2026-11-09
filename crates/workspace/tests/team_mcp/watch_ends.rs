//! A watch on a team task ends on the team server's status (`orchestrator::team_status`, `orchestrator::watch_ends`):
//! this computer keeps a team task only as an identity and its runs, so the status is read from the server in the
//! background. A reading that cannot be made leaves the watch standing and says why in its listing; a local task's
//! watch ends on its stored row as before.

use std::time::Duration;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ModuleId, ProjectId},
  module::{Module, ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{
    Chat, EntryKind, Subscription, TurnState, WatchTarget,
    watch::{EndReason, WatchSummary},
  },
  secret::{SecretValue, SecretVault, VaultKey},
  sharing::TaskRevision,
  store::{RunStore, TaskStore},
  task::organization::{ChangeActor, WorkflowStatus},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::CachedModule;
use orchester_workspace::orchestrator::{
  team_status::{Refreshed, TeamReading, refresh},
  waits::{Followed, Switches, check_merges_in, follow_in},
  watching::WatchReads,
};
use serde_json::json;

use crate::{
  Checkout,
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
