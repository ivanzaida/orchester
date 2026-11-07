//! The parts of *Ready here?* (`Ready here?` `cLHSH` in `Agents · Profile` `NDZXC`): the connection, each MCP server
//! the profile uses and each secret it needs, each ready or with what is missing and its fix.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  id::ResourceId,
  secret::SecretName,
  sharing::{ConnectionProblem, Missing, ProgramProblem},
};
use orchester_workspace::{
  agents::{
    team_hub::Form,
    team_profile::{TeamProfileSheet, UsedServer},
  },
  readiness::Group,
};

use super::fix::Fix;
use crate::screens::team_hub::{
  model::{Entry, Listing},
  words::cli_key,
};

/// How one part stands.
pub(super) enum State {
  Ready(Arc<str>),
  Missing {
    words: Arc<str>,
    fix: Option<Fix>,
    /// Nothing can stand in for it: the glyph is `danger`, not `warning`.
    blocking: bool,
  },
}

pub(super) struct Part {
  /// What the part's controls are called in element ids.
  pub id: String,
  pub label: Arc<str>,
  pub state: State,
}

/// The connection, then each MCP server, then each secret of the profile, as `entry`'s standing says they are.
pub(super) fn parts(ctx: &Ctx, listing: &Listing, entry: &Entry, sheet: &TeamProfileSheet) -> Vec<Part> {
  let missing = entry.standing.as_ref().map_or(&[][..], |standing| standing.missing());
  let mut parts = vec![connection(ctx, sheet, missing)];
  parts.extend(sheet.mcp_servers.iter().map(|used| server(ctx, listing, used, missing)));
  parts.extend(sheet.secrets.iter().map(|name| secret(ctx, name, missing)));
  parts
}

fn connection(ctx: &Ctx, sheet: &TeamProfileSheet, missing: &[Missing]) -> Part {
  let cli = ctx.t(cli_key(sheet.cli));
  let state = match missing.iter().find_map(|part| match part {
    Missing::Connection { problem, .. } => Some(*problem),
    _ => None,
  }) {
    Some(ConnectionProblem::NoneOfKind) => State::Missing {
      words: ctx.t_args("team_hub.ready.noneOfKind", [("cli", &*cli)]),
      fix: Some(Fix::AddConnection),
      blocking: true,
    },
    Some(ConnectionProblem::PickNeeded) => State::Missing {
      words: ctx.t_args("team_hub.ready.pickNeeded", [("cli", &*cli)]),
      fix: Some(Fix::UseConnection),
      blocking: false,
    },
    None => State::Ready(connection_words(ctx, sheet, &cli)),
  };
  Part {
    id: "connection".to_owned(),
    label: ctx.t("team_hub.ready.connection"),
    state,
  }
}

/// *Claude Code · Claude 2 · claude 2.1.173*: what runs the profile, as its last check found it.
fn connection_words(ctx: &Ctx, sheet: &TeamProfileSheet, cli: &str) -> Arc<str> {
  let Some(runs_on) = sheet.runs_on() else {
    return ctx.t("team_hub.ready.ready");
  };
  match (&runs_on.version, sheet.cli.program_name()) {
    (Some(version), Some(program)) => ctx.t_args(
      "team_hub.ready.connectionVersion",
      [
        ("cli", cli),
        ("name", &runs_on.name),
        ("program", program),
        ("version", version),
      ],
    ),
    _ => ctx.t_args("team_hub.ready.connectionOk", [("cli", cli), ("name", &runs_on.name)]),
  }
}

fn server(ctx: &Ctx, listing: &Listing, used: &UsedServer, missing: &[Missing]) -> Part {
  let id = used.server.id;
  let problem = missing.iter().find(|part| of_server(part, id)).or_else(|| {
    missing
      .iter()
      .find(|part| secret_of(part).is_some_and(|name| used.secrets.contains(name)))
  });
  let state = match problem {
    Some(part) => problem_state(ctx, listing, id, part),
    None => State::Ready(server_ready(ctx, listing.find(id).and_then(|found| found.form), used)),
  };
  Part {
    id: format!("mcp-{id}"),
    label: ctx.t_args("team_hub.ready.mcpServer", [("name", used.server.name.as_str())]),
    state,
  }
}

fn of_server(part: &Missing, id: ResourceId) -> bool {
  match part {
    Missing::McpOff { server } | Missing::McpUnsupported { server, .. } | Missing::McpProgram { server, .. } => {
      *server == id
    }
    _ => false,
  }
}

const fn secret_of(part: &Missing) -> Option<&SecretName> {
  match part {
    Missing::Secret { name } | Missing::SecretUnreadable { name } => Some(name),
    _ => None,
  }
}

/// A missing part of a server or a secret, in words that fit its row.
fn problem_state(ctx: &Ctx, listing: &Listing, server: ResourceId, part: &Missing) -> State {
  let words = match part {
    Missing::McpOff { .. } if listing.find(server).is_some_and(|found| found.group == Group::OffHere) => {
      ctx.t("team_hub.ready.switchedOff")
    }
    Missing::McpOff { .. } => ctx.t("team_hub.line.mcpUndecided"),
    Missing::McpProgram {
      problem: ProgramProblem::NotSet,
      ..
    } => ctx.t("team_hub.line.programHere"),
    Missing::McpProgram {
      problem: ProgramProblem::FileGone(_),
      ..
    } => ctx.t("team_hub.ready.programGone"),
    Missing::McpUnsupported { .. } => ctx.t("team_hub.line.unsupportedHere"),
    Missing::Secret { name } => ctx.t_args("team_hub.line.secret", [("name", name.to_string())]),
    Missing::SecretUnreadable { name } => ctx.t_args("team_hub.line.secretUnreadable", [("name", name.to_string())]),
    Missing::Connection { .. } | Missing::LocalMcpProgram { .. } | Missing::LocalSecret { .. } => {
      ctx.t("team_hub.ready.ready")
    }
  };
  State::Missing {
    words,
    fix: Fix::of(part),
    blocking: false,
  }
}

fn server_ready(ctx: &Ctx, form: Option<Form>, used: &UsedServer) -> Arc<str> {
  match (form, used.secrets.as_slice()) {
    (Some(Form::McpProgram), _) => ctx.t("team_hub.ready.programSet"),
    (Some(Form::McpRemote), []) => ctx.t("team_hub.ready.remote"),
    (Some(Form::McpRemote), [one]) => ctx.t_args("team_hub.ready.remoteSecret", [("names", one.to_string())]),
    (Some(Form::McpRemote), many) => ctx.t_args(
      "team_hub.ready.remoteSecrets",
      [(
        "names",
        many.iter().map(ToString::to_string).collect::<Vec<_>>().join(", "),
      )],
    ),
    _ => ctx.t("team_hub.ready.ready"),
  }
}

fn secret(ctx: &Ctx, name: &SecretName, missing: &[Missing]) -> Part {
  let problem = missing.iter().find(|part| secret_of(part) == Some(name));
  let state = match problem {
    Some(part) => State::Missing {
      words: ctx.t("team_hub.ready.noValue"),
      fix: Fix::of(part),
      blocking: false,
    },
    None => State::Ready(ctx.t("team_hub.ready.valueSet")),
  };
  Part {
    id: format!("secret-{name}"),
    label: ctx.t_args("team_hub.ready.secret", [("name", name.to_string())]),
    state,
  }
}
