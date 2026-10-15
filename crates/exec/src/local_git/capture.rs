use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
};

use orchester_core::{
  blob::ContentHash,
  integration::local::{Checkout, IndexEntry, LocalFile, LocalPath},
};

use super::{LocalError, LocalGit, LocalGitError, digest, files::executable};

/// One status read answers HEAD, the branch, every changed path with its HEAD entry, and the ignored boundary:
/// a folder an ignore rule matches is one entry and its content is never listed (`--ignored=matching`), so a
/// build folder costs one line however many files it holds.
pub(super) const STATUS: &[&str] = &[
  "status",
  "--porcelain=v2",
  "--branch",
  "--no-renames",
  "-z",
  "--untracked-files=all",
  "--ignored=matching",
];

/// How a snapshot learns which tracked files changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Freshness {
  /// The checkout's own index and its stat cache, read without writing it (`GIT_OPTIONAL_LOCKS=0`): what the
  /// review shows and re-reads.
  Cached,
  /// An owned index rebuilt from the entries without stat data, so every tracked file is hashed: what a mutation
  /// verifies right before it changes anything, so an edit that kept its timestamp and size cannot hide.
  Verified,
}

impl LocalGit {
  pub(super) fn capture(&self, freshness: Freshness) -> Result<Checkout, LocalGitError> {
    let (stage, staged) = self.listed_index()?;
    let output = match freshness {
      Freshness::Cached => self.raw(STATUS, None, None)?,
      Freshness::Verified => self.rebuilt_status(&stage)?,
    };
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let status = Status::parse(&output.bytes)?;
    let unmerged_heads = if status.unmerged.is_empty() {
      BTreeMap::new()
    } else {
      self.tree_entries_of(&status.head, &status.unmerged)?
    };
    let files = status
      .changed
      .iter()
      .map(|(path, change)| {
        Ok(LocalFile {
          head: change.head.clone().or_else(|| unmerged_heads.get(path).cloned()),
          staged: staged.get(path).cloned(),
          working: super::file_hash(&self.root, path)?,
          executable: executable(&self.root, path)?,
          unstaged: change.unstaged,
          path: path.clone(),
        })
      })
      .collect::<Result<_, LocalGitError>>()?;
    Ok(Checkout {
      root: self.root.to_str().ok_or(LocalGitError::Data)?.to_owned(),
      git_dir: self.git_dir_text.clone(),
      branch: status.branch,
      head: status.head,
      index: digest(&stage),
      files,
      ignored: ignored_digest(&status.ignored),
      operation: self.operation()?,
    })
  }

  /// What changes whenever a snapshot would: the status read, the operation marker, and the size and modification
  /// time of every changed path. Cheaper than [`Self::snapshot`]: no index listing and no content hashing, so a
  /// display can re-read it often and take a snapshot only when it changed.
  pub fn status_fingerprint(&self) -> Result<ContentHash, LocalGitError> {
    let output = self.raw(STATUS, None, None)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let status = Status::parse(&output.bytes)?;
    let mut bytes = output.bytes;
    bytes.extend_from_slice(format!("\0{:?}\0", self.operation()?).as_bytes());
    for path in status.changed.keys() {
      // Only a hint to read again: a missing or unreadable entry simply reads as changed state.
      let stamp = fs::symlink_metadata(self.root.join(path.as_str()))
        .ok()
        .map(|metadata| (metadata.len(), metadata.modified().ok()));
      bytes.extend_from_slice(format!("{path}\0{stamp:?}\0").as_bytes());
    }
    Ok(digest(&bytes))
  }

  /// Status against an owned index rebuilt from `stage` without cache metadata.
  fn rebuilt_status(&self, stage: &[u8]) -> Result<super::process::GitOutput, LocalGitError> {
    let scope = tempfile::Builder::new()
      .prefix("inspect-")
      .tempdir_in(&self.scratch)
      .map_err(|_| LocalGitError::Io)?;
    let index = scope.path().join("index");
    #[cfg(windows)]
    let _scope_owner = super::owned::ExecutionOwnership::private_copy(scope.path(), &self.scratch)?;
    self.index_command(&["read-tree", "--empty"], None, &index)?;
    let copied = self.quiet_index(&["update-index", "-z", "--index-info"], Some(stage), &index)?;
    if copied.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    self.quiet_index(STATUS, None, &index)
  }

  /// The index as `ls-files --stage -z` writes it, and its stage-0 entries. Skip-worktree and assume-unchanged
  /// entries would hide working-copy changes, so they are refused.
  fn listed_index(&self) -> Result<(Vec<u8>, BTreeMap<LocalPath, IndexEntry>), LocalGitError> {
    let output = self.raw(&["ls-files", "--stage", "-v", "-z"], None, None)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let mut stage = Vec::with_capacity(output.bytes.len());
    for entry in output.bytes.split(|byte| *byte == 0).filter(|entry| !entry.is_empty()) {
      let (&flag, rest) = entry.split_first().ok_or(LocalGitError::Data)?;
      if flag == b'S' || flag.is_ascii_lowercase() {
        return Err(LocalError::Unsupported.into());
      }
      stage.extend_from_slice(rest.strip_prefix(b" ").ok_or(LocalGitError::Data)?);
      stage.push(0);
    }
    let entries = parse_entries(&stage, true)?;
    Ok((stage, entries))
  }

  pub fn operation(&self) -> Result<Option<String>, LocalGitError> {
    // Every marker is a per-worktree file directly in this checkout's own Git directory.
    let directory = self.git_dir.as_ref().ok_or(LocalGitError::Data)?;
    for marker in MARKERS {
      let path = directory.join(marker);
      let path = path.as_path();
      let parent = path.parent().ok_or(LocalError::Unsupported)?;
      let _parent_owner = super::owned::Directory::open(parent)?;
      // Only a merge head participates in owned recovery; other markers are refused by name.
      match fs::symlink_metadata(path) {
        Ok(_) if marker == "MERGE_HEAD" => {
          let local = LocalPath::new(marker)?;
          let bytes = super::read_regular(parent, &local)?.ok_or(LocalError::Stale)?;
          return Ok(Some(format!("{marker}:{}", digest(&bytes))));
        }
        Ok(_) => return Ok(Some(marker.to_owned())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(LocalGitError::Io),
      }
    }
    Ok(None)
  }

  pub fn index_entries(&self) -> Result<BTreeMap<LocalPath, IndexEntry>, LocalGitError> {
    Ok(self.listed_index()?.1)
  }

  /// Fingerprint every stage outside explicitly owned writes, including unresolved conflict entries.
  pub fn index_fingerprint_excluding(&self, paths: &[LocalPath]) -> Result<ContentHash, LocalGitError> {
    let output = self.raw(&["ls-files", "--stage", "-z"], None, None)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let mut retained = Vec::new();
    for entry in output.bytes.split(|byte| *byte == 0).filter(|entry| !entry.is_empty()) {
      let text = std::str::from_utf8(entry).map_err(|_| LocalGitError::Data)?;
      let (_, path) = text.split_once('\t').ok_or(LocalGitError::Data)?;
      if !paths.contains(&LocalPath::new(path)?) {
        retained.extend_from_slice(entry);
        retained.push(0);
      }
    }
    Ok(digest(&retained))
  }

  pub fn tree_entries(&self, tree: &str) -> Result<BTreeMap<LocalPath, IndexEntry>, LocalGitError> {
    let output = self.raw(&["ls-tree", "-r", "-z", tree], None, None)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    parse_entries(&output.bytes, false)
  }

  fn tree_entries_of(
    &self,
    tree: &str,
    paths: &BTreeSet<LocalPath>,
  ) -> Result<BTreeMap<LocalPath, IndexEntry>, LocalGitError> {
    let mut args = vec!["ls-tree", "-r", "-z", tree, "--"];
    args.extend(paths.iter().map(LocalPath::as_str));
    let output = self.raw(&args, None, None)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    parse_entries(&output.bytes, false)
  }
}

const MARKERS: [&str; 7] = [
  "index.lock",
  "rebase-merge",
  "rebase-apply",
  "CHERRY_PICK_HEAD",
  "REVERT_HEAD",
  "BISECT_START",
  "MERGE_HEAD",
];

/// The ignored boundary's identity: its entries in sorted order, `.orchester/` left out (Orchester's own folder).
pub(super) fn ignored_digest(entries: &BTreeSet<String>) -> ContentHash {
  let mut bytes = Vec::new();
  for entry in entries.iter().filter(|entry| !entry.starts_with(".orchester/")) {
    bytes.extend_from_slice(entry.as_bytes());
    bytes.push(0);
  }
  digest(&bytes)
}

/// What a changed path differs in.
struct Change {
  head: Option<IndexEntry>,
  unstaged: bool,
}

/// A parsed `status --porcelain=v2 --branch -z` read.
pub(super) struct Status {
  head: String,
  branch: String,
  changed: BTreeMap<LocalPath, Change>,
  unmerged: BTreeSet<LocalPath>,
  /// Untracked paths that no rule ignores, and ignored entries (a folder ends in `/`).
  pub untracked: Vec<String>,
  pub ignored: BTreeSet<String>,
}

impl Status {
  pub(super) fn parse(bytes: &[u8]) -> Result<Self, LocalGitError> {
    let text = std::str::from_utf8(bytes).map_err(|_| LocalGitError::Data)?;
    let mut status = Self {
      head: String::new(),
      branch: String::new(),
      changed: BTreeMap::new(),
      unmerged: BTreeSet::new(),
      untracked: Vec::new(),
      ignored: BTreeSet::new(),
    };
    for line in text.split('\0').filter(|line| !line.is_empty()) {
      status.line(line)?;
    }
    // An unborn branch or a detached HEAD has no reviewed base to integrate into.
    if status.head.is_empty() || status.head == "(initial)" || status.branch.is_empty() || status.branch == "(detached)"
    {
      return Err(LocalGitError::Refused);
    }
    super::validate_object(&status.head)?;
    Ok(status)
  }

  fn line(&mut self, line: &str) -> Result<(), LocalGitError> {
    let (kind, rest) = line.split_once(' ').ok_or(LocalGitError::Data)?;
    match kind {
      "#" => {
        if let Some(head) = rest.strip_prefix("branch.oid ") {
          head.clone_into(&mut self.head);
        } else if let Some(branch) = rest.strip_prefix("branch.head ") {
          branch.clone_into(&mut self.branch);
        }
      }
      "1" => {
        // XY sub mH mI mW hH hI path
        let fields = rest.splitn(8, ' ').collect::<Vec<_>>();
        let [code, _, head_mode, _, _, head_object, _, path] = fields[..] else {
          return Err(LocalGitError::Data);
        };
        let head = if head_mode == "000000" {
          None
        } else {
          super::validate_object(head_object)?;
          Some(IndexEntry {
            mode: u32::from_str_radix(head_mode, 8).map_err(|_| LocalGitError::Data)?,
            object: head_object.to_owned(),
          })
        };
        let unstaged = code.as_bytes().get(1).ok_or(LocalGitError::Data)? != &b'.';
        self.changed.insert(LocalPath::new(path)?, Change { head, unstaged });
      }
      "u" => {
        // XY sub m1 m2 m3 mW h1 h2 h3 path
        let path = LocalPath::new(rest.splitn(10, ' ').nth(9).ok_or(LocalGitError::Data)?)?;
        self.unmerged.insert(path.clone());
        self.changed.insert(
          path,
          Change {
            head: None,
            unstaged: true,
          },
        );
      }
      "?" => {
        self.changed.insert(
          LocalPath::new(rest)?,
          Change {
            head: None,
            unstaged: false,
          },
        );
        self.untracked.push(rest.to_owned());
      }
      "!" => {
        self.ignored.insert(rest.to_owned());
      }
      _ => return Err(LocalGitError::Data),
    }
    Ok(())
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
