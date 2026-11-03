//! Fix CI on a run's pull request: the failing checks go to the run's agent as one message with their names, pages and
//! log tails, colour codes stripped and secret-shaped text redacted; passed checks are left out. Fix CI waits while
//! nothing failed, CI was not read or an earlier Fix CI waits for its agent. *Fix CI automatically* sends a failure on
//! its own up to its limit, records each send and then that it stopped; a send by hand does not count, and changing the
//! limit restarts the count. Only a person changes the switch: automation driving the window is refused.

use orchester_core::{
  conversation::{Arrival, Message},
  id::StepExecutionId,
  pull_request::{
    PullRequestState,
    ci::{
      CiCheck, CiChecks, CiState,
      fix::{AutoFix, FixEventKind, FixLimit, FixedBy},
      redact::{REDACTED, agent_log_tail},
    },
  },
  resource::ModelId,
  run::{Answerer, Run},
  store::{ActivityStore, ConversationStore, PullRequestStore},
  time::Timestamp,
};
use orchester_workspace::{
  messages::Message as UiMessage,
  pull_requests::{
    ci::{PullRequestCi, ReadCi},
    ci_fix::{FixDone, FixError, FixOffer, Fixer, offer, set_auto_fix_in, store_fix_in},
  },
  tasks::detail::TaskDetail,
};

use super::{
  link, pull_request, repo,
  support::{Fixture, at},
};

/// A person's press of *Fix CI*.
const HAND: Fixer = Fixer::Hand { by: Answerer::Person };

/// A log tail as `gh` gives it, with colour codes and secrets a workflow echoed.
const LEAKY_TAIL: &str = "\u{1b}[31merror\u{1b}[0m: this `if` has identical blocks\n\
  curl -H \"Authorization: Bearer abcdEFGH1234ijklMNOP\" https://api.example.com\n\
  export GITHUB_TOKEN=ghp_0123456789abcdefABCDEF0123456789abcd\n\
  DATABASE_PASSWORD=hunter2hunter2\n\
  pushing to https://deploy:s3cr3tpass@git.example.com/repo.git\n\
  AWS key AKIAIOSFODNN7EXAMPLE in use\n\
  error: could not compile `webhooks` due to 1 previous error";

fn check(name: &str, state: CiState, job: u64, tail: Option<&str>) -> CiCheck {
  CiCheck {
    name: name.to_owned(),
    workflow: Some("CI".to_owned()),
    state,
    url: Some(format!(
      "https://github.com/ivanzaida/kontur/actions/runs/900/job/{job}"
    )),
    job: Some(job),
    log_tail: tail.map(str::to_owned),
  }
}

fn read(checks: Vec<CiCheck>) -> ReadCi {
  ReadCi {
    number: 142,
    checks: CiChecks { checks },
    read_at: Timestamp::now(),
  }
}

/// Two failing checks of five's, one with a leaky log tail, one without a log.
fn failed() -> ReadCi {
  read(vec![
    check("build", CiState::Passed, 9001, None),
    check("lint", CiState::Failed, 9002, Some(LEAKY_TAIL)),
    check("test (ubuntu-latest)", CiState::Failed, 9003, None),
    check("test (windows-latest)", CiState::Passed, 9004, None),
    check("e2e", CiState::Skipped, 9005, None),
  ])
}

/// A linked project whose task's run has pull request 142 open.
fn project() -> (Fixture, Run) {
  let mut fixture = Fixture::keyed("KON");
  let task = fixture.task("Add retry to webhook sender", at(10));
  let run = fixture.started_run(task, at(20));
  link(&mut fixture);
  let stored = [pull_request(
    142,
    run.branch().as_str(),
    PullRequestState::Open,
    None,
    900,
  )];
  fixture
    .db
    .replace_pull_requests(&repo(), &stored, at(1_000))
    .expect("the pull requests are stored");
  (fixture, run)
}

fn detail(fixture: &Fixture, run: &Run) -> TaskDetail {
  TaskDetail::load_in(fixture.repo.path(), run.task()).expect("the task")
}

fn messages(fixture: &Fixture, run: &Run) -> Vec<Message> {
  fixture.db.messages(run.task()).expect("the messages")
}

fn fix(fixture: &Fixture, run: &Run, ci: &ReadCi, fixer: Fixer) -> Result<FixDone, FixError> {
  store_fix_in(fixture.repo.path(), (run.task(), run.id()), (ci, false), fixer)
}

/// The run's agent took every message of the task.
fn deliver_all(fixture: &mut Fixture, run: &Run, at_millis: i64) {
  let execution: StepExecutionId = run.current_attempt().expect("attempt").executions()[0].id();
  let model = ModelId::new("gpt-5.5").expect("model");
  let invocation = fixture
    .db
    .start_invocation(run.id(), execution, &model, at(at_millis))
    .expect("invocation")
    .id;
  let mut delivered = Vec::new();
  for mut message in messages(fixture, run) {
    if message.delivery() == Some(orchester_core::conversation::Delivery::Queued) {
      message
        .deliver(run.id(), (execution, invocation), at(at_millis + 1), Arrival::Prompt)
        .expect("delivered");
      delivered.push(message);
    }
  }
  fixture.db.save_deliveries(&delivered).expect("saved");
}

#[test]
fn fix_ci_sends_the_failing_checks_as_one_message() {
  let (fixture, run) = project();
  let done = fix(&fixture, &run, &failed(), HAND).expect("sent");
  assert!(
    matches!(done, FixDone::Stored(ref fix) if fix.by == FixedBy::Hand),
    "{done:?}"
  );
  let stored = messages(&fixture, &run);
  let [message] = stored.as_slice() else {
    panic!("one message: {stored:?}");
  };
  let text = message.body().as_str();
  assert!(
    text.starts_with("CI failed on pull request #142. 2 of 5 checks failed:"),
    "{text}"
  );
  // Each failing check's name links to its page, its log tail in a fenced block (`11b` `X8xYZ`).
  assert!(
    text.contains(
      "[lint](https://github.com/ivanzaida/kontur/actions/runs/900/job/9002) · failed · last lines of the log\n```\n"
    ),
    "{text}"
  );
  assert!(
    text.contains("[test (ubuntu-latest)](https://github.com/ivanzaida/kontur/actions/runs/900/job/9003) · failed · no log could be read"),
    "{text}"
  );
  assert!(!text.contains("Details:"), "the link is the check's name: {text}");
  assert!(text.contains("error: could not compile `webhooks`"), "{text}");
  assert!(
    text.ends_with(&format!(
      "Fix them on {} and push. Don't change the tests' expectations unless they are wrong.",
      run.branch().as_str()
    )),
    "{text}"
  );
  for passed in ["build", "windows-latest", "e2e"] {
    assert!(!text.contains(passed), "passed and skipped checks are left out: {text}");
  }
  let fixes = detail(&fixture, &run).run.expect("the run").ci_fixes;
  let [event] = fixes.events.as_slice() else {
    panic!("one Fix CI is recorded: {fixes:?}");
  };
  assert_eq!(event.number, 142);
  assert_eq!(event.failing, ["lint", "test (ubuntu-latest)"]);
  assert_eq!(
    event.kind,
    FixEventKind::Sent {
      message: message.id(),
      by: FixedBy::Hand
    }
  );
  assert_eq!(fixes.used(), 0, "a send by hand does not count");
}

#[test]
fn fix_ci_redacts_secret_shaped_text_in_log_tails() {
  let (fixture, run) = project();
  fix(&fixture, &run, &failed(), HAND).expect("sent");
  let text = messages(&fixture, &run)[0].body().as_str().to_owned();
  for secret in [
    "abcdEFGH1234ijklMNOP",
    "ghp_0123456789abcdefABCDEF0123456789abcd",
    "hunter2hunter2",
    "s3cr3tpass",
    "AKIAIOSFODNN7EXAMPLE",
  ] {
    assert!(!text.contains(secret), "{secret} reached the agent: {text}");
  }
  assert!(!text.contains('\u{1b}'), "colour codes are stripped: {text}");
  assert!(text.contains(&format!("Authorization: Bearer {REDACTED}")), "{text}");
  assert!(text.contains(&format!("GITHUB_TOKEN={REDACTED}")), "{text}");
  assert!(text.contains("error: this `if` has identical blocks"), "{text}");
  assert!(text.contains("git.example.com/repo.git"), "{text}");
  let private = "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA\n-----END RSA PRIVATE KEY-----\nnext";
  assert_eq!(agent_log_tail(private), format!("{REDACTED}\nnext"));
  // A key's body whose BEGIN line fell outside the tail: 64-character lines holding '/', short segments or long.
  let body = [
    "MIIEowIBAAKCAQ/EAu1SU1LfVLPHC/ozMxH2Mo4lgOEeP/zNm0tRgeLezV6f/fAt",
    "u1SU1LfVLPHCozMxH2Mo4lgOEePzNm0tRgeLezV6ffAt0gunVTLw7onLRnrq0/Ia",
  ];
  for line in body {
    assert_eq!(line.len(), 64, "{line}");
    assert_eq!(agent_log_tail(line), REDACTED, "a PEM body line is redacted");
  }
  let aws = "aws configure set aws_secret wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
  assert_eq!(
    agent_log_tail(aws),
    format!("aws configure set aws_secret {REDACTED}"),
    "a standalone AWS secret access key is redacted"
  );
  for path in [
    "at /home/runner/work/Kontur2/Kontur2/src/main/Webhooks.kt",
    "in crates/desktop-app/src/screens/Task2View/Summary",
  ] {
    assert_eq!(agent_log_tail(path), path, "a path is kept");
  }
  let long: String = (0..60).map(|line| format!("line {line}\n")).collect();
  let tail = agent_log_tail(&long);
  assert_eq!(tail.lines().count(), 40, "the tail is trimmed to its last 40 lines");
  assert!(tail.ends_with("line 59"), "{tail}");
}

#[test]
fn fix_ci_waits_while_nothing_failed_ci_was_not_read_or_a_fix_waits() {
  let (mut fixture, run) = project();
  let why = |ci: &PullRequestCi, detail: &TaskDetail| match offer(detail, false, ci) {
    FixOffer::Unavailable(reason) => reason,
    FixOffer::Available => panic!("Fix CI is unavailable"),
  };
  let shown = detail(&fixture, &run);
  let passed = read(vec![check("build", CiState::Passed, 1, None)]);
  let running = read(vec![check("build", CiState::Running, 1, None)]);
  assert_eq!(
    why(&PullRequestCi::Read(passed), &shown),
    UiMessage::new("fixCi.unavailable.nothingFailed")
  );
  assert_eq!(
    why(&PullRequestCi::Read(running), &shown),
    UiMessage::new("fixCi.unavailable.running")
  );
  assert_eq!(
    why(&PullRequestCi::NotSignedIn, &shown),
    UiMessage::new("fixCi.unavailable.notRead")
  );
  let failing = PullRequestCi::Read(failed());
  assert_eq!(offer(&shown, false, &failing), FixOffer::Available);
  fix(&fixture, &run, &failed(), HAND).expect("sent");
  let waiting = UiMessage::new("fixCi.unavailable.waiting");
  assert_eq!(why(&failing, &detail(&fixture, &run)), waiting);
  assert!(
    matches!(fix(&fixture, &run, &failed(), HAND), Err(FixError::Unavailable(reason)) if reason == waiting),
    "a second press sends nothing"
  );
  deliver_all(&mut fixture, &run, 40);
  assert_eq!(offer(&detail(&fixture, &run), false, &failing), FixOffer::Available);
}

/// A failure of `name`, its job `job`: a new push runs new jobs.
fn failure(name: &str, job: u64) -> ReadCi {
  read(vec![
    check("build", CiState::Passed, 1, None),
    check(name, CiState::Failed, job, Some("assertion `left == right` failed")),
  ])
}

#[test]
fn fix_ci_automatically_sends_up_to_its_limit_and_records_each_send() {
  let (mut fixture, run) = project();
  let root = fixture.repo.path().to_owned();
  assert_eq!(
    fix(&fixture, &run, &failure("test", 10), Fixer::Automatic).expect("read"),
    FixDone::Nothing,
    "off by default"
  );
  let two = AutoFix {
    on: true,
    limit: FixLimit::Two,
  };
  set_auto_fix_in(&root, run.id(), two, Answerer::Person).expect("set");
  let first = fix(&fixture, &run, &failure("test", 10), Fixer::Automatic).expect("sent");
  assert!(
    matches!(first, FixDone::Stored(ref fix) if fix.by == FixedBy::Automatically { count: 1, limit: FixLimit::Two }),
    "{first:?}"
  );
  let text = messages(&fixture, &run)[0].body().as_str().to_owned();
  assert!(
    text.starts_with("CI failed on pull request #142. This is Fix CI 1 of 2"),
    "{text}"
  );
  assert!(text.contains("Fix it on"), "one failing check: {text}");
  deliver_all(&mut fixture, &run, 40);
  assert_eq!(
    fix(&fixture, &run, &failure("test", 10), Fixer::Automatic).expect("read"),
    FixDone::Nothing,
    "a failure is answered once"
  );
  let second = fix(&fixture, &run, &failure("test", 11), Fixer::Automatic).expect("sent");
  assert!(matches!(second, FixDone::Stored(_)), "{second:?}");
  let text = messages(&fixture, &run)[1].body().as_str().to_owned();
  assert!(
    text.starts_with("CI failed on pull request #142 again. This is Fix CI 2 of 2"),
    "{text}"
  );
  deliver_all(&mut fixture, &run, 50);
  assert_eq!(
    fix(&fixture, &run, &failure("test", 12), Fixer::Automatic).expect("read"),
    FixDone::Stopped
  );
  assert_eq!(
    fix(&fixture, &run, &failure("test", 12), Fixer::Automatic).expect("read"),
    FixDone::Nothing,
    "it stops once"
  );
  assert_eq!(messages(&fixture, &run).len(), 2, "nothing more was sent");
  let fixes = detail(&fixture, &run).run.expect("the run").ci_fixes;
  assert_eq!(fixes.used(), 2);
  assert!(fixes.limit_reached());
  let kinds: Vec<_> = fixes.events.iter().map(|event| event.kind).collect();
  assert!(matches!(
    kinds.as_slice(),
    [
      FixEventKind::Sent {
        by: FixedBy::Automatically { count: 1, .. },
        ..
      },
      FixEventKind::Sent {
        by: FixedBy::Automatically { count: 2, .. },
        ..
      },
      FixEventKind::Stopped { limit: FixLimit::Two }
    ]
  ));
  assert!(
    matches!(fix(&fixture, &run, &failure("test", 12), HAND), Ok(FixDone::Stored(_))),
    "Fix CI by hand still works at the limit"
  );
}

#[test]
fn fix_ci_automatically_is_set_by_a_person_only_and_automation_cannot_restart_its_count() {
  let (fixture, run) = project();
  let root = fixture.repo.path().to_owned();
  let one = AutoFix {
    on: true,
    limit: FixLimit::One,
  };
  set_auto_fix_in(&root, run.id(), one, Answerer::Person).expect("set");
  let sent = fix(&fixture, &run, &failure("test", 10), Fixer::Automatic).expect("sent");
  assert!(matches!(sent, FixDone::Stored(_)), "{sent:?}");
  std::thread::sleep(std::time::Duration::from_millis(5));
  let refused = UiMessage::new("appMcp.errors.automationNotAllowed");
  for auto in [
    one,
    AutoFix { on: false, ..one },
    AutoFix {
      limit: FixLimit::Five,
      ..one
    },
  ] {
    assert!(
      matches!(
        set_auto_fix_in(&root, run.id(), auto, Answerer::Automation),
        Err(FixError::Unavailable(ref reason)) if *reason == refused
      ),
      "automation driving the window does not set the switch: {auto:?}"
    );
  }
  let fixes = detail(&fixture, &run).run.expect("the run").ci_fixes;
  assert_eq!(
    fixes.auto, one,
    "the refused changes left the switch as the person set it"
  );
  assert_eq!(fixes.used(), 1, "automation cannot restart the count");
  assert!(
    matches!(
      fixes.auto_step("another failure"),
      orchester_core::pull_request::ci::fix::AutoStep::Stop { .. }
    ),
    "the next failure stops at the limit"
  );
  set_auto_fix_in(&root, run.id(), one, Answerer::Person).expect("set");
  let fixes = detail(&fixture, &run).run.expect("the run").ci_fixes;
  assert_eq!(fixes.used(), 0, "the person changing the switch restarts the count");
}
