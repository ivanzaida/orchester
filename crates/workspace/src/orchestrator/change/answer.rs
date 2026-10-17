//! An answer to an agent's question with suggested answers, or a message to a run's agents, that the Orchestrator's
//! agent asks for (`docs/design/orchestrator-toolset.md`, "Decisions" 5): checked with the rules the tool sends it
//! by, before it is proposed or sent. An answer names the waiting question it answers, so it is applied to that one
//! only; a message is refused where no agent reads it, and where it would decide the plan a run waits on.

use std::path::Path;

use orchester_core::{
  conversation::MessageBody,
  orchestrator::change::{Change, ProposedChange},
  question::GivenAnswer,
};
use orchester_store::ProjectDb;
use rmcp::model::JsonObject;
use serde_json::{Value, json};

use super::run_subject;
use crate::{
  app_mcp::{ToolError, check_answer, find_run, outgoing_refusal, plan_person_only, waiting_request},
  messages::Message,
  tasks::conversation::{
    composer::ComposerMode,
    outgoing::{OutgoingError, prepare_in},
  },
};

/// What `answer_question` asks, as its arguments name it.
pub(in crate::orchestrator) struct Asked {
  run: String,
  question: Option<String>,
  given: GivenAnswer,
}

impl Asked {
  pub(in crate::orchestrator) fn of(arguments: &JsonObject) -> Result<Self, ToolError> {
    let choices = match arguments.get("choices") {
      None | Some(Value::Null) => Vec::new(),
      Some(Value::Array(labels)) => labels
        .iter()
        .map(|label| label.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| ToolError::InvalidArguments("choices: labels are text".to_owned()))?,
      Some(_) => return Err(ToolError::InvalidArguments("choices: a list of labels".to_owned())),
    };
    Ok(Self {
      run: required(arguments, "run")?,
      question: text(arguments, "question"),
      given: GivenAnswer {
        choices,
        text: text(arguments, "text"),
      },
    })
  }
}

/// What `send_message` asks, as its arguments name it.
pub(in crate::orchestrator) struct Sending {
  run: String,
  text: String,
  now: bool,
}

impl Sending {
  pub(in crate::orchestrator) fn of(arguments: &JsonObject) -> Result<Self, ToolError> {
    Ok(Self {
      run: required(arguments, "run")?,
      text: required(arguments, "text")?,
      now: text(arguments, "timing").as_deref() == Some("now"),
    })
  }
}

fn text(arguments: &JsonObject, argument: &str) -> Option<String> {
  arguments.get(argument).and_then(Value::as_str).map(str::to_owned)
}

fn required(arguments: &JsonObject, argument: &str) -> Result<String, ToolError> {
  text(arguments, argument).ok_or_else(|| ToolError::InvalidArguments(format!("{argument}: required")))
}

/// The answer `asked` gives the waiting question it names (or the run's only one) in the project at `root`, and the
/// key of its task; refused as `answer_question` refuses it. Blocks on SQLite.
pub(super) fn answer_change(root: &Path, asked: Asked) -> Result<(ProposedChange, Option<String>), ToolError> {
  let db = ProjectDb::open(root)?;
  let run = find_run(&db, &asked.run)?;
  let waiting = waiting_request(&db.questions(run.id())?, asked.question.as_deref())?;
  check_answer(&waiting, &asked.given)?;
  let (subject, task) = run_subject(&db, &run)?;
  let change = Change::Answer {
    run: run.id(),
    number: run.number().get(),
    request: waiting.request.id().to_owned(),
    choices: asked.given.choices,
    text: asked.given.text,
  };
  Ok((ProposedChange { subject, change }, task))
}

/// The message `sending` sends the agents of the run it names in the project at `root`, the task's latest, and the key
/// of its task; refused where `send_message` would refuse it, and where it would decide the plan the run waits on.
/// Blocks on SQLite.
pub(super) fn message_change(root: &Path, sending: Sending) -> Result<(ProposedChange, Option<String>), ToolError> {
  let Sending { run: named, text, now } = sending;
  let text = MessageBody::new(&text).map_err(|_| {
    ToolError::refused(
      "blank_message",
      vec![Message::new("conversation.errors.blank")],
    )
  })?;
  let db = ProjectDb::open(root)?;
  let run = find_run(&db, &named)?;
  let prepared = prepare_in(root, (run.task(), Some(run.id())), (false, None)).map_err(outgoing_refusal)?;
  match prepared.mode {
    ComposerMode::PlanChanges { .. } => return Err(plan_person_only()),
    ref mode if mode.then().is_none() => {
      return Err(outgoing_refusal(OutgoingError::Closed(prepared.conversation.state)));
    }
    _ => {}
  }
  let (subject, task) = run_subject(&db, &run)?;
  let change = Change::Message {
    run: run.id(),
    number: run.number().get(),
    text: text.into(),
    now,
  };
  Ok((ProposedChange { subject, change }, task))
}

/// Adds the words of an answer or a message to the tool's `arguments`; nothing for another change.
pub(super) fn words(change: &Change, arguments: &mut JsonObject) {
  match change {
    Change::Answer {
      request, choices, text, ..
    } => {
      arguments.insert("question".to_owned(), json!(request));
      arguments.insert("choices".to_owned(), json!(choices));
      if let Some(text) = text {
        arguments.insert("text".to_owned(), json!(text));
      }
    }
    Change::Message { text, now, .. } => {
      arguments.insert("text".to_owned(), json!(text));
      let timing = if *now { "now" } else { "after_turn" };
      arguments.insert("timing".to_owned(), json!(timing));
    }
    Change::Publish { .. } | Change::Cancel { .. } | Change::Merge { .. } => {}
  }
}
