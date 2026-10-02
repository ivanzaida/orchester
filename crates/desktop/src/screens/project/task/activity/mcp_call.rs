//! MCP tool calls in the transcript: how a call was decided, worded as the decision marks of the command
//! categories design (*not asked · exempt tool of pencil*, *approved by you · pencil*), on Claude Code's
//! command row or on the row of a Codex call, which no command shows; and a listed tool that asks after
//! all because Claude Code could name it as another server's tool.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::connection::stream::CommandStatus;

use super::entry::Shown;
use crate::{
  tasks::transcript::{CommandMark, McpToolEntry},
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
  Shown::plain(glyph, call.tool.clone(), meta, None)
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
    CommandMark::ExemptTool { server } => ("runs.activity.decision.notAskedExempt", server),
    CommandMark::Declined { server } => ("runs.activity.decision.declinedUnasked", server),
    CommandMark::ApprovedByYou { server } => ("runs.activity.decision.approvedByYou", server),
    CommandMark::AskedYou { server } => ("runs.activity.decision.askedYou", server),
  };
  ctx.t_args(key, [("server", &**server)])
}
