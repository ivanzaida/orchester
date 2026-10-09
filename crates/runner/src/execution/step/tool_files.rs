//! Orchester's file tools for Claude Code and Codex: the native agent's executors
//! (`orchester_agent::files::FileTools`), confined to the step's `FileScope` (the worktree, writable only
//! when the step may write, the step's extra writable folders, the code index and skills read-only; `.git`,
//! `.orchester` and links that lead out refused). One instance lives as long as the invocation, so its read
//! stamps do: an edit needs a read of the file in this invocation that still matches it.
//!
//! When the agent gets the code index of the run's base revision, the tools use it: `find_symbol` looks
//! definitions up in it, `search_file` for a name lists its definitions before the files that hold it, and
//! `list_files` answers from the index's file list and the files changed in the worktree since the base
//! revision. A definition in a changed file is marked, as its line may have moved. The index holds no file
//! contents, so `read_file` always reads the file on disk. `find_references` lists a name's definitions from the
//! index, then the worktree's files that use it as a whole word, and says when the index is off.

use std::{
  collections::HashSet,
  path::{Path, PathBuf},
};

use orchester_agent::FileTools;
use orchester_core::{
  connection::stream::RunnerProblem,
  native::{
    scope::{FileScope, ScopeRoots},
    session::ToolOutcome,
    tools::ListFiles,
  },
  orchester_tools::{
    CodeIndex, DEFAULT_LIMIT_FILES, FileCall, FindSymbol, OutsideReads, Revision, SearchTerm, SymbolHit, ToolContext,
    ToolResult, changed_files, shown_hits,
  },
  resource::RepositoryAccess,
};
use orchester_exec::{
  ripgrep::{self, LOCAL_APP_DATA},
  run_capped,
};

use super::{orchester_tools::ToolFolders, start::problem};

/// What `find_references` says without the code index.
const NO_INDEX: &str = "[the code index is off for this run: definitions are not told apart from uses]";
/// The most files `find_references` lists.
const REFERENCE_FILES: u32 = 50;
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

  /// Runs the worktree file tool call `call` in `context`, blocking on the file system.
  pub(super) fn run(&self, call: &FileCall, context: &ToolContext<'_>) -> ToolResult {
    let outcome = match call {
      FileCall::Read(input) => self.files.read_lines(input),
      FileCall::List(input) => return self.list(input, context),
      FileCall::Search(input) => return self.search(input, context),
      FileCall::Edit(input) => self.files.edit_lines(input),
      FileCall::Create(input) => self.files.create_file(input),
      FileCall::Delete(filename) => self.files.delete_file(filename),
      FileCall::Move { from, to } => self.files.move_file(from, to),
    };
    answer(outcome)
  }

  /// `search_external_file`: the files under `folder` (checked and resolved) that hold `term`, each let through by
  /// `outside` as a read of it would be.
  pub(super) fn search_external(&self, folder: &Path, term: &str, outside: &OutsideReads) -> ToolResult {
    let keep = |file: &Path| file.to_str().is_some_and(|file| outside.check(file).is_ok());
    answer(self.files.search_outside(folder, (term, DEFAULT_LIMIT_FILES), &keep))
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

  /// `find_references`: the definitions of `name` the code index lists, marked when their file changed since the
  /// index, then the worktree's files that use it as a whole word.
  pub(super) fn references(&self, name: &str, context: &ToolContext<'_>) -> ToolResult {
    let uses = self.files.search_word(name, REFERENCE_FILES);
    if uses.is_error {
      return answer(uses);
    }
    let definitions = match &self.index {
      // An index that cannot be read says so; the uses still answer.
      Some((index, base)) => match index.find(&FindSymbol::definitions_of(name), true) {
        Ok(hits) if hits.is_empty() => format!("no definition of {name} in the code index\n"),
        Ok(hits) => marked(&hits, changed(base, context).as_ref()),
        Err(error) => format!("[the code index cannot be read: {error}]\n"),
      },
      None => format!("{NO_INDEX}\n"),
    };
    ToolResult::of_file_tool(
      format!("Definitions (code index):\n{definitions}\nUses:\n{}", uses.content),
      false,
    )
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

  /// `search_file`: for a name, the definitions the code index lists first, then the files that hold the term.
  fn search(&self, input: &SearchTerm, context: &ToolContext<'_>) -> ToolResult {
    let outcome = self.files.search_term(input);
    let definitions = match &self.index {
      Some((index, base)) if is_name(input.term()) => {
        // An index that cannot be read adds no definitions; the content matches still answer.
        let hits: Vec<SymbolHit> = index
          .find(&FindSymbol::definitions_of(input.term()), true)
          .unwrap_or_default();
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

/// The answer of a file tool that ended as `outcome` says.
fn answer(outcome: ToolOutcome) -> ToolResult {
  ToolResult::of_file_tool(outcome.content, outcome.is_error)
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
