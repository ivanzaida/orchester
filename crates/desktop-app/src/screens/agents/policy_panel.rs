//! Enforcement card VsylR: each setting carries its actual mechanism and status.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  connection::{AgentConnection, CheckResult, invocation::Platform},
  permission::{
    AgentPermissions, RepositoryAccess,
    capability::{PermissionOption, Support, support},
  },
};
use orchester_workspace::agents::data::ProjectAgents;

use super::{fields::Fields, panels, words};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

pub fn enforcement(ctx: &Ctx, fields: &Fields, listed: &ProjectAgents) -> Element {
  let Some(connection) = fields.connection(listed) else {
    return panels::panel("agent-enforcement")
      .child(panels::body().child(words::note(&ctx.t("agents.chooseConnection"))))
      .into();
  };
  let permissions = fields.permissions.get().0;
  let mut rows = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(ExtraSpacing::Inset.role()));
  for (key, option, value) in [
    (
      "agents.repository",
      PermissionOption::of_repository(permissions.repository()),
      ctx.t(if permissions.repository() == RepositoryAccess::ReadOnly {
        "agents.readOnly"
      } else {
        "agents.writeTitle"
      }),
    ),
    (
      "agents.commands",
      PermissionOption::of_commands(permissions.commands()),
      words::commands(ctx, &permissions),
    ),
    (
      "agents.network",
      PermissionOption::of_network(permissions.network()),
      words::network(ctx, &permissions, Some(connection.provider())),
    ),
  ] {
    rows = rows.child(permission_row(ctx, connection, &permissions, key, option, &value));
    if key == "agents.repository" {
      rows = rows.child(folders(ctx, connection, &permissions));
    }
  }
  rows = connection_settings(ctx, fields, listed, rows);
  let notes = panels::body()
    .spacing(SpacingSize::Sm)
    .child(note(Icon::Server, &ctx.t("agents.mcpHint")))
    .child(note(Icon::Info, &ctx.t("agents.localRules")))
    .child(note(Icon::ShieldCheck, &ctx.t("agents.enforcementNote")));
  panels::panel("agent-enforcement")
    .child(header(ctx, connection, &permissions))
    .child(rows)
    .child(notes)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(
          ExtraSpacing::Inset.role(),
          ExtraSpacing::Loose.role(),
        ))
        .background(PaletteColor::SurfaceRaised)
        .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(note(Icon::History, &ctx.t("agents.newRuns"))),
    )
    .into()
}
fn connection_detail(connection: &AgentConnection) -> String {
  match connection.last_check().map(|check| &check.result) {
    Some(CheckResult::Available { version, .. }) => format!("{} · {version}", connection.name()),
    _ => connection.name().to_string(),
  }
}
fn permission_row(
  ctx: &Ctx,
  connection: &AgentConnection,
  permissions: &AgentPermissions,
  key: &str,
  option: PermissionOption,
  value: &str,
) -> Element {
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
  let mechanism = match support {
    Support::Enforced(mechanism) => super::enforcement::mechanism(ctx, mechanism, permissions),
    _ => super::enforcement::unavailable(ctx, Some(connection.provider()), option),
  };
  setting(ctx, key, value, &mechanism, support.is_enforced())
}
fn folders(ctx: &Ctx, connection: &AgentConnection, permissions: &AgentPermissions) -> Element {
  let count = permissions
    .granted_folders()
    .map_or(0, |folders| folders.entries().len());
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
    |mechanism| super::enforcement::mechanism(ctx, mechanism, permissions),
  );
  setting(
    ctx,
    "agents.folders",
    &ctx.t_args("agents.foldersCount", [("count", count.to_string())]),
    &detail,
    effective.is_ok(),
  )
}
fn setting(ctx: &Ctx, key: &str, value: &str, mechanism: &str, enforced: bool) -> Element {
  let color = if enforced {
    PaletteColor::Success
  } else {
    PaletteColor::Warning
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Fine.role())
    .padding(Padding::vertical(ExtraSpacing::Loose.role()))
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Description).flex(1.0))
        .child(icon(
          if enforced { Icon::Check } else { Icon::CircleAlert },
          IconSize::Sm,
          color.clone(),
        ))
        .child(
          Text::new(&ctx.t(if enforced {
            "agents.enforcedState"
          } else {
            "agents.enforcementUnavailable"
          }))
          .variant(ExtraTypography::Note.role())
          .color(color),
        ),
    )
    .child(words::note(value).color(PaletteColor::TextPrimary))
    .child(Text::new(mechanism).variant(ExtraTypography::Note.role()))
    .into()
}
fn note(glyph: Icon, text: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
    .child(words::note(text).flex(1.0))
}
fn connection_settings(ctx: &Ctx, fields: &Fields, listed: &ProjectAgents, mut rows: Column) -> Column {
  let available = fields
    .connection(listed)
    .is_some_and(|connection| !connection.provider().is_native());
  let permissions = fields.permissions.get().0;
  rows = rows.child(setting(
    ctx,
    "agents.mcp",
    &ctx.t_args(
      "agents.mcpCount",
      [
        ("selected", fields.servers.get().0.ids().len().to_string()),
        ("total", listed.mcp.len().to_string()),
      ],
    ),
    &ctx.t(if available {
      "agents.mcpLoading"
    } else {
      "agents.nativeUnavailable"
    }),
    available,
  ));
  rows = rows.child(setting(
    ctx,
    "agents.approvals",
    &ctx.t(if permissions.auto_approve() {
      "agents.approvalsAuto"
    } else if permissions.interactive_tool_approvals() {
      "agents.approvalsAsk"
    } else {
      "agents.approvalsDefault"
    }),
    &ctx.t(if available {
      "agents.approvalMechanism"
    } else {
      "agents.nativeUnavailable"
    }),
    available,
  ));
  rows = rows.child(setting(
    ctx,
    "agents.agentFiles",
    &ctx.t(if listed.agent_files.is_on() {
      "agents.agentFilesOn"
    } else {
      "agents.agentFilesOff"
    }),
    &ctx.t("agents.repositoryFilesMechanism"),
    true,
  ));
  rows
}
fn header(ctx: &Ctx, connection: &AgentConnection, permissions: &AgentPermissions) -> Column {
  let enforced = permissions
    .enforced_on(connection.provider(), Platform::current())
    .is_ok_and(|effective| effective.network_stays_on().is_none())
    && connection.provider().runs_agents();
  let title = if enforced {
    ctx.t_args("agents.enforced", [("provider", connection.name().as_str())])
  } else {
    ctx.t("agents.enforcementTitle")
  };
  let checked = connection.last_check().map_or_else(
    || ctx.t("agents.notChecked"),
    |check| {
      ctx.t_args(
        "agents.checkedAt",
        [("time", orchester_workspace::display::clock(check.at))],
      )
    },
  );
  panels::body()
    .spacing(ExtraSpacing::Snug.role())
    .background(PaletteColor::SurfaceRaised)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(
          if enforced { Icon::ShieldCheck } else { Icon::ShieldAlert },
          IconSize::Md,
          if enforced {
            PaletteColor::Success
          } else {
            PaletteColor::Warning
          },
        ))
        .child(Text::new(&title).variant(ExtraTypography::Strong.role())),
    )
    .child(
      Text::new(&ctx.t_args(
        "agents.connectionState",
        [
          ("provider", connection_detail(connection)),
          ("status", checked.to_string()),
        ],
      ))
      .variant(TypographyStyle::Caption),
    )
}
