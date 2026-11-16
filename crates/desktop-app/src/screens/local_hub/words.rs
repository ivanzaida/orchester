//! What a row and a panel say of a local resource, in a local project's words: no team, so nothing is "here" or "on
//! this computer" (`Agents · v2` L1, L2, L8, L12, L13).

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{
  resource::ResourceKind,
  sharing::{ConnectionProblem, Missing},
  time::Timestamp,
};
use orchester_workspace::{
  agents::team_hub::Form,
  display,
  readiness::{Group, ReadinessStanding, Undecided},
};

use super::model::Entry;
use crate::screens::{
  hub::profile_form::here::{self, Whose},
  team_hub::words::{cli_key, date},
};

/// What a kind is called in a row's tag under All and Archived (`team_hub.kind.tag.*`); a kind the hub lists has one.
pub(super) const fn kind_tag_key(kind: ResourceKind) -> Option<&'static str> {
  match kind {
    ResourceKind::AgentProfile => Some("team_hub.kind.tag.profile"),
    ResourceKind::McpServer => Some("team_hub.kind.tag.mcp"),
    ResourceKind::Tool => Some("team_hub.kind.tag.tool"),
    ResourceKind::Instruction => Some("team_hub.kind.tag.instruction"),
    ResourceKind::Skill => Some("team_hub.kind.tag.skill"),
    _ => None,
  }
}

/// What a kind is in a panel's head (`team_hub.kind.where.*`).
pub(super) const fn kind_where_key(kind: ResourceKind) -> &'static str {
  match kind {
    ResourceKind::AgentProfile => "team_hub.kind.where.profile",
    ResourceKind::McpServer => "team_hub.kind.where.mcp",
    ResourceKind::Tool => "team_hub.kind.where.tool",
    ResourceKind::Instruction => "team_hub.kind.where.instruction",
    _ => "team_hub.kind.where.skill",
  }
}

/// What an MCP server or a tool is, in a row's tag (`team_hub.form.*`).
pub(super) const fn form_key(form: Form) -> &'static str {
  match form {
    Form::McpProgram => "team_hub.form.program",
    Form::McpRemote => "team_hub.form.remote",
    Form::ToolCommand => "team_hub.form.command",
    Form::ToolHttp => "team_hub.form.http",
    Form::ToolDatabase => "team_hub.form.database",
  }
}

/// What a row says and the color it says it in.
pub(super) struct Said {
  pub text: Arc<str>,
  pub color: PaletteColor,
}

/// How an MCP server that is open in its panel differs from what is saved: the edits are not saved yet, or a save was
/// refused and they stay (`Agents · MCP server · save refused` L12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unsaved {
  Edited,
  Refused,
}

pub(super) fn line(ctx: &Ctx, (now, whose): (Timestamp, Whose<'_>), entry: &Entry, unsaved: Option<Unsaved>) -> Said {
  if let Some(at) = entry.archived {
    return Said {
      text: archived_at(ctx, now, at),
      color: PaletteColor::TextMuted,
    };
  }
  let state = state(ctx, entry, whose);
  match unsaved {
    Some(Unsaved::Refused) => Said {
      text: ctx.t("local_hub.line.saveRefused"),
      color: PaletteColor::Danger,
    },
    Some(Unsaved::Edited) => Said {
      text: ctx.t_args("local_hub.line.unsaved", [("rest", state.text.to_string())]),
      color: PaletteColor::Warning,
    },
    None => state,
  }
}

/// *Archived Nov 6, 14:20*.
pub(in crate::screens) fn archived_at(ctx: &Ctx, now: Timestamp, at: Timestamp) -> Arc<str> {
  ctx.t_args(
    "local_hub.line.archived",
    [("date", date(ctx, now, at).to_string()), ("time", display::clock(at))],
  )
}

/// How the entry stands, in the words of its kind.
fn state(ctx: &Ctx, entry: &Entry, whose: Whose<'_>) -> Said {
  let muted = |text: Arc<str>| Said {
    text,
    color: PaletteColor::TextMuted,
  };
  // Only an MCP server has no version at all; any other kind without one has only a draft.
  if entry.number.is_none() && entry.kind != ResourceKind::McpServer {
    return muted(ctx.t("local_hub.line.draft"));
  }
  match (entry.group, &entry.standing) {
    (Group::OffHere, _) => muted(off(ctx, entry)),
    (Group::NeedsYouHere, Some(standing)) => Said {
      text: needs(ctx, entry, standing),
      color: PaletteColor::Warning,
    },
    _ => match entry.kind {
      ResourceKind::McpServer => muted(on(ctx, entry)),
      ResourceKind::AgentProfile => muted(profile_line(ctx, entry, whose)),
      _ => muted(ctx.t("local_hub.line.ready")),
    },
  }
}

fn off(ctx: &Ctx, entry: &Entry) -> Arc<str> {
  if entry.used_by.is_empty() {
    ctx.t("local_hub.line.off")
  } else {
    ctx.t_args("local_hub.line.offRuns", [("names", entry.used_by.join(", "))])
  }
}

fn on(ctx: &Ctx, entry: &Entry) -> Arc<str> {
  if entry.used_by.is_empty() {
    ctx.t("local_hub.line.on")
  } else {
    ctx.t_args("local_hub.line.onUsed", [("names", entry.used_by.join(", "))])
  }
}

/// *Claude Code · Work*: the CLI and the account a profile runs on, by the account's name, never by the name of the
/// connection Orchester keeps for it.
fn profile_line(ctx: &Ctx, entry: &Entry, whose: Whose<'_>) -> Arc<str> {
  let on = entry.profile.as_ref().and_then(|profile| {
    profile
      .runs_on
      .iter()
      .find(|choice| Some(choice.id) == profile.current)
      .map(|choice| here::name(ctx, choice, whose).to_string())
      .or_else(|| profile.connection.clone())
  });
  match (entry.cli, on) {
    (Some(cli), Some(account)) => ctx.t_args(
      "local_hub.line.runsOn",
      [("cli", ctx.t(cli_key(cli)).to_string()), ("account", account)],
    ),
    (Some(cli), None) => ctx.t(cli_key(cli)),
    (None, _) => ctx.t("local_hub.line.ready"),
  }
}

/// What the person has to do: the one part, or how many.
pub(super) fn needs(ctx: &Ctx, entry: &Entry, standing: &ReadinessStanding) -> Arc<str> {
  match standing {
    ReadinessStanding::Missing(readiness) => match readiness.missing() {
      [only] => missing(ctx, entry.kind, only),
      many => ctx.t_args("local_hub.line.many", [("count", many.len().to_string())]),
    },
    ReadinessStanding::Undecided(Undecided::ConnectionGone(_)) if entry.cli.is_some() => connection_gone(ctx, entry),
    ReadinessStanding::Undecided(why) => undecided(ctx, why),
    ReadinessStanding::Ready => ctx.t("local_hub.line.ready"),
  }
}

/// A profile whose connection is gone: choose another of its CLI, or add one when the project has none.
fn connection_gone(ctx: &Ctx, entry: &Entry) -> Arc<str> {
  let cli = entry
    .cli
    .map_or_else(String::new, |cli| ctx.t(cli_key(cli)).to_string());
  let none = entry
    .profile
    .as_ref()
    .is_none_or(|profile| profile.connections.is_empty());
  ctx.t_args(
    if none {
      "local_hub.line.connectionNone"
    } else {
      "local_hub.line.connectionPick"
    },
    [("cli", cli)],
  )
}

/// Why it cannot be said to be ready: the archived resource it names, else that nobody can tell yet.
pub(in crate::screens) fn undecided(ctx: &Ctx, why: &Undecided) -> Arc<str> {
  match why {
    Undecided::RunsArchived(_, name) | Undecided::ChoosesArchivedLocalMcpServer(_, name) => {
      ctx.t_args("team_hub.line.namesArchivedLocal", [("name", name.clone())])
    }
    Undecided::Member { why, .. } => undecided(ctx, why),
    _ => ctx.t("local_hub.line.undecided"),
  }
}

/// One missing part, in the words of an entry of `kind`.
pub(in crate::screens) fn missing(ctx: &Ctx, kind: ResourceKind, part: &Missing) -> Arc<str> {
  match part {
    Missing::Connection { cli, problem, .. } => {
      let cli = ctx.t(cli_key(*cli));
      match problem {
        ConnectionProblem::NoneOfKind => ctx.t_args("local_hub.line.connectionNone", [("cli", &*cli)]),
        ConnectionProblem::PickNeeded => ctx.t_args("local_hub.line.connectionPick", [("cli", &*cli)]),
      }
    }
    Missing::ConnectionSignIn { cli, .. } => {
      ctx.t_args("local_hub.line.connectionSignIn", [("cli", &*ctx.t(cli_key(*cli)))])
    }
    Missing::LocalMcpProgram { .. } if kind == ResourceKind::McpServer => ctx.t("local_hub.line.programHere"),
    Missing::LocalMcpProgram { name, .. } => ctx.t_args("local_hub.line.programOf", [("server", name.to_string())]),
    Missing::LocalSecret { name, .. } | Missing::Secret { name } | Missing::SecretUnreadable { name } => {
      ctx.t_args("local_hub.line.secret", [("name", name.to_string())])
    }
    Missing::DecisionConnection { .. }
    | Missing::McpOff { .. }
    | Missing::McpUnsupported { .. }
    | Missing::McpProgram { .. } => ctx.t("local_hub.line.undecided"),
  }
}
