//! What a transcript entry's row says, whatever it is: an agent's command with how it ended and how its
//! request was answered, a tool step's command with the output its tool kept, a file change (it opens the
//! file in the Changes tab), reasoning, usage, an error, unparsed lines; a native agent's call
//! ([`super::native`]), an MCP call or server ([`super::mcp`]) and a runner problem ([`super::problem`]).

use std::{borrow::Cow, sync::Arc, time::Duration};

use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::stream::{CallChange, CommandStatus, FileChangeKind, OutputStream, ToolCommandResult},
  tokens::compact_tokens,
};
use orchester_workspace::{
  display,
  tasks::transcript::{CommandEntry, Entry, EntryKind, ToolCommandEntry, command_preview, in_worktree},
};

use super::{mcp, native, problem};
use crate::ui::{icon::Icon, message::message_text};

/// What an entry's row shows, apart from its time and whether it is open.
pub(super) struct Shown {
  pub(super) glyph: Icon,
  pub(super) title: Arc<str>,
  /// The title is a command, a path or a tool's name, set in mono; words are set in sans.
  pub(super) code: bool,
  /// How it ended, or what it is, after the title.
  pub(super) meta: Arc<str>,
  /// It failed: its meta is in `danger`.
  pub(super) failed: bool,
  /// How a request about it was answered, on a line under it.
  pub(super) mark: Option<Arc<str>>,
  pub(super) body: Option<Arc<str>>,
  /// The body is a command, set in mono.
  pub(super) mono_body: bool,
  /// Lines of the output block, shown while the row is open.
  pub(super) output: Vec<Arc<str>>,
  /// What the output block leaves out, said above it.
  pub(super) omitted: Option<Arc<str>>,
  /// The change a CLI's call of Orchester's edit tool made, shown while the row is open.
  pub(super) change: Option<Arc<CallChange>>,
  /// The command whole, when the title shows it on one line.
  pub(super) command: Option<Arc<str>>,
  /// Opens and folds on a press.
  pub(super) collapsible: bool,
  /// The repository path a press opens in the Changes tab.
  pub(super) opens: Option<Arc<str>>,
}

impl Shown {
  /// One line of `title` and `meta`, with `body` under it.
  pub(super) const fn plain(glyph: Icon, title: Arc<str>, meta: Arc<str>, body: Option<Arc<str>>) -> Self {
    Self {
      glyph,
      title,
      code: false,
      meta,
      failed: false,
      mark: None,
      body,
      mono_body: false,
      output: Vec::new(),
      omitted: None,
      change: None,
      command: None,
      collapsible: false,
      opens: None,
    }
  }

  /// With `mark` under it.
  pub(super) fn marked(self, mark: Option<Arc<str>>) -> Self {
    Self { mark, ..self }
  }

  /// Whether an open row has anything under its line.
  pub(super) fn has_details(&self) -> bool {
    self.body.is_some() || self.change.is_some() || self.command.is_some() || !self.output.is_empty()
  }
}

/// The lines of `text`.
pub(super) fn lines(text: &str) -> Vec<Arc<str>> {
  text.lines().map(Arc::from).collect()
}

/// `works`: the entry's agent works now, so a call without an end still runs.
pub(super) fn shown(ctx: &Ctx, entry: &Entry, works: bool) -> Shown {
  let plain = Shown::plain;
  match &entry.kind {
    EntryKind::Message(text) => plain(
      Icon::MessageSquare,
      ctx.t("conversation.entry.agentMessage"),
      "".into(),
      Some(text.clone()),
    ),
    EntryKind::Reasoning(text) => Shown {
      collapsible: true,
      ..plain(
        Icon::Lightbulb,
        ctx.t("conversation.entry.reasoning"),
        "".into(),
        Some(text.clone()),
      )
    },
    EntryKind::Command(command) => match &command.native {
      Some(call) => native::call(ctx, command, call, works),
      None => self::command(ctx, command, works),
    },
    EntryKind::Approval(approval) => native::approval(ctx, approval),
    EntryKind::FileChange(change) => file_change(ctx, &change.path, change.kind),
    EntryKind::Usage(usage) => {
      let meta = ctx.t_args(
        "conversation.entry.usageMeta",
        [
          ("input", compact_tokens(usage.input).as_str()),
          ("output", compact_tokens(usage.output).as_str()),
          ("cached", compact_tokens(usage.cached_input).as_str()),
        ],
      );
      plain(Icon::Gauge, ctx.t("conversation.entry.usage"), meta, None)
    }
    EntryKind::Error(message) => plain(
      Icon::CircleAlert,
      ctx.t("conversation.entry.error"),
      "".into(),
      Some(message.clone()),
    ),
    EntryKind::NotSignedIn(message) => Shown {
      output: vec![message.clone()],
      ..plain(
        Icon::CircleAlert,
        ctx.t("conversation.entry.notSignedIn"),
        "".into(),
        Some(ctx.t("conversation.entry.notSignedInDetail")),
      )
    },
    EntryKind::Problem(found) => {
      let (title, body) = problem::text(ctx, found);
      plain(Icon::CircleAlert, title, "".into(), body)
    }
    EntryKind::McpServer(server, found) => mcp::server(ctx, server.as_deref(), found),
    EntryKind::Raw(lines) => unparsed(ctx, lines),
    EntryKind::ToolCommand(command) => tool_command(ctx, command, works),
    EntryKind::McpTool(call) => mcp::call(ctx, call, works),
    EntryKind::ExemptionNotApplied { server, tool, other } => mcp::exemption_not_applied(ctx, (server, tool, other)),
  }
}

/// A file change: its status and path; pressed, it opens the file in the Changes tab.
fn file_change(ctx: &Ctx, path: &str, kind: FileChangeKind) -> Shown {
  let (glyph, status) = file_change_kind(kind);
  // Git's paths use `/` whatever the CLI reported.
  let path: Arc<str> = in_worktree(path).unwrap_or(path).replace('\\', "/").into();
  let title = ctx.t_args(
    "conversation.entry.file",
    [("status", &*ctx.t(status)), ("path", &*path)],
  );
  Shown {
    code: true,
    opens: Some(path),
    ..Shown::plain(glyph, title, ctx.t("conversation.entry.openInChanges"), None)
  }
}

/// A file change's glyph and the key of its status.
pub(super) const fn file_change_kind(kind: FileChangeKind) -> (Icon, &'static str) {
  match kind {
    FileChangeKind::Added => (Icon::FilePlus, "conversation.entry.fileAdded"),
    FileChangeKind::Deleted => (Icon::FileMinus, "conversation.entry.fileDeleted"),
    FileChangeKind::Modified => (Icon::FilePen, "conversation.entry.fileModified"),
  }
}

/// An agent's command: how it ended, how its request was answered, its output while open.
pub(super) fn command(ctx: &Ctx, entry: &CommandEntry, works: bool) -> Shown {
  let (glyph, meta, body) = match entry.status {
    None => (Icon::Terminal, unfinished(ctx, works), None),
    Some(CommandStatus::Declined) => (
      Icon::CircleAlert,
      ctx.t("conversation.entry.declined"),
      Some(ctx.t("conversation.entry.declinedDetail")),
    ),
    // A command that completed with neither an exit code nor a duration has nothing to say of its end.
    Some(status @ (CommandStatus::Completed | CommandStatus::Failed)) => (
      Icon::Terminal,
      ended(ctx, (status == CommandStatus::Failed, entry.exit_code, entry.duration)),
      None,
    ),
  };
  // A person's Deny: what the agent was told says why the command did not run.
  let body = mcp::denial_told(ctx, entry.mark.as_ref()).or(body);
  Shown {
    code: true,
    failed: entry.status == Some(CommandStatus::Failed),
    mark: entry.mark.as_ref().map(|mark| mcp::mark_text(ctx, mark)),
    output: lines(&entry.output),
    command: whole(&entry.command),
    collapsible: true,
    ..Shown::plain(glyph, preview_title(&entry.command), meta, body)
  }
}

/// A tool step's command: its command line, how it ended and the output its tool kept.
pub(super) fn tool_command(ctx: &Ctx, entry: &ToolCommandEntry, works: bool) -> Shown {
  let meta = match entry.result {
    Some(result) => tool_ended(ctx, result, (entry.exit_code, entry.duration)),
    None => unfinished(ctx, works),
  };
  let omitted = (entry.omitted_bytes > 0).then(|| {
    let size = message_text(ctx, &display::byte_size(entry.omitted_bytes));
    ctx.t_args("conversation.entry.outputOmitted", [("size", &*size)])
  });
  Shown {
    code: true,
    failed: matches!(
      entry.result,
      Some(ToolCommandResult::Failed | ToolCommandResult::TimedOut)
    ),
    output: lines(&entry.output),
    omitted,
    command: whole(&entry.command),
    collapsible: true,
    ..Shown::plain(Icon::Terminal, preview_title(&entry.command), meta, None)
  }
}

/// How long a command took, from its start to its end: *9 s*, *2 min*.
fn took(ctx: &Ctx, millis: i64) -> Arc<str> {
  // A clock set back while it ran reads as no time, never as a negative one.
  let millis = u64::try_from(millis).unwrap_or_default();
  message_text(ctx, &display::duration(Duration::from_millis(millis)))
}

/// How an agent's command ended: its exit code and how long it took (*exit 1 · 9 s*), or that it failed
/// when its CLI reported no code; empty when it completed with neither a code nor a time to show.
fn ended(ctx: &Ctx, (failed, code, millis): (bool, Option<i64>, Option<i64>)) -> Arc<str> {
  let took = millis.map(|millis| took(ctx, millis));
  match (code, failed, took) {
    (Some(code), _, Some(took)) => ctx.t_args(
      "conversation.entry.exitTook",
      [("code", code.to_string().as_str()), ("took", &*took)],
    ),
    (Some(code), _, None) => ctx.t_args("conversation.entry.exit", [("code", code.to_string().as_str())]),
    (None, true, Some(took)) => ctx.t_args("conversation.entry.failedTook", [("took", &*took)]),
    (None, true, None) => ctx.t("conversation.entry.failed"),
    (None, false, Some(took)) => took,
    (None, false, None) => "".into(),
  }
}

/// How a tool step's command ended: stopped, timed out, or its exit code (*no exit code* without one), with
/// how long it took when its start and end were seen.
fn tool_ended(ctx: &Ctx, result: ToolCommandResult, (code, millis): (Option<i64>, Option<i64>)) -> Arc<str> {
  let took = millis.map(|millis| took(ctx, millis));
  match (result, code, took) {
    (ToolCommandResult::Stopped, ..) => ctx.t("conversation.entry.stopped"),
    (ToolCommandResult::TimedOut, _, Some(took)) => ctx.t_args("conversation.entry.timedOutTook", [("took", &*took)]),
    (ToolCommandResult::TimedOut, _, None) => ctx.t("conversation.entry.timedOut"),
    (_, Some(code), took) => ended(ctx, (false, Some(code), millis.filter(|_| took.is_some()))),
    (_, None, Some(took)) => ctx.t_args("conversation.entry.noExitCodeTook", [("took", &*took)]),
    (_, None, None) => ctx.t("conversation.entry.noExitCode"),
  }
}

/// A command without an end: running while its agent works, else without an end (the run waits, was paused
/// or interrupted): never progress of its own.
fn unfinished(ctx: &Ctx, works: bool) -> Arc<str> {
  if works {
    ctx.t("conversation.entry.running")
  } else {
    ctx.t("conversation.entry.unfinished")
  }
}

/// Unparsed lines, counted; their lines while open.
fn unparsed(ctx: &Ctx, lines: &[(OutputStream, Arc<str>)]) -> Shown {
  let title = if lines.len() == 1 {
    ctx.t("conversation.entry.unparsed.one")
  } else {
    ctx.t_args(
      "conversation.entry.unparsed.other",
      [("count", lines.len().to_string().as_str())],
    )
  };
  let output = lines
    .iter()
    .map(|(stream, line)| match stream {
      OutputStream::Stdout => line.clone(),
      OutputStream::Stderr => ctx.t_args("conversation.entry.stderrLine", [("line", &**line)]),
    })
    .collect();
  Shown {
    output,
    collapsible: true,
    ..Shown::plain(Icon::CodeXml, title, ctx.t("conversation.entry.rawOutput"), None)
  }
}

/// The row's one-line title, sharing the command itself when it already is one line.
pub(super) fn preview_title(command: &Arc<str>) -> Arc<str> {
  match command_preview(command) {
    Cow::Borrowed(_) => command.clone(),
    Cow::Owned(preview) => preview.into(),
  }
}

/// The command whole, when its title had to put it on one line.
fn whole(command: &Arc<str>) -> Option<Arc<str>> {
  matches!(command_preview(command), Cow::Owned(_)).then(|| command.clone())
}
