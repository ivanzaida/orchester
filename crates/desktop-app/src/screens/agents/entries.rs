//! A profile's row in the list (`Builder` `fYNhS` in `01 Profiles`): its CLI's mark, its name, one line with its
//! connection, model and effort and what it may do, how many pipelines use it and where its version stands; a profile
//! whose connection is gone says so in `warning` (06b).
use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};
use orchester_core::{
  connection::{AgentConnection, AgentProvider, invocation::Platform},
  permission::{AgentPermissions, NetworkAccess, RepositoryAccess},
};
use orchester_workspace::agents::{
  data::{ProfileEntry, ProjectAgents, VersionState},
  form::stored_effort,
};

use super::{AgentsNav, Shown, words};
use crate::{
  routes::AgentTab,
  screens::hub::parts::{Entry, Tone, quiet, status},
  ui::{icon::Icon, pressable::Press},
};

/// The glyph that stands for a connection's CLI or API in a tile.
pub(super) fn provider_glyph(connection: Option<&AgentConnection>) -> Icon {
  match connection {
    Some(connection) if connection.api_provider().is_some() => Icon::Cloud,
    Some(connection) => match connection.provider() {
      AgentProvider::ClaudeCodeCli => Icon::Sparkles,
      AgentProvider::CodexCli => Icon::SquareTerminal,
      AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => Icon::Cpu,
    },
    None => Icon::Bot,
  }
}

pub(super) fn row(ctx: &Ctx, entry: &ProfileEntry, listed: &ProjectAgents, nav: &AgentsNav) -> Element {
  let profile = &entry.profile;
  let connection = listed.connections.iter().find(|c| c.id() == profile.connection());
  let id = entry.id;
  let count = listed.usage.count(id);
  let used = if count == 0 {
    ctx.t("agents.notUsed")
  } else {
    ctx.t_args("agents.pipelineCount", [("count", count.to_string())])
  };
  let (tone, version) = version(ctx, entry.state);
  let entry_row = Entry::new(format!("agent-{id}"), profile.role().as_str())
    .tile(provider_glyph(connection))
    .end(quiet(&used).id(format!("agent-used-{id}")))
    .end(status(&format!("agent-version-{id}"), tone, &version))
    .press(Press::Open(nav.to(Shown::Edit(id, AgentTab::default()))));
  match connection {
    Some(connection) => entry_row.hint(line(ctx, profile, connection)).into(),
    None => entry_row
      .hint_in(ctx.t("agents.rowMissing"), PaletteColor::Warning)
      .into(),
  }
}

/// `Claude Code · Work · claude-opus-5-5 · effort High · Edits files`.
fn line(ctx: &Ctx, profile: &orchester_core::resource::AgentProfile, connection: &AgentConnection) -> Arc<str> {
  let model = match stored_effort(profile) {
    Some(effort) => ctx.t_args(
      "agents.modelEffort",
      [
        ("model", profile.model().as_str().to_owned()),
        ("effort", words::effort(ctx, &effort).to_string()),
      ],
    ),
    None => profile.model().as_str().into(),
  };
  ctx.t_args(
    "agents.rowLine",
    [
      ("connection", connection.name().as_str().to_owned()),
      ("model", model.to_string()),
      (
        "access",
        access(ctx, profile.permissions(), connection.provider()).to_string(),
      ),
    ],
  )
}

/// What the profile may do in a few words: `Edits files`, `Read-only · Network on · cannot block`.
pub(super) fn access(ctx: &Ctx, permissions: &AgentPermissions, provider: AgentProvider) -> Arc<str> {
  let repository = ctx.t(match permissions.repository() {
    RepositoryAccess::ReadOnly => "agents.readOnly",
    RepositoryAccess::Write => "agents.write",
  });
  let effective = permissions.effective_network(provider, Platform::current());
  if effective == NetworkAccess::Off {
    return repository;
  }
  ctx.t_args(
    "agents.accessNetwork",
    [
      ("repository", repository.to_string()),
      ("network", words::network(ctx, permissions, Some(provider)).to_string()),
    ],
  )
}

/// Where a profile's version stands, in the tone of its dot.
pub(super) fn version(ctx: &Ctx, state: VersionState) -> (Tone, Arc<str>) {
  let tone = match state {
    VersionState::Published(_) => Tone::Success,
    VersionState::Draft { published: None } => Tone::Muted,
    VersionState::Draft { .. } | VersionState::StaleDraft { .. } => Tone::Warning,
  };
  (tone, words::list_version(ctx, state))
}
