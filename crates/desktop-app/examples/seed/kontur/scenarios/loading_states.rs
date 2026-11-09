//! `loading-states`: the Tasks scenario's project with what every screen's loading state needs to be reached
//! (ORC-110), so one QA copy holds each screen's reads with `hold_reads`: *Checkout* and a module with two of its
//! tasks (`06 · Module · loading` `e0ZdI`).

use super::{modules, tasks};
use crate::kontur::{Kontur, SeedError};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  tasks::seed(kontur)?;
  modules::checkout(kontur)
}
