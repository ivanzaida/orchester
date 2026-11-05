//! Test suites for the `run_tests` tests: each runs `orchester-test-command`, which prints a report (`cat`) or copies
//! one to where the suite reads it (`copy`), from the report fixtures the core's parser tests read, committed in the
//! project's repository under `fixtures/`.

use std::{fs, path::Path};

use orchester_core::{
  resource::{CommandArg, CommandProgram, ReportFormat, SuiteCommand, SuitePath, TestLayer, TestReport, TestSuite},
  test_report::SuiteRun,
};

use super::{git, tools::TEST_COMMAND};

pub const CARGO: &str = include_str!("../../../core/tests/fixtures/test_reports/cargo.json");
pub const NODE_JUNIT: &str = include_str!("../../../core/tests/fixtures/test_reports/node.junit.xml");
pub const VITEST: &str = include_str!("../../../core/tests/fixtures/test_reports/vitest.json");
pub const PLAYWRIGHT: &str = include_str!("../../../core/tests/fixtures/test_reports/playwright.json");
pub const NODE_TAP: &str = include_str!("../../../core/tests/fixtures/test_reports/node.tap");

/// Commits `files` (path, content) in the repository at `root`, so each run's worktree has them.
pub fn commit(root: &Path, files: &[(&str, &str)]) {
  for (path, content) in files {
    let file = root.join(path);
    fs::create_dir_all(file.parent().expect("a folder")).expect("folder");
    fs::write(&file, content).expect("fixture");
    git(root, &["add", path]);
  }
  git(root, &["commit", "--quiet", "-m", "test fixtures"]);
}

/// The suite running `orchester-test-command` with `arguments`, whose report has `format`, read from the file `path`
/// when one is given.
pub fn suite(arguments: &[&str], format: ReportFormat, path: Option<&str>) -> TestSuite {
  let program = CommandProgram::new(TEST_COMMAND).expect("the test program");
  let arguments = arguments
    .iter()
    .map(|argument| CommandArg::new(argument).expect("an argument"))
    .collect();
  let command = SuiteCommand::new(program, arguments).expect("a command");
  let path = path.map(|path| SuitePath::new(path).expect("a path"));
  TestSuite::new(command, TestReport::new(format, path).expect("a report"), TestLayer::Unit)
}

/// The JSON answer of a `run_tests ok: {…}` call.
pub fn answer(call: &str) -> serde_json::Value {
  let json = call
    .strip_prefix("run_tests ok: ")
    .unwrap_or_else(|| panic!("run_tests answered a result: {call}"));
  serde_json::from_str(json).expect("the answer is JSON")
}

/// The result `run_tests` kept, at the path its answer names.
pub fn stored(answer: &serde_json::Value) -> SuiteRun {
  let file = answer["result"].as_str().expect("the result's path");
  serde_json::from_slice(&fs::read(file).expect("the result is kept")).expect("the result reads")
}
