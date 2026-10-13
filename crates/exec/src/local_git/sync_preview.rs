//! Compute the pending sync's content identity before Git touches the task checkout.
use std::collections::{BTreeMap, BTreeSet};

use orchester_core::integration::local::{Checkout, IndexEntry, LocalFile, LocalPath};

use super::{LocalError, LocalGit, LocalGitError, digest};

impl LocalGit {
  pub fn preview_sync(&self, before: &Checkout, base: &str) -> Result<Checkout, LocalGitError> {
    self.ensure(before)?;
    super::validate_object(base)?;
    if self.is_ancestor(base, &before.head)? {
      return Ok(before.clone());
    }
    let output = self.quiet_plumbing(
      &[
        "-c",
        "merge.conflictStyle=merge",
        "merge-tree",
        "--write-tree",
        "-z",
        "HEAD",
        base,
      ],
      None,
      None,
    )?;
    if !matches!(output.code, Some(0 | 1)) {
      return Err(LocalGitError::Refused);
    }
    let text = std::str::from_utf8(&output.bytes).map_err(|_| LocalGitError::Data)?;
    let mut parts = text.split('\0');
    let tree = parts.next().ok_or(LocalGitError::Data)?;
    super::validate_object(tree)?;
    let working = self.tree_entries(tree)?;
    let heads = self.tree_entries(&before.head)?;
    let mut entries = working
      .iter()
      .map(|(path, entry)| ((path.clone(), 0), entry.clone()))
      .collect::<BTreeMap<_, _>>();
    let mut conflicts = BTreeSet::new();
    for line in parts.take_while(|line| !line.is_empty()) {
      let (metadata, path) = line.split_once('\t').ok_or(LocalGitError::Data)?;
      let fields = metadata.split(' ').collect::<Vec<_>>();
      if fields.len() != 3 {
        return Err(LocalGitError::Data);
      }
      let path = LocalPath::new(path)?;
      let stage = fields[2].parse::<u8>().map_err(|_| LocalGitError::Data)?;
      if !matches!(stage, 1..=3) {
        return Err(LocalGitError::Data);
      }
      super::validate_object(fields[1])?;
      let entry = IndexEntry {
        mode: u32::from_str_radix(fields[0], 8).map_err(|_| LocalGitError::Data)?,
        object: fields[1].to_owned(),
      };
      entries.remove(&(path.clone(), 0));
      entries.insert((path.clone(), stage), entry);
      conflicts.insert(path);
    }
    let index = encode_index(&entries);
    let files = self.sync_files(tree, &heads, &working, &conflicts)?;
    let ignored = self.preview_ignored(tree)?;
    self.ensure(before)?;
    let mut result = before.clone();
    result.files = files;
    result.index = digest(&index);
    result.ignored = ignored;
    result.operation = Some(format!("MERGE_HEAD:{}", digest(format!("{base}\n").as_bytes())));
    Ok(result)
  }

  fn sync_files(
    &self,
    tree: &str,
    heads: &BTreeMap<LocalPath, IndexEntry>,
    working: &BTreeMap<LocalPath, IndexEntry>,
    conflicts: &BTreeSet<LocalPath>,
  ) -> Result<Vec<LocalFile>, LocalGitError> {
    let paths = heads
      .keys()
      .chain(working.keys())
      .chain(conflicts)
      .cloned()
      .collect::<BTreeSet<_>>();
    let mut files = Vec::new();
    for path in paths {
      let head = heads.get(&path).cloned();
      let staged = if conflicts.contains(&path) {
        None
      } else {
        working.get(&path).cloned()
      };
      if head == staged && !conflicts.contains(&path) {
        continue;
      }
      let entry = working.get(&path);
      if entry.is_some_and(|entry| !matches!(entry.mode, 0o100644 | 0o100755)) {
        return Err(LocalError::Unsupported.into());
      }
      let bytes = entry.map(|_| self.checkout_blob(tree, &path)).transpose()?;
      files.push(LocalFile {
        head,
        staged,
        working: bytes.as_deref().map(digest),
        executable: cfg!(unix) && entry.is_some_and(|entry| entry.mode == 0o100755),
        unstaged: conflicts.contains(&path),
        path,
      });
    }
    Ok(files)
  }
}

fn encode_index(entries: &BTreeMap<(LocalPath, u8), IndexEntry>) -> Vec<u8> {
  let mut bytes = Vec::new();
  for ((path, stage), entry) in entries {
    bytes.extend_from_slice(format!("{:06o} {} {stage}\t{path}\0", entry.mode, entry.object).as_bytes());
  }
  bytes
}
