//! Orchester's typed tools (`orchester_core::orchester_tools`): strict input, the program each git call
//! runs with every value one argument of its own, the policy table, reads outside the worktree that never
//! reach a credential location, answers cut to a size, and how each CLI is told about them.

use std::{
  fs,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  command::{CommandError, CommandOutput},
  connection::{
    claude_approval::{ApprovalToken, ClaudeApprovalScope, ClaudeAsk},
    invocation::{CodexMcp, CodexOrchester, Platform},
  },
  orchester_tools::{
    GitCall, OrchesterTool, OutsideReadError, OutsideReads, ProjectTool, StepTools, ToolCall, ToolCallError,
    ToolContext, ToolPolicy, ToolProgram, ToolWork, decide,
  },
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, category::CommandScope},
  resource::{CommandAction, CommandArg, CommandEnvironment, CommandProgram, OutputLimit, RepositoryAccess},
};
use serde_json::{Value, json};
use tempfile::TempDir;

fn permissions(repository: RepositoryAccess) -> AgentPermissions {
  AgentPermissions::new(
    repository,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  )
}

fn writing() -> StepTools {
  StepTools::for_step(&permissions(RepositoryAccess::Write), &[])
}

fn reading() -> StepTools {
  StepTools::for_step(&permissions(RepositoryAccess::ReadOnly), &[])
}

fn parse(name: &str, arguments: &Value) -> Result<ToolCall, ToolCallError> {
  ToolCall::parse(name, arguments, &writing())
}

/// Folders of one test: a worktree, a home folder with credentials, a temporary folder and a dependency's
/// sources outside them all.
struct Folders {
  _root: TempDir,
  worktree: PathBuf,
  home: PathBuf,
  dependency: PathBuf,
  private: PathBuf,
}

impl Folders {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("a temporary folder");
    let base = root.path().to_path_buf();
    let folder = |name: &str| {
      let path = base.join(name);
      fs::create_dir_all(&path).expect("a folder");
      path
    };
    let (worktree, home, dependency, private) = (folder("worktree"), folder("home"), folder("deps"), folder("private"));
    for (path, text) in [
      (home.join(".ssh").join("id_ed25519"), "secret key"),
      (
        home.join("AppData").join("Roaming").join("gh").join("hosts.yml"),
        "token",
      ),
      (home.join(".cargo").join("registry").join("lib.rs"), "fn cached() {}\n"),
      (
        dependency.join("src").join("lib.rs"),
        "line 1\nline 2\nline 3\nline 4\n",
      ),
      (
        dependency.join(".git").join("config"),
        "[remote]\nurl = https://user:token@host/x\n",
      ),
      (private.join("invocation.json"), "{\"token\":\"x\"}"),
    ] {
      fs::create_dir_all(path.parent().expect("a parent")).expect("a folder");
      fs::write(path, text).expect("a file");
    }
    fs::write(dependency.join("blob.bin"), b"a\0b\n").expect("a file");
    Self {
      _root: root,
      worktree,
      home,
      dependency,
      private,
    }
  }

  fn reads(&self) -> OutsideReads {
    let scope = CommandScope::new(&self.worktree, &[], Some(&self.home), None);
    OutsideReads::new(scope, std::slice::from_ref(&self.private))
  }
}

fn text(path: &Path) -> String {
  path.to_string_lossy().into_owned()
}

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
  for (platform, hooks) in [
    (Platform::Windows, "core.hooksPath=NUL"),
    (Platform::MacOs, "core.hooksPath=/dev/null"),
  ] {
    let program = program_of(&call, &ToolContext { platform, ..context });
    assert_eq!(program.command().stdin(), Some(message));
    assert!(!program.command().args().iter().any(|arg| arg.contains("rm -rf")));
    assert!(program.command().args().windows(2).any(|pair| pair == ["-c", hooks]));
    assert!(
      program
        .command()
        .args()
        .ends_with(&["commit", "--no-verify", "-F", "-"].map(str::to_owned))
    );
  }
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
      "status",
      "--short",
      "--branch"
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

/// | class | writing step | read-only step |: read-only tools run, worktree writes run only where the step
/// may write, and the tools that reach outside ask in both.
#[test]
fn orchester_tools_policy_follows_the_table() {
  let write = permissions(RepositoryAccess::Write);
  let read = permissions(RepositoryAccess::ReadOnly);
  for (tool, writing, reading) in [
    (OrchesterTool::GitStatus, Some(ToolPolicy::Runs), Some(ToolPolicy::Runs)),
    (OrchesterTool::GitDiff, Some(ToolPolicy::Runs), Some(ToolPolicy::Runs)),
    (OrchesterTool::GitLog, Some(ToolPolicy::Runs), Some(ToolPolicy::Runs)),
    (OrchesterTool::GitShow, Some(ToolPolicy::Runs), Some(ToolPolicy::Runs)),
    (OrchesterTool::GitAdd, Some(ToolPolicy::Runs), None),
    (OrchesterTool::GitCommit, Some(ToolPolicy::Runs), None),
    (
      OrchesterTool::ReadOutsideFile,
      Some(ToolPolicy::Asks),
      Some(ToolPolicy::Asks),
    ),
    (
      OrchesterTool::RunProjectTool,
      Some(ToolPolicy::Asks),
      Some(ToolPolicy::Asks),
    ),
  ] {
    assert_eq!(decide(tool, &write), writing, "{tool:?}");
    assert_eq!(decide(tool, &read), reading, "{tool:?}");
  }
  let mut asking = read;
  asking.set_interactive_tool_approvals(true);
  assert_eq!(StepTools::for_step(&asking, &[]), reading());
  let names = |tools: &StepTools| -> Vec<&str> { tools.offered().iter().map(|(tool, _)| tool.as_str()).collect() };
  assert_eq!(
    names(&reading()),
    ["git_status", "git_diff", "git_log", "git_show", "read_outside_file"]
  );
  let project = [project_tool("Run tests", "/bin/true")];
  let offered = StepTools::for_step(&write, &project);
  assert_eq!(offered.policy(OrchesterTool::RunProjectTool), Some(ToolPolicy::Asks));
  let listed = offered.mcp_tools();
  let run = listed
    .iter()
    .find(|tool| tool["name"] == "run_project_tool")
    .expect("listed");
  assert_eq!(run["inputSchema"]["properties"]["name"]["enum"], json!(["Run tests"]));
  assert_eq!(run["inputSchema"]["additionalProperties"], json!(false));
  assert!(
    listed
      .iter()
      .all(|tool| tool["inputSchema"]["additionalProperties"] == json!(false))
  );
}

#[test]
fn orchester_tools_tell_each_cli_what_runs_and_what_asks() {
  assert_eq!(
    reading().claude_allow_rules(),
    [
      "mcp__Orchester__git_status",
      "mcp__Orchester__git_diff",
      "mcp__Orchester__git_log",
      "mcp__Orchester__git_show"
    ]
  );
  assert_eq!(
    writing().codex_tools(),
    json!({"tools": {
      "git_status": {"approval_mode": "approve"},
      "git_diff": {"approval_mode": "approve"},
      "git_log": {"approval_mode": "approve"},
      "git_show": {"approval_mode": "approve"},
      "git_add": {"approval_mode": "approve"},
      "git_commit": {"approval_mode": "approve"},
      "read_outside_file": {"approval_mode": "prompt"},
    }})
  );
  let token = ApprovalToken::generate();
  let server = CodexOrchester::new(
    "/bin/orchester-runner",
    &["--orchester-tools"],
    "127.0.0.1:4000",
    token.variable(),
  );
  let mcp = CodexMcp::new(&[]).0.with_orchester(server);
  assert_eq!(
    mcp.args(),
    [
      "-c",
      r#"mcp_servers.Orchester.command="/bin/orchester-runner""#,
      "-c",
      r#"mcp_servers.Orchester.args=["--orchester-tools"]"#,
      "-c",
      r#"mcp_servers.Orchester.env.ORCHESTER_APPROVAL_ADDRESS="127.0.0.1:4000""#,
      "-c",
      r#"mcp_servers.Orchester.env_vars=["ORCHESTER_APPROVAL_TOKEN"]"#,
      "-c",
      "mcp_servers.Orchester.tool_timeout_sec=86460",
      "-c",
      r#"shell_environment_policy.exclude=["ORCHESTER_APPROVAL_TOKEN"]"#,
    ]
  );
  assert_eq!(mcp.secrets().len(), 1);
  assert_eq!(mcp.secrets()[0].expose(), token.expose());
  assert!(!format!("{mcp:?}").contains(token.expose()));
  assert!(!mcp.args().concat().contains(token.expose()));
}

/// Claude Code asks through its permission prompt about the tools without an allow rule: those reach a
/// person whether or not the step asks before MCP actions; a call of one that runs by rule never should.
#[test]
fn orchester_tools_that_ask_reach_a_person_through_claudes_prompt() {
  let scope = ClaudeApprovalScope::of(&permissions(RepositoryAccess::ReadOnly), &[])
    .expect("a step with allowed commands asks")
    .with_orchester_tools(reading());
  let call = |tool: &str| {
    scope.classify(&json!({
      "tool_name": tool,
      "input": {"path": "/deps/src/lib.rs"},
      "tool_use_id": "toolu_1",
    }))
  };
  let ClaudeAsk::Person(request) = call("mcp__Orchester__read_outside_file") else {
    panic!("a read outside asks a person");
  };
  assert_eq!(request.server(), "Orchester");
  assert_eq!(request.action(), "read_outside_file");
  assert!(!request.is_command());
  assert_eq!(request.scope(), Some(r#"{"path":"/deps/src/lib.rs"}"#));
  assert_eq!(call("mcp__Orchester__git_status"), ClaudeAsk::Refused);
  assert_eq!(call("mcp__Orchester__git_commit"), ClaudeAsk::Refused);
  assert_eq!(call("mcp__Orchester__approve"), ClaudeAsk::Refused);
}

#[test]
fn orchester_tools_read_outside_refuses_credentials_and_protected_folders() {
  let folders = Folders::new();
  let reads = folders.reads();
  for path in [
    folders.home.join(".ssh").join("id_ed25519"),
    folders
      .home
      .join("AppData")
      .join("Roaming")
      .join("gh")
      .join("hosts.yml"),
    folders.home.join(".cargo").join("registry").join("lib.rs"),
  ] {
    assert!(
      matches!(reads.read(&text(&path), 1, 10), Err(OutsideReadError::Credentials(_))),
      "{}",
      path.display()
    );
  }
  for path in [
    folders.dependency.join(".git").join("config"),
    folders.private.join("invocation.json"),
  ] {
    assert!(
      matches!(reads.read(&text(&path), 1, 10), Err(OutsideReadError::Protected(_))),
      "{}",
      path.display()
    );
  }
  // A refused folder that holds the worktree, as the repository's `.orchester` does, still lets it be read.
  fs::write(
    folders.worktree.join("notes.md"),
    "in the worktree
",
  )
  .expect("a file");
  let around = folders.worktree.parent().expect("a parent").to_path_buf();
  let scope = CommandScope::new(&folders.worktree, &[], Some(&folders.home), None);
  let holding = OutsideReads::new(scope, &[around]);
  assert!(holding.read(&text(&folders.worktree.join("notes.md")), 1, 10).is_ok());
  assert!(matches!(
    holding.read(&text(&folders.dependency.join("src").join("lib.rs")), 1, 10),
    Err(OutsideReadError::Protected(_))
  ));
  assert!(matches!(
    reads.read("deps/src/lib.rs", 1, 10),
    Err(OutsideReadError::NotAbsolute(_))
  ));
  assert!(matches!(
    reads.read(&text(&folders.dependency), 1, 10),
    Err(OutsideReadError::NotAFile(_))
  ));
  assert!(matches!(
    reads.read(&text(&folders.dependency.join("blob.bin")), 1, 10),
    Err(OutsideReadError::Binary(_))
  ));
  assert!(matches!(
    reads.read(&text(&folders.dependency.join("missing.rs")), 1, 10),
    Err(OutsideReadError::Path(_))
  ));
}

#[test]
fn orchester_tools_read_outside_returns_numbered_lines_and_where_to_go_on() {
  let folders = Folders::new();
  let reads = folders.reads();
  let file = text(&folders.dependency.join("src").join("lib.rs"));
  assert_eq!(
    reads.read(&file, 2, 2).expect("readable"),
    "     2\tline 2\n     3\tline 3\n[the file goes on: read on from offset 4]\n"
  );
  assert_eq!(reads.read(&file, 4, 10).expect("readable"), "     4\tline 4\n");
  let context = context(&folders.worktree, &reads, &[]);
  let call = parse("read_outside_file", &json!({"path": file, "offset": 3})).expect("valid");
  let Ok(ToolWork::Done(result)) = call.work(&context) else {
    panic!("a read is answered at once");
  };
  assert!(!result.is_error());
  assert_eq!(result.text(), "     3\tline 3\n     4\tline 4\n");
}

/// A program's answer is its exit code and what it printed, cut at the end a tool keeps; a stop at the
/// time limit and an exit outside the expected codes are errors.
#[test]
fn orchester_tools_answers_are_bounded_and_carry_the_exit_code() {
  let folders = Folders::new();
  let reads = folders.reads();
  let limited = CommandAction::new(
    CommandProgram::new(&text(&std::env::current_exe().expect("exe"))).expect("a program"),
    Vec::new(),
  )
  .with_output_limit(OutputLimit::from_kib(1).expect("a limit"));
  let tools = [ProjectTool::new("Long", "prints a lot", limited)];
  let context = context(&folders.worktree, &reads, &tools);
  let program = program_of(
    &ToolCall::RunProjectTool {
      name: "Long".to_owned(),
    },
    &context,
  );
  let output = |code: Option<i32>, stdout: String| {
    Ok(CommandOutput {
      code,
      stdout,
      stderr: "warning".to_owned(),
    })
  };
  let long = format!("{}END", "x".repeat(4000));
  let result = program.finished(output(Some(0), long));
  assert!(!result.is_error());
  assert!(result.text().starts_with("exit code: 0\n"), "{}", result.text());
  assert!(result.text().contains("[stderr]\nwarning"));
  assert!(
    result.text().ends_with("bytes of output at the start left out]"),
    "{}",
    result.text()
  );
  assert!(result.text().len() < 1200);
  let failed = program.finished(output(Some(3), "boom".to_owned()));
  assert!(failed.is_error());
  assert!(failed.text().starts_with("exit code: 3\nboom"));
  let stopped = program.finished(Err(CommandError::TimedOut));
  assert!(stopped.is_error());
  assert!(stopped.text().contains("did not finish within 600 seconds"));
  let git = program_of(&parse("git_log", &json!({})).expect("valid"), &context);
  let result = git.finished(output(Some(0), format!("START{}", "y".repeat(200_000))));
  assert!(result.text().starts_with("exit code: 0\nSTART"));
  assert!(result.text().ends_with("bytes of output at the end left out]"));
}

#[test]
fn orchester_tools_run_project_tools_with_their_fixed_arguments() {
  let folders = Folders::new();
  let reads = folders.reads();
  let exe = text(&std::env::current_exe().expect("exe"));
  let tools = [project_tool("Run tests", &exe)];
  let context = context(&folders.worktree, &reads, &tools);
  let step = StepTools::for_step(&permissions(RepositoryAccess::Write), &tools);
  let call = ToolCall::parse("run_project_tool", &json!({"name": "Run tests"}), &step).expect("offered");
  let program = program_of(&call, &context);
  assert_eq!(program.command().program(), Path::new(&exe));
  assert_eq!(program.command().args(), ["--quiet", "a; rm -rf /"]);
  assert_eq!(program.command().current_dir(), Some(folders.worktree.as_path()));
  assert_eq!(
    ToolCall::parse("run_project_tool", &json!({"name": "Deploy"}), &step),
    Err(ToolCallError::UnknownProjectTool("Deploy".to_owned()))
  );
  let missing = [project_tool("Gone", "no-such-program-for-orchester-tests")];
  let context = ToolContext {
    project_tools: &missing,
    ..context
  };
  assert_eq!(
    ToolCall::RunProjectTool {
      name: "Gone".to_owned()
    }
    .work(&context),
    Err(ToolCallError::ProgramNotFound(
      "no-such-program-for-orchester-tests".to_owned()
    ))
  );
}

fn project_tool(name: &str, program: &str) -> ProjectTool {
  let args = ["--quiet", "a; rm -rf /"].map(|arg| CommandArg::new(arg).expect("an argument"));
  let action = CommandAction::new(CommandProgram::new(program).expect("a program"), args.to_vec());
  ProjectTool::new(name, "runs the tests", action)
}

fn context<'a>(worktree: &'a Path, reads: &'a OutsideReads, tools: &'a [ProjectTool]) -> ToolContext<'a> {
  static ENVIRONMENT: std::sync::LazyLock<CommandEnvironment> = std::sync::LazyLock::new(CommandEnvironment::default);
  ToolContext {
    worktree,
    inherited: &[],
    project: &ENVIRONMENT,
    outside: reads,
    project_tools: tools,
    platform: Platform::current(),
  }
}

fn program_of(call: &ToolCall, context: &ToolContext<'_>) -> ToolProgram {
  match call.work(context) {
    Ok(ToolWork::Run(program)) => *program,
    other => panic!("{call:?} runs a program, not {other:?}"),
  }
}
