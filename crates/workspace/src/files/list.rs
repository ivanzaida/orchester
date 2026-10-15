//! The files of one version as the code viewer's tree and file search show them: every file's path,
//! the folders they make, which rows the tree shows with some folders open, and the files a name
//! search finds (like Ctrl+P).

use std::{
  collections::{BTreeMap, BTreeSet, HashMap},
  sync::Arc,
};

use super::path::is_hidden;

/// How a tree row shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryKind {
  File,
  Folder { open: bool },
}

/// One row of the tree: a file or a folder at `depth` (0 at the top).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TreeRow {
  pub path: Arc<str>,
  pub name: Arc<str>,
  pub depth: usize,
  pub kind: EntryKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
  path: Arc<str>,
  name: Arc<str>,
  folder: bool,
}

/// The files of a version, sorted, without what lies in `.git` or `.orchester`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileList {
  paths: Arc<[Arc<str>]>,
  /// Each folder's entries ("" is the top), folders first, then by name without regard to case.
  folders: Arc<HashMap<Arc<str>, Vec<Entry>>>,
}

impl FileList {
  /// The list of `paths` (`/`-separated, relative), hidden ones left out, and folders git names as `sub/` (a
  /// repository inside the folder) too, since they hold no file of this one.
  pub fn new(paths: impl IntoIterator<Item = String>) -> Self {
    let paths: BTreeSet<Arc<str>> = paths
      .into_iter()
      .filter(|path| !path.is_empty() && !path.ends_with('/') && !is_hidden(path))
      .map(Arc::from)
      .collect();
    let folders = folders(&paths);
    Self {
      paths: paths.into_iter().collect(),
      folders: Arc::new(folders),
    }
  }

  pub fn len(&self) -> usize {
    self.paths.len()
  }

  pub fn is_empty(&self) -> bool {
    self.paths.is_empty()
  }

  /// Whether the list has the file `path`.
  pub fn contains(&self, path: &str) -> bool {
    self.paths.binary_search_by(|listed| (**listed).cmp(path)).is_ok()
  }

  /// The tree's rows with the folders in `open` open, top to bottom.
  pub fn rows(&self, open: &BTreeSet<Arc<str>>) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    self.push_rows(&mut rows, "", 0, open);
    rows
  }

  fn push_rows(&self, rows: &mut Vec<TreeRow>, folder: &str, depth: usize, open: &BTreeSet<Arc<str>>) {
    let Some(entries) = self.folders.get(folder) else {
      return;
    };
    for entry in entries {
      let is_open = entry.folder && open.contains(&entry.path);
      rows.push(TreeRow {
        path: entry.path.clone(),
        name: entry.name.clone(),
        depth,
        kind: if entry.folder {
          EntryKind::Folder { open: is_open }
        } else {
          EntryKind::File
        },
      });
      if is_open {
        self.push_rows(rows, &entry.path, depth + 1, open);
      }
    }
  }

  /// The files whose path holds `query`'s characters in order, ignoring case, at most `limit` of them; best
  /// first: the file's name is the query, starts with it, holds it; then the path holds it; then the
  /// characters in order. Shorter paths first within each.
  pub fn search(&self, query: &str, limit: usize) -> NameMatches {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
      return NameMatches::default();
    }
    let mut found: Vec<(u8, &Arc<str>)> = self
      .paths
      .iter()
      .filter_map(|path| rank(path, &query).map(|rank| (rank, path)))
      .collect();
    found.sort_by(|(a_rank, a), (b_rank, b)| a_rank.cmp(b_rank).then(a.len().cmp(&b.len())).then(a.cmp(b)));
    NameMatches {
      total: found.len(),
      paths: found.into_iter().take(limit).map(|(_, path)| path.clone()).collect(),
    }
  }
}

/// What a file-name search found: the best paths and how many matched in all.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameMatches {
  pub paths: Vec<Arc<str>>,
  pub total: usize,
}

/// How well `path` answers the lowercase `query`, lower is better; `None` when it does not.
fn rank(path: &str, query: &str) -> Option<u8> {
  let lower = path.to_lowercase();
  let name = lower.rsplit('/').next().unwrap_or(&lower);
  if name == query {
    Some(0)
  } else if name.starts_with(query) {
    Some(1)
  } else if name.contains(query) {
    Some(2)
  } else if lower.contains(query) {
    Some(3)
  } else {
    in_order(&lower, query).then_some(4)
  }
}

/// Whether `text` holds the characters of `query` in order.
fn in_order(text: &str, query: &str) -> bool {
  let mut chars = text.chars();
  query.chars().all(|wanted| chars.any(|character| character == wanted))
}

/// Where an entry sorts in its folder: folders first, then by name without regard to case, then by name.
type EntryOrder = (bool, String, Arc<str>);

/// The entries of every folder of `paths`.
fn folders(paths: &BTreeSet<Arc<str>>) -> HashMap<Arc<str>, Vec<Entry>> {
  let mut sorted: BTreeMap<Arc<str>, BTreeMap<EntryOrder, Entry>> = BTreeMap::new();
  for path in paths {
    let mut parent: Arc<str> = Arc::from("");
    let mut parts = path.split('/').peekable();
    let mut end = 0;
    while let Some(name) = parts.next() {
      end += name.len();
      let folder = parts.peek().is_some();
      let entry_path: Arc<str> = if folder { Arc::from(&path[..end]) } else { path.clone() };
      let key = (!folder, name.to_lowercase(), Arc::from(name));
      sorted
        .entry(parent.clone())
        .or_default()
        .entry(key)
        .or_insert_with(|| Entry {
          path: entry_path.clone(),
          name: Arc::from(name),
          folder,
        });
      parent = entry_path;
      end += 1;
    }
  }
  sorted
    .into_iter()
    .map(|(folder, entries)| (folder, entries.into_values().collect()))
    .collect()
}
