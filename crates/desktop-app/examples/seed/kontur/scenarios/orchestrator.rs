//! `orchestrator`: the `api-providers` scenario with the Orchestrator set up on Claude · Work, a Claude Code connection
//! checked and signed in whose catalog reports efforts, and two conversations in the new chat's shapes
//! (`Orchestrator · redesign` `hujxu` and `Orchestrator cards · redesign` `zuvUi` in the new orchester.pen): *While
//! you were away* (`01 Conversation`), three agents waiting for the person gathered into one digest and a turn
//! answered with several actions, the one the Orchestrator opens on; and *Every card* (`06 Cards in a conversation`),
//! in History, with each card kind: event lines (done, refused, a run's end), waits answered by the Orchestrator and by
//! the person, decision cards waiting, decided ones folded into lines, a hold of new starts, a daily report, a stopped
//! turn and a failed one. `orchestrator-not-set-up` is the same project with nothing chosen for the Orchestrator
//! (`02 Not set up`): its conversations stay readable.

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ConnectionId, RunId},
  orchestrator::{
    ActionOutcome, AgentWait, Chat, ChatAction, DailyReport, HeldWork, OrchestratorSettings, RunEnd, TurnFailure,
    TurnState, WaitKind,
    change::{Change, ProposedChange},
    report::{DayUsage, MergedBy, MergedTask, PersonWait, PersonWaitKind, ReportRun},
  },
  resource::ModelId,
  store::ConnectionStore,
  time::Timestamp,
  usage::local_date,
};

use super::api_providers;
use crate::kontur::{Kontur, SeedError, name};

/// The model the Orchestrator runs with, as Claude · Work's catalog lists it.
const MODEL: &str = "claude-opus-5-5";

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let work = project(kontur)?;
  let settings = OrchestratorSettings::new(work, model(MODEL)?, Some("high".to_owned()));
  kontur.db.publish_orchestrator(&settings, kontur.ago(3 * 24 * 60))?;
  Ok(())
}

/// The project, its connections and conversations, with nothing chosen for the Orchestrator.
pub fn not_set_up(kontur: &mut Kontur) -> Result<(), SeedError> {
  project(kontur).map(drop)
}

/// The project, its connections and conversations; Claude · Work's id.
fn project(kontur: &mut Kontur) -> Result<ConnectionId, SeedError> {
  api_providers::seed(kontur)?;
  let program = kontur.repo.join(".orchester").join("qa-bin").join("claude");
  let work = work(kontur, program)?;
  every_card(kontur)?;
  away(kontur)?;
  Ok(work)
}

/// Claude · Work: Claude Code, checked and signed in, its models with the efforts it reports.
fn work(kontur: &mut Kontur, program: std::path::PathBuf) -> Result<ConnectionId, SeedError> {
  let at = kontur.ago(2);
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    name("Claude · Work"),
    AgentProvider::ClaudeCodeCli,
    program,
    at,
  )?;
  let efforts = || ["low", "medium", "high"].map(str::to_owned).to_vec();
  let models = [MODEL, "claude-sonnet-5-5", "claude-haiku-4-5"]
    .into_iter()
    .map(|id| model(id).map(|id| CatalogModel::new(id, efforts())))
    .collect::<Result<Vec<_>, _>>()?;
  connection.record_check(ConnectionCheck {
    at,
    result: CheckResult::Available {
      version: "2.1.89".to_owned(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(CatalogSource::Provider, models),
    },
  })?;
  let connection = connection.into_connection();
  kontur.db.create_connection(&connection)?;
  Ok(connection.id())
}

fn model(id: &str) -> Result<ModelId, SeedError> {
  ModelId::new(id).map_err(|error| SeedError::Invalid(error.to_string()))
}

fn body(text: &str) -> Result<MessageBody, SeedError> {
  MessageBody::new(text).map_err(|error| SeedError::Invalid(error.to_string()))
}

/// A turn of `chat` the person started with `text` at `at`, answered with `parts`, and ended as `end` a minute later.
fn turn(
  kontur: &mut Kontur,
  chat: &Chat,
  (text, at): (&str, Timestamp),
  parts: Vec<Part>,
  end: &TurnState,
) -> Result<(), SeedError> {
  let message = ChatMessageId::generate();
  let said = body(text)?;
  kontur.db.keep_chat_message(chat, (message, &said), at)?;
  answer(kontur, chat.id(), at, parts)?;
  kontur.db.end_chat_turn(message, end, later(at, 60))?;
  Ok(())
}

/// One part of an answer.
enum Part {
  Reply(&'static str),
  Action(Box<ChatAction>),
}

fn act(action: ChatAction) -> Part {
  Part::Action(Box::new(action))
}

fn answer(kontur: &mut Kontur, chat: ChatId, at: Timestamp, parts: Vec<Part>) -> Result<(), SeedError> {
  let model = model(MODEL)?;
  for (offset, part) in (1_i64..).zip(parts) {
    let at = later(at, offset * 10);
    match part {
      Part::Reply(text) => kontur.db.add_chat_reply(chat, (text, &model), at)?,
      Part::Action(action) => kontur.db.add_chat_action(chat, &action, at)?,
    }
  }
  Ok(())
}

/// `seconds` after `at`.
fn later(at: Timestamp, seconds: i64) -> Timestamp {
  Timestamp::from_unix_millis(at.unix_millis() + seconds * 1_000)
}

/// A wait of `kind` of task `task`'s run `#number` by `agent`; it starts a turn by itself when `automatic`.
fn wait(kind: WaitKind, (task, title, number): (&str, &str, u32), agent: Option<&str>, automatic: bool) -> AgentWait {
  AgentWait {
    kind,
    task: task.to_owned(),
    title: title.to_owned(),
    run: RunId::generate(),
    number,
    step: Some("Implement".to_owned()),
    place: Some(2),
    others: 0,
    agent: agent.map(str::to_owned),
    link: format!("/tasks/{task}"),
    own: false,
    automatic,
    continuing: None,
    merging: None,
  }
}

/// Keeps `wait` in `chat` at `at`; its entry.
fn keep_wait(kontur: &mut Kontur, chat: &Chat, wait: &AgentWait, at: Timestamp) -> Result<i64, SeedError> {
  let key = format!("{}-{}-{:?}", wait.task, wait.number, wait.kind);
  if !kontur.db.keep_agent_wait(chat, (&key, wait), at)? {
    return Err(SeedError::Invalid(format!("wait {key} is kept twice")));
  }
  kontur
    .db
    .chat_entries(chat.id())?
    .into_iter()
    .map(|entry| entry.seq)
    .max()
    .ok_or_else(|| SeedError::Invalid("the wait's entry".to_owned()))
}

/// The turn wait `seq` of `chat` started by itself at `at`, answered with `parts`.
fn wait_turn(
  kontur: &mut Kontur,
  (chat, seq): (ChatId, i64),
  at: Timestamp,
  parts: Vec<Part>,
) -> Result<(), SeedError> {
  let message = ChatMessageId::generate();
  kontur.db.start_wait_turn((chat, seq), message)?;
  answer(kontur, chat, at, parts)?;
  kontur.db.end_chat_turn(message, &TurnState::Answered, later(at, 60))?;
  Ok(())
}

/// A proposed change `change` of tool `tool` on `task`, gone as `outcome`.
fn change(tool: &str, task: Option<&str>, change: Change, outcome: ActionOutcome) -> ChatAction {
  ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: String::new(),
      change,
    })),
    ..ChatAction::new(tool, task.map(str::to_owned), outcome)
  }
}

/// An answer to run `#number`'s agent.
fn answer_change(number: u32, text: &str) -> Change {
  Change::Answer {
    run: RunId::generate(),
    number,
    request: "question".to_owned(),
    choices: Vec::new(),
    text: Some(text.to_owned()),
  }
}

/// *While you were away*: three waits for the person in a digest, then a turn answered with several actions.
fn away(kontur: &mut Kontur) -> Result<(), SeedError> {
  let start = kontur.ago(40);
  let chat = Chat::named(ChatId::generate(), "Plan the webhook retry work", start);
  for (index, (kind, task, agent)) in [
    (
      WaitKind::Question,
      ("KON-12", "Add retry to webhook sender", 3),
      Some("Builder"),
    ),
    (
      WaitKind::Plan,
      ("KON-16", "Delivery log on endpoint page", 2),
      Some("Planner"),
    ),
    (
      WaitKind::Message,
      ("KON-13", "Batch webhook deliveries", 2),
      Some("Reviewer"),
    ),
  ]
  .into_iter()
  .enumerate()
  {
    let at = later(start, i64::try_from(index).unwrap_or_default() * 60);
    keep_wait(kontur, &chat, &wait(kind, task, agent, false), at)?;
  }
  turn(
    kontur,
    &chat,
    (
      "Split the retry work into tasks we can review, and start what can run.",
      kontur.ago(12),
    ),
    vec![
      Part::Reply(
        "I split the retry work into two tasks and started the one whose pipeline is ready. **KON-19** can't start yet: \
         it waits for KON-18 to merge.",
      ),
      act(ChatAction::new(
        "create_task",
        Some("KON-21".to_owned()),
        ActionOutcome::Done,
      )),
      act(ChatAction::new(
        "create_task",
        Some("KON-22".to_owned()),
        ActionOutcome::Done,
      )),
      act(refused_start()),
      act(change(
        "answer_agent",
        Some("KON-12"),
        answer_change(3, "Keep the backoff in retry.rs and cap it at six attempts."),
        ActionOutcome::Waits,
      )),
    ],
    &TurnState::Answered,
  )
}

/// *Every card*: each card kind, in History.
fn every_card(kontur: &mut Kontur) -> Result<(), SeedError> {
  let start = kontur.ago(26 * 60);
  let chat = Chat::named(ChatId::generate(), "Every card", start);
  turn(
    kontur,
    &chat,
    ("Draft the delivery log tasks and start KON-19.", start),
    vec![
      Part::Reply("I drafted the delivery log tasks. KON-19 can't start yet."),
      act(ChatAction::new(
        "create_task",
        Some("KON-16".to_owned()),
        ActionOutcome::Done,
      )),
      act(refused_start()),
    ],
    &TurnState::Answered,
  )?;
  let at = later(start, 5 * 60);
  waits(kontur, &chat, at)?;
  decisions(kontur, &chat, later(at, 10 * 60))?;
  report(kontur, &chat, later(at, 12 * 60))?;
  turn(
    kontur,
    &chat,
    ("Plan the release notes.", later(at, 14 * 60)),
    vec![Part::Reply(
      "I'm reading the merged tasks of the week. So far: KON-4, KON-11",
    )],
    &TurnState::Stopped,
  )?;
  turn(
    kontur,
    &chat,
    ("What's blocked right now?", later(at, 16 * 60)),
    Vec::new(),
    &TurnState::Failed(TurnFailure::NotSignedIn),
  )
}

/// `run_task` of KON-19, refused.
fn refused_start() -> ChatAction {
  ChatAction::new(
    "run_task",
    Some("KON-19".to_owned()),
    ActionOutcome::Refused {
      code: "blocked".to_owned(),
      reason: "KON-19 waits for KON-18 to merge.".to_owned(),
    },
  )
}

/// A run's end, a wait the Orchestrator answered and a plan it read, from `at`.
fn waits(kontur: &mut Kontur, chat: &Chat, at: Timestamp) -> Result<(), SeedError> {
  keep_wait(
    kontur,
    chat,
    &wait(
      WaitKind::Ended(RunEnd::Succeeded),
      ("KON-4", "Show the signed-in account", 1),
      None,
      false,
    ),
    at,
  )?;
  let answered = keep_wait(
    kontur,
    chat,
    &wait(
      WaitKind::Question,
      ("KON-12", "Add retry to webhook sender", 2),
      Some("Builder"),
      true,
    ),
    later(at, 60),
  )?;
  wait_turn(
    kontur,
    (chat.id(), answered),
    later(at, 70),
    vec![
      Part::Reply("The brief settles it: retries live in retry.rs."),
      act(change(
        "answer_agent",
        Some("KON-12"),
        answer_change(2, "Keep the backoff in retry.rs."),
        ActionOutcome::Done,
      )),
    ],
  )?;
  let plan = keep_wait(
    kontur,
    chat,
    &wait(
      WaitKind::Plan,
      ("KON-16", "Delivery log on endpoint page", 1),
      Some("Planner"),
      true,
    ),
    later(at, 120),
  )?;
  wait_turn(
    kontur,
    (chat.id(), plan),
    later(at, 130),
    vec![Part::Reply(
      "The plan covers the brief; I'd approve it once its file list drops the migrations it doesn't need.",
    )],
  )
}

/// A turn at `at` that held new starts and proposed changes: two waiting, one applied, one dismissed.
fn decisions(kontur: &mut Kontur, chat: &Chat, at: Timestamp) -> Result<(), SeedError> {
  let release = |outcome| change("release_queue", None, Change::Release { task: None }, outcome);
  turn(
    kontur,
    chat,
    ("Let new starts go on, and answer Builder on KON-13.", at),
    vec![
      act(ChatAction {
        held: Some(HeldWork {
          group: None,
          waiting: vec!["KON-21".to_owned(), "KON-22".to_owned()],
        }),
        ..ChatAction::new("hold_queue", None, ActionOutcome::Done)
      }),
      act(release(ActionOutcome::Waits)),
      act(change(
        "answer_agent",
        Some("KON-13"),
        answer_change(2, "Batch per endpoint, at most 50 deliveries."),
        ActionOutcome::Waits,
      )),
      act(release(ActionOutcome::Applied)),
      act(change(
        "answer_agent",
        Some("KON-13"),
        answer_change(1, "Batch everything."),
        ActionOutcome::Dismissed,
      )),
    ],
    &TurnState::Answered,
  )
}

/// The day's report, kept in `chat` at `at`.
fn report(kontur: &mut Kontur, chat: &Chat, at: Timestamp) -> Result<(), SeedError> {
  let run = |task: &str, title: &str, number| ReportRun {
    task: task.to_owned(),
    title: title.to_owned(),
    run: RunId::generate(),
    number,
    step: Some("Review".to_owned()),
    since: Some(at),
    link: format!("/tasks/{task}"),
  };
  let merged = |task: &str, title: &str| MergedTask {
    task: task.to_owned(),
    title: title.to_owned(),
    number: 1,
    at,
    link: format!("/tasks/{task}"),
    by: MergedBy::Person,
  };
  let waiting = |kind, task: &str| PersonWait {
    kind,
    task: Some(task.to_owned()),
    number: Some(1),
    what: None,
    since: Some(at),
    link: format!("/tasks/{task}"),
  };
  let report = DailyReport {
    day: local_date(at),
    built: at,
    merged: vec![
      merged("KON-4", "Show the signed-in account"),
      merged("KON-11", "Retry budget per endpoint"),
    ],
    ready: vec![run("KON-9", "Trim old deliveries", 1)],
    stopped: Vec::new(),
    working: vec![run("KON-12", "Add retry to webhook sender", 2)],
    waiting: vec![
      waiting(PersonWaitKind::Question, "KON-12"),
      waiting(PersonWaitKind::Plan, "KON-16"),
    ],
    actions: Vec::new(),
    usage: DayUsage {
      spent: 1_240_000,
      active_ms: 3 * 60 * 60 * 1_000,
      budget: None,
      tasks: Vec::new(),
      accounts: Vec::new(),
      link: "/usage".to_owned(),
    },
    queued: Vec::new(),
    automatic: false,
  };
  kontur.db.keep_daily_report(chat, &report, at)?;
  Ok(())
}
