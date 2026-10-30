//! `loading-states`: the Tasks scenario's project with what every screen's loading state needs to be reached
//! (ORC-110), so one QA copy holds each screen's reads with `hold_reads`: *Checkout*, a module with two of its
//! tasks (`06 · Module · loading` `e0ZdI`), and the project linked to GitHub with its pull requests stored, read
//! from the project's database with no network (`08 Review · Pull requests` `z3yU9o`).

use super::{modules, pull_requests, tasks};
use crate::kontur::{Kontur, SeedError};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  tasks::seed(kontur)?;
  modules::checkout(kontur)?;
  pull_requests::linked(kontur)
}
