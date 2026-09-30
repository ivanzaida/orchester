//! The text of the index files built from the file records. [`crate::previous`] reads the same
//! formats back.

use std::fmt::Write as _;

use crate::{outline::Item, record::FileRecord};

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

/// `tree.txt`: `<path>\t<language>\t<lines>` per file, in the order of `records`.
pub(crate) fn tree(records: &[FileRecord]) -> String {
  let mut text = String::new();
  for record in records {
    let lines = record
      .lines
      .map_or_else(|| NO_LINES.to_owned(), |lines| lines.to_string());
    writeln!(text, "{}\t{}\t{lines}", record.path, record.language.name()).expect("writing to a String does not fail");
  }
  text
}

/// `symbols.tsv`: `<name>\t<path>\t<line>\t<kind>\t<container>` per definition, sorted by name,
/// then path and line.
pub(crate) fn symbols(records: &[FileRecord]) -> String {
  let mut rows: Vec<_> = records
    .iter()
    .flat_map(|record| record.symbols.iter().map(move |symbol| (record.path.as_str(), symbol)))
    .collect();
  rows.sort_by(|(left_path, left), (right_path, right)| {
    (&left.name, left_path, left.line, left.kind.name()).cmp(&(&right.name, right_path, right.line, right.kind.name()))
  });
  let mut text = String::new();
  for (path, symbol) in rows {
    writeln!(
      text,
      "{}\t{path}\t{}\t{}\t{}",
      symbol.name,
      symbol.line,
      symbol.kind.name(),
      symbol.container
    )
    .expect("writing to a String does not fail");
  }
  text
}

/// `docs.md`: `<path>:<line>:<heading>` per Markdown heading, as `rg -n` prints matches.
pub(crate) fn docs(records: &[FileRecord]) -> String {
  let mut text = String::new();
  for record in records {
    for heading in &record.headings {
      writeln!(text, "{}:{}:{}", record.path, heading.line, heading.text).expect("writing to a String does not fail");
    }
  }
  text
}
