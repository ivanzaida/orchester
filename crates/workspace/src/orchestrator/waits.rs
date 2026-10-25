//! The Orchestrator chat follows the project's agents (ORC-42, `docs/design/orchestrator-toolset.md`, decision 5):
//! each time an agent of a task's latest run asks a question with suggested answers, its step waits for a message,
//! or its plan waits for a decision, and each time a task's latest run ends or stops ([`super::run_ends`]), a
//! conversation keeps one entry for that wait, linking to its card (a run's end: its task's Summary): the one that
//! last dealt with the wait's task or run ([`orchester_core::orchestrator::wait::dealt_with`]), else the project's
//! latest conversation, or a new one while the project has none. With *Let the Orchestrator answer agents* on, a wait
//! the Orchestrator's own answer, message or decision on how the run continues did not raise also starts one
//! Orchestrator turn by itself in its conversation
//! ([`orchester_core::orchestrator::instructions::wait_prompt`]), at most one per wait, never while another turn of
//! that conversation thinks (it waits for that turn to end; a turn of another conversation does not hold it), only
//! while the wait still waits, and only once the project's runner attached. With *Let the Orchestrator continue
//! interrupted runs* on too, the turn of an interrupted run's end continues the run, unless it was interrupted again
//! soon; with *Let the Orchestrator merge finished runs on its own* and *merge runs* on too, the turn of a run's
//! success merges the run when the chat's rule lets it ([`super::auto_merge`]), one such turn at a time in the order
//! the runs ended; a run that committed nothing and whose result is its report is completed by the same rule, without a
//! turn (ORC-93). A daily report kept to start a turn ([`super::report`]) gets its turn the same way, oldest entry
//! first. The app attaches the runner when [`follow_in`] says a turn is pending, and asks it for the turns
//! [`follow_in`] gives, with the chat's tools served for them.

use std::{collections::HashSet, path::Path};

use orchester_core::{
  connection::stream::InvocationOutcome,
  conversation::{Message as TaskMessage, Sender},
  id::{ChatId, ChatMessageId, ProjectId, StepExecutionId},
  orchestrator::{
    AgentWait, Chat, ChatEntry, ChatResume, Continuing, Merging, RunEnd, WaitKind,
    wait::{dealt_with, raised_by_own},
  },
  question::AskedQuestions,
  run::{CompletedBy, Plan},
  store::{ConversationStore, RunStore, TaskStore, WorktreeStore},
  task::Task,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{KeptWait, ProjectDb, StoreError};

pub use super::run_ends::{MergeChecks, SucceededAt};
use super::{
  auto_merge::{completes_in, merging_in},
  run_ends::{Reading, current_ends, merge_checks},
  settings::OrchestratorSetup,
};
use crate::{
  links::{Link, TaskPage},
  messages::{self, Message},
  tasks::{
    changes::run_worktree,
    completion,
    detail::{RunView, run_store_error, run_view},
  },
};

/// An Orchestrator turn a wait started by itself, which the runner is to answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitTurn {
  pub chat: ChatId,
  pub message: ChatMessageId,
}

/// What following the waits did: whether it kept a new wait, the turn a wait started by itself, if one did, and
/// whether a wait would start its turn now but the project's runner has not attached yet. The conversations changed
/// only when `kept` or `turn` is so. `completed`: the Orchestrator completed a run that committed nothing on its own
/// (ORC-93), so the project's tasks changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Followed {
  pub kept: bool,
  pub turn: Option<WaitTurn>,
  pub pending: bool,
  pub completed: bool,
}

/// The person's switches the follower reads, as they are now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Switches {
  /// *Let the Orchestrator answer agents*: a wait starts an Orchestrator turn by itself.
  pub answers: bool,
  /// *Let the Orchestrator continue interrupted runs*: with `answers`, the turn an interrupted run's end starts
  /// continues the run ([`orchester_core::orchestrator::Continuing`]).
  pub continues: bool,
  /// *Let the Orchestrator merge finished runs on its own* and *merge runs*, both on: with `answers`, the turn a run's
  /// success starts merges the run when the chat's rule lets it ([`super::auto_merge`]).
  pub merges: bool,
}

/// Keeps each new wait of the agents of project `project` at `root` in its conversation, and gives the oldest wait,
/// of any conversation, that starts a turn by itself and has none its turn while no turn of its conversation thinks.
/// `switches`: the person's switches now; a wait kept while *answer agents* was on starts no turn once it is off, nor
/// while the Orchestrator has nothing published to run on (the runner could not take the turn).
/// `attached`: whether the project's runner has attached. A runner that attaches ends the turns still thinking as
/// interrupted, so a turn starts only once it is there: without it the turn stays `pending`, for the app to attach the
/// runner and follow again. `checks` tell the runs that succeeded which are merged outside Orchester or have nothing
/// to merge ([`check_merges_in`]); `programs` run git for one they do not tell. Blocks on SQLite, and on git only for
/// such a run.
pub fn follow_in(
  (root, project, programs): (&Path, ProjectId, &Programs),
  checks: &MergeChecks,
  (switches, attached): (Switches, bool),
) -> Result<Followed, Message> {
  let ready = switches.answers && OrchestratorSetup::load_in(root)?.readiness().is_ready();
  let reading = Reading {
    project,
    root,
    programs,
    checks,
  };
  follow(&reading, (switches, ready), attached).map_err(|error| messages::store_error(&error))
}

/// What git tells, in one reading of the branches, of the runs of the project at `root` that succeeded and that
/// Orchester did not merge, and that the conversations do not settle yet, but those known `quiet` (merged outside
/// Orchester, or with nothing to merge): for [`follow_in`], asked before it, outside the follower's lock. Blocks on
/// SQLite and git.
pub fn check_merges_in(
  (root, programs): (&Path, &Programs),
  quiet: &HashSet<SucceededAt>,
) -> Result<MergeChecks, Message> {
  let checked = || {
    let db = ProjectDb::open(root)?;
    let (kept, pending) = settled(&db)?;
    merge_checks(
      &db,
      (root, programs),
      (|key: &str| kept.contains(key) && !pending.contains(key), quiet),
    )
  };
  checked().map_err(|error| messages::store_error(&error))
}

/// The keys of the waits kept, and of those kept that start a turn still to come: a kept end that starts none needs no
/// look again.
fn settled(db: &ProjectDb) -> Result<(HashSet<String>, HashSet<String>), StoreError> {
  let pending = db
    .automatic_waits_without_turn()?
    .into_iter()
    .map(|kept| kept.key)
    .collect();
  Ok((db.kept_wait_keys()?, pending))
}

fn follow(reading: &Reading, (switches, ready): (Switches, bool), attached: bool) -> Result<Followed, StoreError> {
  let mut db = ProjectDb::open(reading.root)?;
  let (kept, pending) = settled(&db)?;
  let mut now = current_waits(&db, reading.project)?;
  now.extend(current_ends(&db, reading, |key| {
    kept.contains(key) && !pending.contains(key)
  })?);
  let new: Vec<&(String, AgentWait)> = now.iter().filter(|(key, _)| !kept.contains(key)).collect();
  let kept = keep_new((&mut db, reading.root), &new, switches)?;
  let completed = switches.answers && switches.merges && complete_on_its_own(&db, reading)?;
  let followed = Followed {
    kept,
    completed,
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

/// Completes, as the Orchestrator would merge them, the runs git found with nothing to merge (`reading.checks`) whose
/// result is their report and that the chat's rule would merge ([`completes_in`], ORC-93); whether it completed any.
/// A completion that is refused leaves the run to the person, as a merge that fails does.
fn complete_on_its_own(db: &ProjectDb, reading: &Reading) -> Result<bool, StoreError> {
  let mut completed = false;
  for &(run, _) in &reading.checks.nothing {
    if !completes_in(db, run)? {
      continue;
    }
    let done = completion::complete_in(
      (reading.root, reading.programs),
      run,
      (CompletedBy::Orchestrator, false),
      Timestamp::now(),
    );
    match done {
      Ok(_) => completed = true,
      Err(message) => tracing::warn!(%run, key = message.key, "the Orchestrator did not complete the run"),
    }
  }
  Ok(completed)
}

/// Keeps each of the `new` waits in the conversation that last dealt with its task ([`dealt_with`]), else in the
/// project's latest conversation, or in one the first such wait opens while the project has none; each starts a turn
/// by itself when *answer agents* is on and the Orchestrator's own answer, given in any conversation, did not raise
/// it, such a turn of an interrupted run's end continues the run as [`continuing`] tells, and one of a run's success
/// merges it as [`super::auto_merge::merging_in`] tells. Whether it kept one.
fn keep_new(
  (db, root): (&mut ProjectDb, &Path),
  new: &[&(String, AgentWait)],
  switches: Switches,
) -> Result<bool, StoreError> {
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
    let own = raised_by_own(&entries, wait);
    let automatic = switches.answers && !own;
    let continuing = if automatic && switches.continues {
      continuing((db, root), wait)?
    } else {
      None
    };
    let merging = if automatic && switches.merges && wait.kind == WaitKind::Ended(RunEnd::Succeeded) {
      Some(merging_in(db, wait.run)?)
    } else {
      None
    };
    let kept = AgentWait {
      own,
      automatic,
      continuing,
      merging,
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

/// What the turn `wait` starts does about its run while the person lets the Orchestrator continue interrupted runs:
/// for an interrupted run's end the chat may continue (a crash, the runner going away, *Quit now* or the database
/// staying busy, its worktree there), it continues the run, or leaves it to the person when it was interrupted again
/// soon or the database is under contention ([`super::guard::continuing_of`]); `None` for any other wait, which the
/// switch does not touch.
fn continuing((db, root): (&ProjectDb, &Path), wait: &AgentWait) -> Result<Option<Continuing>, StoreError> {
  if wait.kind != WaitKind::Ended(RunEnd::Interrupted) {
    return Ok(None);
  }
  let Some(run) = db.run(wait.run).map_err(run_store_error)? else {
    return Ok(None);
  };
  let present = db.worktree_removal(run.id())?.is_none() && run_worktree(root, run.id()).is_dir();
  Ok(
    match ChatResume::of(run.status(), db.runner_failure(run.id())?, present) {
      ChatResume::Interrupted(cause) => Some(super::guard::continuing_of(db, run.id(), cause)?),
      ChatResume::Paused | ChatResume::PersonOnly(_) | ChatResume::Other => None,
    },
  )
}

/// An entry that starts a turn by itself and has none: its conversation, its `seq`, and whether its turn merges a
/// run on its own.
type NextTurn = (ChatId, i64, bool);

/// The entries of every conversation that start a turn by themselves and have none, oldest first: the waits that
/// still wait (their keys among `waiting`), and the daily reports (`super::report`), whose turn adds a summary to what
/// they tell.
fn next_turns(db: &ProjectDb, waiting: &HashSet<&str>) -> Result<Vec<NextTurn>, StoreError> {
  let mut next: Vec<NextTurn> = db
    .automatic_waits_without_turn()?
    .into_iter()
    .filter(|kept| waiting.contains(kept.key.as_str()))
    .map(|kept: KeptWait| (kept.chat, kept.seq, merges(&kept.wait)))
    .chain(
      db.automatic_reports_without_turn()?
        .into_iter()
        .map(|kept| (kept.chat, kept.seq, false)),
    )
    .collect();
  next.sort_by_key(|(_, seq, _)| *seq);
  Ok(next)
}

/// Gives the oldest of `next` whose conversation has no turn thinking its turn, in that conversation; none while a
/// turn thinks in each of theirs. Runs merge one at a time in the order they ended: a turn that merges its run on its
/// own does not start while one that ended before, in any conversation, still waits for its turn or thinks.
fn start(db: &mut ProjectDb, next: &[NextTurn]) -> Result<Option<WaitTurn>, StoreError> {
  let mut merging_before = db.waits_thinking()?.iter().any(|kept| merges(&kept.wait));
  for &(chat, seq, merging) in next {
    if merging && merging_before {
      continue;
    }
    let message = ChatMessageId::generate();
    // Refused while another turn of its conversation thinks: the entry keeps no turn and starts once that one ended.
    if db.start_wait_turn((chat, seq), message)? {
      return Ok(Some(WaitTurn { chat, message }));
    }
    merging_before |= merging;
  }
  Ok(None)
}

/// Whether the turn of `wait` merges its run on its own.
fn merges(wait: &AgentWait) -> bool {
  wait.merging == Some(Merging::Merges)
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

/// Whether `execution`'s latest invocation ended because a person stopped its turn (*Stop turn*).
fn stopped_by_person(view: &RunView, execution: StepExecutionId) -> bool {
  view
    .invocations
    .iter()
    .rev()
    .find(|invocation| invocation.execution == execution)
    .is_some_and(|invocation| invocation.outcome == Some(InvocationOutcome::TurnStopped))
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
      // A step whose turn the person stopped waits for them, not for an answer the Orchestrator could give.
      if choices.iter().any(|asked| asked.request.execution() == execution) || stopped_by_person(&view, execution) {
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
      place: None,
      others: 0,
      own: false,
      automatic: false,
      continuing: None,
      merging: None,
    }
  }
}
