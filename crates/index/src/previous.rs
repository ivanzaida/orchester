//! An earlier index read back, for an incremental build: the records of the files that did not
//! change since its revision are taken over as they are, so they are not read or parsed again.

use std::{
  collections::{HashMap, HashSet},
  fs,
  path::{Path, PathBuf},
};

use orchester_core::revision::Revision;

use crate::{
  IndexError,
  digest::Digests,
  language::Language,
  markdown::Heading,
  outline::Kind,
  outline_file::{outline_name, outline_path},
  record::{FileRecord, Outline, Symbol},
  render::NO_LINES,
  write,
};

pub(crate) struct Previous {
  dir: PathBuf,
  records: HashMap<String, FileRecord>,
  /// The digests of its files, checked before it was opened: which outline files it has.
  digests: Digests,
}

impl Previous {
  /// The index of `revision` in `dir`, whose files match `digests`.
  pub(crate) fn open(dir: &Path, revision: &Revision, digests: Digests) -> Result<Self, IndexError> {
    let unreadable = |detail: String| unreadable(revision, detail);
    let read = |name: &str| {
      let file = dir.join(name);
      fs::read_to_string(&file).map_err(|error| unreadable(format!("cannot read {}: {error}", file.display())))
    };
    let mut records = HashMap::new();
    for line in read(write::TREE)?.lines() {
      let record = tree_row(line).ok_or_else(|| unreadable(format!("unreadable tree row {line:?}")))?;
      records.insert(record.path.clone(), record);
    }
    for line in read(write::SYMBOLS)?.lines() {
      let (path, symbol) = symbol_row(line).ok_or_else(|| unreadable(format!("unreadable symbol row {line:?}")))?;
      let Some(record) = records.get_mut(path) else {
        return Err(unreadable(format!("a symbol of {path}, which is not in the tree")));
      };
      record.symbols.push(symbol);
    }
    let documents: HashSet<String> = records
      .values()
      .filter(|record| record.language == Language::Markdown)
      .map(|record| record.path.clone())
      .collect();
    for line in read(write::DOCS)?.lines() {
      let (path, heading) =
        heading_row(line, &documents).ok_or_else(|| unreadable(format!("unreadable heading row {line:?}")))?;
      records
        .get_mut(path)
        .expect("heading rows name a document of the tree")
        .headings
        .push(heading);
    }
    Ok(Self {
      dir: dir.to_path_buf(),
      records,
      digests,
    })
  }

  /// The record of `path`, which did not change since this index was built, with its outline file
  /// when it is `outlined`; `None` when the index does not have it, or has no outline file for it
  /// (it had another path of the same name ignoring case then).
  pub(crate) fn take(&mut self, path: &str, outlined: bool) -> Option<FileRecord> {
    let mut record = self.records.remove(path)?;
    if outlined {
      // A file the earlier index has no outline of (too large, minified, parsed too slowly, or
      // not written) is read again: this build may make one.
      let digest = self.digests.get(&outline_name(path))?;
      record.outline = Some(Outline::Previous {
        file: outline_path(&self.dir, path),
        digest: digest.clone(),
      });
    }
    Some(record)
  }
}

fn unreadable(revision: &Revision, detail: String) -> IndexError {
  IndexError::Previous {
    revision: revision.to_string(),
    detail,
  }
}

fn tree_row(line: &str) -> Option<FileRecord> {
  let mut fields = line.rsplitn(3, '\t');
  let (lines, language, path) = (fields.next()?, fields.next()?, fields.next()?);
  let mut record = FileRecord::without_content(path.to_owned(), Language::from_name(language)?);
  if lines != NO_LINES {
    record.lines = Some(lines.parse().ok()?);
  }
  Some(record)
}

fn symbol_row(line: &str) -> Option<(&str, Symbol)> {
  let fields: Vec<&str> = line.splitn(5, '\t').collect();
  let [name, path, number, kind, container] = fields[..] else {
    return None;
  };
  let symbol = Symbol {
    name: name.to_owned(),
    line: number.parse().ok()?,
    kind: Kind::from_name(kind)?,
    container: container.to_owned(),
  };
  Some((path, symbol))
}

/// `<path>:<line>:<heading>`, where the path, which may hold a `:` itself, is one of `documents`.
fn heading_row<'a>(line: &'a str, documents: &HashSet<String>) -> Option<(&'a str, Heading)> {
  line.match_indices(':').find_map(|(at, _)| {
    let path = &line[..at];
    if !documents.contains(path) {
      return None;
    }
    let (number, text) = line[at + 1..].split_once(':')?;
    let heading = Heading {
      line: number.parse().ok()?,
      text: text.to_owned(),
    };
    Some((path, heading))
  })
}
