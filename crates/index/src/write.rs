//! Writing an index's files into its folder: `tree.txt`, `symbols.tsv`, `docs.md` and `map.md`
//! streamed from the records, and one outline file per outlined source file, written from the text
//! just made or linked to the unchanged file of the earlier index, so an incremental build does not
//! read or copy what did not change. Creating many small files is what an index spends most of its
//! time on (on Windows each is scanned as it is made), so the outline files are placed by a thread
//! per core. An outline that cannot be written (a full disk, a file another program holds) is left
//! out and logged rather than failing the index.

use std::{
  collections::BTreeSet,
  fs::{self, File},
  io::{self, BufWriter},
  num::NonZeroUsize,
  path::{Path, PathBuf},
  thread,
};

use orchester_core::revision::Revision;

use crate::{
  IndexError,
  digest::{self, Digests, Hashing},
  outline_file::{outline_name, outline_path},
  record::{FileRecord, Outline},
  render,
};

pub(crate) const TREE: &str = "tree.txt";
pub(crate) const SYMBOLS: &str = "symbols.tsv";
pub(crate) const DOCS: &str = "docs.md";
const MAP: &str = "map.md";

/// Writes the index of `records` with `map` into `out`, linking the outlines of the index of `base`;
/// the digest of each file written. Records whose outline could not be written lose it.
pub(crate) fn write(
  out: &Path,
  records: &mut [FileRecord],
  map: &str,
  base: Option<&Revision>,
) -> Result<Digests, IndexError> {
  let mut digests = Digests::new();
  digests.insert(TREE.into(), stream(out, TREE, |file| render::tree(file, records))?);
  digests.insert(
    SYMBOLS.into(),
    stream(out, SYMBOLS, |file| render::symbols(file, records))?,
  );
  digests.insert(DOCS.into(), stream(out, DOCS, |file| render::docs(file, records))?);
  let file = out.join(MAP);
  fs::write(&file, map).map_err(IndexError::io("write", file))?;
  digests.insert(MAP.into(), digest::of(map.as_bytes()));
  for (index, error) in place_outlines(out, records) {
    let record = &mut records[index];
    if matches!(record.outline, Some(Outline::Previous { .. })) {
      // The earlier index went while this one was built: a full build replaces this one.
      return Err(IndexError::Previous {
        revision: base.map(ToString::to_string).unwrap_or_default(),
        detail: format!("cannot link the outline of {}: {error}", record.path),
      });
    }
    tracing::warn!(%error, path = %record.path, "the outline cannot be written; the index goes without it");
    record.outline = None;
  }
  for record in records.iter() {
    let digest = match &record.outline {
      Some(Outline::Text(text)) => digest::of(text.as_bytes()),
      Some(Outline::Previous { digest, .. }) => digest.clone(),
      None => continue,
    };
    digests.insert(outline_name(&record.path), digest);
  }
  Ok(digests)
}

/// Writes `name` in `out` with `render`; its digest.
fn stream(
  out: &Path,
  name: &str,
  render: impl FnOnce(&mut Hashing<BufWriter<File>>) -> io::Result<()>,
) -> Result<String, IndexError> {
  let path = out.join(name);
  let written = File::create(&path).and_then(|file| {
    let mut writer = Hashing::new(BufWriter::new(file));
    render(&mut writer)?;
    writer.finish()
  });
  written.map_err(IndexError::io("write", path))
}

/// Places the outline file of every record that has one, on a thread per core; the records whose
/// outline could not be placed, by position, and why.
fn place_outlines(out: &Path, records: &[FileRecord]) -> Vec<(usize, io::Error)> {
  let outlines: Vec<(usize, PathBuf, &Outline)> = records
    .iter()
    .enumerate()
    .filter_map(|(index, record)| {
      let outline = record.outline.as_ref()?;
      Some((index, outline_path(out, &record.path), outline))
    })
    .collect();
  let folders: Vec<&Path> = outlines
    .iter()
    .filter_map(|(_, file, _)| file.parent())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect();
  // A folder that cannot be made fails the placing of its files, which reports it.
  let missing = in_parallel(&folders, |folder| fs::create_dir_all(folder).err());
  if !missing.is_empty() {
    tracing::warn!(folders = missing.len(), "outline folders could not be created");
  }
  let mut failed = in_parallel(&outlines, |(index, file, outline)| {
    place(file, outline).err().map(|error| (*index, error))
  });
  failed.sort_by_key(|(index, _)| *index);
  failed
}

/// What `work` gives for each of `items` that gives something, `items` shared among a thread per
/// core.
fn in_parallel<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> Option<R> + Sync) -> Vec<R> {
  let workers = thread::available_parallelism().map_or(1, NonZeroUsize::get);
  let chunk = items.len().div_ceil(workers).max(1);
  let work = &work;
  thread::scope(|scope| {
    let handles: Vec<_> = items
      .chunks(chunk)
      .map(|part| scope.spawn(move || part.iter().filter_map(work).collect::<Vec<_>>()))
      .collect();
    handles
      .into_iter()
      .flat_map(|handle| handle.join().expect("a writer thread does not panic"))
      .collect()
  })
}

/// Writes or links `outline` as `file`.
fn place(file: &Path, outline: &Outline) -> io::Result<()> {
  match outline {
    Outline::Text(text) => fs::write(file, text),
    // Indexes are never changed once written, so the two can share the file; a file system
    // without hard links gets a copy.
    Outline::Previous { file: earlier, .. } => {
      fs::hard_link(earlier, file).or_else(|_| fs::copy(earlier, file).map(drop))
    }
  }
}

/// Marks every file under `folder` read-only: an index is shared by the runs of its revision, its
/// outline files also by later indexes that link them, and it never changes once written. Agents
/// that may write outside their worktree (Claude Code writes in the folders it is given) cannot
/// change it; removing it still works.
pub(crate) fn make_read_only(folder: &Path) -> io::Result<()> {
  for entry in fs::read_dir(folder)? {
    let entry = entry?;
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      make_read_only(&path)?;
    } else {
      let mut permissions = entry.metadata()?.permissions();
      if !permissions.readonly() {
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions)?;
      }
    }
  }
  Ok(())
}
