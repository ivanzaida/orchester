//! `get_conversation`: a task's conversation as its Conversation tab shows it for its latest run (ORC-42): what the
//! person and the Orchestrator chat wrote to the agents, what the agents wrote, their questions (in the conversation,
//! and with suggested answers), their plans and how each was decided, and how each step ended; oldest first, newest
//! last, a page at a time. Each entry names who wrote it, its step and run, when, and the link to its card. It reads
//! what the Conversation tab reads, stored with secrets already redacted; tool calls and approvals are left to their
//! own tools.

use orchester_core::{
  question::{AskedQuestions, QuestionState},
  run::Answerer,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{plans, questions, tasks::find_task, wire};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  links::{Link, TaskPage},
  tasks::{
    conversation::{Author, Entry as MessageEntry, EntryState, outgoing::conversation_of},
    detail::{RunView, TaskDetail},
    timeline::{
      self, Item, ItemKind, RunEnd, Sources,
      load::{requests_in, transcript_in},
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
    let items = items(&root, &detail)?;
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
    let entries: Vec<Value> = items[start..end]
      .iter()
      .filter_map(|item| entry(item, run, &link))
      .collect();
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

/// The latest run's timeline as the Conversation tab builds it; without a run, the messages that wait for one.
fn items(root: &std::path::Path, detail: &TaskDetail) -> Result<Vec<Item>, ToolError> {
  let conversation = conversation_of(detail, false);
  let Some(run) = &detail.run else {
    return Ok(
      conversation
        .entries
        .iter()
        .map(|entry| Item {
          key: format!("message-{}", entry.id).into(),
          execution: None,
          at: entry.at,
          kind: ItemKind::Message(entry.clone()),
        })
        .collect(),
    );
  };
  let transcript = transcript_in(root, run.id())?;
  let requests = requests_in(root, run.id())?;
  Ok(timeline::build(&Sources {
    run,
    entries: &transcript,
    conversation: &conversation,
    pending: &requests.pending,
    decided: &requests.decided,
    questions: &requests.questions,
    merge: detail.merge.as_ref(),
  }))
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

/// Who wrote a message: an agent, the person, or the Orchestrator chat on the person's side; and whom to.
fn sender(message: &MessageEntry) -> Value {
  let on_person_side = if message.by_orchestrator { "orchestrator" } else { "person" };
  match &message.author {
    Author::Agent(agent) => agent_author(agent),
    Author::You => json!({ "kind": on_person_side }),
    Author::YouTo(agent) => json!({ "kind": on_person_side, "to": &**agent }),
  }
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

/// A question with suggested answers: its questions and options, where it stands, and who answered it.
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
      let by = match by {
        Answerer::Person => "person",
        Answerer::Automation if asked.by_orchestrator => "orchestrator",
        Answerer::Automation => "automation",
      };
      ("answered", json!(given), json!(by))
    }
  };
  body["state"] = json!(state);
  body["answers"] = answers;
  body["answered_by"] = by;
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
