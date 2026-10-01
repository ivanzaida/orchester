//! The text of the index files built from the file records. [`crate::previous`] reads the same
//! formats back.

use std::{
  fmt::Write as _,
  io::{self, Write},
};

use crate::{
  map::TestFolders,
  outline::{Item, Kind},
  record::FileRecord,
};

/// What `tree.txt` shows for a file without a line count.
pub(crate) const NO_LINES: &str = "-";

/// `outline/<path>.md` of `record`, whose definitions are `items`.
pub(crate) fn outline(record: &FileRecord, items: &[Item]) -> String {
  let mut document = format!(
    "# {}\n{}, {} lines, {} symbols\n\n",
    record.path,
    record.language.name(),
    record.lines.unwrap_or(0),
    record.symbols.len()
  );
  write_items(&mut document, items, 0);
  document
}

fn write_items(document: &mut String, items: &[Item], depth: usize) {
  for item in items {
    writeln!(
      document,
      "{:indent$}- L{}-{} {}",
      "",
      item.start,
      item.end,
      item.signature,
      indent = depth * 2
    )
    .expect("writing to a String does not fail");
    write_items(document, &item.children, depth + 1);
  }
}

/// Writes `tree.txt`: `<path>\t<language>\t<lines>` per file, in the order of `records`.
pub(crate) fn tree(out: &mut impl Write, records: &[FileRecord]) -> io::Result<()> {
  for record in records {
    match record.lines {
      Some(lines) => writeln!(out, "{}\t{}\t{lines}", record.path, record.language.name())?,
      None => writeln!(out, "{}\t{}\t{NO_LINES}", record.path, record.language.name())?,
    }
  }
  Ok(())
}

/// The symbols file that lists a definition. A search by name in `symbols.tsv` finds the types,
/// functions and methods of the code itself first: the definitions of test files and the constants
/// and statics, which outnumber them, are listed apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SymbolList {
  Main,
  Constants,
  Tests,
}

impl SymbolList {
  pub(crate) const ALL: [Self; 3] = [Self::Main, Self::Constants, Self::Tests];

  pub(crate) const fn file(self) -> &'static str {
    match self {
      Self::Main => "symbols.tsv",
      Self::Constants => "symbols-consts.tsv",
      Self::Tests => "symbols-tests.tsv",
    }
  }

  /// The list of a definition of `kind` in the file at `path`, of the repository whose test folders are `tests`.
  fn of(path: &str, kind: Kind, tests: &TestFolders) -> Self {
    if tests.hold(path) {
      Self::Tests
    } else if matches!(kind, Kind::Const | Kind::Static) {
      Self::Constants
    } else {
      Self::Main
    }
  }
}

/// Writes the symbols file of `list`: `<name>\t<path>\t<line>\t<kind>\t<container>` per definition
/// it lists, sorted by name, then path and line. `tests` are the test folders of the repository.
pub(crate) fn symbols(
  out: &mut impl Write,
  records: &[FileRecord],
  list: SymbolList,
  tests: &TestFolders,
) -> io::Result<()> {
  let mut rows: Vec<_> = records
    .iter()
    .flat_map(|record| record.symbols.iter().map(move |symbol| (record.path.as_str(), symbol)))
    .filter(|(path, symbol)| SymbolList::of(path, symbol.kind, tests) == list)
    .collect();
  rows.sort_unstable_by(|(left_path, left), (right_path, right)| {
    (&left.name, left_path, left.line, left.kind.name()).cmp(&(&right.name, right_path, right.line, right.kind.name()))
  });
  for (path, symbol) in rows {
    writeln!(
      out,
      "{}\t{path}\t{}\t{}\t{}",
      symbol.name,
      symbol.line,
      symbol.kind.name(),
      symbol.container
    )?;
  }
  Ok(())
}

/// Writes `docs.md`: `<path>:<line>:<heading>` per Markdown heading, as `rg -n` prints matches.
pub(crate) fn docs(out: &mut impl Write, records: &[FileRecord]) -> io::Result<()> {
  for record in records {
    for heading in &record.headings {
      writeln!(out, "{}:{}:{}", record.path, heading.line, heading.text)?;
    }
  }
  Ok(())
}
