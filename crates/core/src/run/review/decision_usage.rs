//! Usage of a decision review, kept distinct from agent providers and CLI accounts.
use super::ReviewEnd;
use crate::{
  id::{DecisionConnectionId, InvocationId, RunId, StepExecutionId},
  resource::ModelId,
  time::Timestamp,
};

/// One decision review linked to the request it judged. Unknown input usage is never zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionReviewRecord {
  pub run: RunId,
  pub execution: StepExecutionId,
  pub invocation: InvocationId,
  pub approval: String,
  pub connection: DecisionConnectionId,
  pub provider: String,
  /// The server's resolved model/version, or the chosen model when no readable response arrived.
  pub model: ModelId,
  pub started_at: Timestamp,
  pub ended_at: Timestamp,
  pub input_tokens: Option<u64>,
  pub end: ReviewEnd,
}
