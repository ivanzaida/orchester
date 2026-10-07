//! Auto mode's reviewer on a native connection: one model request through the connection's own client
//! (`orchester-agent`'s Anthropic Messages client), built here so every native reviewer asks the same
//! way. No tools are offered, the instructions are the system prompt, the request and its context are
//! the one user message, and the answer must be the verdict's JSON object alone: anything else, a
//! response cut short or a refusal is a review without a verdict, which asks the person. What it spent is
//! the response's usage, as the client read it from either wire API (Anthropic Messages, OpenAI Responses).

use crate::{
  id::ConversationId,
  native::{
    item::{Item, UserOrigin},
    model::{ModelRequest, ModelResponse, StopReason},
    tools::ToolSet,
  },
  resource::ModelId,
  run::review::{
    REVIEW_INSTRUCTIONS, ReviewError, ReviewInput, ReviewSpend, ReviewVerdict, Reviewed, parse_verdict_text,
    review_prompt,
  },
};

/// The most a reviewer may write: one short JSON object, with room for the model's reasoning.
pub const NATIVE_MAX_OUTPUT: u32 = 4_096;

/// The one request that reviews `input` with `model` and `effort`.
pub fn native_request(input: &ReviewInput, model: &ModelId, effort: Option<String>) -> ModelRequest {
  ModelRequest {
    model: model.clone(),
    effort,
    max_output: NATIVE_MAX_OUTPUT,
    system: REVIEW_INSTRUCTIONS.to_owned(),
    tools: ToolSet::none(),
    items: vec![Item::User {
      text: review_prompt(input),
      origin: UserOrigin::Task,
    }],
    cache_key: ConversationId::generate(),
  }
}

/// The verdict in the model's answer: its text, when it ended its turn without calling anything.
pub fn native_verdict(response: &ModelResponse) -> Result<ReviewVerdict, ReviewError> {
  if response.stop != StopReason::EndTurn {
    return Err(ReviewError::Unparseable(format!(
      "the model stopped with {:?}",
      response.stop
    )));
  }
  let text: String = response
    .items
    .iter()
    .filter_map(|item| match item {
      Item::Assistant { text } => Some(text.as_str()),
      _ => None,
    })
    .collect();
  parse_verdict_text(&text)
}

/// The model's answer as a review: its verdict ([`native_verdict`]) and the usage the response reported.
pub fn native_review(response: &ModelResponse) -> Reviewed {
  Reviewed::new(native_verdict(response), ReviewSpend::Reported(response.usage))
}
