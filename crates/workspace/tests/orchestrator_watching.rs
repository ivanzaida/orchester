//! ORC-762: what the project's Orchestrator conversations watch, read from stored state only — the `/watching`
//! answer (frame 06 `Ff7AD`), who leads a module and a task, with *via* (`i687ei` a and b), and what a conversation
//! leads for its header chips and History row (`X93D8T`, `i687ei` c). Run with
//! `cargo test -p orchester-workspace --test orchestrator_watching watching`.

mod support;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ModuleId, TaskId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{Chat, Subscription, WatchTarget, settings::WatchSettings},
  run::{Answerer, PlanDecision, RunLimit},
  task::organization::ChangeActor,
};
use orchester_store::{ProjectDb, WatchStart};
use orchester_workspace::{
  messages::Message,
  orchestrator::watching::{Going, LedIn, WaitsFor, WatchOrder, WatchReads},
};
use support::{Fixture, at};

fn module(db: &mut ProjectDb, name: &str) -> ModuleId {
  let details =
    ModuleDetails::new(Name::new(name).expect("name"), "", (None, None), ModuleState::Open).expect("details");
  db.create_module(details, at(1)).expect("module").id()
}

fn put(db: &mut ProjectDb, task: TaskId, module: ModuleId) {
  db.set_task_module(task, Some(module), ChangeActor::Person, at(2))
    .expect("in the module");
}

fn conversation(db: &mut ProjectDb, title: &str) -> ChatId {
  let chat = ChatId::generate();
  let body = MessageBody::new(title).expect("body");
  db.keep_chat_message(
    &Chat::start(chat, &body, at(3)),
    (ChatMessageId::generate(), &body),
    at(3),
  )
  .expect("kept");
  chat
}

fn watch(db: &mut ProjectDb, chat: ChatId, target: WatchTarget, order: &str, since: i64) {
  let started = db
    .start_subscription(&Subscription::new(chat, target, order, at(since)))
    .expect("kept");
  assert_eq!(started, WatchStart::Started);
}

fn reads(fixture: &Fixture) -> WatchReads {
  WatchReads::read(&fixture.db, fixture.repo.path()).expect("the watch reads")
}

fn keys(messages: &[Message]) -> Vec<&'static str> {
  messages.iter().map(|message| message.key).collect()
}

fn arg<'a>(message: &'a Message, name: &str) -> &'a str {
  message
    .args
    .iter()
    .find(|(arg, _)| *arg == name)
    .map_or("", |(_, value)| value.as_str())
}

/// Webhooks v2 holds a running task, one whose plan waits, a queued one and two drafts, one put in after the
/// subscription; Search index a run stopped at its limit; KON-like task Delivery log export is watched by itself and
/// ready to merge, inside Webhooks v2.
struct Project {
  fixture: Fixture,
  chat: ChatId,
  other: ChatId,
  webhooks: ModuleId,
  search: ModuleId,
  running: TaskId,
  planning: TaskId,
  export: TaskId,
}

fn project() -> Project {
  let mut fixture = Fixture::gated();
  let webhooks = module(&mut fixture.db, "Webhooks v2");
  let search = module(&mut fixture.db, "Search index");
  let first_queued = fixture.queued_task("Elsewhere first", at(5));
  let running = fixture.task("Retry failed deliveries", at(10));
  fixture.started_run(running, at(11));
  let planning = fixture.task("Delivery metrics", at(20));
  let mut plan = fixture.started_run(planning, at(21));
  fixture.propose(&mut plan, "1. Count.", at(22));
  let queued = fixture.queued_task("Dead-letter queue", at(30));
  let draft = fixture.task("Signed receipts", at(40));
  let limited = fixture.task("Reindex on schema change", at(50));
  let mut run = fixture.started_run(limited, at(51));
  let events = run
    .stop_at_limit(RunLimit::Tokens, at(52))
    .expect("stopped at its limit");
  fixture.record(&run, &events);
  let export = fixture.task("Delivery log export", at(60));
  let mut done = fixture.started_run(export, at(61));
  let mut approved = fixture.propose(&mut done, "1. Export.", at(62));
  // Approving the gated step's plan passes it, the run's one step.
  fixture.decide(
    &mut done,
    &mut approved,
    (PlanDecision::Approved, Answerer::Person),
    at(63),
  );
  for task in [running, planning, queued, draft, export] {
    put(&mut fixture.db, task, webhooks);
  }
  put(&mut fixture.db, limited, search);
  assert_ne!(first_queued, queued);

  let chat = conversation(&mut fixture.db, "Webhooks v2 and Search index");
  let other = conversation(&mut fixture.db, "Release checklist");
  watch(
    &mut fixture.db,
    chat,
    WatchTarget::Module(webhooks),
    "Answer agents from the brief.",
    100,
  );
  watch(&mut fixture.db, chat, WatchTarget::Module(search), "", 101);
  watch(
    &mut fixture.db,
    chat,
    WatchTarget::Task(export),
    "Tell me when it is ready.",
    102,
  );
  watch(&mut fixture.db, other, WatchTarget::Project, "", 103);
  let later = fixture.task("Replay deliveries", at(200));
  put(&mut fixture.db, later, webhooks);
  Project {
    fixture,
    chat,
    other,
    webhooks,
    search,
    running,
    planning,
    export,
  }
}

#[test]
fn watching_lists_each_subscription_with_what_goes_on_and_counts_the_rest() {
  let project = project();
  let watching = reads(&project.fixture).watching(project.chat);
  assert_eq!(
    watching.subscriptions.len(),
    3,
    "the conversation's own subscriptions only"
  );
  let [webhooks, search, export] = &watching.subscriptions[..] else {
    panic!("three subscriptions");
  };
  assert_eq!(webhooks.target.target, WatchTarget::Module(project.webhooks));
  assert_eq!(arg(&webhooks.target.name, "name"), "Webhooks v2");
  assert_eq!(webhooks.target.kind.key, "orchestrator.watch.kind.module");
  assert_eq!(webhooks.since, at(100));
  assert_eq!(webhooks.order, WatchOrder::Own("Answer agents from the brief.".into()));
  assert_eq!(
    search.order,
    WatchOrder::None,
    "no order of its own and no default order"
  );

  let going: Vec<(&str, Going)> = webhooks.tasks.iter().map(|task| (&*task.title, task.going)).collect();
  assert_eq!(
    going,
    [
      ("Retry failed deliveries", Going::Running),
      ("Delivery metrics", Going::Waits(WaitsFor::Plan)),
      ("Dead-letter queue", Going::Queued { position: 2 }),
    ],
    "the task watched by itself is listed under its own subscription only"
  );
  let rest = webhooks.rest.as_ref().expect("the drafts are counted");
  assert_eq!(rest.key, "orchestrator.watch.rest.other");
  assert_eq!(
    arg(rest, "count"),
    "2",
    "a task put in after the subscription is the module's too"
  );
  let plan = &webhooks.tasks[1];
  assert_eq!(plan.id, project.planning);
  assert_eq!(plan.state.key, "orchestrator.watch.task.state.waits");
  assert_eq!(
    plan.note.as_ref().map(|note| note.key),
    Some("orchestrator.watch.task.note.plan")
  );
  assert_eq!(
    plan.action.as_ref().map(|action| action.key),
    Some("orchestrator.watch.task.action.decide")
  );
  assert!(
    plan.link.ends_with("/conversation"),
    "the plan is decided on its Conversation: {}",
    plan.link
  );
  assert_eq!(webhooks.tasks[0].id, project.running);

  assert_eq!(
    search.tasks.iter().map(|task| task.going).collect::<Vec<_>>(),
    [Going::StoppedAtLimit { daily_budget: false }]
  );
  assert_eq!(search.rest, None);
  assert_eq!(search.target.target, WatchTarget::Module(project.search));
  assert_eq!(
    export
      .tasks
      .iter()
      .map(|task| (task.id, task.going))
      .collect::<Vec<_>>(),
    [(project.export, Going::ReadyToMerge)]
  );

  assert_eq!(
    keys(&watching.total),
    [
      "orchestrator.watch.total.waits.one",
      "orchestrator.watch.total.limits.one"
    ],
    "1 thing waits for you, 1 run is stopped at a limit"
  );
  assert_eq!(
    keys(&watching.counts.parts()),
    [
      "orchestrator.watch.leads.modules.other",
      "orchestrator.watch.leads.tasks.one"
    ]
  );
}

#[test]
fn watching_of_a_conversation_that_watches_nothing_says_so() {
  let mut project = project();
  let idle = conversation(&mut project.fixture.db, "Nothing here");
  let watching = reads(&project.fixture).watching(idle);
  assert!(watching.subscriptions.is_empty());
  assert!(watching.counts.is_empty());
  assert_eq!(keys(&watching.total), ["orchestrator.watch.total.nothing"]);
}

#[test]
fn watching_takes_the_projects_default_order_for_a_subscription_without_one() {
  let mut project = project();
  project
    .fixture
    .db
    .save_orchestrator_watch(&WatchSettings::new("Ask me first.", None).expect("settings"))
    .expect("kept");
  let watching = reads(&project.fixture).watching(project.chat);
  assert_eq!(
    watching.subscriptions[1].order,
    WatchOrder::ProjectDefault("Ask me first.".into())
  );
}

#[test]
fn watching_who_leads_a_module_and_a_task_with_via() {
  let project = project();
  let reads = reads(&project.fixture);
  let LedIn::Chat(module) = reads.module_led_in(project.webhooks) else {
    panic!("the module is led");
  };
  assert_eq!(module.chat, project.chat);
  assert_eq!(&*module.title, "Webhooks v2 and Search index");
  assert_eq!(module.via, None);
  assert_eq!(module.words.key, "orchestrator.watch.ledIn.chat");
  assert!(module.link.contains(&project.chat.to_string()), "{}", module.link);

  let LedIn::Chat(task) = reads.task_led_in(project.running) else {
    panic!("the task is led");
  };
  assert_eq!(task.chat, project.chat);
  assert_eq!(task.words.key, "orchestrator.watch.ledIn.viaModule");
  assert_eq!(arg(&task.words, "module"), "Webhooks v2");
  assert_eq!(
    task.via.map(|via| via.target),
    Some(WatchTarget::Module(project.webhooks))
  );

  let LedIn::Chat(own) = reads.task_led_in(project.export) else {
    panic!("the task is led");
  };
  assert_eq!(own.via, None, "a task subscription shows no via");
}

#[test]
fn watching_a_task_outside_every_module_is_led_through_the_project_or_by_nobody() {
  let mut project = project();
  let loose = project.fixture.task("Loose end", at(300));
  let reads_now = reads(&project.fixture);
  let LedIn::Chat(leader) = reads_now.task_led_in(loose) else {
    panic!("the project's subscription covers it");
  };
  assert_eq!(leader.chat, project.other);
  assert_eq!(leader.words.key, "orchestrator.watch.ledIn.viaProject");
  project
    .fixture
    .db
    .stop_subscription(project.other, WatchTarget::Project, at(400))
    .expect("stopped");
  let reads_after = reads(&project.fixture);
  assert_eq!(reads_after.task_led_in(loose), LedIn::Nobody);
  assert_eq!(
    reads_after.task_led_in(loose).message().key,
    "orchestrator.watch.ledIn.nobody"
  );
}

#[test]
fn watching_what_a_conversation_leads_in_order_with_counts() {
  let project = project();
  let reads = reads(&project.fixture);
  let leads = reads.chat_leads(project.chat);
  assert_eq!(
    leads.targets.iter().map(|chip| chip.target.target).collect::<Vec<_>>(),
    [
      WatchTarget::Module(project.webhooks),
      WatchTarget::Module(project.search),
      WatchTarget::Task(project.export)
    ]
  );
  assert_eq!((leads.counts.modules, leads.counts.tasks), (2, 1));
  assert_eq!(leads.targets[2].since, at(102));
  let other = reads.chat_leads(project.other);
  assert_eq!(keys(&other.counts.parts()), ["orchestrator.watch.leads.project"]);
  assert_eq!(other.targets[0].target.kind.key, "orchestrator.watch.kind.project");
}
