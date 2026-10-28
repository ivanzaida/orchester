//! *With auto mode on, approve everything in each run* (ORC-199): a project's setting, off until someone turns it
//! on, under which every run that uses auto mode answers its requests from its start as if a person had pressed
//! *Approve everything in this run* on its first one. Orchester then answers what auto-approval approves
//! ([`super::auto_verdict`]: worktree commands, reads, MCP calls) without a card, recorded as auto-approved like
//! the run-wide answer's; what it leaves to a person (network, writes outside the worktree, secrets, installs,
//! destructive or unread commands) still asks, and auto mode's reviewer still reviews it. The runner reads the
//! setting and the run's auto mode at each request, so turning either off stops it from the next request on; a
//! person's own choice for the run (*Stop*, the run's switch) wins over it. It widens what agents may do without a
//! person, so only a person turns it on, or automation while a person lets it change agent permissions; turning it
//! off is anyone's.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::Answerer;

/// The project's setting: off, or on as a person or automation turned it on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApproveEverything(Option<Answerer>);

/// Automation asked to turn the setting on while no person lets it change agent permissions.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("only a person turns on approving everything in each run")]
pub struct ApproveEverythingPersonOnly;

impl ApproveEverything {
  pub const OFF: Self = Self(None);

  /// On, as `by` turned it on.
  pub const fn on(by: Answerer) -> Self {
    Self(Some(by))
  }

  pub const fn is_on(self) -> bool {
    self.0.is_some()
  }

  /// Who turned it on; `None` while it is off.
  pub const fn turned_on_by(self) -> Option<Answerer> {
    self.0
  }

  /// Whether it answers the requests of a run, by whether the run uses auto mode now.
  pub const fn covers(self, auto_mode: bool) -> bool {
    auto_mode && self.is_on()
  }

  /// The setting once `by` asked for it `on`: turning it off is always taken, and asking for what it is already
  /// changes nothing; turning it on is a person's, or automation's when `automation_may` (a person lets automation
  /// change agent permissions).
  pub const fn changed(
    self,
    on: bool,
    by: Answerer,
    automation_may: bool,
  ) -> Result<Self, ApproveEverythingPersonOnly> {
    if !on {
      return Ok(Self::OFF);
    }
    if self.is_on() {
      return Ok(self);
    }
    match by {
      Answerer::Person => Ok(Self::on(Answerer::Person)),
      Answerer::Automation if automation_may => Ok(Self::on(Answerer::Automation)),
      Answerer::Automation => Err(ApproveEverythingPersonOnly),
    }
  }
}
