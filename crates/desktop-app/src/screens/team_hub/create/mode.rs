//! Where a form creates its resource: in a team's project, where Create publishes v1 for the team and the form says
//! what each computer fills in afterwards, or in a local project, where every field is the person's own and Create
//! makes v1 on this computer (`Agents · New agent profile` … `New tool`, L3–L7 in `acFkz`). The forms are the same
//! parts; what differs is the words around them and the fields a local resource can also hold (a profile's connection
//! and MCP servers, a local MCP server's program path and secret values).

use super::super::model::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::screens) enum Mode {
  Team,
  Local,
}

impl Mode {
  pub(super) const fn is_local(&self) -> bool {
    matches!(self, Self::Local)
  }

  /// `local` in a local project, else `team`.
  pub(super) const fn words(&self, team: &'static str, local: &'static str) -> &'static str {
    match self {
      Self::Team => team,
      Self::Local => local,
    }
  }

  /// What the head says after the kind: not created yet, and what Create makes.
  pub(super) const fn not_created(&self, kind: Kind) -> &'static str {
    match (self, kind) {
      (Self::Team, _) => "team_new.notCreatedYet",
      (Self::Local, Kind::McpServer) => "local_new.notCreatedMcp",
      (Self::Local, _) => "local_new.notCreated",
    }
  }

  /// What the bar says at its end about Create.
  pub(super) const fn bar_hint(&self, kind: Kind) -> &'static str {
    match (self, kind) {
      (Self::Team, _) => "team_new.bar.hint",
      (Self::Local, Kind::McpServer) => "local_new.bar.hintMcp",
      (Self::Local, _) => "local_new.bar.hint",
    }
  }

  /// The prefix of the ids of the form's panel and bar, which tests and automation address.
  pub(super) const fn prefix(&self) -> &'static str {
    match self {
      Self::Team => "team",
      Self::Local => "local",
    }
  }
}
