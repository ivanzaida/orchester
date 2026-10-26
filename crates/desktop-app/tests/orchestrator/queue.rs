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
  // A project without tasks shows its title alone: one task, read in, brings the list and its holds.
  let delivery = screen.delivery();
  screen.draft_on(delivery, "First");
  screen.window.projects().feed(project).saved();
  screen
    .window
    .navigation()
    .go(Route::Project(project, ProjectPage::Tasks(TaskList::default())));
  screen.until("Tasks saying so", |screen| {
    screen.exists("tasks-queue-held") && screen.text("tasks-queue-held").contains("New starts on hold")
  });
  screen.click("tasks-queue-release");
  screen.until("released", |screen| !held(screen));
  // Hold new starts is in the header's More actions.
  screen.until("the header's menu", |screen| screen.exists("tasks-more"));
  screen.click("tasks-more");
  screen.until("Hold new starts again", |screen| screen.exists("tasks-queue-hold"));
  screen.click("tasks-queue-hold");
  screen.until("held from Tasks", |screen| held(screen));
  // Automation driving the window is held to automation's rule: it does not release the person's hold.
  screen.until("Release", |screen| screen.exists("tasks-queue-release"));
  lurq::app::events::with_synthetic_input(|| screen.click("tasks-queue-release"));
  screen.until("the refusal", |screen| screen.exists("tasks-queue-error"));
  assert!(held(&screen), "still held");

  // A person's hold is the person's to release: the agent's release is refused and changes nothing.
  screen.go(OrchestratorChat::Latest);
  screen.send(r#"Call release_queue {"project":"Kontur"}"#);
  screen.until("the refusal", |screen| {
    screen.text("orchestrator-conversation").contains("person_only")
  });
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  assert!(held(&screen), "still held");
  assert!(
    screen.text("orchestrator-conversation").contains("Open queue"),
    "the refused card opens the queue"
  );
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

#[test]
fn orchestrator_release_proposed_in_a_turn_on_its_own_is_applied_by_the_person() {
  use orchester_core::{
    orchestrator::{
      ChatAction,
      change::{Change, ProposedChange},
    },
    time::Timestamp,
  };

  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.send(r#"Call hold_queue {"project":"Kontur"}"#);
  screen.until("the hold", |screen| held(screen));
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  // A turn a run's end started by itself proposed letting new starts go on, as the chat keeps it.
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  let release = ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: String::new(),
      change: Change::Release { task: None },
    })),
    ..ChatAction::new("release_queue", None, ActionOutcome::Waits)
  };
  screen
    .db()
    .add_chat_action(chat, &release, Timestamp::now())
    .expect("kept");
  // The conversation is told it changed, as the chat's tools tell it.
  let root = screen.root.clone();
  screen
    .window
    .launch
    .services()
    .expect("the session started")
    .runners
    .chat_changed(&root);
  // The card is the turn's last part, after the hold and the agent's reply.
  let parts = screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .rev()
    .take_while(|entry| !matches!(entry.kind, EntryKind::Person { .. }))
    .count();
  let turn = super::proposal::proposal_card(&screen);
  let card = format!("{}-{}", turn.trim_end_matches("-0"), parts - 1);
  screen.until("the release card", |screen| screen.exists(&format!("{card}-start")));
  // A change with nothing to read takes the person's click once the card read it: nothing widens. Pressed until it
  // takes it (disabled while the card reads).
  let start = format!("{card}-start");
  screen.until("the release", |screen| {
    if screen.exists(&start) {
      screen.click(&start);
    }
    !held(screen)
  });
  screen.until("the card applied", |screen| {
    super::proposal::outcomes(screen).pop() == Some(ActionOutcome::Applied)
  });
}

/// Queues a reviewed group of `keys`' drafts on `delivery` while a person holds the project, so nothing of it starts;
/// the project stays held. The group's key.
fn queued_group(
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
  group.key()
}

/// [`queued_group`], with the group held by automation and the project let go on. The group's key.
fn held_group(
  screen: &Orchestrator,
  delivery: orchester_core::id::ResourceId,
  keys: &[String],
) -> orchester_core::id::IdempotencyKey {
  use orchester_core::{
    task_group::{HoldActor, HoldScope},
    time::Timestamp,
  };

  let group = queued_group(screen, delivery, keys);
  let mut db = screen.db();
  db.set_queue_hold((HoldScope::Group(group), HoldActor::Automation), true, Timestamp::now())
    .expect("group held");
  db.set_queue_hold((HoldScope::Project, HoldActor::Person), false, Timestamp::now())
    .expect("project released");
  group
}

#[test]
fn tasks_show_held_groups_under_the_views_and_keep_the_header() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let project = screen.project;
  let delivery = screen.delivery();
  let keys = ["First", "Second"].map(|title| screen.draft_on(delivery, title));
  screen
    .window
    .navigation()
    .go(Route::Project(project, ProjectPage::Tasks(TaskList::default())));
  screen.until("Tasks", |screen| {
    screen.exists("tasks-more") && screen.exists("tasks-view-all")
  });
  let views = screen.bounds("tasks-view-all").y;
  held_group(&screen, delivery, &keys);

  screen.until("the held group", |screen| screen.exists("tasks-queue-group-0"));
  assert_eq!(screen.bounds("tasks-view-all").y, views, "the header keeps its layout");
  assert!(
    screen.bounds("tasks-queue-group-0").y > views,
    "the group's line is under the views"
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
  // The project stays held by the person, so nothing of the group starts before the chat holds it.
  let group = queued_group(&screen, delivery, &keys);
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
  // Once a person holds it too, automation driving the window cannot resume it; the person can.
  db_person_hold(&screen, group);
  lurq::app::events::with_synthetic_input(|| screen.click(&resume));
  screen.until("the refusal", |screen| {
    screen.exists(&format!("orchestrator-action-{turn}-0-error"))
  });
  assert!(screen.db().queue_holds().expect("holds").groups.contains(&group));
  screen.click(&resume);
  screen.until("resumed", |screen| {
    !screen.db().queue_holds().expect("holds").groups.contains(&group)
  });
}

/// A person holds `group`'s new starts as well, which makes the hold the person's.
fn db_person_hold(screen: &Orchestrator, group: orchester_core::id::IdempotencyKey) {
  use orchester_core::{
    task_group::{HoldActor, HoldScope},
    time::Timestamp,
  };
  screen
    .db()
    .set_queue_hold((HoldScope::Group(group), HoldActor::Person), true, Timestamp::now())
    .expect("held by a person");
}
