//! Edits an agent CLI makes through Orchester's file tools, as the runner records them: Claude Code's call of
//! `mcp__Orchester__edit_file` and Codex's call of `write_file` of the MCP server `Orchester` read as the native
//! agent's rows for those tools, and open to the change each made; a failed, denied or unfinished call shows no
//! change; an empty write says so and does not open; an edit that replaces every occurrence says so instead of counts;
//! the change each call reports after it is not shown again.

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, FileChange, FileChangeKind, MAX_CHANGE_LINES},
  orchester_tools::OrchesterTool,
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use crate::{github::shows, harness::Desktop, native_run::native_run, unreadable_run::at};

const EDIT: &str = "mcp__Orchester__edit_file README.md";
const UNREAD: &str = "mcp__Orchester__edit_file src/lib.rs";
const NOT_READ: &str = "src/lib.rs has not been read: read it first";
const DENIED: &str = "mcp__Orchester__edit_file secret.rs";
/// Lengths of lines whose width in Geist Mono is a whole number of pixels.
const WIDTHS: [usize; 4] = [5, 10, 15, 40];

/// What call `item` of `tool` with `input` asked, settled as the runner stores it.
fn told(item: &str, tool: OrchesterTool, input: &Value) -> AgentEvent {
  let mut event = AgentEvent::NativeCall {
    item: item.to_owned(),
    call: tool.shown_call(input).expect("input the tool takes"),
  };
  event.settle();
  event
}

fn edit_input(path: &str, old: &str, new: &str, replace_all: bool) -> Value {
  json!({"path": path, "edits": [{"old": old, "new": new, "replace_all": replace_all}]})
}

fn started(item: &str, command: &str) -> AgentEvent {
  AgentEvent::CommandStarted {
    item: item.to_owned(),
    command: command.to_owned(),
  }
}

fn finished(item: &str, command: &str, output: &str, status: CommandStatus) -> AgentEvent {
  AgentEvent::CommandFinished {
    item: item.to_owned(),
    command: command.to_owned(),
    output: output.to_owned(),
    exit_code: None,
    status,
  }
}

fn changed(item: &str, path: &str) -> AgentEvent {
  AgentEvent::FileChanged {
    item: item.to_owned(),
    changes: vec![FileChange {
      path: path.to_owned(),
      kind: FileChangeKind::Modified,
    }],
  }
}

/// A Claude Code call of `tool` that ran to `status`, its start at sequence `first`.
fn claude_call(
  item: &str,
  (tool, command): (OrchesterTool, &str),
  input: &Value,
  ended: (&str, CommandStatus),
) -> [AgentEvent; 3] {
  [
    started(item, command),
    told(item, tool, input),
    finished(item, command, ended.0, ended.1),
  ]
}

/// Claude Code's successful edit (sequences 1–4), its failed one (5–7), Codex's write (8–10), Claude Code's edit a
/// person denied (11–13), its empty write (14–16), its edit of every occurrence (17–19), its one-line write (20–22) and
/// its edit that never ended (23–24), its write one line over what a change keeps (25–27) and its edit that only
/// removes the final line break (28–30). The rest is in [`later_calls`].
fn transcript() -> Vec<AgentEvent> {
  let edit = edit_input("README.md", "# Orchester\nhi\n", "# Orchester\nhi there\n", false);
  let done = ("done", CommandStatus::Completed);
  let mut events = Vec::new();
  events.extend(claude_call("toolu_edit", (OrchesterTool::EditFile, EDIT), &edit, done));
  events.push(changed("toolu_edit", "README.md"));
  events.extend(claude_call(
    "toolu_unread",
    (OrchesterTool::EditFile, UNREAD),
    &edit_input("src/lib.rs", "a", "b", false),
    (NOT_READ, CommandStatus::Failed),
  ));
  events.extend([
    told(
      "mcp-1",
      OrchesterTool::WriteFile,
      &write("docs/notes.md", "first note\nsecond note\n"),
    ),
    AgentEvent::McpToolFinished {
      item: "mcp-1".to_owned(),
      server: "Orchester".to_owned(),
      tool: "write_file".to_owned(),
      status: CommandStatus::Completed,
      asked: false,
    },
    changed("mcp-1", "docs/notes.md"),
    started("toolu_denied", DENIED),
    told(
      "toolu_denied",
      OrchesterTool::EditFile,
      &edit_input("secret.rs", "a", "b", false),
    ),
    AgentEvent::PermissionDenied {
      item: "toolu_denied".to_owned(),
      command: DENIED.to_owned(),
    },
  ]);
  events.extend(later_calls());
  events
}

fn write(path: &str, content: &str) -> Value {
  json!({"path": path, "content": content})
}

/// The calls after the denied one, from sequence 14: see [`transcript`]; then an edit whose lines are 5, 10, 15 and 40
/// characters long (31–33), and Codex's search in `src` (34–35).
fn later_calls() -> Vec<AgentEvent> {
  let done = ("done", CommandStatus::Completed);
  let mut events = Vec::new();
  let empty = "mcp__Orchester__write_file empty.txt";
  events.extend(claude_call(
    "toolu_empty",
    (OrchesterTool::WriteFile, empty),
    &write("empty.txt", ""),
    done,
  ));
  let every = "mcp__Orchester__edit_file every.rs";
  let replace = edit_input("every.rs", "use a;", "use b;", true);
  events.extend(claude_call(
    "toolu_every",
    (OrchesterTool::EditFile, every),
    &replace,
    done,
  ));
  let one = "mcp__Orchester__write_file one.txt";
  events.extend(claude_call(
    "toolu_one",
    (OrchesterTool::WriteFile, one),
    &write("one.txt", "only\n"),
    done,
  ));
  let running = "mcp__Orchester__edit_file running.rs";
  events.push(started("toolu_running", running));
  events.push(told(
    "toolu_running",
    OrchesterTool::EditFile,
    &edit_input("running.rs", "a", "b", false),
  ));
  let long: String = (0..=MAX_CHANGE_LINES).map(|line| format!("line {line}\n")).collect();
  let wide = "mcp__Orchester__write_file long.txt";
  events.extend(claude_call(
    "toolu_long",
    (OrchesterTool::WriteFile, wide),
    &write("long.txt", &long),
    done,
  ));
  let unbroken = "mcp__Orchester__edit_file end.txt";
  let input = edit_input("end.txt", "last\n", "last", false);
  events.extend(claude_call(
    "toolu_end",
    (OrchesterTool::EditFile, unbroken),
    &input,
    done,
  ));
  let widths = "mcp__Orchester__edit_file widths.txt";
  let lines = WIDTHS.map(|width| "w".repeat(width)).join("\n");
  let input = edit_input("widths.txt", "a\n", &lines, false);
  events.extend(claude_call(
    "toolu_widths",
    (OrchesterTool::EditFile, widths),
    &input,
    done,
  ));
  let search = json!({"pattern": "useKeymap", "path": "src", "glob": null, "case_insensitive": null, "context": null});
  events.push(told("mcp-search", OrchesterTool::Search, &search));
  events.push(AgentEvent::McpToolFinished {
    item: "mcp-search".to_owned(),
    server: "Orchester".to_owned(),
    tool: "search".to_owned(),
    status: CommandStatus::Completed,
    asked: false,
  });
  events
}

fn seeded() -> Desktop {
  Desktop::with_seeded_project(|repo| {
    let (_, invocation) = native_run(repo);
    let mut db = ProjectDb::open(repo).expect("open the project");
    let events: Vec<(Timestamp, AgentEvent)> = transcript()
      .into_iter()
      .zip(13..)
      .map(|(event, second)| (at(second * 1_000), event))
      .collect();
    db.append_activity(&invocation, &events).expect("transcript");
  })
}

/// The Conversation tab's *Tools* view, where each call is a row of its own.
fn tools_view(desktop: &mut Desktop) {
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-page");
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-show-2");
  desktop.click("conversation-show-2");
  desktop.wait_until("the tools", |desktop| {
    shows(desktop, "conversation", "edit_file · M · +1 −1")
  });
}

#[test]
fn cli_edit_rows_open_to_the_change_each_call_made() {
  let mut desktop = seeded();
  tools_view(&mut desktop);
  assert!(shows(&mut desktop, "conversation", "README.md"));
  assert!(shows(&mut desktop, "conversation", "write_file · 2 lines written"));
  assert!(shows(&mut desktop, "conversation", "docs/notes.md"));
  for change in ["activity-entry-change-1", "activity-entry-change-9"] {
    assert!(
      desktop.tree.get_element_by_id(change).is_none(),
      "a row opens to its change"
    );
  }

  desktop.click("activity-entry-toggle-1");
  desktop.wait_for("activity-entry-change-1");
  assert!(shows(&mut desktop, "activity-entry-change-1", "hi there"));
  assert!(shows(&mut desktop, "activity-entry-change-1", "# Orchester"));

  desktop.click("activity-entry-toggle-9");
  desktop.wait_for("activity-entry-change-9");
  assert!(shows(&mut desktop, "activity-entry-change-9", "second note"));
}

#[test]
fn cli_edit_rows_show_a_failed_call_s_error_and_no_change_twice() {
  let mut desktop = seeded();
  tools_view(&mut desktop);
  // A failed call starts open, as the command it is, with the error it got.
  assert!(shows(&mut desktop, "conversation", UNREAD));
  assert!(shows(&mut desktop, "conversation", NOT_READ));
  assert!(desktop.tree.get_element_by_id("activity-entry-change-5").is_none());
  for path in ["README.md", "docs/notes.md"] {
    assert!(
      desktop
        .tree
        .get_element_by_id(&format!("activity-open-file:{path}"))
        .is_none(),
      "the change of {path} shows in its call's row only"
    );
  }
}

#[test]
fn cli_edit_rows_show_no_change_of_a_denied_or_unfinished_call() {
  let mut desktop = seeded();
  tools_view(&mut desktop);
  // Denied: the command row it was, open as a failure, without a change.
  assert!(shows(&mut desktop, "conversation", DENIED));
  assert!(desktop.tree.get_element_by_id("activity-entry-change-11").is_none());
  // Still running when the turn ended: nothing to open.
  assert!(shows(&mut desktop, "conversation", "edit_file · running"));
  assert!(desktop.tree.get_element_by_id("activity-entry-toggle-23").is_none());
  assert!(desktop.tree.get_element_by_id("activity-entry-change-23").is_none());
}

#[test]
fn cli_edit_rows_say_an_empty_write_one_line_and_every_occurrence() {
  let mut desktop = seeded();
  tools_view(&mut desktop);
  assert!(shows(&mut desktop, "conversation", "write_file · empty file written"));
  assert!(
    desktop.tree.get_element_by_id("activity-entry-toggle-14").is_none(),
    "an empty write has nothing to open"
  );
  assert!(shows(&mut desktop, "conversation", "write_file · 1 line written"));
  assert!(shows(
    &mut desktop,
    "conversation",
    "edit_file · M · replaces every occurrence"
  ));
  desktop.click("activity-entry-toggle-17");
  desktop.wait_for("activity-entry-change-17");
  assert!(shows(
    &mut desktop,
    "activity-entry-change-17",
    "Replaces every occurrence; the lines show one"
  ));
  assert!(shows(&mut desktop, "activity-entry-change-17", "use b;"));
}

#[test]
fn cli_edit_rows_count_one_line_left_out_and_say_a_final_line_break() {
  let mut desktop = seeded();
  tools_view(&mut desktop);
  desktop.click("activity-entry-toggle-25");
  desktop.wait_for("activity-entry-change-25");
  assert!(shows(
    &mut desktop,
    "activity-entry-change-25",
    "1 more line of the change is not shown"
  ));
  desktop.click("activity-entry-toggle-28");
  desktop.wait_for("activity-entry-change-28");
  assert!(shows(
    &mut desktop,
    "activity-entry-change-28",
    "Removes the final line break"
  ));
}

/// A line whose width is a whole number of pixels stays on one line: its code box spans the row (as the one-letter
/// line's does) instead of being exactly as wide as its text, which a fractional display scale can wrap.
#[test]
fn cli_edit_rows_keep_each_line_of_a_change_on_one_line() {
  let mut desktop = seeded();
  tools_view(&mut desktop);
  desktop.click("activity-entry-toggle-31");
  desktop.wait_for("activity-entry-change-31");
  let one = desktop.bounds("activity-change-text-31-0");
  for (at, width) in WIDTHS.into_iter().enumerate() {
    let line = desktop.bounds(&format!("activity-change-text-31-{}", at + 1));
    assert!(
      (line.width - one.width).abs() < 0.5 && line.width > 300.0,
      "{width} characters: {} wide, the row's code {}",
      line.width,
      one.width
    );
    assert!(
      (line.height - one.height).abs() < 0.5,
      "{width} characters stay on one line"
    );
  }
}

#[test]
fn cli_edit_rows_name_where_a_codex_search_looks() {
  let mut desktop = seeded();
  tools_view(&mut desktop);
  assert!(shows(&mut desktop, "conversation", "“useKeymap”"));
  assert!(shows(&mut desktop, "conversation", "search · src"));
}
