//! Orchester's typed tools end to end: the relay each CLI starts (`orchester-runner --claude-permission-prompt`
//! for Claude Code, `--orchester-tools` for Codex) lists the step's tools and the runner runs a call. A call outside
//! the worktree runs only once approved: through Claude Code's permission prompt, or asked about by the runner when
//! the call arrives (every Codex call, and a call a CLI ran unasked), whether or not the step asks before MCP actions;
//! the approval names the folder the call reads in, and Approve similar covers later reads and searches there; a path
//! the tools never reach is refused before anybody is asked. Needs `git`. Run with `cargo test -p orchester-runner
//! --features test-agent --test orchester_tools orchester_tools`.

mod support;

use std::{
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  command::Command,
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  orchester_tools::OutputKept,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  run::{Answerer, ApprovalAnswer, DenialNote, PendingToolApproval, Run, RunStatus},
  source::SourcePath,
};
use orchester_exec::run_capped;
use orchester_ipc::Event;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{
  PATIENCE, Setup, TestProject, project, reported, scheduler_with, threads, tools::TEST_COMMAND, wait_for,
};

fn permissions(repository: RepositoryAccess) -> AgentPermissions {
  AgentPermissions::new(
    repository,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  )
}

async fn pending(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) -> PendingToolApproval {
  pending_where(root, events, run, |_| true).await
}

/// The run's first waiting request that is `wanted`. A request is stored before a rule of the run answers it, so one
/// a rule answers can be seen waiting for a moment; this waits for the one a person must answer.
async fn pending_where(
  root: &Path,
  events: &mut mpsc::UnboundedReceiver<Event>,
  run: RunId,
  wanted: impl Fn(&PendingToolApproval) -> bool,
) -> PendingToolApproval {
  tokio::time::timeout(PATIENCE, async {
    loop {
      let db = ProjectDb::open(root).expect("open");
      if let Some(request) = db
        .pending_tool_approvals(run)
        .expect("requests")
        .into_iter()
        .find(&wanted)
      {
        return request;
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("an approval arrives")
}

/// The folders the fakes read outside the worktree, `deps-a` and `deps-b` of the repository, with a file each.
fn outside_folders(project: &TestProject) -> (PathBuf, PathBuf) {
  let root = project.repo.path();
  let (first, second) = (root.join("deps-a"), root.join("deps-b"));
  for (folder, text) in [
    (&first, "outside line 1\noutside line 2\n"),
    (&second, "other 1\nother 2\n"),
  ] {
    std::fs::create_dir_all(folder).expect("a folder outside");
    std::fs::write(folder.join("notes.txt"), text).expect("a file outside");
  }
  (first, second)
}

fn same_folder(target: Option<&str>, folder: &Path) -> bool {
  let canonical = |path: &Path| std::fs::canonicalize(path).ok();
  target.is_some_and(|target| canonical(Path::new(target)) == canonical(folder))
}

/// Starts the run, answers the search in `deps-a` with Approve similar and the read in `deps-b` once, and waits for
/// its end. The read in `deps-a` is answered by the rule, and the read of `.git/config` is refused unasked.
async fn approve_outside(project: &TestProject) -> (Run, ProjectDb) {
  let root = project.repo.path();
  let (first, second) = outside_folders(project);
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let search = pending(root, &mut events, id).await;
  assert!(!search.command);
  assert_eq!(
    (search.server.as_str(), search.action.as_str()),
    ("Orchester", "search_external_file")
  );
  assert!(same_folder(search.target.as_deref(), &first), "{:?}", search.target);
  scheduler
    .decide_tool_approval(id, search.id, ApprovalAnswer::Similar, Answerer::Person)
    .await
    .expect("approved for the folder");
  // The read in `deps-a` the rule answers may be seen waiting for a moment first.
  let read = pending_where(root, &mut events, id, |request| {
    same_folder(request.target.as_deref(), &second)
  })
  .await;
  assert_eq!(read.action, "read_external_file");
  scheduler
    .decide_tool_approval(id, read.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  let db = ProjectDb::open(root).expect("open");
  assert!(db.pending_tool_approvals(id).expect("requests").is_empty());
  (run, db)
}

/// Claude Code asks through its permission prompt about each call outside the worktree: a person approves the
/// search for its folder, the read there is then answered by that rule, a read elsewhere asks again, and a read of
/// `.git` is denied before anybody is asked. Its own file tools are taken away.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_tools_serve_claude_code_through_its_permission_prompt() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions(RepositoryAccess::Write)),
    ..Setup::new("Use Orchester's tools.")
  });
  let (run, db) = approve_outside(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let id = run.id();
  assert_eq!(
    reported(&db, id, "orchester-tools: "),
    [
      "approve,read_file,list_files,search_file,edit_file,create_file,delete_file,move_file,find_references,\
       search_external_file,read_external_file,list_run_processes,stop_run_process,call_run_app,start_qa_app,get_task,\
       save_memory,report_progress,ask_question"
    ]
  );
  let decisions = reported(&db, id, "orchester-permission: ");
  assert_eq!(decisions.len(), 4, "{decisions:#?}");
  assert_eq!(
    decisions[..3],
    [
      "search_external_file allow",
      "read_external_file allow",
      "read_external_file allow"
    ]
  );
  assert!(decisions[3].starts_with("read_external_file deny"), "{decisions:#?}");
  let calls = reported(&db, id, "orchester-call: ");
  assert_eq!(calls.len(), 3, "{calls:#?}");
  assert!(
    calls[0].starts_with("search_external_file ok") && calls[0].contains("notes.txt"),
    "{calls:#?}"
  );
  assert!(calls[0].ends_with("     2: outside line 2"), "{calls:#?}");
  assert!(
    calls[1].ends_with(":      1\toutside line 1 |      2\toutside line 2"),
    "{calls:#?}"
  );
  assert!(calls[2].ends_with(":      2\tother 2 |      2\tother 2"), "{calls:#?}");
  let argv = reported(&db, id, "argv: ").pop().expect("argv");
  assert!(argv.contains("mcp__Orchester__edit_file"), "{argv}");
  assert!(!argv.contains("mcp__Orchester__read_external_file"), "{argv}");
  assert!(argv.contains("--disallowedTools"), "{argv}");
  for gone in ["git_status", "git_commit", "write_file", "read_outside_file"] {
    assert!(!argv.contains(&format!("mcp__Orchester__{gone}")), "{argv}");
  }
  let told = reported(&db, id, "prompt| ");
  assert!(
    told
      .iter()
      .any(|line| line.contains("mcp__Orchester__read_external_file")),
    "the prompt points the agent at the tools"
  );
}

/// Codex gets Orchester's server in its configuration, with its token only in its environment, and every
/// tool approved in the thread's configuration: the runner asks a person about each call outside the worktree when
/// it arrives, in a step that does not ask before MCP actions, and runs it once approved or covered by a rule.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_tools_serve_codex_and_ask_in_the_runner() {
  let project = project(&Setup {
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::AnyInWorktree,
      NetworkAccess::Off,
    )),
    ..Setup::new("Use Orchester's tools.")
  });
  let (run, db) = approve_outside(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let id = run.id();
  assert_eq!(
    reported(&db, id, "orchester-tools: "),
    [
      "read_file,list_files,search_file,find_references,search_external_file,read_external_file,list_run_processes,\
       stop_run_process,call_run_app,get_task,save_memory,report_progress"
    ]
  );
  assert_eq!(reported(&db, id, "turn-request: "), ["5", "7"]);
  assert_eq!(run.attempts().len(), 1);
  let calls = reported(&db, id, "orchester-call: ");
  assert_eq!(calls.len(), 4, "{calls:#?}");
  assert!(calls[0].starts_with("search_external_file ok: "), "{calls:#?}");
  assert!(calls[0].ends_with("notes.txt"), "{calls:#?}");
  assert_eq!(
    calls[1..3],
    [
      "read_external_file ok:      1\toutside line 1",
      "read_external_file ok:      2\tother 2"
    ]
  );
  assert!(
    calls[3].starts_with("read_external_file error") && calls[3].contains("protected"),
    "{calls:#?}"
  );
  let args = reported(&db, id, "mcp-arg: ");
  assert!(
    args.contains(&r#"mcp_servers.Orchester.args=["--orchester-tools"]"#.to_owned()),
    "{args:#?}"
  );
  assert_eq!(reported(&db, id, "mcp-env: "), ["ORCHESTER_APPROVAL_TOKEN=<redacted>"]);
  let approved = args
    .iter()
    .find_map(|arg| arg.strip_prefix("mcp_servers.Orchester.tools="))
    .expect("Orchester's tools approved");
  for tool in ["read_external_file", "search_external_file", "read_file"] {
    assert!(
      approved.contains(&format!(r#""{tool}"={{approval_mode="approve"}}"#)),
      "{approved}"
    );
  }
  assert!(!approved.contains("git_status"), "{approved}");
  let thread = threads(&db, id).pop().expect("a thread");
  assert!(thread["config"].get("mcp_servers").is_none(), "{thread}");
}

fn source(folder: &Path) -> SourcePath {
  SourcePath::new(&folder.to_string_lossy()).expect("an absolute path")
}

/// The run's sources, snapshotted when it starts from the project's and the task's lists, let Codex read and search
/// there without anybody being asked; `.git` stays refused inside a source too, and the prompt names them.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_tools_read_the_runs_sources_without_asking() {
  let project = project(&Setup {
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::AnyInWorktree,
      NetworkAccess::Off,
    )),
    ..Setup::new("Use Orchester's tools.")
  });
  let root = project.repo.path();
  let (first, second) = outside_folders(&project);
  let mut db = ProjectDb::open(root).expect("open");
  db.set_project_sources(&[source(&first), source(&root.join(".git"))])
    .expect("project sources");
  db.set_task_sources(project.task, &[source(&second)])
    .expect("task sources");
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let paths: Vec<&str> = run.snapshot().sources().paths().map(SourcePath::as_str).collect();
  assert_eq!(paths.len(), 3, "{paths:?}");
  let db = ProjectDb::open(root).expect("open");
  let calls = reported(&db, id, "orchester-call: ");
  assert_eq!(calls.len(), 4, "{calls:#?}");
  assert!(calls[0].starts_with("search_external_file ok: "), "{calls:#?}");
  assert_eq!(
    calls[1..3],
    [
      "read_external_file ok:      1\toutside line 1",
      "read_external_file ok:      2\tother 2"
    ]
  );
  assert!(calls[3].contains("protected"), "{calls:#?}");
  let told = reported(&db, id, "prompt| ").join("\n");
  assert!(
    told.contains(&format!("The run's sources: {}", first.display())),
    "{told}"
  );
}

/// Claude Code's permission prompt is answered by the runner for a call in the run's sources; a call elsewhere still
/// reaches a person.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_tools_answer_claudes_prompt_in_the_runs_sources() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions(RepositoryAccess::Write)),
    ..Setup::new("Use Orchester's tools.")
  });
  let root = project.repo.path();
  let (first, second) = outside_folders(&project);
  let mut db = ProjectDb::open(root).expect("open");
  db.set_project_sources(&[source(&first)]).expect("project sources");
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let read = pending(root, &mut events, id).await;
  assert_eq!(read.action, "read_external_file");
  assert!(same_folder(read.target.as_deref(), &second), "{:?}", read.target);
  scheduler
    .decide_tool_approval(id, read.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let decisions = reported(&db, id, "orchester-permission: ");
  assert_eq!(
    decisions[..3],
    [
      "search_external_file allow",
      "read_external_file allow",
      "read_external_file allow"
    ]
  );
  assert_eq!(reported(&db, id, "orchester-call: ").len(), 3);
  assert_eq!(
    db.decided_tool_approvals(id).expect("requests").len(),
    1,
    "only the read outside the sources asked"
  );
}

/// A CLI that runs a call it should have asked about gets no answer unasked: the runner asks itself, and a
/// denial reaches the agent as the call's error.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_tools_that_ask_never_run_unapproved() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions(RepositoryAccess::Write)),
    ..Setup::new("Use Orchester's tools without asking.")
  });
  outside_folders(&project);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  for action in ["search_external_file", "read_external_file", "read_external_file"] {
    let request = pending(root, &mut events, id).await;
    assert_eq!(request.action, action);
    assert!(request.id.contains(":orchester-call-"), "{}", request.id);
    scheduler
      .decide_tool_approval(id, request.id, ApprovalAnswer::Deny, Answerer::Person)
      .await
      .expect("denied");
  }
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert!(reported(&db, id, "orchester-permission: ").contains(&"search_external_file skipped".to_owned()));
  let calls = reported(&db, id, "orchester-call: ");
  assert_eq!(calls.len(), 4, "{calls:#?}");
  assert!(
    calls[..3]
      .iter()
      .all(|call| call.contains(" error ") && call.contains("denied")),
    "{calls:#?}"
  );
  assert!(calls[3].contains("protected"), "{calls:#?}");
}

/// A denial with a message refuses the relay's call with the person's words, which the agent reads as the call's error.
#[tokio::test(flavor = "multi_thread")]
async fn orchester_tools_denied_with_a_message_tell_the_call_why() {
  const WORDS: &str = "Read the vendored copy in the worktree instead.";
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions(RepositoryAccess::Write)),
    ..Setup::new("Use Orchester's tools without asking.")
  });
  outside_folders(&project);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  for number in 0..3 {
    let request = pending(root, &mut events, id).await;
    let answer = if number == 0 {
      ApprovalAnswer::DenyWithMessage {
        message: DenialNote::new(WORDS).expect("words"),
      }
    } else {
      ApprovalAnswer::Deny
    };
    scheduler
      .decide_tool_approval(id, request.id, answer, Answerer::Person)
      .await
      .expect("denied");
  }
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let calls = reported(&db, id, "orchester-call: ");
  assert!(
    calls[0].contains(" error ")
      && calls[0].contains(&format!(
        "The person reviewing this run denied Orchester search_external_file: {WORDS}"
      )),
    "{calls:#?}"
  );
  let decided = db.decided_tool_approvals(id).expect("decided");
  assert_eq!(decided[0].note.as_ref().map(DenialNote::as_str), Some(WORDS));
  assert!(decided[1..].iter().all(|decided| decided.note.is_none()));
}

/// A program's output is read as it comes and kept within the cap, at the end the tool keeps, whatever it
/// prints.
#[test]
fn orchester_tools_keep_program_output_within_the_cap() {
  let command = Command::new(Path::new(TEST_COMMAND), ["print", "300000"]);
  let limit = 64 * 1024;
  let head = run_capped(&command, Duration::from_secs(60), (limit, OutputKept::Head)).expect("ran");
  assert!(head.stdout.len() <= limit, "{}", head.stdout.len());
  assert!(head.stdout.contains("line 1\n"));
  assert!(head.left_out > 3_000_000, "{}", head.left_out);
  let tail = run_capped(&command, Duration::from_secs(60), (limit, OutputKept::Tail)).expect("ran");
  assert!(tail.stdout.len() <= limit);
  assert!(tail.stdout.trim_end().ends_with("line 300000"));
  assert_eq!(tail.code, Some(0));
}
