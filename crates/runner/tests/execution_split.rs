//! The split of `src/execution/` (`docs/architecture/remote-runners.md`, "The split"): the coordinator's modules sit
//! under `coordinator/`, one run's work under `executor/`, and the executor does not name the coordinator. The
//! places that still do are listed in [`KNOWN`]; a new one fails, and so does one that is gone and still listed.

use std::{
  collections::BTreeSet,
  fs,
  path::{Path, PathBuf},
};

/// Executor code that names a coordinator module today, as `(file under executor/, module)`: the run's task reads the
/// project's daily budget and maps refusals through `idle`, and takes and gives back agent slots on the `board` the
/// scheduler shares with it.
const KNOWN: &[(&str, &str)] = &[
  ("halt.rs", "idle"),
  ("permissions.rs", "idle"),
  ("plan.rs", "idle"),
  ("run.rs", "board"),
];

fn execution() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("src/execution")
}

fn rust_files(folder: &Path, found: &mut Vec<PathBuf>) {
  let entries = fs::read_dir(folder).unwrap_or_else(|error| panic!("cannot list {}: {error}", folder.display()));
  for entry in entries {
    let path = entry.expect("a folder entry can be read").path();
    if path.is_dir() {
      rust_files(&path, found);
    } else if path.extension().is_some_and(|extension| extension == "rs") {
      found.push(path);
    }
  }
}

/// The coordinator modules `line` names, outside comments: what follows each `coordinator::`.
fn named_coordinator_modules(line: &str) -> Vec<String> {
  let code = line.split("//").next().unwrap_or_default();
  code
    .match_indices("coordinator")
    .filter(|(at, _)| {
      let before = code[..*at].chars().next_back();
      !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
    .map(|(at, word)| {
      let rest = &code[at + word.len()..];
      rest
        .strip_prefix("::")
        .map(|rest| rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect())
        .unwrap_or_default()
    })
    .collect()
}

#[test]
fn every_execution_file_but_the_scheduler_sits_under_the_coordinator_or_the_executor() {
  let execution = execution();
  let mut files = Vec::new();
  rust_files(&execution, &mut files);
  let elsewhere: Vec<_> = files
    .iter()
    .map(|file| file.strip_prefix(&execution).expect("the file is under src/execution"))
    .filter(|file| *file != Path::new("mod.rs") && !file.starts_with("coordinator") && !file.starts_with("executor"))
    .collect();
  assert!(
    elsewhere.is_empty(),
    "outside coordinator/ and executor/: {elsewhere:?}"
  );
}

#[test]
fn executor_names_no_coordinator_module_beyond_the_known_places() {
  let executor = execution().join("executor");
  let mut files = Vec::new();
  rust_files(&executor, &mut files);
  assert!(!files.is_empty(), "no Rust file under {}", executor.display());
  let mut named = BTreeSet::new();
  for file in &files {
    let text = fs::read_to_string(file).unwrap_or_else(|error| panic!("cannot read {}: {error}", file.display()));
    let relative = file
      .strip_prefix(&executor)
      .expect("the file is under executor/")
      .to_string_lossy()
      .replace('\\', "/");
    for line in text.lines() {
      for module in named_coordinator_modules(line) {
        named.insert((relative.clone(), module));
      }
    }
  }
  let known: BTreeSet<(String, String)> = KNOWN
    .iter()
    .map(|(file, module)| ((*file).to_owned(), (*module).to_owned()))
    .collect();
  let new: Vec<_> = named.difference(&known).collect();
  assert!(
    new.is_empty(),
    "executor code names coordinator modules; reach the project through the store or the scheduler handle instead: {new:?}"
  );
  let gone: Vec<_> = known.difference(&named).collect();
  assert!(gone.is_empty(), "no longer named; remove them from KNOWN: {gone:?}");
}

#[test]
fn a_coordinator_path_in_code_is_found_and_one_in_a_comment_is_not() {
  assert_eq!(
    named_coordinator_modules("use crate::execution::coordinator::idle;"),
    ["idle"]
  );
  assert_eq!(
    named_coordinator_modules("use super::super::coordinator::{board::Board, idle};"),
    [""]
  );
  assert!(
    named_coordinator_modules("//! the coordinator's side is `crate::execution::coordinator::plan`'s").is_empty()
  );
  assert!(named_coordinator_modules("let x = 1; // see coordinator::idle").is_empty());
  assert!(named_coordinator_modules("let subcoordinator = 1;").is_empty());
}
