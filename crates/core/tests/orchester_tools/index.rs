//! The code index as Orchester's tools read it: `find_symbol` ranked as ORC-48 splits the tables, signatures
//! from the outlines, hits in changed files marked; the file tools read into the native agent's inputs and
//! offered by the step's access; `find_symbol` offered only with the index.

use std::{collections::HashSet, fs};

use orchester_core::orchester_tools::{CodeIndex, FindSymbol, OrchesterTool, StepTools, ToolCallError, shown_hits};
use serde_json::{Value, json};

use crate::{parse, reading, writing};

fn query(value: &Value) -> FindSymbol {
  serde_json::from_value(value.clone()).expect("a valid query")
}

fn index() -> (tempfile::TempDir, CodeIndex) {
  let folder = tempfile::tempdir().expect("a folder");
  let root = folder.path();
  fs::write(
    root.join("symbols.tsv"),
    "Config\tsrc/config.rs\t3\tstruct\t\nconfig_path\tsrc/config.rs\t9\tfunction\t\nload_config\tsrc/load.rs\t1\tfunction\t\n\
     config\tsrc/lib.rs\t1\tmodule\t\nparse\tsrc/config.rs\t12\tmethod\timpl Config\n",
  )
  .expect("symbols");
  fs::write(
    root.join("symbols-consts.tsv"),
    "CONFIG_LIMIT\tsrc/config.rs\t1\tconst\t\n",
  )
  .expect("consts");
  fs::write(
    root.join("symbols-tests.tsv"),
    "config_loads\ttests/config.rs\t4\tfunction\t\n",
  )
  .expect("tests");
  fs::create_dir_all(root.join("outline").join("src")).expect("outlines");
  fs::write(
    root.join("outline").join("src").join("config.rs.md"),
    "- L3-8 pub struct Config\n  - L12-20 pub fn parse(text: &str) -> Config\n",
  )
  .expect("an outline");
  fs::write(
    root.join("tree.txt"),
    "src/config.rs\trust\t20\nsrc/lib.rs\trust\t3\nREADME.md\tmarkdown\t1\n",
  )
  .expect("tree");
  let index = CodeIndex::new(root);
  (folder, index)
}

#[test]
fn orchester_tools_find_symbol_ranks_as_the_index_splits_its_tables() {
  let (_folder, index) = index();
  let names = |hits: Vec<orchester_core::orchester_tools::SymbolHit>| -> Vec<String> {
    hits.into_iter().map(|hit| hit.name).collect()
  };
  assert_eq!(
    names(index.find(&query(&json!({"name": "Config"})), false).expect("found")),
    ["Config", "config", "config_path", "load_config"]
  );
  assert_eq!(
    names(
      index
        .find(
          &query(&json!({"name": "config", "include_consts": true, "include_tests": true})),
          false
        )
        .expect("found")
    ),
    [
      "config",
      "Config",
      "config_path",
      "load_config",
      "CONFIG_LIMIT",
      "config_loads"
    ]
  );
  assert_eq!(
    names(
      index
        .find(&query(&json!({"name": "config", "kind": "function"})), false)
        .expect("found")
    ),
    ["config_path", "load_config"]
  );
  assert_eq!(
    names(index.find(&FindSymbol::definitions_of("Config"), true).expect("found")),
    ["Config"]
  );
  let hits = index.find(&query(&json!({"name": "parse"})), false).expect("found");
  assert_eq!(hits[0].signature.as_deref(), Some("pub fn parse(text: &str) -> Config"));
  let changed = HashSet::from(["src/config.rs".to_owned()]);
  assert_eq!(
    shown_hits(&hits, &changed),
    "src/config.rs:12 method parse (in impl Config) [changed since the index: the line may have moved]\n    \
     pub fn parse(text: &str) -> Config\n"
  );
  assert_eq!(
    index.files().expect("files"),
    ["src/config.rs", "src/lib.rs", "README.md"]
  );
}

#[test]
fn orchester_tools_file_tools_follow_the_step() {
  for name in ["read_file", "list_files", "search_file", "edit_file", "create_file"] {
    assert!(
      writing()
        .policy(OrchesterTool::from_name(name).expect("a tool"))
        .is_some(),
      "{name}"
    );
  }
  let read = parse("read_file", &json!({"filename": "src/lib.rs"})).expect("valid");
  assert_eq!(read.tool(), OrchesterTool::ReadFile);
  assert!(matches!(
    parse("read_file", &json!({"filename": "src/lib.rs", "lines": 3})),
    Err(ToolCallError::Invalid { .. })
  ));
  for name in ["edit_file", "create_file"] {
    let tool = OrchesterTool::from_name(name).expect("a tool");
    assert_eq!(reading().policy(tool), None, "{name}");
    assert_eq!(
      orchester_core::orchester_tools::ToolCall::parse(name, &json!({"filename": "x", "content": "y"}), &reading()),
      Err(ToolCallError::NotOffered(tool))
    );
  }
  assert_eq!(reading().policy(OrchesterTool::FindSymbol), None);
  let indexed = reading().with_code_index();
  assert!(
    indexed
      .claude_allow_rules()
      .contains(&"mcp__Orchester__find_symbol".to_owned())
  );
  assert_eq!(
    indexed.prompt_paragraph(true).lines().last(),
    Some(
      "Use the shell for builds, tests and git, not to read or search files. Read and search files outside the \
       worktree with mcp__Orchester__read_external_file and mcp__Orchester__search_external_file of the Orchester \
       MCP server: in the run's sources they run at once, elsewhere a person is asked first. The server also has \
       mcp__Orchester__find_symbol, mcp__Orchester__find_references, mcp__Orchester__list_run_processes, \
       mcp__Orchester__stop_run_process, mcp__Orchester__get_task, \
       mcp__Orchester__save_memory, mcp__Orchester__report_progress. Find apps you started with \
       mcp__Orchester__list_run_processes and stop them with mcp__Orchester__stop_run_process(pid), instead of \
       Stop-Process or kill. They run without asking; executables outside this run\'s worktree, the runner and the \
       agent CLI are refused. Stop only apps you started."
    )
  );
  let listed = StepTools::for_step(
    &crate::permissions(orchester_core::resource::RepositoryAccess::Write),
    &[],
  )
  .mcp_tools();
  let edit = listed.iter().find(|tool| tool["name"] == "edit_file").expect("listed");
  assert_eq!(
    edit["inputSchema"]["required"],
    json!(["filename", "start_line", "end_line", "expected", "content"])
  );
}

/// Paths from the index are plain relative paths or nothing: a symbol or file entry that leaves the index or
/// the worktree is dropped before anything reads it, so no outline outside the index is read.
#[test]
fn orchester_tools_index_drops_crafted_paths() {
  for (path, plain) in [
    ("src/lib.rs", true),
    ("../../loot", false),
    (r"x\..\..\secret.txt", false),
    ("/etc/passwd", false),
    ("C:secret.txt", false),
    ("src//lib.rs", false),
    ("./src/lib.rs", false),
    ("src/\u{7}bell.rs", false),
  ] {
    assert_eq!(orchester_core::orchester_tools::plain_relative(path), plain, "{path}");
  }
  let (folder, index) = index();
  let root = folder.path();
  fs::write(
    root.join("symbols.tsv"),
    "evil\t../../loot\t7\tfunction\t\nevil\tsrc/evil.rs\t2\tfunction\t\n",
  )
  .expect("symbols");
  fs::write(
    root.join("..").join("loot.md"),
    "- L7-9 a line from outside the index\n",
  )
  .expect("loot");
  let hits = index.find(&query(&json!({"name": "evil"})), false).expect("found");
  assert_eq!(hits.len(), 1);
  assert_eq!(hits[0].path, "src/evil.rs");
  fs::write(
    root.join("tree.txt"),
    "src/a.rs\trust\t1\n../../secret.txt\ttext\t1\nx\\..\\y\ttext\t1\n",
  )
  .expect("tree");
  assert_eq!(index.files().expect("files"), ["src/a.rs"]);
}

#[test]
fn orchester_tools_find_symbol_needs_a_name_and_finds_consts_by_kind() {
  assert!(matches!(
    parse("find_symbol", &json!({"name": "  "})),
    Err(ToolCallError::NotOffered(_) | ToolCallError::Invalid { .. })
  ));
  let indexed = writing().with_code_index();
  assert!(matches!(
    orchester_core::orchester_tools::ToolCall::parse("find_symbol", &json!({"name": " "}), &indexed),
    Err(ToolCallError::Invalid { .. })
  ));
  let (_folder, index) = index();
  let hits = index
    .find(&query(&json!({"name": "limit", "kind": "const"})), false)
    .expect("found");
  assert_eq!(
    hits.iter().map(|hit| hit.name.as_str()).collect::<Vec<_>>(),
    ["CONFIG_LIMIT"]
  );
}

/// Every answer stays below Claude Code's MCP output limit, with a note on how to narrow it.
#[test]
fn orchester_tools_answers_stay_within_the_mcp_limit() {
  use orchester_core::orchester_tools::{MAX_ANSWER_BYTES, ToolResult};
  let answer = ToolResult::of_file_tool("z".repeat(500_000), false).mcp();
  let text = answer["content"][0]["text"].as_str().expect("text");
  assert!(text.len() <= MAX_ANSWER_BYTES, "{}", text.len());
  assert!(
    text.ends_with("narrow the call with start_line and length, limit_files, a longer term or a path]"),
    "{}",
    &text[text.len() - 120..]
  );
  let small = ToolResult::of_file_tool("small".to_owned(), false).mcp();
  assert_eq!(small["content"][0]["text"], "small");
}
