//! The Orchestrator chat follows the project's agents (ORC-42, `docs/design/orchestrator-toolset.md`, decision 5):
//! each time an agent of a task's latest run asks a question with suggested answers, its step waits for a message,
//! or its plan waits for a decision, the conversation keeps one entry for that wait, linking to its card; in the
//! project's latest conversation, or a new one while the project has none. With *Let the Orchestrator answer agents*
//! on, a wait the Orchestrator's own answer did not raise also starts one Orchestrator turn by itself
//! ([`orchester_core::orchestrator::instructions::wait_prompt`]), at most one per wait, never while another turn of
//! the conversation thinks (it waits for that turn to end), only while the wait still waits, and only once the
//! project's runner attached. The app attaches the runner when [`follow_in`] says a turn is pending, and asks it for
//! the turns [`follow_in`] gives, with the chat's tools served for them.

use std::{collections::HashSet, path::Path};

use orchester_core::{
  conversation::{Message as TaskMessage, Sender},
  id::{ChatId, ChatMessageId, ProjectId, StepExecutionId},
  orchestrator::{AgentWait, Chat, ChatEntry, WaitKind, wait::raised_by_own},
  question::AskedQuestions,
  run::Plan,
  store::{ConversationStore, RunStore, TaskStore},
  task::Task,
  time::Timestamp,
};
use orchester_store::{KeptWait, ProjectDb, StoreError};

use super::settings::OrchestratorSetup;
use crate::{
  links::{Link, TaskPage},
  messages::{self, Message},
  tasks::detail::{RunView, run_store_error, run_view},
};

/// An Orchestrator turn a wait started by itself, which the runner is to answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitTurn {
  pub chat: ChatId,
  pub message: ChatMessageId,
}

/// What following the waits did: whether it kept a new wait, the turn a wait started by itself, if one did, and
/// whether a wait would start its turn now but the project's runner has not attached yet. The conversations changed
/// only when `kept` or `turn` is so.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Followed {
  pub kept: bool,
  pub turn: Option<WaitTurn>,
  pub pending: bool,
}

/// Keeps each new wait of the agents of project `project` at `root` in its conversation, and gives the oldest wait
/// that starts a turn by itself and has none its turn while no turn of its conversation thinks. `answers`: whether
/// *Let the Orchestrator answer agents* is on now; a wait kept while it was on starts no turn once it is off, nor
/// while the Orchestrator has nothing published to run on (the runner could not take the turn). `attached`: whether
/// the project's runner has attached. A runner that attaches ends the turns still thinking as interrupted, so a turn
/// starts only once it is there: without it the turn stays `pending`, for the app to attach the runner and follow
/// again. Blocks on SQLite.
pub fn follow_in(root: &Path, project: ProjectId, (answers, attached): (bool, bool)) -> Result<Followed, Message> {
  let ready = answers && OrchestratorSetup::load_in(root)?.readiness().is_ready();
  follow(root, project, (answers, ready), attached).map_err(|error| messages::store_error(&error))
}

fn follow(
  root: &Path,
  project: ProjectId,
  (answers, ready): (bool, bool),
  attached: bool,
) -> Result<Followed, StoreError> {
  let mut db = ProjectDb::open(root)?;
  let now = current_waits(&db, project)?;
  let kept = db.kept_wait_keys()?;
  let new: Vec<&(String, AgentWait)> = now.iter().filter(|(key, _)| !kept.contains(key)).collect();
  let chat = match (db.latest_chat()?, new.first()) {
    (Some(id), _) => db.chat(id)?,
    (None, Some((_, first))) => Some(Chat::named(ChatId::generate(), &opening_title(first), Timestamp::now())),
    (None, None) => None,
  };
  let Some(chat) = chat else {
    return Ok(Followed::default());
  };
  let kept = keep_new(&mut db, &chat, &new, answers)?;
  let followed = Followed {
    kept,
    ..Followed::default()
  };
  if !ready {
    return Ok(followed);
  }
  let waiting: HashSet<&str> = now.iter().map(|(key, _)| key.as_str()).collect();
  let Some(next) = next_turn(&db, chat.id(), &waiting)? else {
    return Ok(followed);
  };
  if !attached {
    return Ok(Followed {
      pending: true,
      ..followed
    });
  }
  let turn = start(&mut db, chat.id(), &next)?;
  Ok(Followed { turn, ..followed })
}

/// Keeps the `new` waits in `chat`, each starting a turn by itself when `answers` and the Orchestrator's own answer,
/// given in any conversation, did not raise it; whether it kept one.
fn keep_new(db: &mut ProjectDb, chat: &Chat, new: &[&(String, AgentWait)], answers: bool) -> Result<bool, StoreError> {
  if new.is_empty() {
    return Ok(false);
  }
  let entries = every_entry(db)?;
  let mut kept_any = false;
  for (key, wait) in new {
    let own = raised_by_own(&entries, wait.run);
    let kept = AgentWait {
      own,
      automatic: answers && !own,
      ..wait.clone()
    };
    kept_any |= db.keep_agent_wait(chat, (key, &kept), Timestamp::now())?;
  }
  Ok(kept_any)
}

/// The oldest wait of `chat` that starts a turn by itself, has none and still waits (its key among `waiting`).
fn next_turn(db: &ProjectDb, chat: ChatId, waiting: &HashSet<&str>) -> Result<Option<KeptWait>, StoreError> {
  Ok(
    db.waits_without_turn(chat)?
      .into_iter()
      .find(|kept| kept.wait.automatic && waiting.contains(kept.key.as_str())),
  )
}

/// Gives wait `next` of `chat` its turn; none while another turn of the conversation thinks.
fn start(db: &mut ProjectDb, chat: ChatId, next: &KeptWait) -> Result<Option<WaitTurn>, StoreError> {
  let message = ChatMessageId::generate();
  // Refused while another turn of the conversation thinks: the wait keeps no turn and starts once that one ended.
  let started = db.start_wait_turn((chat, next.seq), message)?;
  Ok(started.then_some(WaitTurn { chat, message }))
}

/// The title of a conversation `wait` opens: its task and what waits.
fn opening_title(wait: &AgentWait) -> String {
  format!("{} · {}", wait.task, wait.title)
}

/// The entries of every conversation of the project, in the order they were kept.
fn every_entry(db: &ProjectDb) -> Result<Vec<ChatEntry>, StoreError> {
  let mut entries = Vec::new();
  for listing in db.chats()? {
    entries.extend(db.chat_entries(listing.chat.id())?);
  }
  entries.sort_by_key(|entry| entry.seq);
  Ok(entries)
}

/// The latest message `execution`'s agent wrote or was given among `messages`. A step that waits for a message again
/// once it got one is a new wait, whether or not its agent wrote anything new.
fn last_exchanged(messages: &[TaskMessage], execution: StepExecutionId) -> Option<&TaskMessage> {
  messages.iter().rev().find(|message| match message.sender() {
    Sender::Agent { execution: by, .. } => *by == execution,
    Sender::Person { delivery, .. } => delivery.execution() == Some(execution),
  })
}

/// What the agents of the latest runs of project `project`'s tasks wait for now, each by the key it is kept by: the
/// questions with suggested answers, the steps that wait for a message (but those of such a question), and the plans
/// that wait for a decision.
fn current_waits(db: &ProjectDb, project: ProjectId) -> Result<Vec<(String, AgentWait)>, StoreError> {
  let asked = db.waiting_questions()?;
  let mut waits = Vec::new();
  for task in db.tasks()? {
    let Some(Ok(run)) = db.runs_of_task(task.id()).map_err(run_store_error)?.pop() else {
      continue;
    };
    // A run that ended waits for no one.
    if run.status().is_terminal() {
      continue;
    }
    let choices: Vec<&AskedQuestions> = asked.iter().filter(|asked| asked.request.run() == run.id()).collect();
    if choices.is_empty() && run.awaiting_answer().is_empty() && run.awaiting_plan().is_empty() {
      continue;
    }
    let messages = db.messages(task.id())?;
    let view = run_view(db, run)?;
    let wait = Waiting {
      project,
      task: &task,
      view: &view,
    };
    for asked in &choices {
      let request = &asked.request;
      let key = format!("question:{}:{}", view.id(), request.id());
      waits.push((
        key,
        wait.of(WaitKind::Question, request.execution(), Some(request.agent())),
      ));
    }
    for execution in view.run.awaiting_answer() {
      if choices.iter().any(|asked| asked.request.execution() == execution) {
        continue;
      }
      let last = last_exchanged(&messages, execution).map_or_else(|| "-".to_owned(), |last| last.id().to_string());
      waits.push((
        format!("message:{execution}:{last}"),
        wait.of(WaitKind::Message, execution, None),
      ));
    }
    for execution in view.run.awaiting_plan() {
      let Some(plan) = Plan::latest_of(&view.plans, execution) else {
        continue;
      };
      let key = format!("plan:{execution}:{}", plan.version.get());
      waits.push((key, wait.of(WaitKind::Plan, execution, None)));
    }
  }
  Ok(waits)
}

/// A task's latest run that waits, which its waits are named after.
struct Waiting<'a> {
  project: ProjectId,
  task: &'a Task,
  view: &'a RunView,
}

impl Waiting<'_> {
  /// A wait of `kind` of the step `execution` runs, whose agent is `agent` when the question names it.
  fn of(&self, kind: WaitKind, execution: StepExecutionId, agent: Option<&str>) -> AgentWait {
    let step = self.view.step_of(execution);
    AgentWait {
      kind,
      task: self.task.key().to_string(),
      title: self.task.details().title.as_str().to_owned(),
      run: self.view.id(),
      number: self.view.run.number().get(),
      step: step.map(|step| step.name.to_string()),
      agent: agent
        .map(str::to_owned)
        .or_else(|| step.and_then(|step| step.agent.as_deref().map(str::to_owned))),
      link: Link::Task(self.project, self.task.key().clone(), TaskPage::Conversation).path(),
      own: false,
      automatic: false,
    }
  }
}
