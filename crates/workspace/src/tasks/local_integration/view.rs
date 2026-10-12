use std::path::Path;

use orchester_core::{
  blob::ContentHash,
  id::RunId,
  integration::local::{Checkout, LocalConflict, LocalIntegrationStore, LocalOperation, LocalPath, LocalPhase},
  secret::{SecretVault, VaultKey},
  store::SecretStore,
};
use orchester_exec::local_git::LocalGit;
use orchester_store::ProjectDb;

use super::{IntegrationError, LocalIntegration};

/// Only safe display content crosses into the visual layer. Sensitive content is unavailable for text editing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Preview {
  Text(String),
  Binary,
  Sensitive,
  Absent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictView {
  pub file: LocalConflict,
  pub expected: Option<ContentHash>,
  pub ancestor: Preview,
  pub current: Preview,
  pub incoming: Preview,
  pub result: Preview,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntegrationView {
  pub operation: Option<LocalOperation>,
  pub checkout: Checkout,
  pub conflicts: Vec<ConflictView>,
  pub diff: Preview,
  pub checked_current: bool,
  pub editor_policy: EditorPolicy,
  pub external: Option<orchester_core::integration::local::ContainedResult>,
}

/// Reads actual checkout and retained journal independently of any worker lease, so progress stays observable.
pub fn read(
  root: &Path,
  run: RunId,
  selected: Option<&LocalPath>,
  vault: &dyn SecretVault,
) -> Result<IntegrationView, IntegrationError> {
  let scratch =
    orchester_exec::local_git::prepare_scratch(root, &[".orchester", "local-integration", &run.to_string()])?;
  let db = ProjectDb::open(root)?;
  let main = LocalGit::new(root, &scratch)?;
  super::ensure_project_git(&main)?;
  let checkout = main.snapshot()?;
  let operation = db.local_operation(run)?;
  use orchester_core::store::RunStore;
  let saved_run = db
    .run(run)
    .map_err(|_| IntegrationError::Run)?
    .ok_or(IntegrationError::Run)?;
  let external = super::external::contained_result(&db, &main, &checkout, &saved_run, operation.as_ref())?;
  let redact = Redaction::load(&db, vault)?;
  let task_root = orchester_exec::local_git::owned_directory(root, &[".orchester", "worktrees", &run.to_string()])?;
  let mut conflicts = Vec::new();
  if let Some(operation) = &operation {
    let files = match operation.phase() {
      LocalPhase::TaskConflicts => {
        let task_root = task_root.as_ref().ok_or(IntegrationError::Run)?;
        let task_scratch = orchester_exec::local_git::prepare_scratch(task_root, &[".orchester", "local-integration"])?;
        let task = LocalGit::new(task_root, &task_scratch)?;
        for file in task.conflicts()? {
          conflicts.push(conflict(&task, file, &redact)?);
        }
        Vec::new()
      }
      LocalPhase::RestoreConflicts => {
        let session = LocalIntegration::open(root, run)?;
        session.restore_conflicts()?
      }
      _ => Vec::new(),
    };
    for file in files {
      conflicts.push(conflict(&main, file, &redact)?);
    }
  }
  let diff = if let Some(exact) = operation.as_ref().and_then(LocalOperation::checked) {
    let diff = main.text(&[
      "diff",
      "--no-color",
      "--no-ext-diff",
      "--no-textconv",
      &exact.base.head,
      &exact.result.head,
      "--",
    ])?;
    preview(Some(diff.as_bytes()), &LocalPath::new("checked-result")?, &redact)
  } else {
    match selected {
      Some(path) => {
        if checkout
          .files
          .iter()
          .any(|file| file.path == *path && file.head.is_none())
        {
          preview(main.file(path)?.as_deref(), path, &redact)
        } else {
          let diff = main.diff(true, std::slice::from_ref(path))?;
          let working = main.diff(false, std::slice::from_ref(path))?;
          preview(Some(format!("{diff}\n{working}").as_bytes()), path, &redact)
        }
      }
      None => Preview::Absent,
    }
  };
  let checked_current = checked_current(root, &checkout, operation.as_ref(), task_root.as_deref())?;
  Ok(IntegrationView {
    editor_policy: EditorPolicy(std::sync::Arc::new(redact)),
    checked_current,
    external,
    operation,
    checkout,
    conflicts,
    diff,
  })
}

fn conflict(git: &LocalGit, file: LocalConflict, redact: &Redaction) -> Result<ConflictView, IntegrationError> {
  let version = |entry: &Option<orchester_core::integration::local::IndexEntry>| -> Result<Preview, IntegrationError> {
    let bytes = entry.as_ref().map(|entry| git.blob(&entry.object)).transpose()?;
    Ok(preview(bytes.as_deref(), &file.path, redact))
  };
  let result = git.file(&file.path)?;
  Ok(ConflictView {
    ancestor: version(&file.ancestor)?,
    current: version(&file.current)?,
    incoming: version(&file.incoming)?,
    result: preview(result.as_deref(), &file.path, redact),
    expected: result.as_deref().map(orchester_exec::local_git::digest),
    file,
  })
}

struct Redaction {
  secrets: Vec<orchester_core::secret::SecretValue>,
  available: bool,
}

impl Redaction {
  fn load(db: &ProjectDb, vault: &dyn SecretVault) -> Result<Self, IntegrationError> {
    let mut redact = Self {
      secrets: Vec::new(),
      available: true,
    };
    for secret in db.secrets()? {
      match vault.read(&VaultKey::new(db.project().id(), secret.id())) {
        Ok(value) => redact.secrets.push(value),
        Err(_) => redact.available = false,
      }
    }
    Ok(redact)
  }
}

fn preview(bytes: Option<&[u8]>, path: &LocalPath, redact: &Redaction) -> Preview {
  let Some(bytes) = bytes else {
    return Preview::Absent;
  };
  let Ok(text) = std::str::from_utf8(bytes) else {
    return Preview::Binary;
  };
  if bytes.contains(&0) {
    return Preview::Binary;
  }
  let sensitive = path.as_str().split('/').any(sensitive_name);
  let sensitive_diff = text
    .lines()
    .filter(|line| line.starts_with("diff --git ") || line.starts_with("+++ ") || line.starts_with("--- "))
    .any(|line| line.split(['/', ' ', '"', '\t']).any(sensitive_name));
  if sensitive
    || sensitive_diff
    || !redact.available
    || redact.secrets.iter().any(|value| {
      !value.expose().is_empty()
        && (text.contains(value.expose())
          || path.as_str().contains(value.expose())
          || escaped_secret(text, value.expose()))
    })
  {
    return Preview::Sensitive;
  }
  Preview::Text(text.to_owned())
}

impl LocalIntegration {
  pub fn selected_commit_preview(&self, tree: &str, vault: &dyn SecretVault) -> Result<Preview, IntegrationError> {
    let diff = self.main.text(&[
      "diff",
      "--no-ext-diff",
      "--no-textconv",
      &self.operation.reviewed().head,
      tree,
      "--",
    ])?;
    let redact = Redaction::load(&self.db, vault)?;
    Ok(preview(
      Some(diff.as_bytes()),
      &LocalPath::new("selected-index")?,
      &redact,
    ))
  }

  pub fn checked_diff(&self, vault: &dyn SecretVault) -> Result<Preview, IntegrationError> {
    let exact = self.operation.checked().ok_or(super::LocalError::NotChecked)?;
    let diff = self.main.text(&[
      "diff",
      "--no-ext-diff",
      "--no-textconv",
      &exact.base.head,
      &exact.result.head,
      "--",
    ])?;
    let redact = Redaction::load(&self.db, vault)?;
    Ok(preview(
      Some(diff.as_bytes()),
      &LocalPath::new("checked-result")?,
      &redact,
    ))
  }
}
fn sensitive_name(part: &str) -> bool {
  let part = part.to_ascii_lowercase();
  part == ".env" || part.starts_with(".env.") || part.ends_with(".pem") || part.ends_with(".key")
}
fn escaped_secret(text: &str, secret: &str) -> bool {
  let Ok(encoded) = serde_json::to_string(secret) else {
    return true;
  };
  let Some(value) = encoded.strip_prefix('"').and_then(|value| value.strip_suffix('"')) else {
    return true;
  };
  !value.is_empty() && text.contains(value)
}
pub fn pending_restoration(root: &Path, run: RunId) -> Result<Option<LocalPhase>, IntegrationError> {
  orchester_exec::local_git::owned_store(root)?;
  let saved = ProjectDb::open(root)?.local_operation(run)?;
  Ok(
    saved
      .filter(|operation| operation.merged().is_some() && operation.phase() != LocalPhase::Complete)
      .map(|operation| operation.phase()),
  )
}
#[derive(Clone)]
pub struct EditorPolicy(std::sync::Arc<Redaction>);
impl std::fmt::Debug for EditorPolicy {
  fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    out.write_str("EditorPolicy(<redacted>)")
  }
}
impl PartialEq for EditorPolicy {
  fn eq(&self, other: &Self) -> bool {
    self.0.available == other.0.available
      && self.0.secrets.len() == other.0.secrets.len()
      && self
        .0
        .secrets
        .iter()
        .zip(&other.0.secrets)
        .all(|(a, b)| a.expose() == b.expose())
  }
}
impl Eq for EditorPolicy {}
impl EditorPolicy {
  /// Sensitive drafts stay in memory until explicitly cleared; no native text field exposes them.
  pub fn sensitive(&self, text: &str) -> bool {
    !self.0.available
      || self.0.secrets.iter().any(|value| {
        !value.expose().is_empty() && (text.contains(value.expose()) || escaped_secret(text, value.expose()))
      })
  }
}

fn checked_current(
  root: &Path,
  checkout: &Checkout,
  operation: Option<&LocalOperation>,
  task_root: Option<&Path>,
) -> Result<bool, IntegrationError> {
  Ok(if let Some(exact) = operation.and_then(LocalOperation::checked) {
    if let Some(task_root) = task_root {
      orchester_exec::local_git::owned_store(root)?;
      let scratch = orchester_exec::local_git::prepare_scratch(task_root, &[".orchester", "local-integration"])?;
      LocalGit::new(task_root, &scratch)?.snapshot()? == exact.result && *checkout == exact.base
    } else {
      false
    }
  } else {
    true
  })
}
