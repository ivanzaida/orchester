//! The person's decisions on a run — a plan, a stuck run, a tool approval, a question's answers — each answered once
//! it is carried out, without holding up the runner's other requests.

use orchester_core::{
  id::{RunId, StepExecutionId},
  question::GivenAnswer,
  run::{Answerer, ApprovalAnswer, EventSequence, PlanDecision, PlanVersion, StuckDecision},
};
use orchester_ipc::Command;

use super::{Answer, Later};
use crate::execution::Scheduler;

impl Later<'_> {
  /// Routes the decision commands.
  pub(super) fn decision(&self, handle: Scheduler, command: &Command) -> Answer {
    match *command {
      Command::DecidePlan {
        run,
        execution,
        version,
        ref decision,
        by,
      } => self.plan(handle, (run, execution, version), (decision.clone(), by)),
      Command::DecideStuckRun { run, ref decision } => self.stuck(handle, run, (decision.clone(), None)),
      Command::DecideStuckRunAs {
        run,
        expected,
        ref decision,
        by,
      } => self.stuck(handle, run, (decision.clone(), Some((by, expected)))),
      Command::DecideToolApproval {
        run,
        ref approval,
        ref answer,
        by,
      } => self.tool_approval(handle, run, approval, (answer.clone(), by)),
      Command::AnswerQuestion {
        run,
        ref request,
        ref answers,
        by,
      } => self.question(handle, run, (request, answers, by)),
      _ => unreachable!("answer routes only the five decision commands here"),
    }
  }

  fn tool_approval(
    &self,
    scheduler: Scheduler,
    run: RunId,
    approval: &str,
    (answer, by): (ApprovalAnswer, Answerer),
  ) -> Answer {
    let approval = approval.to_owned();
    self.answer(run, async move {
      scheduler.decide_tool_approval(run, approval, answer, by).await
    })
  }

  fn question(
    &self,
    scheduler: Scheduler,
    run: RunId,
    (request, answers, by): (&str, &[GivenAnswer], Answerer),
  ) -> Answer {
    let (request, answers) = (request.to_owned(), answers.to_vec());
    self.answer(run, async move {
      scheduler.answer_question(run, request, answers, by).await
    })
  }

  fn plan(
    &self,
    scheduler: Scheduler,
    (run, execution, version): (RunId, StepExecutionId, PlanVersion),
    (decision, by): (PlanDecision, Answerer),
  ) -> Answer {
    self.answer(run, async move {
      scheduler.decide_plan(run, execution, version, decision, by).await
    })
  }

  fn stuck(
    &self,
    scheduler: Scheduler,
    run: RunId,
    (decision, seen): (StuckDecision, Option<(Answerer, EventSequence)>),
  ) -> Answer {
    self.answer(run, async move {
      match seen {
        Some((by, expected)) => scheduler.decide_stuck_as(run, decision, by, expected).await,
        None => scheduler.decide_stuck(run, decision).await,
      }
    })
  }
}
