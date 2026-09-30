//! What the index holds of one tracked file, read from its content or taken over from an earlier
//! index.

use std::path::PathBuf;

use crate::{
  IndexError,
  git::Blob,
  language::Language,
  markdown::{self, Heading},
  outline::{self, Item, Kind},
  render,
};

/// A definition as `symbols.tsv` lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Symbol {
  pub(crate) name: String,
  pub(crate) line: usize,
  pub(crate) kind: Kind,
  /// The enclosing item, `impl Worktree`; empty at the top of the file.
  pub(crate) container: String,
}

/// A source file's outline document.
#[derive(Debug)]
pub(crate) enum Outline {
  Text(String),
  /// The unchanged file of the earlier index an incremental build starts from.
  Previous(PathBuf),
}

/// One tracked file in the index.
#[derive(Debug)]
pub(crate) struct FileRecord {
  pub(crate) path: String,
  pub(crate) language: Language,
  /// `None` for a file that is no text.
  pub(crate) lines: Option<usize>,
  pub(crate) symbols: Vec<Symbol>,
  /// The `outline/<path>.md` document, for a file of a language with a grammar whose path gets an
  /// outline file ([`crate::outline_file::outlined`]).
  pub(crate) outline: Option<Outline>,
  pub(crate) headings: Vec<Heading>,
}

impl FileRecord {
  /// A file whose content is not read: a link, a submodule, or a path the index cannot name.
  pub(crate) const fn without_content(path: String, language: Language) -> Self {
    Self {
      path,
      language,
      lines: None,
      symbols: Vec::new(),
      outline: None,
      headings: Vec::new(),
    }
  }

  /// The record of the file at `path` with content `blob`, with an outline when `outlined`.
  pub(crate) fn read(path: String, blob: Blob, outlined: bool) -> Result<Self, IndexError> {
    let Blob::Text(content) = blob else {
      return Ok(Self::without_content(path, Language::BINARY));
    };
    let language = Language::of(&path);
    let lines = line_count(&content);
    let mut record = Self {
      path,
      language,
      lines: Some(lines),
      symbols: Vec::new(),
      outline: None,
      headings: Vec::new(),
    };
    if language == Language::Markdown {
      record.headings = markdown::headings(&String::from_utf8_lossy(&content));
    } else if language.has_grammar() {
      let items = outline::items(language, &content).ok_or_else(|| IndexError::Parse {
        path: record.path.clone(),
      })?;
      record.symbols = symbols(&items, "");
      record.outline = outlined.then(|| Outline::Text(render::outline(&record, &items)));
    }
    Ok(record)
  }
}

/// Lines as an editor numbers them: a last line without a line break counts.
fn line_count(content: &[u8]) -> usize {
  let breaks = content.iter().filter(|byte| **byte == b'\n').count();
  breaks + usize::from(content.last().is_some_and(|last| *last != b'\n'))
}

/// `items` and everything nested in them, each inside `container`.
fn symbols(items: &[Item], container: &str) -> Vec<Symbol> {
  let mut found = Vec::new();
  for item in items {
    found.push(Symbol {
      name: item.name.clone(),
      line: item.start,
      kind: item.kind,
      container: container.to_owned(),
    });
    found.extend(symbols(&item.children, &format!("{} {}", item.kind.name(), item.name)));
  }
  found
}
