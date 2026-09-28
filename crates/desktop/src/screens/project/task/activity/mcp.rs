//! A transcript entry for an MCP server the agent works without (`McpServer` events): which
//! server, and why — left out before the CLI started, or reported by the CLI as not started.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::connection::mcp::McpServerProblem;

/// The entry's title and the sentence under it; the CLI's own words, when it said why, go in the
/// output block.
pub(super) fn mcp_server_text(
  ctx: &Ctx,
  server: Option<&str>,
  problem: &McpServerProblem,
) -> (Arc<str>, Arc<str>, Option<Arc<str>>) {
  let title = match server {
    Some(server) => ctx.t_args("runs.activity.mcp.title", [("server", server)]),
    None => ctx.t("runs.activity.mcp.titleUnnamed"),
  };
  let (body, said) = match problem {
    McpServerProblem::Missing => (ctx.t("runs.activity.mcp.missing"), None),
    McpServerProblem::Disabled => (ctx.t("runs.activity.mcp.disabled"), None),
    McpServerProblem::ProgramNotFound { program } => (
      ctx.t_args("runs.activity.mcp.programNotFound", [("program", program.as_str())]),
      None,
    ),
    McpServerProblem::WorkingDirectoryUnsupported => (ctx.t("runs.activity.mcp.workingDirectory"), None),
    McpServerProblem::UnsafeScriptArgument { position } => (
      ctx.t_args(
        "runs.activity.mcp.scriptArgument",
        [("position", (position + 1).to_string().as_str())],
      ),
      None,
    ),
    McpServerProblem::NotUnicode => (ctx.t("runs.activity.mcp.notUnicode"), None),
    McpServerProblem::SecretMissing { .. } => (ctx.t("runs.activity.mcp.secretMissing"), None),
    McpServerProblem::SecretArchived { secret } => (
      ctx.t_args("runs.activity.mcp.secretArchived", [("secret", secret.as_str())]),
      None,
    ),
    McpServerProblem::SecretUnreadable { secret } => (
      ctx.t_args("runs.activity.mcp.secretUnreadable", [("secret", secret.as_str())]),
      None,
    ),
    McpServerProblem::SharedVariable { variable } => (
      ctx.t_args("runs.activity.mcp.sharedVariable", [("variable", variable.as_str())]),
      None,
    ),
    McpServerProblem::ReservedVariable { variable } => (
      ctx.t_args("runs.activity.mcp.reservedVariable", [("variable", variable.as_str())]),
      None,
    ),
    McpServerProblem::FailedToStart { detail } => (
      ctx.t("runs.activity.mcp.failedToStart"),
      detail.as_deref().map(Arc::from),
    ),
  };
  (title, body, said)
}
