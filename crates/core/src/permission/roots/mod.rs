//! Autopilot's boundary (`docs/design/approval-modes.md`): a tool request reduced to what it touches, the paths it
//! names, reads and writes and its git effects, compared with the run's allowed roots. The roots are the run's
//! worktree, its Sources (the run's snapshot, which every step of it may read: [`crate::source`]) and the folders the
//! agent CLI keeps its own files of the run in ([`agent_temp_folders`]). Every path is normalized first
//! ([`RootPath`]).
//!
//! A command is reduced from its classified parts ([`CommandPart::touches`]): its arguments, redirections and the
//! folder each part runs in, a `git -C`, `--git-dir` or `--work-tree` folder among them; then git's effects
//! ([`git_effects`]): a push, a ref other than the run's branch, a write of the main checkout's shared `.git`. A
//! file tool names its paths; an MCP call touches nothing, unless it is a local filesystem server's, whose paths the
//! caller names. The first reason found decides, part by part in order: a path outside the roots (in the main
//! checkout, or anywhere else), then git's effect, then a word that could not be placed.

mod agent_temp;
mod git_effects;
mod normal;

use std::path::{Path, PathBuf};

use thiserror::Error;

use self::git_effects::{GitEffect, effect};
pub use self::{
  agent_temp::{AgentHomes, agent_temp_folders},
  normal::RootPath,
};
use crate::{
  connection::{AgentProvider, invocation::Platform},
  permission::category::{ClassifiedCommand, CommandCategory, CommandPart},
  source::RunSources,
};

/// What a tool request touches, compared with the run's roots; the first reason found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RootsCheck {
  /// Everything it touches lies in the roots, and it changes no ref but the run's branch.
  Inside,
  Outside {
    path: PathBuf,
  },
  /// It pushes to a remote.
  Push,
  /// It changes a ref other than the run's branch: a branch by its short name, any other ref by its full name.
  OtherRef {
    name: String,
  },
  /// It touches the main checkout outside the run's worktree, or writes the repository's shared `.git`.
  MainCheckout {
    path: PathBuf,
  },
  /// What it touches could not be determined.
  Unknown,
}

/// A tool request, as the roots read it.
#[derive(Clone, Copy, Debug)]
pub enum ToolRequest<'a> {
  Command(&'a ClassifiedCommand),
  /// A file tool's paths as the agent wrote them (a read, edit or write, a search's folder, a patch's files); a
  /// relative one lies in the worktree.
  Files(&'a [String]),
  /// An MCP tool's call: the paths a local filesystem server's call names, none for any other server.
  McpTool {
    paths: &'a [String],
  },
}

/// Why one of the roots was refused.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RunRootsError {
  /// Not absolute, or a link on its way cannot be read.
  #[error("`{0}` is not an absolute path whose links can be read")]
  Unreadable(String),
}

/// What an allowed root is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootKind {
  Worktree,
  Source,
  AgentTemp,
}

/// One allowed root of a run, normalized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllowedRoot {
  kind: RootKind,
  path: RootPath,
}

impl AllowedRoot {
  pub const fn kind(&self) -> RootKind {
    self.kind
  }

  pub fn path(&self) -> &Path {
    self.path.path()
  }
}

/// A run's allowed roots, its branch and its repository's main checkout, on `platform`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunRoots {
  platform: Platform,
  branch: String,
  /// The worktree as the agent CLI was started in it, which names its own folders ([`agent_temp_folders`]).
  started_in: PathBuf,
  roots: Vec<AllowedRoot>,
  main_checkout: Option<RootPath>,
}

impl RunRoots {
  /// The roots of a run whose worktree is `worktree` (as its agent starts in it) on branch `branch`.
  pub fn new(platform: Platform, worktree: &Path, branch: &str) -> Result<Self, RunRootsError> {
    Ok(Self {
      platform,
      branch: branch.to_owned(),
      started_in: worktree.to_path_buf(),
      roots: vec![AllowedRoot {
        kind: RootKind::Worktree,
        path: folder(platform, worktree)?,
      }],
      main_checkout: None,
    })
  }

  /// These roots for a repository whose main checkout is `folder`: what lies in it outside the roots, and its
  /// `.git`, is [`RootsCheck::MainCheckout`]. Without it a write of the repository's shared state cannot be told.
  pub fn with_main_checkout(mut self, folder: &Path) -> Result<Self, RunRootsError> {
    self.main_checkout = Some(self::folder(self.platform, folder)?);
    Ok(self)
  }

  /// These roots with the run's `sources`. A source whose links cannot be read is left out, so nothing in it counts
  /// as inside.
  #[must_use]
  pub fn with_sources(self, sources: &RunSources) -> Self {
    let paths: Vec<PathBuf> = sources.paths().map(|source| source.path().to_path_buf()).collect();
    self.with_folders(RootKind::Source, &paths)
  }

  /// These roots with the folders `provider`'s CLI keeps its own files of the run in ([`agent_temp_folders`]).
  #[must_use]
  pub fn with_agent_temp(self, provider: AgentProvider, homes: AgentHomes<'_>) -> Self {
    let folders = agent_temp_folders(provider, self.platform, &self.started_in, homes);
    self.with_folders(RootKind::AgentTemp, &folders)
  }

  fn with_folders(mut self, kind: RootKind, folders: &[PathBuf]) -> Self {
    let found = folders
      .iter()
      .filter_map(|folder| RootPath::folder(self.platform, folder))
      .map(|path| AllowedRoot { kind, path });
    self.roots.extend(found.collect::<Vec<_>>());
    self
  }

  /// The allowed roots: the worktree first, then the Sources and the agent's own folders.
  pub fn roots(&self) -> &[AllowedRoot] {
    &self.roots
  }

  fn worktree(&self) -> &RootPath {
    &self.roots[0].path
  }

  /// What `request` touches, compared with the roots.
  pub fn check(&self, request: &ToolRequest<'_>) -> RootsCheck {
    match request {
      ToolRequest::Command(command) => self.command(command),
      ToolRequest::Files(paths) | ToolRequest::McpTool { paths } => paths
        .iter()
        .map(|path| match RootPath::new(self.platform, path, Some(self.worktree())) {
          Some(path) => self.place(&path),
          None => RootsCheck::Unknown,
        })
        .find(|check| *check != RootsCheck::Inside)
        .unwrap_or(RootsCheck::Inside),
    }
  }

  /// Where `path` lies: in a root, in the main checkout, or outside.
  pub fn place(&self, path: &RootPath) -> RootsCheck {
    if self.roots.iter().any(|root| path.is_in(&root.path)) {
      return RootsCheck::Inside;
    }
    let path_buf = path.path().to_path_buf();
    match &self.main_checkout {
      Some(main) if path.is_in(main) => RootsCheck::MainCheckout { path: path_buf },
      _ => RootsCheck::Outside { path: path_buf },
    }
  }

  fn command(&self, command: &ClassifiedCommand) -> RootsCheck {
    if command.parts().is_empty() {
      return RootsCheck::Unknown;
    }
    let mut git_environment = false;
    command
      .parts()
      .iter()
      .find_map(|part| {
        let check = self.part(part, git_environment);
        git_environment |= sets_git_environment(part);
        check
      })
      .unwrap_or(RootsCheck::Inside)
  }

  /// The first reason `part` is not inside: a path it touches, git's effect, a word not placed. After an earlier part
  /// set a variable of git's own (`export GIT_DIR=…`), which repository git works on cannot be told.
  fn part(&self, part: &CommandPart, git_environment: bool) -> Option<RootsCheck> {
    for touched in part.touches() {
      let check = RootPath::folder(self.platform, touched).map_or(RootsCheck::Unknown, |path| self.place(&path));
      if check != RootsCheck::Inside {
        return Some(check);
      }
    }
    if git_environment && runs_git(part) {
      return Some(RootsCheck::Unknown);
    }
    self
      .git(part)
      .or_else(|| part.unplaced().then_some(RootsCheck::Unknown))
  }

  /// Git's effect in `part`. Git under another program that does more than read (`xargs git …`, `env GIT_DIR=… git`)
  /// cannot be followed.
  fn git(&self, part: &CommandPart) -> Option<RootsCheck> {
    if part.program() != Some("git") {
      let runs_git = runs_git(part) && part.category() != CommandCategory::ReadOnly;
      return runs_git.then_some(RootsCheck::Unknown);
    }
    Some(
      match effect(&part.words()[1..], &self.branch, self.worktree().path())? {
        GitEffect::Push => RootsCheck::Push,
        GitEffect::Ref(name) => RootsCheck::OtherRef { name },
        GitEffect::Shared => match &self.main_checkout {
          Some(main) => RootsCheck::MainCheckout {
            path: main.path().join(".git"),
          },
          None => RootsCheck::Unknown,
        },
        GitEffect::Unread => RootsCheck::Unknown,
      },
    )
  }
}

/// `path` as a root: absolute, normalized.
fn folder(platform: Platform, path: &Path) -> Result<RootPath, RunRootsError> {
  RootPath::folder(platform, path).ok_or_else(|| RunRootsError::Unreadable(path.to_string_lossy().into_owned()))
}

/// Whether `word` names git, by itself or with a folder.
fn is_git(word: &str) -> bool {
  let name = word.rsplit(['/', '\\']).next().unwrap_or(word).to_lowercase();
  name == "git" || name == "git.exe"
}

/// Whether `part` runs git, by itself or under another program.
fn runs_git(part: &CommandPart) -> bool {
  part.program() == Some("git") || part.words().iter().any(|word| is_git(word))
}

/// The shell's commands that set, export or remove a variable by its name.
const SETTERS: [&str; 9] = [
  "export", "declare", "typeset", "readonly", "local", "set", "setenv", "setx", "unset",
];

/// Whether `part`, not git itself, sets a variable git reads (`GIT_DIR`, `GIT_WORK_TREE`, any `GIT_…`) for the parts
/// after it: an assignment (`export GIT_DIR=…`, `GIT_DIR=…`), a name a setter is given (`export GIT_DIR`), or
/// PowerShell's `$env:GIT_DIR`.
fn sets_git_environment(part: &CommandPart) -> bool {
  if part.program() == Some("git") {
    return false;
  }
  let setter = part
    .words()
    .first()
    .is_some_and(|word| SETTERS.contains(&word.to_lowercase().as_str()));
  part.words().iter().any(|word| {
    let (word, powershell) = match word.get(..5).filter(|start| start.eq_ignore_ascii_case("$env:")) {
      Some(_) => (&word[5..], true),
      None => (word.as_str(), false),
    };
    let name = word.split('=').next().unwrap_or(word);
    name.to_ascii_uppercase().starts_with("GIT_") && (powershell || setter || word.contains('='))
  })
}
