//! MCP tool calls in the transcript: how a call was decided, worded as the decision marks of the command
//! categories design (*not asked · exempt tool of pencil*, *approved by you · pencil*), on Claude Code's
//! command row or on the row of a Codex call, which no command shows; and a listed tool that asks after
//! all because Claude Code could name it as another server's tool.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{connection::stream::CommandStatus, permission::category::CommandCategory, run::DecidedBy};

use super::entry::Shown;
use crate::{
  tasks::transcript::{CommandMark, DecidedMark, McpToolEntry},
  ui::icon::Icon,
};

/// The row of a Codex MCP call: the tool, and how it was decided or which server it belongs to.
pub(super) fn call(ctx: &Ctx, call: &McpToolEntry) -> Shown {
  let meta = match &call.mark {
    Some(mark) => mark_text(ctx, mark),
    None => ctx.t_args("runs.activity.mcpCall", [("server", &*call.server)]),
  };
  let (glyph, meta) = match call.status {
    Some(CommandStatus::Failed | CommandStatus::Declined) => (
      Icon::AlertCircle,
      with_mark_text(ctx, &ctx.t("runs.activity.mcpCallFailed"), &meta),
    ),
    _ => (Icon::Plug, meta),
  };
  let told = denial_told(ctx, call.mark.as_ref());
  Shown::plain(glyph, call.tool.clone(), meta, told)
}

/// What a person's Deny told the agent, under the row of the call it denied.
pub(super) fn denial_told(ctx: &Ctx, mark: Option<&CommandMark>) -> Option<Arc<str>> {
  let CommandMark::Decided(mark) = mark? else {
    return None;
  };
  let told = mark.decision.told.as_deref()?;
  Some(ctx.t_args("runs.activity.decision.told", [("message", told)]))
}

/// A listed tool that got no Claude Code rule: `(server, tool, other)`.
pub(super) fn exemption_not_applied(ctx: &Ctx, (server, tool, other): (&str, &str, &str)) -> Shown {
  let names = [("server", server), ("tool", tool), ("other", other)];
  Shown::plain(
    Icon::TriangleAlert,
    ctx.t_args("runs.activity.decision.exemptionNotApplied", names),
    "".into(),
    Some(ctx.t_args("runs.activity.decision.exemptionNotAppliedDetail", names)),
  )
}

/// A command row's `meta` followed by `mark`.
pub(super) fn with_mark(ctx: &Ctx, meta: &str, mark: &CommandMark) -> Arc<str> {
  with_mark_text(ctx, meta, &mark_text(ctx, mark))
}

fn with_mark_text(ctx: &Ctx, meta: &str, mark: &str) -> Arc<str> {
  ctx.t_args("runs.activity.decision.withMeta", [("meta", meta), ("mark", mark)])
}

fn mark_text(ctx: &Ctx, mark: &CommandMark) -> Arc<str> {
  let (key, server) = match mark {
    CommandMark::Decided(mark) => return decided_text(ctx, mark),
    CommandMark::ExemptTool { server } => ("runs.activity.decision.notAskedExempt", server),
    CommandMark::Declined { server } => ("runs.activity.decision.declinedUnasked", server),
    CommandMark::ApprovedByYou { server } => ("runs.activity.decision.approvedByYou", server),
    CommandMark::AskedYou { server } => ("runs.activity.decision.askedYou", server),
  };
  ctx.t_args(key, [("server", &**server)])
}

/// The mark of an answered request (`ZkOPf`): who answered, and what about (*pencil batch_design*, or a
/// command's category).
fn decided_text(ctx: &Ctx, mark: &DecidedMark) -> Arc<str> {
  let subject = match (mark.command, mark.decision.category) {
    (true, Some(category)) => ctx.t(category_key(category)),
    (true, None) => ctx.t("runs.activity.decision.aCommand"),
    (false, _) => ctx.t_args(
      "runs.activity.decision.tool",
      [("server", &*mark.server), ("tool", &*mark.action)],
    ),
  };
  let key = match mark.decision.decided_by {
    DecidedBy::Person => "runs.activity.decision.approvedByYouOnce",
    DecidedBy::PersonSimilar => "runs.activity.decision.approvedSimilarAdded",
    DecidedBy::PersonAlways => "runs.activity.decision.approvedAlwaysAdded",
    DecidedBy::PersonRun => "runs.activity.decision.approvedRunAdded",
    DecidedBy::PersonDeny => "runs.activity.decision.deniedByYou",
    DecidedBy::SimilarRule => "runs.activity.decision.bySimilarRule",
    DecidedBy::AlwaysAllowed => "runs.activity.decision.alwaysAllowed",
    DecidedBy::AutoApproved => "runs.activity.decision.autoApproved",
  };
  ctx.t_args(key, [("subject", &*subject)])
}

/// A command category's name in a mark.
pub(in crate::screens::project) const fn category_key(category: CommandCategory) -> &'static str {
  match category {
    CommandCategory::ReadOnly => "runs.activity.decision.category.readOnly",
    CommandCategory::Worktree => "runs.activity.decision.category.worktree",
    CommandCategory::Network => "runs.activity.decision.category.network",
    CommandCategory::OutsideWrite => "runs.activity.decision.category.outsideWrite",
    CommandCategory::Unknown => "runs.activity.decision.category.unknown",
    CommandCategory::HistoryDestructive => "runs.activity.decision.category.historyDestructive",
  }
}
