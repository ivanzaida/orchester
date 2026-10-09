//! Every shipped locale has exactly the keys and placeholders of the fallback locale, and every key the
//! code names exists (lurq would show the raw key otherwise).

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::Path,
};

use lurq::app::App;
use orchester_app::i18n::{self, FALLBACK_LOCALE, LOCALES};
use serde_json::Value;

/// Leaf key → sorted `{{placeholder}}` names.
fn leaves(value: &Value, prefix: &str, out: &mut BTreeMap<String, Vec<String>>) {
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

fn locale_leaves(contents: &str) -> BTreeMap<String, Vec<String>> {
  let mut out = BTreeMap::new();
  leaves(
    &serde_json::from_str(contents).expect("locale file is valid JSON"),
    "",
    &mut out,
  );
  out
}

fn fallback() -> &'static str {
  LOCALES
    .iter()
    .find(|(locale, _)| *locale == FALLBACK_LOCALE)
    .map(|(_, contents)| *contents)
    .expect("the fallback locale ships")
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

/// Every string literal in `source` that starts with one of the locale's namespaces and a dot
/// (`"screens.tasks"`), wherever it is written: in a `t(...)` call, a table of menu rows, a `match`.
fn keys_in(source: &str, namespaces: &BTreeSet<String>) -> Vec<String> {
  let mut keys = Vec::new();
  for namespace in namespaces {
    let opening = format!("\"{namespace}.");
    for part in source.split(opening.as_str()).skip(1) {
      if let Some((rest, _)) = part.split_once('"') {
        keys.push(format!("{namespace}.{rest}"));
      }
    }
  }
  keys.retain(|key| key.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'.'));
  keys
}

#[test]
fn locales_load_into_lurq() {
  i18n::install(&App::new()).expect("bundled locales load");
}

#[test]
fn locales_have_identical_keys_and_placeholders() {
  let expected = locale_leaves(fallback());
  for (locale, contents) in LOCALES {
    assert_eq!(
      locale_leaves(contents),
      expected,
      "locale `{locale}` differs from `{FALLBACK_LOCALE}`"
    );
  }
}

#[test]
fn every_key_named_in_code_exists() {
  let known = locale_leaves(fallback());
  let namespaces: BTreeSet<String> = known
    .keys()
    .filter_map(|key| key.split_once('.').map(|(namespace, _)| namespace.to_owned()))
    .collect();
  let mut sources = Vec::new();
  rust_sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut sources);
  let named: BTreeSet<String> = sources.iter().flat_map(|source| keys_in(source, &namespaces)).collect();
  assert!(
    named.contains("screens.overview"),
    "the scan finds the keys the code names"
  );
  let missing: Vec<&String> = named.iter().filter(|key| !known.contains_key(*key)).collect();
  assert!(
    missing.is_empty(),
    "keys missing from {FALLBACK_LOCALE}.json: {missing:?}"
  );
}

#[test]
fn locales_fill_their_placeholders() {
  let app = App::new();
  i18n::install(&app).expect("bundled locales load");
  let label = app
    .i18n()
    .t_args("tabs.projectScreen", [("project", "kontur"), ("screen", "Tasks")]);
  assert_eq!(&*label, "kontur · Tasks");
  assert_eq!(
    &*app.i18n().t_args("status.mcpListening", [("port", 52341)]),
    "MCP on :52341"
  );
}
