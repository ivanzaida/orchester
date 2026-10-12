use orchester_core::integration::local::{IndexEntry, LocalConflict, LocalError, LocalPath, Placement, Resolution};

use super::{LocalGit, LocalGitError};

impl LocalGit {
  pub fn conflicts(&self) -> Result<Vec<LocalConflict>, LocalGitError> {
    let output = self.raw(&["ls-files", "--unmerged", "-z"], None, None)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let text = std::str::from_utf8(&output.bytes).map_err(|_| LocalGitError::Data)?;
    let mut files = std::collections::BTreeMap::<LocalPath, LocalConflict>::new();
    for line in text.split('\0').filter(|entry| !entry.is_empty()) {
      let (meta, path) = line.split_once('\t').ok_or(LocalGitError::Data)?;
      let fields: Vec<_> = meta.split(' ').collect();
      if fields.len() != 3 {
        return Err(LocalGitError::Data);
      }
      let path = LocalPath::new(path)?;
      let file = files.entry(path.clone()).or_insert(LocalConflict {
        path,
        ancestor: None,
        current: None,
        incoming: None,
      });
      let entry = IndexEntry {
        mode: u32::from_str_radix(fields[0], 8).map_err(|_| LocalGitError::Data)?,
        object: fields[1].to_owned(),
      };
      super::validate_object(&entry.object)?;
      match fields[2] {
        "1" => file.ancestor = Some(entry),
        "2" => file.current = Some(entry),
        "3" => file.incoming = Some(entry),
        _ => return Err(LocalGitError::Data),
      }
    }
    Ok(files.into_values().collect())
  }

  /// Each resolution compares its unmerged entries and current bytes, then stages just that file.
  pub fn resolve_task(
    &self,
    reviewed: &LocalConflict,
    expected: Option<&orchester_core::blob::ContentHash>,
    resolution: &Resolution,
  ) -> Result<(), LocalGitError> {
    if !self.conflicts()?.contains(reviewed) {
      return Err(LocalError::Stale.into());
    }
    let (content, entry) = self.resolution_bytes(reviewed, resolution)?;
    match resolution {
      Resolution::KeepBoth(destination) => {
        let incoming = reviewed.incoming.as_ref().ok_or(LocalError::Unsupported)?;
        let incoming_bytes = self.blob(&incoming.object)?;
        if self.file(destination)?.is_some() || self.index_entries()?.contains_key(destination) {
          return Err(LocalError::Collision.into());
        }
        super::write_regular(&self.root, destination, None, Some(&incoming_bytes))?;
        self.set_index(destination, Some(incoming), None)?;
      }
      Resolution::AtPath { destination, .. } if destination != &reviewed.path => {
        if self.file(destination)?.is_some() || self.index_entries()?.contains_key(destination) {
          return Err(LocalError::Collision.into());
        }
        super::write_regular(&self.root, destination, None, content.as_deref())?;
        self.set_index(destination, entry.as_ref(), None)?;
        super::write_regular(&self.root, &reviewed.path, expected, None)?;
        return self.set_index(&reviewed.path, None, None);
      }
      _ => {}
    }
    super::write_regular(&self.root, &reviewed.path, expected, content.as_deref())?;
    self.set_index(&reviewed.path, entry.as_ref(), None)?;
    Ok(())
  }

  pub fn resolution_bytes(
    &self,
    file: &LocalConflict,
    resolution: &Resolution,
  ) -> Result<(Option<Vec<u8>>, Option<IndexEntry>), LocalGitError> {
    if [&file.ancestor, &file.current, &file.incoming].iter().any(|entry| {
      entry
        .as_ref()
        .is_some_and(|entry| !matches!(entry.mode, 0o100644 | 0o100755))
    }) {
      return Err(LocalError::Unsupported.into());
    }
    let entry = match resolution {
      Resolution::Text(text) => {
        if has_markers(text) {
          return Err(LocalError::Unresolved.into());
        }
        let bytes = text.as_bytes().to_vec();
        let entry = IndexEntry {
          mode: file
            .current
            .as_ref()
            .or(file.incoming.as_ref())
            .map_or(0o100644, |entry| entry.mode),
          object: self.put_path_blob(&file.path, &bytes)?,
        };
        return Ok((Some(bytes), Some(entry)));
      }
      Resolution::Current | Resolution::KeepBoth(_) => file.current.as_ref(),
      Resolution::Incoming => file.incoming.as_ref(),
      Resolution::Delete => None,
      Resolution::AtPath { incoming, .. } => {
        if *incoming {
          file.incoming.as_ref()
        } else {
          file.current.as_ref()
        }
      }
    };
    if entry.is_some_and(|entry| !matches!(entry.mode, 0o100644 | 0o100755)) {
      return Err(LocalError::Unsupported.into());
    }
    let bytes = entry.map(|entry| self.blob(&entry.object)).transpose()?;
    Ok((bytes, entry.cloned()))
  }

  pub fn finish_task_resolution(&self, message: &str) -> Result<(), LocalGitError> {
    if !self.conflicts()?.is_empty() {
      return Err(LocalError::Unresolved.into());
    }
    let current = self.snapshot()?;
    for file in current.files {
      if self
        .file(&file.path)?
        .is_some_and(|bytes| std::str::from_utf8(&bytes).is_ok_and(has_markers))
      {
        return Err(LocalError::Unresolved.into());
      }
    }
    self.text(&["commit", "-m", message])?;
    Ok(())
  }

  pub fn place(&self, path: &LocalPath, entry: Option<&IndexEntry>, placement: Placement) -> Result<(), LocalGitError> {
    match placement {
      Placement::Stage => self.set_index(path, entry, None),
      Placement::LeaveUnstaged => {
        let head = self.tree_entries("HEAD")?;
        self.set_index(path, head.get(path), None)
      }
    }
  }

  /// Three-way text merge in operation scratch. Non-text and delete/modify results need explicit choices.
  pub fn merge_bytes(&self, base: &[u8], current: &[u8], incoming: &[u8]) -> Result<Option<Vec<u8>>, LocalGitError> {
    if current == base {
      return Ok(Some(incoming.to_vec()));
    }
    if incoming == base || current == incoming {
      return Ok(Some(current.to_vec()));
    }
    if [base, current, incoming]
      .iter()
      .any(|bytes| bytes.contains(&0) || std::str::from_utf8(bytes).is_err())
    {
      return Ok(None);
    }
    use std::io::Write;
    let mut versions = Vec::new();
    for bytes in [current, base, incoming] {
      let mut file = tempfile::NamedTempFile::new_in(&self.scratch).map_err(|_| LocalGitError::Io)?;
      file.write_all(bytes).map_err(|_| LocalGitError::Io)?;
      file.as_file().sync_all().map_err(|_| LocalGitError::Io)?;
      versions.push(file);
    }
    let names = versions
      .iter()
      .map(|file| {
        let path = file.path().to_str().ok_or(LocalGitError::Data)?;
        Ok(path.strip_prefix(r"\\?\").unwrap_or(path))
      })
      .collect::<Result<Vec<_>, LocalGitError>>()?;
    let output = self.raw(&["merge-file", "-p", names[0], names[1], names[2]], None, None)?;
    Ok((output.code == Some(0)).then_some(output.bytes))
  }
}

pub fn has_markers(text: &str) -> bool {
  text.lines().any(|line| {
    ["<<<<<<<", "=======", ">>>>>>>", "|||||||"]
      .iter()
      .any(|marker| line.starts_with(marker))
  })
}
