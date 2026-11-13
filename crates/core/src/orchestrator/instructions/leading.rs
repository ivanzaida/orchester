//! What a turn of a subscription that leads to the end is told beside its event's prompt
//! (`docs/plan/orchestrator-autonomy/OA-18-…`): nobody is at the computer, so it reads where the target stands,
//! moves what it may, and ends the event in an action, a parking or a named wait. The tools hold the limits; this
//! holds the judgement. The prompts of a subscription that only follows are not given it.

use crate::orchestrator::{
  task_event::{TaskEvent, TaskEventKind},
  wait::{AgentWait, RunEnd, WaitKind},
};

/// The judgement every leading turn is asked for.
const LEADING: &str = "This conversation leads this task to the end, and nobody is at the computer: you are not   asked to report, you are asked to move it.\n\
  - Read where the target stands first (get_standing), then act on this event.\n\
  - Decide what is the task's to decide. A question the brief, the criteria, the project's docs and AGENTS.md do not   answer is decided in this order: what the design shows; what the app already does for the same thing; what   surprises the person least; the smaller and more easily reverted change.\n\
  - Park what is not yours with park_task, and go on with the rest: anything irreversible or outward (a push, a   release, deleting work, another system, money), anything a tool refuses as the person's, a ceiling that was   reached, a tool that did not answer after another try. Where the order of this conversation asks to be asked   first, park instead of deciding.\n\
  - Accept a step as it is only when what it could not check cannot be checked at all; a tool that did not answer is   tried again, and then the task is parked. Say which step and what was left unverified.\n\
  - When tasks of the target would change the same files, do not run them side by side: make one wait for the other   (update_task blocked_by) where the project keeps that, or start them one after the other.\n\
  - Never wait in words: do not end the turn by telling the person to press something your tools can do, nor by   asking a question nobody is there to answer.\n\
  - Give your reason, in one sentence, with every action that is a judgement.";

/// What the turn of `wait`, an event of a task led to the end, is told beside its prompt. `stall_step`: for a run
/// that makes no progress, which period of the stall this is, from 1.
pub fn leading_note(wait: &AgentWait, stall_step: Option<u32>) -> String {
  match about(wait.kind, stall_step) {
    Some(about) => format!("{LEADING}\n{about}"),
    None => LEADING.to_owned(),
  }
}

/// What a leading turn does about an event of `kind` in particular.
const fn about(kind: WaitKind, stall_step: Option<u32>) -> Option<&'static str> {
  match kind {
    WaitKind::Approval => Some(
      "About this request: when approve_once is refused person_only, it is one only the person may allow. Deny it,        with words for the agent, when the task plainly does not need it; otherwise park the task as an approval,        with what is asked, why it is held and what you recommend, and decide nothing: the request's card stays the        person's. If you already denied a request like it for this agent, park rather than deny again.",
    ),
    WaitKind::Ended(RunEnd::Failed) => Some(
      "About this failure: when the run's work is worth keeping and it can be returned, return it to the right step        with a note (continue_run return_to). When it cannot be continued, run the task again with run_task and your        reason: a new run starts from the base branch, without the failed run's work. Past a ceiling        (ceiling_reached), park the task with what failed each time. If the same step failed the same way after your        note, park rather than repeat the note.",
    ),
    WaitKind::Event(TaskEvent {
      kind: TaskEventKind::NoProgress { .. },
      ..
    }) => Some(match stall_step {
      Some(2) => {
        "About this stall: it is the second period without progress, and your message did not help. Stop the run          with pause_run and continue it from its last checkpoint with resume_run."
      }
      Some(step) if step >= 3 => {
        "About this stall: it is the third period without progress. Park the task (needs: other) with what the run          was doing when it stopped moving, and go on with the rest."
      }
      _ => {
        "About this stall: read what the run did last, and write to its agents with send_message what should get          them going again. You are told again if it still makes no progress."
      }
    }),
    _ => None,
  }
}
