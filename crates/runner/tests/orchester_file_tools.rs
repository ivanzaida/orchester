//! Orchester's file tools end to end through the relay, with the fake CLIs: the native agent's executors
//! confined to the worktree, an edit only after a read in the same invocation, no edit in a read-only step,
//! answers within their cap; and, with the code index of the run's base revision, `find_symbol`, a search
//! that lists a name's definitions first and a listing from the index, a changed file marked. Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test orchester_file_tools
//! orchester_file_tools`.

mod support;

use std::fs;

use orchester_core::{
  code_index::CodeIndexSetting,
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  run::{Run, RunStatus},
};
use orchester_store::ProjectDb;

use self::support::{Setup, TestProject, git, project, reported, scheduler_with, wait_for};

fn permissions(repository: RepositoryAccess, commands: CommandPermission) -> AgentPermissions {
  AgentPermissions::new(repository, commands, NetworkAccess::Off)
}

async fn finished(project: &TestProject) -> (Run, ProjectDb) {
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id: RunId = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  let db = ProjectDb::open(root).expect("open");
  assert!(
    db.pending_tool_approvals(id).expect("requests").is_empty(),
    "nothing asked"
  );
  (run, db)
}

/// Claude Code reads, lists, searches, edits and writes through Orchester's tools, each by its allow rule:
/// an edit before a read is refused, paths out of the worktree and into `.git` or `.orchester` are refused,
/// and a large file's answer is cut.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_file_tools_serve_claude_code_in_a_writing_step() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions(
      RepositoryAccess::Write,
      CommandPermission::Allowed(AllowedCommands::default()),
    )),
    ..Setup::new("Use the file tools.")
  });
  let (run, db) = finished(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let id = run.id();
  let listed = reported(&db, id, "orchester-tools: ").pop().expect("listed");
  assert!(
    listed.starts_with("approve,read_file,list_files,search,edit_file,write_file,git_status"),
    "{listed}"
  );
  let permissions = reported(&db, id, "orchester-permission: ");
  assert!(
    permissions.iter().all(|line| line.ends_with("allow-by-rule")),
    "{permissions:#?}"
  );
  let calls = reported(&db, id, "orchester-call: ");
  let expect = [
    ("list_files ok", "README.md"),
    ("edit_file error", "has not been read"),
    ("read_file ok", "hi"),
    ("edit_file ok", ""),
    ("write_file ok", ""),
    ("search ok", "README.md"),
    ("read_file error", "outside the folders this step may read"),
    ("write_file error", "protected"),
    ("write_file error", "protected"),
    ("read_file ok", "truncated"),
  ];
  assert_eq!(calls.len(), expect.len(), "{calls:#?}");
  for (call, (start, holds)) in calls.iter().zip(expect) {
    assert!(
      call.starts_with(start) && call.contains(holds),
      "{start} … {holds}: {call}"
    );
  }
  let head = git(project.repo.path(), &["rev-parse", run.branch().as_str()]);
  let readme = git(project.repo.path(), &["show", &format!("{head}:README.md")]);
  assert!(readme.starts_with("hi there"), "{readme}");
  let argv = reported(&db, id, "argv: ").pop().expect("argv");
  assert!(argv.contains("mcp__Orchester__edit_file"), "{argv}");
  let told = reported(&db, id, "prompt| ").join("\n");
  assert!(
    told.contains("mcp__Orchester__edit_file (after reading the file)"),
    "{told}"
  );
}

/// A read-only step is offered no file tool that changes anything: Codex reads, and its edit and write are
/// refused as not offered.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_file_tools_refuse_changes_in_a_read_only_step() {
  let project = project(&Setup {
    permissions: Some(permissions(
      RepositoryAccess::ReadOnly,
      CommandPermission::AnyInWorktree,
    )),
    ..Setup::new("Use the file tools.")
  });
  let (run, db) = finished(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let listed = reported(&db, run.id(), "orchester-tools: ").pop().expect("listed");
  assert!(listed.starts_with("read_file,list_files,search,git_status"), "{listed}");
  assert!(
    !listed.contains("edit_file") && !listed.contains("find_symbol"),
    "{listed}"
  );
  assert_eq!(
    reported(&db, run.id(), "orchester-call: "),
    [
      "read_file ok:      1\thi",
      "edit_file error: `edit_file` is not available in this step",
      "write_file error: `write_file` is not available in this step"
    ]
  );
}

/// A project whose base revision has `src/lib.rs` with `parse_config` and `ConfigStore`.
fn with_sources(code_index: CodeIndexSetting) -> TestProject {
  let project = project(&Setup {
    code_index,
    ..Setup::new("Use the index tools.")
  });
  let root = project.repo.path();
  fs::create_dir_all(root.join("src")).expect("a folder");
  fs::write(
    root.join("src").join("lib.rs"),
    "pub fn parse_config() {}\npub struct ConfigStore;\n",
  )
  .expect("a file");
  git(root, &["add", "src/lib.rs"]);
  git(root, &["commit", "--quiet", "-m", "sources"]);
  project
}

/// With the code index, `find_symbol` finds definitions and marks one in a file changed since the index,
/// `search` lists a name's definitions before its matches, and `list_files` knows the index's files and a new
/// one.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_file_tools_use_the_code_index() {
  let project = with_sources(CodeIndexSetting::On);
  let (run, db) = finished(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let listed = reported(&db, run.id(), "orchester-tools: ").pop().expect("listed");
  assert!(listed.contains("write_file,find_symbol,git_status"), "{listed}");
  let calls = reported(&db, run.id(), "orchester-call: ");
  assert_eq!(
    calls,
    [
      "find_symbol ok: src/lib.rs:1 function parse_config [changed since the index: the line may have moved]",
      "find_symbol ok: src/lib.rs:2 struct ConfigStore [changed since the index: the line may have moved]",
      "search ok: Definitions (code index):",
      "list_files ok: src/fresh.rs"
    ]
  );
  let answers = reported(&db, run.id(), "orchester-answer| ");
  assert!(answers.contains(&"src/lib.rs".to_owned()), "{answers:#?}");
  assert!(answers.contains(&"Matches:".to_owned()), "{answers:#?}");
}

/// Without the code index there is no `find_symbol`, and `search` answers with the content matches alone.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_file_tools_without_the_code_index_search_as_before() {
  let project = with_sources(CodeIndexSetting::Off);
  let (run, db) = finished(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let listed = reported(&db, run.id(), "orchester-tools: ").pop().expect("listed");
  assert!(!listed.contains("find_symbol"), "{listed}");
  let calls = reported(&db, run.id(), "orchester-call: ");
  assert!(calls[0].starts_with("find_symbol error"), "{calls:#?}");
  assert!(calls[2].starts_with("search ok: src/lib.rs:"), "{calls:#?}");
}
