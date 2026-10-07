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
    model::{ModelError, ModelRequest, ModelResponse, StopReason, StreamEvent},
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

/// What a native review spent, followed through its client's stream events. A complete response the client
/// dropped (cut at a tool call and asked again) reports what it spent as [`StreamEvent::Spent`]. An attempt
/// that failed leaves the review's spend unknown when the API may have billed it: its stream broke after it
/// began (the usage it reported misses the rest of its output), or the request was lost on the way (a
/// transport error, which does not tell a connection that never opened from a request sent and then lost, or an
/// answer in an unexpected form). A request refused with an HTTP status, or an error event that opened its
/// stream, spent nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeSpend {
  dropped: TokenUsage,
  /// The attempt under way reported what it had spent.
  attempt_spent: bool,
  /// An earlier attempt may have been billed for more than was reported.
  uncertain: bool,
}

impl NativeSpend {
  pub fn observe(&mut self, event: &StreamEvent) {
    match event {
      StreamEvent::Spent(usage) => {
        self.dropped += *usage;
        self.attempt_spent = true;
      }
      StreamEvent::Retrying { error, .. } => {
        self.uncertain |= self.attempt_spent || may_have_billed(error);
        self.attempt_spent = false;
      }
      _ => {}
    }
  }

  /// The model's answer as a review: its verdict ([`native_verdict`]), and its usage with what the dropped
  /// responses spent; unknown when an earlier attempt may have been billed for more.
  pub fn answered(self, response: &ModelResponse) -> Reviewed {
    let mut spent = response.usage;
    spent += self.dropped;
    let spend = if self.uncertain {
      ReviewSpend::Unknown
    } else {
      ReviewSpend::Reported(spent)
    };
    Reviewed::new(native_verdict(response), spend)
  }

  /// A review the client gave up on with `error`: what its complete dropped responses spent, nothing when
  /// there were none, and unknown when any attempt, the last one included, may have been billed.
  pub fn unanswered(self, error: &ModelError) -> Reviewed {
    let spend = if self.uncertain || self.attempt_spent || may_have_billed(error) {
      ReviewSpend::Unknown
    } else if self.dropped == TokenUsage::default() {
      ReviewSpend::Nothing
    } else {
      ReviewSpend::Reported(self.dropped)
    };
    Reviewed::new(Err(ReviewError::CouldNotRun(error.to_string())), spend)
  }
}

/// Whether a request that failed with `error` may have reached the model: an answer cut off or in an
/// unexpected form, not a refusal with an HTTP status.
const fn may_have_billed(error: &ModelError) -> bool {
  matches!(error, ModelError::Transport(_) | ModelError::Protocol(_))
}
