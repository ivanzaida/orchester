//! Building the index of one revision into a folder: the records of its files, read from git or
//! taken over from an earlier index when git says they did not change, then every index file.

use std::{
  collections::{BTreeMap, HashSet},
  path::{Path, PathBuf},
  time::Instant,
};

use orchester_core::{revision::Revision, time::Timestamp};

use crate::{
  IndexError,
  digest::Digests,
  git::{Blob, EntryKind, Repository, TreeEntry},
  language::Language,
  manifest::{self, BuildMode, Counts, Manifest},
  map, outline_file,
  parallel::{Pending, read_files},
  previous::Previous,
  progress::{BuildPhase, ProgressFile},
  record::FileRecord,
  write,
};

/// An earlier index an incremental build starts from, and the paths that changed since.
pub(crate) struct Base {
  pub(crate) revision: Revision,
  pub(crate) dir: PathBuf,
  pub(crate) changed: HashSet<String>,
  /// The digests of its files, which were checked.
  pub(crate) digests: Digests,
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
pub(crate) fn build(
  repo: &Path,
  revision: &Revision,
  base: Option<&Base>,
  (out, progress): (&Path, &ProgressFile),
) -> Result<Built, IndexError> {
  let started = Instant::now();
  let git = Repository::new(repo);
  let entries = git.tree(revision)?;
  progress.phase(BuildPhase::Indexing { files: entries.len() });
  let manifest_objects: Vec<(String, String)> = entries
    .iter()
    .filter(|entry| entry.kind == EntryKind::File && map::is_manifest(&entry.path))
    .map(|entry| (entry.path.clone(), entry.object.clone()))
    .collect();
  let mut collected = collect(&git, entries, base)?;
  let manifests = read_manifests(&git, manifest_objects)?;
  let map = map::render(revision, &collected.records, &manifests);
  progress.phase(BuildPhase::Writing);
  let digests = write::write(out, &mut collected.records, &map.text, base.map(|base| &base.revision))?;
  let counts = Counts::of(&collected.records);
  let bytes = write::size(out).map_err(IndexError::io("measure", out))?;
  let manifest = Manifest {
    format: manifest::FORMAT,
    generator: manifest::generator(),
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
    bytes,
    digests,
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
  let mut previous = base
    .map(|base| Previous::open(&base.dir, &base.revision, base.digests.clone()))
    .transpose()?;
  let outlined: HashSet<String> = outline_file::outlined(
    entries
      .iter()
      .filter(|entry| entry.kind == EntryKind::File && Language::of(&entry.path).has_grammar())
      .map(|entry| entry.path.as_str()),
  )
  .into_iter()
  .map(str::to_owned)
  .collect();
  let mut slots: Vec<Option<FileRecord>> = Vec::with_capacity(entries.len());
  let mut pending: Vec<Pending> = Vec::new();
  let mut reused = 0;
  for entry in entries {
    let unchanged = base.is_some_and(|base| !base.changed.contains(&entry.path));
    let outline = outlined.contains(&entry.path);
    if unchanged
      && let Some(previous) = previous.as_mut()
      && let Some(record) = previous.take(&entry.path, outline)
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
        pending.push(Pending {
          slot: slots.len(),
          path: entry.path,
          object: entry.object,
          outlined: outline,
        });
        slots.push(None);
      }
    }
  }
  let read = pending.len();
  for (slot, record) in read_files(git, pending)? {
    slots[slot] = Some(record);
  }
  let records = slots
    .into_iter()
    .map(|slot| slot.expect("every file waiting for its content was read"))
    .collect();
  Ok(Collected { records, read, reused })
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
