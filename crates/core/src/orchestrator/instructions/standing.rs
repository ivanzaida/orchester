//! The prompt of the turn an event of a led target starts (`docs/plan/orchestrator-autonomy/OA-13-…`, `OA-15-…`):
//! work can start, or the target is stuck.

use crate::orchestrator::{
  WatchTarget,
  standing::{Raised, StandingEvent},
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
       where the target stands with get_watches (`standing`) and each task with get_task before you start it. Start \
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
  }
}
