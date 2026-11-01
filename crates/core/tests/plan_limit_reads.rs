//! Plan limits read outside runs (ORC-55): Codex App Server's `account/rateLimits/read` and Claude Code's *Check now*
//! from recorded and schema-shaped outputs (`fixtures/plan-limits.md`), each account's reading kept with its source,
//! and when the reader reads.

use std::{cell::RefCell, path::Path};

use orchester_core::{
  account::AccountCli,
  command::{Command, CommandError, CommandOutput, LineExchange},
  connection::{
    AgentProvider,
    plan_limits::{
      Credits, LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings, ReadingSource,
      read::{
        CLAUDE_CHECK_MODEL, ReadError, claude_check_command, codex_read_command, read_claude_check, read_codex_limits,
      },
      schedule::{Outcome, RATE_LIMITED_MILLIS, RETRY_MILLIS, ReadSchedule, Trigger, VISIBLE_INTERVAL_MILLIS},
    },
  },
  id::AgentAccountId,
  time::Timestamp,
};

const READ: &str = include_str!("fixtures/codex/rate-limits-read.jsonl");
const SIGNED_OUT: &str = include_str!("fixtures/codex/rate-limits-read-signed-out.jsonl");
const CHECK: &str = include_str!("fixtures/claude/check-now.jsonl");
const CHECK_SIGNED_OUT: &str = include_str!("fixtures/claude/check-now-signed-out.jsonl");

/// App Server as a fixture: what it printed for each `CODEX_HOME`, given line by line until the caller has its answer.
struct Recorded<'a> {
  homes: Vec<(&'a str, &'a str)>,
  asked: RefCell<Vec<Command>>,
}

impl<'a> Recorded<'a> {
  fn of(homes: Vec<(&'a str, &'a str)>) -> Self {
    Self {
      homes,
      asked: RefCell::new(Vec::new()),
    }
  }
}

impl LineExchange for Recorded<'_> {
  fn exchange(&self, command: &Command, done: &mut dyn FnMut(&str) -> bool) -> Result<Vec<String>, CommandError> {
    self.asked.borrow_mut().push(command.clone());
    let home = command
      .env()
      .iter()
      .find(|(name, _)| name == "CODEX_HOME")
      .map(|(_, value)| value.as_str());
    let output = self
      .homes
      .iter()
      .find(|(folder, _)| Some(*folder) == home)
      .map(|(_, output)| *output)
      .ok_or(CommandError::NotFound)?;
    let mut lines = Vec::new();
    for line in output.lines() {
      lines.push(line.to_owned());
      if done(line) {
        break;
      }
    }
    Ok(lines)
  }
}

fn environment(home: &str) -> Vec<(String, String)> {
  vec![("CODEX_HOME".to_owned(), home.to_owned())]
}

fn window(limits: &PlanLimits, minutes: u64) -> &LimitWindow {
  limits
    .windows
    .iter()
    .find(|window| window.minutes() == minutes)
    .unwrap_or_else(|| panic!("a {minutes}-minute window"))
}

#[test]
fn plan_limit_reads_codex_reads_the_plan_bucket_without_a_turn() {
  let codex = Recorded::of(vec![("C:/accounts/codex-work", READ)]);
  let command = codex_read_command(Path::new("codex"), environment("C:/accounts/codex-work"));
  let limits = read_codex_limits(&codex, &command).expect("App Server answered the read");
  assert_eq!(window(&limits, 300).percent(), Some(42.0));
  assert_eq!(window(&limits, 10_080).percent(), Some(65.0));
  assert_eq!(
    window(&limits, 300).resets(),
    Some(Timestamp::from_unix_seconds(1_790_910_000))
  );
  assert_eq!(
    limits.windows.len(),
    2,
    "the bucket of one model (90%) is not the plan's"
  );
  assert_eq!(limits.plan.as_deref(), Some("pro"));
  assert_eq!(
    limits.credits,
    Some(Credits {
      has_credits: false,
      unlimited: false,
      balance: Some("0".to_owned()),
      spend_control_reached: false,
    })
  );
  assert_eq!(limits.blocked, None);
  let asked = codex.asked.borrow();
  let args = asked[0].args();
  assert_eq!(args[..2], ["app-server", "--stdio"]);
  for setting in ["features.plugins=false", "features.hooks=false"] {
    assert!(
      args.windows(2).any(|pair| pair == ["-c", setting]),
      "the user's own setup is off for a read too: {setting}: {args:?}"
    );
  }
  let input = asked[0].stdin().expect("the requests are on its input");
  assert!(
    asked[0].interactive_stdin(),
    "the input stays open until the answer is in"
  );
  for method in ["\"initialize\"", "\"initialized\"", "\"account/rateLimits/read\""] {
    assert!(input.contains(method), "{input}");
  }
  for turn in ["thread/start", "turn/start"] {
    assert!(!input.contains(turn), "a read starts no thread and no turn: {input}");
  }
}

#[test]
fn plan_limit_reads_codex_says_why_there_is_no_reading() {
  let command = |home: &str| codex_read_command(Path::new("codex"), environment(home));
  let signed_out = Recorded::of(vec![("C:/scratch/home", SIGNED_OUT)]);
  assert_eq!(
    read_codex_limits(&signed_out, &command("C:/scratch/home")),
    Err(ReadError::NotSignedIn)
  );
  let held = r#"{"id":2,"error":{"code":-32000,"message":"Too many requests (429), try again later"}}"#;
  let held_back = Recorded::of(vec![("C:/held", held)]);
  assert!(matches!(
    read_codex_limits(&held_back, &command("C:/held")),
    Err(ReadError::RateLimited(_))
  ));
  let gone = Recorded::of(vec![("C:/gone", SIGNED_OUT.lines().next().expect("a line"))]);
  assert_eq!(read_codex_limits(&gone, &command("C:/gone")), Err(ReadError::NoAnswer));
  let empty = Recorded::of(vec![("C:/empty", r#"{"id":2,"result":{"rateLimits":{}}}"#)]);
  assert_eq!(
    read_codex_limits(&empty, &command("C:/empty")),
    Err(ReadError::NoLimits)
  );
  let missing = Recorded::of(Vec::new());
  assert_eq!(
    read_codex_limits(&missing, &command("C:/elsewhere")),
    Err(ReadError::Command(CommandError::NotFound))
  );
}

#[test]
fn plan_limit_reads_each_account_keeps_its_own_reading_and_its_source() {
  let work = AgentAccountId::generate();
  let default = AccountCli::Codex.default_account();
  let quiet = r#"{"id":2,"result":{"rateLimits":{"limitId":"codex","primary":{"usedPercent":7,"windowDurationMins":300,"resetsAt":1790910000}}}}"#;
  let codex = Recorded::of(vec![("C:/accounts/codex-work", READ), ("C:/Users/ana/.codex", quiet)]);
  let mut readings = PlanReadings::default();
  for (account, home, at) in [
    (work, "C:/accounts/codex-work", 10),
    (default, "C:/Users/ana/.codex", 20),
  ] {
    let limits = read_codex_limits(&codex, &codex_read_command(Path::new("codex"), environment(home))).expect("read");
    let reading = LimitReading {
      provider: AgentProvider::CodexCli,
      account: Some(account),
      at: Timestamp::from_unix_millis(at),
      limits,
    };
    assert!(readings.observe_from(reading, ReadingSource::Read));
  }
  let percent = |readings: &PlanReadings, account| {
    readings
      .of_account(account)
      .and_then(|reading| reading.fullest(Timestamp::from_unix_millis(30)))
      .and_then(LimitWindow::percent)
  };
  assert_eq!(percent(&readings, work), Some(65.0));
  assert_eq!(percent(&readings, default), Some(7.0));
  assert_eq!(readings.source_of(work), Some(ReadingSource::Read));
  // A turn's newer report takes the account's place, with its source; the other account keeps its read.
  let turn = LimitReading {
    provider: AgentProvider::CodexCli,
    account: Some(work),
    at: Timestamp::from_unix_millis(40),
    limits: PlanLimits {
      windows: vec![LimitWindow::new(300, Some(50.0), None, LimitState::Normal, false).expect("a window")],
      plan: None,
      credits: None,
      blocked: None,
    },
  };
  assert!(readings.observe(turn.clone()));
  assert_eq!(readings.source_of(work), Some(ReadingSource::Turn));
  assert_eq!(readings.source_of(default), Some(ReadingSource::Read));
  // An older read changes nothing: the newest reading wins whatever its source.
  let older = LimitReading {
    at: Timestamp::from_unix_millis(35),
    ..turn
  };
  assert!(!readings.observe_from(older, ReadingSource::Read));
  assert_eq!(readings.source_of(work), Some(ReadingSource::Turn));
}

#[test]
fn plan_limit_reads_claude_check_now_is_the_smallest_turn_and_reads_its_events() {
  let command = claude_check_command(
    Path::new("claude"),
    vec![("CLAUDE_CONFIG_DIR".to_owned(), "C:/accounts/claude-work".to_owned())],
    Path::new("C:/Temp/check"),
  );
  let args = command.args();
  let after = |flag: &str| {
    args
      .iter()
      .position(|arg| arg == flag)
      .and_then(|index| args.get(index + 1))
      .map(String::as_str)
  };
  assert_eq!(after("--model"), Some(CLAUDE_CHECK_MODEL));
  assert_eq!(after("--tools"), Some(""), "no tools");
  assert_eq!(after("--output-format"), Some("stream-json"));
  for flag in [
    "-p",
    "--strict-mcp-config",
    "--no-session-persistence",
    "--disable-slash-commands",
  ] {
    assert!(args.iter().any(|arg| arg == flag), "{flag}: {args:?}");
  }
  assert!(
    command.stdin().is_some_and(|prompt| prompt.len() < 20),
    "a prompt of a few words"
  );
  let output = |stdout: &str| CommandOutput {
    code: Some(0),
    stdout: stdout.to_owned(),
    stderr: String::new(),
  };
  let limits = read_claude_check(&output(CHECK)).expect("the check reported its limits");
  assert_eq!(limits.len(), 2, "one event per window");
  assert_eq!(window(&limits[0], 300).percent(), Some(37.0));
  assert_eq!(
    window(&limits[1], 10_080).state(Timestamp::from_unix_millis(0)),
    LimitState::Near
  );
  assert_eq!(
    read_claude_check(&output(CHECK_SIGNED_OUT)),
    Err(ReadError::NotSignedIn)
  );
  let without = CHECK
    .lines()
    .filter(|line| !line.contains("rate_limit_event"))
    .collect::<Vec<_>>()
    .join("\n");
  assert_eq!(
    read_claude_check(&output(&without)),
    Err(ReadError::NoLimits),
    "an API key has no plan limits"
  );
  assert_eq!(read_claude_check(&output("")), Err(ReadError::NoAnswer));
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

#[test]
fn plan_limit_reads_schedule_reads_at_start_and_on_triggers_only() {
  let work = AgentAccountId::generate();
  let none = |_| None;
  let mut schedule = ReadSchedule::default();
  schedule.track(&[work]);
  assert_eq!(
    schedule.due(at(0), false, none),
    [work],
    "a new account is read at once"
  );
  schedule.record(work, at(0), Outcome::Read);
  assert!(schedule.due(at(0), false, none).is_empty());
  let later = at(10 * VISIBLE_INTERVAL_MILLIS);
  assert!(
    schedule.due(later, false, none).is_empty(),
    "nothing polls while nothing is shown"
  );
  assert_eq!(schedule.next_due(later, false, none), None);
  schedule.request(Trigger::Opened);
  assert_eq!(schedule.due(later, false, none), [work], "the hint or Usage opened");
  schedule.record(work, later, Outcome::Read);
  schedule.request(Trigger::AccountsChanged);
  assert_eq!(schedule.due(later, false, none), [work], "an account changed");
}

#[test]
fn plan_limit_reads_schedule_reads_every_few_minutes_only_while_shown() {
  let work = AgentAccountId::generate();
  let none = |_| None;
  let mut schedule = ReadSchedule::default();
  schedule.track(&[work]);
  schedule.record(work, at(0), Outcome::Read);
  assert_eq!(schedule.next_due(at(1), true, none), Some(at(VISIBLE_INTERVAL_MILLIS)));
  assert!(schedule.due(at(VISIBLE_INTERVAL_MILLIS - 1), true, none).is_empty());
  assert_eq!(schedule.due(at(VISIBLE_INTERVAL_MILLIS), true, none), [work]);
  assert!(schedule.due(at(VISIBLE_INTERVAL_MILLIS), false, none).is_empty());
}

#[test]
fn plan_limit_reads_schedule_reads_once_after_a_known_reset() {
  let work = AgentAccountId::generate();
  let reading = LimitReading {
    provider: AgentProvider::CodexCli,
    account: Some(work),
    at: at(0),
    limits: PlanLimits {
      windows: vec![LimitWindow::new(300, Some(100.0), Some(at(1_000)), LimitState::Reached, false).expect("a window")],
      plan: None,
      credits: None,
      blocked: None,
    },
  };
  let of = |_| Some(&reading);
  let mut schedule = ReadSchedule::default();
  schedule.track(&[work]);
  schedule.record(work, at(0), Outcome::Read);
  assert_eq!(
    schedule.next_due(at(10), false, of),
    Some(at(1_000)),
    "it waits for the reset"
  );
  assert!(schedule.due(at(999), false, of).is_empty());
  assert_eq!(schedule.due(at(1_000), false, of), [work], "the reset passed");
  schedule.record(work, at(1_000), Outcome::Read);
  assert!(schedule.due(at(5_000), false, of).is_empty(), "once");
}

#[test]
fn plan_limit_reads_schedule_backs_off_after_failures_and_waits_for_a_sign_in() {
  let work = AgentAccountId::generate();
  let none = |_| None;
  let mut schedule = ReadSchedule::default();
  schedule.track(&[work]);
  schedule.record(work, at(0), Outcome::Failed);
  schedule.request(Trigger::Opened);
  assert!(
    schedule.due(at(RETRY_MILLIS - 1), true, none).is_empty(),
    "it waits after a failure"
  );
  assert_eq!(schedule.next_due(at(1), true, none), Some(at(RETRY_MILLIS)));
  assert_eq!(schedule.due(at(RETRY_MILLIS), true, none), [work]);
  schedule.record(work, at(RETRY_MILLIS), Outcome::Failed);
  schedule.request(Trigger::Opened);
  assert_eq!(
    schedule.next_due(at(RETRY_MILLIS + 1), true, none),
    Some(at(3 * RETRY_MILLIS)),
    "twice as long after the second failure in a row"
  );
  schedule.record(work, at(0), Outcome::RateLimited);
  schedule.request(Trigger::Opened);
  assert!(
    schedule.next_due(at(1), true, none) >= Some(at(RATE_LIMITED_MILLIS)),
    "longer when held back"
  );
  schedule.record(work, at(0), Outcome::SignedOut);
  schedule.request(Trigger::Opened);
  assert!(
    schedule.due(at(100 * RETRY_MILLIS), true, none).is_empty(),
    "nobody signed in: no read"
  );
  assert_eq!(schedule.next_due(at(1), true, none), None);
  schedule.request(Trigger::AccountsChanged);
  assert_eq!(schedule.due(at(1), true, none), [work], "a sign-in changes the account");
}

#[test]
fn plan_limit_reads_schedule_forgets_accounts_no_longer_read() {
  let (work, client) = (AgentAccountId::generate(), AgentAccountId::generate());
  let none = |_| None;
  let mut schedule = ReadSchedule::default();
  schedule.track(&[work, client]);
  schedule.record(work, at(0), Outcome::Read);
  schedule.track(&[work]);
  assert!(
    schedule.due(at(1), false, none).is_empty(),
    "the removed account is not read"
  );
  schedule.track(&[work, client]);
  assert_eq!(
    schedule.due(at(1), false, none),
    [client],
    "an account that comes back is new"
  );
}
