//! A profile page's permissions (`01c · Profile · permissions` `WAWHr`, 07 `J1ixkr`): Repository access with its extra
//! writable folders, Approvals with Manual's allowed commands, Network, and MCP servers as switches. An option the
//! connection cannot enforce here cannot be chosen and says why; a widening change is asked about inside the section
//! that widens it.
mod approvals;
mod commands;
mod folders;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  connection::{AgentProvider, invocation::Platform, mcp::McpServerSelection},
  permission::{
    AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess,
    capability::{PermissionOption, support},
  },
};
use orchester_workspace::{
  agents::{
    data::ProjectAgents,
    permissions::{option_detail, option_title},
  },
  messages::Message,
};

pub(super) use self::{approvals::mode_name, commands::CommandList, folders::FolderList};
use super::fields::{Fields, Servers};
use crate::{
  project::ShownProject,
  screens::hub::parts::{boxed, field, group, head, note},
  theme::{ExtraSpacing, ExtraTypography, HubSpacing, HubText},
  ui::{
    icon::Icon,
    message::message_text,
    toggle::{Toggle, ToggleKind},
    unavailable::reason_line,
  },
};

/// Where a widening question opens: the section of the field that widens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Section {
  Connection,
  Repository,
  Network,
  Mcp,
  Approvals,
}

impl Section {
  /// The section a widening reason is about; the allowed commands are Manual's, in Approvals.
  pub(super) fn of(reason: &Message) -> Self {
    match reason.key {
      "appMcp.widens.repository" | "appMcp.widens.writableFolder" => Self::Repository,
      "appMcp.widens.network" => Self::Network,
      "appMcp.widens.mcpServers" => Self::Mcp,
      "appMcp.widens.commands"
      | "appMcp.widens.toolCallsUnasked"
      | "appMcp.widens.autoApprove"
      | "appMcp.widens.autopilot" => Self::Approvals,
      _ => Self::Connection,
    }
  }
}

/// What the permission sections read.
#[derive(Clone)]
pub(super) struct Context<'a> {
  pub fields: &'a Fields,
  pub listed: &'a ProjectAgents,
  pub project: &'a ShownProject,
}

/// The four sections, Approvals right after Repository access (07); `asked` opens in the one it names.
pub(super) fn sections(ctx: &mut Ctx, context: &Context<'_>, asked: Option<(Section, Element)>) -> Vec<Element> {
  let provider = context.fields.connection(context.listed).map(|c| c.provider());
  let (section, mut question) = match asked {
    Some((section, question)) => (Some(section), Some(question)),
    None => (None, None),
  };
  let mut take = |here: Section| if section == Some(here) { question.take() } else { None };
  let repository = repository(ctx, context, provider, take(Section::Repository));
  let approvals = approvals::group(ctx, context, provider, take(Section::Approvals));
  let network = network_section(ctx, context.fields, provider, take(Section::Network));
  let mcp = mcp(ctx, context, provider, take(Section::Mcp));
  vec![repository, approvals, network, mcp]
}

fn section(id: &str, (title, count): (&str, Option<&str>), rows: Vec<Element>) -> Element {
  group(id, head(id, title, count, None), boxed(&format!("{id}-rows"), rows)).into()
}

/// A choice of a permission as a radio row: its title with what it does under it, or why the connection cannot
/// enforce it here.
pub(super) fn option_row(
  ctx: &Ctx,
  fields: &Fields,
  provider: Option<AgentProvider>,
  option: PermissionOption,
) -> Element {
  let permissions = fields.permissions.get().0;
  let selected = [
    PermissionOption::of_repository(permissions.repository()),
    PermissionOption::of_commands(permissions.commands()),
  ]
  .contains(&option);
  let available = provider.is_some_and(|p| support(p, Platform::current(), option).is_enforced());
  let title = match option {
    PermissionOption::ReadOnly => ctx.t("agents.readOnly"),
    PermissionOption::EditWorktree => ctx.t("agents.writeTitle"),
    _ => message_text(ctx, &Message::new(option_title(option))),
  };
  let detail = match option {
    PermissionOption::ReadOnly => ctx.t("agents.readOnlyHint"),
    PermissionOption::EditWorktree => ctx.t("agents.writeHint"),
    _ => message_text(ctx, &Message::new(option_detail(option, provider))),
  };
  let value = fields.permissions.clone();
  let toggle = Toggle::new(
    format!("agent-option-{option:?}"),
    ToggleKind::Radio,
    selected,
    title.clone(),
  )
  .hide_label();
  let toggle = if available {
    toggle.on_change(move |_| {
      value.update(|p| match option {
        PermissionOption::ReadOnly => p.0.set_repository(RepositoryAccess::ReadOnly),
        PermissionOption::EditWorktree => p.0.set_repository(RepositoryAccess::Write),
        PermissionOption::AskBeforeCommands => p.0.set_commands(CommandPermission::AskBefore),
        PermissionOption::AllowedCommands => {
          if !matches!(p.0.commands(), CommandPermission::Allowed(_)) {
            p.0.set_commands(CommandPermission::Allowed(AllowedCommands::default()));
          }
        }
        PermissionOption::AnyCommand => p.0.set_commands(CommandPermission::AnyInWorktree),
        PermissionOption::NetworkOff => p.0.set_network(NetworkAccess::Off),
        PermissionOption::NetworkOn => p.0.set_network(NetworkAccess::On),
        // No row offers a host list: choosing one needs its hosts, which this page does not edit.
        PermissionOption::NetworkHosts => {}
      })
    })
  } else {
    toggle
  };
  Row::new()
    .id(format!("agent-option-{option:?}-row"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(toggle)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(&title).variant(ExtraTypography::Strong.role()))
        .child(
          Text::new(&detail)
            .variant(HubText::Hint.role())
            .width(Dimension::Pct(100.0)),
        )
        .with_children((!available).then(|| {
          reason_line(
            &format!("agent-option-{option:?}-reason"),
            &super::enforcement::unavailable(ctx, provider, option),
          )
        })),
    )
    .into()
}

fn repository(
  ctx: &mut Ctx,
  context: &Context<'_>,
  provider: Option<AgentProvider>,
  asked: Option<Element>,
) -> Element {
  let folders = ctx.mount_keyed::<FolderList>("agent-folder-list", folders::props(context));
  let mut rows = vec![
    option_row(ctx, context.fields, provider, PermissionOption::ReadOnly),
    option_row(ctx, context.fields, provider, PermissionOption::EditWorktree),
    folders,
  ];
  rows.extend(asked);
  section("agent-repository", (&ctx.t("agents.repository"), None), rows)
}

/// *Network* (07 `I9OrmW`): the commands moved to Approvals.
fn network_section(ctx: &Ctx, fields: &Fields, provider: Option<AgentProvider>, asked: Option<Element>) -> Element {
  let mut rows = vec![network(ctx, fields, provider)];
  rows.extend(asked);
  section("agent-network-section", (&ctx.t("agents.network"), None), rows)
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
  let state = ctx.t(if on { "agents.on" } else { "agents.off" });
  let toggle = Toggle::new("agent-network", ToggleKind::Switch, on, state.clone()).hide_label();
  let toggle = if available {
    toggle
      .on_change(move |on| value.update(|p| p.0.set_network(if on { NetworkAccess::On } else { NetworkAccess::Off })))
  } else {
    toggle
  };
  let control = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&state).variant(HubText::Status.role()))
    .child(toggle);
  let row = field(
    "agent-network",
    &ctx.t("agents.networkAccess"),
    Some(&ctx.t("agents.networkHint")),
    control,
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(row)
    .with_children((!available).then(|| {
      Row::new()
        .padding(
          Padding::new()
            .left(HubSpacing::Row.role())
            .bottom(HubSpacing::FieldY.role()),
        )
        .child(reason_line(
          "agent-network-reason",
          &super::enforcement::unavailable(ctx, provider, target),
        ))
    }))
    .into()
}

/// A section the connection offers nothing of (01e, a native connection).
fn unavailable_section(ctx: &Ctx, id: &str, title: &str) -> Element {
  let row = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
    .spacing(ExtraSpacing::Tight.role())
    .child(Text::new(&ctx.t("agents.unavailable")).variant(ExtraTypography::Strong.role()))
    .child(Text::new(&ctx.t("agents.nativeUnavailable")).variant(HubText::Hint.role()));
  section(id, (title, None), vec![row.into()])
}

fn mcp(ctx: &mut Ctx, context: &Context<'_>, provider: Option<AgentProvider>, asked: Option<Element>) -> Element {
  let (fields, listed) = (context.fields, context.listed);
  if provider.is_none_or(|p| p.is_native()) {
    return unavailable_section(ctx, "agent-mcp", &ctx.t("agents.mcp"));
  }
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
    let toggle = if on || (server.enabled() && selected.ids().len() < McpServerSelection::MAX) {
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
    let detail = server_detail(ctx, server);
    rows.push(
      field(
        &format!("agent-mcp-{id}-row"),
        server.name().as_str(),
        Some(&detail),
        toggle,
      )
      .into(),
    );
  }
  if rows.is_empty() {
    rows.push(empty_mcp(ctx, context));
  }
  rows.extend(asked);
  let count = ctx.t_args(
    "agents.mcpCount",
    [
      ("selected", selected.ids().len().to_string()),
      ("total", listed.mcp.len().to_string()),
    ],
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(section("agent-mcp", (&ctx.t("agents.mcp"), Some(&count)), rows))
    .with_children(
      (selected.ids().len() >= McpServerSelection::MAX)
        .then(|| note("agent-mcp-limit", Icon::Info, &ctx.t("agents.mcpLimit"))),
    )
    .child(note("agent-mcp-note", Icon::Info, &ctx.t("agents.mcpNote")))
    .into()
}

/// No MCP server in the project yet, with Add an MCP server.
fn empty_mcp(ctx: &mut Ctx, context: &Context<'_>) -> Element {
  use crate::{
    routes::{NewConnection, ProjectPage, Route},
    ui::button::{ActionButton, ButtonKind},
  };
  let nav = crate::navigation::Navigation::of(ctx);
  let add = ActionButton::new("agent-add-mcp", ButtonKind::Ghost, ctx.t("agents.addMcp"))
    .icon(Icon::Plus)
    .opens(nav.to(Route::Project(
      context.project.open.project.id(),
      ProjectPage::NewConnection(NewConnection::McpServer),
    )));
  field("agent-mcp-none", &ctx.t("agents.mcpNone"), None, add).into()
}

fn server_detail(ctx: &Ctx, server: &orchester_core::connection::mcp::McpServer) -> String {
  use orchester_core::connection::mcp::McpTransport;
  let transport = match server.transport() {
    McpTransport::Stdio(stdio) => ctx.t_args("agents.mcpStdio", [("program", stdio.program().as_str())]),
    McpTransport::Http(http) => ctx.t_args("agents.mcpHttp", [("host", http.url().host())]),
  };
  if server.enabled() {
    transport.to_string()
  } else {
    ctx
      .t_args(
        "agents.mcpTurnedOff",
        [
          ("transport", transport.to_string()),
          ("off", ctx.t("agents.mcpDisabled").to_string()),
        ],
      )
      .to_string()
  }
}
