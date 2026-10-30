//! The Modules screens (ORC-112, `Modules · redesign` `oEELJ`).
//!
//! `modules`: over the Usage scenario's runs, whose merges of the last two weeks are history enough for a forecast:
//! *Webhooks v2*, open from 12 days ago to 45 days ahead, its tasks in every workflow status, moved in on its first
//! day and moved along day by day since (a burndown with history, forecast by its target); *Billing exports*, open
//! from 6 days ago to 2 days ahead with most of its work left (forecast after its target); *API rate limits*, open
//! without dates (no ideal line); and *Onboarding v1*, closed.
//!
//! `modules-short`: the Tasks scenario's project, nothing merged yet, with *Checkout* and two of its tasks (not enough
//! history for a forecast). `modules-none`: the same project without a module (the list's empty state).
//!
//! `modules-base-branch` (`Base branch · redesign` `ri5ru`, 02 `eyrnI`, 04b `NDTCw`): `modules` with the branches
//! `release/2.0` and `staging` beside `main`, and `origin/hotfix-1.9` only on the remote; *Webhooks v2* runs from
//! `release/2.0`, and yesterday the project's base branch `main` moved to *Billing exports* and *API rate limits*,
//! which had none, as Project settings › General says once.

use chrono::{Days, NaiveDate};
use orchester_core::{
  integration::base::BaseChoice,
  module::{ModuleDetails, ModuleState},
  name::Name,
  run::BranchName,
  store::TaskStore,
  task::{
    Task,
    organization::{ChangeActor, Priority, TaskOrganization, WorkflowStatus},
  },
  time::Timestamp,
  usage::local_date,
};

use super::{tasks, usage};
use crate::kontur::{Kontur, SeedError, git};

const DAY: i64 = 24 * 60;

/// One module: its name, what it delivers, its dates in days from today and its state, and its tasks: each a title,
/// the workflow status it ends in, and the day before today it reached it.
struct Shape {
  name: &'static str,
  description: &'static str,
  dates: (Option<i64>, Option<i64>),
  state: ModuleState,
  tasks: &'static [(&'static str, WorkflowStatus, i64)],
}

const WEBHOOKS: Shape = Shape {
  name: "Webhooks v2",
  description: "Retries, signing and the delivery log. Failed webhook deliveries retry with backoff, every endpoint \
    signs its payloads with its own secret, and the delivery log keeps each attempt so a failed delivery can be \
    replayed.",
  dates: (Some(-12), Some(45)),
  state: ModuleState::Open,
  tasks: &[
    ("Retry webhook deliveries with backoff", WorkflowStatus::InProgress, 2),
    ("Webhook signing secrets per endpoint", WorkflowStatus::InReview, 1),
    ("Verify webhook signatures on receive", WorkflowStatus::Todo, 3),
    ("Replay a failed delivery", WorkflowStatus::Backlog, 12),
    ("Webhook endpoint settings page", WorkflowStatus::Done, 11),
    ("Delivery log table", WorkflowStatus::Done, 10),
    ("Store each delivery attempt", WorkflowStatus::Done, 9),
    ("Delivery log retention", WorkflowStatus::Done, 8),
    ("Show a delivery's payload", WorkflowStatus::Done, 6),
    ("Timeouts for slow endpoints", WorkflowStatus::Done, 5),
    ("Pause a failing endpoint", WorkflowStatus::Done, 4),
    ("Webhook test button", WorkflowStatus::Done, 2),
    ("Legacy webhook format", WorkflowStatus::Canceled, 7),
  ],
};

const BILLING: Shape = Shape {
  name: "Billing exports",
  description: "CSV and PDF exports for invoices",
  dates: (Some(-6), Some(2)),
  state: ModuleState::Open,
  tasks: &[
    ("Export invoices as CSV", WorkflowStatus::Done, 5),
    ("Invoice PDF layout", WorkflowStatus::Done, 3),
    ("Export a date range", WorkflowStatus::Done, 1),
    ("Email the export when it is ready", WorkflowStatus::InProgress, 1),
    ("Currency in exports", WorkflowStatus::Todo, 6),
    ("Tax columns", WorkflowStatus::Todo, 6),
    ("Export credit notes", WorkflowStatus::Todo, 6),
    ("Scheduled monthly export", WorkflowStatus::Backlog, 6),
    ("Export to the accounting tool", WorkflowStatus::Backlog, 6),
  ],
};

const RATE_LIMITS: Shape = Shape {
  name: "API rate limits",
  description: "Per-user limits and headers",
  dates: (None, None),
  state: ModuleState::Open,
  tasks: &[
    ("Count requests per user", WorkflowStatus::Done, 4),
    ("Rate limit headers", WorkflowStatus::InProgress, 2),
    ("Limits per plan", WorkflowStatus::Todo, 5),
    ("Warn before the limit", WorkflowStatus::Backlog, 5),
  ],
};

const ONBOARDING: Shape = Shape {
  name: "Onboarding v1",
  description: "The first steps of a new account",
  dates: (Some(-30), Some(-14)),
  state: ModuleState::Closed,
  tasks: &[
    ("Welcome screen", WorkflowStatus::Done, 20),
    ("Sample project", WorkflowStatus::Done, 18),
    ("First task tour", WorkflowStatus::Done, 15),
  ],
};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  usage::seed(kontur)?;
  for shape in [&WEBHOOKS, &BILLING, &RATE_LIMITS, &ONBOARDING] {
    module(kontur, shape)?;
  }
  Ok(())
}

/// `modules-base-branch`: `modules` with branches to choose from, one module on its own base branch and the
/// project's base branch moved to two others.
pub fn base_branch(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur)?;
  git::branches(&kontur.repo, &["release/2.0", "staging"], &["hotfix-1.9"])?;
  let branch = |name: &str| BranchName::new(name).map_err(|error| SeedError::Invalid(error.to_string()));
  let modules = kontur.db.modules()?;
  let id = |name: &str| {
    modules
      .iter()
      .find(|module| module.name().as_str() == name)
      .map(|module| module.id())
      .ok_or_else(|| SeedError::Invalid(format!("no module {name}")))
  };
  let checked_out = BaseChoice::CheckedOut;
  let release = BaseChoice::Branch(branch("release/2.0")?);
  kontur.db.set_module_base(id(WEBHOOKS.name)?, &checked_out, release)?;
  let main = branch(git::MAIN)?;
  let took = [id(BILLING.name)?, id(RATE_LIMITS.name)?];
  for module in took {
    kontur
      .db
      .set_module_base(module, &checked_out, BaseChoice::Branch(main.clone()))?;
  }
  // The store records a move only as it migrates a project that had a base branch; the seed writes what it leaves.
  let database = rusqlite::Connection::open(kontur.database())?;
  database.execute(
    "INSERT INTO base_branch_move (id, branch, moved_at) VALUES (1, ?1, ?2)",
    rusqlite::params![main.as_str(), kontur.ago(DAY).unix_millis()],
  )?;
  for module in took {
    database.execute(
      "INSERT INTO base_branch_move_modules (module_id) VALUES (?1)",
      [module.to_string()],
    )?;
  }
  Ok(())
}

/// `modules-short`: a module on a project with nothing merged yet.
pub fn short(kontur: &mut Kontur) -> Result<(), SeedError> {
  tasks::seed(kontur)?;
  checkout(kontur)
}

/// *Checkout*, open from 3 days ago to 4 days ahead, with two of the project's Todo tasks moved in on its first day.
pub fn checkout(kontur: &mut Kontur) -> Result<(), SeedError> {
  let details = details(
    kontur,
    ("Checkout", "Card form and the order summary"),
    (Some(-3), Some(4)),
    ModuleState::Open,
  )?;
  let module = kontur.db.create_module(details, kontur.ago(3 * DAY))?;
  let stored = kontur.db.tasks()?;
  let picked: Vec<&Task> = stored
    .iter()
    .filter(|task| task.organization().status() == WorkflowStatus::Todo)
    .take(2)
    .collect();
  for task in picked {
    let at = kontur.ago(3 * DAY);
    kontur
      .db
      .set_task_module(task.id(), Some(module.id()), ChangeActor::Person, at)?;
  }
  Ok(())
}

/// `modules-none`: the project, no module.
pub fn none(kontur: &mut Kontur) -> Result<(), SeedError> {
  tasks::seed(kontur)
}

/// The module `shape` names, made on its first day with its tasks in it, each moved to its status on its day.
fn module(kontur: &mut Kontur, shape: &Shape) -> Result<(), SeedError> {
  let first = shape
    .tasks
    .iter()
    .map(|(_, _, ago)| *ago)
    .max()
    .unwrap_or(0)
    .max(shape.dates.0.map_or(0, i64::abs));
  let made = kontur.ago(first * DAY + 60);
  let details = details(kontur, (shape.name, shape.description), shape.dates, shape.state)?;
  let module = kontur.db.create_module(details, made)?;
  for (title, status, ago) in shape.tasks {
    let task = kontur.task((title, "What the task changes, as its brief says.", &[]), None, made)?;
    kontur
      .db
      .set_task_module(task.id(), Some(module.id()), ChangeActor::Person, made)?;
    organize(kontur, &task, WorkflowStatus::Todo, made)?;
    if *status != WorkflowStatus::Todo {
      organize(kontur, &task, *status, kontur.ago(ago * DAY))?;
    }
  }
  Ok(())
}

fn organize(kontur: &mut Kontur, task: &Task, status: WorkflowStatus, at: Timestamp) -> Result<(), SeedError> {
  let organization = TaskOrganization::new(status, Priority::None, None, Vec::new(), Vec::new())
    .map_err(|error| SeedError::Invalid(error.to_string()))?;
  kontur
    .db
    .set_task_organization(task.id(), &organization, ChangeActor::Person, at)?;
  Ok(())
}

/// A module's details, its dates `days` from today.
fn details(
  kontur: &Kontur,
  (name, description): (&str, &str),
  (start, target): (Option<i64>, Option<i64>),
  state: ModuleState,
) -> Result<ModuleDetails, SeedError> {
  let today = local_date(kontur.now);
  let day = |days: i64| -> Option<NaiveDate> {
    if days >= 0 {
      today.checked_add_days(Days::new(days.unsigned_abs()))
    } else {
      today.checked_sub_days(Days::new(days.unsigned_abs()))
    }
  };
  let name = Name::new(name).map_err(|error| SeedError::Invalid(error.to_string()))?;
  ModuleDetails::new(name, description, (start.and_then(day), target.and_then(day)), state)
    .map_err(|error| SeedError::Invalid(error.to_string()))
}
