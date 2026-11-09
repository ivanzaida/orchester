//! The words of a team ask's or start's refusal on the task page: why a claim command or a start did not do what was
//! asked.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_api::v2::AuthorityErrorCode;
use orchester_workspace::{actions::LifecycleRefusal, team_work::ClaimEnd};

use super::TeamAsk;
use crate::{
  screens::{settings::orchester::moment, task::TaskContext},
  team::{
    act::Acted,
    start::{Started, Stop},
  },
  ui::message::message_text,
};

/// Why a claim command did not do what was asked; `None` when it did.
pub(super) fn acted_words(ctx: &Ctx, task: &TaskContext, ask: TeamAsk, acted: &Acted) -> Option<Arc<str>> {
  let key = match acted {
    Acted::Done => return None,
    // Someone renewed or took the claim first: the page shows the claim as read again, and says who.
    Acted::Refused(AuthorityErrorCode::ClaimConflict) if ask == TeamAsk::TakeOver => {
      let claim = task.team.as_ref()?.task.claim.as_ref()?;
      let time = moment(ctx, claim.renewed);
      return Some(ctx.t_args(
        "summary.team.refused.renewed",
        [("holder", claim.holder.name.as_str()), ("time", time.as_str())],
      ));
    }
    Acted::Refused(code) => return Some(refused_words(ctx, *code)),
    // Only a task edit names a pipeline; any other action that met one was refused as invalid.
    Acted::PipelineRefused(_) => return Some(refused_words(ctx, AuthorityErrorCode::Invalid)),
    Acted::NeedsVersion(required) => {
      return Some(ctx.t_args("summary.team.refused.needsVersion", [("version", required.as_str())]));
    }
    Acted::Lifecycle(refusal) => match refusal {
      LifecycleRefusal::NotHolder => "summary.team.refused.notHolder",
      LifecycleRefusal::NotAdmin => "summary.team.refused.notAdmin",
      LifecycleRefusal::StopRunFirst => "summary.team.refused.stopRunFirst",
      LifecycleRefusal::MissingReason => "summary.team.refused.missingReason",
    },
    Acted::Changed(_) => "summary.team.refused.changed",
    Acted::Unavailable => "summary.team.refused.unavailable",
    Acted::SignedOut => "summary.team.refused.signedOut",
    Acted::Failed => "summary.team.refused.failed",
  };
  Some(ctx.t(key))
}

/// Why a start did not start the run; `None` when it did.
pub(super) fn started_words(ctx: &Ctx, task: &TaskContext, ask: TeamAsk, started: &Started) -> Option<Arc<str>> {
  let stop = match started {
    Started::Run(_) => return None,
    Started::Waiting => return Some(ctx.t("summary.team.refused.waiting")),
    Started::Stopped(stop) => stop,
  };
  Some(match stop {
    Stop::Claim(acted) => return acted_words(ctx, task, ask, acted),
    Stop::NotReady => ctx.t("summary.team.refused.notReady"),
    // A start refused as invalid is the server's own reason, not an agent at work.
    Stop::Refused(AuthorityErrorCode::Invalid, _) => ctx.t("summary.team.refused.other"),
    Stop::Refused(code, claim) => {
      let why = refused_words(ctx, *code);
      claim.map_or(why.clone(), |claim| with_claim(ctx, &why, claim))
    }
    Stop::BranchNotFound(branch) => ctx.t_args("summary.team.refused.branch", [("branch", branch.as_str())]),
    // The Summary's card lists each part with its fix (`not_ready`), not a notice.
    Stop::NotReadyHere(_) => return None,
    Stop::NotReadable => ctx.t("summary.team.refused.notReadable"),
    Stop::BundleMissing(message, claim) => match claim {
      Some(claim) => with_claim(ctx, &message_text(ctx, message), *claim),
      None => message_text(ctx, message),
    },
    Stop::StartUnreadable => ctx.t("summary.team.refused.startUnreadable"),
    Stop::BundleSchema { read, built, claim } => {
      let key = if built > read {
        "summary.team.refused.bundleSchemaNewer"
      } else {
        "summary.team.refused.bundleSchemaOlder"
      };
      let why = ctx.t_args(key, [("read", read.to_string()), ("built", built.to_string())]);
      with_claim(ctx, &why, *claim)
    }
    Stop::BundleUnreadable(claim) => with_claim(ctx, &ctx.t("summary.team.refused.bundleUnreadable"), *claim),
    Stop::BundleMismatch(claim) => with_claim(ctx, &ctx.t("summary.team.refused.bundleMismatch"), *claim),
    Stop::InputsFailed(claim) => with_claim(ctx, &ctx.t("summary.team.refused.inputsFailed"), *claim),
    Stop::Runner(messages) => messages.first().map_or_else(
      || ctx.t("summary.team.refused.failed"),
      |message| message_text(ctx, message),
    ),
    Stop::Failed => ctx.t("summary.team.refused.failed"),
  })
}

/// `why` a start failed for good, then what became of the task's claim.
fn with_claim(ctx: &Ctx, why: &str, claim: ClaimEnd) -> Arc<str> {
  let key = match claim {
    ClaimEnd::Released => "summary.team.refused.claimReleased",
    ClaimEnd::Kept => "summary.team.refused.claimKept",
    ClaimEnd::Held => "summary.team.refused.claimHeld",
  };
  format!("{why} {}", ctx.t(key)).into()
}

/// A refusal's code as the page says it.
fn refused_words(ctx: &Ctx, code: AuthorityErrorCode) -> Arc<str> {
  let key = match code {
    AuthorityErrorCode::ClaimConflict => "summary.team.refused.conflict",
    AuthorityErrorCode::ModuleClaimConflict => "summary.team.refused.moduleConflict",
    AuthorityErrorCode::ClaimLost | AuthorityErrorCode::SessionMismatch => "summary.team.refused.lost",
    AuthorityErrorCode::Invalid => "summary.team.refused.busy",
    AuthorityErrorCode::TransitionDenied => "summary.team.refused.denied",
    AuthorityErrorCode::DecisionRequired => "summary.team.refused.decision",
    AuthorityErrorCode::Forbidden => "summary.team.refused.forbidden",
    AuthorityErrorCode::MembershipRevoked => "summary.team.refused.removed",
    AuthorityErrorCode::ModuleRequired => "summary.team.refused.notReady",
    AuthorityErrorCode::Unavailable | AuthorityErrorCode::RateLimited | AuthorityErrorCode::Unauthenticated => {
      "summary.team.refused.unavailable"
    }
    _ => "summary.team.refused.other",
  };
  ctx.t(key)
}
