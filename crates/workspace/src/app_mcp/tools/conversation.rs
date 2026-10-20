//! `get_conversation`: a task's conversation as its Conversation tab shows it for its latest run (ORC-42): what the
//! person and the Orchestrator chat wrote to the agents, what the agents wrote, their questions (in the conversation,
//! and with suggested answers), their plans and how each was decided, how a decision went on with a run that stopped
//! (a return or a restart), Orchester's notices that the base branch moved (ORC-54) and how each step ended; oldest
//! first, newest last, a page at a time. Each entry names who wrote or decided it (for the Orchestrator chat, how it
//! went out: sent by the chat or applied by the person, ORC-45), its step and run, when, and the link to its card;
//! after the newest page's entries, the answers and messages the chat proposed that no one applied. It reads what the
//! Conversation tab reads, stored with secrets
//! already redacted; tool calls and approvals are left to their own tools.

use orchester_core::{
  question::{AskedQuestions, QuestionState},
  run::Answerer,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{base_moved, orchestrated, plans, questions, tasks::find_task, wire};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  links::{Link, TaskPage},
  tasks::{
    conversation::{
      Author, Entry as MessageEntry, EntryState,
      orchestrator::{Proposal, continued, message_offer, proposals},
      outgoing::conversation_of,
    },
    detail::{RunView, TaskDetail},
    timeline::{
      self, Item, ItemKind, Mark, RunEnd, Sources,
      load::{requests_in, transcript_in},
      mark::MarkKind,
    },
  },
};

/// The most entries a page holds, and how many when the call names none.
const MOST: usize = 200;
const PAGE: usize = 50;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetConversation {
  /// The project's id, or its name.
  pub project: String,
  /// The task's key (`KON-12`), or its id.
  pub task: String,
  /// Only entries older than this one: the `cursor` of the oldest entry of a page read before.
  #[serde(default)]
  pub before: Option<String>,
  /// At most this many entries, the newest of them: 50 when left out, 200 at most.
  #[serde(default)]
  pub limit: Option<usize>,
}

pub async fn get_conversation(services: &Services, args: GetConversation) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let (project, root) = (open.project.id(), open.repo_root);
    let task = find_task(&ProjectDb::open(&root)?, &args.task)?;
    let detail = TaskDetail::load_in(&root, task.id())?;
    let link = Link::Task(project, task.key().clone(), TaskPage::Conversation).path();
    let (items, proposed) = items(&root, &detail)?;
    let end = match &args.before {
      Some(cursor) => items
        .iter()
        .position(|item| *item.key == **cursor)
        .ok_or_else(|| ToolError::InvalidArguments(format!("before: no entry has the cursor {cursor}")))?,
      None => items.len(),
    };
    let limit = args.limit.unwrap_or(PAGE).clamp(1, MOST);
    let start = end.saturating_sub(limit);
    let run = detail.run.as_ref();
    let mut entries: Vec<Value> = items[start..end]
      .iter()
      .filter_map(|item| entry(item, run, &link))
      .collect();
    if let (None, Some(run)) = (&args.before, run) {
      let number = (run.id(), run.run.number().get());
      entries.extend(
        proposed
          .iter()
          .map(|proposal| orchestrated::proposal(proposal, number, &link)),
      );
    }
    Ok(json!({
      "task": { "key": task.key().to_string(), "title": task.details().title.as_str() },
      "run": run.map(|run| json!({ "id": run.id().to_string(), "number": run.run.number().get() })),
      "entries": entries,
      "more": start > 0,
      "link": link,
    }))
  })
  .await
}

/// The latest run's timeline as the Conversation tab builds it, and the answers and messages the Orchestrator chat
/// proposed for the run; without a run, the messages that wait for one.
fn items(root: &std::path::Path, detail: &TaskDetail) -> Result<(Vec<Item>, Vec<Proposal>), ToolError> {
  let conversation = conversation_of(detail, false);
  let Some(run) = &detail.run else {
    let waiting = conversation
      .entries
      .iter()
      .map(|entry| Item {
        key: format!("message-{}", entry.id).into(),
        execution: None,
        at: entry.at,
        kind: ItemKind::Message(entry.clone()),
      })
      .collect();
    return Ok((waiting, Vec::new()));
  };
  let transcript = transcript_in(root, run.id())?;
  let requests = requests_in(root, run.id())?;
  let offered = message_offer(detail, run.id());
  let proposed = proposals(&requests.orchestrated, (offered.as_ref(), &requests.questions));
  let mut items = timeline::build(&Sources {
    run,
    entries: &transcript,
    conversation: &conversation,
    pending: &requests.pending,
    decided: &requests.decided,
    questions: &requests.questions,
    merge: detail.merge.as_ref(),
  });
  continued(&mut items, &requests.orchestrated);
  Ok((items, proposed))
}

/// `item` as the tool gives it; `None` for what other tools give (steps, tool calls, approvals, problems).
fn entry(item: &Item, run: Option<&RunView>, link: &str) -> Option<Value> {
  let step_of = || {
    item
      .execution
      .and_then(|execution| run?.step_of(execution))
      .map(|step| step.name.to_string())
  };
  let (kind, author, step, body) = match &item.kind {
    ItemKind::AgentMessage { agent, step, text } => (
      "agent_message",
      agent_author(agent),
      Some(step.to_string()),
      json!({ "text": &**text }),
    ),
    ItemKind::Message(message) => ("message", sender(message), step_of(), message_body(message)),
    ItemKind::Question { entry, step } => (
      "agent_question",
      sender(entry),
      step.as_deref().map(str::to_owned),
      json!({ "text": &*entry.body, "open": entry.state == EntryState::Question { open: true } }),
    ),
    ItemKind::Plan(card) => (
      "plan",
      agent_author(&card.agent),
      Some(card.step.to_string()),
      json!({
        "version": card.plan.version.get(),
        "text": &card.plan.text,
        "waits_for_decision": card.waiting,
        "decision": plans::decided(&card.plan),
      }),
    ),
    ItemKind::Choice(card) => (
      "question",
      agent_author(&card.agent),
      Some(card.step.to_string()),
      choice_body(&card.asked),
    ),
    ItemKind::Result(card) => (
      "step_result",
      card.agent.as_deref().map_or(Value::Null, agent_author),
      Some(card.step.to_string()),
      json!({ "passed": card.passed, "summary": card.reported.as_deref() }),
    ),
    ItemKind::End(end) => ("run_end", Value::Null, None, json!({ "end": run_end(end) })),
    ItemKind::Mark(mark) if mark.by.is_some() || mark.orchestrator.is_some() => {
      ("run_continued", decider(mark), step_of(), continued_body(mark))
    }
    ItemKind::BaseMoved(mark) => (
      "base_moved",
      json!({ "kind": "orchester" }),
      step_of(),
      base_moved::shown(mark),
    ),
    ItemKind::Step(_) | ItemKind::Mark(_) | ItemKind::Approval(_) | ItemKind::Tools(_) | ItemKind::Problem(_) => {
      return None;
    }
  };
  let mut value = json!({
    "cursor": &*item.key,
    "kind": kind,
    "author": author,
    "step": step,
    "run": run.map(|run| json!({ "id": run.id().to_string(), "number": run.run.number().get() })),
    "at": wire::time(item.at),
    "link": link,
  });
  if let (Value::Object(value), Value::Object(body)) = (&mut value, body) {
    value.extend(body);
  }
  Some(value)
}

fn agent_author(agent: &str) -> Value {
  json!({ "kind": "agent", "name": agent })
}

/// Who wrote a message: an agent, the person, automation, or the Orchestrator chat on the person's side, with how it
/// went out and the chat's conversation it came from; and whom to.
fn sender(message: &MessageEntry) -> Value {
  let mut author = match message.orchestrator {
    Some(from) => orchestrated::author(from),
    None if message.by_automation => json!({ "kind": "automation" }),
    None => json!({ "kind": "person" }),
  };
  match &message.author {
    Author::Agent(agent) => agent_author(agent),
    Author::You => author,
    Author::YouTo(agent) => {
      author["to"] = json!(&**agent);
      author
    }
  }
}

/// Who decided how the run went on in `mark`: the Orchestrator chat, with how its decision went out (sent by it, or
/// applied by the person), else the person or automation.
fn decider(mark: &Mark) -> Value {
  match (mark.orchestrator, mark.by) {
    (Some(from), _) => orchestrated::author(from),
    (None, Some(Answerer::Automation)) => json!({ "kind": "automation" }),
    (None, _) => json!({ "kind": "person" }),
  }
}

/// How a decision went on with the run in `mark`: the work returned (with the note for its agent, or to resolve the
/// conflicts with a branch) or a step restarted, to which step, and the attempt it started.
fn continued_body(mark: &Mark) -> Value {
  let (how, to, note, resolving) = match &mark.kind {
    MarkKind::Returned { to, .. } => ("returned", to, None, None),
    MarkKind::Restarted { step } => ("restarted", step, None, None),
    MarkKind::ReturnedWithNote { to, note, resolving } => ("returned_with_note", to, Some(note), resolving.as_ref()),
  };
  json!({
    "continued": how,
    "to": &**to,
    "note": note.map(|note| &**note),
    "resolving": resolving.map(|branch| &**branch),
    "attempt": mark.attempt,
  })
}

/// A message's text and where it stands.
fn message_body(message: &MessageEntry) -> Value {
  let state = match &message.state {
    EntryState::Queued(_) => "queued",
    EntryState::Delivered { .. } => "delivered",
    EntryState::NotDelivered { .. } => "not_delivered",
    EntryState::Reply { .. } => "reply",
    EntryState::Question { .. } => "question",
  };
  json!({ "text": &*message.body, "state": state })
}

/// A question with suggested answers: its questions and options, where it stands, and who answered it; for the
/// Orchestrator chat, how its answer went out (`answered_by_orchestrator`).
fn choice_body(asked: &AskedQuestions) -> Value {
  let mut body = questions::shown(asked);
  let (state, answers, by) = match &asked.state {
    QuestionState::Waiting => ("waiting", Value::Null, Value::Null),
    QuestionState::Interrupted { .. } => ("ended_without_answer", Value::Null, Value::Null),
    QuestionState::Answered { answers, by, .. } => {
      let given: Vec<Value> = answers
        .iter()
        .map(|answer| json!({ "choices": answer.choices(), "text": answer.text() }))
        .collect();
      // The chat's answer is the Orchestrator's, whether it sent it or the person applied it.
      let by = match by {
        _ if asked.orchestrator.is_some() => "orchestrator",
        Answerer::Person => "person",
        Answerer::Automation => "automation",
      };
      ("answered", json!(given), json!(by))
    }
  };
  body["state"] = json!(state);
  body["answers"] = answers;
  body["answered_by"] = by;
  body["answered_by_orchestrator"] = json!(asked.orchestrator.filter(|_| !asked.waits()).map(orchestrated::author));
  body
}

const fn run_end(end: &RunEnd) -> &'static str {
  match end {
    RunEnd::Ready => "ready",
    RunEnd::Merged(_) => "merged",
    RunEnd::Failed(_) => "failed",
    RunEnd::Cancelled => "cancelled",
  }
}
