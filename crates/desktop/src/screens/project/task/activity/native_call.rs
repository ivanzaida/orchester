//! A native agent's tool call as its transcript row (04.H10 `IcGvI`): a file tool by its own glyph with
//! its target as the title and the tool name first in the meta (*read_file · lines 1–140 · 4.8 KB*,
//! *search · src/** · 9 matches in 4 files*, *edit_file · M · +38 −6 · open in Changes*); `run_command`
//! as a CLI's command row, with *shell · pwsh* when it used a shell; a command a person denied as
//! *Denied by a person · the agent continues* (*run_command · shell · pwsh*), its string in mono as the body
//! and the result the agent was given in the output block, told apart from one Orchester refused; and an
//! approval as a line of its own (*Approved by you · pnpm add …*, *approve once · waited 48 s*). A CLI's call of
//! Orchester's same file tool reads the same way, from what its input asked (*edit_file · M · +3 −1*), and opens to
//! the change it made rather than to the Changes tab: the change of that one call is known.

use std::{sync::Arc, time::Duration};

use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::stream::{CallChange, CallDetail, CallInput, CommandStatus, DeclinedBy, FileChangeKind, NativeCall},
  native::tools::ToolName,
  run::DecidedBy,
};

use super::{
  command_entry::{self, preview_title},
  entry::{Shown, file_change_kind},
  mcp_call,
};
use crate::{
  display,
  tasks::transcript::{ApprovalEntry, CommandEntry, CommandMark},
  ui::{icon::Icon, message_text},
};

/// The row of native call `entry`, which did what `call` says.
pub(super) fn call(ctx: &Ctx, entry: &CommandEntry, call: &NativeCall) -> Shown {
  if let Some(declined) = &call.declined {
    return declined_command(ctx, entry, call, declined.by, &declined.result);
  }
  if call.tool == ToolName::RunCommand {
    let mut shown = command_entry::command(ctx, entry);
    if let Some(CallDetail::Command { shell: Some(shell) }) = &call.detail {
      shown.meta = ctx.t_args(
        "runs.activity.native.withShell",
        [("meta", &*shown.meta), ("shell", shell.name())],
      );
    }
    return shown;
  }
  match &call.input {
    // A CLI's failed or refused call shows its error as any command of it does.
    Some(_) if failed(entry) => command_entry::command(ctx, entry),
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

/// A CLI's call of one of Orchester's file tools, as the native agent's call of the tool: its target as the title,
/// the tool first in the meta with how much an edit or a write changed, and how a request about it was answered;
/// open, the change it made, or what a read or a search answered. What the native executors count of a read or a
/// search the CLI's answer does not say, so the meta names only the tool. `(status, output, mark)` are its row's.
pub(super) fn requested(
  ctx: &Ctx,
  (tool, input): (ToolName, &CallInput),
  (status, output, mark): (Option<CommandStatus>, &str, Option<&CommandMark>),
) -> Shown {
  let name = tool.as_str();
  let meta = match (status, input.change()) {
    (None, _) => ctx.t_args("runs.activity.native.running", [("tool", name)]),
    (Some(_), Some(change)) => change_meta(ctx, tool, change),
    (Some(_), None) => name.into(),
  };
  let glyph = match tool {
    ToolName::ReadFile => Icon::FileText,
    ToolName::ListFiles => Icon::FolderSearch,
    ToolName::Search => Icon::Search,
    _ => Icon::FilePenLine,
  };
  let title: Arc<str> = match tool {
    ToolName::Search => ctx.t_args("runs.activity.native.searchTitle", [("pattern", input.target())]),
    _ => input.target().into(),
  };
  // The change shows once the call made it; a read or a search shows what it answered.
  let change = input
    .change()
    .filter(|change| status == Some(CommandStatus::Completed) && !change.is_empty())
    .cloned();
  let output: Vec<Arc<str>> = match tool {
    ToolName::EditFile | ToolName::WriteFile => Vec::new(),
    _ => output.lines().map(Arc::from).collect(),
  };
  Shown {
    collapsible: change.is_some() || !output.is_empty(),
    output: Some(output),
    change,
    ..Shown::plain(glyph, title, meta, None).marked(mark.map(|mark| mcp_call::mark_text(ctx, mark)))
  }
}

/// What an edit or a write changed, after the tool's name: the lines it added and removed, or that it replaced every
/// occurrence (how many it does not say); a write's lines, or that it wrote an empty file.
fn change_meta(ctx: &Ctx, tool: ToolName, change: &CallChange) -> Arc<str> {
  let name = tool.as_str();
  let status = ctx.t(file_change_kind(FileChangeKind::Modified).1);
  let added = change.counts().map_or(0, |counts| counts.added);
  match (tool, change.counts()) {
    (ToolName::WriteFile, _) if change.is_empty() => ctx.t_args("runs.activity.native.emptyWritten", [("tool", name)]),
    (ToolName::WriteFile, _) => {
      let key = if added == 1 {
        "runs.activity.native.written.one"
      } else {
        "runs.activity.native.written.other"
      };
      ctx.t_args(key, [("tool", name), ("count", added.to_string().as_str())])
    }
    (_, Some(counts)) => ctx.t_args(
      "runs.activity.native.edited",
      [
        ("tool", name),
        ("status", &*status),
        ("added", counts.added.to_string().as_str()),
        ("removed", counts.removed.to_string().as_str()),
      ],
    ),
    (_, None) => ctx.t_args(
      "runs.activity.native.editedEvery",
      [("tool", name), ("status", &*status)],
    ),
  }
}

/// A file tool's row: its target, what it read, found or changed, and its output while open.
fn file_tool(ctx: &Ctx, entry: &CommandEntry, call: &NativeCall) -> Shown {
  let tool = call.tool.as_str();
  let target = target(&entry.command, call.tool);
  let meta = match (entry.status, &call.detail) {
    (None, _) => ctx.t_args("runs.activity.native.running", [("tool", tool)]),
    (Some(CommandStatus::Completed), Some(detail)) => detail_meta(ctx, tool, detail),
    _ => ctx.t_args("runs.activity.native.failed", [("tool", tool)]),
  };
  let glyph = match (call.tool, &call.detail) {
    (ToolName::ReadFile, _) => Icon::FileText,
    (ToolName::ListFiles, _) => Icon::FolderSearch,
    (ToolName::Search, _) => Icon::Search,
    (_, Some(CallDetail::Changed { change, .. })) => file_change_kind(*change).0,
    _ => Icon::FilePenLine,
  };
  let changed =
    entry.status == Some(CommandStatus::Completed) && matches!(call.detail, Some(CallDetail::Changed { .. }));
  Shown {
    collapsible: !changed,
    opens: changed.then(|| target.clone()),
    output: (!changed).then(|| entry.output.lines().map(Arc::from).collect()),
    ..Shown::plain(glyph, title(ctx, target, call), meta, None)
  }
}

/// A search's title is its pattern in quotes (*“registerShortcut”*), its scope said in the meta; any
/// other file tool's is its target.
fn title(ctx: &Ctx, target: Arc<str>, call: &NativeCall) -> Arc<str> {
  if call.tool != ToolName::Search {
    return target;
  }
  let scope = match &call.detail {
    Some(CallDetail::Searched { scope: Some(scope), .. }) => target.strip_suffix(scope.as_str()),
    _ => None,
  };
  let pattern = scope.map_or(&*target, str::trim_end);
  ctx.t_args("runs.activity.native.searchTitle", [("pattern", pattern)])
}

/// What a finished file tool call did, after the tool's name.
fn detail_meta(ctx: &Ctx, tool: &str, detail: &CallDetail) -> Arc<str> {
  let number = |value: u64| value.to_string();
  match detail {
    CallDetail::Read { first, last, bytes } => {
      let size = message_text(ctx, &display::byte_size(*bytes));
      if *last == 0 {
        ctx.t_args("runs.activity.native.readSize", [("tool", tool), ("size", &*size)])
      } else {
        ctx.t_args(
          "runs.activity.native.readLines",
          [
            ("tool", tool),
            ("first", number(*first).as_str()),
            ("last", number(*last).as_str()),
            ("size", &*size),
          ],
        )
      }
    }
    CallDetail::Listed { entries } => ctx.t_args(
      "runs.activity.native.listed",
      [("tool", tool), ("count", number(*entries).as_str())],
    ),
    CallDetail::Searched { scope, matches, files } => {
      let (matches, files) = (number(*matches), number(*files));
      match scope {
        Some(scope) => ctx.t_args(
          "runs.activity.native.searchedIn",
          [
            ("tool", tool),
            ("scope", scope.as_str()),
            ("matches", matches.as_str()),
            ("files", files.as_str()),
          ],
        ),
        None => ctx.t_args(
          "runs.activity.native.searched",
          [("tool", tool), ("matches", matches.as_str()), ("files", files.as_str())],
        ),
      }
    }
    CallDetail::Changed { change, added, removed } => ctx.t_args(
      "runs.activity.native.changed",
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

/// What a file tool's call is about, from the label Activity gave it: `read_file src/a.rs:1-200`
/// names `src/a.rs`.
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
  match by {
    DeclinedBy::Person => {
      let tool = call.tool.as_str();
      let meta = match &call.detail {
        Some(CallDetail::Command { shell: Some(shell) }) => ctx.t_args(
          "runs.activity.native.withShell",
          [("meta", tool), ("shell", shell.name())],
        ),
        _ => tool.into(),
      };
      let given = std::iter::once(ctx.t("runs.activity.native.given"));
      Shown {
        mono_body: true,
        output: Some(given.chain(result.lines().map(Arc::from)).collect()),
        collapsible: true,
        ..Shown::plain(
          Icon::CircleX,
          ctx.t("runs.activity.native.denied"),
          meta,
          Some(entry.command.clone()),
        )
      }
    }
    DeclinedBy::Orchester => Shown {
      output: Some(result.lines().map(Arc::from).collect()),
      collapsible: true,
      ..Shown::plain(
        Icon::ShieldAlert,
        preview_title(&entry.command),
        ctx.t("runs.activity.native.refused"),
        None,
      )
    },
  }
}

/// A native command's approval, one line: who approved which command, how, and how long it waited for a
/// person (*Approved by you · pnpm add @kontur/keymap*, *approve once · waited 48 s*).
pub(super) fn approval(ctx: &Ctx, approval: &ApprovalEntry) -> Shown {
  let decision = &approval.mark.decision;
  let (title, how) = match decision.decided_by {
    DecidedBy::Person => ("runs.activity.native.approvedByYou", Some("runs.activity.native.once")),
    DecidedBy::PersonSimilar => (
      "runs.activity.native.approvedByYou",
      Some("runs.activity.native.similar"),
    ),
    DecidedBy::PersonAlways => (
      "runs.activity.native.approvedByYou",
      Some("runs.activity.native.always"),
    ),
    DecidedBy::PersonRun => ("runs.activity.native.approvedByYou", Some("runs.activity.native.run")),
    DecidedBy::SimilarRule => ("runs.activity.native.bySimilarRule", None),
    DecidedBy::AlwaysAllowed => ("runs.activity.native.alwaysAllowed", None),
    DecidedBy::AutoApproved => ("runs.activity.native.autoApproved", None),
    DecidedBy::PersonDeny => ("runs.activity.native.denied", None),
    DecidedBy::Permissions => ("runs.activity.native.allowedByProfile", None),
    DecidedBy::Automation => ("runs.activity.native.approvedByAutomation", None),
    DecidedBy::AutomationDeny => ("runs.activity.native.deniedByAutomation", None),
    DecidedBy::Reviewer => ("runs.activity.native.approvedByAutoMode", None),
  };
  // An approval a person gave shows their hand (`IcGvI`), one a rule gave its shield.
  let glyph = if how.is_some() { Icon::Hand } else { Icon::ShieldCheck };
  let how = how.map(|key| ctx.t(key));
  let waited = decision
    .waited_ms
    .map(|millis| message_text(ctx, &display::duration(Duration::from_millis(millis))));
  let meta = match (how, waited) {
    (Some(how), Some(waited)) => ctx.t_args("runs.activity.native.waited", [("how", &*how), ("duration", &*waited)]),
    (Some(how), None) => how,
    (None, _) => "".into(),
  };
  let command = preview_title(&approval.command);
  Shown::plain(glyph, ctx.t_args(title, [("command", &*command)]), meta, None)
}
