//! The team scenarios of tasks the stand-in team server records as closed or blocked (decision D4, `Task - v2`
//! T4–T6, T11a–f): the QA app's own states make them (`examples/qa_app/team/closed.rs`), so these only name the state.

use super::team::{mark, state::State};
use crate::kontur::{Kontur, SeedError};

/// An Admin on one task of each way the team server records a close or a block.
pub fn admin(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::ClosedAdmin)
}

/// As [`admin`], the person a member.
pub fn member(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::ClosedMember)
}
