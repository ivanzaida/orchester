//! The prompt of the turn an event of a led target starts (`docs/plan/orchestrator-autonomy/OA-13-…`, `OA-15-…`,
//! `OA-20-…`): work can start, the target is stuck, or a module whose every task is done waits for its acceptance or
//! its delivery.

use crate::{
  module::delivery::PersonsDelivery,
  orchestrator::{
    WatchTarget,
    standing::{Raised, StandingEvent},
  },
};

/// The prompt of the Orchestrator turn `event` started by itself. Its first line says what happened, which is what a
/// rewound conversation is told of it.
pub fn standing_prompt(event: &StandingEvent) -> String {
  let name = event.name.as_deref().unwrap_or_default();
  let target = match event.target {
    WatchTarget::Module(_) => format!("the module {name}"),
    WatchTarget::Task(_) => format!("the task {name}"),
    WatchTarget::WithoutModule => "the tasks without a module".to_owned(),
    WatchTarget::Project => "every task of the project".to_owned(),
  };
  let nobody = "Nobody asked you in this chat: Orchester started this turn because Let the Orchestrator drive is on.";
  match &event.raised {
    Raised::CanStart { tasks } => format!(
      "[Orchester] This conversation leads {target} to the end, and work can start now: {tasks}.\n{nobody} Read \
       where the target stands with get_standing and each task with get_task before you start it. Start \
       with run_task what should start now, as many as agents are free; start tasks that would change the same files \
       one after the other, not together. A task whose run failed is run again with run_task and your reason, within \
       its ceiling of runs, only when you see why it failed and that another run can pass. What only the person can \
       move, park with park_task and go on with the rest.",
      tasks = tasks.join(", "),
    ),
    Raised::Stuck { left, budget_used } => {
      let left: Vec<String> = left
        .iter()
        .map(|left| match &left.note {
          Some(note) => format!("{} ({}: {note})", left.task, left.standing),
          None => format!("{} ({})", left.task, left.standing),
        })
        .collect();
      let budget = if *budget_used {
        " The budget of this leading is used, so nothing more is started under it."
      } else {
        ""
      };
      format!(
        "[Orchester] This conversation leads {target} to the end, and it is stuck: nothing can start and nothing \
         moves. What is left: {left}.\n{nobody}{budget} Tell the person once, clearly and briefly: for each task what \
         it waits for and what exactly they are to do, with its link. Start, continue and merge nothing in this \
         turn; you are told again when where it stands changes.",
        left = left.join("; "),
      )
    }
    Raised::Accept => format!(
      "[Orchester] This conversation leads {target} to the end, and every task of it is done: its work is on the \
       module's own branch, and nothing looked at the whole of it yet.\n{nobody} Start its acceptance with \
       start_acceptance and your reason: Orchester makes the acceptance task and its brief, and its run reviews the \
       module's whole diff against its base branch. When that run ends you are told. If it finds defects, create \
       one task of the module for each finding (create_task), cancel the acceptance task, and lead those tasks to \
       the end: the acceptance is started again after them."
    ),
    Raised::Deliver => format!(
      "[Orchester] This conversation leads {target} to the end, every task of it is done and its acceptance passed \
       for the branch as it is now.\n{nobody} Ask for its delivery with deliver_module and your reason: it merges \
       the module's branch into its base branch. A delivery that is refused (the base moved and the merge \
       conflicts, the person's checkout stands in the way, an Admin's authorization is missing) is the person's: \
       say what was refused and what they are to do, and deliver nothing by another way."
    ),
    Raised::DeliveryWaits { why } => {
      let why = match why {
        PersonsDelivery::NoAcceptance => {
          "the module names no acceptance pipeline, so nothing looked at the whole of it: its delivery is the \
           person's"
        }
        PersonsDelivery::AcceptanceCeiling => {
          "its acceptances did not pass, as many as you start by yourself: what comes next is the person's"
        }
        PersonsDelivery::NotAllowed => {
          "its acceptance passed, and Deliver when accepted is off for it: its delivery is the person's"
        }
      };
      format!(
        "[Orchester] This conversation leads {target} to the end, and every task of it is done; {why}.\n{nobody} \
         Tell the person once, clearly and briefly, what was done, what the acceptance found if one ran, and that \
         the module waits for them to deliver it, with its link. Start, merge and deliver nothing in this turn."
      )
    }
  }
}
