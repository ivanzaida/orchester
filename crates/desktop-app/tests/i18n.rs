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

/// Every string literal in `source` that starts with an area's name and a dot (`"shell.screens.tasks"`),
/// wherever it is written: in a `t(...)` call, a table of menu rows, a `match`.
fn keys_in(source: &str, areas: &[&str]) -> Vec<String> {
  let mut keys = Vec::new();
  for area in areas {
    let opening = format!("\"{area}.");
    for part in source.split(opening.as_str()).skip(1) {
      if let Some((rest, _)) = part.split_once('"') {
        keys.push(format!("{area}.{rest}"));
      }
    }
  }
  keys.retain(|key| {
    key
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_'))
  });
  keys
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
fn every_key_named_in_code_exists() {
  let known = known_keys();
  let areas: Vec<&str> = fallback().areas.iter().map(|area| area.name).collect();
  let mut sources = Vec::new();
  rust_sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut sources);
  let named: BTreeSet<String> = sources.iter().flat_map(|source| keys_in(source, &areas)).collect();
  for scanned in ["shell.screens.overview", "task.tabs.summary", "common.newTab"] {
    assert!(named.contains(scanned), "the scan finds the keys the code names");
  }
  let missing: Vec<&String> = named.iter().filter(|key| !known.contains_key(*key)).collect();
  assert!(
    missing.is_empty(),
    "keys missing from the {FALLBACK_LOCALE} files: {missing:?}"
  );
}

#[test]
fn locales_fill_their_placeholders() {
  let app = App::new();
  i18n::install(&app).expect("bundled locales load");
  let label = app
    .i18n()
    .t_args("shell.tabs.projectScreen", [("project", "kontur"), ("screen", "Tasks")]);
  assert_eq!(&*label, "kontur · Tasks");
  assert_eq!(
    &*app.i18n().t_args("shell.status.mcpListening", [("port", 52341)]),
    "MCP on :52341"
  );
  assert_eq!(&*app.i18n().t("task.tabs.summary"), "Summary");
  assert_eq!(&*app.i18n().t("common.newTab"), "New tab");
}
