//! `init_repository`, a tool of the Orchestrator chat's own (ORC-72, `docs/design/orchestrator-toolset.md`): making
//! the project's folder a git repository with its first commit, or giving the repository without a commit it is
//! already its first commit, through the setup page's own functions ([`crate::repository::init`]). It writes to the
//! folder, so it is always a card the person applies ([`super::change`]), whatever auto mode and the person's switches
//! say: the call only reads what the commit would hold (its branch, the files it adds, the `.gitignore` it honours in
//! full and the files that look as if they hold secrets). Applying checks the folder again: a repository that has a
//! commit is never touched, a `.gitignore` never written over, and Orchester's `.orchester/` is kept out of git as
//! for every project. Nothing adds a remote, pushes or changes git's settings.

use std::{
  fs,
  io::{ErrorKind, Write},
  path::Path,
};

use orchester_core::{
  command::CommandRunner,
  orchestrator::change::{Change, FirstCommitOf, Gitignore, ProposedChange},
};
use orchester_store::exclude_project_data;
use rmcp::{
  handler::server::common::schema_for_type,
  model::{JsonObject, Tool, ToolAnnotations},
  schemars::JsonSchema,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
  app_mcp::ToolError,
  messages::Message,
  repository::init::{
    FirstCommit, INITIAL_COMMIT, InitError, InitStep, LocalRepository, SurveyError, create_repository,
    first_commit_files, initial_branch, make_first_commit, read_local, sensitive,
  },
};

/// The tool's name.
pub const INIT_REPOSITORY: &str = "init_repository";

/// What the chat's agent reads of the tool.
const DESCRIPTION: &str = "Proposes making this project's folder a git repository with its first commit, \"Initial \
  commit\", for a folder that is not a git repository yet, or only that first commit for a repository without one: \
  git init on the branch git's init.defaultBranch names, else main; with gitignore, that whole text written as the \
  folder's .gitignore first (only where the folder has none, else refused as gitignore_exists: the folder's own is \
  honoured as it is); then a first commit of the folder's files that .gitignore does not list (first_commit files) or \
  an empty one (empty). Orchester's .orchester folder always stays out of git. It never adds a remote, pushes or \
  changes git's settings. It writes to the project's folder, so in this chat it is always a card the person applies \
  or dismisses in the conversation, whatever Autopilot and the person's switches say: nothing is made until they \
  apply it, on the card or by approving it in a later message (apply_proposal, only when no file looks as if it holds \
  secrets). The card shows the branch, how many files the commit adds, the whole .gitignore \
  it honours and the files that look as if they hold secrets (such as .env), which the person decides on: propose a \
  .gitignore that keeps out what should not be committed, e.g. .orchester/ and the stack's build output. Refused: a \
  repository with a commit already (has_commits: its history is never touched), git not installed (git_not_found), a \
  folder that cannot be written (folder_not_writable), git's own error in its words (git_failed), a first commit of \
  files where there are none (nothing_to_commit), and when applied, a commit git cannot make because it does not know \
  the author (author_unknown, with the git config commands that tell it) or a folder that changed since it was \
  proposed (changed_since_proposed).";

/// The tool as the chat's `tools/list` answers.
pub fn tool() -> Tool {
  Tool::new(INIT_REPOSITORY, DESCRIPTION, schema_for_type::<InitRepository>())
    .with_annotations(ToolAnnotations::new().read_only(false).open_world(false))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InitRepository {
  /// The project's id, or its name: the chat's own.
  pub project: String,
  /// What the first commit holds: `files`, the folder's files that `.gitignore` does not list, or `empty`, nothing.
  pub first_commit: FirstCommitArgument,
  /// The whole text of a `.gitignore` written before the commit, for a folder that has none. Left out, the folder's
  /// own `.gitignore`, if it has one, is honoured as it is.
  #[serde(default)]
  pub gitignore: Option<String>,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum FirstCommitArgument {
  Files,
  Empty,
}

/// What git keeps out of the first commit besides `.gitignore`: Orchester's own folder, which every project excludes
/// through the repository's `info/exclude`.
const PROJECT_DATA: &str = "/.orchester/";
const GITIGNORE: &str = ".gitignore";

/// The first commit the agent's `arguments` propose for the project named `project` at `root`, read with `runner`;
/// refused when the folder cannot have it. Changes nothing. Blocks on git.
pub fn propose(
  runner: &dyn CommandRunner,
  (root, project): (&Path, &str),
  arguments: &JsonObject,
) -> Result<ProposedChange, ToolError> {
  let asked: InitRepository = serde_json::from_value(Value::Object(arguments.clone()))
    .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
  let (exists, branch) = match read_local(runner, root) {
    LocalRepository::NotRepository => (false, initial_branch(runner).map_err(refusal)?.name().to_owned()),
    LocalRepository::NoCommits { branch: Some(branch) } => (true, branch),
    LocalRepository::NoCommits { branch: None } => (true, initial_branch(runner).map_err(refusal)?.name().to_owned()),
    other => return Err(not_open(&other)),
  };
  let gitignore = gitignore(root, asked.gitignore)?;
  let first = match asked.first_commit {
    FirstCommitArgument::Files => FirstCommitOf::Files,
    FirstCommitArgument::Empty => FirstCommitOf::Empty,
  };
  let files = match first {
    FirstCommitOf::Files => added(runner, root, gitignore.as_ref())?,
    FirstCommitOf::Empty => Vec::new(),
  };
  if first == FirstCommitOf::Files && files.is_empty() {
    return Err(refused("nothing_to_commit", "orchestrator.repository.nothingToCommit"));
  }
  Ok(ProposedChange {
    subject: project.to_owned(),
    change: Change::InitRepository {
      branch,
      first,
      files: files.len(),
      exists,
      gitignore,
      sensitive: sensitive(&files),
    },
  })
}

/// Carries out `change`, the first commit proposed for the project at `root`, with `runner`, once the folder is checked
/// again: a repository with a commit since is refused and left as it is, and so is a `.gitignore`, or a file that looks
/// as if it holds secrets, the person was not shown. Blocks on git.
pub fn apply(runner: &dyn CommandRunner, root: &Path, change: &Change) -> Result<Value, ToolError> {
  let Change::InitRepository {
    first,
    gitignore,
    sensitive: shown,
    ..
  } = change
  else {
    return Err(ToolError::InvalidArguments(
      "not a repository's first commit".to_owned(),
    ));
  };
  let made = match read_local(runner, root) {
    LocalRepository::NotRepository => None,
    LocalRepository::NoCommits { branch } => Some(branch),
    other => return Err(not_open(&other)),
  };
  let writes = match gitignore {
    Some(Gitignore::Writes { text }) => Some(text.as_str()),
    Some(Gitignore::Kept { .. }) | None => None,
  };
  if writes.is_some() && root.join(GITIGNORE).exists() {
    return Err(refused(
      "changed_since_proposed",
      "orchestrator.repository.gitignoreAppeared",
    ));
  }
  if *first == FirstCommitOf::Files {
    let appeared: Vec<String> = sensitive(&added(runner, root, gitignore.as_ref())?)
      .into_iter()
      .filter(|file| !shown.contains(file))
      .collect();
    if !appeared.is_empty() {
      return Err(ToolError::refused(
        "changed_since_proposed",
        vec![Message::new("orchestrator.repository.sensitiveAppeared").with("files", appeared.join(", "))],
      ));
    }
  }
  let branch = match made {
    Some(branch) => branch,
    None => Some(create_repository(runner, root).map_err(refusal)?),
  };
  // The repository is there, made or read just now: only writing its `info/exclude` can fail.
  exclude_project_data(root).map_err(|_| refusal(InitError::NotWritable))?;
  if let Some(text) = writes {
    write_gitignore(root, text)?;
  }
  let first = match first {
    FirstCommitOf::Files => FirstCommit::Files,
    FirstCommitOf::Empty => FirstCommit::Empty,
  };
  let committed = make_first_commit(runner, root, first).map_err(refusal)?;
  Ok(json!({
    "repository": { "branch": branch, "commit": INITIAL_COMMIT, "files": committed.files },
  }))
}

/// The `.gitignore` the commit honours: the chat's `proposed` text, for a folder without one, or the folder's own.
fn gitignore(root: &Path, proposed: Option<String>) -> Result<Option<Gitignore>, ToolError> {
  let path = root.join(GITIGNORE);
  let own = match fs::read_to_string(&path) {
    Ok(text) => Some(text),
    Err(error) if error.kind() == ErrorKind::NotFound => None,
    Err(_) => {
      return Err(refused(
        "gitignore_unreadable",
        "orchestrator.repository.gitignoreUnreadable",
      ));
    }
  };
  match (proposed, own) {
    (Some(_), Some(_)) => Err(refused("gitignore_exists", "orchestrator.repository.gitignoreExists")),
    (Some(text), None) if text.trim().is_empty() => Err(ToolError::InvalidArguments(
      "gitignore: leave it out rather than empty".to_owned(),
    )),
    (Some(text), None) if text.ends_with('\n') => Ok(Some(Gitignore::Writes { text })),
    (Some(text), None) => Ok(Some(Gitignore::Writes {
      text: format!("{text}\n"),
    })),
    (None, own) => Ok(own.map(|text| Gitignore::Kept { text })),
  }
}

/// The files a first commit of the folder at `root` adds, `.gitignore` the chat's text when it writes one.
fn added(runner: &dyn CommandRunner, root: &Path, gitignore: Option<&Gitignore>) -> Result<Vec<String>, ToolError> {
  let writes = match gitignore {
    Some(Gitignore::Writes { text }) => Some(text.as_str()),
    Some(Gitignore::Kept { .. }) | None => None,
  };
  let excluded = format!("{}{PROJECT_DATA}\n", writes.unwrap_or_default());
  let mut files = first_commit_files(runner, root, &excluded).map_err(|error| match error {
    SurveyError::GitMissing => refusal(InitError::GitMissing),
    SurveyError::Unreadable => refused("git_failed", "orchestrator.repository.unread"),
  })?;
  // The `.gitignore` written first is one of them.
  if writes.is_some() && !files.iter().any(|file| file == GITIGNORE) {
    files.push(GITIGNORE.to_owned());
    files.sort();
  }
  Ok(files)
}

/// Writes `text` as the folder's new `.gitignore`; one that appeared meanwhile is never written over.
fn write_gitignore(root: &Path, text: &str) -> Result<(), ToolError> {
  let written = fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(root.join(GITIGNORE))
    .and_then(|mut file| file.write_all(text.as_bytes()));
  match written {
    Ok(()) => Ok(()),
    Err(error) if error.kind() == ErrorKind::AlreadyExists => Err(refused(
      "changed_since_proposed",
      "orchestrator.repository.gitignoreAppeared",
    )),
    Err(_) => Err(refusal(InitError::NotWritable)),
  }
}

/// Why a folder that is not open to a first commit, as git read it, is refused.
fn not_open(local: &LocalRepository) -> ToolError {
  match local {
    LocalRepository::GitMissing => refusal(InitError::GitMissing),
    LocalRepository::Commits { .. } => refusal(InitError::HasCommits),
    LocalRepository::NotRepository | LocalRepository::NoCommits { .. } | LocalRepository::Unread => {
      refused("git_failed", "orchestrator.repository.unread")
    }
  }
}

/// The refusal of `error`, with a stable code: `git_not_found`, `folder_not_writable`, `has_commits`,
/// `staged_files`, `git_failed` in git's words, `author_unknown` with the `git config` commands that tell git the
/// author, `git_stopped`, and `changed_since_proposed` for a folder that changed while it was made.
fn refusal(error: InitError) -> ToolError {
  let said = |step: InitStep, text: String| {
    ToolError::refused(
      "git_failed",
      vec![
        Message::new("orchestrator.repository.gitFailed")
          .with("command", command(step).to_owned())
          .with("said", text),
      ],
    )
  };
  match error {
    InitError::GitMissing => refused("git_not_found", "orchestrator.repository.gitMissing"),
    InitError::NotWritable => refused("folder_not_writable", "orchestrator.repository.notWritable"),
    InitError::HasCommits => refused("has_commits", "orchestrator.repository.hasCommits"),
    InitError::AlreadyRepository | InitError::NotRepository => {
      refused("changed_since_proposed", "orchestrator.repository.changed")
    }
    InitError::Staged => refused("staged_files", "orchestrator.repository.staged"),
    InitError::InitFailed(text) => said(InitStep::Init, text),
    InitError::AddFailed(text) => said(InitStep::Add, text),
    InitError::CommitFailed(text) => said(InitStep::Commit, text),
    InitError::NoIdentity(text) => ToolError::refused(
      "author_unknown",
      vec![Message::new("orchestrator.repository.authorUnknown").with("said", text)],
    ),
    InitError::Stopped(step) => ToolError::refused(
      "git_stopped",
      vec![Message::new("orchestrator.repository.stopped").with("command", command(step).to_owned())],
    ),
  }
}

/// The git command of `step`, as people read it.
const fn command(step: InitStep) -> &'static str {
  match step {
    InitStep::Init => "git init",
    InitStep::Add => "git add",
    InitStep::Commit => "git commit",
  }
}

fn refused(code: &str, key: &'static str) -> ToolError {
  ToolError::refused(code, vec![Message::new(key)])
}
