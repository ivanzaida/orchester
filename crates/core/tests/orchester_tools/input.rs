//! Strict input, and the git program of each call with every value one argument of its own.

use std::{path::Path, time::Duration};

use orchester_core::orchester_tools::{GitCall, OrchesterTool, ToolCall, ToolCallError};
use serde_json::{Value, json};

use crate::{Folders, context, parse, program_of, reading};

#[test]
fn orchester_tools_read_their_input_strictly() {
  assert_eq!(
    parse("git_diff", &json!({"path": "src/main.rs", "staged": true})).map(|call| call.tool()),
    Ok(OrchesterTool::GitDiff)
  );
  assert_eq!(
    parse("git_status", &Value::Null).map(|call| call.tool()),
    Ok(OrchesterTool::GitStatus)
  );
  assert_eq!(
    parse("git_log", &json!({"path": null, "count": null})),
    Ok(ToolCall::Git(GitCall::Log { path: None, count: 20 }))
  );
  for (name, arguments) in [
    ("git_status", json!({"verbose": true})),
    ("git_diff", json!({"path": 3})),
    ("git_log", json!({"count": 0})),
    ("git_log", json!({"count": 201})),
    ("git_show", json!({})),
    ("git_add", json!({"paths": []})),
    ("git_commit", json!({"message": "  "})),
    ("read_outside_file", json!({"path": "/x", "limit": 10_001})),
  ] {
    assert!(
      matches!(parse(name, &arguments), Err(ToolCallError::Invalid { .. })),
      "{name} {arguments}"
    );
  }
  assert_eq!(
    parse("rm", &json!({})),
    Err(ToolCallError::UnknownTool("rm".to_owned()))
  );
  assert_eq!(
    ToolCall::parse("git_commit", &json!({"message": "x"}), &reading()),
    Err(ToolCallError::NotOffered(OrchesterTool::GitCommit))
  );
  assert_eq!(
    parse("run_project_tool", &json!({"name": "Run tests"})),
    Err(ToolCallError::NotOffered(OrchesterTool::RunProjectTool))
  );
}
/// Paths stay in the worktree and revisions are no options; everything else an agent writes is one
/// argument of its own, after the end of the options, and a commit message never is one at all.
#[test]
fn orchester_tools_keep_every_value_one_argument() {
  for path in ["../outside", "/etc/passwd", r"C:\Windows", "a/../../b", ":(top)x", ""] {
    assert!(parse("git_diff", &json!({ "path": path })).is_err(), "{path}");
  }
  for revision in ["--output=/tmp/x", "-p", "HEAD; rm -rf /", "HEAD rm", "HEAD:secret", ""] {
    assert!(
      parse("git_show", &json!({ "revision": revision })).is_err(),
      "{revision}"
    );
  }
  let folders = Folders::new();
  let reads = folders.reads();
  let context = context(&folders.worktree, &reads, &[]);
  let attempts = [
    "; rm -rf /",
    "$(whoami)",
    "`id`",
    "a \"quoted\" 'name'",
    "-rf",
    "--output=x",
  ];
  for attempt in attempts {
    let call = parse("git_diff", &json!({ "path": attempt })).expect("a relative path");
    let program = program_of(&call, &context);
    let args = program.command().args();
    assert_eq!(args.last().map(String::as_str), Some(attempt), "{args:?}");
    assert_eq!(args[args.len() - 2], "--", "{args:?}");
    assert!(args.contains(&"--literal-pathspecs".to_owned()));
  }
  let call = parse("git_add", &json!({"paths": attempts})).expect("relative paths");
  let program = program_of(&call, &context);
  let args = program.command().args();
  let end = args.iter().position(|arg| arg == "--").expect("the end of the options");
  assert_eq!(&args[end + 1..], attempts);
  let call = parse("git_show", &json!({"revision": "$(whoami)", "path": "src/a b.rs"})).expect("a revision");
  let args = program_of(&call, &context).command().args().to_vec();
  assert_eq!(args[args.len() - 2..], ["--end-of-options", "$(whoami):src/a b.rs"]);
  let message = "Fix; rm -rf / $(whoami) \"quoted\"\n\nBody";
  let call = parse("git_commit", &json!({ "message": message })).expect("a message");
  let program = program_of(&call, &context);
  assert_eq!(program.command().stdin(), Some(message));
  assert!(!program.command().args().iter().any(|arg| arg.contains("rm -rf")));
  // Hooks come from an absolute, empty folder the runner owns, never a folder relative to the worktree.
  let hooks = format!("core.hooksPath={}", Path::new("/orchester-private/no-hooks").display());
  assert!(
    program
      .command()
      .args()
      .windows(2)
      .any(|pair| pair == ["-c", hooks.as_str()])
  );
  assert!(
    program
      .command()
      .args()
      .ends_with(&["commit", "--no-verify", "-F", "-"].map(str::to_owned))
  );
}
#[test]
fn orchester_tools_git_runs_hardened_in_the_worktree_without_a_shell() {
  let folders = Folders::new();
  let reads = folders.reads();
  let context = context(&folders.worktree, &reads, &[]);
  let program = program_of(&parse("git_status", &json!({})).expect("valid"), &context);
  let command = program.command();
  assert_eq!(command.program(), Path::new("git"));
  assert_eq!(command.current_dir(), Some(folders.worktree.as_path()));
  assert_eq!(
    command.args(),
    [
      "-c",
      "core.fsmonitor=false",
      "-c",
      "safe.bareRepository=explicit",
      "--no-pager",
      "--no-optional-locks",
      &format!("--git-dir={}", Path::new("/repo/.git/worktrees/run").display()),
      &format!("--work-tree={}", folders.worktree.display()),
      "-c",
      &format!("core.hooksPath={}", Path::new("/orchester-private/no-hooks").display()),
      "-c",
      "log.showSignature=false",
      "-c",
      "submodule.recurse=false",
      "-c",
      "diff.ignoreSubmodules=all",
      "-c",
      "status.submoduleSummary=false",
      "status",
      "--short",
      "--branch",
      "--ignore-submodules=all"
    ]
  );
  assert!(!command.inherits_env());
  assert!(
    command
      .env()
      .iter()
      .any(|(name, value)| name == "GIT_TERMINAL_PROMPT" && value == "0")
  );
  assert_eq!(program.timeout(), Duration::from_secs(60));
}

/// `git_add` stages no repository of its own: neither one in the worktree nor a path inside one.
#[test]
fn orchester_tools_git_add_refuses_nested_repositories() {
  let folders = Folders::new();
  std::fs::create_dir_all(folders.worktree.join("vendor").join("lib").join(".git")).expect("a nested repository");
  std::fs::write(folders.worktree.join("vendor").join("lib").join("a.txt"), "a").expect("a file");
  let reads = folders.reads();
  let context = context(&folders.worktree, &reads, &[]);
  for path in ["vendor/lib", "vendor/lib/a.txt"] {
    let call = parse("git_add", &json!({ "paths": [path] })).expect("valid");
    assert_eq!(
      call.work(&context),
      Err(ToolCallError::NestedRepository(path.to_owned()))
    );
  }
  let call = parse("git_add", &json!({"paths": ["vendor/other.txt"]})).expect("valid");
  assert!(call.work(&context).is_ok());
}
