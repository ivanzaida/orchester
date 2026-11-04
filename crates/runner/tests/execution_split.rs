//! The split of `src/execution/` (`docs/architecture/remote-runners.md`, "The split"): the coordinator's modules sit
//! under `coordinator/`, one run's work under `executor/`, and the executor does not name the coordinator. The
//! places that still do are listed in [`KNOWN`]; a new one fails, and so does one that is gone and still listed. Only
//! the coordinator creates runs from the person's checkout, and only the executor adds or removes run worktrees.

use std::{
  collections::BTreeSet,
  fs,
  path::{Path, PathBuf},
};

/// Executor code that names a coordinator module today, as `(file under executor/, module)`: the run's task reads the
/// project's daily budget and maps refusals through `idle`, and takes and gives back agent slots on the `board` the
/// scheduler shares with it.
const KNOWN: &[(&str, &str)] = &[
  ("halt.rs", "idle"),
  ("permissions.rs", "idle"),
  ("plan.rs", "idle"),
  ("run.rs", "board"),
];

fn execution() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("src/execution")
}

fn rust_files(folder: &Path, found: &mut Vec<PathBuf>) {
  let entries = fs::read_dir(folder).unwrap_or_else(|error| panic!("cannot list {}: {error}", folder.display()));
  for entry in entries {
    let path = entry.expect("a folder entry can be read").path();
    if path.is_dir() {
      rust_files(&path, found);
    } else if path.extension().is_some_and(|extension| extension == "rs") {
      found.push(path);
    }
  }
}

/// The coordinator modules `line` names, outside comments: what follows each `coordinator::`.
fn named_coordinator_modules(line: &str) -> Vec<String> {
  let code = line.split("//").next().unwrap_or_default();
  code
    .match_indices("coordinator")
    .filter(|(at, _)| {
      let before = code[..*at].chars().next_back();
      !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
    .map(|(at, word)| {
      let rest = &code[at + word.len()..];
      rest
        .strip_prefix("::")
        .map(|rest| rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect())
        .unwrap_or_default()
    })
    .collect()
}

#[test]
fn every_execution_file_but_the_scheduler_sits_under_the_coordinator_or_the_executor() {
  let execution = execution();
  let mut files = Vec::new();
  rust_files(&execution, &mut files);
  let elsewhere: Vec<_> = files
    .iter()
    .map(|file| file.strip_prefix(&execution).expect("the file is under src/execution"))
    .filter(|file| *file != Path::new("mod.rs") && !file.starts_with("coordinator") && !file.starts_with("executor"))
    .collect();
  assert!(
    elsewhere.is_empty(),
    "outside coordinator/ and executor/: {elsewhere:?}"
  );
}

#[test]
fn executor_names_no_coordinator_module_beyond_the_known_places() {
  let executor = execution().join("executor");
  let mut files = Vec::new();
  rust_files(&executor, &mut files);
  assert!(!files.is_empty(), "no Rust file under {}", executor.display());
  let mut named = BTreeSet::new();
  for file in &files {
    let text = fs::read_to_string(file).unwrap_or_else(|error| panic!("cannot read {}: {error}", file.display()));
    let relative = file
      .strip_prefix(&executor)
      .expect("the file is under executor/")
      .to_string_lossy()
      .replace('\\', "/");
    for line in text.lines() {
      for module in named_coordinator_modules(line) {
        named.insert((relative.clone(), module));
      }
    }
  }
  let known: BTreeSet<(String, String)> = KNOWN
    .iter()
    .map(|(file, module)| ((*file).to_owned(), (*module).to_owned()))
    .collect();
  let new: Vec<_> = named.difference(&known).collect();
  assert!(
    new.is_empty(),
    "executor code names coordinator modules; reach the project through the store or the scheduler handle instead: {new:?}"
  );
  let gone: Vec<_> = known.difference(&named).collect();
  assert!(gone.is_empty(), "no longer named; remove them from KNOWN: {gone:?}");
}

/// The project database's writes of the run log: the executor sends its records to the coordinator's sink instead
/// (`docs/architecture/remote-runners.md`, rule 2).
const RUN_LOG_WRITES: &[&str] = &[
  "append_events",
  "append_events_with_plan",
  "append_runner_failure",
  "write_run_log",
];

/// The project database's writes of invocations and their transcript (`invocations`, `agent_events` and the
/// `invocation_*` tables): the executor sends these records to the coordinator's sink too (ORC-678).
const INVOCATION_WRITES: &[&str] = &[
  "start_invocation",
  "start_command_invocation",
  "save_invocation",
  "save_invocation_permissions",
  "append_activity",
  "save_invocation_prompt",
  "save_invocation_code_index",
  "save_invocation_engine",
  "save_edit_progress",
  "save_injected_memory",
  "save_invocation_connection",
  "save_invocation_account",
  "save_invocation_provider",
  "write_invocation_start",
  "write_activity",
  "write_invocation",
];

/// Where executor code names any of `writes` as a whole word outside comments, as `file: write`.
fn executor_writes(writes: &[&str]) -> Vec<String> {
  let executor = execution().join("executor");
  let mut files = Vec::new();
  rust_files(&executor, &mut files);
  let mut found = Vec::new();
  for file in &files {
    let text = fs::read_to_string(file).unwrap_or_else(|error| panic!("cannot read {}: {error}", file.display()));
    for line in text.lines() {
      let code = line.split("//").next().unwrap_or_default();
      for write in writes {
        let named = code.match_indices(write).any(|(at, _)| {
          let before = code[..at].chars().next_back();
          let after = code[at + write.len()..].chars().next();
          let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
          !word(before) && !word(after)
        });
        if named {
          found.push(format!("{}: {write}", file.display()));
        }
      }
    }
  }
  found
}

#[test]
fn executor_writes_the_run_log_only_through_the_sink() {
  let writes = executor_writes(RUN_LOG_WRITES);
  assert!(writes.is_empty(), "executor code writes the run log itself: {writes:?}");
}

#[test]
fn executor_writes_invocations_and_their_transcript_only_through_the_sink() {
  let writes = executor_writes(INVOCATION_WRITES);
  assert!(
    writes.is_empty(),
    "executor code writes invocation or transcript records itself: {writes:?}"
  );
}

/// The project database's writes of the task's conversation (`messages` and their deliveries), agents' questions
/// (`questions`) and tool requests (`tool_approvals`, `approval_interruptions`): the executor sends these records to
/// the coordinator's sink too (ORC-679).
const DECISION_WRITES: &[&str] = &[
  "add_agent_message",
  "save_deliveries",
  "change_delivery",
  "send_message",
  "send_message_with",
  "ask_questions",
  "answer_questions",
  "interrupt_questions",
  "end_waiting_question",
  "undelivered_questions",
  "request_tool_approval",
  "record_came_to_person",
  "set_reviewing",
  "record_review",
  "claim_tool_approval",
  "record_request_answer",
  "rule_answered",
  "record_denial_note",
  "delivered_tool_approval",
  "interrupt_tool_approvals",
  "end_waiting_tool_approval",
  "write_message",
  "write_question",
  "write_approval",
];

#[test]
fn executor_writes_messages_questions_and_tool_requests_only_through_the_sink() {
  let writes = executor_writes(DECISION_WRITES);
  assert!(
    writes.is_empty(),
    "executor code writes messages, questions or tool requests itself: {writes:?}"
  );
}

/// The project database's writes of steps' outputs (`step_outputs`) and progress lines (`step_progress`), what reviews
/// cost (`reviewer_usage`), plan limits reported (`plan_limit_snapshots`), a run's step connection
/// (`run_step_connections`), warm workspaces (`workspaces`, `workspace_waits`, `borrowing_runs`), worktree removals
/// (`run_worktree_removals`, `run_worktrees_not_removed`, `run_resolutions`) and base notices delivered or merged
/// (`base_notices`): the executor sends these records to the coordinator's sink too (ORC-681).
const STEP_WRITES: &[&str] = &[
  "save_step_output",
  "report_step_progress",
  "record_reviewer_usage",
  "save_limit_snapshot",
  "set_step_connection",
  "borrow_workspace",
  "return_workspace",
  "return_runner_workspaces",
  "set_workspace_stranded",
  "end_workspace_wait",
  "set_run_borrows",
  "save_worktree_removal",
  "begin_worktree_removal",
  "undo_worktree_removal",
  "save_worktree_not_removed",
  "clear_worktree_not_removed",
  "deliver_base_notice",
  "merge_base_notice",
  "note_base_notice_overlap",
  "write_step",
  "write_workspace",
  "write_removal",
  "write_notice",
];

/// Whether `code` calls `write` (`.write(` or `::write(`); a definition of the same name is no call.
fn calls(code: &str, write: &str) -> bool {
  [format!(".{write}("), format!("::{write}(")]
    .iter()
    .any(|call| code.contains(call.as_str()))
}

/// Where executor code calls any of `writes` outside comments, as `file: write`. Definitions do not count: the
/// executor's own store over the sink implements core's `WorktreeStore`.
fn executor_calls(writes: &[&str]) -> Vec<String> {
  let executor = execution().join("executor");
  let mut files = Vec::new();
  rust_files(&executor, &mut files);
  let mut found = Vec::new();
  for file in &files {
    let text = fs::read_to_string(file).unwrap_or_else(|error| panic!("cannot read {}: {error}", file.display()));
    for line in text.lines() {
      let code = line.split("//").next().unwrap_or_default();
      for write in writes.iter().filter(|write| calls(code, write)) {
        found.push(format!("{}: {write}", file.display()));
      }
    }
  }
  found
}

#[test]
fn executor_writes_step_records_workspaces_removals_and_notices_only_through_the_sink() {
  let writes = executor_calls(STEP_WRITES);
  assert!(
    writes.is_empty(),
    "executor code writes step outputs, progress, usage, plan limits, workspaces, removals or notices itself: \
     {writes:?}"
  );
}

#[test]
fn a_call_of_a_write_is_found_and_its_definition_is_not() {
  assert!(calls("db.save_worktree_removal(run, at)", "save_worktree_removal"));
  assert!(calls(
    "WorktreeStore::save_worktree_removal(db, run, at)",
    "save_worktree_removal"
  ));
  assert!(!calls(
    "fn save_worktree_removal(&mut self, run: RunId)",
    "save_worktree_removal"
  ));
}
/// The lines of the Rust files under `src/execution/<side>` that name any of `names` outside comments, as
/// `file: line`.
fn lines_naming(side: &str, names: &[&str]) -> Vec<String> {
  let folder = execution().join(side);
  let mut files = Vec::new();
  rust_files(&folder, &mut files);
  assert!(!files.is_empty(), "no Rust file under {}", folder.display());
  let mut found = Vec::new();
  for file in &files {
    let text = fs::read_to_string(file).unwrap_or_else(|error| panic!("cannot read {}: {error}", file.display()));
    let code_lines = text.lines().map(|line| line.split("//").next().unwrap_or_default());
    for line in code_lines.filter(|code| names.iter().any(|name| code.contains(name))) {
      found.push(format!("{}: {}", file.display(), line.trim()));
    }
  }
  found
}

#[test]
fn only_the_coordinator_reads_the_checkout_for_a_new_run_and_numbers_it() {
  let found = lines_naming(
    "executor",
    &[
      "create_run_of_queued_task",
      "next_run_base",
      "checked_out_branch",
      "set_branch_tip",
      "for_next_run",
    ],
  );
  assert!(
    found.is_empty(),
    "executor code creates runs or reads the person's checkout for one; the coordinator does: {found:#?}"
  );
}

#[test]
fn only_the_executor_adds_or_removes_run_worktrees() {
  let found = lines_naming(
    "coordinator",
    &[
      "Worktrees",
      "clean_up_recorded",
      "finish_removal",
      "sweep_worktree_folders",
    ],
  );
  assert!(
    found.is_empty(),
    "coordinator code adds or removes run worktrees; it hands the executor a start order or a removal instead: {found:#?}"
  );
}

/// The executor reads task and project memory only through the coordinator link (ORC-668): it never searches the
/// project database's memory indexes or reads its records and Memory setting itself.
#[test]
fn the_executor_reads_memory_only_through_the_coordinator_link() {
  let found = lines_naming(
    "executor",
    &[
      "next_invocation_memory",
      "step_memory",
      "search_memories",
      "unreadable_memories",
      "memory_settings",
      ".memories(",
      ".memory(",
    ],
  );
  assert!(
    found.is_empty(),
    "executor code reads memory from the project database; ask the coordinator link instead: {found:#?}"
  );
}

#[test]
fn a_coordinator_path_in_code_is_found_and_one_in_a_comment_is_not() {
  assert_eq!(
    named_coordinator_modules("use crate::execution::coordinator::idle;"),
    ["idle"]
  );
  assert_eq!(
    named_coordinator_modules("use super::super::coordinator::{board::Board, idle};"),
    [""]
  );
  assert!(
    named_coordinator_modules("//! the coordinator's side is `crate::execution::coordinator::plan`'s").is_empty()
  );
  assert!(named_coordinator_modules("let x = 1; // see coordinator::idle").is_empty());
  assert!(named_coordinator_modules("let subcoordinator = 1;").is_empty());
}
