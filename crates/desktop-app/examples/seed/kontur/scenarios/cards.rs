//! `cards`: one task per state of the conversation's decision cards (`Cards and rows inside the conversation`
//! `YhiGC`), each a Feature delivery run at work on Implement unless said otherwise: a question its step waits
//! on; questions with suggested answers waiting (one answer, several answers), answered by the person,
//! answered by automation and ended before an answer; a command waiting for approval; commands approved once
//! by the person, denied by the person, approved by automation and ended without an answer; a plan longer
//! than its card shows waiting (v1), and a plan revised after changes were asked for (v2).

use orchester_core::{
  question::{GivenAnswer, Question, QuestionOption, QuestionRequest, QuestionTurn},
  run::{Answerer, DecidedBy, Invocation, PendingToolApproval, Run, ToolApprovalDecision},
  store::ActivityStore,
  task::Task,
  time::Timestamp,
};

use crate::kontur::{
  Kontur, SeedError,
  catalogue::Published,
  run::Drive,
  shapes::{self, Shape},
};

const CRITERIA: [&str; 1] = ["The change is covered by tests."];
/// A plan longer than its card shows at first.
const LONG_PLAN: &str = "Approach\n1. Wrap send() in a retry loop: exponential backoff from 500 ms, full jitter.\n2. Retry \
                         network errors, 5xx and 429; honor Retry-After up to 60 s.\n3. Add retry_budget to \
                         EndpointSettings, default 5 attempts.\n4. Record every attempt in webhook_attempts with status \
                         and delay.\n5. Stop retrying once the endpoint is disabled.\n6. Show the attempts on the \
                         endpoint's page.\n7. Cover the backoff and the budget with tests.\n8. Document the setting.";
/// The agent that asks, Implement's.
const AGENT: &str = "Builder";

/// What a run's request is left as.
#[derive(Clone, Copy)]
enum Card {
  /// One of the shapes the task page shows (a question, an approval, a plan).
  Shape(Shape),
  /// A question with suggested answers; several answers may be picked.
  Choice { several: bool, ends: Ending },
  /// A command that asks to be approved, and how it ended.
  Command(&'static str, Option<Decision>),
  /// Plan proposes a plan, which waits for a decision.
  Plan(&'static str),
}

/// How a question with suggested answers ended.
#[derive(Clone, Copy)]
enum Ending {
  Waiting,
  Answered(Answerer),
  Interrupted,
}

/// How a command's request ended.
#[derive(Clone, Copy)]
enum Decision {
  Decided(DecidedBy),
  Ended,
}

const CARDS: [(&str, Card, &str); 13] = [
  (
    "Sync contacts with HubSpot",
    Card::Shape(Shape::Question("implement")),
    "question waiting",
  ),
  (
    "Pick where the docs button goes",
    Card::Choice {
      several: false,
      ends: Ending::Waiting,
    },
    "question with options waiting",
  ),
  (
    "Choose the export formats",
    Card::Choice {
      several: true,
      ends: Ending::Waiting,
    },
    "question with several answers waiting",
  ),
  (
    "Name the retry setting",
    Card::Choice {
      several: false,
      ends: Ending::Answered(Answerer::Person),
    },
    "question with options answered by you",
  ),
  (
    "Pick the cache store",
    Card::Choice {
      several: false,
      ends: Ending::Answered(Answerer::Automation),
    },
    "question with options answered by automation",
  ),
  (
    "Pick the log level",
    Card::Choice {
      several: false,
      ends: Ending::Interrupted,
    },
    "question with options ended without an answer",
  ),
  (
    "Add serde to the webhook payloads",
    Card::Shape(Shape::Approval("implement")),
    "command waiting for approval",
  ),
  (
    "Add rand to the jitter",
    Card::Command("cargo add rand", Some(Decision::Decided(DecidedBy::Person))),
    "command approved once by you",
  ),
  (
    "Publish the webhook crate",
    Card::Command(
      "cargo publish --dry-run",
      Some(Decision::Decided(DecidedBy::PersonDeny)),
    ),
    "command denied by you",
  ),
  (
    "Fetch the latest schema",
    Card::Command("git fetch origin", Some(Decision::Decided(DecidedBy::Automation))),
    "command approved by automation",
  ),
  (
    "Push the release tag",
    Card::Command("git push origin v1.2.0", Some(Decision::Ended)),
    "command ended without an answer",
  ),
  ("Add retry to webhook sender", Card::Plan(LONG_PLAN), "plan v1 waiting"),
  (
    "Back off when GitHub rate-limits the sync",
    Card::Shape(Shape::PlanRevised),
    "plan v2 waiting",
  ),
];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  let pipeline = &catalogue.feature_delivery;
  for (index, (title, card, state)) in CARDS.into_iter().enumerate() {
    let minutes = i64::try_from(CARDS.len() - index).expect("a short list") * 30;
    let (created, started) = (kontur.ago(minutes + 20), kontur.ago(minutes));
    let mut task = kontur.task((title, title, &CRITERIA), Some(pipeline), created)?;
    if let Card::Plan(plan) = card {
      kontur.queue(&mut task, pipeline)?;
      let mut drive = Drive::start(kontur, &task, pipeline, started)?;
      drive.pass_before("plan")?.propose("plan", plan)?;
      kontur.seeded(&task, state);
      continue;
    }
    let Card::Shape(shape) = card else {
      let (run, invocation) = at_work(kontur, &mut task, pipeline, started)?;
      kontur.seeded(&task, state);
      let at = kontur.ago(minutes - 10);
      match card {
        Card::Choice { several, ends } => choice(kontur, (&run, &invocation), several, (ends, at))?,
        Card::Command(command, ends) => approval(kontur, (&run, &invocation), command, (ends, at))?,
        Card::Shape(_) | Card::Plan(_) => {}
      }
      continue;
    };
    shapes::apply(kontur, &mut task, Some(pipeline), shape, started)?;
  }
  Ok(())
}

/// `task`'s run, started at `at`, with Implement's agent at work: the run and its invocation.
fn at_work(
  kontur: &mut Kontur,
  task: &mut Task,
  pipeline: &Published,
  at: Timestamp,
) -> Result<(Run, Invocation), SeedError> {
  kontur.queue(task, pipeline)?;
  let mut drive = Drive::start(kontur, task, pipeline, at)?;
  drive.pass_before("implement")?.begin("implement")?;
  let run = drive.finish();
  let invocation = kontur
    .db
    .invocations(run.id())?
    .into_iter()
    .rev()
    .find(|invocation| invocation.ended_at.is_none())
    .ok_or(SeedError::NotAtWork)?;
  Ok((run, invocation))
}

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}

/// A question with three suggested answers, asked in the agent's turn at `at` and left as `ends`.
fn choice(
  kontur: &mut Kontur,
  (run, invocation): (&Run, &Invocation),
  several: bool,
  (ends, at): (Ending, Timestamp),
) -> Result<(), SeedError> {
  let (text, options): (&str, [(&str, Option<&str>); 3]) = if several {
    (
      "Which formats should the export offer?",
      [
        ("CSV", Some("Opens in any spreadsheet.")),
        ("JSON", Some("For scripts and imports.")),
        ("PDF", None),
      ],
    )
  } else {
    (
      "Where should the docs button go?",
      [
        (
          "In the top bar",
          Some("Beside the account menu, where people look for help."),
        ),
        ("In the sidebar footer", None),
        ("In the command palette only", None),
      ],
    )
  };
  let options = options
    .into_iter()
    .map(|(label, description)| QuestionOption::new(label, description))
    .collect();
  let question = Question::new(text, Some("Docs"), options, several).map_err(invalid)?;
  let id = format!("{}:ask_1", invocation.id);
  let request = QuestionRequest::new(
    id.clone(),
    (run.id(), invocation.execution, invocation.id),
    AGENT.to_owned(),
    vec![question],
    (at, QuestionTurn::InTurn),
  )
  .map_err(invalid)?;
  kontur.db.ask_questions(&request)?;
  match ends {
    Ending::Waiting => {}
    Ending::Answered(by) => {
      let given = GivenAnswer {
        choices: vec!["In the top bar".to_owned()],
        text: None,
      };
      let answers = request.answer(&[given]).map_err(invalid)?;
      let answered = Timestamp::from_unix_millis(at.unix_millis() + 60_000);
      kontur
        .db
        .answer_questions((run.id(), &id), &answers, (by, answered), None)?;
    }
    Ending::Interrupted => {
      kontur.db.end_waiting_question(run.id(), &id)?;
    }
  }
  Ok(())
}

/// A request to run `command`, asked at `at` and left as `ends`.
fn approval(
  kontur: &mut Kontur,
  (run, invocation): (&Run, &Invocation),
  command: &str,
  (ends, at): (Option<Decision>, Timestamp),
) -> Result<(), SeedError> {
  let request = PendingToolApproval {
    id: format!("{}:call_1", invocation.id),
    run: run.id(),
    execution: invocation.execution,
    invocation: invocation.id,
    agent: AGENT.to_owned(),
    command: true,
    scope: Some("The step needs it to go on.".to_owned()),
    server: "command".to_owned(),
    action: command.to_owned(),
    target: None,
    requested_at: at,
    native: None,
  };
  kontur.db.request_tool_approval(&request)?;
  let decided = Timestamp::from_unix_millis(at.unix_millis() + 60_000);
  match ends {
    None => {}
    Some(Decision::Decided(by)) => {
      let decision = if by.approves() {
        ToolApprovalDecision::Approved
      } else {
        ToolApprovalDecision::Denied
      };
      kontur.db.claim_tool_approval(run.id(), &request.id, by, decided)?;
      kontur.db.delivered_tool_approval(run.id(), &request.id, decision)?;
    }
    Some(Decision::Ended) => {
      kontur.db.interrupt_tool_approvals(run.id(), invocation.id)?;
    }
  }
  Ok(())
}
