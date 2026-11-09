//! *Ready?* of a local profile (`Ready?` `uDnwU` in L1): the connection it runs on, each MCP server it chooses and each
//! secret those take, each ready or with what is missing and the fix at its end. The rows are the team profile's
//! ([`crate::screens::team_hub::profile::ready`]); only what stands behind them is a local project's: the connection
//! is the profile's own and a server's program and secret values are the server's, fixed in its own panel.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{connection::AgentProvider, id::ConnectionId, secret::SecretName, sharing::Missing};
use orchester_workspace::agents::{
  local_hub::{LocalHub, LocalProfile},
  team_profile::ConnectionChoice,
};

use crate::screens::team_hub::{
  profile::{
    fix::Offer,
    parts::{Part, State},
  },
  words::cli_key,
};

/// What a local profile's row offers at its end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Fix {
  /// Keep the connection the person chose as the profile's.
  UseConnection,
  /// Go where a connection of the CLI is added.
  AddConnection,
  /// Go where the connection the profile runs on is signed in.
  SignIn,
  /// Open the MCP server's panel, where its program path is set.
  SetPath(ConnectionId),
  /// Open the MCP server's panel, where the value of a secret it takes is set.
  AddValue(ConnectionId),
}

impl Offer for Fix {
  fn link_key(&self) -> &'static str {
    match self {
      Self::UseConnection => "team_hub.fix.link.use",
      Self::AddConnection => "team_hub.fix.link.add",
      Self::SignIn => "team_hub.fix.link.signIn",
      Self::SetPath(_) => "team_hub.fix.link.setPath",
      Self::AddValue(_) => "team_hub.fix.link.addValue",
    }
  }

  fn name(&self) -> &'static str {
    match self {
      Self::UseConnection => "use",
      Self::AddConnection => "add",
      Self::SignIn => "sign-in",
      Self::SetPath(_) => "set-path",
      Self::AddValue(_) => "add-value",
    }
  }
}

/// The connection, then each MCP server, then each secret of `profile`, as `hub` says they stand.
pub(super) fn parts(ctx: &Ctx, hub: &LocalHub, profile: &LocalProfile) -> Vec<Part<Fix>> {
  let mut parts = vec![connection(ctx, profile)];
  parts.extend(profile.mcp_servers.iter().map(|(id, name)| server(ctx, hub, *id, name)));
  parts.extend(profile.secrets.iter().map(|name| secret(ctx, hub, profile, name)));
  parts
}

fn connection(ctx: &Ctx, profile: &LocalProfile) -> Part<Fix> {
  let state = match (profile.current, profile.cli) {
    (Some(current), Some(cli)) => {
      let found = profile.connections.iter().find(|choice| choice.id == current);
      if found.is_some_and(|choice| choice.signed_out) {
        return Part {
          id: "connection".to_owned(),
          label: ctx.t("team_hub.ready.connection"),
          state: State::Missing {
            words: ctx.t_args("team_hub.ready.signedOut", [("cli", &*ctx.t(cli_key(cli)))]),
            fix: Some(Fix::SignIn),
            blocking: true,
          },
        };
      }
      State::Ready(found.map_or_else(
        || ctx.t("local_hub.ready.ready"),
        |choice| connection_words(ctx, cli, choice),
      ))
    }
    (None, Some(cli)) => gone(ctx, &ctx.t(cli_key(cli)), &profile.connections),
    (_, None) => State::Missing {
      words: ctx.t_args("local_hub.ready.noCli", [("model", profile.model.as_str())]),
      fix: None,
      blocking: true,
    },
  };
  Part {
    id: "connection".to_owned(),
    label: ctx.t("team_hub.ready.connection"),
    state,
  }
}

/// *Claude Code · Claude 2 · claude 2.1.173*: what runs the profile, as its last check found it.
fn connection_words(ctx: &Ctx, provider: AgentProvider, choice: &ConnectionChoice) -> Arc<str> {
  let cli = ctx.t(cli_key(provider));
  match (&choice.version, provider.program_name()) {
    (Some(version), Some(program)) => ctx.t_args(
      "local_hub.ready.connectionVersion",
      [
        ("cli", &*cli),
        ("name", &choice.name),
        ("program", program),
        ("version", version),
      ],
    ),
    _ => ctx.t_args("team_hub.ready.connectionOk", [("cli", &*cli), ("name", &choice.name)]),
  }
}

/// The connection the profile names is gone: none of its CLI to add, the only one to use, or several to choose from.
fn gone(ctx: &Ctx, cli: &str, connections: &[ConnectionChoice]) -> State<Fix> {
  match connections {
    [] => State::Missing {
      words: ctx.t_args("local_hub.ready.noneOfKind", [("cli", cli)]),
      fix: Some(Fix::AddConnection),
      blocking: true,
    },
    [only] => State::Missing {
      words: ctx.t_args("local_hub.ready.onlyOne", [("cli", cli), ("name", &only.name)]),
      fix: Some(Fix::UseConnection),
      blocking: false,
    },
    many => State::Missing {
      words: ctx.t_args(
        "local_hub.ready.pickAmong",
        [("count", many.len().to_string()), ("cli", cli.to_owned())],
      ),
      fix: Some(Fix::UseConnection),
      blocking: false,
    },
  }
}

fn server(ctx: &Ctx, hub: &LocalHub, id: ConnectionId, name: &str) -> Part<Fix> {
  let missing = hub.mcp_server(id).map_or(&[][..], |found| found.standing().missing());
  let state = match missing.first() {
    Some(Missing::LocalMcpProgram { .. }) => State::Missing {
      words: ctx.t("local_hub.line.programHere"),
      fix: Some(Fix::SetPath(id)),
      blocking: false,
    },
    Some(Missing::LocalSecret { name, .. }) => State::Missing {
      words: ctx.t_args("local_hub.line.secret", [("name", name.to_string())]),
      fix: Some(Fix::AddValue(id)),
      blocking: false,
    },
    _ if hub.mcp_server(id).is_some_and(|found| !found.server().enabled()) => {
      State::Ready(ctx.t("local_hub.ready.serverOff"))
    }
    _ => State::Ready(ctx.t("local_hub.ready.ready")),
  };
  Part {
    id: format!("mcp-{id}"),
    label: ctx.t_args("team_hub.ready.mcpServer", [("name", name)]),
    state,
  }
}

fn secret(ctx: &Ctx, hub: &LocalHub, profile: &LocalProfile, name: &SecretName) -> Part<Fix> {
  let lacking = profile.mcp_servers.iter().find(|(id, _)| {
    hub
      .mcp_server(*id)
      .is_some_and(|found| found.standing().missing().iter().any(|part| lacks(part, name)))
  });
  let state = match lacking {
    Some((id, _)) => State::Missing {
      words: ctx.t("local_hub.ready.noValue"),
      fix: Some(Fix::AddValue(*id)),
      blocking: false,
    },
    None => State::Ready(ctx.t("local_hub.ready.valueSet")),
  };
  Part {
    id: format!("secret-{name}"),
    label: ctx.t_args("team_hub.ready.secret", [("name", name.to_string())]),
    state,
  }
}

fn lacks(part: &Missing, name: &SecretName) -> bool {
  matches!(part, Missing::LocalSecret { name: lacking, .. } if lacking == name)
}

/// How many parts are missing.
pub(super) fn missing(parts: &[Part<Fix>]) -> usize {
  parts
    .iter()
    .filter(|part| matches!(part.state, State::Missing { .. }))
    .count()
}
