//! Connection enforcement beside a profile's page (`Connection enforcement` `Lj9fe` in 01c): the CLI and when it was
//! checked, then each setting with what enforces it and *Enforced* or *Not enforced*, the approval mode and the
//! repository's agent files too, and that the limits apply to new runs after publishing.
use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  connection::{AgentConnection, AgentProvider, CheckResult, invocation::Platform},
  permission::{
    AgentPermissions, ApprovalMode, AutopilotMechanism, AutopilotSupport, CantEnforce, RepositoryAccess,
    capability::{PermissionOption, Support, support},
  },
};
use orchester_workspace::agents::data::ProjectAgents;

use super::{enforcement, fields::Fields, words};
use crate::{
  screens::hub::parts::{boxed, group, head, note},
  theme::{ExtraSpacing, ExtraTypography, HubSpacing, HubText},
  ui::icon::Icon,
};

pub fn enforcement(ctx: &Ctx, fields: &Fields, listed: &ProjectAgents) -> Element {
  let title = ctx.t("agents.enforcementTitle");
  let Some(connection) = fields.connection(listed) else {
    return group(
      "agent-enforcement",
      head("agent-enforcement", &title, None, None),
      note(
        "agent-enforcement-none",
        Icon::Unplug,
        &ctx.t("agents.chooseConnection"),
      ),
    )
    .into();
  };
  let rows = rows(ctx, connection, &fields.permissions.get().0, listed);
  Column::new()
    .id("agent-enforcement")
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Sm)
    .child(group(
      "agent-enforcement-group",
      head(
        "agent-enforcement-group",
        &title,
        None,
        Some(
          Text::new(&checked(ctx, connection))
            .variant(HubText::Hint.role())
            .nowrap()
            .into(),
        ),
      ),
      boxed("agent-enforcement-rows", rows),
    ))
    .child(note(
      "agent-enforcement-note",
      Icon::Info,
      &ctx.t("agents.enforcementNotes"),
    ))
    .into()
}

/// Each setting with what enforces it, then MCP, Approvals and the repository's agent files.
fn rows(
  ctx: &Ctx,
  connection: &AgentConnection,
  permissions: &AgentPermissions,
  listed: &ProjectAgents,
) -> Vec<Element> {
  let mut rows = Vec::new();
  for (key, option) in [
    (
      "agents.repository",
      PermissionOption::of_repository(permissions.repository()),
    ),
    (
      "agents.folders",
      PermissionOption::of_repository(permissions.repository()),
    ),
    ("agents.commands", PermissionOption::of_commands(permissions.commands())),
    ("agents.network", PermissionOption::of_network(permissions.network())),
  ] {
    let (mechanism, enforced) = if key == "agents.folders" {
      folders(ctx, connection, permissions)
    } else {
      setting(ctx, connection, permissions, option)
    };
    rows.push(row(ctx, key, &ctx.t(key), &mechanism, State::Enforced(enforced)));
  }
  let available = !connection.provider().is_native();
  rows.push(row(
    ctx,
    "agents.mcp",
    &ctx.t("agents.mcp"),
    &ctx.t(if available {
      "agents.mcpLoading"
    } else {
      "agents.nativeUnavailable"
    }),
    State::Enforced(available),
  ));
  let (mode, enforced) = approval_mode(ctx, connection, permissions.approval_mode());
  rows.push(row(
    ctx,
    "agents.approvals",
    &ctx.t("agents.enforcement.approvalMode.title"),
    &mode,
    State::Enforced(enforced),
  ));
  let files = ctx.t(if listed.agent_files.is_on() {
    "agents.agentFilesOn"
  } else {
    "agents.agentFilesOff"
  });
  rows.push(row(
    ctx,
    "agents.agentFiles",
    &ctx.t("agents.agentFiles"),
    &ctx.t("agents.repositoryFilesMechanism"),
    State::Value(files),
  ));
  rows
}

/// The Approval mode row (07, 07b, 07d `J0MveH`): the profile's mode and how its connection's CLI hands requests to
/// Orchester in it, and whether it can. A CLI that can't hand them runs Autopilot as Manual, and says why
/// (`docs/design/approval-modes.md` › Enforcement); Manual starts as before on every CLI.
fn approval_mode(ctx: &Ctx, connection: &AgentConnection, mode: ApprovalMode) -> (Arc<str>, bool) {
  let cli = || {
    ctx.t(match connection.provider() {
      AgentProvider::CodexCli => "agents.enforcement.approvalMode.cli.codex",
      _ => "agents.enforcement.approvalMode.cli.claudeCode",
    })
  };
  let (how, enforced) = match (mode, connection.autopilot_support()) {
    (ApprovalMode::Manual, _) => (ctx.t("agents.enforcement.approvalMode.manual"), true),
    (ApprovalMode::Autopilot, AutopilotSupport::Enforced(AutopilotMechanism::Orchester)) => {
      (ctx.t("agents.enforcement.approvalMode.native"), true)
    }
    (ApprovalMode::Autopilot, AutopilotSupport::Enforced(_)) => (
      ctx.t_args("agents.enforcement.approvalMode.autopilot", [("cli", cli())]),
      true,
    ),
    (ApprovalMode::Autopilot, AutopilotSupport::CantEnforce(why)) => {
      let key = match why {
        CantEnforce::CliTooOld { .. } => "agents.enforcement.approvalMode.cantEnforce",
        CantEnforce::VersionUnknown { .. } => "agents.enforcement.approvalMode.versionUnknown",
      };
      (ctx.t_args(key, [("cli", cli())]), false)
    }
  };
  let line = ctx.t_args(
    "agents.enforcement.approvalMode.line",
    [("mode", super::permissions::mode_name(ctx, mode)), ("how", how)],
  );
  (line, enforced)
}

/// `Claude Code · 2.1.173 · checked 16:40`.
fn checked(ctx: &Ctx, connection: &AgentConnection) -> Arc<str> {
  let name = match connection.last_check().map(|check| &check.result) {
    Some(CheckResult::Available { version, .. }) => ctx
      .t_args(
        "agents.connectionVersion",
        [("name", connection.name().as_str()), ("version", version.as_str())],
      )
      .to_string(),
    _ => connection.name().to_string(),
  };
  let when = connection.last_check().map_or_else(
    || ctx.t("agents.notChecked"),
    |check| {
      ctx.t_args(
        "agents.checkedAt",
        [("time", orchester_workspace::display::clock(check.at))],
      )
    },
  );
  ctx.t_args(
    "agents.connectionState",
    [("provider", name), ("status", when.to_string())],
  )
}

fn setting(
  ctx: &Ctx,
  connection: &AgentConnection,
  permissions: &AgentPermissions,
  option: PermissionOption,
) -> (Arc<str>, bool) {
  let effective = permissions.enforced_on(connection.provider(), Platform::current());
  let support = effective.as_ref().map_or_else(
    |_| support(connection.provider(), Platform::current(), option),
    |effective| {
      if option == PermissionOption::NetworkOff && effective.network_stays_on().is_some() {
        support(connection.provider(), Platform::current(), option)
      } else {
        Support::Enforced(effective.mechanism(option.setting()))
      }
    },
  );
  match support {
    Support::Enforced(mechanism) => (enforcement::mechanism(ctx, mechanism, permissions), true),
    _ => (
      enforcement::unavailable(ctx, Some(connection.provider()), option),
      false,
    ),
  }
}

fn folders(ctx: &Ctx, connection: &AgentConnection, permissions: &AgentPermissions) -> (Arc<str>, bool) {
  let effective = permissions.enforced_on(connection.provider(), Platform::current());
  let mechanism = effective.as_ref().ok().and_then(|p| p.writable_folders());
  let detail = mechanism.map_or_else(
    || {
      ctx.t(if permissions.repository() == RepositoryAccess::ReadOnly {
        "agents.foldersReadOnly"
      } else {
        "agents.none"
      })
    },
    |mechanism| enforcement::mechanism(ctx, mechanism, permissions),
  );
  (detail, effective.is_ok())
}

/// What a row says at its end.
enum State {
  Enforced(bool),
  Value(Arc<str>),
}

/// A row named by `key` (its id), saying `label`, what enforces it and `state` at its end.
fn row(ctx: &Ctx, key: &str, label: &str, mechanism: &str, state: State) -> Element {
  let (end, color) = match state {
    State::Enforced(true) => (ctx.t("agents.enforcedState"), PaletteColor::Success),
    State::Enforced(false) => (ctx.t("agents.enforcementUnavailable"), PaletteColor::Warning),
    State::Value(value) => (value, PaletteColor::TextSecondary),
  };
  let name = key.trim_start_matches("agents.");
  Row::new()
    .id(format!("agent-enforcement-{name}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::RowY.role()))
    .spacing(lurq::app::theme::SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(label).variant(ExtraTypography::Strong.role()))
        .child(
          Text::new(mechanism)
            .variant(HubText::Hint.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .child(Text::new(&end).variant(HubText::Status.role()).color(color))
    .into()
}

/// The profile's permissions and whether its connection enforces them, for the page head.
pub fn summary(
  ctx: &Ctx,
  fields: &Fields,
  listed: &ProjectAgents,
) -> (Arc<str>, Option<(crate::screens::hub::parts::Tone, Arc<str>)>) {
  let permissions = fields.permissions.get().0;
  let connection = fields.connection(listed);
  let summary = words::summary(ctx, &permissions, connection.map(|c| c.provider()));
  (summary, connection.map(|c| enforcement::status(ctx, &permissions, c)))
}
