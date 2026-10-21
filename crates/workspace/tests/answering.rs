//! The pending-answers rule (`tasks::answering`, ORC-70): what an answer's command answers, exactly that item and no
//! other; a project's marks while the runner has not answered, after it took or refused them, and once a reading of
//! the tasks begun after the settling takes over.

use std::{num::NonZeroU32, path::Path};

use orchester_core::{
  id::RunId,
  question::GivenAnswer,
  run::{Answerer, PlanVersion, RunNumber},
  time::Timestamp,
};
use orchester_ipc::{Command, SessionEnd};
use orchester_workspace::{
  activity::Activity,
  runner::Runners,
  tasks::{
    answering::Answered,
    list::{AwaitedPlan, AwaitedQuestion},
    needs_you::{ChoiceWaiting, WaitingItem, WaitingKind},
  },
};

fn item(run: RunId, kind: WaitingKind) -> WaitingItem {
  WaitingItem {
    task: None,
    run: Some((run, RunNumber::new(NonZeroU32::MIN))),
    step: None,
    since: Timestamp::now(),
    kind,
  }
}

fn choice(run: RunId, request: &str) -> WaitingItem {
  item(
    run,
    WaitingKind::Choice(ChoiceWaiting {
      request: request.into(),
      agent: "Explorer".into(),
      first: "Where?".into(),
      count: 1,
    }),
  )
}

fn plan(run: RunId, version: PlanVersion) -> WaitingItem {
  item(
    run,
    WaitingKind::Plan(AwaitedPlan {
      version,
      agent: "Planner".into(),
    }),
  )
}

#[test]
fn answering_covers_exactly_the_item_its_command_answers() {
  let (run, other) = (RunId::generate(), RunId::generate());
  let answer = Command::AnswerQuestion {
    run,
    request: "q-1".to_owned(),
    answers: vec![GivenAnswer {
      choices: vec!["Top bar".to_owned()],
      text: None,
    }],
    by: Answerer::Person,
  };
  let answered = Answered::of(&answer).expect("a question's answer answers its request");
  assert!(answered.covers(&choice(run, "q-1")));
  assert!(
    !answered.covers(&choice(run, "q-2")),
    "another request of the run waits on"
  );
  assert!(!answered.covers(&choice(other, "q-1")), "another run's waits on");
  assert!(
    !answered.covers(&plan(run, PlanVersion::FIRST)),
    "and so does the run's plan"
  );
  let second = PlanVersion::FIRST.next();
  let decided = Answered::Plan { run, version: second };
  assert!(decided.covers(&plan(run, second)));
  assert!(
    !decided.covers(&plan(run, PlanVersion::FIRST)),
    "another version waits on"
  );
  let message = Answered::Message { run };
  let question = item(
    run,
    WaitingKind::Question(Some(AwaitedQuestion {
      agent: "Builder".into(),
      text: "Which crate?".into(),
    })),
  );
  assert!(
    message.covers(&question),
    "the composer's answer answers the agent's question"
  );
  assert!(
    !message.covers(&choice(run, "q-1")),
    "not a question with suggested answers"
  );
  assert_eq!(
    Answered::of(&Command::PauseRun { run }),
    None,
    "a pause answers nothing"
  );
  let stuck = item(run, WaitingKind::StoppedAtLimit { daily_budget: false });
  assert!(Answered::Limit { run }.covers(&stuck));
}

fn runners(runtime: &tokio::runtime::Runtime) -> Runners {
  Runners::new(
    "no-runner".into(),
    runtime.handle().clone(),
    Activity::new(),
    SessionEnd::Clean,
  )
}

#[test]
fn answering_keeps_a_mark_until_a_reading_after_its_settling_takes_over() {
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let runners = runners(&runtime);
  let (root, run) = (Path::new("kontur"), RunId::generate());
  let mark = runners.answering(root, Answered::Decision { run });
  let marked = runners.answers_now(root);
  assert_eq!(marked.answered(), [Answered::Decision { run }]);
  assert!(
    runners.answers_now(Path::new("atlas")).is_empty(),
    "one set per project"
  );
  let before = marked.settled();
  runners.answers_read(root, before);
  assert!(
    !runners.answers_now(root).is_empty(),
    "no reading ends a mark on its way"
  );
  mark.settle(true);
  let taken = runners.answers_now(root);
  assert_eq!(taken.settled(), before + 1, "the settling reads the tasks again");
  assert_eq!(taken.answered().len(), 1, "a taken answer stays marked until a reading");
  runners.answers_read(root, before);
  assert!(
    !runners.answers_now(root).is_empty(),
    "a reading begun before the settling knows nothing of it"
  );
  runners.answers_read(root, taken.settled());
  assert!(
    runners.answers_now(root).is_empty(),
    "a reading begun after it takes over"
  );
}

#[test]
fn answering_a_refusal_or_a_dropped_send_settles_the_mark() {
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let runners = runners(&runtime);
  let (root, run) = (Path::new("kontur"), RunId::generate());
  let refused = runners.answering(root, Answered::Limit { run });
  let dropped = runners.answering(root, Answered::Decision { run });
  refused.settle(false);
  assert_eq!(
    runners.answers_now(root).answered(),
    [Answered::Decision { run }],
    "a refusal brings its item back at once"
  );
  drop(dropped);
  let left = runners.answers_now(root);
  assert_eq!(
    left.answered().len(),
    1,
    "a send that ended without word waits for a reading"
  );
  runners.answers_read(root, left.settled());
  assert!(runners.answers_now(root).is_empty());
}
