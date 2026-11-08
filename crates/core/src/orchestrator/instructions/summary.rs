//! The prompt of the turn a subscription that ended by itself starts (ORC-764): write the summary of its module or
//! task, changing nothing.

use crate::orchestrator::watch::{EndReason, WatchSummary, WatchTarget};

/// The prompt of the Orchestrator turn `summary`'s end started by itself: read what the module or the task did and
/// write its summary, changing nothing.
pub fn watch_summary_prompt(summary: &WatchSummary) -> String {
  let (what, read) = match summary.target {
    WatchTarget::Module(_) => (
      format!("the module {}", summary.name),
      "Read the module with get_module and its tasks with get_task, and where it helps get_run",
    ),
    _ => (
      format!(
        "the task {}{}",
        summary.name,
        summary
          .title
          .as_deref()
          .map(|title| format!(" «{title}»"))
          .unwrap_or_default()
      ),
      "Read the task with get_task, and where it helps get_run",
    ),
  };
  let why = match summary.reason {
    EndReason::TaskDone => "it is Done".to_owned(),
    EndReason::TaskCanceled => "it is Canceled".to_owned(),
    EndReason::ModuleClosed { done, canceled } => {
      format!("every task of it is Done or Canceled ({done} Done, {canceled} Canceled)")
    }
  };
  let order = summary
    .order
    .as_deref()
    .map(|order| format!(" The person's order for it was: {order}"))
    .unwrap_or_default();
  format!(
    "[Orchester] This conversation watched {what} ({link}), and its watching ended by itself: {why}.{order}\n\
     Nobody asked you in this chat: Orchester started this turn because Let the Orchestrator drive is on. \
     {read}. Then write a short summary of what was done, what was canceled and what is left for the person to look \
     at, each with its link. Change nothing: start, resume, pause, cancel, merge, continue, answer, send, publish, \
     create, update, hold and release nothing; what you recommend, the person does in the app.",
    link = summary.link,
  )
}
