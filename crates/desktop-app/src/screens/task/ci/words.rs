//! The words the CI parts share: the badge of how the checks stand, the CI line's count, why the checks were not read,
//! and the agent a Fix CI goes to.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::pull_request::ci::{CiChecks, CiState, fix::is_failing};
use orchester_workspace::{
  pull_requests::{ci::PullRequestCi, ci_fix::writing_step},
  tasks::{
    conversation::outgoing::{conversation_of, offer},
    detail::TaskDetail,
  },
};

use super::Look;
use crate::ui::chip::BadgeTone;

/// The badge's tone and word.
pub fn badge(look: Look) -> (BadgeTone, &'static str) {
  match look {
    Look::Running => (BadgeTone::Info, "ci.state.running"),
    Look::Passed => (BadgeTone::Success, "ci.state.passed"),
    Look::Failed => (BadgeTone::Danger, "ci.state.failed"),
    Look::Fixing => (BadgeTone::Warning, "ci.state.fixing"),
    Look::NoChecks | Look::Unknown => (BadgeTone::Neutral, "ci.state.unknown"),
  }
}

/// *CI · 2 of 5 checks failed*, or what the checks are when they were not read.
pub fn count(ctx: &Ctx, ci: &PullRequestCi, look: Look) -> Arc<str> {
  let PullRequestCi::Read(read) = ci else {
    return ctx.t("ci.summary.notRead");
  };
  let checks = &read.checks;
  let total = checks.checks.len().to_string();
  match look {
    Look::NoChecks => ctx.t("ci.summary.none"),
    Look::Running => {
      let done = checks.checks.iter().filter(|check| !check.state.is_pending()).count();
      ctx.t_args("ci.summary.running", [("done", done.to_string()), ("total", total)])
    }
    Look::Passed => ctx.t_args(
      "ci.summary.passed",
      [("passed", passed(checks).to_string()), ("total", total)],
    ),
    Look::Failed | Look::Fixing | Look::Unknown => ctx.t_args(
      "ci.summary.failed",
      [("failed", failing(checks).len().to_string()), ("total", total)],
    ),
  }
}

/// The checks that passed or were skipped.
pub fn passed(checks: &CiChecks) -> usize {
  checks
    .checks
    .iter()
    .filter(|check| matches!(check.state, CiState::Passed | CiState::Skipped))
    .count()
}

/// The failing checks' names, in `gh`'s order.
pub fn failing(checks: &CiChecks) -> Vec<&str> {
  checks
    .checks
    .iter()
    .filter(|check| is_failing(check.state))
    .map(|check| check.name.as_str())
    .collect()
}

/// Why the checks were not read, as a callout's title and body: `(title, body, warning)`; a warning (no `gh`, no
/// sign-in) shows the command that fixes it.
pub fn not_read(ci: &PullRequestCi) -> (&'static str, &'static str, bool) {
  match ci {
    PullRequestCi::GhMissing => ("ci.gh.notFoundTitle", "ci.gh.notFound", true),
    PullRequestCi::NotSignedIn => ("ci.gh.notSignedInTitle", "ci.gh.notSignedIn", true),
    PullRequestCi::RateLimited { .. } => ("ci.gh.readFailedTitle", "ci.gh.rateLimited", false),
    PullRequestCi::NotRead(orchester_core::repository::RepositoryError::Network { .. }) => {
      ("ci.gh.readFailedTitle", "ci.gh.unreachable", false)
    }
    PullRequestCi::Unavailable(_) => ("ci.gh.readFailedTitle", "ci.gh.unavailable", false),
    PullRequestCi::NotRead(_) | PullRequestCi::NotLinked | PullRequestCi::NoPullRequest | PullRequestCi::Read(_) => {
      ("ci.gh.readFailedTitle", "ci.gh.readFailed", false)
    }
  }
}

/// Why the checks were not read, in a few words for an event line.
pub fn why_not_read(ci: &PullRequestCi) -> &'static str {
  match ci {
    PullRequestCi::GhMissing => "ci.event.whyNotFound",
    PullRequestCi::NotSignedIn => "ci.event.whyNotSignedIn",
    _ => "ci.event.whyFailed",
  }
}

/// The agent a Fix CI goes to: the one of the step that wrote the code (`ci_fix::writing_step`), else the one the run's
/// state addresses, as the composer would send to it.
pub fn agent(ctx: &Ctx, detail: &TaskDetail, lost: bool) -> Arc<str> {
  if let Some(agent) = detail
    .run
    .as_ref()
    .and_then(writing_step)
    .and_then(|step| step.agent.clone())
  {
    return agent;
  }
  let conversation = conversation_of(detail, lost);
  let (_, recipient) = offer(&conversation, detail.run.as_ref(), None);
  conversation
    .recipients
    .iter()
    .find(|choice| choice.recipient == recipient)
    .and_then(|choice| choice.agent.clone())
    .unwrap_or_else(|| ctx.t("ci.theAgent"))
}
