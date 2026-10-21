//! Story points' rules (ORC-49): the Fibonacci scale, when automation's estimate is wanted (never over a person's
//! value, again after the brief or criteria change), the calibration of merged tasks and the examples an estimate
//! reads, and the estimator's answer as each CLI prints it. `cargo test -p orchester-core --test story_points
//! story_points`.

use std::path::Path;

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::reviewer::{CliEstimator, EstimatorCli, ReviewFolder, read_codex_estimate},
  id::TaskId,
  name::Name,
  resource::ModelId,
  run::review::{ReviewError, ReviewSpend},
  task::{
    TaskDetails, TaskLimits,
    estimate::{
      EstimateBasis, EstimateInput, EstimateSource, PointsRefused, StoryEstimate, StoryPoints,
      calibration::{CalibrationRecord, TaskEffort, calibration, comparables},
      check_automation, estimate_prompt, parse_estimate_text,
    },
  },
  time::Timestamp,
};

fn points(value: u32) -> StoryPoints {
  StoryPoints::new(value).expect("on the scale")
}

fn details(title: &str, brief: &str, criteria: &[&str]) -> TaskDetails {
  TaskDetails {
    title: Name::new(title).expect("title"),
    brief: brief.to_owned(),
    acceptance_criteria: criteria.iter().map(|criterion| (*criterion).to_owned()).collect(),
    pipeline: None,
    limits: TaskLimits::NONE,
  }
}

fn merged(
  title: &str,
  value: Option<u32>,
  (tokens, active_ms, attempts): (u64, u64, u32),
  at: i64,
) -> CalibrationRecord {
  CalibrationRecord {
    task: TaskId::generate(),
    key: format!("ORC-{at}"),
    title: title.to_owned(),
    brief: String::new(),
    points: value.map(|value| (points(value), EstimateSource::Automation)),
    effort: TaskEffort {
      tokens,
      active_ms,
      attempts,
      runs: 1,
    },
    merged_at: Timestamp::from_unix_millis(at),
  }
}

#[test]
fn story_points_take_the_fibonacci_scale_only() {
  let scale: Vec<u32> = StoryPoints::all().map(StoryPoints::get).collect();
  assert_eq!(scale, [1, 2, 3, 5, 8, 13, 21]);
  for off in [0, 4, 6, 20, 34] {
    assert!(StoryPoints::new(off).is_err(), "{off}");
  }
}

#[test]
fn story_points_a_person_set_are_never_wanted_from_automation() {
  let basis = EstimateBasis::of(&details("Login", "Add login.", &["Works"]));
  let person = StoryEstimate::person(points(5), Timestamp::now());
  assert!(!StoryEstimate::wanted(Some(&person), &basis));
  assert_eq!(check_automation(Some(&person)), Err(PointsRefused::PersonsValue));
  assert!(StoryEstimate::wanted(None, &basis), "a new task wants an estimate");
  assert_eq!(check_automation(None), Ok(()));
}

#[test]
fn story_points_are_wanted_again_when_the_brief_or_criteria_change() {
  let before = details("Login", "Add login.", &["Works", " "]);
  let estimate = StoryEstimate::automation(points(3), Some("Small."), EstimateBasis::of(&before), Timestamp::now());
  let same = EstimateBasis::of(&details("Renamed", " Add login. ", &["Works"]));
  assert!(
    !StoryEstimate::wanted(Some(&estimate), &same),
    "a title, spaces or a blank criterion change nothing"
  );
  let brief = EstimateBasis::of(&details("Login", "Add login and logout.", &["Works"]));
  let criteria = EstimateBasis::of(&details("Login", "Add login.", &["Works", "Is fast"]));
  assert!(StoryEstimate::wanted(Some(&estimate), &brief));
  assert!(StoryEstimate::wanted(Some(&estimate), &criteria));
  assert_eq!(check_automation(Some(&estimate)), Ok(()), "automation replaces its own");
}

#[test]
fn story_points_reason_is_kept_to_one_plain_line() {
  let estimate = StoryEstimate::automation(
    points(8),
    Some("Touches \"three\" crates.\nNeeds a migration."),
    EstimateBasis::of(&details("A", "", &[])),
    Timestamp::now(),
  );
  assert_eq!(estimate.reason(), Some("Touches 'three' crates. Needs a migration."));
}

#[test]
fn story_points_calibration_reads_the_median_effort_per_points_value() {
  let records = [
    merged("a", Some(3), (100, 60_000, 1), 1),
    merged("b", Some(3), (300, 180_000, 2), 2),
    merged("c", Some(3), (200, 120_000, 1), 3),
    merged("d", Some(8), (900, 600_000, 3), 4),
    merged("e", None, (50, 1_000, 1), 5),
  ];
  let rows = calibration(&records);
  assert_eq!(rows.len(), 2, "tasks without points are left out: {rows:?}");
  assert_eq!(
    (
      rows[0].points,
      rows[0].tasks,
      rows[0].median_tokens,
      rows[0].median_active_ms,
      rows[0].median_attempts
    ),
    (points(3), 3, 200, 120_000, 1)
  );
  assert_eq!(
    (rows[1].points, rows[1].tasks, rows[1].median_tokens),
    (points(8), 1, 900)
  );
}

#[test]
fn story_points_examples_are_the_merged_tasks_most_like_the_task() {
  let records = [
    merged("Storage migration for tasks", Some(8), (900, 1, 1), 1),
    merged("Unrelated button colour", Some(1), (10, 1, 1), 2),
    merged("Task storage cleanup", Some(3), (100, 1, 1), 3),
    merged("Without points storage", None, (1, 1, 1), 4),
  ];
  // Shares storage, migration and tasks with the first, task and storage with the second, nothing with the third.
  let task = details("Migrate task storage", "A storage migration of all tasks.", &[]);
  let picked: Vec<&str> = comparables(&records, &task)
    .iter()
    .map(|record| record.title.as_str())
    .collect();
  assert_eq!(
    picked,
    [
      "Storage migration for tasks",
      "Task storage cleanup",
      "Unrelated button colour"
    ]
  );
  let prompt = estimate_prompt(&EstimateInput::new(&task, &records));
  assert!(prompt.contains("<<<AGENT DATA>>>") && prompt.contains("median_effort_per_points"));
  assert!(prompt.contains("Storage migration for tasks"));
}

#[test]
fn story_points_answer_must_be_on_the_scale_with_a_reason() {
  let answer = parse_estimate_text(r#"{"points": 5, "reason": "Two crates."}"#).expect("an answer");
  assert_eq!((answer.points, answer.reason.as_str()), (points(5), "Two crates."));
  for wrong in [
    r#"{"points": 4, "reason": "Off the scale."}"#,
    r#"{"points": 5, "reason": " "}"#,
    r#"{"points": 5}"#,
    "Five points.",
  ] {
    assert!(
      matches!(parse_estimate_text(wrong), Err(ReviewError::Unparseable(_))),
      "{wrong}"
    );
  }
}

/// A CLI that answers every command with `output`.
struct Cli(Result<CommandOutput, CommandError>);

impl CommandRunner for Cli {
  fn run(&self, _: &Command) -> Result<CommandOutput, CommandError> {
    self.0.clone()
  }
}

fn estimator(cli: EstimatorCli) -> CliEstimator {
  CliEstimator {
    cli,
    executable: Path::new("/bin/cli").to_path_buf(),
    model: (ModelId::new("claude-haiku-4-5-20251001").expect("model"), None),
    folder: ReviewFolder {
      work: "/tmp/estimate/work".into(),
      instructions: "/tmp/estimate/instructions.md".into(),
      schema: "/tmp/estimate/schema.json".into(),
    },
    environment: Vec::new(),
  }
}

#[test]
fn story_points_from_claude_code_are_its_structured_output_with_its_usage() {
  let stdout = concat!(
    r#"{"type":"system","subtype":"init"}"#,
    "\n",
    r#"{"type":"result","is_error":false,"structured_output":{"points":13,"reason":"Large."},"usage":{"input_tokens":100,"output_tokens":9,"cache_creation_input_tokens":0,"cache_read_input_tokens":40}}"#,
  );
  let output = CommandOutput {
    code: Some(0),
    stdout: stdout.to_owned(),
    stderr: String::new(),
  };
  let input = EstimateInput::new(&details("A", "B", &[]), &[]);
  let estimated = estimator(EstimatorCli::ClaudeCode).estimate(&input, &Cli(Ok(output)));
  assert_eq!(estimated.answer.expect("points").points, points(13));
  let ReviewSpend::Reported(usage) = estimated.spend else {
    panic!("usage was reported: {:?}", estimated.spend);
  };
  assert_eq!(usage.counted(), 109);
  let command = estimator(EstimatorCli::ClaudeCode).command(&input);
  assert!(
    command
      .args()
      .windows(2)
      .any(|pair| pair[0] == "--json-schema" && pair[1].contains("\"points\"")),
    "the estimate's schema, not the verdict's"
  );

  let never = estimator(EstimatorCli::ClaudeCode).estimate(&input, &Cli(Err(CommandError::NotFound)));
  assert!(never.answer.is_err());
  assert_eq!(
    never.spend,
    ReviewSpend::Nothing,
    "a CLI that never started spent nothing"
  );
}

#[test]
fn story_points_from_codex_are_its_last_message() {
  let stdout = concat!(
    r#"{"type":"thread.started"}"#,
    "\n",
    r#"{"type":"item.completed","item":{"type":"agent_message","text":"{\"points\":2,\"reason\":\"Tiny.\"}"}}"#,
    "\n",
    r#"{"type":"turn.completed","usage":{"input_tokens":50,"cached_input_tokens":10,"output_tokens":5}}"#,
  );
  let estimated = read_codex_estimate(stdout);
  assert_eq!(estimated.answer.expect("points").points, points(2));
  assert!(matches!(estimated.spend, ReviewSpend::Reported(usage) if usage.counted() == 45));
}
