use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::Path,
};

use orchester_core::integration::local::{Checkout, IndexEntry, LocalFile, LocalPath};

use super::{LocalError, LocalGit, LocalGitError, digest, files::executable};

impl LocalGit {
  pub(super) fn capture(&self) -> Result<Checkout, LocalGitError> {
    let head = self.text(&["rev-parse", "--verify", "HEAD"])?;
    let branch = self.text(&["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    let git_dir = self.text(&["rev-parse", "--absolute-git-dir"])?;
    // Index cache timestamps may change during Check. Identity binds to content, modes and stages.
    let index = self.raw(&["ls-files", "--stage", "-z"], None, None)?;
    if index.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let staged = self.index_entries()?;
    let heads = self.tree_entries("HEAD")?;
    let status = self.fresh_status(&index.bytes)?;
    if status.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let unstaged_output = self.raw(
      &["diff", "--no-ext-diff", "--no-textconv", "--name-only", "-z", "--"],
      None,
      None,
    )?;
    if unstaged_output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let unstaged = std::str::from_utf8(&unstaged_output.bytes)
      .map_err(|_| LocalGitError::Data)?
      .split('\0')
      .filter(|path| !path.is_empty())
      .map(LocalPath::new)
      .collect::<Result<BTreeSet<_>, _>>()?;
    let actual_working = working_paths(&status.bytes)?;
    let files = changed_paths(&status.bytes)?
      .into_iter()
      .map(|path| {
        let working = self.file(&path)?;
        Ok(LocalFile {
          head: heads.get(&path).cloned(),
          staged: staged.get(&path).cloned(),
          working: working.as_deref().map(digest),
          executable: executable(&self.root, &path)?,
          unstaged: unstaged.contains(&path) || actual_working.contains(&path),
          path,
        })
      })
      .collect::<Result<_, LocalGitError>>()?;
    let ignored = self.raw(
      &["ls-files", "--others", "--ignored", "--exclude-standard", "-z"],
      None,
      None,
    )?;
    if ignored.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let ignored = ignored
      .bytes
      .split(|byte| *byte == 0)
      .filter(|path| !path.starts_with(b".orchester/") && !path.is_empty())
      .collect::<Vec<_>>();
    let mut ignored_bytes = Vec::new();
    for path in ignored {
      ignored_bytes.extend_from_slice(path);
      ignored_bytes.push(0);
    }
    let ignored = ignored_bytes;
    Ok(Checkout {
      root: self.root.to_str().ok_or(LocalGitError::Data)?.to_owned(),
      git_dir,
      branch,
      head,
      index: digest(&index.bytes),
      files,
      ignored: digest(&ignored),
      operation: self.operation()?,
    })
  }

  fn fresh_status(&self, entries: &[u8]) -> Result<super::process::GitOutput, LocalGitError> {
    // Rebuild cache metadata in an owned index so external timestamp-preserving writes cannot hide edits.
    let index = tempfile::Builder::new()
      .prefix("inspect-")
      .tempfile_in(&self.scratch)
      .map_err(|_| LocalGitError::Io)?
      .into_temp_path();
    fs::remove_file(&index).map_err(|_| LocalGitError::Io)?;
    self.index_command(&["read-tree", "--empty"], None, &index)?;
    let copied = self.quiet_index(&["update-index", "-z", "--index-info"], Some(entries), &index)?;
    if copied.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    self.quiet_index(
      &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
      None,
      &index,
    )
  }

  pub fn operation(&self) -> Result<Option<String>, LocalGitError> {
    for marker in [
      "index.lock",
      "rebase-merge",
      "rebase-apply",
      "CHERRY_PICK_HEAD",
      "REVERT_HEAD",
      "BISECT_START",
      "MERGE_HEAD",
    ] {
      let path = self.text(&["rev-parse", "--path-format=absolute", "--git-path", marker])?;
      let path = Path::new(&path);
      if path.exists() {
        let identity = if path.is_file() {
          digest(&fs::read(path).map_err(|_| LocalGitError::Io)?).to_string()
        } else {
          String::new()
        };
        return Ok(Some(format!("{marker}:{identity}")));
      }
    }
    Ok(None)
  }

  pub fn index_entries(&self) -> Result<BTreeMap<LocalPath, IndexEntry>, LocalGitError> {
    let flags = self.raw(&["ls-files", "-v", "-z"], None, None)?;
    if flags.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    if flags.bytes.split(|byte| *byte == 0).any(|entry| {
      entry
        .first()
        .is_some_and(|flag| *flag == b'S' || flag.is_ascii_lowercase())
    }) {
      return Err(LocalError::Unsupported.into());
    }
    let output = self.raw(&["ls-files", "--stage", "-z"], None, None)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    parse_entries(&output.bytes, true)
  }

  pub fn tree_entries(&self, tree: &str) -> Result<BTreeMap<LocalPath, IndexEntry>, LocalGitError> {
    let output = self.raw(&["ls-tree", "-r", "-z", tree], None, None)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    parse_entries(&output.bytes, false)
  }
}

pub(super) fn parse_entries(bytes: &[u8], index: bool) -> Result<BTreeMap<LocalPath, IndexEntry>, LocalGitError> {
  let text = std::str::from_utf8(bytes).map_err(|_| LocalGitError::Data)?;
  let mut entries = BTreeMap::new();
  for entry in text.split('\0').filter(|entry| !entry.is_empty()) {
    let (metadata, path) = entry.split_once('\t').ok_or(LocalGitError::Data)?;
    let fields: Vec<_> = metadata.split(' ').collect();
    if fields.len() != 3 {
      return Err(LocalGitError::Data);
    }
    if index && fields[2] != "0" {
      continue;
    }
    let mode = u32::from_str_radix(fields[0], 8).map_err(|_| LocalGitError::Data)?;
    let object = fields[if index { 1 } else { 2 }].to_owned();
    super::validate_object(&object)?;
    entries.insert(LocalPath::new(path)?, IndexEntry { mode, object });
  }
  Ok(entries)
}

fn changed_paths(bytes: &[u8]) -> Result<BTreeSet<LocalPath>, LocalGitError> {
  let text = std::str::from_utf8(bytes).map_err(|_| LocalGitError::Data)?;
  let mut entries = text.split('\0').filter(|entry| !entry.is_empty());
  let mut paths = BTreeSet::new();
  while let Some(entry) = entries.next() {
    let code = entry.get(..2).ok_or(LocalGitError::Data)?;
    paths.insert(LocalPath::new(entry.get(3..).ok_or(LocalGitError::Data)?)?);
    if code.contains(['R', 'C']) {
      paths.insert(LocalPath::new(entries.next().ok_or(LocalGitError::Data)?)?);
    }
  }
  Ok(paths)
}
fn working_paths(bytes: &[u8]) -> Result<BTreeSet<LocalPath>, LocalGitError> {
  let text = std::str::from_utf8(bytes).map_err(|_| LocalGitError::Data)?;
  let mut entries = text.split('\0').filter(|entry| !entry.is_empty());
  let mut paths = BTreeSet::new();
  while let Some(entry) = entries.next() {
    let code = entry.get(..2).ok_or(LocalGitError::Data)?;
    let path = LocalPath::new(entry.get(3..).ok_or(LocalGitError::Data)?)?;
    if code != "??" && code.as_bytes()[1] != b' ' {
      paths.insert(path);
    }
    if code.contains(['R', 'C']) {
      let _old_path = entries.next().ok_or(LocalGitError::Data)?;
    }
  }
  Ok(paths)
}
