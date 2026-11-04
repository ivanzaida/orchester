//! The executor's folders come from its execution root (`docs/architecture/remote-runners.md`, rule 6): locally the
//! repository, so each folder is where it always was, and no executor code joins `.orchester/...` itself.

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  id::{RunId, StepExecutionId},
  workspaces::WorkspaceNumber,
};
use orchester_index::CodebaseIndexes;
use orchester_runner::{
  execution_root::{self, ExecutionRoot},
  private_file,
  worktree::Worktrees,
};

fn repository() -> PathBuf {
  std::path::absolute(env!("CARGO_MANIFEST_DIR")).expect("an absolute repository path")
}

#[test]
fn execution_root_local_keeps_every_repository_folder_where_it_was() {
  let repository = repository();
  let state = repository.join(".orchester");
  let root = ExecutionRoot::local(&repository);
  let (run, execution) = (RunId::generate(), StepExecutionId::generate());
  let workspace = WorkspaceNumber::new(2).expect("two");

  assert_eq!(root.repository(), repository);
  assert_eq!(root.run_worktree(run), state.join("worktrees").join(run.to_string()));
  assert_eq!(
    root.worktree(&execution.to_string()),
    state.join("worktrees").join(execution.to_string())
  );
  assert_eq!(root.workspace(workspace), state.join("workspaces").join("2"));
  assert_eq!(root.run_storage(run), state.join("runs").join(run.to_string()));
  assert_eq!(root.skills(execution), state.join("skills").join(execution.to_string()));
  assert_eq!(root.tools(), state.join("tools"));
  assert_eq!(root.attachments(), state.join("attachments"));
  assert_eq!(
    format!("{:?}", root.code_indexes()),
    format!("{:?}", CodebaseIndexes::new(&repository))
  );
  assert_eq!(
    execution_root::relative_worktree(&run.to_string()),
    format!(".orchester/worktrees/{run}")
  );
}

#[test]
fn execution_root_local_worktrees_are_the_runs_worktrees() {
  let repository = repository();
  let root = ExecutionRoot::local(&repository);
  let worktrees = Worktrees::new(&repository).expect("worktrees");
  let (run, execution) = (RunId::generate(), StepExecutionId::generate());

  assert_eq!(worktrees.path(run), root.run_worktree(run));
  assert_eq!(worktrees.step_path(execution), root.worktree(&execution.to_string()));
}

#[test]
fn execution_root_computer_folders_stay_where_they_were() {
  let private = std::env::temp_dir().join("orchester-mcp");
  assert_eq!(execution_root::private_files(), private);
  let project = orchester_core::id::ProjectId::generate();
  assert_eq!(private_file::directory(project), private.join(project.to_string()));
  #[cfg(windows)]
  {
    let data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    assert_eq!(
      execution_root::qa_slots(),
      data.as_ref().map(|data| data.join("orchester-qa-slots"))
    );
    assert_eq!(
      execution_root::app_discovery(),
      data.map(|data| data.join("orchester-app-mcp"))
    );
  }
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

/// What derives an executor folder by itself rather than through the execution root.
const OWN_JOINS: &[&str] = &[
  "join(\".orchester",
  "\".orchester/worktrees",
  "\".orchester/workspaces",
  "\".orchester/runs",
  "artifacts_folder(",
  "CodebaseIndexes::new(",
  "temp_dir().join(",
  "LOCALAPPDATA",
];

#[test]
fn execution_root_is_the_only_executor_code_that_joins_its_folders() {
  let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
  let mut files = Vec::new();
  rust_files(&source.join("execution").join("executor"), &mut files);
  rust_files(&source.join("private_file"), &mut files);
  files.push(source.join("worktree.rs"));
  rust_files(&source.join("worktree"), &mut files);
  let mut found = Vec::new();
  for file in &files {
    let text = fs::read_to_string(file).unwrap_or_else(|error| panic!("cannot read {}: {error}", file.display()));
    for line in text.lines() {
      let code = line.split("//").next().unwrap_or_default();
      if OWN_JOINS.iter().any(|join| code.contains(join)) {
        found.push(format!("{}: {}", file.display(), line.trim()));
      }
    }
  }
  assert!(
    found.is_empty(),
    "derive these folders from the execution root (`execution_root`): {found:#?}"
  );
}
