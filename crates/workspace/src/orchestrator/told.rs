//! What the Orchestrator's agent reads of the tools that work otherwise in its chat than for any client of the app
//! (`docs/design/orchestrator-toolset.md`): while the chat drives, only what its turns add to a tool; while it does
//! not, what it proposes and what it never does from a turn Orchester started by itself.

use orchester_core::orchestrator::change::Publishing;
use rmcp::model::Tool;

use super::{
  approval_turn::DECIDE_APPROVAL,
  change::control::{PAUSE_RUN, RELEASE_QUEUE, RESUME_RUN},
  tools::{ANSWER_QUESTION, CANCEL_RUN, CONTINUE_RUN, DUPLICATE_PIPELINE, MERGE_RUN, RUN_TASK, SEND_MESSAGE},
};

/// What the agent reads of a tool that works otherwise in the chat, while it `drives` or not; `None` for one that
/// works as for any client.
pub(super) fn in_chat(tool: &Tool, drives: bool) -> Option<String> {
  let name = tool.name.as_ref();
  let own = tool.description.as_deref().unwrap_or_default();
  if drives {
    return driving(name, own);
  }
  let first = match name {
    RUN_TASK => return Some(PROPOSE_RUN.to_owned()),
    DUPLICATE_PIPELINE => DUPLICATE_AS_DRAFT,
    CANCEL_RUN => PROPOSE_CHANGE,
    MERGE_RUN => PROPOSE_MERGE,
    RESUME_RUN => RESUME_PAUSED,
    PAUSE_RUN => PROPOSE_ON_ITS_OWN,
    RELEASE_QUEUE => return Some(format!("{PROPOSE_ON_ITS_OWN} {RELEASE_REFUSED} {own}")),
    CONTINUE_RUN => PROPOSE_CONTINUE,
    ANSWER_QUESTION | SEND_MESSAGE => PROPOSE_ANSWER,
    _ if Publishing::of_tool(name).is_some() => PROPOSE_PUBLISH,
    _ => return None,
  };
  Some(format!("{first} {own}"))
}

/// What the agent reads of tool `name`, whose own description is `own`, while the chat drives.
fn driving(name: &str, own: &str) -> Option<String> {
  let first = match name {
    RUN_TASK => DRIVE_RUN,
    DUPLICATE_PIPELINE => DRIVE_DUPLICATE,
    MERGE_RUN => DRIVE_MERGE,
    RESUME_RUN => DRIVE_RESUME,
    DECIDE_APPROVAL => DRIVE_APPROVAL,
    _ => return None,
  };
  Some(format!("{first} {own}"))
}

/// What the agent reads first of `run_task` while the chat drives.
const DRIVE_RUN: &str = "Starts the run at once, through the same checks as for any client. A task marked Blocked \
  (its blocked_reason) is refused as task_blocked, naming the reason: you never start it; the person may start it in \
  the app. In a turn Orchester started by itself (its message begins with [Orchester]) it starts only in the turn of a \
  run's success you merged where an order of what you lead applies (not_merge_turn otherwise), only a task this \
  conversation leads (not_led otherwise) and only one that has no run yet (already_ran: running a task again is the \
  person's); in a turn a day's report or a watching's end started it is \
  refused (report_turn). There, start only what the turn's message says you may.";

/// What the agent reads first of `duplicate_pipeline` while the chat drives.
const DRIVE_DUPLICATE: &str = "In this chat it saves the copy as a draft, and publishes its first version as \
  publish_pipeline does: a copy always widens what agents may do, so it is refused (person_only) unless a person \
  turned on Let automation change agent permissions. The answer names the copy and how its publication went.";

/// What the agent reads first of `merge_run` while the chat drives.
const DRIVE_MERGE: &str = "Merges at once. In a turn Orchester started by itself it merges only a run the chat's rule \
  lets it: the task's latest run, succeeded, every step passed (a step accepted as it was counts, whoever accepted it), the task \
  not saying Don't merge automatically, not marked Blocked, no unfinished task blocking it, new starts not held; \
  another run is refused as left_to_person, naming why. The merge keeps the rules below: only a task's latest run \
  that succeeded, and only when the main checkout needs no local integration; a refusal (needs_decision, \
  merge_failed) leaves conflict choices and approving the merged result to the person on the Local integration page. \
  When merge_failed names conflicting files, the next step is continue_run with resolve_conflicts, which sends the \
  run back to an agent that may write to resolve them; or the person resolves them on the Local integration page.";

/// What the agent reads first of `decide_approval` while the chat drives.
const DRIVE_APPROVAL: &str = "Decides one tool request at once. In a turn Orchester started by a tool request it \
  decides only that request (another is refused as not_this_request). Approve it once only when it plainly serves the \
  task and is safe: inside the run's worktree, nothing that cannot be undone, nothing that reaches another system or \
  person, no secret. list_approvals names a request's hold (hold), a hard rule of auto mode \
  that would send it to a person: a reason to look harder, not a refusal; a push, rewriting history, deleting outside \
  the worktree or touching credentials only when the task plainly needs it. One auto mode's reviewer still judges is \
  refused (reviewer_judging). Deny it, with a message that tells the agent why, only when it is \
  plainly wrong. Give your reason \
  in one sentence (reason): the conversation shows it under the request. While the chat drives, Let the Orchestrator \
  drive is what lets it answer, whatever Let automation answer approvals says. When you are not sure, do not call it: \
  tell the person what is asked and what you recommend; the request waits for them on its card. It never allows a \
  tool or command for good.";

/// What the agent reads first of `resume_run` while the chat drives.
const DRIVE_RESUME: &str = "Resumes a run a pause stopped, and continues a run that a crash, the runner going away or \
  Quit now interrupted, or that the runner stopped because the project database stayed busy (status runner_failure \
  storage_contention), from its last checkpoint. In a turn Orchester started by itself it continues only the run whose \
  interruption started the turn, once, not when it was interrupted again within 10 minutes of the time before \
  (interrupted_again), and not while more than 3 runs of the project were stopped by the busy database within 10 \
  minutes (storage_contention). A run stopped at a limit is refused (use_continue_run): continue_run decides it. One the person canceled, one \
  the runner could not record for any other reason and one whose worktree is gone are refused here (person_only).";

/// What the agent reads of `run_task` while the chat does not drive: it proposes the start, unless the project allows
/// Autopilot.
const PROPOSE_RUN: &str = "Proposes running a task to the person: checks that the task could run now (a draft is \
  queued and started, a queued task starts, an ended one runs again) and starts nothing. The person starts or \
  dismisses it in the conversation. When the project allows Autopilot (get_approvals' allow_autopilot), it \
  starts the run instead, through the same checks, and answers with the run, but never in a turn Orchester started \
  by itself: there it only proposes. A task marked Blocked (its blocked_reason) is refused as task_blocked, naming \
  the reason: you never start or propose it; the person may start it in the app. Refusals name why.";

/// What the agent reads first of `duplicate_pipeline`.
const DUPLICATE_AS_DRAFT: &str = "In this chat it saves the copy as a draft, and its first version is published as \
  publish_pipeline publishes in this chat: proposed to the person. The answer names the copy and how its publication \
  went.";

/// What the agent reads first of `cancel_run`.
const PROPOSE_CHANGE: &str = "In this chat it changes nothing itself: it checks what the call names and proposes the \
  change to the person, who applies or dismisses it in the conversation; once applied, it is done as follows.";

/// What the agent reads first of `merge_run`.
const PROPOSE_MERGE: &str = "In this chat it checks the run and proposes the merge to the person, who applies or \
  dismisses it in the conversation. Call it only for a run the person asked you in this chat to merge. The merge \
  keeps the rules below: only a task's latest run that succeeded, and only when the main checkout needs no local \
  integration; a refusal (needs_decision, merge_failed) leaves conflict choices and approving the merged result to \
  the person on the Local integration page. When merge_failed names conflicting files, the next step is to propose \
  continue_run with resolve_conflicts, which sends the run back to an agent that may write to resolve them; or the \
  person resolves them on the Local integration page. A refusal with the code integration_preflight_failed or \
  program_running merged nothing and left nothing on that page: tell the person its reason as the tool gave it, and \
  that merge_run can be asked again; never say it was left to them there.";

/// What the agent reads first of `resume_run`.
const RESUME_PAUSED: &str = "In this chat it resumes a run a pause stopped, and in a turn Orchester started by itself \
  it only proposes that. A run that a crash, the runner going away or Quit now interrupted, or that the runner stopped \
  because the project database stayed busy, is continued by the person on the task's Summary and refused here \
  (person_only), as are a run stopped at a limit (its budget or time limit, the daily or a group's budget, a step's \
  budget, an account's plan limit), one the person canceled, one the runner could not record for any other reason and \
  one whose worktree is gone.";

/// What the agent reads first of `pause_run` and `release_queue`.
const PROPOSE_ON_ITS_OWN: &str = "In a turn Orchester started by itself (its message begins with [Orchester]) it \
  only proposes this to the person, who applies or dismisses it in the conversation; otherwise it is done as \
  follows.";

/// What the agent reads of `release_queue` after [`PROPOSE_ON_ITS_OWN`]: which holds and releases are refused.
const RELEASE_REFUSED: &str = "In a turn Orchester started by itself a release is refused before it is proposed when \
  its task is in no reviewed group (not_in_group), nothing is held there (nothing_held) or only a person's hold is \
  (person_only). In any turn a release where nothing is held is refused (nothing_held), and so is hold_queue where \
  new starts are held already (already_held): nothing was changed.";

/// What the agent reads first of `continue_run`.
const PROPOSE_CONTINUE: &str = "In this chat it takes only allow_retry, return_to and resolve_conflicts: allow_retry \
  and return_to for a run whose step used up its retries, broke again, or waits unverified or unchanged, and \
  return_to also for a run that ended where it offers it; resolve_conflicts for a run whose merge stopped at \
  conflicts (merge_failed), with an optional step whose agent may write and an optional note. It checks the choice \
  as below and proposes it to the person, who applies or dismisses it in the conversation. raise_limits, \
  raise_step_budget, resume and accept are the person's and refused here (person_only). Give return_to a note that \
  tells the agent what to do differently.";

/// What the agent reads first of `answer_question` and `send_message`.
const PROPOSE_ANSWER: &str = "In this chat it checks the answer and proposes it to the person, who applies or \
  dismisses it in the conversation. Once applied, it is recorded as automation's and the task's conversation names \
  you as its sender. A message that would decide the plan a run waits on is refused (plan_decision_person_only): \
  plan decisions are the person's. It is done as follows.";

/// What the agent reads first of a tool that publishes a draft.
const PROPOSE_PUBLISH: &str = "In this chat it proposes the publication to the person, who applies or dismisses it \
  in the conversation. Once applied, it is done as follows, and what widens what agents may do still needs Let \
  automation change agent permissions.";
