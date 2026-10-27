//! The `orchestrator` scenario's waits that still stand: tasks on Feature delivery whose latest runs truly wait (an
//! agent's question, a plan to decide) or stand where they ended (interrupted, ready to merge), each kept under the
//! key the app's follower keeps it by (`orchester_workspace::orchestrator::waits`), so the follower finds nothing new
//! to keep and the conversation reads them as standing (`ChatView::standing`).

use orchester_core::{
  conversation::Sender,
  orchestrator::{AgentWait, RunEnd, WaitKind, change::StepName},
  resource::StepKey,
  run::{Plan, Run},
  store::{ConversationStore, RunStore},
  task::Task,
  time::Timestamp,
};

use crate::kontur::{
  Kontur, SeedError,
  catalogue::Catalogue,
  shapes::{self, Shape},
};

const CRITERIA: [&str; 2] = ["The change is covered by tests.", "Nothing changes for existing users."];

/// A wait as the conversation keeps it, and the key the follower knows it by.
pub struct Kept {
  pub wait: AgentWait,
  pub key: String,
}

/// A task's run, and the task.
pub struct Ran {
  pub task: Task,
  pub run: Run,
}

/// Task `title` (`brief`) on Feature delivery, left in `shape`, its run started at `at`.
pub fn task_in(
  kontur: &mut Kontur,
  catalogue: &Catalogue,
  (title, brief): (&str, &str),
  (shape, at): (Shape, Timestamp),
) -> Result<Ran, SeedError> {
  let pipeline = Some(&catalogue.feature_delivery);
  let created = Timestamp::from_unix_millis(at.unix_millis() - 30 * 60_000);
  let mut task = kontur.task((title, brief, &CRITERIA), pipeline, created)?;
  shapes::apply(kontur, &mut task, pipeline, shape, at)?;
  let task = kontur.find(&task.key().to_string())?;
  let run = kontur
    .db
    .runs_of_task(task.id())?
    .into_iter()
    .flatten()
    .last()
    .ok_or(SeedError::NotAtWork)?;
  Ok(Ran { task, run })
}

/// A draft `title` (`brief`) on Feature delivery, every detail set: what a proposed start starts.
pub fn draft(
  kontur: &mut Kontur,
  catalogue: &Catalogue,
  (title, brief): (&str, &str),
  at: Timestamp,
) -> Result<Task, SeedError> {
  let mut task = kontur.task((title, brief, &CRITERIA), Some(&catalogue.feature_delivery), at)?;
  shapes::apply(
    kontur,
    &mut task,
    Some(&catalogue.feature_delivery),
    Shape::ReadyDraft,
    at,
  )?;
  Ok(task)
}

/// The wait of `ran`'s Implement agent, which asked a question and waits for the answer.
pub fn asked(kontur: &Kontur, ran: &Ran) -> Result<Kept, SeedError> {
  let execution = *ran.run.awaiting_answer().first().ok_or(SeedError::NotAtWork)?;
  let last = kontur
    .db
    .messages(ran.task.id())?
    .into_iter()
    .rev()
    .find(|message| matches!(message.sender(), Sender::Agent { execution: by, .. } if *by == execution))
    .map_or_else(|| "-".to_owned(), |message| message.id().to_string());
  Ok(Kept {
    wait: wait_of(WaitKind::Message, ran, "implement", Some("Builder")),
    key: format!("message:{execution}:{last}"),
  })
}

/// The wait of `ran`'s Plan step, whose plan waits for a decision.
pub fn planned(kontur: &Kontur, ran: &Ran) -> Result<Kept, SeedError> {
  let execution = *ran.run.awaiting_plan().first().ok_or(SeedError::NotAtWork)?;
  let plans = kontur.db.plans(ran.run.id())?;
  let plan = Plan::latest_of(&plans, execution).ok_or(SeedError::NotAtWork)?;
  Ok(Kept {
    wait: wait_of(WaitKind::Plan, ran, "plan", Some("Planner")),
    key: format!("plan:{execution}:{}", plan.version.get()),
  })
}

/// The end `end` of `ran`'s run, which stands where it ended, stopped at `step` when it stopped at one.
pub fn ended(kontur: &Kontur, ran: &Ran, (end, step): (RunEnd, Option<&str>)) -> Result<Kept, SeedError> {
  let event = kontur
    .db
    .ended_runs_since(Timestamp::from_unix_millis(0))?
    .into_iter()
    .find(|ended| ended.run == ran.run.id())
    .ok_or(SeedError::NotAtWork)?;
  let mut wait = wait_of(WaitKind::Ended(end), ran, step.unwrap_or_default(), None);
  if step.is_none() {
    (wait.place, wait.step) = (None, None);
  }
  Ok(Kept {
    wait,
    key: format!("end:{}:{}", event.run, event.sequence.get()),
  })
}

/// A wait of `kind` of `ran`'s step `step`, by `agent`, as the follower makes it.
fn wait_of(kind: WaitKind, ran: &Ran, step: &str, agent: Option<&str>) -> AgentWait {
  let key = ran.task.key().to_string();
  let named = StepKey::new(step)
    .ok()
    .and_then(|step| StepName::in_run(&ran.run, &step));
  AgentWait {
    kind,
    title: ran.task.details().title.as_str().to_owned(),
    run: ran.run.id(),
    number: ran.run.number().get(),
    place: named.as_ref().map(|named| named.place),
    step: named.map(|named| named.name),
    others: 0,
    agent: agent.map(str::to_owned),
    link: format!("/tasks/{key}"),
    task: key,
    own: false,
    automatic: false,
    continuing: None,
    merging: None,
  }
}
