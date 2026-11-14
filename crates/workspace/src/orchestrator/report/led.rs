//! What the Orchestrator did on its own on a day and what it left to the person
//! (`orchester_core::orchestrator::report::LedDay`, `docs/plan/orchestrator-autonomy/OA-25-…`): its decisions of the
//! day with their reasons, what is parked, the led targets that are stuck, the events left to the person after their
//! tries, what holds its turns, and what each leading subscription spent. Read from the project's records; a day
//! with nothing of it has no such section.

use chrono::NaiveDate;
use orchester_core::{
  orchestrator::{
    Held, NextTry, WatchTarget,
    report::{LedDay, ReportDecision, ReportHeld, ReportLeading, ReportParked},
    standing::TargetStanding,
  },
  time::Timestamp,
};
use orchester_store::{DecisionsRead, ProjectDb, StoreError};

use super::rules::Day;
use crate::{
  orchestrator::{ceilings::spent, held::held, standing::Reads},
  tasks::recorded::task_of,
};

/// The section of `day` in `db`'s project, read at `now`; `None` when there is nothing of it to tell.
pub(super) fn of(db: &ProjectDb, (day, now): (NaiveDate, Timestamp)) -> Result<Option<LedDay>, StoreError> {
  let (start, end) = Day::of(day).bounds();
  let mut decisions = db.decisions(&DecisionsRead {
    since: Some(start),
    until: Some(end),
    ..DecisionsRead::default()
  })?;
  decisions.sort_by_key(|decision| (decision.made.at, decision.number));
  let mut parked = Vec::new();
  for parking in db.parked_tasks()? {
    // A parked task the project no longer keeps has nobody to act on it.
    if let Some(task) = task_of(db, parking.task)? {
      parked.push(ReportParked {
        task: task.key().to_string(),
        needs: parking.needs.name().to_owned(),
        to_do: parking.to_do.as_str().to_owned(),
        since: parking.at,
      });
    }
  }
  let left_to_person = db
    .automatic_waits_failed()?
    .iter()
    .filter(|failed| NextTry::after(&failed.last, failed.earlier.len()) == NextTry::LeftToPerson)
    .count();
  let (leading, stuck) = leading(db)?;
  let led = LedDay {
    open_decisions: decisions.iter().filter(|decision| decision.review.is_open()).count(),
    decisions: decisions
      .into_iter()
      .map(|decision| ReportDecision {
        number: decision.number,
        tool: decision.made.tool,
        task: decision.made.task,
        reason: decision.made.reason.as_str().to_owned(),
        review: decision.review.name().to_owned(),
        at: decision.made.at,
      })
      .collect(),
    parked,
    stuck,
    left_to_person,
    held: held(db, now)?.map(|held| ReportHeld {
      by: match held {
        Held::Budget { .. } => "budget",
        Held::PlanLimit { .. } => "plan_limit",
      }
      .to_owned(),
      until: held.until(),
    }),
    leading,
  };
  Ok((!led.is_empty()).then_some(led))
}

/// The subscriptions that lead to the end, each with where it stands and what it spent, and those of them that are
/// stuck, by their names. Reads nothing of the project's tasks while none leads to the end.
fn leading(db: &ProjectDb) -> Result<(Vec<ReportLeading>, Vec<String>), StoreError> {
  let subscriptions = db.active_subscriptions()?;
  let ends: Vec<_> = subscriptions
    .as_slice()
    .iter()
    .filter(|subscription| subscription.mode().ceilings().is_some())
    .collect();
  if ends.is_empty() {
    return Ok((Vec::new(), Vec::new()));
  }
  let reads = Reads::read(db)?;
  let (mut leading, mut stuck) = (Vec::new(), Vec::new());
  for subscription in ends {
    let led = reads.led(db, subscription)?;
    let target = match (subscription.target(), &led.name) {
      (_, Some(name)) => name.clone(),
      (WatchTarget::WithoutModule, None) => "Tasks without a module".to_owned(),
      (_, None) => db.project().name().as_str().to_owned(),
    };
    if matches!(led.standing.target, TargetStanding::Stuck { .. }) {
      stuck.push(target.clone());
    }
    leading.push(ReportLeading {
      target,
      standing: led.standing.target.name().to_owned(),
      spent: spent(db, &subscriptions, subscription)?.get(),
      budget: subscription
        .mode()
        .ceilings()
        .and_then(|ceilings| ceilings.budget())
        .map(|budget| budget.get()),
    });
  }
  Ok((leading, stuck))
}
