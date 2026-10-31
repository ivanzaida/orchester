//! What auto mode's reviewer spends (decided 2026-10-07): every review that asked a model is recorded
//! with the request it judged ([`ReviewRecord`]): the provider, account and model, the tokens its provider
//! reported, when it ran and how it ended. Its tokens count like an agent's turn: toward the run's and
//! the step's budgets, the project's daily budget and the Usage dashboard. What a review spent but did
//! not report readably is unknown, never zero: it is recorded as unknown and counts nowhere.

use super::{Review, ReviewError, ReviewFailure, ReviewVerdict};
use crate::{
  connection::{AgentProvider, plan_limits::PlanLimits},
  id::{AgentAccountId, InvocationId, RunId, StepExecutionId},
  resource::ModelId,
  run::TokenUsage,
  time::Timestamp,
  tokens::TokensUsed,
  usage::UsedProvider,
};

/// What a reviewer spent on one review.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewSpend {
  /// No model was asked: the reviewer cannot be reached, or its program could not be started.
  Nothing,
  /// A model was asked, but what it spent could not be read.
  Unknown,
  /// What the provider reported.
  Reported(TokenUsage),
}

/// A reviewer's answer about one request, with what it spent and the account's plan limits its CLI
/// reported with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reviewed {
  pub verdict: Result<ReviewVerdict, ReviewError>,
  pub spend: ReviewSpend,
  pub limits: Vec<PlanLimits>,
}

impl Reviewed {
  /// `verdict` at `spend`, without plan limits.
  pub const fn new(verdict: Result<ReviewVerdict, ReviewError>, spend: ReviewSpend) -> Self {
    Self {
      verdict,
      spend,
      limits: Vec::new(),
    }
  }
}

/// What a review that asked a model cost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewCost {
  pub model: ModelId,
  /// `None` when what it spent could not be read.
  pub usage: Option<TokenUsage>,
  pub limits: Vec<PlanLimits>,
}

impl ReviewCost {
  /// What `reviewed` by `model` cost; `None` when it asked no model.
  pub fn of(reviewed: &Reviewed, model: &ModelId) -> Option<Self> {
    let usage = match reviewed.spend {
      ReviewSpend::Nothing => return None,
      ReviewSpend::Unknown => None,
      ReviewSpend::Reported(usage) => Some(usage),
    };
    Some(Self {
      model: model.clone(),
      usage,
      limits: reviewed.limits.clone(),
    })
  }
}

/// Auto mode's handling of one request and, when it asked a model, what that cost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoReview {
  pub review: Review,
  pub cost: Option<ReviewCost>,
}

/// How a review that asked a model ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewEnd {
  Approved,
  Asked,
  Failed(ReviewFailure),
}

impl ReviewEnd {
  /// How `review` ended; `None` for a request a hard rule kept from the reviewer.
  pub fn of(review: &Review) -> Option<Self> {
    match review {
      Review::Approved { .. } => Some(Self::Approved),
      Review::Asked { .. } => Some(Self::Asked),
      Review::Decided { decision } if decision.approves() => Some(Self::Approved),
      Review::Decided { .. } => Some(Self::Asked),
      Review::Failed { failure, .. } => Some(Self::Failed(*failure)),
      Review::Held { .. } => None,
    }
  }
}

/// One review that asked a model, linked to the request it judged and the invocation that asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewRecord {
  pub run: RunId,
  pub execution: StepExecutionId,
  pub invocation: InvocationId,
  /// The request's id.
  pub approval: String,
  pub provider: AgentProvider,
  /// The CLI account the reviewer's connection ran on; `None` for a native reviewer, a provider without
  /// accounts, or a review recorded before the account was kept.
  pub account: Option<AgentAccountId>,
  /// The reviewer's connection; `None` for a review recorded before it was kept.
  pub connection: Option<UsedProvider>,
  pub model: ModelId,
  pub started_at: Timestamp,
  pub ended_at: Timestamp,
  /// `None`: what it spent could not be read.
  pub usage: Option<TokenUsage>,
  pub end: ReviewEnd,
}

impl ReviewRecord {
  /// What counts toward budgets: nothing while its usage is unknown.
  pub fn counted_tokens(&self) -> TokensUsed {
    self.usage.map_or(TokensUsed::ZERO, TokenUsage::counted_tokens)
  }
}

/// The usage `reviews` reported, summed; reviews whose usage is unknown add nothing.
pub fn reviews_usage<'a>(reviews: impl IntoIterator<Item = &'a ReviewRecord>) -> TokenUsage {
  reviews.into_iter().filter_map(|review| review.usage).sum()
}
