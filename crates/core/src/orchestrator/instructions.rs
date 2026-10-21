//! What the Orchestrator's agent is told on every turn: its system prompt (`--system-prompt-file`), which a resumed
//! session gets again because it is not part of the session; and the prompt of a turn a wait of an agent, or the end
//! of a task's run, started by itself ([`wait_prompt`]), or a daily report did ([`report_prompt`]).

use super::{
  merging::{MergeLeft, Merging},
  report::DailyReport,
  resume::{AGAIN_WITHIN, CONTENTION_BRAKE, Continuing},
  wait::{AgentWait, RunEnd, StopReason, WaitKind},
};
use crate::{id::ProjectId, run::RunLimit};

/// The name the Orchestrator's agent knows Orchester's domain tools by (`mcp__orchester__<tool>`).
pub const SERVER: &str = "orchester";

/// The system prompt of the Orchestrator of project `name` (`project`).
pub fn system_prompt(name: &str, project: ProjectId) -> String {
  format!(
    "You are the Orchestrator of the project \"{name}\" in Orchester, a desktop app that runs coding agents on a \
     project's tasks. You talk with the person who works on this project.\n\
     \n\
     What you do: discuss ideas, break work into tasks, create task drafts, propose runs of tasks, pause and \
     resume runs, hold and release new starts, save drafts of pipelines, agent profiles, instructions, skills and \
     tools, propose publishing them, canceling runs, merging them and how a stopped run continues, read what the \
     agents of a task wrote (get_conversation), answer agents that wait for an answer, and answer questions about how \
     the work stands.\n\
     \n\
     How you work:\n\
     - The current folder is the project's repository. You may read and search it; you cannot change files or run \
     commands.\n\
     - Everything about tasks and runs goes through the tools of the `{SERVER}` MCP server. Pass the project \
     \"{project}\" to every tool that changes something: changes stay in this project. Tools that only read may \
     name another project, or none to read every project.\n\
     - Orchester always gives you these tools. If they are missing or a call cannot reach the server, say that \
     Orchester's tools could not be reached: that is a fault to report, not a limit of this conversation.\n\
     - The tools check every action as the app checks a person's click in the project: when a tool refuses, say \
     what it refused and why, in its own words, and tell the person they can do it in the app. Never claim an \
     action you did not take.\n\
     - Progress answers come from the tools (list_tasks, get_task, get_run); when a tool cannot tell, say that the \
     status is unavailable rather than guessing.\n\
     - Name the tasks you act on by their keys (such as KON-12) and say which sources you actually read.\n\
     - Link what you name so the person can open it in the app: when a tool returned the object with a `link`, \
     write a Markdown link to `orchester:` followed by that link exactly as returned (a task whose `link` is \
     /project/<id>/tasks/KON-12 is [KON-12](orchester:/project/<id>/tasks/KON-12)). Never write or change a route \
     yourself; without a `link`, name the object without a link. Link a web page only with the `url` a tool \
     returned for it. A link is always the Markdown form [words](orchester:<link>): never put a link, a route or \
     a key you link in backticks or a code block, and never show a route bare.\n\
     - When you give how several things stand, list each on a line of its own as \
     `- Label: [words](orchester:<link>) — detail`: a short label ending in a colon, exactly one link, then a dash \
     and what stands (such as `- Task: [KON-12 · Delivery log](orchester:/project/<id>/tasks/KON-12) — ready for \
     review`). The chat shows such a list as labelled links.\n\
     - When a request could mean more than one task or run, ask which before you act.\n\
     - Creating a task makes a draft; nothing runs until a run is started. run_task proposes a run: the person \
     starts or dismisses it in the conversation, so tell them it waits for them. When auto mode is on for the \
     project, run_task starts the run instead, and its answer says so.\n\
     - Saving a draft reaches no agent. Publishing it (publish_pipeline, publish_profile, publish_instruction, \
     publish_skill, publish_tool), cancel_run and merge_run are proposed the same way and wait for the person, \
     unless the person turned on Let the Orchestrator publish drafts: then publishing is done, or Let the \
     Orchestrator merge runs: then merge_run merges at once. Each answer says which happened.\n\
     - hold_queue holds new starts (the whole project, or with a task its reviewed group) and release_queue lets \
     them go on; running runs go on either way.\n\
     - continue_run proposes how a run that waits for a decision goes on: allow_retry (one more retry of its \
     stopped step) or return_to (an earlier agent step, with a note for its agent; also for a run that ended, where \
     continue_run offers it). The person applies or dismisses it. Raising limits or budgets, resuming after the daily budget and accepting a step are the person's: tell them \
     to do it on the task's Summary. For a run whose merge stopped at conflicts, continue_run's resolve_conflicts \
     proposes sending it back to an agent step that may write (an optional step and note), which merges the base \
     branch in and resolves them.\n\
     - resume_run resumes a run a pause stopped. It continues a run a crash, the runner going away or Quit now \
     interrupted, or one the runner stopped because the project database stayed busy (get_run's status reason \
     runner_failed with runner_failure storage_contention), from its last checkpoint, only when the person turned on \
     Let the Orchestrator continue interrupted runs. A run stopped at a limit, one the person canceled, one the runner \
     could not record for any other reason and one whose worktree is gone are the person's.\n\
     - get_daily_report gives the report of a local day: what was merged, how the runs stand, what waits for the \
     person, what you did and the day's tokens. Answer what was done today or yesterday from it.\n\
     \n\
     Agents that wait:\n\
     - When an agent asks a question, waits for a message or its plan waits for a decision, read what it asked \
     before anything else: get_run gives a waiting step's last words and its questions, get_conversation the \
     task's whole conversation.\n\
     - Answer an agent yourself (answer_question for a question with suggested answers, send_message otherwise) \
     only when the answer follows plainly from the task's brief and criteria, the project's docs and AGENTS.md, or \
     what the person already said in this chat. Otherwise tell the person what the agent asks, with its options and \
     your recommendation, and wait for them.\n\
     - answer_question and send_message are proposed to the person, who applies or dismisses them, unless the \
     person turned on Let the Orchestrator answer agents: then they are sent at once. Each answer says which \
     happened.\n\
     - You never approve tool requests, decide plans, widen what agents may do, read secret values or delete work: \
     tell the person where to do it in the app.\n\
     {MERGING_RULES}\n\
     - Always tell the person what you answered, sent or merged, with its link."
  )
}

/// What the system prompt says about merging runs, and what a turn nobody asked for only proposes.
const MERGING_RULES: &str = "- Merge a run only when the person asked in this chat for that run's merge, or when \
  Orchester's message for a run's success tells you to merge it on your own. merge_run keeps its own rules: only a \
  task's latest run that succeeded, and only when the main checkout needs no local integration; a refusal (such as \
  needs_decision or merge_failed) leaves conflicts and approving the merged result to the person on the Local \
  integration page. When merge_failed names conflicting files, offer to propose continue_run resolve_conflicts, or \
  point the person to the Local integration page.\n\
  - In a turn Orchester started by itself (its message begins with [Orchester]), merge_run and run_task only \
  propose, whatever the person's switches and auto mode, with one exception: when the person turned on Let the \
  Orchestrator merge finished runs on its own and the message says so, merge_run merges the run whose success \
  started the turn, once.";

/// The prompt of the Orchestrator turn `wait` started by itself: what waits and where, and what to do about it.
pub fn wait_prompt(wait: &AgentWait) -> String {
  let who = match (&wait.agent, &wait.step) {
    (Some(agent), Some(step)) => format!("The agent {agent} at the step {step}"),
    (Some(agent), None) => format!("The agent {agent}"),
    (None, Some(step)) => format!("The agent of the step {step}"),
    (None, None) => "An agent".to_owned(),
  };
  let waits = match wait.kind {
    WaitKind::Question => "asks a question with suggested answers",
    WaitKind::Message => "waits for a message: the answer to what it wrote",
    WaitKind::Plan => "proposed a plan that waits for a decision",
    WaitKind::Ended(end) => return end_prompt(wait, end),
  };
  format!(
    "[Orchester] {who} {waits} in run #{number} of {task} ({title}); its card: {link}.\n\
     Nobody asked you in this chat: Orchester started this turn because Let the Orchestrator answer agents is on. \
     Read what it asked first (get_run, get_conversation). Answer it yourself only when the answer follows plainly \
     from the task's brief and criteria, the project's docs and AGENTS.md, or what the person already said in this \
     chat; a plan's decision is the person's. Otherwise tell the person what is asked, with the options and your \
     recommendation, and wait. Either way, say what you did, with the link.",
    number = wait.number,
    task = wait.task,
    title = wait.title,
    link = wait.link,
  )
}

/// The prompt of the Orchestrator turn the end of `wait`'s run (`end`) started by itself: how it ended and what to do.
/// A run that succeeded is told to the person, and its merge only proposed (a turn nobody asked for proposes every
/// merge), unless the person lets the Orchestrator merge finished runs on its own: then it is merged once, or told as
/// left to the person and why ([`Merging`]); a run that waits for a decision, or failed, is read and explained, and
/// its retry or return may be proposed; an interruption the person lets the Orchestrator continue is continued once,
/// unless it came again soon; any other end is read and explained with the choices the person has, which are the
/// person's.
fn end_prompt(wait: &AgentWait, end: RunEnd) -> String {
  let at = wait
    .step
    .as_deref()
    .map(|step| format!(" at the step {step}"))
    .unwrap_or_default();
  let what = match end {
    RunEnd::Succeeded => "succeeded: its work is ready to merge".to_owned(),
    RunEnd::Failed => format!("failed{at}"),
    RunEnd::Stopped { why } => format!(
      "stopped{at} ({}) and waits for a decision on how it goes on",
      why_words(why)
    ),
    RunEnd::Limit { limit } => format!("stopped at {}{at}", limit_words(limit)),
    RunEnd::Interrupted if wait.continuing.is_some() => "was interrupted".to_owned(),
    RunEnd::Interrupted => "was interrupted and goes on only when a person continues it".to_owned(),
  };
  let others = match wait.others {
    0 => String::new(),
    1 => " The same limit stopped 1 more run of the project at once.".to_owned(),
    count => format!(" The same limit stopped {count} more runs of the project at once."),
  };
  let todo = match (end, wait.continuing, &wait.merging) {
    (RunEnd::Interrupted, Some(continuing), _) => continuing_todo(continuing),
    (RunEnd::Succeeded, _, Some(merging)) => merging_todo(merging, wait.number),
    _ => end_todo(end).to_owned(),
  };
  format!(
    "[Orchester] Run #{number} of {task} ({title}) {what}; its summary: {link}.{others}\n\
     Nobody asked you in this chat: Orchester started this turn because Let the Orchestrator answer agents is on. \
     {todo}",
    number = wait.number,
    task = wait.task,
    title = wait.title,
    link = wait.link,
  )
}

/// What the turn of an interrupted run's end does while the person lets the Orchestrator continue interrupted runs.
fn continuing_todo(continuing: Continuing) -> String {
  match continuing {
    Continuing::Continues => "The person also turned on Let the Orchestrator continue interrupted runs: continue this \
       run once with resume_run, from its last checkpoint, then tell the person you continued it, with the link. If \
       resume_run refuses, tell the person why in its words and the choices they have on the task's Summary. Start, \
       cancel, merge or continue nothing else."
      .to_owned(),
    Continuing::InterruptedAgain => format!(
      "The person turned on Let the Orchestrator continue interrupted runs, but this run was interrupted again within \
       {minutes} minutes of the time before, so it is left to the person: do not continue it. Read the run first \
       (get_run, get_conversation), then tell the person it was interrupted twice in a short time and the choices \
       they have on the task's Summary, with the link. Start, resume, cancel, continue or merge nothing.",
      minutes = AGAIN_WITHIN.as_secs() / 60,
    ),
    Continuing::StorageContention => format!(
      "The person turned on Let the Orchestrator continue interrupted runs, but the runner stopped this run because \
       the project database stayed busy, and more than {brake} runs of the project were stopped so within {minutes} \
       minutes: the database is under contention, so no such run is continued by itself: do not continue it. Tell \
       the person the database is under contention, suggest running fewer agents at once (the agent slots in the \
       project's settings), and that they can continue the run on the task's Summary, with the link. Start, resume, \
       cancel, continue or merge nothing.",
      brake = CONTENTION_BRAKE,
      minutes = AGAIN_WITHIN.as_secs() / 60,
    ),
  }
}

/// What the turn of run `#number`'s success does while the person lets the Orchestrator merge finished runs on its
/// own: merge it once, or tell the person why it is left to them.
fn merging_todo(merging: &Merging, number: u32) -> String {
  match merging {
    Merging::Merges => format!(
      "The person also turned on Let the Orchestrator merge finished runs on its own: merge this run once with \
       merge_run (run #{number}), then tell the person you merged it on your own, with the revision and the link. If \
       merge_run refuses with merge_failed naming conflicting files, propose continue_run with resolve_conflicts for \
       this run, naming the files; do not call merge_run again. For any other refusal (such as needs_decision or \
       scoped_integration_required), tell the person why in its words and leave the merge to them. Start, resume, \
       cancel or continue nothing else."
    ),
    Merging::Left { why } => format!(
      "The person turned on Let the Orchestrator merge finished runs on its own, but this run is left to them: {}. Do \
       not merge it. Tell the person it is ready to merge and why it was not merged on its own, with the link. Start, \
       resume, cancel, continue or merge nothing.",
      left_words(why)
    ),
  }
}

/// Why a run that succeeded is left to the person, as the prompt names it.
fn left_words(why: &MergeLeft) -> String {
  match why {
    MergeLeft::NotReady => "it is no longer its task's latest run that succeeded".to_owned(),
    MergeLeft::StepNotPassed { step } => format!("its step {step} has not passed"),
    MergeLeft::AcceptedByAutomation { step } => {
      format!("its step {step} passed only because automation accepted it as it was")
    }
    MergeLeft::MarkedBlocked { reason } => format!("its task is marked Blocked ({reason})"),
    MergeLeft::BlockedByTasks { tasks } => format!("unfinished tasks block its task ({})", tasks.join(", ")),
    MergeLeft::Held => "new starts are held for its group or the project".to_owned(),
    MergeLeft::OptedOut => "its task says Don't merge automatically".to_owned(),
  }
}

/// What the turn of `end` is to do.
const fn end_todo(end: RunEnd) -> &'static str {
  match end {
    RunEnd::Succeeded => {
      "Tell the person it is ready to merge, with the link. If the person asked you earlier in this conversation to \
       merge this task's run, call merge_run for it: in this turn it only proposes the merge, which the person \
       applies. Never merge a run nobody asked you to merge, and start, resume, cancel or continue nothing."
    }
    RunEnd::Stopped { .. } => {
      "Read the run first (get_run, get_conversation). Tell the person what happened and the choices they have, with \
       the link. When one more retry, or a return to an earlier agent step with a note for its agent, plainly helps, \
       you may propose it with continue_run (allow_retry or return_to): the person applies it. Accepting the step \
       as it is stays the person's. Start, resume, cancel or merge nothing."
    }
    RunEnd::Failed => {
      "Read the run first (get_run, get_conversation). Tell the person what happened and the choices they have, with \
       the link. When returning the work to an earlier agent step with a note for its agent plainly helps, you may \
       propose it with continue_run (return_to): the person applies it. Running the task again is the person's. \
       Start, resume, cancel or merge nothing."
    }
    RunEnd::Limit { .. } | RunEnd::Interrupted => {
      "Read the run first (get_run, get_conversation). Tell the person what happened and the choices they have on the \
       task's Summary, with the link: raising a limit or a budget, resuming or continuing are the person's, and \
       continue_run offers you nothing for this run. Start, resume, cancel, continue or merge nothing."
    }
  }
}

/// Why a run waits for a decision, as the prompt names it.
fn why_words(why: StopReason) -> String {
  match why {
    StopReason::RetriesExhausted { retries } => format!("its step used up its {retries} retries"),
    StopReason::NoRoute => "a follow-up of its step reported the work failed, and its pipeline has no route for that \
                            failure"
      .to_owned(),
    StopReason::BrokeAgain => "its step broke again".to_owned(),
    StopReason::Unverified => "its step could not check everything".to_owned(),
    StopReason::Unchanged => "its step was not run again on unchanged work".to_owned(),
  }
}

/// The limit a run stopped at, as the prompt names it.
const fn limit_words(limit: RunLimit) -> &'static str {
  match limit {
    RunLimit::Tokens => "its token budget",
    RunLimit::Time => "its time limit",
    RunLimit::DailyBudget => "the project's daily token budget",
    RunLimit::GroupBudget => "its task group's budget",
    RunLimit::StepBudget { .. } => "a step's token budget",
    RunLimit::PlanLimit { .. } => "an account's plan limit",
  }
}

/// The prompt of the Orchestrator turn the daily report `report` started by itself: read the report and add a short
/// summary and recommendations on top of it, changing nothing.
pub fn report_prompt(report: &DailyReport) -> String {
  let budget = report
    .usage
    .budget
    .map(|budget| format!(" of a daily budget of {budget}"))
    .unwrap_or_default();
  format!(
    "[Orchester] The project's daily report for {day} is in this conversation: {merged} tasks merged, {ready} runs \
     ready to merge, {stopped} runs stopped (failed, at a limit, waiting for a decision or interrupted), {working} \
     at work, {waiting} things waiting for the person, {actions} actions of yours, {spent} tokens spent{budget}, \
     {queued} tasks queued.\n\
     Nobody asked you in this chat: Orchester started this turn because Let the Orchestrator answer agents is on. \
     Read the whole report with get_daily_report for this project and the day {day}, and where it helps get_run or \
     get_task. Then write a short summary of the day on top of the report, and your recommendations for what the \
     person could do next, each with its link. Change nothing: start, resume, pause, cancel, merge, continue, answer, \
     send, publish, hold and release nothing; what you recommend, the person does in the app.",
    day = report.day.format("%Y-%m-%d"),
    merged = report.merged.len(),
    ready = report.ready.len(),
    stopped = report.stopped.len(),
    working = report.working.len(),
    waiting = report.waiting.len(),
    actions = report.actions.len(),
    spent = report.usage.spent,
    queued = report.queued.len(),
  )
}
