//! What the Orchestrator chat wrote to a run's agents, as the tools give it (ORC-45): the chat as an author, with how
//! what it wrote went out (sent by the chat itself, or applied by the person) and its conversation; and the answers
//! and messages it proposed that no one applied, read with the conversation's rules
//! ([`crate::tasks::conversation::orchestrator`]). A proposal was never sent, whatever became of it.

use std::path::Path;

use orchester_core::{
  id::{RunId, TaskId},
  orchestrator::{FromOrchestrator, SentWay},
  question::AskedQuestions,
  run::Run,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{questions, wire};
use crate::{
  app_mcp::error::ToolError,
  tasks::{
    conversation::orchestrator::{Proposal, ProposalState, Proposed, message_offer, proposals},
    detail::TaskDetail,
  },
};

/// The Orchestrator chat as the author of what `from` says it wrote: how it went out (`direct`, `applied_by_person`, or
/// `null` for what was kept before that was recorded) and its conversation.
pub(super) fn author(from: FromOrchestrator) -> Value {
  json!({
    "kind": "orchestrator",
    "sent": from.way.map(SentWay::word),
    "chat": from.chat.map(|chat| chat.to_string()),
  })
}

/// What `get_run` lists of `run` in the project at `root` that waits for the person besides approvals and plans: its
/// waiting questions among `asked`, each with the answer the chat proposed for it, and the answers and messages the
/// chat proposed that wait; `link`, the task's conversation. Blocks on SQLite.
pub(super) fn awaiting(
  root: &Path,
  run: &Run,
  (asked, link): (&[AskedQuestions], &str),
) -> Result<(Vec<Value>, Vec<Value>), ToolError> {
  let proposed = of_run(root, (run.task(), run.id()), asked)?;
  let questions = questions::waiting(asked, (&proposed, link));
  let number = (run.id(), run.number().get());
  let waiting = proposed
    .iter()
    .filter(|waits| waits.waits())
    .map(|waits| proposal(waits, number, link))
    .collect();
  Ok((questions, waiting))
}

/// The answers and messages the chat proposed for `run` of `task` in the project at `root`, whatever became of them
/// but sent; `questions` are the run's. A message is stale once the run would no longer take it as it was checked.
/// Blocks on SQLite.
fn of_run(root: &Path, (task, run): (TaskId, RunId), questions: &[AskedQuestions]) -> Result<Vec<Proposal>, ToolError> {
  let detail = TaskDetail::load_in(root, task)?;
  let offered = message_offer(&detail, run);
  let kept = ProjectDb::open(root)?.run_answer_actions(run)?;
  Ok(proposals(&kept, (offered.as_ref(), questions)))
}

/// `proposal`, of run `#number` (`run`): where it stands, what it would send and the chat's conversation that holds
/// it; `link`, the task's conversation, where a person applies or dismisses one that waits, as in the chat.
pub(super) fn proposal(proposal: &Proposal, (run, number): (RunId, u32), link: &str) -> Value {
  let (state, reason) = match &proposal.state {
    ProposalState::Waits => ("waits_for_person", None),
    ProposalState::Applying => ("applying", None),
    ProposalState::Dismissed => ("dismissed", None),
    ProposalState::Stale => ("stale", None),
    ProposalState::Refused(reason) => ("refused", Some(&**reason)),
  };
  json!({
    "cursor": &*proposal.key(),
    "kind": "orchestrator_proposal",
    "author": { "kind": "orchestrator", "sent": Value::Null, "chat": proposal.chat.to_string() },
    "state": state,
    "reason": reason,
    "proposed": proposed(&proposal.proposed),
    "sent": false,
    "run": { "id": run.to_string(), "number": number },
    "at": wire::time(proposal.at),
    "link": link,
  })
}

/// The answer the chat proposed for a waiting question, while it waits for the person; `null` when there is none.
pub(super) fn proposed_answer(waiting: Option<&Proposal>) -> Value {
  waiting.map_or(Value::Null, |proposal| {
    let mut proposed = proposed(&proposal.proposed)["answer"].take();
    proposed["by"] = json!({ "kind": "orchestrator", "chat": proposal.chat.to_string() });
    proposed["waits_for_person"] = json!(true);
    proposed
  })
}

fn proposed(proposed: &Proposed) -> Value {
  match proposed {
    Proposed::Message { text, then } => json!({ "message": { "text": &**text, "then": then } }),
    Proposed::Answer { request, choices, text } => {
      let choices: Vec<&str> = choices.iter().map(|choice| &**choice).collect();
      json!({ "answer": { "question": &**request, "choices": choices, "text": text.as_deref() } })
    }
  }
}
