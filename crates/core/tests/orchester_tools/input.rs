//! Strict input: each tool's call is read into its type, unknown fields and bad values refused; the removed tools are
//! unknown; and how a call of a worktree file tool reads in the transcript.

use orchester_core::{
  connection::stream::FileChangeKind,
  native::tools::ToolName,
  orchester_tools::{FileCall, OrchesterTool, ToolCall, ToolCallError},
};
use serde_json::{Value, json};

use crate::{parse, reading};

fn edit(start: u32, end: u32, expected: &str, content: &str) -> Value {
  json!({"filename": "src/main.rs", "start_line": start, "end_line": end, "expected": expected, "content": content})
}

#[test]
fn orchester_tools_read_their_input_strictly() {
  let read = parse(
    "read_file",
    &json!({"filename": "src/main.rs", "start_line": 3, "length": null}),
  )
  .expect("valid");
  let ToolCall::File(file) = &read else {
    panic!("a file call");
  };
  let FileCall::Read(lines) = file.as_ref() else {
    panic!("a read");
  };
  assert_eq!(
    (lines.filename(), lines.start_line(), lines.length()),
    ("src/main.rs", 3, 2000)
  );
  for (name, arguments) in [
    ("search_file", json!({"term": "parse_config"})),
    ("search_file", json!({"term": "x", "limit_files": 200})),
    (
      "edit_file",
      edit(1, 1, "fn main() {", "fn main() -> Result<(), Error> {"),
    ),
    ("edit_file", edit(4, 3, "", "// added")),
    ("create_file", json!({"filename": "notes/new.md", "content": ""})),
    ("list_files", json!({"pattern": null, "path": null})),
    ("read_external_file", json!({"filename": "/deps/lib.rs"})),
    ("search_external_file", json!({"base_directory": "/deps", "term": "fn"})),
  ] {
    assert_eq!(
      parse(name, &arguments).map(|call| call.tool().as_str()),
      Ok(name),
      "{arguments}"
    );
  }
  for (name, arguments) in [
    ("read_file", json!({"filename": "  "})),
    ("read_file", json!({"path": "src/main.rs"})),
    ("read_file", json!({"filename": "a", "length": 10_001})),
    ("read_file", json!({"filename": "a", "start_line": 0})),
    ("search_file", json!({"term": ""})),
    ("search_file", json!({"term": "a\nb"})),
    ("search_file", json!({"term": "x", "limit_files": 201})),
    ("search_file", json!({"pattern": "x"})),
    (
      "edit_file",
      json!({"filename": "a", "start_line": 1, "end_line": 1, "content": "x"}),
    ),
    ("edit_file", edit(3, 1, "", "x")),
    ("edit_file", edit(3, 2, "not empty", "x")),
    ("edit_file", edit(1, 1, "same", "same")),
    (
      "edit_file",
      json!({"path": "a", "edits": [{"old": "a", "new": "b", "replace_all": null}]}),
    ),
    ("create_file", json!({"filename": "", "content": "x"})),
    ("create_file", json!({"filename": "a"})),
    ("read_external_file", json!({"path": "/deps/lib.rs"})),
    ("search_external_file", json!({"base_directory": "/deps", "term": " "})),
  ] {
    assert!(
      matches!(parse(name, &arguments), Err(ToolCallError::Invalid { .. })),
      "{name} {arguments}"
    );
  }
  assert_eq!(
    ToolCall::parse("edit_file", &edit(1, 1, "a", "b"), &reading()),
    Err(ToolCallError::NotOffered(OrchesterTool::EditFile))
  );
  assert_eq!(
    ToolCall::parse("create_file", &json!({"filename": "a", "content": ""}), &reading()),
    Err(ToolCallError::NotOffered(OrchesterTool::CreateFile))
  );
  assert_eq!(
    parse("run_project_tool", &json!({"name": "Run tests"})),
    Err(ToolCallError::NotOffered(OrchesterTool::RunProjectTool))
  );
}

/// Orchester's git tools, `write_file`, `read_outside_file` and the old `search` are gone: agents use git through their
/// shell, change files with `edit_file` and `create_file`, and read outside with `read_external_file`.
#[test]
fn orchester_tools_removed_tools_are_unknown() {
  for gone in [
    "git_status",
    "git_diff",
    "git_log",
    "git_show",
    "git_add",
    "git_commit",
    "write_file",
    "read_outside_file",
    "search",
  ] {
    assert_eq!(
      parse(gone, &json!({})),
      Err(ToolCallError::UnknownTool(gone.to_owned())),
      "{gone}"
    );
  }
}

/// A CLI's call of `edit_file` reads as the native agent's edit, its expected lines replaced by its content, and
/// changes the file it names; `create_file` reads as a write and adds its file; a refused input shows no call.
#[test]
fn orchester_tools_file_calls_read_as_native_rows() {
  let edited = OrchesterTool::EditFile
    .shown_call(&edit(2, 2, "old line", "new line"))
    .expect("an edit");
  assert_eq!(edited.tool, ToolName::EditFile);
  assert_eq!(edited.input.as_ref().map(|input| input.target()), Some("src/main.rs"));
  let change = OrchesterTool::EditFile
    .changed_file(&edit(2, 2, "old line", "new line"))
    .expect("a change");
  assert_eq!(
    (change.path.as_str(), change.kind),
    ("src/main.rs", FileChangeKind::Modified)
  );
  let created = json!({"filename": "notes/new.md", "content": "hello\n"});
  assert_eq!(
    OrchesterTool::CreateFile.shown_call(&created).map(|call| call.tool),
    Some(ToolName::WriteFile)
  );
  assert_eq!(
    OrchesterTool::CreateFile
      .changed_file(&created)
      .map(|change| change.kind),
    Some(FileChangeKind::Added)
  );
  let searched = OrchesterTool::SearchFile
    .shown_call(&json!({"term": "parse_config"}))
    .expect("a search");
  assert_eq!(searched.tool, ToolName::Search);
  assert_eq!(
    searched.input.as_ref().map(|input| input.target()),
    Some("parse_config")
  );
  assert!(OrchesterTool::EditFile.shown_call(&edit(3, 1, "", "x")).is_none());
  assert!(
    OrchesterTool::ReadExternalFile
      .shown_call(&json!({"filename": "/deps/lib.rs"}))
      .is_none()
  );
}

/// A link to `target` folder at `link`: a junction on Windows, which needs no privilege, a symbolic link on macOS.
pub(crate) fn link_folder(target: &std::path::Path, link: &std::path::Path) {
  #[cfg(windows)]
  {
    let made = std::process::Command::new("cmd")
      .arg("/C")
      .arg("mklink")
      .arg("/J")
      .arg(link)
      .arg(target)
      .output()
      .expect("cmd runs");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
  }
  #[cfg(not(windows))]
  std::os::unix::fs::symlink(target, link).expect("a link");
}
