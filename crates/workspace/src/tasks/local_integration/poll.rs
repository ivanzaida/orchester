//! What a review re-reads every few seconds instead of the whole view: the main checkout's status fingerprint, the
//! run's journal record, its branch tip, the tip of its delivery target and its recorded merge, and the task
//! worktree's status while the view shows it. When the fingerprint is unchanged, the view read with it is still
//! current. Nothing here hashes a file: an edit Git's stat cache hides is found by the hashed check an operation
//! takes, and named after its refusal ([`super::changed_since_review`]).
use std::path::{Path, PathBuf};

use orchester_core::{
  blob::ContentHash,
  id::RunId,
  integration::local::{LocalIntegrationStore, LocalOperation, LocalPhase},
  store::{MergeStore, RunStore},
};
use orchester_exec::local_git::{LocalGit, digest};
use orchester_store::ProjectDb;

use super::IntegrationError;

pub fn fingerprint(root: &Path, run: RunId) -> Result<ContentHash, IntegrationError> {
  let scratch =
    orchester_exec::local_git::prepare_scratch(root, &[".orchester", "local-integration", &run.to_string()])?;
  let _store_owner = orchester_exec::local_git::owned::StoreOwnership::acquire(root)?;
  let db = ProjectDb::open(root)?;
  let main = LocalGit::new(root, &scratch)?;
  let operation = db.local_operation(run)?;
  let saved_run = db
    .run(run)
    .map_err(|_| IntegrationError::Run)?
    .ok_or(IntegrationError::Run)?;
  let mut parts = Vec::new();
  parts.extend_from_slice(main.status_fingerprint()?.as_str().as_bytes());
  parts.extend(serde_json::to_vec(&operation).expect("a journal record is plain data, which always serializes"));
  parts.extend(format!("{:?}", main.branch_tip(saved_run.branch())?).as_bytes());
  // A merge into the delivery target while another branch is checked out changes what the page says, too.
  let target = saved_run
    .base_branch()
    .map(|branch| main.branch_tip(branch))
    .transpose()?;
  parts.extend(format!("{target:?}").as_bytes());
  parts.extend(format!("{:?}", db.run_merge(run)?.map(|merge| merge.revision)).as_bytes());
  parts.extend(
    format!(
      "{:?}",
      saved_run.last_checkpoint().map(|saved| saved.revision().clone())
    )
    .as_bytes(),
  );
  if let Some(task_root) = shown_task_root(root, run, operation.as_ref())? {
    let task_scratch = orchester_exec::local_git::prepare_scratch(&task_root, &[".orchester", "local-integration"])?;
    let task = LocalGit::new(&task_root, &task_scratch)?;
    parts.extend_from_slice(task.status_fingerprint()?.as_str().as_bytes());
  }
  Ok(digest(&parts))
}

/// The run's worktree, looked for only while the page shows something of it: the conflicts of a sync, or whether a
/// checked result is still the worktree's. Every other read takes the run's result from its branch in the
/// repository and leaves the worktree folder alone (docs/architecture/remote-runners.md, "Local integration").
pub(super) fn shown_task_root(
  root: &Path,
  run: RunId,
  operation: Option<&LocalOperation>,
) -> Result<Option<PathBuf>, IntegrationError> {
  let shows_task =
    operation.is_some_and(|saved| saved.checked().is_some() || saved.phase() == LocalPhase::TaskConflicts);
  if !shows_task {
    return Ok(None);
  }
  Ok(orchester_exec::local_git::owned_directory(
    root,
    &[".orchester", "worktrees", &run.to_string()],
  )?)
}
