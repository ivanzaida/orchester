//! `cards`: one task per state of the conversation's decision cards (`Cards and rows inside the conversation`
//! `YhiGC`), each a Feature delivery run at work on Implement unless said otherwise: a question its step waits
//! on; questions with suggested answers waiting (one answer, several answers), answered by the person,
//! answered by automation and ended before an answer; a command waiting for approval, which auto mode left to the
//! person by a hard rule (its card says why); commands approved once by the person, denied by the person with a
//! message ([`DENIAL_NOTE`]), approved by automation and ended without an answer; a plan longer
//! than its card shows waiting (v1), and a plan revised after changes were asked for (v2); Orchester's
//! `start_qa_app` waiting for approval, which offers *Approve for every run of this project*, with a rule of the
//! project kept for an earlier version of the QA script (Project settings › Auto mode lists it), made by approving a
//! `start_qa_app` of another task for every run of the project.
//!
//! Its runs are recorded, not executed: without a runner a run recorded at work reads interrupted, and so does
//! what its agent asked in its turn; a runner that attaches ends it. So that the waiting command and
//! questions with suggested answers can be answered on screen without a runner, their step is recorded
//! waiting for the person (`held`), which the runner itself does not do for them; two more are left at
//! work, so they read interrupted. Live runs (`live`) show them as the runner keeps them.

use orchester_core::{
  orchester_tools::{self, OrchesterTool, QaApproval},
  question::{GivenAnswer, Question, QuestionOption, QuestionRequest, QuestionTurn},
  run::{
    Answerer, DecidedBy, DenialNote, Invocation, PendingToolApproval, ProjectScope, Run, ToolApprovalDecision,
    WaitReason,
    review::{Review, ReviewHold},
  },
  store::{ActivityStore, RunStore},
  task::Task,
  time::Timestamp,
};

use crate::kontur::{
  Kontur, SeedError,
  catalogue::Published,
  run::Drive,
  shapes::{self, APPROVAL_COMMAND, Shape},
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
  Choice { several: bool, ends: Ending, held: bool },
  /// A command that asks to be approved, how it ended, and whether its step is held waiting for it.
  Command(&'static str, Option<Decision>, bool),
  /// Plan proposes a plan, which waits for a decision.
  Plan(&'static str),
  /// Orchester's own `read_external_file` asks to read in a folder beside the repository, held waiting.
  OutsideRead,
  /// Orchester's own `start_qa_app` asks to start a QA copy: held waiting, or approved by the person for every run of
  /// the project.
  QaStart { decided: bool },
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

/// The QA script's git blob the waiting `start_qa_app` names, and the one a rule of the project was kept for.
const QA_SCRIPT: &str = "3f2a1b9c0d4e5f60718293a4b5c6d7e8f9012345";
const EARLIER_QA_SCRIPT: &str = "77aa0012bb34cc56dd78ee90ff12aa34bb56cc78";
/// What the person told the agent as they denied its command (*Deny with a message*).
const DENIAL_NOTE: &str = "Publish it from CI once the version is bumped.";

const CARDS: [(&str, Card, &str); 18] = [
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
      held: true,
    },
    "question with options waiting",
  ),
  (
    "Choose the export formats",
    Card::Choice {
      several: true,
      ends: Ending::Waiting,
      held: true,
    },
    "question with several answers waiting",
  ),
  (
    "Name the retry setting",
    Card::Choice {
      several: false,
      ends: Ending::Answered(Answerer::Person),
      held: false,
    },
    "question with options answered by you",
  ),
  (
    "Pick the cache store",
    Card::Choice {
      several: false,
      ends: Ending::Answered(Answerer::Automation),
      held: false,
    },
    "question with options answered by automation",
  ),
  (
    "Pick the log level",
    Card::Choice {
      several: false,
      ends: Ending::Interrupted,
      held: false,
    },
    "question with options ended without an answer",
  ),
  (
    "Add serde to the webhook payloads",
    Card::Command(APPROVAL_COMMAND, None, true),
    "command waiting for approval",
  ),
  (
    "Add jitter to the retries",
    Card::Command("cargo add rand", None, false),
    "command waiting, its run interrupted",
  ),
  (
    "Pick the retry header",
    Card::Choice {
      several: false,
      ends: Ending::Waiting,
      held: false,
    },
    "question with options waiting, its run interrupted",
  ),
  (
    "Add rand to the jitter",
    Card::Command("cargo add rand", Some(Decision::Decided(DecidedBy::Person)), false),
    "command approved once by you",
  ),
  (
    "Publish the webhook crate",
    Card::Command(
      "cargo publish --dry-run",
      Some(Decision::Decided(DecidedBy::PersonDeny)),
      false,
    ),
    "command denied by you",
  ),
  (
    "Fetch the latest schema",
    Card::Command(
      "git fetch origin",
      Some(Decision::Decided(DecidedBy::Automation)),
      false,
    ),
    "command approved by automation",
  ),
  (
    "Push the release tag",
    Card::Command("git push origin v1.2.0", Some(Decision::Ended), false),
    "command ended without an answer",
  ),
  ("Add retry to webhook sender", Card::Plan(LONG_PLAN), "plan v1 waiting"),
  (
    "Read the docs of the sibling project",
    Card::OutsideRead,
    "Orchester's read outside the worktree waiting for approval",
  ),
  (
    "Back off when GitHub rate-limits the sync",
    Card::Shape(Shape::PlanRevised),
    "plan v2 waiting",
  ),
  (
    "Check the board in a QA copy",
    Card::QaStart { decided: false },
    "Orchester's start_qa_app waiting for approval",
  ),
  (
    "Check the settings page in a QA copy",
    Card::QaStart { decided: true },
    "Orchester's start_qa_app approved by you for every run of this project",
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
      // Two minutes into the agent's turn.
      let at = Timestamp::from_unix_millis(invocation.started_at.unix_millis() + 2 * 60_000);
      match card {
        Card::Choice { several, ends, held } => {
          choice(kontur, (&run, &invocation), several, (ends, at))?;
          hold(kontur, run, (&invocation, held))?;
        }
        Card::Command(command, ends, held) => {
          approval(kontur, (&run, &invocation), command, (ends, at))?;
          if held && ends.is_none() {
            held_by_auto_mode(kontur, (&run, &invocation))?;
          }
          hold(kontur, run, (&invocation, held))?;
        }
        Card::OutsideRead => {
          outside_read(kontur, (&run, &invocation), at)?;
          hold(kontur, run, (&invocation, true))?;
        }
        Card::QaStart { decided } => {
          qa_start(kontur, (&run, &invocation), (decided, at))?;
          hold(kontur, run, (&invocation, !decided))?;
        }
        Card::Shape(_) | Card::Plan(_) => {}
      }
      continue;
    };
    shapes::apply(kontur, &mut task, Some(pipeline), shape, started)?;
  }
  Ok(())
}

/// `task`'s run, started at `at`, with Implement's agent at work: the run and its invocation.
pub(super) fn at_work(
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

/// With `held`, `invocation`'s step is recorded waiting for the person, so its request reads waiting without a
/// runner.
pub(super) fn hold(
  kontur: &mut Kontur,
  mut run: Run,
  (invocation, held): (&Invocation, bool),
) -> Result<(), SeedError> {
  if !held {
    return Ok(());
  }
  let at = Timestamp::from_unix_millis(invocation.started_at.unix_millis() + 3 * 60_000);
  let events = run.wait_step(invocation.execution, WaitReason::Approval, at)?;
  kontur.db.append_events(&run, &events)?;
  Ok(())
}

/// Auto mode left the waiting request of `invocation` to the person without a review, by a hard rule (the step's
/// network is off and the command installs), so its card says why.
fn held_by_auto_mode(kontur: &mut Kontur, (run, invocation): (&Run, &Invocation)) -> Result<(), SeedError> {
  let held = Review::Held {
    hold: ReviewHold::NetworkOff,
  };
  kontur
    .db
    .record_review(run.id(), &format!("{}:call_1", invocation.id), &held)?;
  Ok(())
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

/// Orchester's own `read_external_file` asks, at `at`, to read in `kontur-docs/specs` beside the repository.
fn outside_read(kontur: &mut Kontur, (run, invocation): (&Run, &Invocation), at: Timestamp) -> Result<(), SeedError> {
  let folder = kontur
    .repo
    .parent()
    .ok_or(SeedError::NotAtWork)?
    .join("kontur-docs")
    .join("specs");
  let request = PendingToolApproval {
    id: format!("{}:read_1", invocation.id),
    run: run.id(),
    execution: invocation.execution,
    invocation: invocation.id,
    agent: AGENT.to_owned(),
    command: false,
    scope: Some(r#"{"path":"README.md"}"#.to_owned()),
    server: orchester_tools::SERVER.to_owned(),
    action: OrchesterTool::ReadExternalFile.as_str().to_owned(),
    target: Some(folder.display().to_string()),
    requested_at: at,
    native: None,
  };
  kontur.db.request_tool_approval(&request)?;
  Ok(())
}

/// Orchester's own `start_qa_app` asks, at `at`, to start a QA copy on the `tasks` scenario. Waiting, it names the
/// QA script as it is now; `decided`, an earlier version of it, which the person approved for every run of the
/// project a minute later: the rule of the project names that request, and answered three later calls.
fn qa_start(
  kontur: &mut Kontur,
  (run, invocation): (&Run, &Invocation),
  (decided, at): (bool, Timestamp),
) -> Result<(), SeedError> {
  let script = if decided { EARLIER_QA_SCRIPT } else { QA_SCRIPT };
  let target = QaApproval {
    script: script.to_owned(),
    scenario: "tasks".to_owned(),
    changed: Vec::new(),
  };
  let request = PendingToolApproval {
    id: format!("{}:qa_1", invocation.id),
    run: run.id(),
    execution: invocation.execution,
    invocation: invocation.id,
    agent: AGENT.to_owned(),
    command: false,
    scope: Some(r#"{"scenario":"tasks"}"#.to_owned()),
    server: orchester_tools::SERVER.to_owned(),
    action: OrchesterTool::StartQaApp.as_str().to_owned(),
    target: Some(target.target()),
    requested_at: at,
    native: None,
  };
  kontur.db.request_tool_approval(&request)?;
  if !decided {
    return Ok(());
  }
  let answered = Timestamp::from_unix_millis(at.unix_millis() + 60_000);
  kontur
    .db
    .claim_tool_approval(run.id(), &request.id, DecidedBy::PersonSimilar, answered)?;
  kontur
    .db
    .delivered_tool_approval(run.id(), &request.id, ToolApprovalDecision::Approved)?;
  let scope = ProjectScope::of_request(&request).ok_or_else(|| invalid("a start_qa_app rule"))?;
  let rule = kontur
    .db
    .add_project_approval_rule(&scope, (run.id(), &request.id), answered)?;
  for _ in 0..3 {
    kontur.db.project_rule_answered(rule.id)?;
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
      // The person's denial says why, as *Deny with a message* sends it.
      if by == DecidedBy::PersonDeny {
        let note = DenialNote::new(DENIAL_NOTE).expect("the seed's words fit a denial");
        kontur.db.record_denial_note(run.id(), &request.id, &note)?;
      }
      kontur.db.delivered_tool_approval(run.id(), &request.id, decision)?;
    }
    Some(Decision::Ended) => {
      kontur.db.interrupt_tool_approvals(run.id(), invocation.id)?;
    }
  }
  Ok(())
}
