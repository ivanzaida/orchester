//! `team-hub-waiting`: the Agents hub of `team-hub` with a new instruction, *Release notes*, that this computer created
//! while the team server could not be reached: its v1 waits in the outbox, written as the form's Create writes it
//! (`ProjectDb::create_published`), and the hub lists it as *Waiting to send to the team server* with no version number
//! (ORC-835). Pick Instructions in the filter. The seed has no team server to answer, so it stays waiting until one
//! does.

use std::collections::BTreeSet;

use orchester_core::{
  name::Name,
  resource::{AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, Slug},
};

use super::team_hub::seed;
use crate::kontur::{Kontur, SeedError};

pub fn waiting(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur)?;
  let notes = Instruction::new(
    PromptText::new("Write release notes for customers, in plain words.").map_err(invalid)?,
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .map_err(invalid)?;
  let at = kontur.ago(2);
  kontur.db.create_published(
    Slug::new("release-notes").map_err(invalid)?,
    Name::new("Release notes").map_err(invalid)?,
    &notes,
    at,
  )?;
  Ok(())
}

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}
