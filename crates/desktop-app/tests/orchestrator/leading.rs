//! Leading to the end, as a conversation shows it (the board `Orchestrator autonomy · frames to approve`: `L01`
//! `QMwUB`, `Parts · Lines` `e6nsBS`, `Parts · Before you leave` `k0jNa`, `Parts · An event whose turn failed or is
//! held` `tkjdw`): the line and the header chip of a target led to the end, *Before you leave* under that line and
//! *Check again*, nothing of it for a target that is only followed; the lines of the events of the target as a whole,
//! with what each adds and where it leads; and under an event whose turn failed, when it is
//! tried again, that it was left to the person with *Try again*, a person's press only, and that it is held, with
//! the header's quiet line.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  connection::AgentConnection,
  id::{ChatId, ChatMessageId, TaskId},
  module::{
    ModuleDetails, ModuleState,
    delivery::{OwnBranch, PersonsDelivery},
  },
  name::Name,
  orchestrator::{
    Chat, ChatEvent, EntryKind, PlanHold, Subscription, TurnFailure, TurnState, WatchTarget,
    standing::{Left, Raised, StandingEvent},
    watch::NamedTarget,
  },
  run::BranchName,
  store::{ConnectionStore, ResourceStore, TaskStore},
  task::organization::ChangeActor,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::{
  waits::{Followed, MergeChecks, Switches, follow_in},
  watch::watch_in,
};

use super::{
  harness::{Orchestrator, Setup},
  standing::waiting,
};

/// A module named `name`, holding `task` when one is given, as a target named as people read it.
fn module(db: &mut ProjectDb, name: &str, task: Option<TaskId>) -> NamedTarget {
  let details = ModuleDetails::new(Name::new(name).expect("name"), "", (None, None), ModuleState::Open);
  let module = db
    .create_module(details.expect("details"), Timestamp::now())
    .expect("module");
  if let Some(task) = task {
    db.set_task_module(task, Some(module.id()), ChangeActor::Person, Timestamp::now())
      .expect("moved");
  }
  NamedTarget {
    target: WatchTarget::Module(module.id()),
    name: Some(name.to_owned()),
  }
}

/// The rows of the watch lines `chat` keeps, oldest first.
fn lines(screen: &Orchestrator, chat: ChatId) -> Vec<String> {
  let entries = screen.db().chat_entries(chat).expect("entries");
  entries
    .iter()
    .filter(|entry| matches!(entry.kind, EntryKind::Event(ChatEvent::Watch { .. })))
    .map(|entry| format!("orchestrator-watch-wait-{}", entry.seq))
    .collect()
}

/// What the lines of the *Before you leave* card `card` say, findings first, then what was not checked.
fn findings(screen: &mut Orchestrator, card: &str) -> (Vec<String>, Vec<String>) {
  let said = |screen: &mut Orchestrator, kind: &str| -> Vec<String> {
    let ids = (0..12).map(|at| format!("{card}-{kind}-{at}-words"));
    ids
      .filter_map(|id| screen.exists(&id).then(|| screen.text(&id)))
      .collect()
  };
  (said(screen, "finding"), said(screen, "not-checked"))
}

#[test]
fn orchestrator_leading_to_the_end_shows_on_its_line_its_chip_and_before_you_leave() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  screen.let_orchestrator_drive(false);
  let delivery = screen.delivery();
  let key = screen.draft_on(delivery, "Retry failed deliveries");
  let mut db = screen.db();
  let tasks = db.tasks().expect("tasks");
  let task = tasks
    .iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  let led = module(&mut db, "Webhooks v2", Some(task.id()));
  let followed = module(&mut db, "Search index", None);
  drop(db);
  let chat = Chat::named(ChatId::generate(), "Webhooks v2 and Search index", Timestamp::now());
  let order = "Lead to the end: answer agents from the brief.";
  watch_in(&screen.root, &chat, std::slice::from_ref(&led), order).expect("led to the end");
  watch_in(&screen.root, &chat, std::slice::from_ref(&followed), "").expect("followed");
  let lines = lines(&screen, chat.id());
  let [leading, following] = lines.as_slice() else {
    panic!("two lines: {lines:?}");
  };

  screen.go(OrchestratorChat::Chat(chat.id()));
  screen.until("the line of the target led to the end", |screen| {
    screen.text(&format!("{leading}-words")) == "Leading Webhooks v2 to the end · 1 task"
  });
  assert_eq!(
    screen.text(&format!("{leading}-aside")),
    "— it starts them, merges them and parks what needs you"
  );
  assert!(
    screen
      .text(&format!("{following}-words"))
      .starts_with("Leading Search index")
  );
  assert_eq!(
    screen.text(&format!("{following}-aside")),
    "— their waits, run ends, conflicts, limits and stalls start a turn here"
  );

  // The chips: the one led to the end says so, the followed one stays as it was.
  let chip = |target: &NamedTarget| match target.target {
    WatchTarget::Module(id) => format!("orchestrator-leads-module-{id}"),
    _ => unreachable!("a module"),
  };
  screen.until("the chips", |screen| screen.exists(&chip(&followed)));
  assert_eq!(screen.text(&format!("{}-mode", chip(&led))), "to the end");
  assert!(!screen.exists(&format!("{}-mode", chip(&followed))));

  // Before you leave, under the leading line only.
  let card = format!("{leading}-leaving-0");
  screen.until("what would stop the work", |screen| {
    screen.exists(&format!("{card}-count"))
  });
  assert!(
    !screen.exists(&format!("{following}-leaving-0")),
    "nothing for a followed target"
  );
  let (found, unchecked) = findings(&mut screen, &card);
  assert_eq!(
    found.first().map(String::as_str),
    Some("Let the Orchestrator drive is off")
  );
  assert_eq!(screen.text(&format!("{card}-finding-0-place")), "Project settings ↗");
  assert!(found.contains(&"The computer may go to sleep".to_owned()), "{found:?}");
  assert_eq!(
    found.last().map(String::as_str),
    Some("Closing the window stops the work")
  );
  assert_eq!(unchecked, ["Not checked: whether the tasks' MCP servers answer"]);
  // Closing the window is not counted: it is what the person must not do.
  let stopping = found.len() - 1;
  assert_eq!(
    screen.text(&format!("{card}-count")),
    format!("{stopping} things can stop Webhooks v2 while you are away")
  );

  // Check again reads the state anew.
  screen.let_orchestrator_drive(true);
  screen.click(&format!("{card}-again"));
  screen.until("the switch's line gone", |screen| {
    screen.text(&format!("{card}-finding-0-words")) != "Let the Orchestrator drive is off"
  });
  assert_eq!(findings(&mut screen, &card).0.len(), found.len() - 1);
}

/// A conversation that leads everything in the project, as the person's watch of *Everything* makes it.
fn everything(screen: &Orchestrator) -> Chat {
  let mut db = screen.db();
  let now = Timestamp::now();
  let chat = Chat::named(ChatId::generate(), "Everything", now);
  db.keep_watch_entries(&chat, &[], now).expect("the conversation");
  db.start_subscription(&Subscription::new(chat.id(), WatchTarget::Project, "", now))
    .expect("watching");
  chat
}

/// Follows the project's waits as the app's follower does while the chat drives, its runner attached.
fn follow_driving(screen: &Orchestrator) -> Followed {
  let programs = Programs::processes();
  let driving = (Switches { drives: true }, true);
  follow_in(
    (&screen.root, screen.project, &programs),
    &MergeChecks::default(),
    driving,
  )
  .expect("followed")
}

/// The entries of the waits `chat` keeps, oldest first.
fn waits(screen: &Orchestrator, chat: ChatId) -> Vec<i64> {
  let entries = screen.db().chat_entries(chat).expect("entries");
  entries
    .iter()
    .filter(|entry| matches!(entry.kind, EntryKind::Wait { .. }))
    .map(|entry| entry.seq)
    .collect()
}

/// The account the Orchestrator's connection runs on is at its plan limit, and its CLI did not say until when.
fn hold_at_the_plan_limit(screen: &Orchestrator) {
  let mut db = screen.db();
  let settings = db.orchestrator_settings().expect("read").expect("published");
  let connection = db
    .connection(settings.connection())
    .expect("read")
    .expect("the connection");
  let connection = AgentConnection::try_from(connection).expect("an agent connection");
  let account = connection
    .account()
    .id(connection.provider())
    .expect("the connection's account");
  let limit = PlanHold {
    since: Timestamp::now(),
    resets: None,
  };
  db.hold_orchestrator(account, limit).expect("kept");
}

#[test]
fn orchestrator_leading_event_says_what_became_of_its_turn_and_try_again_is_a_persons() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  screen.let_orchestrator_drive(true);
  let delivery = screen.delivery();
  let chat = everything(&screen);
  waiting(&screen, delivery, "Retry failed deliveries");
  let turn = follow_driving(&screen).turn.expect("the wait's turn");
  let failed = TurnState::Failed(TurnFailure::TimeLimit { minutes: 10 });
  let mut db = screen.db();
  db.end_chat_turn(turn.message, &failed, Timestamp::now())
    .expect("ended");
  drop(db);

  // Tried again in a minute, as the second of four.
  screen.go(OrchestratorChat::Chat(chat.id()));
  let state = format!("orchestrator-turn-state-{}", turn.message);
  screen.until("the try to come", |screen| {
    let words = screen.text(&format!("{state}-words"));
    words.starts_with("Couldn't answer — trying again at ") && words.ends_with(" · 2 of 4")
  });
  assert_eq!(
    screen.text(&format!("{state}-why")),
    "— the answer took longer than 10 minutes"
  );
  assert!(!screen.exists(&format!("{state}-try-again")), "it is tried by itself");
  assert!(!screen.exists("orchestrator-held"));

  // Its next turn fails for what only the person can lift: left to them, with Try again.
  let seq = waits(&screen, chat.id())[0];
  let second = ChatMessageId::generate();
  let mut db = screen.db();
  assert!(db.retry_wait_turn((chat.id(), seq), second).expect("given"));
  let not_signed_in = TurnState::Failed(TurnFailure::NotSignedIn);
  db.end_chat_turn(second, &not_signed_in, Timestamp::now())
    .expect("ended");
  drop(db);
  // And a second event is kept while the Orchestrator's account is at its plan limit: its turn is held.
  hold_at_the_plan_limit(&screen);
  waiting(&screen, delivery, "Delivery log export");
  assert_eq!(follow_driving(&screen).turn, None, "held");
  let held = waits(&screen, chat.id())[1];

  screen.go(OrchestratorChat::New);
  screen.go(OrchestratorChat::Chat(chat.id()));
  let state = format!("orchestrator-turn-state-{second}");
  screen.until("the event left to the person", |screen| {
    screen.text(&format!("{state}-words")) == "Left to you after 2 tries"
  });
  assert_eq!(screen.text(&format!("{state}-why")), "— its CLI isn't signed in");
  let held = format!("orchestrator-turn-state-wait-{held}");
  assert!(
    screen
      .text(&format!("{held}-words"))
      .starts_with("Held — the account's plan limit is reached · trying again at "),
    "{}",
    screen.text(&format!("{held}-words"))
  );
  assert!(
    screen
      .text("orchestrator-held-words")
      .starts_with("Orchestrator is held · trying again at ")
  );

  // Try again is a person's press: a synthetic one starts nothing, and says nothing.
  let button = format!("{state}-try-again");
  with_synthetic_input(|| screen.click(&button));
  assert_eq!(screen.text(&format!("{state}-why")), "— its CLI isn't signed in");
  // A person's asks for the turn; with no runner to answer, it says why nothing started.
  screen.click(&button);
  screen.until("why it was not tried again", |screen| {
    screen.text(&format!("{state}-why")).starts_with("— not tried again: ")
  });
  let entries = screen.db().chat_entries(chat.id()).expect("entries");
  let turns: Vec<_> = entries.iter().filter_map(|entry| entry.kind.turn()).collect();
  assert!(
    turns.iter().all(|(_, turn)| !turn.is_thinking()),
    "no turn was started: {turns:?}"
  );
}

/// Keeps `raised` as an event of what `chat` leads as `target`, as the follower keeps one; the row of its line.
fn raise(screen: &Orchestrator, (chat, target): (ChatId, &NamedTarget), raised: Raised, after: i64) -> String {
  let mut db = screen.db();
  let subscriptions = db.active_subscriptions().expect("subscriptions");
  let leading = subscriptions.of(target.target).expect("led").clone();
  let event = StandingEvent {
    target: target.target,
    name: target.name.clone(),
    raised,
    automatic: false,
  };
  let at = Timestamp::from_unix_millis(Timestamp::now().unix_millis() + after);
  db.keep_standing(&leading, (&format!("test-{after}"), Some(&event)), at)
    .expect("kept");
  let entries = db.chat_entries(chat).expect("entries");
  let kept = entries
    .iter()
    .find(|entry| matches!(&entry.kind, EntryKind::Standing { event: kept, .. } if **kept == event));
  format!("orchestrator-standing-wait-{}", kept.expect("the event's entry").seq)
}

/// Module *Webhooks v2* with a branch of its own, accepted on the delivery pipeline, led to the end by a new
/// conversation; the module, the conversation and the pipeline's name.
fn led_module_with_its_own_branch(screen: &Orchestrator) -> (NamedTarget, Chat, String) {
  let delivery = screen.delivery();
  let mut db = screen.db();
  let led = module(&mut db, "Webhooks v2", None);
  let WatchTarget::Module(id) = led.target else {
    unreachable!("a module");
  };
  let own = OwnBranch {
    acceptance: Some(delivery),
    ..OwnBranch::new(BranchName::new("module/webhooks-v2").expect("branch"))
  };
  db.set_module_own_branch(id, Some(&own)).expect("its own branch");
  let pipeline = db.resource(delivery).expect("read").expect("the pipeline");
  let chat = Chat::named(ChatId::generate(), "Webhooks v2", Timestamp::now());
  watch_in(&screen.root, &chat, std::slice::from_ref(&led), "lead to the end").expect("led to the end");
  (led, chat, pipeline.name().as_str().to_owned())
}

/// One event of each kind a target led to the end raises: work can start, stuck, its acceptance can start, it can be
/// delivered, its delivery waits for the person.
fn each_event() -> [Raised; 5] {
  let left = |task: &str| Left {
    task: task.to_owned(),
    standing: "parked".to_owned(),
    note: None,
  };
  [
    Raised::CanStart {
      tasks: vec!["KON-41".to_owned(), "KON-42".to_owned()],
    },
    Raised::Stuck {
      left: vec![left("KON-41"), left("KON-42"), left("KON-43")],
      budget_used: false,
    },
    Raised::Accept,
    Raised::Deliver,
    Raised::DeliveryWaits {
      why: PersonsDelivery::NotAllowed,
    },
  ]
}

/// The words of the line `row` and what it adds after them.
fn said(screen: &mut Orchestrator, row: &str) -> (String, String) {
  (
    screen.text(&format!("{row}-words")),
    screen.text(&format!("{row}-aside")),
  )
}

#[test]
fn orchestrator_leading_events_of_the_target_are_lines_with_what_each_adds() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  screen.let_orchestrator_drive(false);
  let (led, chat, pipeline) = led_module_with_its_own_branch(&screen);
  let rows: Vec<String> = (1..)
    .zip(each_event())
    .map(|(after, raised)| raise(&screen, (chat.id(), &led), raised, after))
    .collect();
  let [can_start, stuck, accept, deliver, waits] = rows.as_slice() else {
    panic!("five events: {rows:?}");
  };

  screen.go(OrchestratorChat::Chat(chat.id()));
  screen.until("the events' lines", |screen| screen.exists(&format!("{waits}-words")));
  let line = |words: &str, aside: &str| (words.to_owned(), aside.to_owned());
  let starts = "Work can start in Webhooks v2: KON-41, KON-42";
  assert_eq!(
    said(&mut screen, can_start),
    line(starts, "— nothing they depend on is open")
  );
  assert!(screen.exists(&format!("{can_start}-time")), "it says when");
  assert_eq!(
    said(&mut screen, stuck),
    line("Webhooks v2 is stuck: 3 left for you", "— nothing else can start")
  );
  assert_eq!(screen.text(&format!("{stuck}-open")), "Needs you ↗");
  assert!(
    !screen.exists(&format!("{stuck}-time")),
    "the link is in the time's place"
  );
  let accepts = "Every task of Webhooks v2 is done: its acceptance can start";
  assert_eq!(said(&mut screen, accept), line(accepts, &format!("— {pipeline}")));
  let delivers = "— module/webhooks-v2 into the branch your checkout has";
  assert_eq!(
    said(&mut screen, deliver),
    line("Webhooks v2 is accepted: it can be delivered", delivers)
  );
  let persons = "Every task of Webhooks v2 is done: its delivery waits for you";
  assert_eq!(
    said(&mut screen, waits),
    line(persons, "— Deliver when accepted is off")
  );
  assert_eq!(screen.text(&format!("{waits}-open")), "Module ↗");
}
