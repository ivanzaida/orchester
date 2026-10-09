//! The workspace's locale files: every shipped locale has the keys and placeholders of the fallback
//! one, every key the sources name is there, and every key there is named by a source.

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::Path,
};

use orchester_workspace::i18n::{FALLBACK_LOCALE, LOCALES};
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

fn fallback() -> BTreeMap<String, Vec<String>> {
  let (_, contents) = LOCALES
    .iter()
    .find(|(locale, _)| *locale == FALLBACK_LOCALE)
    .expect("the fallback locale is shipped");
  locale_leaves(contents)
}

/// Every string literal of the sources that reads like a key of one of the locale's sections
/// (dotted, starting with a section's name), and the first literal of every `Message::new(`, `t(`
/// and `t_args(` call, whatever it starts with.
fn named_keys(sections: &BTreeSet<&str>) -> BTreeSet<String> {
  let mut sources = Vec::new();
  rust_sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut sources);
  let mut keys = BTreeSet::new();
  for source in &sources {
    for call in ["Message::new(\"", ".t(\"", ".t_args(\""] {
      for part in source.split(call).skip(1) {
        if let Some((key, _)) = part.split_once('"') {
          keys.insert(key.to_owned());
        }
      }
    }
    // Every text between two quotes: a literal, or the code between two literals, which never reads
    // as a dotted key.
    let quoted = source.match_indices('"').filter_map(|(at, _)| {
      let rest = &source[at + 1..];
      rest.find('"').map(|end| &rest[..end])
    });
    for literal in quoted {
      let dotted = literal.contains('.')
        && literal
          .chars()
          .all(|character| character.is_ascii_alphanumeric() || character == '.' || character == '_');
      if dotted
        && literal
          .split('.')
          .next()
          .is_some_and(|section| sections.contains(section))
      {
        keys.insert(literal.to_owned());
      }
    }
  }
  keys
}

fn rust_sources(dir: &Path, out: &mut Vec<String>) {
  for entry in fs::read_dir(dir).expect("source directory is readable") {
    let path = entry.expect("source directory entry is readable").path();
    if path.is_dir() {
      rust_sources(&path, out);
    } else if path.extension().is_some_and(|ext| ext == "rs") {
      out.push(fs::read_to_string(path).expect("source file is UTF-8"));
    }
  }
}

#[test]
fn locales_have_identical_keys_and_placeholders() {
  let expected = fallback();
  for (locale, contents) in LOCALES {
    assert_eq!(
      locale_leaves(contents),
      expected,
      "locale `{locale}` differs from `{FALLBACK_LOCALE}`"
    );
  }
}

#[test]
fn locales_hold_every_key_the_sources_name() {
  let known = fallback();
  let sections: BTreeSet<&str> = known.keys().filter_map(|key| key.split('.').next()).collect();
  let missing: Vec<String> = named_keys(&sections)
    .into_iter()
    .filter(|key| !known.contains_key(key))
    .collect();
  assert!(
    missing.is_empty(),
    "keys missing from {FALLBACK_LOCALE}.json: {missing:?}"
  );
}

/// Words the sources no longer name belong to no one: an app's own words live in its own files.
#[test]
fn locales_hold_only_keys_the_sources_name() {
  let known = fallback();
  let sections: BTreeSet<&str> = known.keys().filter_map(|key| key.split('.').next()).collect();
  let named = named_keys(&sections);
  let unused: Vec<&String> = known.keys().filter(|key| !named.contains(*key)).collect();
  assert!(unused.is_empty(), "keys no source names: {unused:?}");
}
