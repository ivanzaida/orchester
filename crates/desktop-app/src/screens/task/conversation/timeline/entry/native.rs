//! A native agent's tool call as its row (04.H10 `IcGvI`): a file tool by its own glyph with its target as
//! the title and the tool first in the meta (*read_file · lines 1–140 · 4.8 KB*, *edit_file · M · +38 −6 ·
//! open in Changes*); `run_command` as a command's row, with *shell · pwsh* when it used a shell; a command a
//! person denied, its string as the body and the result the agent was given as output, told apart from one
//! Orchester refused; and an approval as a line of its own (*Approved by you · pnpm add …*). A CLI's call of
//! Orchester's same file tool reads the same way, from what its input asked, and opens to the change it made.

use std::{sync::Arc, time::Duration};

use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::stream::{CallChange, CallDetail, CallInput, CommandStatus, DeclinedBy, FileChangeKind, NativeCall},
  native::tools::ToolName,
  run::DecidedBy,
};
use orchester_workspace::{
  display,
  tasks::transcript::{ApprovalEntry, CommandEntry, CommandMark},
};

use super::{
  mcp,
  shown::{self, Shown, file_change_kind, lines, preview_title},
};
use crate::ui::{icon::Icon, message::message_text};

/// The row of native call `entry`, which did what `call` says.
pub(super) fn call(ctx: &Ctx, entry: &CommandEntry, call: &NativeCall) -> Shown {
  if let Some(declined) = &call.declined {
    return declined_command(ctx, entry, call, declined.by, &declined.result);
  }
  if call.tool == ToolName::RunCommand {
    let mut shown = shown::command(ctx, entry);
    if let Some(CallDetail::Command { shell: Some(shell) }) = &call.detail {
      shown.meta = ctx.t_args(
        "conversation.native.withShell",
        [("meta", &*shown.meta), ("shell", shell.name())],
      );
    }
    return shown;
  }
  match &call.input {
    // A CLI's failed or refused call shows its error as any command of it does.
    Some(_) if failed(entry) => shown::command(ctx, entry),
    Some(input) => requested(
      ctx,
      (call.tool, input),
      (entry.status, &entry.output, entry.mark.as_ref()),
    ),
    None => file_tool(ctx, entry, call),
  }
}

fn failed(entry: &CommandEntry) -> bool {
  matches!(entry.status, Some(CommandStatus::Failed | CommandStatus::Declined))
    || entry.mark.as_ref().is_some_and(CommandMark::is_denial)
}

/// A CLI's call of one of Orchester's file tools, as the native agent's call of the tool: its target as the
/// title, the tool first in the meta with how much an edit or a write changed, and how a request about it was
/// answered; open, the change it made, or what a read or a search answered.
pub(super) fn requested(
  ctx: &Ctx,
  (tool, input): (ToolName, &CallInput),
  (status, output, mark): (Option<CommandStatus>, &str, Option<&CommandMark>),
) -> Shown {
  let name = tool.as_str();
  let meta = match (status, input.change()) {
    (None, _) => ctx.t_args("conversation.native.running", [("tool", name)]),
    (Some(_), Some(change)) => change_meta(ctx, tool, change),
    (Some(_), None) => match input.scope() {
      Some(scope) => ctx.t_args("conversation.native.searchScope", [("tool", name), ("scope", scope)]),
      None => name.into(),
    },
  };
  let title: Arc<str> = match tool {
    ToolName::Search => ctx.t_args("conversation.native.searchTitle", [("pattern", input.target())]),
    _ => input.target().into(),
  };
  // The change shows once the call made it; a read or a search shows what it answered.
  let change = input
    .change()
    .filter(|change| status == Some(CommandStatus::Completed) && !change.is_empty())
    .cloned();
  let output = match tool {
    ToolName::EditFile | ToolName::WriteFile => Vec::new(),
    _ => lines(output),
  };
  Shown {
    collapsible: change.is_some() || !output.is_empty(),
    output,
    change,
    ..Shown::plain(tool_glyph(tool), title, meta, None).marked(mark.map(|mark| mcp::mark_text(ctx, mark)))
  }
}

const fn tool_glyph(tool: ToolName) -> Icon {
  match tool {
    ToolName::ReadFile => Icon::FileText,
    ToolName::ListFiles => Icon::FolderSearch,
    ToolName::Search => Icon::Search,
    _ => Icon::FilePenLine,
  }
}

/// What an edit or a write changed, after the tool's name.
fn change_meta(ctx: &Ctx, tool: ToolName, change: &CallChange) -> Arc<str> {
  let name = tool.as_str();
  let status = ctx.t(file_change_kind(FileChangeKind::Modified).1);
  let added = change.counts().map_or(0, |counts| counts.added);
  match (tool, change.counts()) {
    (ToolName::WriteFile, _) if change.is_empty() => ctx.t_args("conversation.native.emptyWritten", [("tool", name)]),
    (ToolName::WriteFile, _) if added == 1 => ctx.t_args("conversation.native.written.one", [("tool", name)]),
    (ToolName::WriteFile, _) => ctx.t_args(
      "conversation.native.written.other",
      [("tool", name), ("count", added.to_string().as_str())],
    ),
    (_, Some(counts)) => ctx.t_args(
      "conversation.native.edited",
      [
        ("tool", name),
        ("status", &*status),
        ("added", counts.added.to_string().as_str()),
        ("removed", counts.removed.to_string().as_str()),
      ],
    ),
    (_, None) => ctx.t_args(
      "conversation.native.editedEvery",
      [("tool", name), ("status", &*status)],
    ),
  }
}

/// A file tool's row: its target, what it read, found or changed, and its output while open; a change
/// opens the file in the Changes tab.
fn file_tool(ctx: &Ctx, entry: &CommandEntry, call: &NativeCall) -> Shown {
  let tool = call.tool.as_str();
  let target = target(&entry.command, call.tool);
  let meta = match (entry.status, &call.detail) {
    (None, _) => ctx.t_args("conversation.native.running", [("tool", tool)]),
    (Some(CommandStatus::Completed), Some(detail)) => detail_meta(ctx, tool, detail),
    _ => ctx.t_args("conversation.native.failed", [("tool", tool)]),
  };
  let glyph = match (call.tool, &call.detail) {
    (_, Some(CallDetail::Changed { change, .. })) => file_change_kind(*change).0,
    (tool, _) => tool_glyph(tool),
  };
  let changed =
    entry.status == Some(CommandStatus::Completed) && matches!(call.detail, Some(CallDetail::Changed { .. }));
  Shown {
    failed: entry.status == Some(CommandStatus::Failed),
    collapsible: !changed,
    opens: changed.then(|| target.clone()),
    output: if changed { Vec::new() } else { lines(&entry.output) },
    ..Shown::plain(glyph, title(ctx, target, call), meta, None)
  }
}

/// A search's title is its pattern in quotes, its scope said in the meta; any other file tool's is its
/// target.
fn title(ctx: &Ctx, target: Arc<str>, call: &NativeCall) -> Arc<str> {
  if call.tool != ToolName::Search {
    return target;
  }
  let scope = match &call.detail {
    Some(CallDetail::Searched { scope: Some(scope), .. }) => target.strip_suffix(scope.as_str()),
    _ => None,
  };
  let pattern = scope.map_or(&*target, str::trim_end);
  ctx.t_args("conversation.native.searchTitle", [("pattern", pattern)])
}

/// What a finished file tool call did, after the tool's name.
fn detail_meta(ctx: &Ctx, tool: &str, detail: &CallDetail) -> Arc<str> {
  let number = |value: u64| value.to_string();
  match detail {
    CallDetail::Read { first, last, bytes } => {
      let size = message_text(ctx, &display::byte_size(*bytes));
      if *last == 0 {
        return ctx.t_args("conversation.native.readSize", [("tool", tool), ("size", &*size)]);
      }
      let (first, last) = (number(*first), number(*last));
      ctx.t_args(
        "conversation.native.readLines",
        [("tool", tool), ("first", &first), ("last", &last), ("size", &*size)],
      )
    }
    CallDetail::Listed { entries } => ctx.t_args(
      "conversation.native.listed",
      [("tool", tool), ("count", number(*entries).as_str())],
    ),
    CallDetail::Searched { scope, matches, files } => {
      let (matches, files) = (number(*matches), number(*files));
      match scope {
        Some(scope) => ctx.t_args(
          "conversation.native.searchedIn",
          [
            ("tool", tool),
            ("scope", scope.as_str()),
            ("matches", &matches),
            ("files", &files),
          ],
        ),
        None => ctx.t_args(
          "conversation.native.searched",
          [("tool", tool), ("matches", &matches), ("files", &files)],
        ),
      }
    }
    CallDetail::Changed { change, added, removed } => ctx.t_args(
      "conversation.native.changed",
      [
        ("tool", tool),
        ("status", &*ctx.t(file_change_kind(*change).1)),
        ("added", number(*added).as_str()),
        ("removed", number(*removed).as_str()),
      ],
    ),
    // A file tool reports no command; its name, an identifier, says what it was.
    CallDetail::Command { .. } => tool.into(),
  }
}

/// What a file tool's call is about, from its label: `read_file src/a.rs:1-200` names `src/a.rs`.
fn target(label: &Arc<str>, tool: ToolName) -> Arc<str> {
  let rest = label.strip_prefix(tool.as_str()).map(str::trim_start).unwrap_or(label);
  let rest = match tool {
    ToolName::ReadFile => rest.rsplit_once(':').map_or(rest, |(path, _)| path),
    _ => rest,
  };
  if rest.is_empty() { ".".into() } else { rest.into() }
}

/// A command that did not run: a person's denial, or Orchester's refusal, with the result the agent got.
fn declined_command(ctx: &Ctx, entry: &CommandEntry, call: &NativeCall, by: DeclinedBy, result: &str) -> Shown {
  let output = |first: Option<Arc<str>>| first.into_iter().chain(result.lines().map(Arc::from)).collect();
  match by {
    DeclinedBy::Person => {
      let tool = call.tool.as_str();
      let meta = match &call.detail {
        Some(CallDetail::Command { shell: Some(shell) }) => ctx.t_args(
          "conversation.native.withShell",
          [("meta", tool), ("shell", shell.name())],
        ),
        _ => tool.into(),
      };
      Shown {
        failed: true,
        mono_body: true,
        output: output(Some(ctx.t("conversation.native.given"))),
        collapsible: true,
        ..Shown::plain(
          Icon::CircleX,
          ctx.t("conversation.native.denied"),
          meta,
          Some(entry.command.clone()),
        )
      }
    }
    DeclinedBy::Orchester => Shown {
      failed: true,
      output: output(None),
      collapsible: true,
      ..Shown::plain(
        Icon::ShieldAlert,
        preview_title(&entry.command),
        ctx.t("conversation.native.refused"),
        None,
      )
    },
  }
}

/// A native command's approval, one line: who approved which command, how, and how long it waited for a
/// person.
pub(super) fn approval(ctx: &Ctx, approval: &ApprovalEntry) -> Shown {
  let decision = &approval.mark.decision;
  let (title, how) = match decision.decided_by {
    DecidedBy::Person => ("conversation.native.approvedByYou", Some("conversation.native.once")),
    DecidedBy::PersonSimilar => ("conversation.native.approvedByYou", Some("conversation.native.similar")),
    DecidedBy::PersonAlways => ("conversation.native.approvedByYou", Some("conversation.native.always")),
    DecidedBy::PersonRun => ("conversation.native.approvedByYou", Some("conversation.native.run")),
    DecidedBy::SimilarRule => ("conversation.native.bySimilarRule", None),
    DecidedBy::AlwaysAllowed => ("conversation.native.alwaysAllowed", None),
    DecidedBy::AutoApproved => ("conversation.native.autoApproved", None),
    DecidedBy::PersonDeny => ("conversation.native.deniedCommand", None),
    DecidedBy::Permissions => ("conversation.native.allowedByProfile", None),
    DecidedBy::Automation => ("conversation.native.approvedByAutomation", None),
    DecidedBy::AutomationDeny => ("conversation.native.deniedByAutomation", None),
    DecidedBy::Reviewer => ("conversation.native.approvedByAutoMode", None),
  };
  // An approval a person gave shows their hand, one a rule gave its shield.
  let glyph = if how.is_some() { Icon::Hand } else { Icon::ShieldCheck };
  let how = how.map(|key| ctx.t(key));
  let waited = decision
    .waited_ms
    .map(|millis| message_text(ctx, &display::duration(Duration::from_millis(millis))));
  let meta = match (how, waited) {
    (Some(how), Some(waited)) => ctx.t_args("conversation.native.waited", [("how", &*how), ("duration", &*waited)]),
    (Some(how), None) => how,
    (None, _) => "".into(),
  };
  let command = preview_title(&approval.command);
  Shown::plain(glyph, ctx.t_args(title, [("command", &*command)]), meta, None)
}
