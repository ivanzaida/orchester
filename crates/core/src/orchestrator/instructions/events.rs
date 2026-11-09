//! The prompt of the Orchestrator turn an event of a task's run started by itself ([`super::super::task_event`]): what
//! happened and where, what the order of the conversation that leads the task asks, and what to do where the order
//! says nothing about it.

use crate::orchestrator::{
  task_event::{TaskEvent, TaskEventKind},
  wait::AgentWait,
};

/// The prompt of the Orchestrator turn `event` of `wait`'s run started by itself. `order` is the order of the
/// conversation that leads the task, in the person's words, when one leads it; without one the turn was started
/// because the person lets the Orchestrator drive.
pub fn event_prompt(wait: &AgentWait, event: TaskEvent, order: Option<&str>) -> String {
  let run = format!(
    "run #{number} of {task} ({title})",
    number = wait.number,
    task = wait.task,
    title = wait.title
  );
  let what = match event.kind {
    TaskEventKind::NoProgress { minutes, .. } => {
      format!("The {run} made no progress for {minutes} minutes: no transcript event, step change or tool call")
    }
    TaskEventKind::MergeConflict => format!("Merging {run} hit conflicts"),
    TaskEventKind::WaitedMerged { task, .. } => {
      format!("The {run} waits on the task with id {task}, which was merged: its work is on the base branch")
    }
    TaskEventKind::WaitedClosed { task, .. } => format!(
      "The {run} waits on the task with id {task}, which was closed without merging: the work it waits for will not \
       come"
    ),
  };
  let why = match order {
    Some(order) => format!(
      "Orchester started this turn because this conversation leads the task. Its order, in the person's words: \
       \"{order}\". Do what the order asks about this event, within your rules; where it \
       says nothing about it: {todo}",
      todo = todo(event.kind)
    ),
    None => format!(
      "Orchester started this turn because Let the Orchestrator drive is on. {todo}",
      todo = todo(event.kind)
    ),
  };
  format!(
    "[Orchester] {what}; its card: {link}.\nNobody asked you in this chat: {why}",
    link = wait.link
  )
}

/// What the turn of an event of `kind` does where no order says otherwise.
const fn todo(kind: TaskEventKind) -> &'static str {
  match kind {
    TaskEventKind::NoProgress { .. } => {
      "Read the run first (get_run, get_conversation) to see what it did last. Tell the person it made no progress, \
       what it was doing and the choices they have, with the link. When a message plainly helps its agents go on, you \
       may send one with send_message. Start, resume, cancel or merge nothing."
    }
    TaskEventKind::MergeConflict => {
      "Read the run first (get_run): the merge names the conflicting files. Call continue_run with \
       resolve_conflicts for this run, naming the files, or point the person to the Local integration page; do not \
       call merge_run again. Tell the person what conflicts, with the link. Start, resume, cancel or merge nothing."
    }
    TaskEventKind::WaitedMerged { .. } => {
      "Read the run first (get_run, get_conversation). Tell the person the work it waited for is merged and it can \
       go on, with the link. When its agent waits for a message, you may tell it so with send_message. Start, resume, \
       cancel or merge nothing."
    }
    TaskEventKind::WaitedClosed { .. } => {
      "Read the run first (get_run, get_conversation). Tell the person the task it waited on was closed without \
       merging, and the choices they have (go on without that work, run that task again, or cancel this run), with \
       the link; they are the person's. Start, resume, cancel or merge nothing."
    }
  }
}
