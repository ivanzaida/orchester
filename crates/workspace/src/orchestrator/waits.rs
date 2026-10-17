//! The Orchestrator chat follows the project's agents (ORC-42, `docs/design/orchestrator-toolset.md`, decision 5):
//! each time an agent of a task's latest run asks a question with suggested answers, its step waits for a message,
//! or its plan waits for a decision, a conversation keeps one entry for that wait, linking to its card: the one that
//! last dealt with the wait's task or run ([`orchester_core::orchestrator::wait::dealt_with`]), else the project's
//! latest conversation, or a new one while the project has none. With *Let the Orchestrator answer agents* on, a wait
//! the Orchestrator's own answer did not raise also starts one Orchestrator turn by itself in its conversation
//! ([`orchester_core::orchestrator::instructions::wait_prompt`]), at most one per wait, never while another turn of
//! that conversation thinks (it waits for that turn to end; a turn of another conversation does not hold it), only
//! while the wait still waits, and only once the project's runner attached. The app attaches the runner when
//! [`follow_in`] says a turn is pending, and asks it for the turns [`follow_in`] gives, with the chat's tools served
//! for them.

use std::{collections::HashSet, path::Path};

use orchester_core::{
  conversation::{Message as TaskMessage, Sender},
  id::{ChatId, ChatMessageId, ProjectId, StepExecutionId},
  orchestrator::{
    AgentWait, Chat, ChatEntry, WaitKind,
    wait::{dealt_with, raised_by_own},
  },
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

/// Keeps each new wait of the agents of project `project` at `root` in its conversation, and gives the oldest wait,
/// of any conversation, that starts a turn by itself and has none its turn while no turn of its conversation thinks.
/// `answers`: whether *Let the Orchestrator answer agents* is on now; a wait kept while it was on starts no turn once
/// it is off, nor while the Orchestrator has nothing published to run on (the runner could not take the turn).
/// `attached`: whether the project's runner has attached. A runner that attaches ends the turns still thinking as
/// interrupted, so a turn starts only once it is there: without it the turn stays `pending`, for the app to attach the
/// runner and follow again. Blocks on SQLite.
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
  let kept = keep_new(&mut db, &new, answers)?;
  let followed = Followed {
    kept,
    ..Followed::default()
  };
  if !ready {
    return Ok(followed);
  }
  let waiting: HashSet<&str> = now.iter().map(|(key, _)| key.as_str()).collect();
  let next = next_turns(&db, &waiting)?;
  if next.is_empty() {
    return Ok(followed);
  }
  if !attached {
    return Ok(Followed {
      pending: true,
      ..followed
    });
  }
  let turn = start(&mut db, &next)?;
  Ok(Followed { turn, ..followed })
}

/// Keeps each of the `new` waits in the conversation that last dealt with its task ([`dealt_with`]), else in the
/// project's latest conversation, or in one the first such wait opens while the project has none; each starts a turn
/// by itself when `answers` and the Orchestrator's own answer, given in any conversation, did not raise it. Whether
/// it kept one.
fn keep_new(db: &mut ProjectDb, new: &[&(String, AgentWait)], answers: bool) -> Result<bool, StoreError> {
  if new.is_empty() {
    return Ok(false);
  }
  let conversations = conversations(db)?;
  let mut entries: Vec<ChatEntry> = conversations.iter().flat_map(|(_, entries)| entries.clone()).collect();
  entries.sort_by_key(|entry| entry.seq);
  let mut latest = match db.latest_chat()? {
    Some(id) => db.chat(id)?,
    None => None,
  };
  let mut kept_any = false;
  for (key, wait) in new {
    let own = raised_by_own(&entries, wait.run);
    let kept = AgentWait {
      own,
      automatic: answers && !own,
      ..wait.clone()
    };
    let about = conversations
      .iter()
      .flat_map(|(chat, entries)| entries.iter().map(|entry| (chat.id(), entry)));
    let dealt = dealt_with(about, wait).and_then(|id| conversations.iter().find(|(chat, _)| chat.id() == id));
    let chat = match dealt {
      Some((chat, _)) => chat,
      None => &*latest.get_or_insert_with(|| Chat::named(ChatId::generate(), &opening_title(wait), Timestamp::now())),
    };
    kept_any |= db.keep_agent_wait(chat, (key, &kept), Timestamp::now())?;
  }
  Ok(kept_any)
}

/// The waits of every conversation that start a turn by themselves, have none and still wait (their keys among
/// `waiting`), oldest first.
fn next_turns(db: &ProjectDb, waiting: &HashSet<&str>) -> Result<Vec<KeptWait>, StoreError> {
  Ok(
    db.automatic_waits_without_turn()?
      .into_iter()
      .filter(|kept| waiting.contains(kept.key.as_str()))
      .collect(),
  )
}

/// Gives the oldest of `next` whose conversation has no turn thinking its turn, in that conversation; none while a
/// turn thinks in each of theirs.
fn start(db: &mut ProjectDb, next: &[KeptWait]) -> Result<Option<WaitTurn>, StoreError> {
  for kept in next {
    let message = ChatMessageId::generate();
    // Refused while another turn of its conversation thinks: the wait keeps no turn and starts once that one ended.
    if db.start_wait_turn((kept.chat, kept.seq), message)? {
      return Ok(Some(WaitTurn {
        chat: kept.chat,
        message,
      }));
    }
  }
  Ok(None)
}

/// The title of a conversation `wait` opens: its task and what waits.
fn opening_title(wait: &AgentWait) -> String {
  format!("{} · {}", wait.task, wait.title)
}

/// Every conversation of the project, with its entries.
fn conversations(db: &ProjectDb) -> Result<Vec<(Chat, Vec<ChatEntry>)>, StoreError> {
  db.chats()?
    .into_iter()
    .map(|listing| {
      let entries = db.chat_entries(listing.chat.id())?;
      Ok((listing.chat, entries))
    })
    .collect()
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
