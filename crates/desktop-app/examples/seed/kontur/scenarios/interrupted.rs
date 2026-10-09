//! `interrupted`: the `tasks` project after Orchester ended without closing (`Overview · interrupted runs ·
//! dark` `JAIYi`): its runs are still recorded as running, and the application database's last session has
//! no end, so the next start finds it crashed and the runner records those runs interrupted.

use orchester_store::AppDb;

use super::tasks;
use crate::kontur::{Kontur, SeedError};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  tasks::seed(kontur)?;
  let started = kontur.ago(90);
  // The pid of a process that ended: this one, once the seed exits.
  AppDb::open(&kontur.app_database)?.start_session(std::process::id(), started)?;
  Ok(())
}
