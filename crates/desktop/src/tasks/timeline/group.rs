//! Tool groups (`uoK7y`, `docs/design/orchester-workspace.md`, "Grouping rules"): the longest run of
//! consecutive tool events of one step execution between two items that are not tool events. Tool
//! events are commands, file reads and searches, file changes, MCP tool calls, memory searches,
//! reasoning summaries, usage reports and unparsed lines; agent messages and problems never fold
//! into a group. A group's summary counts by kind: files read, searches, memory, commands run (named
//! when there are one or two), files edited, MCP calls; a failed command is named with its exit
//! code, and a group whose command still runs ends with it.

use std::{collections::BTreeSet, sync::Arc};

use orchester_core::{
  connection::stream::{CommandStatus, ToolCommandResult},
  time::Timestamp,
};

use crate::tasks::transcript::{CommandMark, Entry, EntryKind};

/// Commands a group names before it only counts them.
const NAMED_COMMANDS: usize = 2;
/// Programs that read a file, and ones that search.
const READERS: [&str; 10] = [
  "cat",
  "head",
  "tail",
  "less",
  "more",
  "type",
  "get-content",
  "gc",
  "nl",
  "bat",
];
const SEARCHERS: [&str; 12] = [
  "rg",
  "grep",
  "find",
  "fd",
  "findstr",
  "select-string",
  "sls",
  "ag",
  "ls",
  "dir",
  "get-childitem",
  "gci",
];
/// Claude Code's tools, as its transcript names them, that read and that search.
const READ_TOOLS: [&str; 2] = ["Read", "NotebookRead"];
const SEARCH_TOOLS: [&str; 5] = ["Grep", "Glob", "LS", "WebSearch", "WebFetch"];
/// Shells a CLI runs a command in, and the option before the command they run.
const SHELLS: [&str; 6] = ["pwsh", "powershell", "bash", "sh", "zsh", "cmd"];
const SHELL_OPTIONS: [&str; 4] = ["-command", "-c", "-lc", "/c"];

/// What a tool event is, for the summary of its group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
  Read,
  Search,
  Memory,
  Command,
  Edit,
  Mcp,
  /// Reasoning, usage and unparsed lines: in the group, not counted.
  Note,
}

/// Whether `entry` is a tool event, and which; `None` for a message or a problem.
pub fn tool_kind(entry: &Entry) -> Option<ToolKind> {
  match &entry.kind {
    EntryKind::Command(command) if command.status == Some(CommandStatus::Declined) => None,
    EntryKind::Command(command) => Some(command_kind(&command.command)),
    EntryKind::ToolCommand(_) => Some(ToolKind::Command),
    EntryKind::FileChange(_) => Some(ToolKind::Edit),
    // A declined call is a problem row whatever it calls, a memory tool too, and however it ended.
    EntryKind::McpTool(call) if matches!(call.mark, Some(CommandMark::Declined { .. })) => None,
    EntryKind::McpTool(call) if call.tool.contains("memory") => Some(ToolKind::Memory),
    EntryKind::McpTool(_) => Some(ToolKind::Mcp),
    EntryKind::Reasoning(_) | EntryKind::Usage(_) | EntryKind::Raw(_) => Some(ToolKind::Note),
    EntryKind::Message(_)
    | EntryKind::Error(_)
    | EntryKind::NotSignedIn(_)
    | EntryKind::Problem(_)
    | EntryKind::McpServer(..)
    | EntryKind::ExemptionNotApplied { .. } => None,
  }
}

/// A command's kind by what it runs: a Claude Code tool by its name, a shell command by its program,
/// looked for inside the shell a CLI wrapped it in.
fn command_kind(command: &str) -> ToolKind {
  let tool = command.split_whitespace().next().unwrap_or_default();
  if READ_TOOLS.contains(&tool) {
    return ToolKind::Read;
  }
  if SEARCH_TOOLS.contains(&tool) {
    return ToolKind::Search;
  }
  let inner = unwrapped(command);
  let mut words = inner.split_whitespace();
  let program = program_name(words.next().unwrap_or_default());
  if program == "git" && words.next() == Some("grep") {
    return ToolKind::Search;
  }
  if READERS.contains(&program.as_str()) || (program == "sed" && inner.contains(" -n ")) {
    return ToolKind::Read;
  }
  if SEARCHERS.contains(&program.as_str()) {
    return ToolKind::Search;
  }
  ToolKind::Command
}

/// The command a shell runs, when `command` starts its shell (`"…\pwsh.exe" -Command "pnpm test"`,
/// `bash -lc 'cargo test'`); `command` itself otherwise.
pub fn unwrapped(command: &str) -> &str {
  let command = command.trim();
  let (first, rest) = match command.strip_prefix('"') {
    Some(quoted) => match quoted.split_once('"') {
      Some((program, rest)) => (program, rest),
      None => return command,
    },
    None => command.split_once(char::is_whitespace).unwrap_or((command, "")),
  };
  if !SHELLS.contains(&program_name(first).as_str()) {
    return command;
  }
  let rest = rest.trim_start();
  let Some((option, inner)) = rest.split_once(char::is_whitespace) else {
    return command;
  };
  if !SHELL_OPTIONS.contains(&option.to_ascii_lowercase().as_str()) {
    return command;
  }
  let inner = inner.trim();
  inner
    .strip_prefix('"')
    .and_then(|inner| inner.strip_suffix('"'))
    .or_else(|| inner.strip_prefix('\'').and_then(|inner| inner.strip_suffix('\'')))
    .unwrap_or(inner)
}

/// A program's name without its folder and `.exe`, in lower case.
fn program_name(program: &str) -> String {
  let name = program.rsplit(['/', '\\']).next().unwrap_or(program);
  let name = name.strip_suffix(".exe").unwrap_or(name);
  name.to_ascii_lowercase()
}

/// A consecutive run of tool events of one step execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolGroup {
  pub entries: Vec<Entry>,
  pub summary: GroupSummary,
}

/// What a group's header says of it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GroupSummary {
  pub reads: usize,
  pub searches: usize,
  pub memory: bool,
  /// The commands it ran, as many as it names.
  pub named: Vec<Arc<str>>,
  pub commands: usize,
  /// Files it edited, each once.
  pub edits: usize,
  pub mcp: usize,
  /// Its tool calls: everything but reasoning, usage and unparsed lines.
  pub calls: usize,
  /// The first command that failed, and its exit code.
  pub failed: Option<(Arc<str>, Option<i64>)>,
  /// The command still running at its end, while it grows.
  pub running: Option<Arc<str>>,
  pub first: Option<Timestamp>,
  pub last: Option<Timestamp>,
}

impl ToolGroup {
  pub fn new(entries: Vec<Entry>) -> Self {
    let summary = GroupSummary::of(&entries);
    Self { entries, summary }
  }
}

impl GroupSummary {
  /// What `entries`, tool events in order, add up to.
  pub fn of(entries: &[Entry]) -> Self {
    let mut summary = Self {
      first: entries.first().map(|entry| entry.at),
      last: entries.last().map(|entry| entry.at),
      ..Self::default()
    };
    let mut edited = BTreeSet::new();
    for entry in entries {
      let Some(kind) = tool_kind(entry) else {
        continue;
      };
      summary.calls += usize::from(kind != ToolKind::Note);
      match kind {
        ToolKind::Read => summary.reads += 1,
        ToolKind::Search => summary.searches += 1,
        ToolKind::Memory => summary.memory = true,
        ToolKind::Command => summary.command(entry),
        ToolKind::Edit => {
          if let EntryKind::FileChange(change) = &entry.kind {
            edited.insert(change.path.clone());
          }
        }
        ToolKind::Mcp => summary.mcp += 1,
        ToolKind::Note => {}
      }
    }
    summary.edits = edited.len();
    summary.running = entries.last().and_then(running_command);
    summary
  }

  fn command(&mut self, entry: &Entry) {
    let Some((command, failed, exit)) = command_of(entry) else {
      return;
    };
    self.commands += 1;
    let named: Arc<str> = unwrapped(&command).lines().next().unwrap_or_default().into();
    if self.named.len() < NAMED_COMMANDS {
      self.named.push(named.clone());
    }
    if failed && self.failed.is_none() {
      self.failed = Some((named, exit));
    }
  }

  /// Counts of `entries` by kind, for what a group leaves out: reads, searches, commands, edits and
  /// MCP calls, in that order.
  pub fn kinds(entries: &[Entry]) -> [usize; 5] {
    let mut kinds = [0; 5];
    for entry in entries {
      let slot = match tool_kind(entry) {
        Some(ToolKind::Read) => 0,
        Some(ToolKind::Search | ToolKind::Memory) => 1,
        Some(ToolKind::Command) => 2,
        Some(ToolKind::Edit) => 3,
        Some(ToolKind::Mcp) => 4,
        Some(ToolKind::Note) | None => continue,
      };
      kinds[slot] += 1;
    }
    kinds
  }
}

/// A command entry's command, whether it failed, and its exit code.
fn command_of(entry: &Entry) -> Option<(Arc<str>, bool, Option<i64>)> {
  match &entry.kind {
    EntryKind::Command(command) => Some((
      command.command.clone(),
      command.status == Some(CommandStatus::Failed),
      command.exit_code,
    )),
    EntryKind::ToolCommand(command) => Some((
      command.command.clone(),
      matches!(
        command.result,
        Some(ToolCommandResult::Failed | ToolCommandResult::TimedOut)
      ),
      command.exit_code,
    )),
    _ => None,
  }
}

/// The command `entry` still runs.
fn running_command(entry: &Entry) -> Option<Arc<str>> {
  let running = match &entry.kind {
    EntryKind::Command(command) if command.status.is_none() => &command.command,
    EntryKind::ToolCommand(command) if command.result.is_none() => &command.command,
    _ => return None,
  };
  Some(unwrapped(running).lines().next().unwrap_or_default().into())
}
