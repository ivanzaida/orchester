//! The role of a profile as the Agents hub's panels show and edit it (`Role` in L17 `b7Tbxb`): its name, what it
//! answers for, and the role instructions that come first in its agents' prompts. A panel holds what is typed as it
//! is, and asks here whether it can be saved, by the rules the profile's editor saves it by ([`super::form`]).

use orchester_core::{
  name::Name,
  resource::{AgentProfile, PromptText, TeamAgentProfile},
};

use super::form::{prompt_error, role_error};
use crate::messages::Message;

/// The role's fields as typed: words, which [`Self::errors`] checks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoleText {
  /// The role's name.
  pub role: String,
  /// What it answers for.
  pub responsibilities: String,
  /// The role instructions, first in its agents' prompts.
  pub instructions: String,
}

impl RoleText {
  /// The role of `profile`.
  pub fn of(profile: &AgentProfile) -> Self {
    Self {
      role: profile.role().as_str().to_owned(),
      responsibilities: profile.responsibilities().to_owned(),
      instructions: profile.instructions().as_str().to_owned(),
    }
  }

  /// The role of the team profile `profile`.
  pub fn of_team(profile: &TeamAgentProfile) -> Self {
    Self {
      role: profile.role().as_str().to_owned(),
      responsibilities: profile.responsibilities().to_owned(),
      instructions: profile.instructions().as_str().to_owned(),
    }
  }

  /// Why the fields cannot be saved, per field; `None` when they can.
  pub fn errors(&self) -> Option<RoleErrors> {
    let errors = RoleErrors {
      role: Name::new(&self.role).err().map(|error| role_error(&error)),
      instructions: PromptText::new(&self.instructions)
        .err()
        .map(|error| prompt_error(&error)),
    };
    (errors.role.is_some() || errors.instructions.is_some()).then_some(errors)
  }
}

/// Why a role's fields cannot be saved: its name is no name, or its instructions are no prompt.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoleErrors {
  pub role: Option<Message>,
  pub instructions: Option<Message>,
}

impl RoleErrors {
  /// The first thing wrong, in the fields' order.
  pub fn first(&self) -> Option<&Message> {
    self.role.as_ref().or(self.instructions.as_ref())
  }
}
