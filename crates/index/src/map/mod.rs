//! `map.md`: the architecture overview of the revision, generated from its files. Each package a
//! manifest describes gets its folder, name, description, entry files, test folders, counts and the
//! first two levels of its module tree; the folders outside every package get their file counts.
//! A file belongs to the package of the deepest folder holding it.

mod packages;

use std::{
  collections::{BTreeMap, BTreeSet, HashMap, HashSet},
  fmt::Write as _,
};

use orchester_core::revision::Revision;

use self::packages::{Context, Package, join};
use crate::record::FileRecord;

/// Folders of a package that hold its tests.
const TEST_FOLDERS: [&str; 4] = ["tests", "test", "__tests__", "spec"];
/// The most names listed for the content of one folder.
const LISTED: usize = 20;

pub(crate) struct Map {
  pub(crate) text: String,
  pub(crate) packages: usize,
}

/// Whether the file at `path` is a manifest the map reads.
pub(crate) fn is_manifest(path: &str) -> bool {
  let name = path.rsplit('/').next().unwrap_or(path);
  packages::MANIFESTS.contains(&name)
}

/// The map of `records`, the files of `revision`, whose manifests are `manifests` (by path).
pub(crate) fn render(revision: &Revision, records: &[FileRecord], manifests: &BTreeMap<String, String>) -> Map {
  let files: BTreeSet<&str> = records.iter().map(|record| record.path.as_str()).collect();
  let folders: HashSet<&str> = files
    .iter()
    .flat_map(|path| path.match_indices('/').map(|(at, _)| &path[..at]))
    .collect();
  let context = Context {
    files: &files,
    folders: &folders,
    manifests,
  };
  let mut found = Vec::new();
  let mut unreadable = Vec::new();
  for (path, text) in manifests {
    match packages::read(path, text, &context) {
      Ok(Some(package)) => found.push(package),
      Ok(None) => {}
      Err(error) => unreadable.push((path, error)),
    }
  }
  found.sort_by(|left, right| (&left.dir, left.ecosystem).cmp(&(&right.dir, right.ecosystem)));
  let mut owned: Vec<Vec<&FileRecord>> = found.iter().map(|_| Vec::new()).collect();
  let mut outside = Vec::new();
  let mut by_dir: HashMap<&str, usize> = HashMap::new();
  for (index, package) in found.iter().enumerate() {
    by_dir.entry(package.dir.as_str()).or_insert(index);
  }
  for record in records {
    match owner(&by_dir, &record.path) {
      Some(package) => owned[package].push(record),
      None => outside.push(record),
    }
  }
  let mut text = String::new();
  let lines: usize = records.iter().filter_map(|record| record.lines).sum();
  let symbols: usize = records.iter().map(|record| record.symbols.len()).sum();
  line(
    &mut text,
    format_args!(
      "# Repository map\n\nRevision {revision}: {} files, {lines} lines, {symbols} symbols, {} packages.",
      records.len(),
      found.len()
    ),
  );
  for (package, files) in found.iter().zip(&owned) {
    package_section(&mut text, package, files);
  }
  outside_section(&mut text, &outside);
  if !unreadable.is_empty() {
    line(&mut text, format_args!("\n## Unreadable manifests\n"));
    for (path, error) in unreadable {
      line(&mut text, format_args!("- {path}: {error}"));
    }
  }
  Map {
    text,
    packages: found.len(),
  }
}

/// The package of the deepest folder holding `path`, of the packages by folder `by_dir`.
fn owner(by_dir: &HashMap<&str, usize>, path: &str) -> Option<usize> {
  path
    .rmatch_indices('/')
    .map(|(at, _)| &path[..at])
    .chain([""])
    .find_map(|folder| by_dir.get(folder).copied())
}

fn package_section(text: &mut String, package: &Package, files: &[&FileRecord]) {
  let dir = if package.dir.is_empty() { "." } else { &package.dir };
  line(
    text,
    format_args!("\n## {dir}: {} ({})\n", package.name, package.ecosystem),
  );
  if let Some(description) = &package.description {
    line(
      text,
      format_args!("{}\n", description.split_whitespace().collect::<Vec<_>>().join(" ")),
    );
  }
  let lines: usize = files.iter().filter_map(|record| record.lines).sum();
  let symbols: usize = files.iter().map(|record| record.symbols.len()).sum();
  line(
    text,
    format_args!("- files: {}, lines: {lines}, symbols: {symbols}", files.len()),
  );
  if !package.entries.is_empty() {
    line(text, format_args!("- entry: {}", package.entries.join(", ")));
  }
  let relative: Vec<(&str, &FileRecord)> = files
    .iter()
    .map(|record| {
      let path = record.path.as_str();
      let inside = if package.dir.is_empty() {
        path
      } else {
        &path[package.dir.len() + 1..]
      };
      (inside, *record)
    })
    .collect();
  let top = level(relative.iter().copied());
  let tests: Vec<String> = top
    .iter()
    .filter(|(name, entry)| entry.folder && TEST_FOLDERS.contains(&name.as_str()))
    .map(|(name, entry)| entry.name(name))
    .collect();
  if !tests.is_empty() {
    line(text, format_args!("- tests: {}", tests.join(", ")));
  }
  let source = package.source_root.as_deref();
  let modules = level(relative.iter().filter_map(|(path, record)| match source {
    Some(root) => path.strip_prefix(root)?.strip_prefix('/').map(|inner| (inner, *record)),
    None => (!is_test(path)).then_some((*path, *record)),
  }));
  match source {
    Some(root) => line(text, format_args!("- modules in {}/:", join(&package.dir, root))),
    None => line(text, format_args!("- modules:")),
  }
  for (name, entry) in &modules {
    line(text, format_args!("  - {}", entry.describe(name)));
  }
  if let Some(root) = source {
    let also: Vec<String> = top
      .iter()
      .filter(|(name, entry)| name.as_str() != root && !(entry.folder && TEST_FOLDERS.contains(&name.as_str())))
      .map(|(name, entry)| entry.name(name))
      .collect();
    if !also.is_empty() {
      line(text, format_args!("- also: {}", listed(also)));
    }
  }
}

fn outside_section(text: &mut String, outside: &[&FileRecord]) {
  if outside.is_empty() {
    return;
  }
  line(text, format_args!("\n## Outside packages\n"));
  let top = level(outside.iter().map(|record| (record.path.as_str(), *record)));
  let root_files: Vec<String> = top
    .iter()
    .filter(|(_, entry)| !entry.folder)
    .map(|(name, _)| name.clone())
    .collect();
  for (name, entry) in top.iter().filter(|(_, entry)| entry.folder) {
    line(text, format_args!("- {}", entry.name(name)));
  }
  if !root_files.is_empty() {
    line(text, format_args!("- root files: {}", listed(root_files)));
  }
}

fn is_test(path: &str) -> bool {
  let first = path.split('/').next().unwrap_or(path);
  path.contains('/') && TEST_FOLDERS.contains(&first)
}

/// A file or a folder at one level of a tree.
#[derive(Default)]
struct Entry {
  folder: bool,
  files: usize,
  lines: usize,
  /// The names in a folder, a folder's with `/`.
  children: BTreeSet<String>,
}

impl Entry {
  /// `name/ (3 files)` for a folder, `name` for a file.
  fn name(&self, name: &str) -> String {
    if self.folder {
      format!("{name}/ ({})", files(self.files))
    } else {
      name.to_owned()
    }
  }

  /// `name/ (3 files, 120 lines): a.rs, b/` for a folder, `name (40 lines)` for a file.
  fn describe(&self, name: &str) -> String {
    if self.folder {
      let children = listed(self.children.iter().cloned().collect());
      format!("{name}/ ({}, {}): {children}", files(self.files), lines(self.lines))
    } else {
      format!("{name} ({})", lines(self.lines))
    }
  }
}

/// The first level of the tree of `files` (paths relative to its top).
fn level<'a>(files: impl Iterator<Item = (&'a str, &'a FileRecord)>) -> BTreeMap<String, Entry> {
  let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
  for (path, record) in files {
    let (first, rest) = path
      .split_once('/')
      .map_or((path, None), |(first, rest)| (first, Some(rest)));
    let entry = entries.entry(first.to_owned()).or_default();
    entry.files += 1;
    entry.lines += record.lines.unwrap_or(0);
    if let Some(rest) = rest {
      entry.folder = true;
      entry.children.insert(
        rest
          .split_once('/')
          .map_or_else(|| rest.to_owned(), |(second, _)| format!("{second}/")),
      );
    }
  }
  entries
}

fn files(count: usize) -> String {
  counted(count, "file")
}

fn lines(count: usize) -> String {
  counted(count, "line")
}

fn counted(count: usize, noun: &str) -> String {
  if count == 1 {
    format!("1 {noun}")
  } else {
    format!("{count} {noun}s")
  }
}

/// `names` joined, at most [`LISTED`] of them.
fn listed(names: Vec<String>) -> String {
  let more = names.len().saturating_sub(LISTED);
  let mut shown = names.into_iter().take(LISTED).collect::<Vec<_>>().join(", ");
  if more > 0 {
    write!(shown, ", +{more} more").expect("writing to a String does not fail");
  }
  shown
}

fn line(text: &mut String, arguments: std::fmt::Arguments<'_>) {
  writeln!(text, "{arguments}").expect("writing to a String does not fail");
}
