//! Building the index of one revision into a folder: the records of its files, read from git or
//! taken over from an earlier index when git says they did not change, then every index file.

use std::{
  collections::{BTreeMap, HashSet},
  fs,
  num::NonZeroUsize,
  path::{Path, PathBuf},
  sync::mpsc,
  thread,
  time::Instant,
};

use orchester_core::{revision::Revision, time::Timestamp};

use crate::{
  IndexError,
  git::{Blob, EntryKind, Repository, TreeEntry},
  language::Language,
  manifest::{self, BuildMode, Counts, Manifest},
  map,
  previous::Previous,
  record::FileRecord,
  render,
};

pub(crate) const TREE: &str = "tree.txt";
pub(crate) const SYMBOLS: &str = "symbols.tsv";
pub(crate) const DOCS: &str = "docs.md";
const MAP: &str = "map.md";
const OUTLINES: &str = "outline";
/// Files read ahead of each parsing thread: enough to keep it busy, few enough to bound memory.
const QUEUED_PER_THREAD: usize = 16;

/// `outline/<path>.md` of the index in `dir`.
pub(crate) fn outline_path(dir: &Path, path: &str) -> PathBuf {
  let mut file = path
    .split('/')
    .fold(dir.join(OUTLINES), |folder, name| folder.join(name));
  file.as_mut_os_string().push(".md");
  file
}

/// An earlier index an incremental build starts from, and the paths that changed since.
pub(crate) struct Base {
  pub(crate) revision: Revision,
  pub(crate) dir: PathBuf,
  pub(crate) changed: HashSet<String>,
}

/// What a build did.
pub(crate) struct Built {
  pub(crate) manifest: Manifest,
  /// Files whose content was read and indexed.
  pub(crate) read: usize,
  /// Files taken over from the earlier index.
  pub(crate) reused: usize,
}

/// Builds the index of `revision` of the repository at `repo` into the empty folder `out`, from
/// `base` when given.
pub(crate) fn build(repo: &Path, revision: &Revision, base: Option<&Base>, out: &Path) -> Result<Built, IndexError> {
  let started = Instant::now();
  let git = Repository::new(repo);
  let entries = git.tree(revision)?;
  let manifest_objects: Vec<(String, String)> = entries
    .iter()
    .filter(|entry| entry.kind == EntryKind::File && map::is_manifest(&entry.path))
    .map(|entry| (entry.path.clone(), entry.object.clone()))
    .collect();
  let collected = collect(&git, entries, base)?;
  let manifests = read_manifests(&git, manifest_objects)?;
  let map = map::render(revision, &collected.records, &manifests);
  write(out, &collected.records, &map.text)?;
  let counts = Counts::of(&collected.records);
  let manifest = Manifest {
    format: manifest::FORMAT,
    revision: revision.clone(),
    built_at: Timestamp::now(),
    build_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    build: match base {
      Some(base) => BuildMode::Incremental {
        from: base.revision.clone(),
        changed: base.changed.len(),
      },
      None => BuildMode::Full,
    },
    files: counts.files,
    lines: counts.lines,
    symbols: counts.symbols,
    outlines: counts.outlines,
    doc_headings: counts.doc_headings,
    packages: map.packages,
    languages: counts.languages,
  };
  manifest.write(out)?;
  Ok(Built {
    manifest,
    read: collected.read,
    reused: collected.reused,
  })
}

/// The records of every tracked file, in git's order.
struct Collected {
  records: Vec<FileRecord>,
  read: usize,
  reused: usize,
}

fn collect(git: &Repository<'_>, entries: Vec<TreeEntry>, base: Option<&Base>) -> Result<Collected, IndexError> {
  let mut previous = base.map(|base| Previous::open(&base.dir, &base.revision)).transpose()?;
  let mut slots: Vec<Option<FileRecord>> = Vec::with_capacity(entries.len());
  let mut pending: Vec<(usize, String)> = Vec::new();
  let mut objects = Vec::new();
  let mut reused = 0;
  for entry in entries {
    let unchanged = base.is_some_and(|base| !base.changed.contains(&entry.path));
    if unchanged
      && let Some(previous) = previous.as_mut()
      && let Some(record) = previous.take(&entry.path)?
    {
      slots.push(Some(record));
      reused += 1;
      continue;
    }
    match entry.kind {
      EntryKind::Symlink => slots.push(Some(FileRecord::without_content(entry.path, Language::SYMLINK))),
      EntryKind::Submodule => slots.push(Some(FileRecord::without_content(entry.path, Language::SUBMODULE))),
      // A line break or tab in a path would break the rows it is written in.
      EntryKind::File if entry.path.chars().any(char::is_control) => slots.push(Some(FileRecord::without_content(
        entry.path.escape_debug().to_string(),
        Language::OTHER,
      ))),
      EntryKind::File => {
        pending.push((slots.len(), entry.path));
        objects.push(entry.object);
        slots.push(None);
      }
    }
  }
  let read = pending.len();
  for (slot, record) in read_files(git, pending, objects)? {
    slots[slot] = Some(record);
  }
  let records = slots
    .into_iter()
    .map(|slot| slot.expect("every file waiting for its content was read"))
    .collect();
  Ok(Collected { records, read, reused })
}

/// The records of the files `pending` (`(slot, path)`), whose blobs are `objects`: git reads them
/// in order while a thread per core parses them, each taking every n-th file.
fn read_files(
  git: &Repository<'_>,
  pending: Vec<(usize, String)>,
  objects: Vec<String>,
) -> Result<Vec<(usize, FileRecord)>, IndexError> {
  let workers = thread::available_parallelism().map_or(1, NonZeroUsize::get);
  thread::scope(|scope| {
    let (senders, handles): (Vec<_>, Vec<_>) = (0..workers)
      .map(|_| {
        let (sender, receiver) = mpsc::sync_channel::<(usize, String, Blob)>(QUEUED_PER_THREAD);
        let handle = scope.spawn(move || {
          receiver
            .into_iter()
            .map(|(slot, path, blob)| FileRecord::read(path, blob).map(|record| (slot, record)))
            .collect::<Result<Vec<_>, IndexError>>()
        });
        (sender, handle)
      })
      .collect();
    let mut paths = pending.into_iter();
    let reading = git.read_blobs(objects, |position, blob| {
      let (slot, path) = paths.next().expect("one path per requested blob");
      // A thread stops taking files only after an error, which joining it reports.
      if senders[position % workers].send((slot, path, blob)).is_err() {
        tracing::debug!("an indexing thread stopped early");
      }
      Ok::<(), IndexError>(())
    });
    drop(senders);
    let mut records = Vec::new();
    for handle in handles {
      records.extend(handle.join().expect("an indexing thread does not panic")?);
    }
    reading?;
    Ok(records)
  })
}

/// The text of the package manifests `objects` (`(path, object)`), by path.
fn read_manifests(
  git: &Repository<'_>,
  objects: Vec<(String, String)>,
) -> Result<BTreeMap<String, String>, IndexError> {
  let (paths, objects): (Vec<String>, Vec<String>) = objects.into_iter().unzip();
  let mut texts = BTreeMap::new();
  git.read_blobs(objects, |position, blob| {
    if let Blob::Text(content) = blob {
      texts.insert(paths[position].clone(), String::from_utf8_lossy(&content).into_owned());
    }
    Ok::<(), IndexError>(())
  })?;
  Ok(texts)
}

fn write(out: &Path, records: &[FileRecord], map: &str) -> Result<(), IndexError> {
  let files = [
    (TREE, render::tree(records)),
    (SYMBOLS, render::symbols(records)),
    (DOCS, render::docs(records)),
    (MAP, map.to_owned()),
  ];
  for (name, text) in files {
    let file = out.join(name);
    fs::write(&file, text).map_err(IndexError::io("write", file))?;
  }
  let mut folders = HashSet::new();
  for record in records {
    let Some(outline) = &record.outline else {
      continue;
    };
    let file = outline_path(out, &record.path);
    let folder = file.parent().expect("an outline file is inside the outline folder");
    if folders.insert(folder.to_path_buf()) {
      fs::create_dir_all(folder).map_err(IndexError::io("create", folder))?;
    }
    fs::write(&file, outline).map_err(IndexError::io("write", file))?;
  }
  Ok(())
}
