//! Where a scenario leaves a task: a draft, queued, or a run stopped at one of the states the task page
//! shows (`Task page` `iwrwB`), recorded step by step through [`Drive`].

use orchester_core::{
  run::{RunLimit, RunStatus},
  task::{Task, TaskStatus},
  time::Timestamp,
};

use super::{Kontur, SeedError, catalogue::Published, run::Drive};

/// What the Plan step proposes first, and after the person asked for changes.
const PLAN_V1: &str = "1. Read the rate limits GitHub sends with each response.\n2. Wait until the limit resets \
                       before the next request.\n3. Cover the waiting with tests.";
const PLAN_V2: &str = "1. Read the rate limits GitHub sends with each response.\n2. Queue messages while the limit \
                       is spent and send them when it resets.\n3. Show queued messages in the status bar.\n4. Cover \
                       the queue with tests.";
const CHANGES_NOTE: &str = "Don't drop messages while we wait: queue them and show how many wait.";
const QUESTION: &str = "Should a contact that exists in both keep the HubSpot owner or ours?";
/// What the Builder asks to run in the approval state.
pub const APPROVAL_COMMAND: &str = "cargo add serde --features derive";

/// A task's state, with the step its run stopped at where that matters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
  /// A draft that misses details: it cannot be queued yet.
  Draft,
  /// A draft with every detail: Run queues it.
  ReadyDraft,
  /// Queued; its run has not started.
  Queued,
  Running(&'static str),
  /// Running, its agent waiting for a person to approve a command ([`APPROVAL_COMMAND`]).
  Approval(&'static str),
  /// The step's agent asked a question and waits for the answer.
  Question(&'static str),
  /// Plan proposed its first plan, which waits for a decision.
  PlanReview,
  /// Plan proposed a second plan after the person asked for changes to the first.
  PlanRevised,
  Paused(&'static str),
  AtLimit(&'static str, RunLimit),
  /// The app ended while the run worked.
  Interrupted(&'static str),
  Cancelled(&'static str),
  /// The step failed until its retries ran out: the run waits for a person.
  Stuck(&'static str),
  /// As [`Shape::Stuck`], then the person stopped the run.
  Failed(&'static str),
  /// Every step passed: ready for review.
  Succeeded,
  /// Ready for review, then merged into main and its worktree removed.
  Merged,
  /// Running at the step, then its stored record damaged so it cannot be read back.
  Unreadable(&'static str),
}

impl Shape {
  /// How the seed's report names the state.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Draft => "draft, missing details",
      Self::ReadyDraft => "draft, ready to queue",
      Self::Queued => "queued, run not started",
      Self::Running(_) => "running",
      Self::Approval(_) => "running, approval waiting",
      Self::Question(_) => "question waiting",
      Self::PlanReview => "plan v1 waiting",
      Self::PlanRevised => "plan v2 waiting",
      Self::Paused(_) => "paused",
      Self::AtLimit(_, RunLimit::DailyBudget) => "stopped by the daily budget",
      Self::AtLimit(..) => "stopped at its token limit",
      Self::Interrupted(_) => "interrupted",
      Self::Cancelled(_) => "cancelled",
      Self::Stuck(_) => "waits after retries",
      Self::Failed(_) => "failed",
      Self::Succeeded => "ready for review",
      Self::Merged => "merged",
      Self::Unreadable(_) => "latest run unreadable",
    }
  }
}

/// Leaves `task` in `shape`, its run (if it has one) started at `at` on `pipeline`.
pub fn apply(
  kontur: &mut Kontur,
  task: &mut Task,
  pipeline: Option<&Published>,
  shape: Shape,
  at: Timestamp,
) -> Result<(), SeedError> {
  kontur.seeded(task, shape.name());
  if matches!(shape, Shape::Draft | Shape::ReadyDraft) {
    return Ok(());
  }
  let pipeline = pipeline.ok_or(SeedError::NoPipeline)?;
  if task.status() == TaskStatus::Draft {
    kontur.queue(task, pipeline)?;
  }
  if shape == Shape::Queued {
    return Ok(());
  }
  let mut drive = Drive::start(kontur, task, pipeline, at)?;
  drive_to(&mut drive, shape)?;
  let run = drive.finish();
  if let Shape::Unreadable(_) = shape {
    kontur.damage(run.id())?;
  }
  Ok(())
}

fn drive_to(drive: &mut Drive<'_>, shape: Shape) -> Result<(), SeedError> {
  match shape {
    Shape::Draft | Shape::ReadyDraft | Shape::Queued => {}
    Shape::Running(step) | Shape::Unreadable(step) => {
      drive.pass_before(step)?.begin(step)?;
    }
    Shape::Approval(step) => {
      drive
        .pass_before(step)?
        .begin(step)?
        .approval(step, APPROVAL_COMMAND, "Builder")?;
    }
    Shape::Question(step) => {
      drive
        .pass_before(step)?
        .begin(step)?
        .save_work(step)?
        .ask(step, QUESTION)?;
    }
    Shape::PlanReview => {
      drive.pass_before("plan")?.propose("plan", PLAN_V1)?;
    }
    Shape::PlanRevised => {
      let first = drive.pass_before("plan")?.propose("plan", PLAN_V1)?;
      drive
        .request_changes("plan", first, CHANGES_NOTE)?
        .propose("plan", PLAN_V2)?;
    }
    Shape::Paused(step) => {
      drive.pass_before(step)?.begin(step)?.save_work(step)?.pause()?;
    }
    Shape::AtLimit(step, limit) => {
      drive
        .pass_before(step)?
        .begin(step)?
        .save_work(step)?
        .stop_at_limit(limit)?;
    }
    Shape::Interrupted(step) => {
      drive.pass_before(step)?.begin(step)?.save_work(step)?.interrupt()?;
    }
    Shape::Cancelled(step) => {
      drive.pass_before(step)?.begin(step)?.cancel()?;
    }
    Shape::Stuck(step) => {
      drive.fail_until_stuck(step)?;
    }
    Shape::Failed(step) => {
      drive.fail_until_stuck(step)?;
      if matches!(drive.run().status(), RunStatus::WaitingForPerson(_)) {
        drive.stop()?;
      }
    }
    Shape::Succeeded => {
      drive.pass_all()?;
    }
    Shape::Merged => {
      drive.pass_all()?.merge()?;
    }
  }
  Ok(())
}
