//! What Orchester's tools read of the code index of the run's base revision (`crate::code_index`, the
//! `orchester-index` crate's files): the symbol tables (`symbols.tsv`, `symbols-consts.tsv`,
//! `symbols-tests.tsv`: `name<TAB>path<TAB>line<TAB>kind<TAB>container`), the outlines (`outline/<path>.md`,
//! `- L<start>-<end> <signature>`) and the file list (`tree.txt`, `path<TAB>language<TAB>lines`).
//!
//! `find_symbol` ranks hits as ORC-48 split the tables: the code's own definitions, then its constants and
//! statics, then the definitions in test folders (each only when asked for); within a table an exact name
//! before one that differs only in case, then a name that starts with the query, then one that holds it;
//! then by path and line. The index describes the base revision: a hit in a file changed in the worktree
//! since then is marked, as its line may have moved.

use std::{
  collections::HashSet,
  fmt::Write,
  fs::File,
  io::{self, BufRead, BufReader},
  num::NonZeroU32,
  path::{Path, PathBuf},
};

use serde::Deserialize;

/// The most hits one answer lists.
pub const MAX_HITS: usize = 50;
/// The most matches kept while a table is scanned, before they are ranked.
const MAX_KEPT: usize = 5000;
/// The longest outline file read for a signature.
const MAX_OUTLINE_BYTES: u64 = 1024 * 1024;
const SYMBOLS: &str = "symbols.tsv";
const CONSTS: &str = "symbols-consts.tsv";
const TESTS: &str = "symbols-tests.tsv";
const TREE: &str = "tree.txt";
const OUTLINES: &str = "outline";

/// The input of `find_symbol`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FindSymbol {
  name: String,
  #[serde(default)]
  kind: Option<String>,
  #[serde(default)]
  include_tests: Option<bool>,
  #[serde(default)]
  include_consts: Option<bool>,
}

impl FindSymbol {
  /// The input for exactly `name` in the code's own definitions, as `search` looks it up.
  pub fn definitions_of(name: &str) -> Self {
    Self {
      name: name.to_owned(),
      kind: None,
      include_tests: None,
      include_consts: None,
    }
  }

  pub fn name(&self) -> &str {
    &self.name
  }
}

/// One definition the index lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolHit {
  pub name: String,
  pub path: String,
  pub line: NonZeroU32,
  pub kind: String,
  pub container: String,
  /// From the file's outline, when it has one.
  pub signature: Option<String>,
  /// How well it matches: lower is better.
  rank: (u8, u8),
}

/// The code index of the run's base revision, in its folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeIndex {
  folder: PathBuf,
}

impl CodeIndex {
  pub fn new(folder: &Path) -> Self {
    Self {
      folder: folder.to_path_buf(),
    }
  }

  /// The definitions that match `query`, best first, at most [`MAX_HITS`]; `exact` keeps only names
  /// equal to it.
  pub fn find(&self, query: &FindSymbol, exact: bool) -> io::Result<Vec<SymbolHit>> {
    let mut tables = vec![(SYMBOLS, 0)];
    if query.include_consts.unwrap_or(false) {
      tables.push((CONSTS, 1));
    }
    if query.include_tests.unwrap_or(false) {
      tables.push((TESTS, 2));
    }
    let wanted = query.name.to_lowercase();
    let mut hits = Vec::new();
    for (table, order) in tables {
      let Some(lines) = self.lines(table)? else {
        continue;
      };
      for line in lines {
        let Some(hit) = symbol(&line?, (&query.name, &wanted), order) else {
          continue;
        };
        let kind_matches = query
          .kind
          .as_ref()
          .is_none_or(|kind| kind.eq_ignore_ascii_case(&hit.kind));
        if kind_matches && (!exact || hit.rank.1 == 0) {
          hits.push(hit);
          if hits.len() >= MAX_KEPT {
            rank(&mut hits);
          }
        }
      }
    }
    rank(&mut hits);
    for hit in &mut hits {
      hit.signature = self.signature(&hit.path, hit.line);
    }
    Ok(hits)
  }

  /// The files the index lists, relative to the repository root with `/`.
  pub fn files(&self) -> io::Result<Vec<String>> {
    let Some(lines) = self.lines(TREE)? else {
      return Ok(Vec::new());
    };
    lines
      .map(|line| line.map(|line| line.split('\t').next().unwrap_or_default().to_owned()))
      .filter(|path| path.as_ref().map_or(true, |path| !path.is_empty()))
      .collect()
  }

  /// The lines of index file `name`; `None` when the index has no such file.
  fn lines(&self, name: &str) -> io::Result<Option<io::Lines<BufReader<File>>>> {
    match File::open(self.folder.join(name)) {
      Ok(file) => Ok(Some(BufReader::new(file).lines())),
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
      Err(error) => Err(error),
    }
  }

  /// The signature `outline/<path>.md` gives the item starting at `line`.
  fn signature(&self, path: &str, line: NonZeroU32) -> Option<String> {
    let outline = self.folder.join(OUTLINES).join(format!("{path}.md"));
    if outline.metadata().ok()?.len() > MAX_OUTLINE_BYTES {
      return None;
    }
    let prefix = format!("L{line}-");
    BufReader::new(File::open(outline).ok()?)
      .lines()
      .map_while(Result::ok)
      .find_map(|entry| {
        let entry = entry.trim_start().strip_prefix("- ")?;
        let (range, signature) = entry.split_once(' ')?;
        range.starts_with(&prefix).then(|| signature.trim().to_owned())
      })
  }
}

/// The hit `line` of a table of `order` is for `(name, lowercase name)`, if it matches at all.
fn symbol(line: &str, (name, wanted): (&str, &str), order: u8) -> Option<SymbolHit> {
  let mut columns = line.split('\t');
  let (found, path, at, kind) = (columns.next()?, columns.next()?, columns.next()?, columns.next()?);
  let container = columns.next().unwrap_or_default();
  let lower = found.to_lowercase();
  let closeness = if found == name {
    0
  } else if lower == wanted {
    1
  } else if lower.starts_with(wanted) {
    2
  } else if lower.contains(wanted) {
    3
  } else {
    return None;
  };
  Some(SymbolHit {
    name: found.to_owned(),
    path: path.to_owned(),
    line: at.parse().ok()?,
    kind: kind.to_owned(),
    container: container.to_owned(),
    signature: None,
    rank: (order, closeness),
  })
}

/// `hits` best first, at most [`MAX_HITS`].
fn rank(hits: &mut Vec<SymbolHit>) {
  hits.sort_by(|a, b| (a.rank.0, a.rank.1, &a.path, a.line).cmp(&(b.rank.0, b.rank.1, &b.path, b.line)));
  hits.truncate(MAX_HITS);
}

/// `hits` as the agent reads them: `path:line kind name (in container)` and the signature, each hit in a file
/// of `changed` marked as changed since the index.
pub fn shown_hits(hits: &[SymbolHit], changed: &HashSet<String>) -> String {
  let mut text = String::new();
  for hit in hits {
    // Writing to a String cannot fail.
    let _ = write!(text, "{}:{} {} {}", hit.path, hit.line, hit.kind, hit.name);
    if !hit.container.is_empty() {
      let _ = write!(text, " (in {})", hit.container);
    }
    if changed.contains(&hit.path) {
      text.push_str(" [changed since the index: the line may have moved]");
    }
    text.push('\n');
    if let Some(signature) = &hit.signature {
      let _ = writeln!(text, "    {signature}");
    }
  }
  text
}
