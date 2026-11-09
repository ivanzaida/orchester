//! ORC-763: what the /watch pickers offer (`docs/design/orchestrator-watch.md`, frames 02 `c8pALl` and 03 `mQdxa`):
//! the open modules and the tasks that match what was typed, with their counts and statuses and who leads each,
//! and the targets with nothing left to watch faded with their reason. Run with
//! `cargo test -p orchester-workspace --test watch_pickers watch_pickers`.

mod support;

use std::sync::Arc;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ModuleId, TaskId},
  module::{Module, ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{Chat, Subscription, Subscriptions, TurnState, WatchTarget, watch::EndReason},
  task::organization::{ChangeActor, WorkflowStatus},
  time::Timestamp,
};
use orchester_workspace::{
  mentions::watch::{Choosing, Leader, Leaders, WatchCatalog, WatchTask, modules::Counts},
  tasks::data::ProjectTasks,
};

fn module(name: &str, state: ModuleState) -> Module {
  let name = Name::new(name).expect("a name");
  let details = ModuleDetails::new(name, "", (None, None), state).expect("details");
  Module::new(ModuleId::generate(), details, Timestamp::from_unix_millis(1))
}

/// Task `KON-<number>` titled `title`, changed at `at`.
fn task(number: u32, title: &str, module: Option<&Module>, status: WorkflowStatus, at: i64) -> WatchTask {
  WatchTask {
    id: TaskId::generate(),
    key: format!("KON-{number}").parse().expect("a task key"),
    title: title.into(),
    module: module.map(Module::id),
    status,
    running: false,
    waits: false,
    at: Timestamp::from_unix_millis(at),
  }
}

fn watch(chat: ChatId, target: WatchTarget) -> Subscription {
  Subscription::new(chat, target, "", Timestamp::from_unix_millis(5))
}

/// Webhooks v2 (KON-41 running, KON-42 waits for the person, KON-43 Done), Mobile release (KON-47 Done, KON-48
/// Canceled), the closed Billing (KON-50), and KON-52 without a module, running. *Retry plan* leads Webhooks v2 and
/// KON-52; this conversation leads the tasks without a module.
struct Kontur {
  catalog: WatchCatalog,
  here: ChatId,
  other: ChatId,
  webhooks: ModuleId,
  mobile: ModuleId,
}

fn kontur() -> Kontur {
  let webhooks = module("Webhooks v2", ModuleState::Open);
  let mobile = module("Mobile release", ModuleState::Open);
  let billing = module("Billing", ModuleState::Closed);
  let mut tasks = vec![
    WatchTask {
      running: true,
      ..task(
        41,
        "Retry failed deliveries",
        Some(&webhooks),
        WorkflowStatus::InProgress,
        30,
      )
    },
    WatchTask {
      waits: true,
      ..task(
        42,
        "Sign webhook payloads",
        Some(&webhooks),
        WorkflowStatus::InProgress,
        20,
      )
    },
    task(43, "Webhook docs", Some(&webhooks), WorkflowStatus::Done, 40),
    task(47, "Store screenshots", Some(&mobile), WorkflowStatus::Done, 50),
    task(48, "Tablet layout", Some(&mobile), WorkflowStatus::Canceled, 10),
    task(50, "Invoice export", Some(&billing), WorkflowStatus::Todo, 15),
  ];
  tasks.push(WatchTask {
    running: true,
    ..task(
      52,
      "Shared rate limiter for deliveries",
      None,
      WorkflowStatus::InProgress,
      60,
    )
  });
  let (here, other) = (ChatId::generate(), ChatId::generate());
  let subscriptions = Subscriptions::new([
    watch(other, WatchTarget::Module(webhooks.id())),
    watch(other, WatchTarget::Task(tasks[6].id)),
    watch(here, WatchTarget::WithoutModule),
  ])
  .expect("one leader each");
  let leaders = Leaders::new(subscriptions, [(other, Arc::from("Retry plan"))]);
  Kontur {
    here,
    other,
    webhooks: webhooks.id(),
    mobile: mobile.id(),
    catalog: WatchCatalog {
      modules: vec![webhooks, mobile, billing],
      tasks,
      leaders,
    },
  }
}

fn elsewhere(kontur: &Kontur, via: Option<WatchTarget>) -> Leader {
  Leader::Elsewhere {
    chat: kontur.other,
    title: Some("Retry plan".into()),
    via,
  }
}

#[test]
fn watch_pickers_offer_open_modules_with_counts_then_the_whole_targets() {
  let kontur = kontur();
  let choosing = Choosing {
    chat: kontur.here,
    added: &[WatchTarget::Project],
  };
  let rows = kontur.catalog.modules("", choosing);
  let targets: Vec<WatchTarget> = rows.iter().map(|row| row.target).collect();
  assert_eq!(
    targets,
    [
      WatchTarget::Module(kontur.webhooks),
      WatchTarget::Module(kontur.mobile),
      WatchTarget::WithoutModule,
      WatchTarget::Project
    ],
    "the closed Billing is not offered; the whole targets come last"
  );
  assert_eq!(rows[0].name.as_deref(), Some("Webhooks v2"));
  assert_eq!(
    rows[0].counts,
    Counts {
      tasks: 3,
      running: 1,
      waiting: 1
    }
  );
  assert_eq!(rows[0].leader, elsewhere(&kontur, None));
  assert_eq!(rows[0].faded, None);
  assert_eq!(
    rows[1].faded,
    Some(EndReason::ModuleClosed { done: 1, canceled: 1 }),
    "every task of Mobile release is closed: nothing left to watch"
  );
  assert_eq!(rows[1].leader, Leader::Nobody);
  assert_eq!(rows[2].name, None);
  assert_eq!(
    rows[2].counts,
    Counts {
      tasks: 1,
      running: 1,
      waiting: 0
    }
  );
  assert_eq!(rows[2].leader, Leader::Here { via: None });
  assert_eq!(rows[3].counts.tasks, 7);
  assert_eq!(rows[3].leader, Leader::JustAdded, "the command chose it already");

  let typed = kontur.catalog.modules("WEB", choosing);
  let targets: Vec<WatchTarget> = typed.iter().map(|row| row.target).collect();
  assert_eq!(
    targets,
    [
      WatchTarget::Module(kontur.webhooks),
      WatchTarget::WithoutModule,
      WatchTarget::Project
    ],
    "modules by name, ignoring case"
  );
}

#[test]
fn watch_pickers_faded_modules_follow_those_left_to_watch() {
  let mut kontur = kontur();
  kontur.catalog.modules.swap(0, 1);
  let rows = kontur.catalog.modules(
    "",
    Choosing {
      chat: kontur.here,
      added: &[],
    },
  );
  assert_eq!(rows[0].target, WatchTarget::Module(kontur.webhooks));
  assert_eq!(rows[1].target, WatchTarget::Module(kontur.mobile));
  assert!(rows[1].faded.is_some());
}

#[test]
fn watch_pickers_offer_tasks_by_key_or_title_with_who_leads_them() {
  let kontur = kontur();
  let choosing = Choosing {
    chat: kontur.here,
    added: &[WatchTarget::Task(kontur.catalog.tasks[5].id)],
  };
  let rows = kontur.catalog.tasks("", choosing);
  let keys: Vec<String> = rows.iter().map(|row| row.key.to_string()).collect();
  assert_eq!(
    keys,
    ["KON-52", "KON-41", "KON-42", "KON-50", "KON-47", "KON-43", "KON-48"],
    "most recently changed first, the Done and Canceled ones last"
  );
  let row = |key: &str| rows.iter().find(|row| row.key.to_string() == key).expect("offered");
  assert_eq!(row("KON-52").leader, elsewhere(&kontur, None), "led itself elsewhere");
  assert_eq!(row("KON-52").module, None);
  assert_eq!(
    row("KON-41").leader,
    elsewhere(&kontur, Some(WatchTarget::Module(kontur.webhooks))),
    "led elsewhere through its module"
  );
  assert_eq!(
    row("KON-41").module.as_ref().map(|module| &*module.name),
    Some("Webhooks v2")
  );
  assert_eq!(row("KON-41").status, WorkflowStatus::InProgress);
  assert_eq!(row("KON-50").leader, Leader::JustAdded);
  assert_eq!(row("KON-47").faded, Some(EndReason::TaskDone));
  assert_eq!(row("KON-48").faded, Some(EndReason::TaskCanceled));
  assert_eq!(row("KON-41").faded, None);

  let by_title = kontur.catalog.tasks("deliveries", choosing);
  let keys: Vec<String> = by_title.iter().map(|row| row.key.to_string()).collect();
  assert_eq!(keys, ["KON-52", "KON-41"], "by title, ignoring case");
  let by_key = kontur.catalog.tasks("kon-4", choosing);
  assert_eq!(by_key.len(), 5, "by key");
  assert!(kontur.catalog.tasks("payroll", choosing).is_empty());
}

#[test]
fn watch_pickers_tasks_come_here_through_this_conversations_subscription() {
  let mut kontur = kontur();
  let without = task(60, "Clean old logs", None, WorkflowStatus::Todo, 70);
  let id = without.id;
  kontur.catalog.tasks.push(without);
  let rows = kontur.catalog.tasks(
    "KON-60",
    Choosing {
      chat: kontur.here,
      added: &[],
    },
  );
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0].id, id);
  assert_eq!(
    rows[0].leader,
    Leader::Here {
      via: Some(WatchTarget::WithoutModule)
    },
    "comes here already through the tasks without a module"
  );
}

#[test]
fn watch_pickers_read_the_project_and_its_leaders() {
  let mut fixture = support::Fixture::new();
  let running = fixture.task("Retry failed deliveries", support::at(10));
  let idle = fixture.task("Webhook docs", support::at(20));
  let details = ModuleDetails::new(
    Name::new("Webhooks v2").expect("a name"),
    "",
    (None, None),
    ModuleState::Open,
  )
  .expect("details");
  let webhooks = fixture.db.create_module(details, support::at(25)).expect("created");
  for id in [running, idle] {
    fixture
      .db
      .set_task_module(id, Some(webhooks.id()), ChangeActor::Person, support::at(26))
      .expect("moved");
  }
  fixture.started_run(running, support::at(30));
  let chat = Chat::named(ChatId::generate(), "Retry plan", support::at(40));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Watch the webhooks").expect("text");
  fixture
    .db
    .keep_chat_message(&chat, (person, &text), support::at(40))
    .expect("kept");
  fixture
    .db
    .end_chat_turn(person, &TurnState::Answered, support::at(41))
    .expect("ended");
  fixture
    .db
    .start_subscription(&watch(chat.id(), WatchTarget::Module(webhooks.id())))
    .expect("started");

  let tasks = ProjectTasks::read(&fixture.db, fixture.repo.path(), None).expect("tasks");
  let leaders = Leaders::read(&fixture.db).expect("leaders");
  let catalog = WatchCatalog::of(&tasks, leaders);
  let rows = catalog.modules(
    "",
    Choosing {
      chat: ChatId::generate(),
      added: &[],
    },
  );
  assert_eq!(rows[0].target, WatchTarget::Module(webhooks.id()));
  assert_eq!(rows[0].counts.tasks, 2);
  assert_eq!(rows[0].counts.running, 1, "the started run runs");
  assert_eq!(
    rows[0].leader,
    Leader::Elsewhere {
      chat: chat.id(),
      title: Some("Retry plan".into()),
      via: None
    }
  );
}
