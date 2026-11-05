//! A `run_tests` call kept with the run (`orchester_core::test_report::SuiteRun`) and what the agent is answered from
//! it: the totals and each failure with its message, file:line, excerpt and artifacts, never the tests' captured
//! output; a report that cannot be read is an error with the exit code and the last lines, never a pass; and one test's
//! output through `test_output`. Also the artifact globs a suite copies its files by.

use orchester_core::{
  resource::ArtifactGlob,
  test_report::{ReportFormat, StoredArtifact, SuiteRun, SuiteRunEnd, TestRunId, last_lines, test_name},
};
use serde_json::Value;

use super::{CARGO, read};

fn run(end: SuiteRunEnd, exit_code: Option<i32>) -> SuiteRun {
  SuiteRun {
    id: TestRunId::FIRST,
    suite: "unit".to_owned(),
    filter: Some("tests".to_owned()),
    exit_code,
    duration_ms: 1200,
    end,
    artifacts: vec![StoredArtifact {
      source: "target/shot.png".to_owned(),
      stored: "/runs/1/tests/t1/artifacts/target/shot.png".to_owned(),
      test: Some("tests > subtracts".to_owned()),
    }],
    artifact_problems: Vec::new(),
  }
}

fn read_cargo(exit_code: Option<i32>) -> SuiteRun {
  run(
    SuiteRunEnd::Read {
      report: read(ReportFormat::CargoJson, CARGO),
    },
    exit_code,
  )
}

#[test]
fn test_runs_answer_totals_and_failures_without_captured_output() {
  let answer = read_cargo(Some(101)).answer("/runs/1/tests/t1/result.json");
  assert!(!answer.is_error());
  assert!(!answer.text().contains("subtracting 5 - 3"), "{}", answer.text());
  let json: Value = serde_json::from_str(answer.text()).expect("JSON");
  assert_eq!(json["run"], "t1");
  assert_eq!(json["passed"], false);
  assert_eq!(json["exit_code"], 101);
  assert_eq!(json["totals"]["failed"], 2);
  assert_eq!(json["result"], "/runs/1/tests/t1/result.json");
  let subtracts = json["failures"]
    .as_array()
    .expect("failures")
    .iter()
    .find(|failure| failure["name"] == "tests > subtracts")
    .expect("the failure is listed");
  assert_eq!(subtracts["outcome"], "failed");
  assert_eq!(subtracts["location"], r"src\lib.rs:21");
  assert!(
    subtracts["message"]
      .as_str()
      .is_some_and(|message| message.starts_with("assertion"))
  );
  assert!(subtracts["excerpt"].is_string());
  assert_eq!(
    subtracts["artifacts"],
    serde_json::json!(["/runs/1/tests/t1/artifacts/target/shot.png"])
  );
}

#[test]
fn test_runs_pass_only_with_a_clean_report_and_exit_code_zero() {
  let clean = r#"{"type":"suite","event":"started","test_count":1}
{"type":"test","event":"started","name":"adds"}
{"type":"test","name":"adds","event":"ok"}
{"type":"suite","event":"ok","passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":0}"#;
  let passing = |exit_code| {
    run(
      SuiteRunEnd::Read {
        report: read(ReportFormat::CargoJson, clean),
      },
      exit_code,
    )
  };
  assert!(passing(Some(0)).passed());
  let crashed = passing(Some(3));
  assert!(!crashed.passed(), "an exit code fails a report without failures");
  let json: Value = serde_json::from_str(crashed.answer("r").text()).expect("JSON");
  assert_eq!(json["passed"], false);
  assert!(json["note"].as_str().is_some_and(|note| note.contains("exited with 3")));
  assert!(
    !read_cargo(Some(0)).passed(),
    "failed tests fail it whatever the exit code"
  );
}

#[test]
fn test_runs_unreadable_report_or_timeout_is_an_error_never_a_pass() {
  let unreadable = run(
    SuiteRunEnd::Unreadable {
      error: "the program wrote no report at `target/junit.xml`".to_owned(),
      last_lines: last_lines("pid: 1\nbuilding\n", "error: linker failed\n"),
    },
    Some(1),
  );
  assert!(!unreadable.passed());
  let answer = unreadable.answer("/runs/1/tests/t1/result.json");
  assert!(answer.is_error());
  assert_eq!(
    answer.text(),
    "the suite `unit` did not pass: its report cannot be read (the program wrote no report at `target/junit.xml`); \
     exit code 1. Its last lines:\npid: 1\nbuilding\nerror: linker failed\n[kept as t1 in /runs/1/tests/t1/result.json]"
  );
  let timed_out = run(SuiteRunEnd::TimedOut { timeout_secs: 600 }, None);
  assert!(!timed_out.passed());
  let answer = timed_out.answer("r");
  assert!(answer.is_error());
  assert!(
    answer.text().contains("did not finish within 600 seconds"),
    "{}",
    answer.text()
  );
  assert!(unreadable.test_output("adds").is_error(), "no report, no test's output");
}

#[test]
fn test_runs_test_output_gives_one_tests_output_by_its_name() {
  let kept = read_cargo(Some(101));
  let output = kept.test_output("tests > subtracts");
  assert!(!output.is_error());
  assert!(
    output
      .text()
      .starts_with("tests > subtracts: failed at src\\lib.rs:21\nmessage:\nassertion"),
    "{}",
    output.text()
  );
  assert!(
    output.text().contains("output:\nsubtracting 5 - 3"),
    "{}",
    output.text()
  );
  assert_eq!(
    kept.test_output("subtracts").text(),
    output.text(),
    "its own name, when one test has it"
  );
  assert!(kept.test_output("multiplies").is_error());
}

#[test]
fn test_runs_are_kept_and_read_back_as_json() {
  let kept = read_cargo(Some(101));
  let json = serde_json::to_string(&kept).expect("serializes");
  assert_eq!(serde_json::from_str::<SuiteRun>(&json).expect("reads"), kept);
  assert_eq!(
    "t7".parse::<TestRunId>().map(|id| id.next().to_string()),
    Ok("t8".to_owned())
  );
  for refused in ["t0", "7", "t", "tx", "t1234567890"] {
    assert!(refused.parse::<TestRunId>().is_err(), "{refused}");
  }
  let report = kept.report().expect("read");
  assert!(
    report
      .tests()
      .iter()
      .any(|case| test_name(case) == "tests > nested > deep")
  );
}

#[test]
fn test_runs_artifact_globs_match_within_a_name_and_across_folders() {
  let glob = |value: &str| ArtifactGlob::new(value).expect("a glob");
  let pngs = glob("test-results/**/*.png");
  assert_eq!(pngs.base(), ["test-results"]);
  assert!(pngs.matches("test-results/a.png"));
  assert!(pngs.matches("test-results/cart/chromium/failed-1.png"));
  assert!(!pngs.matches("test-results/a.zip"));
  assert!(!pngs.matches("other/a.png"));
  let one = glob("target/report-?.xml");
  assert_eq!(one.base(), ["target"]);
  assert!(one.matches("target/report-1.xml"));
  assert!(!one.matches("target/report-12.xml"));
  assert!(!one.matches("target/nested/report-1.xml"));
  let plain = glob(r"target\junit.xml");
  assert_eq!(plain.base(), ["target"]);
  assert!(plain.matches("target/junit.xml"));
  assert!(glob("**/*.log").base().is_empty());
  assert!(glob("**/*.log").matches("a.log"));
}
