//! Orchester's `read_file`, `edit_file` and `create_file` for agent CLIs: an edit needs a read of the file in this
//! invocation, refuses a file changed since that read and lines that are not what it expects, and says what they hold
//! now; a create refuses a file that exists; paths stay in the step's folders.

use std::fs;

use orchester_core::{
  connection::stream::{CallDetail, FileChangeKind},
  orchester_tools::{FileCall, LineEdit, NewFile, ReadLines, StepTools, ToolCall},
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
};
use serde_json::{Value, json};

use crate::{Layout, failed, ok, text};

/// The worktree file tool call of `name` with `input`, as Orchester's relay reads it in a writing step.
pub fn file_call(name: &str, input: &Value) -> FileCall {
  let permissions = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  );
  match ToolCall::parse(name, input, &StepTools::for_step(&permissions, &[])) {
    Ok(ToolCall::File(call)) => *call,
    other => panic!("{name}: {other:?}"),
  }
}

fn read(filename: &str) -> ReadLines {
  match file_call("read_file", &json!({ "filename": filename })) {
    FileCall::Read(read) => read,
    other => panic!("{other:?}"),
  }
}

fn edit(filename: &str, (start, end): (u32, u32), expected: &str, content: &str) -> LineEdit {
  let input = json!({"filename": filename, "start_line": start, "end_line": end, "expected": expected,
    "content": content});
  match file_call("edit_file", &input) {
    FileCall::Edit(edit) => edit,
    other => panic!("{other:?}"),
  }
}

fn create(filename: &str, content: &str) -> NewFile {
  match file_call("create_file", &json!({"filename": filename, "content": content})) {
    FileCall::Create(create) => create,
    other => panic!("{other:?}"),
  }
}

/// An edit needs a read in this invocation; then it changes exactly the expected lines, and a later edit needs no new
/// read of the file it wrote itself.
#[test]
fn files_line_edit_needs_a_current_read() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  let rename = edit(
    "src/main.rs",
    (2, 2),
    "    println!(\"hello\");",
    "    println!(\"world\");",
  );
  let unread = failed(tools.edit_lines(&rename));
  assert!(unread.contains("has not been read"), "{unread}");
  let lines = ok(tools.read_lines(&read("src/main.rs")));
  assert_eq!(
    lines,
    "     1\tfn main() {\n     2\t    println!(\"hello\");\n     3\t}\n"
  );
  let outcome = tools.edit_lines(&rename);
  assert_eq!(outcome.changes[0].kind, FileChangeKind::Modified);
  assert_eq!(
    outcome.detail,
    Some(CallDetail::Changed {
      change: FileChangeKind::Modified,
      added: 1,
      removed: 1
    })
  );
  assert!(ok(outcome).contains("the file has 3 lines now"));
  assert_eq!(layout.read("src/main.rs"), "fn main() {\n    println!(\"world\");\n}\n");
  ok(tools.edit_lines(&edit("src/main.rs", (3, 2), "", "    // done")));
  assert_eq!(
    layout.read("src/main.rs"),
    "fn main() {\n    println!(\"world\");\n    // done\n}\n"
  );
}

/// Lines that are not what the call expects change nothing, and the answer shows what they hold now; a file a command
/// changed since the read is refused the same way.
#[test]
fn files_line_edit_refuses_stale_lines_and_stale_reads() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  ok(tools.read_lines(&read("README.md")));
  let refused = failed(tools.edit_lines(&edit("README.md", (2, 2), "hello here", "hi")));
  assert!(
    refused.contains("not what `expected` says") && refused.contains("     2\thello there"),
    "{refused}"
  );
  assert_eq!(layout.read("README.md"), "# Readme\nhello there\n");
  let past = failed(tools.edit_lines(&edit("README.md", (2, 3), "hello there\nmore", "hi")));
  assert!(past.contains("the file has 2 lines"), "{past}");
  fs::write(layout.worktree.join("README.md"), "# Readme\nchanged by a command\n").expect("file");
  let stale = failed(tools.edit_lines(&edit("README.md", (2, 2), "hello there", "hi")));
  assert!(
    stale.contains("changed since you read it") && stale.contains("     2\tchanged by a command"),
    "{stale}"
  );
  assert_eq!(layout.read("README.md"), "# Readme\nchanged by a command\n");
  let missing = failed(tools.edit_lines(&edit("gone.md", (1, 1), "a", "b")));
  assert!(missing.contains("create_file"), "{missing}");
}

/// A new file is made with its folders; one that exists is refused, read or not.
#[test]
fn files_create_refuses_an_existing_file() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  let outcome = tools.create_file(&create("notes/deep/new.md", "one\ntwo\n"));
  assert_eq!(outcome.changes[0].kind, FileChangeKind::Added);
  assert!(ok(outcome).contains("created notes/deep/new.md"));
  assert_eq!(layout.read("notes/deep/new.md"), "one\ntwo\n");
  let again = failed(tools.create_file(&create("notes/deep/new.md", "x")));
  assert!(
    again.contains("already exists") && again.contains("edit_file"),
    "{again}"
  );
  ok(tools.read_lines(&read("README.md")));
  assert!(failed(tools.create_file(&create("README.md", "x"))).contains("already exists"));
  assert_eq!(layout.read("README.md"), "# Readme\nhello there\n");
  // An extra writable folder outside the worktree is written by its path.
  let cached = layout.cache.join("out.txt");
  ok(tools.create_file(&create(&text(&cached), "cached\n")));
  assert_eq!(fs::read_to_string(cached).expect("file"), "cached\n");
}

/// Paths out of the worktree, into `.git` or `.orchester`, and the read-only code index are refused.
#[test]
fn files_line_tools_stay_in_the_steps_folders() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  for path in ["../../outside/secret.txt", ".git/config", ".orchester/x.txt"] {
    let error = failed(tools.create_file(&create(path, "x")));
    assert!(
      error.contains("outside the folders") || error.contains("protected"),
      "{path}: {error}"
    );
  }
  let outside = text(&layout.root.join("outside").join("secret.txt"));
  assert!(failed(tools.read_lines(&read(&outside))).contains("outside the folders"));
  let index = text(&layout.index.join("symbols.tsv"));
  ok(tools.read_lines(&read(&index)));
  let error = failed(tools.edit_lines(&edit(&index, (1, 1), "hello\tsrc/nested/lib.rs\t1\tfunction\t", "x")));
  assert!(error.contains("protected"), "{error}");
}

/// A deletion removes one file; a move renames one to a path where no file is, making its folders, and the moved file
/// is read again before an edit; both stay in the step's folders.
#[test]
fn files_delete_and_move_stay_in_the_steps_folders() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  let deleted = tools.delete_file("README.md");
  assert_eq!(deleted.changes[0].kind, FileChangeKind::Deleted);
  ok(deleted);
  assert!(!layout.worktree.join("README.md").exists());
  assert!(failed(tools.delete_file("README.md")).contains("does not exist"));
  assert!(failed(tools.delete_file("src")).contains("folder"));
  ok(tools.read_lines(&read("src/main.rs")));
  let moved = tools.move_file("src/main.rs", "src/bin/main.rs");
  let kinds: Vec<(&str, FileChangeKind)> = moved
    .changes
    .iter()
    .map(|change| (change.path.as_str(), change.kind))
    .collect();
  assert_eq!(
    kinds,
    [
      ("src/main.rs", FileChangeKind::Deleted),
      ("src/bin/main.rs", FileChangeKind::Added)
    ]
  );
  ok(moved);
  assert!(layout.read("src/bin/main.rs").starts_with("fn main()"));
  let unread = failed(tools.edit_lines(&edit("src/bin/main.rs", (1, 1), "fn main() {", "fn run() {")));
  assert!(unread.contains("has not been read"), "{unread}");
  let taken = failed(tools.move_file("src/nested/lib.rs", "src/bin/main.rs"));
  assert!(taken.contains("already exists"), "{taken}");
  for (from, to) in [
    ("src/nested/lib.rs", "../../outside/lib.rs"),
    ("src/nested/lib.rs", ".git/lib.rs"),
    ("../../outside/secret.txt", "secret.txt"),
  ] {
    let error = failed(tools.move_file(from, to));
    assert!(
      error.contains("outside the folders") || error.contains("protected"),
      "{from} -> {to}: {error}"
    );
  }
  assert!(failed(tools.delete_file(".git")).contains("protected"));
  assert!(layout.root.join("outside").join("secret.txt").exists());
}
