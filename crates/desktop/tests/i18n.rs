use std::{collections::BTreeMap, fs, path::Path};

use lurq::app::App;
use orchester_desktop::i18n::{self, FALLBACK_LOCALE, LOCALES};
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

/// First string-literal argument of every `t(`, `t_args(` and `Message::new(` call, and every
/// `statusBar.*` literal (the status bar picks some keys by value, e.g. what an item opens).
fn keys_used_in(source: &str) -> Vec<String> {
  let mut keys = Vec::new();
  for call in [".t(\"", ".t_args(\"", "Message::new(\""] {
    for part in source.split(call).skip(1) {
      if let Some((key, _)) = part.split_once('"') {
        keys.push(key.to_owned());
      }
    }
  }
  for part in source.split("\"statusBar.").skip(1) {
    if let Some((key, _)) = part.split_once('"') {
      keys.push(format!("statusBar.{key}"));
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
fn locales_load_into_lurq() {
  i18n::install(&App::new()).unwrap();
}

#[test]
fn locales_have_identical_keys_and_placeholders() {
  let (_, fallback) = LOCALES.iter().find(|(locale, _)| *locale == FALLBACK_LOCALE).unwrap();
  let expected = locale_leaves(fallback);
  for (locale, contents) in LOCALES {
    assert_eq!(
      locale_leaves(contents),
      expected,
      "locale `{locale}` differs from `{FALLBACK_LOCALE}`"
    );
  }
}

#[test]
fn every_key_used_in_code_exists() {
  let (_, fallback) = LOCALES.iter().find(|(locale, _)| *locale == FALLBACK_LOCALE).unwrap();
  let known = locale_leaves(fallback);
  let mut sources = Vec::new();
  rust_sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut sources);
  let missing: Vec<String> = sources
    .iter()
    .flat_map(|source| keys_used_in(source))
    .filter(|key| !known.contains_key(key))
    .collect();
  assert!(
    missing.is_empty(),
    "keys missing from {FALLBACK_LOCALE}.json: {missing:?}"
  );
}

/// Counted minutes read naturally: "of its 1 minute", "of its 30 minutes".
#[test]
fn locales_count_minutes_with_plural_forms() {
  let app = App::new();
  i18n::install(&app).expect("bundled locales load");
  let detail = |key: &str, limit: &str| {
    app.i18n().t_args(
      key,
      [("run", "3"), ("used", "1"), ("limit", limit), ("agent", "Builder")],
    )
  };
  assert!(detail("runs.limit.timeDetail.one", "1").contains("used 1 of its 1 minute of active agent time"));
  assert!(detail("runs.limit.timeDetail.other", "30").contains("of its 30 minutes of"));
  assert!(detail("runs.limit.timeDetailUnsaved.one", "1").contains("of its 1 minute of"));
  let continue_with = app
    .i18n()
    .t_args("runs.limit.continueWithMinutes.one", [("minutes", "1")]);
  assert_eq!(&*continue_with, "Continue with 1 minute");
  let not_above = app.i18n().t_args("errors.timeLimitNotAboveUsed.one", [("used", "1")]);
  assert_eq!(&*not_above, "Enter more than the 1 minute this run already used.");
}

/// Task statuses read as the status label variants of the design (`P3HONM`): "Completed", not
/// "Finished"; "Needs input", not "Needs you".
#[test]
fn locales_name_task_statuses_as_designed() {
  let app = App::new();
  i18n::install(&app).expect("bundled locales load");
  let status = |phase: &str| app.i18n().t(&format!("tasks.status.{phase}"));
  let designed = [
    ("running", "Running"),
    ("queued", "Queued"),
    ("waiting", "Needs input"),
    ("paused", "Paused"),
    ("stoppedAtLimit", "Stopped at limit"),
    ("failed", "Failed"),
    ("succeeded", "Completed"),
    ("interrupted", "Interrupted"),
    ("cancelled", "Cancelled"),
    ("draft", "Draft"),
  ];
  for (phase, label) in designed {
    assert_eq!(&*status(phase), label, "tasks.status.{phase}");
  }
}
