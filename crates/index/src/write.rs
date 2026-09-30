//! Writing an index's files into its folder: `tree.txt`, `symbols.tsv`, `docs.md` and `map.md`
//! streamed from the records, and one outline file per outlined source file, written from the text
//! just made or linked to the unchanged file of the earlier index, so an incremental build does not
//! read or copy what did not change. An outline that cannot be written (a full disk, a file another
//! program holds) is left out and logged rather than failing the index.

use std::{
  collections::HashSet,
  fs::{self, File},
  io::{self, BufWriter, Write},
  path::{Path, PathBuf},
};

use orchester_core::revision::Revision;

use crate::{
  IndexError,
  outline_file::outline_path,
  record::{FileRecord, Outline},
  render,
};

pub(crate) const TREE: &str = "tree.txt";
pub(crate) const SYMBOLS: &str = "symbols.tsv";
pub(crate) const DOCS: &str = "docs.md";
const MAP: &str = "map.md";

/// Writes the index of `records` with `map` into `out`, linking the outlines of the index of `base`.
/// Records whose outline could not be written lose it.
pub(crate) fn write(
  out: &Path,
  records: &mut [FileRecord],
  map: &str,
  base: Option<&Revision>,
) -> Result<(), IndexError> {
  stream(out, TREE, |file| render::tree(file, records))?;
  stream(out, SYMBOLS, |file| render::symbols(file, records))?;
  stream(out, DOCS, |file| render::docs(file, records))?;
  let file = out.join(MAP);
  fs::write(&file, map).map_err(IndexError::io("write", file))?;
  let mut folders = HashSet::new();
  for record in records.iter_mut() {
    let Some(outline) = &record.outline else {
      continue;
    };
    let file = outline_path(out, &record.path);
    match place(&file, outline, &mut folders) {
      Ok(()) => {}
      // The earlier index went while this one was built: a full build replaces this one.
      Err(error) if matches!(outline, Outline::Previous(_)) => {
        return Err(IndexError::Previous {
          revision: base.map(ToString::to_string).unwrap_or_default(),
          detail: format!("cannot link the outline of {}: {error}", record.path),
        });
      }
      Err(error) => {
        tracing::warn!(%error, path = %record.path, "the outline cannot be written; the index goes without it");
        record.outline = None;
      }
    }
  }
  Ok(())
}

fn stream(
  out: &Path,
  name: &str,
  render: impl FnOnce(&mut BufWriter<File>) -> io::Result<()>,
) -> Result<(), IndexError> {
  let path = out.join(name);
  let written = File::create(&path).and_then(|file| {
    let mut writer = BufWriter::new(file);
    render(&mut writer)?;
    writer.flush()
  });
  written.map_err(IndexError::io("write", path))
}

/// Writes or links `outline` as `file`, creating its folder the first time.
fn place(file: &Path, outline: &Outline, folders: &mut HashSet<PathBuf>) -> io::Result<()> {
  let folder = file.parent().expect("an outline file is inside the outline folder");
  if folders.insert(folder.to_path_buf()) {
    fs::create_dir_all(folder)?;
  }
  match outline {
    Outline::Text(text) => fs::write(file, text),
    // Indexes are never changed once written, so the two can share the file; a file system
    // without hard links gets a copy.
    Outline::Previous(earlier) => fs::hard_link(earlier, file).or_else(|_| fs::copy(earlier, file).map(drop)),
  }
}
