//! The end-of-day report of the work done (ORC-47, `orchester_core::orchestrator::report`): built from what the
//! project's database holds ([`facts`]) by the rules of [`rules`], delivered once a day at the time the person set
//! (Settings › Notifications; off until they turn it on) as an entry of the project's latest Orchestrator
//! conversation, or of one it opens while the project has none. A day without activity gets none; a report whose time
//! passed while the app was closed comes at its next start, labelled with its day ([`rules::due_days`]). With *Let the
//! Orchestrator answer agents* on, the report starts one Orchestrator turn by itself, which adds a summary and only
//! proposes (`orchestrator::waits` starts it as a wait's). `get_daily_report` reads the same report.

mod facts;
mod rules;
mod usage;
mod waiting;

use std::path::Path;

use chrono::NaiveDate;
use orchester_core::{
  id::ChatId,
  orchestrator::{Chat, DailyReport},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

pub use self::rules::{CATCH_UP_DAYS, due_days};
use self::{facts::DayFacts, rules::active};
use super::settings::OrchestratorSetup;
use crate::messages::{self, Message};

/// The facts of `day` in the project at `root` read at `now`, with the latest runs `programs`' git proves merged
/// without Orchester settled.
fn facts_in(
  (db, root, programs): (&ProjectDb, &Path, &Programs),
  (day, now): (NaiveDate, Timestamp),
) -> Result<DayFacts, StoreError> {
  let mut facts = DayFacts::read(db, (day, now))?;
  facts.settle_outside(programs, root);
  Ok(facts)
}

/// The report of `day` in the project at `root` as it reads at `now`; git, run by `programs`, tells which succeeded
/// runs were merged without Orchester. Blocks on SQLite and git.
pub fn build_in(
  (root, programs): (&Path, &Programs),
  (day, now): (NaiveDate, Timestamp),
) -> Result<DailyReport, Message> {
  let built = || {
    let db = ProjectDb::open(root)?;
    let facts = facts_in((&db, root, programs), (day, now))?;
    Ok::<_, StoreError>(rules::build(&facts, db.project().id()))
  };
  built().map_err(|error| messages::store_error(&error))
}

/// The report of `day` as `get_daily_report` gives it: the one the project's conversation kept for that day, the same
/// it was delivered with, or else built now; and whether it was kept. Blocks on SQLite and git.
pub fn report_in(
  (root, programs): (&Path, &Programs),
  (day, now): (NaiveDate, Timestamp),
) -> Result<(DailyReport, bool), Message> {
  let kept = ProjectDb::open(root)
    .and_then(|db| db.daily_report(&DailyReport::key_of(day)))
    .map_err(|error| messages::store_error(&error))?;
  match kept {
    Some(report) => Ok((report, true)),
    None => Ok((build_in((root, programs), (day, now))?, false)),
  }
}

/// What delivering a day's report did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
  /// The report was kept in the conversation now.
  Kept,
  /// The day had its report already.
  Known,
  /// The day had no activity: it gets no report.
  Quiet,
}

/// Delivers the report of `day` in the project at `root` at `now`, unless the day has one or had no activity: in the
/// project's latest conversation, or a new one titled `title` while it has none. It starts an Orchestrator turn by
/// itself when `answers` (*Let the Orchestrator answer agents*) is on and the Orchestrator has what it runs on.
/// Blocks on SQLite and git.
pub fn deliver_in(
  (root, programs): (&Path, &Programs),
  answers: bool,
  (day, now): (NaiveDate, Timestamp),
  title: &str,
) -> Result<Delivery, Message> {
  let automatic = answers && OrchestratorSetup::load_in(root)?.readiness().is_ready();
  let delivered = || {
    let mut db = ProjectDb::open(root)?;
    if db.daily_report(&DailyReport::key_of(day))?.is_some() {
      return Ok(Delivery::Known);
    }
    let facts = facts_in((&db, root, programs), (day, now))?;
    let report = rules::build(&facts, db.project().id());
    if !active(&facts, &report) {
      return Ok(Delivery::Quiet);
    }
    let report = DailyReport { automatic, ..report };
    let chat = match db.latest_chat()? {
      Some(id) => db.chat(id)?,
      None => None,
    }
    .unwrap_or_else(|| Chat::named(ChatId::generate(), title, now));
    Ok::<_, StoreError>(if db.keep_daily_report(&chat, &report, now)? {
      Delivery::Kept
    } else {
      Delivery::Known
    })
  };
  delivered().map_err(|error| messages::store_error(&error))
}
