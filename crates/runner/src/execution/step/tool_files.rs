//! Orchester's file tools for Claude Code and Codex: the native agent's executors
//! (`orchester_agent::files::FileTools`), confined to the step's `FileScope` (the worktree, writable only
//! when the step may write, the step's extra writable folders, the code index and skills read-only; `.git`,
//! `.orchester` and links that lead out refused). One instance lives as long as the invocation, so its read
//! stamps do: an edit needs a read of the file in this invocation that still matches it.
//!
//! When the agent gets the code index of the run's base revision, the tools use it: `find_symbol` looks
//! definitions up in it, `search` for a name lists its definitions before the content matches, and
//! `list_files` answers from the index's file list and the files changed in the worktree since the base
//! revision. A definition in a changed file is marked, as its line may have moved.

use std::{collections::HashSet, path::PathBuf};

use orchester_agent::FileTools;
use orchester_core::{
  connection::stream::RunnerProblem,
  native::{
    scope::{FileScope, ScopeRoots},
    tools::{ListFiles, Search, ToolInput},
  },
  orchester_tools::{CodeIndex, FindSymbol, Revision, SymbolHit, ToolContext, ToolResult, changed_files, shown_hits},
  resource::RepositoryAccess,
};
use orchester_exec::{
  ripgrep::{self, LOCAL_APP_DATA},
  run_capped,
};

use super::{orchester_tools::ToolFolders, start::problem};

/// What a definition list says when the files changed since the index are unknown.
const UNKNOWN_CHANGES: &str = "[the files changed since the index could not be read: any line may have moved]";
/// The most bytes of git's list of changed files read.
const CHANGED_CAP: usize = 4 * 1024 * 1024;

/// The file tools of one invocation, and the code index they use when the agent gets it.
pub(super) struct ToolFiles {
  files: FileTools,
  index: Option<(CodeIndex, Revision)>,
}

impl ToolFiles {
  /// The file tools of a step with repository access `access`, working in `folders`, whose run started from
  /// `base`. They search with the ripgrep on the runner's `PATH` (or winget's), else they walk the folders.
  pub(super) fn new(access: RepositoryAccess, folders: ToolFolders<'_>, base: &str) -> Result<Self, RunnerProblem> {
    let scope = FileScope::new(&ScopeRoots {
      worktree: folders.worktree,
      access,
      code_index: folders.code_index,
      skills: folders.skills,
      writable_folders: folders.writable,
    })
    .map_err(|error| problem(format!("cannot confine Orchester's file tools: {error}")))?;
    let ripgrep: Option<PathBuf> = ripgrep::locate(
      std::env::var_os("PATH").as_deref(),
      std::env::var_os(LOCAL_APP_DATA).as_deref(),
      &[],
    )
    .map(|found| found.path);
    let index = match folders.code_index {
      Some(folder) => {
        let base = Revision::new(base).map_err(|error| problem(format!("the run's base revision: {error}")))?;
        Some((CodeIndex::new(folder), base))
      }
      None => None,
    };
    Ok(Self {
      files: FileTools::new(scope, ripgrep),
      index,
    })
  }

  /// Whether the tools use the code index.
  pub(super) const fn indexed(&self) -> bool {
    self.index.is_some()
  }

  /// Runs the file tool call `input` in `context`, blocking on the file system.
  pub(super) fn run(&self, input: &ToolInput, context: &ToolContext<'_>) -> ToolResult {
    let outcome = match input {
      ToolInput::ReadFile(input) => self.files.read_file(input),
      ToolInput::ListFiles(input) => return self.list(input, context),
      ToolInput::Search(input) => return self.search(input, context),
      ToolInput::EditFile(input) => self.files.edit_file(input),
      ToolInput::WriteFile(input) => self.files.write_file(input),
      ToolInput::RunCommand(_) | ToolInput::FinishStep(_) | ToolInput::AskQuestion(_) => {
        return ToolResult::unavailable("this is no file tool");
      }
    };
    ToolResult::of_file_tool(outcome.content, outcome.is_error)
  }

  /// `find_symbol`: the definitions the code index lists for `query`.
  pub(super) fn symbols(&self, query: &FindSymbol, context: &ToolContext<'_>) -> ToolResult {
    let Some((index, base)) = &self.index else {
      return ToolResult::unavailable("the code index is off for this run");
    };
    match index.find(query, false) {
      Ok(hits) if hits.is_empty() => {
        ToolResult::of_file_tool(format!("no definition of {} in the code index", query.name()), false)
      }
      Ok(hits) => ToolResult::of_file_tool(marked(&hits, changed(base, context).as_ref()), false),
      Err(error) => ToolResult::unavailable(&format!("the code index cannot be read: {error}")),
    }
  }

  /// `list_files`, from the code index's file list and the files changed since when the agent gets it.
  fn list(&self, input: &ListFiles, context: &ToolContext<'_>) -> ToolResult {
    let outcome = match &self.index {
      Some((index, base)) => match index.files() {
        // Without the worktree's changes the index's list would miss files: the folder is listed instead.
        Ok(mut known) => match changed(base, context) {
          Some(changed) => {
            known.extend(changed);
            known.sort();
            known.dedup();
            self.files.list_known_files(input, &known)
          }
          None => self.files.list_files(input),
        },
        Err(_) => self.files.list_files(input),
      },
      None => self.files.list_files(input),
    };
    ToolResult::of_file_tool(outcome.content, outcome.is_error)
  }

  /// `search`: for a name, the definitions the code index lists under the searched folder first, then the
  /// content matches as always.
  fn search(&self, input: &Search, context: &ToolContext<'_>) -> ToolResult {
    let outcome = self.files.search(input);
    let definitions = match &self.index {
      Some((index, base)) if is_name(input.pattern()) => {
        // An index that cannot be read adds no definitions; the content matches still answer.
        let hits: Vec<SymbolHit> = index
          .find(&FindSymbol::definitions_of(input.pattern()), true)
          .unwrap_or_default()
          .into_iter()
          .filter(|hit| self.files.search_covers(input, &hit.path))
          .collect();
        (!hits.is_empty()).then(|| marked(&hits, changed(base, context).as_ref()))
      }
      _ => None,
    };
    match definitions {
      Some(definitions) if !outcome.is_error => ToolResult::of_file_tool(
        format!(
          "Definitions (code index):\n{definitions}\nMatches:\n{}",
          outcome.content
        ),
        false,
      ),
      _ => ToolResult::of_file_tool(outcome.content, outcome.is_error),
    }
  }
}

/// Whether `pattern` is a plain name a definition could have.
fn is_name(pattern: &str) -> bool {
  let mut chars = pattern.chars();
  chars.next().is_some_and(|first| first.is_alphabetic() || first == '_')
    && chars.all(|c| c.is_alphanumeric() || c == '_')
    && pattern.len() >= 3
}

/// The files of the worktree changed since `base`, relative to it with `/`; `None` when git cannot say.
fn changed(base: &Revision, context: &ToolContext<'_>) -> Option<HashSet<String>> {
  let mut paths = HashSet::new();
  for program in changed_files(base, context) {
    let output = run_capped(program.command(), program.timeout(), (CHANGED_CAP, program.cap().1)).ok()?;
    // A git that failed, or a list cut short, tells nothing reliable.
    if output.code != Some(0) || output.left_out > 0 {
      return None;
    }
    paths.extend(
      output
        .stdout
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned),
    );
  }
  Some(paths)
}

/// `hits` as the agent reads them, marked by `changed`, or with a note when the changes are unknown.
fn marked(hits: &[SymbolHit], changed: Option<&HashSet<String>>) -> String {
  match changed {
    Some(changed) => shown_hits(hits, changed),
    None => format!("{}{UNKNOWN_CHANGES}\n", shown_hits(hits, &HashSet::new())),
  }
}
