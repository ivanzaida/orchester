//! The locale files, one per locale and area: each area's file holds its own keys only, so no key is in
//! two files; every shipped locale has exactly the areas, keys and placeholders of the fallback locale;
//! and every key the code names exists (lurq would show the raw key otherwise).

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::Path,
};

use lurq::app::App;
use orchester_app::i18n::{self, Area, FALLBACK_LOCALE, LOCALES, Locale};
use serde_json::Value;

/// Leaf key → sorted `{{placeholder}}` names.
type Leaves = BTreeMap<String, Vec<String>>;

fn leaves(value: &Value, prefix: &str, out: &mut Leaves) {
  match value {
    Value::Object(map) => {
      for (key, child) in map {
        let path = if prefix.is_empty() {
          key.clone()
        } else {
          format!("{prefix}.{key}")
        };
        leaves(child, &path, out);
      }
    }
    Value::String(text) => {
      let mut names: Vec<String> = text
        .split("{{")
        .skip(1)
        .filter_map(|part| part.split_once("}}").map(|(name, _)| name.trim().to_owned()))
        .collect();
      names.sort();
      out.insert(prefix.to_owned(), names);
    }
    other => panic!("{prefix}: locale values must be strings or objects, got {other}"),
  }
}

fn json(area: &Area) -> Value {
  serde_json::from_str(area.contents).unwrap_or_else(|error| panic!("{}.json is not JSON: {error}", area.name))
}

fn area_leaves(area: &Area) -> Leaves {
  let mut out = BTreeMap::new();
  leaves(&json(area), "", &mut out);
  out
}

fn fallback() -> &'static Locale {
  LOCALES
    .iter()
    .find(|locale| locale.name == FALLBACK_LOCALE)
    .expect("the fallback locale ships")
}

/// Every key of the fallback locale, with its placeholders.
fn known_keys() -> Leaves {
  fallback().areas.iter().flat_map(area_leaves).collect()
}

fn rust_sources(dir: &Path, out: &mut Vec<String>) {
  for entry in fs::read_dir(dir).expect("source directory is readable") {
    let path = entry.expect("source directory entry is readable").path();
    if path.is_dir() {
      rust_sources(&path, out);
    } else if path.extension().is_some_and(|extension| extension == "rs") {
      out.push(fs::read_to_string(path).expect("source file is UTF-8"));
    }
  }
}

/// What a dotted literal ends in when it names a file rather than a key (`"reg.exe"`, `"orchester.db"`).
const FILE_EXTENSIONS: [&str; 14] = [
  "db", "exe", "ico", "json", "lock", "log", "md", "png", "rc", "rs", "svg", "toml", "ttf", "txt",
];

/// The string literals of `source` outside line comments, wherever they are written: a `t(...)` call, a
/// `*_key` table, a menu row. Char literals (`'"'`) do not open a string; a raw string is read like a
/// plain one, which is enough for the keys.
fn string_literals(source: &str) -> Vec<String> {
  let chars: Vec<char> = source.chars().collect();
  let mut literals = Vec::new();
  let mut at = 0;
  while at < chars.len() {
    at = match (chars[at], chars.get(at + 1), chars.get(at + 2)) {
      ('/', Some('/'), _) => skip_to(&chars, at, '\n'),
      ('\'', Some('\\'), _) => skip_to(&chars, at + 2, '\''),
      ('\'', _, Some('\'')) => at + 2,
      ('"', ..) => {
        let (literal, end) = read_string(&chars, at + 1);
        literals.push(literal);
        end
      }
      _ => at,
    } + 1;
  }
  literals
}

/// The index of the first `end` at or after `from`, or the end of `chars`.
fn skip_to(chars: &[char], from: usize, end: char) -> usize {
  chars[from..]
    .iter()
    .position(|c| *c == end)
    .map_or(chars.len(), |offset| from + offset)
}

/// The string that starts at `from`, escapes kept as the character they escape, and the index of its
/// closing quote.
fn read_string(chars: &[char], from: usize) -> (String, usize) {
  let mut literal = String::new();
  let mut at = from;
  while at < chars.len() && chars[at] != '"' {
    if chars[at] == '\\' {
      at += 1;
    }
    literal.extend(chars.get(at));
    at += 1;
  }
  (literal, at)
}

/// What a dotted literal starts with when it is a reverse-DNS name rather than a key (`"com.apple.universalaccess"`).
const DOMAIN_ROOTS: [&str; 1] = ["com"];

/// A literal that reads as a key: a first segment of lowercase letters and `_` (an area's name), then
/// segments of letters, digits and `_`, and not a file name or a reverse-DNS name.
fn is_key_shaped(literal: &str) -> bool {
  let Some((area, rest)) = literal.split_once('.') else {
    return false;
  };
  let segment = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
  let last = rest.rsplit('.').next().unwrap_or(rest);
  !area.is_empty()
    && area.bytes().all(|byte| byte.is_ascii_lowercase() || byte == b'_')
    && rest.split('.').all(segment)
    && !FILE_EXTENSIONS.contains(&last)
    && !DOMAIN_ROOTS.contains(&area)
}

/// Every key-shaped literal in the crate's sources.
fn keys_named_in_code() -> BTreeSet<String> {
  let mut sources = Vec::new();
  rust_sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut sources);
  sources
    .iter()
    .flat_map(|source| string_literals(source))
    .filter(|literal| is_key_shaped(literal))
    .collect()
}

/// The keys of `keys` whose area, their first segment, is not one of `areas`.
fn without_area<'a>(keys: &'a BTreeSet<String>, areas: &[&str]) -> Vec<&'a String> {
  keys
    .iter()
    .filter(|key| !key.split('.').next().is_some_and(|area| areas.contains(&area)))
    .collect()
}

#[test]
fn locales_load_into_lurq() {
  i18n::install(&App::new()).expect("bundled locales load");
}

#[test]
fn an_area_file_holds_its_own_keys_only_so_no_key_is_in_two_files() {
  for locale in LOCALES {
    let mut owners: BTreeMap<String, &str> = BTreeMap::new();
    for area in locale.areas {
      let top: Vec<String> = json(area)
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default();
      assert_eq!(
        top,
        [area.name],
        "{}/{}.json holds its keys under `{}` and nothing else",
        locale.name,
        area.name,
        area.name
      );
      for key in area_leaves(area).into_keys() {
        if let Some(other) = owners.insert(key.clone(), area.name) {
          panic!("`{key}` is in {}/{other}.json and {}.json", locale.name, area.name);
        }
      }
    }
  }
}

#[test]
fn locales_have_the_areas_keys_and_placeholders_of_the_fallback() {
  let expected: BTreeMap<&str, Leaves> = fallback()
    .areas
    .iter()
    .map(|area| (area.name, area_leaves(area)))
    .collect();
  for locale in LOCALES {
    let areas: BTreeMap<&str, Leaves> = locale.areas.iter().map(|area| (area.name, area_leaves(area))).collect();
    assert_eq!(
      areas.keys().collect::<Vec<_>>(),
      expected.keys().collect::<Vec<_>>(),
      "locale `{}` has the areas of `{FALLBACK_LOCALE}`",
      locale.name
    );
    for (area, leaves) in &areas {
      assert_eq!(
        leaves, &expected[area],
        "{}/{area}.json differs from {FALLBACK_LOCALE}/{area}.json",
        locale.name
      );
    }
  }
}

#[test]
fn every_key_named_in_code_has_its_area_file_and_exists() {
  let known = known_keys();
  let areas: Vec<&str> = fallback().areas.iter().map(|area| area.name).collect();
  let named = keys_named_in_code();
  for scanned in ["shell.screens.overview", "task.tabs.summary", "common.newTab"] {
    assert!(named.contains(scanned), "the scan finds the keys the code names");
  }
  let orphans = without_area(&named, &areas);
  assert!(
    orphans.is_empty(),
    "keys whose area has no file in {FALLBACK_LOCALE}/: {orphans:?}"
  );
  let missing: Vec<&String> = named.iter().filter(|key| !known.contains_key(*key)).collect();
  assert!(
    missing.is_empty(),
    "keys missing from the {FALLBACK_LOCALE} files: {missing:?}"
  );
}

#[test]
fn every_key_scan_reads_code_literals_and_flags_a_missing_area() {
  let source = r#"
    let title = ctx.t("tasks.title"); // ctx.t("notes.title") is a comment
    const QUOTE: char = '"';
    let label = ctx.t_args("shell.tabs.projectScreen", [("project", name)]);
    let program = "reg.exe"; let font = "lucide.ttf"; let address = "127.0.0.1";
    const ENTRIES: [&str; 1] = ["common.newTab"];
  "#;
  let keys: BTreeSet<String> = string_literals(source)
    .into_iter()
    .filter(|literal| is_key_shaped(literal))
    .collect();
  assert_eq!(
    keys.iter().map(String::as_str).collect::<Vec<_>>(),
    ["common.newTab", "shell.tabs.projectScreen", "tasks.title"]
  );
  assert_eq!(
    without_area(&keys, &["common", "shell"]),
    [&"tasks.title".to_owned()],
    "a key of an area without a file is caught"
  );
}

#[test]
fn locales_fill_their_placeholders() {
  let app = App::new();
  i18n::install(&app).expect("bundled locales load");
  let title = app.i18n().t_args("shell.app.windowTitle", [("tab", "Tasks")]);
  assert_eq!(&*title, "Tasks — Orchester");
  assert_eq!(
    &*app
      .i18n()
      .t_args("shell.status.mcpListening", [("address", "127.0.0.1:47821")]),
    "MCP 127.0.0.1:47821"
  );
  assert_eq!(&*app.i18n().t("task.tabs.summary"), "Summary");
  assert_eq!(&*app.i18n().t("common.newTab"), "New tab");
}
