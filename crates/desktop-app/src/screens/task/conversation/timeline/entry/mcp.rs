//! MCP in the transcript: how a request was answered, worded as the decision marks of the command categories
//! design (*approved by you · Network*, *not asked · exempt tool of pencil*), on a command's row or on the row
//! of a Codex MCP call, which no command shows; a listed tool that asks after all because Claude Code could name
//! it as another server's tool; and an MCP server the agent works without, and why.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::{mcp::McpServerProblem, stream::CommandStatus},
  permission::category::CommandCategory,
  run::DecidedBy,
};
use orchester_workspace::tasks::transcript::{CommandMark, DecidedMark, McpToolEntry};

use super::{native, shown::Shown};
use crate::ui::icon::Icon;

/// The row of a Codex MCP call: the tool, and how it was decided or which server it belongs to.
/// `works`: its agent works now, so a call without an end still runs.
pub(super) fn call(ctx: &Ctx, call: &McpToolEntry, works: bool) -> Shown {
  let failed = matches!(call.status, Some(CommandStatus::Failed | CommandStatus::Declined));
  let refused = call
    .mark
    .as_ref()
    .is_some_and(|mark| mark.is_denial() || matches!(mark, CommandMark::Declined { .. }));
  // A successful call of one of Orchester's file tools reads as the native agent's call of the tool.
  if let Some(native) = &call.native
    && let Some(input) = &native.input
    && !failed
    && !refused
  {
    return native::requested(ctx, (native.tool, input), (call.status, "", call.mark.as_ref()), works);
  }
  let mark = call.mark.as_ref().map(|mark| mark_text(ctx, mark));
  let meta = if failed {
    ctx.t("conversation.entry.mcpCallFailed")
  } else {
    ctx.t_args("conversation.entry.mcpCall", [("server", &*call.server)])
  };
  let glyph = if failed { Icon::CircleAlert } else { Icon::Plug };
  let told = denial_told(ctx, call.mark.as_ref());
  Shown {
    code: true,
    failed,
    ..Shown::plain(glyph, call.tool.clone(), meta, told).marked(mark)
  }
}

/// What a person's Deny told the agent.
pub(super) fn denial_told(ctx: &Ctx, mark: Option<&CommandMark>) -> Option<Arc<str>> {
  let CommandMark::Decided(mark) = mark? else {
    return None;
  };
  let told = mark.decision.told.as_deref()?;
  Some(ctx.t_args("conversation.decision.told", [("message", told)]))
}

/// A listed tool that got no Claude Code rule: `(server, tool, other)`.
pub(super) fn exemption_not_applied(ctx: &Ctx, (server, tool, other): (&str, &str, &str)) -> Shown {
  let names = [("server", server), ("tool", tool), ("other", other)];
  Shown::plain(
    Icon::TriangleAlert,
    ctx.t_args("conversation.decision.exemptionNotApplied", names),
    "".into(),
    Some(ctx.t_args("conversation.decision.exemptionNotAppliedDetail", names)),
  )
}

/// How a request about a call was answered, as the mark under its row says it (`Decision mark` `CCpkr`).
pub(super) fn mark_text(ctx: &Ctx, mark: &CommandMark) -> Arc<str> {
  let (key, server) = match mark {
    CommandMark::Decided(mark) => return decided_text(ctx, mark),
    CommandMark::ExemptTool { server } => ("conversation.decision.notAskedExempt", server),
    CommandMark::Declined { server } => ("conversation.decision.declinedUnasked", server),
    CommandMark::ApprovedByYou { server } => ("conversation.decision.approvedByYou", server),
    CommandMark::AskedYou { server } => ("conversation.decision.askedYou", server),
  };
  ctx.t_args(key, [("server", &**server)])
}

/// The mark of an answered request: who answered, and what about (*pencil batch_design*, or a command's
/// category).
fn decided_text(ctx: &Ctx, mark: &DecidedMark) -> Arc<str> {
  let subject = match (mark.command, mark.decision.category) {
    (true, Some(category)) => ctx.t(subject_key(category, mark.decision.secrets)),
    (true, None) => ctx.t("conversation.decision.aCommand"),
    (false, _) => ctx.t_args(
      "conversation.decision.tool",
      [("server", &*mark.server), ("tool", &*mark.action)],
    ),
  };
  let key = match mark.decision.decided_by {
    DecidedBy::Person => "conversation.decision.approvedByYouOnce",
    DecidedBy::PersonSimilar => "conversation.decision.approvedSimilarAdded",
    DecidedBy::PersonAlways => "conversation.decision.approvedAlwaysAdded",
    DecidedBy::PersonRun => "conversation.decision.approvedRunAdded",
    DecidedBy::PersonDeny => "conversation.decision.deniedByYou",
    DecidedBy::SimilarRule => "conversation.decision.bySimilarRule",
    DecidedBy::AlwaysAllowed => "conversation.decision.alwaysAllowed",
    DecidedBy::AutoApproved => "conversation.decision.autoApproved",
    DecidedBy::Permissions => "conversation.decision.permissions",
    DecidedBy::Automation => "conversation.decision.approvedByAutomation",
    DecidedBy::AutomationDeny => "conversation.decision.deniedByAutomation",
    DecidedBy::Reviewer => "conversation.decision.approvedByAutoMode",
  };
  ctx.t_args(key, [("subject", &*subject)])
}

/// What a command is called where its category shows: *Credential read* when it reads a credential
/// location, else its category.
pub(super) const fn subject_key(category: CommandCategory, secrets: bool) -> &'static str {
  if secrets {
    return "conversation.decision.category.credentialRead";
  }
  match category {
    CommandCategory::ReadOnly => "conversation.decision.category.readOnly",
    CommandCategory::Worktree => "conversation.decision.category.worktree",
    CommandCategory::Network => "conversation.decision.category.network",
    CommandCategory::OutsideWrite => "conversation.decision.category.outsideWrite",
    CommandCategory::Unknown => "conversation.decision.category.unknown",
    CommandCategory::HistoryDestructive => "conversation.decision.category.historyDestructive",
  }
}

/// An MCP server the agent works without: which, and why; the CLI's own words, when it said why, as output.
pub(super) fn server(ctx: &Ctx, server: Option<&str>, problem: &McpServerProblem) -> Shown {
  let title = match server {
    Some(server) => ctx.t_args("conversation.mcp.title", [("server", server)]),
    None => ctx.t("conversation.mcp.titleUnnamed"),
  };
  let (body, said) = server_problem(ctx, problem);
  Shown {
    output: said.into_iter().collect(),
    ..Shown::plain(Icon::Unplug, title, "".into(), Some(body))
  }
}

fn server_problem(ctx: &Ctx, problem: &McpServerProblem) -> (Arc<str>, Option<Arc<str>>) {
  let one = |key: &str, name: &str, value: &str| ctx.t_args(key, [(name, value)]);
  let body = match problem {
    McpServerProblem::Missing => ctx.t("conversation.mcp.missing"),
    McpServerProblem::Disabled => ctx.t("conversation.mcp.disabled"),
    McpServerProblem::ProgramNotFound { program } => one("conversation.mcp.programNotFound", "program", program),
    McpServerProblem::ProgramMoved { program } => one("conversation.mcp.programMoved", "program", program),
    McpServerProblem::WorkingDirectoryUnsupported => ctx.t("conversation.mcp.workingDirectory"),
    McpServerProblem::UnsafeScriptArgument { position } => one(
      "conversation.mcp.scriptArgument",
      "position",
      &(position + 1).to_string(),
    ),
    McpServerProblem::NotUnicode => ctx.t("conversation.mcp.notUnicode"),
    McpServerProblem::SecretMissing { .. } => ctx.t("conversation.mcp.secretMissing"),
    McpServerProblem::SecretArchived { secret } => one("conversation.mcp.secretArchived", "secret", secret),
    McpServerProblem::SecretUnreadable { secret } => one("conversation.mcp.secretUnreadable", "secret", secret),
    McpServerProblem::SharedVariable { variable } => one("conversation.mcp.sharedVariable", "variable", variable),
    McpServerProblem::ReservedVariable { variable } => one("conversation.mcp.reservedVariable", "variable", variable),
    McpServerProblem::FailedToStart { detail } => {
      return (
        ctx.t("conversation.mcp.failedToStart"),
        detail.as_deref().map(Arc::from),
      );
    }
    McpServerProblem::ToolRelaysMissing { reached, started } => ctx.t_args(
      "conversation.mcp.toolRelaysMissing",
      [
        ("reached", reached.to_string().as_str()),
        ("started", started.to_string().as_str()),
      ],
    ),
    McpServerProblem::NotSupportedByConnection => ctx.t("conversation.mcp.notSupportedByConnection"),
  };
  (body, None)
}
