//! Holding new starts from the chat and from Tasks: the Orchestrator's agent holds the project's queue through the
//! same tool as any client of the app, Tasks says new starts are held, and *Release* there lets them go on.

use orchester_app::routes::{OrchestratorChat, ProjectPage, Route, TaskList};
use orchester_core::orchestrator::{ActionOutcome, EntryKind};

use super::harness::{Orchestrator, Setup};

/// Whether new starts are held in the whole project.
fn held(screen: &Orchestrator) -> bool {
  screen.db().queue_holds().expect("holds").project
}

#[test]
fn orchestrator_holds_new_starts_and_tasks_releases_them() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.send(r#"Call hold_queue {"project":"Kontur"}"#);
  screen.until("the held queue", |screen| {
    screen.text("orchestrator-conversation").contains("New starts held")
  });
  assert!(held(&screen));
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  let done = screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .any(|entry| matches!(entry.kind, EntryKind::Action(action) if action.tool == "hold_queue" && action.outcome == ActionOutcome::Done));
  assert!(done, "the action is kept as done");
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));

  let project = screen.project;
  screen
    .window
    .navigation()
    .go(Route::Project(project, ProjectPage::Tasks(TaskList::default())));
  screen.until("Tasks saying so", |screen| {
    screen.text("tasks-queue-held").contains("New starts held")
  });
  screen.click("tasks-queue-release");
  screen.until("released", |screen| !held(screen));
  screen.until("Hold new starts again", |screen| screen.exists("tasks-queue-hold"));
  screen.click("tasks-queue-hold");
  screen.until("held from Tasks", |screen| held(screen));

  // A person's hold is the person's to release: the agent's release is refused and changes nothing.
  screen.go(OrchestratorChat::Latest);
  screen.send(r#"Call release_queue {"project":"Kontur"}"#);
  screen.until("the refusal", |screen| {
    screen.text("orchestrator-conversation").contains("person_only")
  });
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  assert!(held(&screen), "still held");
}

#[test]
fn orchestrator_releases_a_hold_it_set_itself() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.send(r#"Call hold_queue {"project":"Kontur"}"#);
  screen.until("the hold", |screen| held(screen));
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  screen.send(r#"Call release_queue {"project":"Kontur"}"#);
  screen.until("the release", |screen| !held(screen));
}

/// Queues a reviewed group of `keys`' drafts on `delivery` while the project is held, so nothing of it starts; holds
/// the group as `actor` and lets the project go on. The group's key.
fn held_group(
  screen: &Orchestrator,
  delivery: orchester_core::id::ResourceId,
  keys: &[String],
) -> orchester_core::id::IdempotencyKey {
  use orchester_core::{
    id::IdempotencyKey,
    resource::Pipeline,
    store::{ResourceStore, TaskStore},
    task_group::{HoldActor, HoldScope, ReviewedPipeline, Scheduling, TaskGroup},
    time::Timestamp,
    tokens::TokenBudget,
  };

  let mut db = screen.db();
  db.set_queue_hold((HoldScope::Project, HoldActor::Person), true, Timestamp::now())
    .expect("held");
  let tasks: Vec<_> = keys
    .iter()
    .map(|key| {
      db.tasks()
        .expect("tasks")
        .into_iter()
        .find(|task| task.key().to_string() == *key)
        .expect("the draft")
        .id()
    })
    .collect();
  let version = db
    .latest_version::<Pipeline>(delivery)
    .expect("read")
    .expect("published")
    .number();
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    tasks.clone(),
    Scheduling::Sequential,
    TokenBudget::new(1_000_000).expect("cap"),
  )
  .expect("group");
  let reviewed: Vec<_> = tasks
    .iter()
    .map(|task| ReviewedPipeline {
      task: *task,
      pipeline: delivery,
      version,
    })
    .collect();
  db.queue_task_group(&group, None, &reviewed).expect("queued");
  db.set_queue_hold(
    (HoldScope::Group(group.key()), HoldActor::Automation),
    true,
    Timestamp::now(),
  )
  .expect("group held");
  db.set_queue_hold((HoldScope::Project, HoldActor::Person), false, Timestamp::now())
    .expect("project released");
  group.key()
}

#[test]
fn tasks_show_held_groups_under_the_tabs_and_keep_the_header() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let project = screen.project;
  let delivery = screen.delivery();
  let keys = ["First", "Second"].map(|title| screen.draft_on(delivery, title));
  screen
    .window
    .navigation()
    .go(Route::Project(project, ProjectPage::Tasks(TaskList::default())));
  screen.until("Tasks", |screen| {
    screen.exists("tasks-queue-hold") && screen.exists("tasks-tab-all")
  });
  let tabs = screen.bounds("tasks-tab-all").y;
  held_group(&screen, delivery, &keys);

  screen.until("the held group", |screen| screen.exists("tasks-queue-group-0"));
  assert_eq!(screen.bounds("tasks-tab-all").y, tabs, "the header keeps its layout");
  assert!(
    screen.bounds("tasks-queue-group-0").y > tabs,
    "the group's line is under the tabs"
  );
  assert!(screen.text("tasks-queue-group-0").contains("KON-1, KON-2"));
  screen.click("tasks-queue-release-group-0");
  screen.until("released", |screen| {
    screen.db().queue_holds().expect("holds").groups.is_empty()
  });
}

#[test]
fn orchestrator_hold_card_names_the_group_its_tasks_and_resumes() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let delivery = screen.delivery();
  let keys = ["First", "Second"].map(|title| screen.draft_on(delivery, title));
  let group = held_group(&screen, delivery, &keys);
  screen.go(OrchestratorChat::New);
  screen.send(r#"Call hold_queue {"project":"Kontur","task":"KON-1"}"#);
  screen.until("the card", |screen| {
    screen.text("orchestrator-conversation").contains("Resume new starts")
  });
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  let card = screen.text("orchestrator-conversation");
  assert!(card.contains("New starts held · the group of KON-1"), "{card}");
  assert!(
    card.contains("2 waiting tasks held") && card.contains("KON-2"),
    "{card}"
  );
  assert!(
    !card.contains("New starts held in Kontur"),
    "a group hold is not the project's: {card}"
  );
  let db = screen.db();
  let chat = db.chats().expect("chats").remove(0).chat.id();
  let turn = db
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .find_map(|entry| match entry.kind {
      EntryKind::Person { message, .. } => Some(message),
      _ => None,
    })
    .expect("the message");
  let resume = format!("orchestrator-action-{turn}-0-resume");
  screen.click(&resume);
  screen.until("resumed", |screen| {
    !screen.db().queue_holds().expect("holds").groups.contains(&group)
  });
}
