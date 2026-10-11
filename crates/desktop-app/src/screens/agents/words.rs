use std::sync::Arc;

use lurq::{app::ctx::Ctx, components::Text};
use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::RepositoryAccess,
};
use orchester_workspace::agents::data::VersionState;

use crate::theme::ExtraTypography;

pub fn note(text: &str) -> Text {
  Text::new(text).variant(ExtraTypography::Prose.role())
}

pub fn version(ctx: &Ctx, state: VersionState, dirty: bool) -> Arc<str> {
  let published = state.published().map_or(0, |v| v.get());
  if dirty {
    return if published == 0 {
      ctx.t("agents.unsavedNew")
    } else {
      ctx.t_args("agents.unsaved", [("version", published.to_string())])
    };
  }
  let key = match state {
    VersionState::Published(_) => "agents.published",
    VersionState::StaleDraft { based_on, .. } => {
      return ctx.t_args(
        "agents.stale",
        [
          ("base", based_on.map_or(0, |v| v.get()).to_string()),
          ("version", published.to_string()),
        ],
      );
    }
    VersionState::Draft { published: None } => "agents.notPublished",
    VersionState::Draft { .. } => "agents.saved",
  };
  ctx.t_args(key, [("version", published.to_string())])
}

pub fn state_note(ctx: &Ctx, state: VersionState, dirty: bool) -> Arc<str> {
  let key = if dirty {
    "agents.saveFirst"
  } else {
    match state {
      VersionState::StaleDraft { .. } => "agents.staleNote",
      VersionState::Published(_) => "agents.noChanges",
      VersionState::Draft { published: None } => "agents.firstNote",
      VersionState::Draft { .. } => "agents.versionNote",
    }
  };
  let version = state.published().map_or(0, |v| v.get());
  ctx.t_args(
    key,
    [
      ("version", version.to_string()),
      ("next", version.saturating_add(1).to_string()),
    ],
  )
}

pub fn list_version(ctx: &Ctx, state: VersionState) -> Arc<str> {
  match state {
    VersionState::Draft { published: None } => ctx.t("agents.firstDraft"),
    VersionState::Draft {
      published: Some(version),
    } => ctx.t_args(
      "agents.draft",
      [
        ("version", version.get().to_string()),
        ("next", version.get().saturating_add(1).to_string()),
      ],
    ),
    other => version(ctx, other, false),
  }
}

pub fn commands(ctx: &Ctx, permissions: &AgentPermissions) -> Arc<str> {
  match permissions.commands() {
    CommandPermission::AskBefore => ctx.t("agents.asksCommands"),
    CommandPermission::AnyInWorktree => ctx.t("agents.anyCommands"),
    CommandPermission::Allowed(list) => {
      ctx.t_args("agents.commandsCount", [("count", list.entries().len().to_string())])
    }
  }
}
pub fn network(ctx: &Ctx, permissions: &AgentPermissions, provider: Option<AgentProvider>) -> Arc<str> {
  let Some(provider) = provider else {
    return ctx.t("agents.networkUnknown");
  };
  let effective = permissions.effective_network(provider, Platform::current());
  ctx.t(if effective != permissions.network() {
    "agents.networkFallback"
  } else if effective == NetworkAccess::On {
    "agents.networkOn"
  } else {
    "agents.networkOff"
  })
}
pub fn summary(ctx: &Ctx, permissions: &AgentPermissions, provider: Option<AgentProvider>) -> Arc<str> {
  let repository = ctx.t(match permissions.repository() {
    RepositoryAccess::ReadOnly => "agents.readOnly",
    RepositoryAccess::Write => "agents.write",
  });
  let network = network(ctx, permissions, provider);
  ctx.t_args(
    "agents.summary",
    [
      ("repository", repository),
      ("commands", commands(ctx, permissions)),
      ("network", network),
    ],
  )
}
