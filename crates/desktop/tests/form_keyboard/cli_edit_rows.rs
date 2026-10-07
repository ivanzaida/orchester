//! Edits an agent CLI makes through Orchester's file tools, as the runner records them: Claude Code's call of
//! `mcp__Orchester__edit_file` and Codex's call of `write_file` of the MCP server `Orchester` read as the native
//! agent's rows for those tools, and open to the change each made; a failed call shows its error and no change; the
//! change each call reports after it is not shown again.

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, FileChange, FileChangeKind, NativeCall},
  orchester_tools::OrchesterTool,
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::json;

use crate::{github::shows, harness::Desktop, native_run::native_run, unreadable_run::at};

const EDIT: &str = "mcp__Orchester__edit_file README.md";
const UNREAD: &str = "mcp__Orchester__edit_file src/lib.rs";
const NOT_READ: &str = "src/lib.rs has not been read: read it first";

fn told(item: &str, call: &NativeCall) -> AgentEvent {
  AgentEvent::NativeCall {
    item: item.to_owned(),
    call: call.clone(),
  }
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

/// Claude Code's successful edit (sequences 1–4), its failed one (5–7), then Codex's write (8–10).
fn transcript() -> Vec<AgentEvent> {
  let edit = OrchesterTool::EditFile
    .shown_call(&json!({"path": "README.md", "edits": [
      {"old": "# Orchester\nhi\n", "new": "# Orchester\nhi there\n", "replace_all": null}
    ]}))
    .expect("an edit's input");
  let unread = OrchesterTool::EditFile
    .shown_call(&json!({"path": "src/lib.rs", "edits": [{"old": "a", "new": "b", "replace_all": null}]}))
    .expect("an edit's input");
  let write = OrchesterTool::WriteFile
    .shown_call(&json!({"path": "docs/notes.md", "content": "first note\nsecond note\n"}))
    .expect("a write's input");
  vec![
    started("toolu_edit", EDIT),
    told("toolu_edit", &edit),
    finished(
      "toolu_edit",
      EDIT,
      "edited README.md: 1 edit(s) applied",
      CommandStatus::Completed,
    ),
    changed("toolu_edit", "README.md"),
    started("toolu_unread", UNREAD),
    told("toolu_unread", &unread),
    finished("toolu_unread", UNREAD, NOT_READ, CommandStatus::Failed),
    told("mcp-1", &write),
    AgentEvent::McpToolFinished {
      item: "mcp-1".to_owned(),
      server: "Orchester".to_owned(),
      tool: "write_file".to_owned(),
      status: CommandStatus::Completed,
      asked: false,
    },
    changed("mcp-1", "docs/notes.md"),
  ]
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
