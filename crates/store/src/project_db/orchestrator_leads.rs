//! What an Orchestrator conversation's turns are told of what it leads (ORC-769, `orchester_core::orchestrator::
//! Leads`), read when the turn starts: its subscriptions that have not ended, each named as people read its target and
//! with the order that applies (the person's text, else the project's default order), for the standing instructions
//! of every turn ([`ProjectDb::chat_leads`]); and, in a turn a wait started by itself, the subscription its event came
//! through, the most specific of the conversation's own that takes the wait's task in
//! ([`ProjectDb::wait_turn_prompt`]). Nothing here changes a subscription.

use orchester_core::{
  id::{ChatId, RunId, TaskId},
  orchestrator::{
    AcceptedUnverified, AgentWait, Lead, Leads, Merging, Subscription, Subscriptions, WatchTarget,
    instructions::wait_prompt, watch::event_subscription,
  },
  store::{ActivityStore, RunStore, TaskStore},
  task::Task,
};
use rusqlite::OptionalExtension;

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

impl ProjectDb {
  /// What `chat` leads now, each subscription named by [`Self::watch_target_label`], with the project's default
  /// order.
  pub fn chat_leads(&self, chat: ChatId) -> Result<Leads, StoreError> {
    let leads = self
      .chat_subscriptions(chat)?
      .iter()
      .map(|subscription| Ok(Lead::new(subscription, self.watch_target_label(subscription.target())?)))
      .collect::<Result<Vec<_>, StoreError>>()?;
    Ok(Leads::new(leads, &self.orchestrator_watch()?))
  }

  /// The prompt of the turn `wait` started by itself in `chat`: [`wait_prompt`] in a conversation that leads nothing;
  /// otherwise [`Leads::event_prompt`], with what it leads and the subscription the wait came through.
  pub fn wait_turn_prompt(&self, chat: ChatId, wait: &AgentWait) -> Result<String, StoreError> {
    let leads = self.chat_leads(chat)?;
    let prompt = if leads.is_empty() {
      wait_prompt(wait)
    } else {
      leads.event_prompt(wait, self.came_through(chat, wait.run)?)
    };
    Ok(match self.accepted_note(wait)? {
      Some(note) => format!("{prompt}\n\n{note}"),
      None => prompt,
    })
  }

  /// For the turn of a run's success that merges it, the steps of the run automation accepted as they were and what
  /// they left unverified ([`AcceptedUnverified::note`]); `None` for any other turn, and for a run automation accepted
  /// no step of.
  fn accepted_note(&self, wait: &AgentWait) -> Result<Option<String>, StoreError> {
    if wait.merging != Some(Merging::Merges) {
      return Ok(None);
    }
    let Some(run) = self.run(wait.run).map_err(|error| corrupt("run", error))? else {
      return Ok(None);
    };
    let events = self.events(wait.run).map_err(|error| corrupt("run events", error))?;
    let outputs = self.step_outputs(wait.run)?;
    Ok(AcceptedUnverified::note(&AcceptedUnverified::of_run(
      &run, &events, &outputs,
    )))
  }

  /// The target of the subscription of `chat` an event of `run`'s task comes through ([`Self::task_came_through`]);
  /// `None` also once the run or its task is gone.
  pub fn came_through(&self, chat: ChatId, run: RunId) -> Result<Option<WatchTarget>, StoreError> {
    match self.run_task(run)? {
      Some(task) => self.task_came_through(chat, &task),
      None => Ok(None),
    }
  }

  /// The target of the subscription of `chat` an event of `task` comes through: the most specific of its own that
  /// takes the task in ([`event_subscription`]); `None` when none does.
  pub fn task_came_through(&self, chat: ChatId, task: &Task) -> Result<Option<WatchTarget>, StoreError> {
    let own = Subscriptions::new(self.chat_subscriptions(chat)?)
      .map_err(|error| corrupt("orchestrator subscriptions", error))?;
    Ok(event_subscription(&own, chat, task.id(), task.module()).map(Subscription::target))
  }

  /// `target` as a conversation's prompts and tools name it: a task's key and title, a module's name, the project's
  /// name, or *Tasks without a module*.
  pub fn watch_target_label(&self, target: WatchTarget) -> Result<String, StoreError> {
    Ok(match target {
      WatchTarget::Task(id) => self.task(id)?.map_or_else(
        || "a task the project no longer keeps".to_owned(),
        |task| format!("{} ({})", task.key(), task.details().title.as_str()),
      ),
      WatchTarget::Module(id) => self.module(id)?.map_or_else(
        || "a module the project no longer keeps".to_owned(),
        |module| module.name().as_str().to_owned(),
      ),
      WatchTarget::WithoutModule => "Tasks without a module".to_owned(),
      WatchTarget::Project => self.project().name().as_str().to_owned(),
    })
  }

  /// The task of `run`, while both are kept.
  fn run_task(&self, run: RunId) -> Result<Option<Task>, StoreError> {
    let task: Option<String> = self
      .connection
      .query_row("SELECT task_id FROM runs WHERE id = ?1", [run.to_string()], |row| {
        row.get(0)
      })
      .optional()?;
    match task {
      Some(task) => self.task(parse::<TaskId>("task id", &task)?),
      None => Ok(None),
    }
  }
}
