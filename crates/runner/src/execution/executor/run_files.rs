//! The files of a run's worktree for the desktop's code viewer ([`orchester_ipc::Command::RunFiles`],
//! `docs/architecture/remote-runners.md`, rule 5): the executor reads them where the worktree is, so the desktop never
//! opens the worktree's folder for them. A file is checked with the file scope agents' file tools use
//! (`FileScope::repository_readable`): a link or junction that leads out of the worktree, and `.git` and `.orchester`,
//! are refused. A file above [`MAX_VIEW_BYTES`] is measured, not read; the desktop tells text from binary in the bytes.

use std::{
  collections::HashSet,
  fs, io,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  command::{CommandError, CommandRunner},
  diff::git::git_command,
  id::RunId,
  native::scope::{FileScope, ScopeError, ScopeRoots},
  permission::RepositoryAccess,
  repository::redact_userinfo,
};
use orchester_exec::ProcessRunner;
use orchester_ipc::{
  Reply,
  run_files::{
    FileBytes, MAX_LIST_PAGE_LEN, MAX_VIEW_BYTES, RunFileFailure, RunFileRead, RunFilesReply, RunFilesRequest,
  },
};

use crate::{execution::Scheduler, execution_root::ExecutionRoot};

/// A git listing that does not answer within this time fails, as the desktop's quick git reads do.
const GIT_TIMEOUT: Duration = Duration::from_secs(30);

impl Scheduler {
  /// The answer to [`orchester_ipc::Command::RunFiles`]: `request` about the worktree of `run`, read off the runner's
  /// tasks, since it blocks on the file system and git.
  pub async fn run_files(self, run: RunId, request: RunFilesRequest) -> Reply {
    let root = self.inner.project_root.clone();
    let answered = tokio::task::spawn_blocking(move || answer_run_files(&root, run, request))
      .await
      .unwrap_or_else(|error| {
        tracing::error!(%run, %error, "reading a run's files ended abnormally");
        RunFilesReply::Failed(RunFileFailure::Unreadable {
          kind: io::ErrorKind::Other.into(),
        })
      });
    Reply::RunFiles(answered)
  }
}

/// The answer to `request` about the worktree of `run` of the project at `project_root`, its execution root here.
/// Blocks on the file system and git. The desktop's window tests, which start no runner, answer with it in-process.
pub fn answer_run_files(project_root: &Path, run: RunId, request: RunFilesRequest) -> RunFilesReply {
  let worktree = ExecutionRoot::local(project_root).run_worktree(run);
  if !worktree.is_dir() {
    return RunFilesReply::Gone;
  }
  let worktree = worktree.as_path();
  let answered = match request {
    RunFilesRequest::Present => Ok(RunFilesReply::Present),
    RunFilesRequest::List { from } => list(worktree).map(|files| list_page(files, from, MAX_LIST_PAGE_LEN)),
    RunFilesRequest::Read { path } => read(worktree, &path).map(RunFilesReply::Read),
  };
  answered.unwrap_or_else(RunFilesReply::Failed)
}

/// The page of `files` that starts at the `from`th: as many as fit `budget` bytes of JSON, and at least one, so a
/// listing however long is answered in replies that each fit a frame.
pub fn list_page(files: Vec<String>, from: u64, budget: usize) -> RunFilesReply {
  let from = usize::try_from(from).unwrap_or(usize::MAX);
  let mut page = Vec::new();
  let mut used = 0;
  let mut rest = files.into_iter().skip(from).peekable();
  while let Some(file) = rest.peek() {
    // A name's JSON: itself escaped, its quotes and the comma after it.
    let len = serde_json::to_string(file).map_or(file.len() * 6 + 2, |json| json.len()) + 1;
    if !page.is_empty() && used + len > budget {
      break;
    }
    used += len;
    page.extend(rest.next());
  }
  let next = rest
    .peek()
    .map(|_| u64::try_from(from.saturating_add(page.len())).unwrap_or(u64::MAX));
  RunFilesReply::Listed { files: page, next }
}

/// The files git tracks in `worktree` and the ones it does not ignore, without tracked files that were deleted.
fn list(worktree: &Path) -> Result<Vec<String>, RunFileFailure> {
  let git = ProcessRunner::new(GIT_TIMEOUT);
  let listed = |args: &[&str]| -> Result<Vec<String>, RunFileFailure> {
    let output = git
      .run(&git_command(Path::new("git"), worktree, args.iter().copied()))
      .map_err(|error| match error {
        CommandError::NotFound => RunFileFailure::GitMissing,
        other => RunFileFailure::Git {
          detail: other.to_string(),
        },
      })?;
    if !output.success() {
      return Err(RunFileFailure::Git {
        detail: redact_userinfo(output.stderr.trim()),
      });
    }
    Ok(
      output
        .stdout
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect(),
    )
  };
  let deleted: HashSet<String> = listed(&["ls-files", "-z", "--deleted"])?.into_iter().collect();
  let mut files = listed(&["ls-files", "-z", "--cached", "--others", "--exclude-standard"])?;
  files.retain(|path| !deleted.contains(path));
  Ok(files)
}

/// The file `path` of `worktree`: its bytes, or its size when it is too large to show.
fn read(worktree: &Path, path: &str) -> Result<RunFileRead, RunFileFailure> {
  let resolved = resolve(worktree, path)?;
  let unreadable = |error: io::Error| match error.kind() {
    io::ErrorKind::NotFound => RunFileFailure::Missing,
    kind => RunFileFailure::Unreadable { kind: kind.into() },
  };
  let metadata = fs::metadata(&resolved).map_err(unreadable)?;
  if metadata.is_dir() {
    return Err(RunFileFailure::Folder);
  }
  if !metadata.is_file() {
    return Err(RunFileFailure::Unreadable {
      kind: io::ErrorKind::Unsupported.into(),
    });
  }
  if metadata.len() > MAX_VIEW_BYTES {
    return Ok(RunFileRead::TooLarge { size: metadata.len() });
  }
  let bytes = fs::read(&resolved).map_err(unreadable)?;
  Ok(RunFileRead::Bytes(FileBytes { bytes }))
}

/// Where `path` lies in `worktree` with its links resolved, or why it may not be read.
fn resolve(worktree: &Path, path: &str) -> Result<PathBuf, RunFileFailure> {
  let scope = FileScope::new(&ScopeRoots {
    worktree,
    access: RepositoryAccess::ReadOnly,
    code_index: None,
    skills: None,
    writable_folders: &[],
    run_storage: None,
  })
  .map_err(|error| refused(&error))?;
  scope.repository_readable(path).map_err(|error| refused(&error))
}

fn refused(error: &ScopeError) -> RunFileFailure {
  match error {
    ScopeError::NotReadable(_) | ScopeError::NetworkPath(_) => RunFileFailure::Outside,
    ScopeError::Protected(_) => RunFileFailure::Hidden,
    ScopeError::Unresolved { kind, .. } => RunFileFailure::Unreadable { kind: (*kind).into() },
    ScopeError::Nul
    | ScopeError::DevicePath(_)
    | ScopeError::DeviceName(_)
    | ScopeError::AlternateStream(_)
    | ScopeError::DriveRelative(_)
    | ScopeError::NotWritable(_) => RunFileFailure::Invalid,
  }
}
