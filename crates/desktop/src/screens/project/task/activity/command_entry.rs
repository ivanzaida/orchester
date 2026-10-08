//! A command's transcript entry: an agent's command, with how it ended and how its request was
//! answered, and a tool step's command tool with the output its tool kept.

use std::{borrow::Cow, sync::Arc};

use lurq::app::ctx::Ctx;
use orchester_core::connection::stream::{CommandStatus, ToolCommandResult};

use super::{entry::Shown, mcp_call};
use crate::{
  display,
  tasks::transcript::{CommandEntry, ToolCommandEntry, tenths_of_seconds},
  ui::{command_row::command_preview, icon::Icon, message_text},
};

pub(super) fn command(ctx: &Ctx, entry: &CommandEntry) -> Shown {
  let (glyph, meta, body) = match (entry.status, entry.ended()) {
    (Some(CommandStatus::Declined), _) => (
      Icon::AlertCircle,
      ctx.t("runs.activity.declined"),
      Some(ctx.t("runs.activity.declinedDetail")),
    ),
    (_, Some(ended)) => (Icon::Terminal, message_text(ctx, &ended), None),
    (_, None) => (Icon::Terminal, ctx.t("runs.activity.commandRunning"), None),
  };
  // A person's Deny: what the agent was told says why the command did not run.
  let body = mcp_call::denial_told(ctx, entry.mark.as_ref()).or(body);
  Shown {
    glyph,
    title: preview_title(&entry.command),
    meta,
    mark: entry.mark.as_ref().map(|mark| mcp_call::mark_text(ctx, mark)),
    body,
    mono_body: false,
    output: Some(entry.output.lines().map(Arc::from).collect()),
    omitted: None,
    change: None,
    collapsible: true,
    opens: None,
  }
}

/// A tool step's command: its command line, how it ended and the output its tool kept.
pub(super) fn tool_command(ctx: &Ctx, entry: &ToolCommandEntry) -> Shown {
  let seconds = entry.duration.map(tenths_of_seconds).unwrap_or_default();
  let code = entry.exit_code.map(|code| code.to_string()).unwrap_or_default();
  let meta = match (entry.result, entry.exit_code) {
    (None, _) => ctx.t("runs.activity.commandRunning"),
    (Some(ToolCommandResult::TimedOut), _) => ctx.t_args("runs.activity.timedOut", [("seconds", seconds.as_str())]),
    (Some(ToolCommandResult::Stopped), _) => ctx.t("runs.activity.stopped"),
    (Some(_), Some(_)) => ctx.t_args(
      "runs.activity.exitDuration",
      [("code", code.as_str()), ("seconds", seconds.as_str())],
    ),
    (Some(_), None) => ctx.t_args("runs.activity.noExitCode", [("seconds", seconds.as_str())]),
  };
  let omitted = (entry.omitted_bytes > 0).then(|| {
    let size = message_text(ctx, &display::byte_size(entry.omitted_bytes));
    ctx.t_args("runs.activity.outputOmitted", [("size", &*size)])
  });
  Shown {
    glyph: Icon::Terminal,
    title: preview_title(&entry.command),
    meta,
    mark: None,
    body: None,
    mono_body: false,
    output: Some(entry.output.lines().map(Arc::from).collect()),
    omitted,
    change: None,
    collapsible: true,
    opens: None,
  }
}

/// The header's one-line preview, sharing the command itself when it already is one line.
pub(super) fn preview_title(command: &Arc<str>) -> Arc<str> {
  match command_preview(command) {
    Cow::Borrowed(_) => command.clone(),
    Cow::Owned(preview) => preview.into(),
  }
}
