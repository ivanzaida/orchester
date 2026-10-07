//! Auto mode's reviewer on a native connection: one model request through the connection's own client
//! (`orchester-agent`'s Anthropic Messages client), built here so every native reviewer asks the same
//! way. No tools are offered, the instructions are the system prompt, the request and its context are
//! the one user message, and the answer must be the verdict's JSON object alone: anything else, a
//! response cut short or a refusal is a review without a verdict, which asks the person. What it spent is
//! the response's usage with what the client reported of the responses it dropped on the way ([`NativeSpend`]).

use crate::{
  id::ConversationId,
  native::{
    item::{Item, UserOrigin},
    model::{ModelRequest, ModelResponse, StopReason, StreamEvent},
    tools::ToolSet,
  },
  resource::ModelId,
  run::{
    TokenUsage,
    review::{
      REVIEW_INSTRUCTIONS, ReviewError, ReviewInput, ReviewSpend, ReviewVerdict, Reviewed, parse_verdict_text,
      review_prompt,
    },
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

/// What a native review spent, followed through its client's stream events: the responses the client
/// dropped (a broken stream, one asked again) report what they spent as [`StreamEvent::Spent`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeSpend {
  dropped: TokenUsage,
}

impl NativeSpend {
  pub fn observe(&mut self, event: &StreamEvent) {
    if let StreamEvent::Spent(usage) = event {
      self.dropped += *usage;
    }
  }

  /// The model's answer as a review: its verdict ([`native_verdict`]), and its usage with what the dropped
  /// responses spent.
  pub fn answered(self, response: &ModelResponse) -> Reviewed {
    let mut spent = response.usage;
    spent += self.dropped;
    Reviewed::new(native_verdict(response), ReviewSpend::Reported(spent))
  }

  /// A review the client gave up on with `error`: what its dropped responses spent, nothing when there were
  /// none. That holds for a client that reports what every broken stream spent, as `orchester-agent`'s
  /// Anthropic client does (the stream's first event carries the usage), so a request the API refused or
  /// never received spent nothing. Its Responses client reports no usage for a broken stream: a reviewer on
  /// it would need [`ReviewSpend::Unknown`] instead.
  pub fn unanswered(self, error: ReviewError) -> Reviewed {
    let spend = if self.dropped == TokenUsage::default() {
      ReviewSpend::Nothing
    } else {
      ReviewSpend::Reported(self.dropped)
    };
    Reviewed::new(Err(error), spend)
  }
}
