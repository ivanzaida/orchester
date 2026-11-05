//! The execution rows a project's runs leave in its database, in a form two projects that ran the same scenario can be
//! compared in: every row of every execution table, with what each project makes of its own left out. Generated ids
//! (UUIDs) read `<id>`, git revisions `<revision>`, the project's folder `<root>`, the process ids the fake CLIs
//! report (`pid: <pid>`) and the loopback ports the runner's relays listen on (`127.0.0.1:<port>`), and times
//! `<time>`: a column or a JSON key named `at`, `…_at`, `…since…`, `…_ms` or `…duration…`. JSON text is compared as
//! its value, so key order does not count. Each table's rows are compared as a multiset of their canonical text.
//!
//! The transcript's own order is left out too ([`UNORDERED`]): a CLI's standard error and its events come through two
//! pipes, read as each has data, so the place of a stderr line among the events differs from one run to the next even
//! in one process.

use std::{
  collections::{BTreeMap, BTreeSet},
  path::Path,
};

use rusqlite::{Connection, OpenFlags, types::ValueRef};
use serde_json::Value;

/// The tables a run's execution writes, by the start of their names: runs and their attempts, step executions,
/// checkpoints, events, plans, budgets, permissions, failures and worktrees; invocations, their transcript and
/// prompts; step outputs and progress; the task's messages, questions and tool requests; reviews' usage, plan limits;
/// base notices and borrowed workspaces.
const EXECUTION_TABLES: &[&str] = &[
  "run",
  "attempts",
  "step_",
  "checkpoints",
  "plan",
  "invocation",
  "agent_events",
  "native_",
  "messages",
  "questions",
  "tool_approvals",
  "approval_interruptions",
  "reviewer_usage",
  "base_notices",
  "borrowing_runs",
  "workspace_waits",
];

/// The columns that only order a table's rows by when they were read, as `(table, column)`.
const UNORDERED: &[(&str, &str)] = &[("agent_events", "sequence")];

/// How many times each canonical row is in a table.
type Table = BTreeMap<String, usize>;

/// The execution rows of one project, by table.
#[derive(Debug, PartialEq, Eq)]
pub struct Rows(BTreeMap<String, Table>);

impl Rows {
  /// The rows the project at `root` holds now.
  pub fn of(root: &Path) -> Self {
    let db = Connection::open_with_flags(
      root.join(".orchester").join("orchester.db"),
      OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("open the project database");
    let folder = Folder::of(root);
    let tables: Vec<String> = db
      .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
      .expect("list tables")
      .query_map([], |row| row.get(0))
      .expect("list tables")
      .collect::<Result<_, _>>()
      .expect("table names");
    let rows = tables
      .into_iter()
      .filter(|table| EXECUTION_TABLES.iter().any(|start| table.starts_with(start)))
      .map(|table| {
        let rows = table_rows(&db, &table, &folder);
        (table, rows)
      })
      .collect();
    Self(rows)
  }

  /// The rows of `table`, each in its canonical text.
  pub fn table(&self, table: &str) -> Vec<&str> {
    let rows = self.0.get(table).unwrap_or_else(|| panic!("no table {table}"));
    rows
      .iter()
      .flat_map(|(row, count)| std::iter::repeat_n(row.as_str(), *count))
      .collect()
  }

  /// What differs from `other`, by table: each row this one holds more often (`-`) and each row `other` holds more
  /// often (`+`), cut to its first 600 characters; empty when they are the same.
  pub fn differences(&self, other: &Self) -> String {
    let mut found = String::new();
    let tables: BTreeSet<&String> = self.0.keys().chain(other.0.keys()).collect();
    let none = Table::new();
    for table in tables {
      let mine = self.0.get(table).unwrap_or(&none);
      let theirs = other.0.get(table).unwrap_or(&none);
      let rows: BTreeSet<&String> = mine.keys().chain(theirs.keys()).collect();
      for row in rows {
        let (here, there) = (
          mine.get(row).copied().unwrap_or(0),
          theirs.get(row).copied().unwrap_or(0),
        );
        let shown: String = row.chars().take(600).collect();
        if here > there {
          found.push_str(&format!("{table} -{} {shown}\n", here - there));
        } else if there > here {
          found.push_str(&format!("{table} +{} {shown}\n", there - here));
        }
      }
    }
    found
  }
}

fn table_rows(db: &Connection, table: &str, folder: &Folder) -> Table {
  let mut statement = db
    .prepare(&format!("SELECT * FROM \"{table}\""))
    .expect("read the table");
  let columns: Vec<String> = statement.column_names().into_iter().map(str::to_owned).collect();
  let mut rows = statement.query([]).expect("read the table");
  let mut found = Table::new();
  while let Some(row) = rows.next().expect("read a row") {
    let mut values = serde_json::Map::new();
    for (index, column) in columns.iter().enumerate() {
      if UNORDERED.contains(&(table, column.as_str())) {
        continue;
      }
      let value = row.get_ref(index).expect("read a value");
      values.insert(column.clone(), cell(column, value, folder));
    }
    *found.entry(Value::Object(values).to_string()).or_default() += 1;
  }
  found
}

/// The value of `column` in a row, normalized.
fn cell(column: &str, value: ValueRef<'_>, folder: &Folder) -> Value {
  if is_time(column) && value != ValueRef::Null {
    return Value::from("<time>");
  }
  match value {
    ValueRef::Null => Value::Null,
    ValueRef::Integer(number) => Value::from(number),
    ValueRef::Real(number) => Value::from(number),
    ValueRef::Text(text) => {
      let text = String::from_utf8_lossy(text);
      match serde_json::from_str::<Value>(&text) {
        Ok(json @ (Value::Object(_) | Value::Array(_))) => normalized(json, folder),
        _ => Value::from(folder.text(&text)),
      }
    }
    ValueRef::Blob(bytes) => Value::from(format!("<{} bytes>", bytes.len())),
  }
}

/// `json` with the project's own values left out, as in a cell.
fn normalized(json: Value, folder: &Folder) -> Value {
  match json {
    Value::Object(map) => Value::Object(
      map
        .into_iter()
        .map(|(key, value)| {
          let value = if is_time(&key) && !value.is_null() {
            Value::from("<time>")
          } else {
            normalized(value, folder)
          };
          (folder.text(&key), value)
        })
        .collect(),
    ),
    Value::Array(values) => Value::Array(values.into_iter().map(|value| normalized(value, folder)).collect()),
    Value::String(text) => match serde_json::from_str::<Value>(&text) {
      Ok(json @ (Value::Object(_) | Value::Array(_))) => normalized(json, folder),
      _ => Value::from(folder.text(&text)),
    },
    other => other,
  }
}

/// Whether a column or a key named `name` holds a time.
fn is_time(name: &str) -> bool {
  let name = name.to_ascii_lowercase();
  name == "at"
    || name.ends_with("_at")
    || name.ends_with("_ms")
    || name.contains("since")
    || name.contains("duration")
    || name.contains("elapsed")
}

/// The project's folder, in each way its path may be written.
struct Folder {
  /// Longest first, so a longer way is replaced before a shorter one inside it.
  ways: Vec<String>,
}

impl Folder {
  fn of(root: &Path) -> Self {
    let mut ways = BTreeSet::new();
    for path in [Some(root.to_path_buf()), root.canonicalize().ok()]
      .into_iter()
      .flatten()
    {
      let shown = path.display().to_string();
      let plain = shown.strip_prefix(r"\\?\").unwrap_or(&shown).to_owned();
      for way in [shown.clone(), plain] {
        ways.insert(way.replace('\\', "/"));
        ways.insert(way.replace('\\', r"\\"));
        ways.insert(way);
      }
    }
    // The folder's own name is the temporary directory's: unique to this project wherever it shows.
    if let Some(name) = root.file_name() {
      ways.insert(name.to_string_lossy().into_owned());
    }
    let mut ways: Vec<String> = ways.into_iter().filter(|way| !way.is_empty()).collect();
    ways.sort_by_key(|way| std::cmp::Reverse(way.len()));
    Self { ways }
  }

  /// `text` with the folder, generated ids, git revisions, process ids and loopback ports left out.
  fn text(&self, text: &str) -> String {
    let mut text = text.to_owned();
    for way in &self.ways {
      text = text.replace(way.as_str(), "<root>");
    }
    let text = number_after(&text, "pid: ", "<pid>");
    let text = number_after(&text, "127.0.0.1:", "<port>");
    words(&text)
  }
}

/// `text` with the number right after each `marker` read `token`.
fn number_after(text: &str, marker: &str, token: &str) -> String {
  let mut out = String::with_capacity(text.len());
  let mut rest = text;
  while let Some(at) = rest.find(marker) {
    let after = &rest[at + marker.len()..];
    let digits = after.len() - after.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    out.push_str(&rest[..at + marker.len()]);
    if digits > 0 {
      out.push_str(token);
    }
    rest = &after[digits..];
  }
  out.push_str(rest);
  out
}

/// `text` with each UUID read `<id>` and each git revision `<revision>`: in words of letters, digits and dashes.
fn words(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  let mut word = String::new();
  for c in text.chars() {
    if c.is_ascii_alphanumeric() || c == '-' {
      word.push(c);
    } else {
      out.push_str(&word_of(&word));
      word.clear();
      out.push(c);
    }
  }
  out.push_str(&word_of(&word));
  out
}

/// `word`, ASCII letters, digits and dashes, with its revision or the UUIDs in it left out.
fn word_of(word: &str) -> String {
  if word.len() == 40 && word.chars().all(|c| c.is_ascii_hexdigit()) {
    return "<revision>".to_owned();
  }
  let mut out = String::with_capacity(word.len());
  let mut at = 0;
  while at < word.len() {
    if word.get(at..at + 36).is_some_and(is_uuid) {
      out.push_str("<id>");
      at += 36;
    } else {
      out.push_str(&word[at..=at]);
      at += 1;
    }
  }
  out
}

fn is_uuid(word: &str) -> bool {
  let parts: Vec<&str> = word.split('-').collect();
  parts.len() == 5
    && parts.iter().map(|part| part.len()).eq([8, 4, 4, 4, 12])
    && parts.iter().all(|part| part.chars().all(|c| c.is_ascii_hexdigit()))
}
