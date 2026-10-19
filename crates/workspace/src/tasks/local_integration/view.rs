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

use super::{
  IntegrationError, LocalIntegration,
  stats::{ShownSnapshot, Snapshot, SnapshotStats},
};

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
  pub draft: Option<orchester_core::integration::local::EditorCandidate>,
  pub draft_stale: bool,
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
  pub stats: SnapshotStats,
  /// What the external evidence was read from, so a later read can keep it while these are unchanged.
  pub evidence_inputs: super::external::EvidenceInputs,
  /// Restored paths changed since their restoration was verified: the person's own edits, which cleanup leaves.
  pub changed_since_restoration: Vec<LocalPath>,
}

impl IntegrationView {
  /// Whether `selection` can be reviewed as a commit: every path is listed and has staged or new content. A path
  /// changed only in the working copy has nothing in the index for a commit to take.
  pub fn commit_selectable(&self, selection: &[LocalPath]) -> bool {
    !selection.is_empty()
      && selection.iter().all(|path| {
        self
          .checkout
          .files
          .iter()
          .find(|file| file.path == *path)
          .is_some_and(|file| !file.only_unstaged())
      })
  }
}

/// Reads actual checkout and retained journal independently of any worker lease, so progress stays observable.
pub fn read(
  root: &Path,
  run: RunId,
  selected: Option<&ShownSnapshot>,
  vault: &dyn SecretVault,
) -> Result<IntegrationView, IntegrationError> {
  read_after(root, run, (selected, vault), None)
}

/// [`read`], keeping the external evidence of `previous` while what it was read from is unchanged.
pub fn read_after(
  root: &Path,
  run: RunId,
  (selected, vault): (Option<&ShownSnapshot>, &dyn SecretVault),
  previous: Option<&IntegrationView>,
) -> Result<IntegrationView, IntegrationError> {
  let scratch =
    orchester_exec::local_git::prepare_scratch(root, &[".orchester", "local-integration", &run.to_string()])?;
  let _store_owner = orchester_exec::local_git::owned::StoreOwnership::acquire(root)?;
  let mut db = ProjectDb::open(root)?;
  let main = LocalGit::new(root, &scratch)?;
  super::ensure_project_git(&main)?;
  let checkout = main.snapshot()?;
  let mut operation = db.local_operation(run)?;
  use orchester_core::store::RunStore;
  let saved_run = db
    .run(run)
    .map_err(|_| IntegrationError::Run)?
    .ok_or(IntegrationError::Run)?;
  let evidence_inputs = super::external::EvidenceInputs::read(&db, &main, &checkout, &saved_run, operation.as_ref())?;
  let external = match previous {
    Some(previous) if previous.evidence_inputs == evidence_inputs => previous.external.clone(),
    _ => super::external::evidence(&main, &evidence_inputs)?,
  };
  if let (Some(evidence), Some(saved)) = (&external, operation.as_mut()) {
    super::direct::settle_outside(root, &mut db, saved, evidence);
  }
  let redact = Redaction::load(&db, vault)?;
  redact.ensure_metadata(&checkout)?;
  redact.ensure_metadata(&operation)?;
  redact.ensure_text_metadata(saved_run.branch().as_str())?;
  if let Some(shown) = selected {
    redact.ensure_text_metadata(shown.path.as_str())?;
  }
  let task_root = orchester_exec::local_git::owned_directory(root, &[".orchester", "worktrees", &run.to_string()])?;
  let conflicts = read_conflicts(&db, operation.as_ref(), &checkout, &main, task_root.as_deref(), &redact)?;
  let diff = read_diff(&main, operation.as_ref(), selected, &redact)?;
  // Sizes serve choosing what to commit or preserve; later phases skip the extra Git reads of each refresh.
  let reviewing = operation.as_ref().is_none_or(|op| op.phase() == LocalPhase::Review);
  let stats = if reviewing && !checkout.files.is_empty() {
    SnapshotStats::read(&main, &checkout)?
  } else {
    SnapshotStats::default()
  };
  let checked_current = checked_current(root, &checkout, operation.as_ref(), task_root.as_deref())?;
  let changed_since_restoration = match &operation {
    Some(saved) if matches!(saved.phase(), LocalPhase::RestorationVerified | LocalPhase::Canceled) => {
      super::restore::changed_since_restoration(&main, saved)?
    }
    _ => Vec::new(),
  };
  Ok(IntegrationView {
    evidence_inputs,
    changed_since_restoration,
    editor_policy: EditorPolicy(std::sync::Arc::new(redact)),
    checked_current,
    external,
    operation,
    checkout,
    conflicts,
    diff,
    stats,
  })
}

impl LocalIntegration {
  pub fn candidate_sensitive(
    &self,
    candidate: &orchester_core::integration::local::EditorCandidate,
    vault: &dyn SecretVault,
  ) -> Result<bool, IntegrationError> {
    let policy = EditorPolicy(std::sync::Arc::new(Redaction::load(&self.db, vault)?));
    Ok(policy.sensitive(candidate.text()) || policy.sensitive(candidate.destination()))
  }
}

fn read_conflicts(
  db: &ProjectDb,
  operation: Option<&LocalOperation>,
  checkout: &Checkout,
  main: &LocalGit,
  task_root: Option<&Path>,
  redact: &Redaction,
) -> Result<Vec<ConflictView>, IntegrationError> {
  let mut conflicts = Vec::new();
  if let Some(operation) = operation {
    let files = match operation.phase() {
      LocalPhase::TaskConflicts => {
        let task_root = task_root.ok_or(IntegrationError::Run)?;
        let task_scratch = orchester_exec::local_git::prepare_scratch(task_root, &[".orchester", "local-integration"])?;
        let task = LocalGit::new(task_root, &task_scratch)?;
        let snapshot = task.snapshot()?;
        for file in task.conflicts()? {
          redact.ensure_metadata(&file)?;
          let mut shown = conflict(&task, file, redact)?;
          attach_draft(db, operation.run(), operation, checkout, Some(&snapshot), &mut shown)?;
          conflicts.push(shown);
        }
        Vec::new()
      }
      LocalPhase::RestoreConflicts => super::restore::restoration_files(db, operation, main)?,
      _ => Vec::new(),
    };
    for file in files {
      redact.ensure_metadata(&file)?;
      let mut shown = conflict(main, file, redact)?;
      attach_draft(db, operation.run(), operation, checkout, None, &mut shown)?;
      conflicts.push(shown);
    }
  }

  Ok(conflicts)
}
fn read_diff(
  main: &LocalGit,
  operation: Option<&LocalOperation>,
  selected: Option<&ShownSnapshot>,
  redact: &Redaction,
) -> Result<Preview, IntegrationError> {
  if let Some(exact) = operation.and_then(LocalOperation::checked) {
    redact.ensure_tree_metadata(main, &exact.result.head)?;
    let diff = main.text(&[
      "diff",
      "--no-color",
      "--no-ext-diff",
      "--no-textconv",
      &exact.base.head,
      &exact.result.head,
      "--",
    ])?;
    return Ok(preview(
      Some(diff.as_bytes()),
      &LocalPath::new("checked-result")?,
      redact,
    ));
  }
  let Some(ShownSnapshot { path, snapshot }) = selected else {
    return Ok(Preview::Absent);
  };
  Ok(match snapshot {
    Snapshot::Untracked => preview(main.file(path)?.as_deref(), path, redact),
    Snapshot::Staged | Snapshot::Unstaged => {
      let diff = main.diff(*snapshot == Snapshot::Staged, std::slice::from_ref(path))?;
      preview(Some(diff.as_bytes()), path, redact)
    }
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
    draft: None,
    draft_stale: false,
  })
}

pub(super) struct Redaction {
  secrets: Vec<orchester_core::secret::SecretValue>,
  available: bool,
}

impl Redaction {
  pub(super) fn ensure_tree_metadata(&self, git: &LocalGit, tree: &str) -> Result<(), IntegrationError> {
    for path in git.tree_entries(tree)?.keys() {
      self.ensure_text_metadata(path.as_str())?;
    }
    Ok(())
  }

  pub(super) fn ensure_metadata(&self, value: &impl serde::Serialize) -> Result<(), IntegrationError> {
    let text = serde_json::to_string(value).map_err(|_| IntegrationError::SensitiveMetadata)?;
    self.ensure_text_metadata(&text)
  }

  pub(super) fn ensure_text_metadata(&self, text: &str) -> Result<(), IntegrationError> {
    if !self.available
      || self.secrets.iter().any(|value| {
        !value.expose().is_empty() && (text.contains(value.expose()) || escaped_secret(text, value.expose()))
      })
    {
      return Err(IntegrationError::SensitiveMetadata);
    }
    Ok(())
  }

  pub(super) fn load(db: &ProjectDb, vault: &dyn SecretVault) -> Result<Self, IntegrationError> {
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
    redact.ensure_metadata(&self.operation)?;
    redact.ensure_tree_metadata(&self.main, tree)?;
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
    redact.ensure_metadata(&self.operation)?;
    redact.ensure_tree_metadata(&self.main, &exact.result.head)?;
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
  let _store_owner = orchester_exec::local_git::owned::StoreOwnership::acquire(root)?;
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

fn attach_draft(
  db: &ProjectDb,
  run: RunId,
  operation: &LocalOperation,
  main: &Checkout,
  task: Option<&Checkout>,
  shown: &mut ConflictView,
) -> Result<(), IntegrationError> {
  use orchester_core::integration::local::{ConflictKind, DraftIdentity};
  let kind = if task.is_some() {
    ConflictKind::Task
  } else {
    ConflictKind::Restoration
  };
  let identity = DraftIdentity {
    kind,
    main: main.clone(),
    task: task.cloned(),
    file: shown.file.clone(),
    expected: shown.expected.clone(),
  };
  if let Some(saved) = operation
    .editor_drafts()
    .iter()
    .find(|saved| saved.identity == identity)
  {
    shown.draft = Some(super::drafts::read_candidate(db, run, saved)?);
  } else {
    shown.draft_stale = operation
      .editor_drafts()
      .iter()
      .any(|saved| saved.identity.kind == kind && saved.identity.file.path == shown.file.path);
  }
  Ok(())
}
