//! A run's waiting questions with suggested answers, as `get_run` lists them, and `answer_question`, which
//! answers one as the card in the task conversation does: through the runner, which gives the answer to
//! the asking agent. The answer is checked first, with the card's own rules: only the question's options,
//! one answer to a single-choice question, never an empty one.

use orchester_core::{
  question::{AnswerError, AskedQuestions, GivenAnswer, Question, QuestionTurn},
  run::Answerer,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{runs::find_run, wire};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::Message,
  tasks::{
    question::{Answering, answer_error},
    run::control_message,
  },
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct AnswerQuestion {
  /// The project's id, or its name.
  pub project: String,
  /// The run's id, or its number (`7` or `#7`).
  pub run: String,
  /// The question's id, as `get_run` lists it; left out, the run's only waiting question.
  #[serde(default)]
  pub question: Option<String>,
  /// The labels of the options picked, as `get_run` lists them.
  #[serde(default)]
  pub choices: Vec<String>,
  /// An answer in your own words, besides the options picked or instead of them.
  #[serde(default)]
  pub text: Option<String>,
}

/// The run's questions that wait for an answer, each with its options.
pub fn waiting(questions: &[AskedQuestions]) -> Vec<Value> {
  questions.iter().filter(|asked| asked.waits()).map(shown).collect()
}

fn shown(asked: &AskedQuestions) -> Value {
  let request = &asked.request;
  let questions: Vec<Value> = request.questions().iter().map(question).collect();
  json!({
    "id": request.id(),
    "agent": request.agent(),
    "asked_at": wire::time(request.asked_at()),
    // In its turn, the agent waits for the answer; otherwise its step ended with the question.
    "agent_waits_in_turn": request.turn() == QuestionTurn::InTurn,
    "questions": questions,
  })
}

fn question(question: &Question) -> Value {
  let options: Vec<Value> = question
    .options()
    .iter()
    .map(|option| json!({ "label": option.label(), "description": option.description() }))
    .collect();
  json!({
    "text": question.text(),
    "header": question.header(),
    "multi_select": question.multi_select(),
    "options": options,
    "own_answer": true,
  })
}

pub async fn answer_question(services: &Services, args: AnswerQuestion) -> Result<Value, ToolError> {
  let reading = services.clone();
  let AnswerQuestion {
    project,
    run,
    question,
    choices,
    text,
  } = args;
  let (project, asked) = blocking(move || {
    let project = reading.project(&project)?;
    let db = ProjectDb::open(&project.repo_root)?;
    let run = find_run(&db, &run)?;
    let asked = waiting_request(&db.questions(run.id())?, question.as_deref())?;
    Ok((project, asked))
  })
  .await?;
  let given = GivenAnswer { choices, text };
  let [one] = asked.request.questions() else {
    return Err(ToolError::refused(
      "several_questions",
      vec![Message::new("appMcp.errors.severalQuestions")],
    ));
  };
  one
    .answer(&given.choices, given.text.as_deref())
    .map_err(|error| refused(&error))?;
  let answering = Answering {
    run: asked.request.run(),
    request: asked.request.id().to_owned(),
    answers: vec![given],
  };
  services
    .runners
    .control_run_in(&project, answering.command(Answerer::Automation))
    .await
    .map_err(|error| ToolError::runner(&error, vec![control_message(&error)]))?;
  Ok(json!({ "question": { "id": asked.request.id(), "answered": true } }))
}

/// The waiting request named `id`, or the run's only one when none is named.
fn waiting_request(questions: &[AskedQuestions], id: Option<&str>) -> Result<AskedQuestions, ToolError> {
  let waiting: Vec<&AskedQuestions> = questions.iter().filter(|asked| asked.waits()).collect();
  match (id, waiting.as_slice()) {
    (Some(id), _) => waiting
      .iter()
      .find(|asked| asked.request.id() == id)
      .map(|asked| (*asked).clone())
      .ok_or_else(|| ToolError::not_found(Missing::Question, id)),
    (None, [only]) => Ok((*only).clone()),
    (None, []) => Err(ToolError::refused(
      "no_waiting_question",
      vec![Message::new("appMcp.errors.noWaitingQuestion")],
    )),
    (None, _) => Err(ToolError::refused(
      "question_ambiguous",
      vec![Message::new("appMcp.errors.questionAmbiguous")],
    )),
  }
}

/// An answer the question does not take, with the card's words for why.
fn refused(error: &AnswerError) -> ToolError {
  let code = match error {
    AnswerError::Empty => "empty_answer",
    AnswerError::UnknownChoice { .. } => "unknown_choice",
    AnswerError::RepeatedChoice { .. } => "repeated_choice",
    AnswerError::SeveralForSingle => "several_choices",
    AnswerError::AnswerCount { .. } => "answer_count",
  };
  ToolError::refused(code, vec![answer_error(error)])
}
