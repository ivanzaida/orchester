//! Permissions tab dAvQg and unavailable options xepKp.
mod lists;
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentProvider, invocation::Platform, mcp::McpServerSelection},
  permission::{
    AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess,
    capability::{PermissionOption, support},
  },
};
use orchester_workspace::agents::{
  data::ProjectAgents,
  permissions::{option_detail, option_title},
};

use super::{
  fields::{Fields, Servers, label},
  words,
};
use crate::{
  project::ShownProject,
  theme::{ExtraSpacing, IconSize},
  ui::{
    form_section::{Place, section, side_row},
    icon::{Icon, icon},
    message::message_text,
    option_card::OptionCard,
    toggle::{Toggle, ToggleKind},
    unavailable::reason_line,
  },
};

#[derive(Clone)]
pub struct Props {
  pub fields: Fields,
  pub listed: ProjectAgents,
  pub project: ShownProject,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.listed == other.listed
      && self.project == other.project
      && self.fields.permissions.id() == other.fields.permissions.id()
  }
}
impl DevtoolsInspectable for Props {}
pub struct PermissionsForm;
impl Component for PermissionsForm {
  type Props = Props;
  fn create(_: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let fields = &props.fields;
    let provider = fields.connection(&props.listed).map(|c| c.provider());
    let lists = ctx.mount_keyed::<lists::Lists>("agent-permission-lists", props.clone());
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Roomy.role())
      .child(repository(ctx, fields, provider))
      .child(lists)
      .child(network(ctx, fields, provider))
      .child(mcp(ctx, &props, provider))
      .child(approvals(ctx, fields, provider))
  }
}
fn option(ctx: &Ctx, fields: &Fields, provider: Option<AgentProvider>, option: PermissionOption) -> Element {
  let permissions = fields.permissions.get().0;
  let selected = [
    PermissionOption::of_repository(permissions.repository()),
    PermissionOption::of_commands(permissions.commands()),
    PermissionOption::of_network(permissions.network()),
  ]
  .contains(&option);
  let value = fields.permissions.clone();
  let (glyph, title, detail) = if option == PermissionOption::ReadOnly {
    (Icon::Eye, ctx.t("agents.readOnly"), ctx.t("agents.readOnlyHint"))
  } else {
    (Icon::PencilLine, ctx.t("agents.writeTitle"), ctx.t("agents.writeHint"))
  };
  let card = OptionCard::new(
    format!("agent-option-{option:?}"),
    (glyph, title),
    detail,
    selected,
    move || {
      value.update(|p| match option {
        PermissionOption::ReadOnly => p.0.set_repository(RepositoryAccess::ReadOnly),
        PermissionOption::EditWorktree => p.0.set_repository(RepositoryAccess::Write),
        PermissionOption::AskBeforeCommands => p.0.set_commands(CommandPermission::AskBefore),
        PermissionOption::AllowedCommands => p.0.set_commands(CommandPermission::Allowed(AllowedCommands::default())),
        PermissionOption::AnyCommand => p.0.set_commands(CommandPermission::AnyInWorktree),
        PermissionOption::NetworkOff => p.0.set_network(NetworkAccess::Off),
        PermissionOption::NetworkOn => p.0.set_network(NetworkAccess::On),
      })
    },
  );
  if provider.is_some_and(|p| support(p, Platform::current(), option).is_enforced()) {
    card.into()
  } else {
    card
      .unavailable(super::enforcement::unavailable(ctx, provider, option))
      .into()
  }
}
fn command_option(ctx: &Ctx, fields: &Fields, provider: Option<AgentProvider>, option: PermissionOption) -> Element {
  let selected = PermissionOption::of_commands(fields.permissions.get().0.commands()) == option;
  let title = message_text(ctx, &orchester_workspace::messages::Message::new(option_title(option)));
  let detail = if option == PermissionOption::AllowedCommands
    && !provider.is_some_and(|p| support(p, Platform::current(), option).is_enforced())
  {
    ctx.t("agents.allowedHint")
  } else {
    message_text(
      ctx,
      &orchester_workspace::messages::Message::new(option_detail(option, provider)),
    )
  };
  let value = fields.permissions.clone();
  let available = provider.is_some_and(|p| support(p, Platform::current(), option).is_enforced());
  let toggle = Toggle::new(format!("agent-option-{option:?}"), ToggleKind::Radio, selected, title);
  let toggle = if available {
    toggle.on_change(move |_| {
      if PermissionOption::of_commands(value.get().0.commands()) == option {
        return;
      }
      value.update(|p| {
        p.0.set_commands(match option {
          PermissionOption::AllowedCommands => CommandPermission::Allowed(AllowedCommands::default()),
          PermissionOption::AnyCommand => CommandPermission::AnyInWorktree,
          _ => CommandPermission::AskBefore,
        })
      })
    })
  } else {
    toggle
  };
  Column::new()
    .spacing(SpacingSize::Xs)
    .child(toggle)
    .child(words::note(&detail))
    .with_children((!available).then(|| {
      reason_line(
        &format!("agent-option-{option:?}-reason"),
        &super::enforcement::unavailable(ctx, provider, option),
      )
    }))
    .into()
}
fn repository(ctx: &Ctx, fields: &Fields, provider: Option<AgentProvider>) -> Element {
  section(
    "agent-repository",
    &ctx.t("agents.repository"),
    None,
    vec![
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .child(
          Column::new()
            .flex(1.0)
            .child(option(ctx, fields, provider, PermissionOption::ReadOnly)),
        )
        .child(
          Column::new()
            .flex(1.0)
            .child(option(ctx, fields, provider, PermissionOption::EditWorktree)),
        )
        .into(),
    ],
  )
  .into()
}
fn network(ctx: &Ctx, fields: &Fields, provider: Option<AgentProvider>) -> Element {
  let on =
    provider.is_some_and(|p| fields.permissions.get().0.effective_network(p, Platform::current()) == NetworkAccess::On);
  let value = fields.permissions.clone();
  let target = if on {
    PermissionOption::NetworkOff
  } else {
    PermissionOption::NetworkOn
  };
  let available = provider.is_some_and(|p| support(p, Platform::current(), target).is_enforced());
  let toggle = Toggle::new(
    "agent-network",
    ToggleKind::Switch,
    on,
    ctx.t(if on { "agents.on" } else { "agents.off" }),
  )
  .hide_label();
  let toggle = if available {
    toggle
      .on_change(move |on| value.update(|p| p.0.set_network(if on { NetworkAccess::On } else { NetworkAccess::Off })))
  } else {
    toggle
  };
  section(
    "agent-network-section",
    &ctx.t("agents.network"),
    None,
    vec![
      side_row(
        label(ctx, "agent-network", "agents.networkAccess", "agents.networkHint"),
        Row::new()
          .spacing(SpacingSize::Sm)
          .child(words::note(&ctx.t(if on { "agents.on" } else { "agents.off" })))
          .child(toggle),
        Place::Last,
        false,
      ),
      Column::new()
        .with_children((!available).then(|| {
          reason_line(
            "agent-network-reason",
            &super::enforcement::unavailable(ctx, provider, target),
          )
        }))
        .into(),
    ],
  )
  .into()
}
fn approvals(ctx: &Ctx, fields: &Fields, provider: Option<AgentProvider>) -> Element {
  let available = provider.is_some_and(|p| !p.is_native());
  let mut rows = Vec::new();
  for auto in [true, false] {
    let (id, title, hint, key) = if auto {
      ("agent-auto", "agents.whenAsked", "agents.whenAskedHint", "agents.auto")
    } else {
      (
        "agent-ask-tools",
        "agents.toolCalls",
        "agents.toolCallsHint",
        "agents.askTools",
      )
    };
    let permissions = fields.permissions.get().0;
    let on = if auto {
      permissions.auto_approve()
    } else {
      permissions.interactive_tool_approvals()
    };
    let value = fields.permissions.clone();
    let toggle = Toggle::new(id, ToggleKind::Checkbox, on, ctx.t(key));
    let toggle = if available {
      toggle.on_change(move |on| {
        value.update(|p| {
          if auto {
            p.0.set_auto_approve(on)
          } else {
            p.0.set_interactive_tool_approvals(on)
          }
        })
      })
    } else {
      toggle
    };
    rows.push(side_row(label(ctx, id, title, hint), toggle, Place::Within, false));
  }
  if !available {
    rows.push(reason_line("agent-approvals-unavailable", &ctx.t("agents.nativeUnavailable")).into());
  }
  section("agent-approvals", &ctx.t("agents.approvals"), None, rows).into()
}
fn mcp(ctx: &mut Ctx, props: &Props, provider: Option<AgentProvider>) -> Element {
  let (fields, listed) = (&props.fields, &props.listed);
  let available = provider.is_some_and(|p| !p.is_native());
  let selected = fields.servers.get().0;
  let mut rows = Vec::new();
  for server in listed.mcp.iter() {
    let id = server.id();
    let signal = fields.servers.clone();
    let on = selected.ids().contains(&id);
    let toggle = Toggle::new(
      format!("agent-mcp-{id}"),
      ToggleKind::Switch,
      on,
      server.name().as_str(),
    )
    .hide_label();
    let toggle = if available && (on || (server.enabled() && selected.ids().len() < McpServerSelection::MAX)) {
      toggle.on_change(move |on| {
        let mut ids = signal.get().0.ids().to_vec();
        ids.retain(|kept| *kept != id);
        if on {
          ids.push(id);
        }
        // The list comes from a validated selection and unique project server ids; respect its public limit.
        if let Ok(selected) = McpServerSelection::new(ids) {
          signal.set(Servers(selected));
        }
      })
    } else {
      toggle
    };
    rows.push(mcp_row(ctx, server, toggle));
  }
  if rows.is_empty() {
    rows.push(words::note(&ctx.t("agents.mcpNone")).into());
  }
  if !available {
    rows.push(reason_line("agent-mcp-unavailable", &ctx.t("agents.nativeUnavailable")).into());
  }
  if selected.ids().len() >= McpServerSelection::MAX {
    rows.push(words::note(&ctx.t("agents.mcpLimit")).into());
  }
  let nav = crate::navigation::Navigation::of(ctx);
  let project = props.project.open.project.id();
  if listed.mcp.is_empty() {
    rows.push(
      crate::ui::button::ActionButton::new(
        "agent-add-mcp",
        crate::ui::button::ButtonKind::Ghost,
        ctx.t("agents.addMcp"),
      )
      .on_click(move || {
        nav.go(crate::routes::Route::Project(
          project,
          crate::routes::ProjectPage::NewConnection(crate::routes::NewConnection::McpServer),
        ))
      })
      .into(),
    );
  }
  let count = ctx.t_args(
    "agents.mcpSelection",
    [
      ("selected", selected.ids().len().to_string()),
      ("total", listed.mcp.len().to_string()),
    ],
  );
  section("agent-mcp", &ctx.t("agents.mcp"), Some(&count), rows).into()
}
fn mcp_row(ctx: &Ctx, server: &orchester_core::connection::mcp::McpServer, toggle: Toggle) -> Element {
  let (glyph, detail) = match server.transport() {
    orchester_core::connection::mcp::McpTransport::Stdio(stdio) => (
      Icon::Server,
      ctx.t_args("agents.mcpStdio", [("program", stdio.program().as_str())]),
    ),
    orchester_core::connection::mcp::McpTransport::Http(http) => {
      (Icon::Globe, ctx.t_args("agents.mcpHttp", [("host", http.url().host())]))
    }
  };

  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(lurq::node::padding::Padding::vertical(SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .child(icon(glyph, IconSize::Md, PaletteColor::TextMuted))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(server.name().as_str()).variant(TypographyStyle::Label))
        .child(words::note(&detail))
        .with_children((!server.enabled()).then(|| words::note(&ctx.t("agents.mcpDisabled")))),
    )
    .child(toggle)
    .into()
}
