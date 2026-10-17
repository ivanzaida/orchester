//! An MCP server's inspector (`O6Vo2a`, the states of `XRCed`): with Turn off or Turn on in its header, what
//! the agent's CLI starts or reaches (program, arguments, folder, or endpoint and bearer secret), its
//! variables with a secret named by its name and environment only, the tools that run without asking, its
//! tools (which Orchester cannot list, so Refresh tools is unavailable), the profiles that choose it, and in
//! the footer what it does with Test connection (unavailable for the same reason), Edit connection and
//! Remove. Turning off a server profiles choose, and removing one, ask in place in the footer; a server
//! profiles still choose cannot be removed, and says which they are.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::connection::mcp::{HttpServer, McpEnvValue, McpServer, McpTransport, StdioServer};
use orchester_workspace::{connections::mcp::enabling, display};

use super::{Actions, Confirm, Inspecting, Parts, footer, header};
use crate::{
  screens::connections::{
    widen::{WidenConfirm, WidenProps},
    words::{mcp_glyph, mcp_status},
    work::Work,
  },
  theme::{ConnectionsText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    icon::{Icon, icon},
    inspector::{self, PropertyValue, property, variable},
    message::message_text,
    notice::{Notice, NoticeTone},
    unavailable::reason_line,
  },
};

/// The longest program path a property shows in full, as an agent CLI's.
const PATH_CHARS: usize = 26;

pub(super) fn parts(ctx: &mut Ctx, server: &McpServer, inspecting: &Inspecting<'_>, actions: &Actions) -> Parts {
  let users: Vec<String> = inspecting
    .listed
    .used_by(server.id())
    .map(|used| used.role.to_string())
    .collect();
  let switch = switch(ctx, server, inspecting, actions, !users.is_empty());
  let header = header(
    ctx,
    (mcp_glyph(server), server.name().as_str()),
    mcp_status(server.enabled()),
    Some(switch),
  );
  let mut body: Vec<Element> = secret_missing(ctx, server, inspecting).into_iter().collect();
  body.push(properties(ctx, server, inspecting).into());
  body.extend(variables(ctx, server, inspecting));
  body.push(exempt_tools(ctx, server));
  body.push(discovered_tools(ctx));
  body.push(used_by(ctx, server, &users, actions));
  let footer = match inspecting.confirming {
    Some(Confirm::TurnOn) => turn_on(ctx, server, actions),
    Some(Confirm::TurnOff) => confirmation(ctx, server, Narrowing::TurnOff, &users, actions),
    Some(Confirm::Remove) => confirmation(ctx, server, Narrowing::Remove, &users, actions),
    None => plain_footer(ctx, server, inspecting, actions, &users),
  };
  Parts {
    header,
    body,
    footer: Some(footer),
  }
}

/// Turn off in the header of a server that is on, Turn on in that of one that is off.
fn switch(ctx: &Ctx, server: &McpServer, inspecting: &Inspecting<'_>, actions: &Actions, used: bool) -> Element {
  let (id, start, confirm) = (server.id(), actions.start.clone(), actions.confirm.clone());
  let enabled = server.enabled();

  let button = if enabled {
    ActionButton::new(
      "connection-turn-off",
      ButtonKind::Ghost,
      ctx.t("connections.mcp.turnOff"),
    )
  } else {
    ActionButton::new(
      "connection-turn-on",
      ButtonKind::Secondary,
      ctx.t("connections.mcp.turnOn"),
    )
  }
  .icon(Icon::Power);
  match (inspecting.running, inspecting.confirming) {
    (Some(Work::SetEnabled(_)), _) => button.loading().into(),
    (Some(_), _) | (None, Some(_)) => button.disabled().into(),
    (None, None) if enabled && used => button.on_click(move || confirm(Some(Confirm::TurnOff))).into(),
    (None, None) if enabled => button.on_click(move || start(id, Work::SetEnabled(false))).into(),
    (None, None) => button.on_click(move || confirm(Some(Confirm::TurnOn))).into(),
  }
}

fn properties(ctx: &Ctx, server: &McpServer, inspecting: &Inspecting<'_>) -> Column {
  let rows = match server.transport() {
    McpTransport::Stdio(stdio) => stdio_rows(ctx, stdio),
    McpTransport::Http(http) => http_rows(ctx, http, inspecting),
  };
  Column::new()
    .id("connection-properties")
    .width(Dimension::Pct(100.0))
    .with_children(rows)
}

/// What the agent's CLI starts: the program, its arguments and folder.
fn stdio_rows(ctx: &Ctx, stdio: &StdioServer) -> Vec<Row> {
  let arguments: Vec<&str> = stdio.args().iter().map(|arg| arg.as_str()).collect();
  let (folder, folder_kind) = match stdio.working_directory() {
    Some(folder) => (folder.as_str().to_owned(), PropertyValue::Mono),
    None => (ctx.t("connections.mcp.theWorktree").to_string(), PropertyValue::Plain),
  };
  let mut rows = vec![
    property(
      "connection-type",
      &ctx.t("connections.mcp.property.type"),
      &ctx.t("connections.mcp.typeStdio"),
      PropertyValue::Plain,
    ),
    property(
      "connection-program",
      &ctx.t("connections.mcp.property.program"),
      &display::shorten_path(Path::new(stdio.program().as_str()), PATH_CHARS),
      PropertyValue::Mono,
    ),
  ];
  if !arguments.is_empty() {
    rows.push(property(
      "connection-arguments",
      &ctx.t("connections.mcp.property.arguments"),
      &arguments.join(" "),
      PropertyValue::Mono,
    ));
  }
  rows.push(property(
    "connection-folder",
    &ctx.t("connections.mcp.property.folder"),
    &folder,
    folder_kind,
  ));
  rows.push(property(
    "connection-started-by",
    &ctx.t("connections.mcp.property.startedBy"),
    &ctx.t("connections.mcp.startedByCli"),
    PropertyValue::Plain,
  ));
  rows
}

/// Where the agent's CLI reaches the server, and with which secret.
fn http_rows(ctx: &Ctx, http: &HttpServer, inspecting: &Inspecting<'_>) -> Vec<Row> {
  let authentication = match http.bearer() {
    Some(secret) => ctx.t_args(
      "connections.mcp.bearer",
      [("secret", &*secret_name(ctx, inspecting, secret))],
    ),
    None => ctx.t("connections.mcp.noAuthentication"),
  };
  vec![
    property(
      "connection-type",
      &ctx.t("connections.mcp.property.type"),
      &ctx.t("connections.mcp.typeHttp"),
      PropertyValue::Plain,
    ),
    property(
      "connection-endpoint",
      &ctx.t("connections.mcp.property.endpoint"),
      http.url().as_str(),
      PropertyValue::Mono,
    ),
    property(
      "connection-authentication",
      &ctx.t("connections.property.authentication"),
      &authentication,
      PropertyValue::Plain,
    ),
    property(
      "connection-reached-by",
      &ctx.t("connections.mcp.property.reachedBy"),
      &ctx.t("connections.mcp.reachedByCli"),
      PropertyValue::Plain,
    ),
  ]
}

/// That a variable or the bearer token takes its value from a secret the project archived or no longer has, so the
/// runner leaves the server out, and what a person does about it.
fn secret_missing(ctx: &Ctx, server: &McpServer, inspecting: &Inspecting<'_>) -> Option<Element> {
  let bearer = match server.transport() {
    McpTransport::Http(http) => http.bearer(),
    McpTransport::Stdio(_) => None,
  };
  let variables = server.environment().iter().filter_map(|(_, value)| match value {
    McpEnvValue::Secret(secret) => Some(*secret),
    McpEnvValue::Text(_) => None,
  });
  let missing = bearer
    .into_iter()
    .chain(variables)
    .any(|id| !inspecting.listed.secrets.iter().any(|secret| secret.id() == id));
  missing.then(|| {
    Notice::new(
      "connection-secret-missing",
      NoticeTone::Warning,
      ctx.t("connections.mcp.secretMissing.title"),
    )
    .body(ctx.t("connections.mcp.secretMissing.detail"))
    .into()
  })
}

/// `NAME (environment)` of secret `id`, or that the project no longer has it.
fn secret_name(ctx: &Ctx, inspecting: &Inspecting<'_>, id: orchester_core::id::SecretId) -> Arc<str> {
  inspecting
    .listed
    .secrets
    .iter()
    .find(|secret| secret.id() == id)
    .map_or_else(
      || ctx.t("connections.mcp.secretGone"),
      |secret| {
        ctx.t_args(
          "connections.mcp.secretName",
          [
            ("name", secret.name().as_str()),
            ("environment", secret.environment().as_str()),
          ],
        )
      },
    )
}

fn variables(ctx: &Ctx, server: &McpServer, inspecting: &Inspecting<'_>) -> Option<Element> {
  if server.environment().is_empty() {
    return None;
  }
  let rows = server.environment().iter().enumerate().map(|(index, (name, value))| {
    let id = format!("connection-variable-{index}");
    match value {
      McpEnvValue::Text(text) => variable(&id, name.as_str(), text, PropertyValue::Mono),
      McpEnvValue::Secret(secret) => {
        let from = ctx.t_args(
          "connections.mcp.fromSecret",
          [("secret", &*secret_name(ctx, inspecting, *secret))],
        );
        variable(&id, name.as_str(), &from, PropertyValue::Plain)
      }
    }
  });
  Some(
    inspector::section("connection-variables", SpacingSize::Sm)
      .child(inspector::section_title(
        "connection-variables-title",
        &ctx.t("connections.mcp.variables"),
        None,
      ))
      .child(Column::new().width(Dimension::Pct(100.0)).with_children(rows))
      .into(),
  )
}

fn exempt_tools(ctx: &Ctx, server: &McpServer) -> Element {
  let tools = server.exempt_tools().tools();
  let section = inspector::section("connection-exempt-tools", SpacingSize::Sm).child(inspector::section_title(
    "connection-exempt-tools-title",
    &ctx.t("connections.mcp.exemptTools"),
    None,
  ));
  let note = |key| Text::new(&ctx.t(key)).variant(ExtraTypography::Note.role());
  if tools.is_empty() {
    return section.child(note("connections.mcp.exemptToolsNone")).into();
  }
  section
    .child(
      Row::new()
        .id("connection-exempt-tool-chips")
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .wrap()
        .with_children(
          tools
            .iter()
            .enumerate()
            .map(|(index, tool)| TagChip::new(format!("connection-exempt-tool-{index}"), tool.as_str()).mono()),
        ),
    )
    .child(note("connections.mcp.exemptToolsNote"))
    .into()
}

fn discovered_tools(ctx: &Ctx) -> Element {
  let reason = ctx.t("connections.mcp.toolsNote");
  inspector::section("connection-discovered-tools", SpacingSize::Sm)
    .child(inspector::section_title(
      "connection-discovered-tools-title",
      &ctx.t("connections.mcp.tools"),
      None,
    ))
    .child(Text::new(&reason).variant(ExtraTypography::Note.role()))
    .child(
      Row::new().child(
        ActionButton::new(
          "connection-refresh-tools",
          ButtonKind::Secondary,
          ctx.t("connections.mcp.refreshTools"),
        )
        .unavailable(reason.clone()),
      ),
    )
    .into()
}

fn used_by(ctx: &Ctx, server: &McpServer, users: &[String], actions: &Actions) -> Element {
  let section = super::used_by(ctx, users.to_vec(), actions, "connections.mcp.notUsed");
  if server.enabled() || users.is_empty() {
    return section;
  }
  let key = if users.len() == 1 {
    "connections.mcp.offUsersOne"
  } else {
    "connections.mcp.offUsers"
  };
  let off = ctx.t_args(key, [("profiles", users.join(", "))]);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(section)
    .child(inspector::note("connection-off-note", Icon::CircleMinus, &off))
    .into()
}

fn plain_footer(
  ctx: &Ctx,
  server: &McpServer,
  inspecting: &Inspecting<'_>,
  actions: &Actions,
  users: &[String],
) -> Element {
  let tools_reason = ctx.t("connections.mcp.toolsNote");
  let id = server.id();
  let busy = inspecting.running.is_some();
  let test = ActionButton::new("connection-test", ButtonKind::Secondary, ctx.t("connections.test"))
    .icon(Icon::PlugZap)
    .unavailable(tools_reason);
  let edit = ActionButton::new("connection-edit", ButtonKind::Secondary, ctx.t("connections.edit")).icon(Icon::Pencil);
  let edit = if busy {
    edit.disabled()
  } else {
    edit.opens((actions.edit)(id))
  };
  let remove =
    ActionButton::new("connection-remove", ButtonKind::Ghost, ctx.t("connections.mcp.remove")).icon(Icon::Trash2);
  let in_use = (!users.is_empty()).then(|| ctx.t_args("connections.mcp.removeInUse", [("profiles", users.join(", "))]));
  let remove = match (&in_use, inspecting.running) {
    (Some(reason), _) => remove.unavailable(reason.clone()),
    (None, Some(Work::Remove)) => remove.loading(),
    (None, Some(_)) => remove.disabled(),
    (None, None) => {
      let confirm = actions.confirm.clone();
      remove.on_click(move || confirm(Some(Confirm::Remove)))
    }
  };
  let reasons = in_use
    .map(|reason| reason_line("connection-remove-reason", &reason).into())
    .into_iter()
    .collect();
  let sentence = if server.enabled() {
    ctx.t("connections.mcp.detail")
  } else {
    ctx.t("connections.mcp.offDetail")
  };
  let rows = vec![vec![test.into(), edit.into()], vec![remove.into()]];
  footer(ctx, inspecting, (&sentence, Vec::new()), (rows, reasons))
}

/// Turning the server on, asked of a person in place with what it widens (`widen`).
fn turn_on(ctx: &mut Ctx, server: &McpServer, actions: &Actions) -> Element {
  let reasons: Arc<[Arc<str>]> = enabling(server, true)
    .iter()
    .map(|reason| message_text(ctx, reason))
    .collect();
  let (start, close, keep, id) = (
    actions.start.clone(),
    actions.confirm.clone(),
    actions.confirm.clone(),
    server.id(),
  );
  let props = WidenProps {
    card: false,
    automation: crate::mcp::input_gate::Guarded::PermissionWidening,
    id: "connection-confirm-turn-on".into(),
    title: ctx.t_args("connections.mcp.confirmOn.title", [("name", server.name().as_str())]),
    reasons,
    confirm: ctx.t("connections.mcp.turnOn"),
    keep: ctx.t("connections.mcp.confirmOn.keep"),
    on_confirm: Arc::new(move || {
      close(None);
      start(id, Work::SetEnabled(true));
    }),
    on_keep: Arc::new(move || keep(None)),
  };
  ctx.mount_keyed::<WidenConfirm>(&format!("connection-turn-on-{id}"), props)
}

/// The question the footer asks in place (`Confirm · Turn off` `D2FVJJ`): what turning the server off, or
/// removing it, does, with the way back first.
/// What the footer asks before it narrows what agents get.
#[derive(Clone, Copy)]
enum Narrowing {
  TurnOff,
  Remove,
}

/// A narrowing's words: what it asks, what it does, its action's id, label, icon and work, and the way back.
struct Asked {
  title: Arc<str>,
  body: Arc<str>,
  act: (&'static str, Arc<str>, Icon, Work),
  keep: Arc<str>,
}

fn asked(ctx: &Ctx, name: &str, confirm: Narrowing, users: &[String]) -> Asked {
  match confirm {
    Narrowing::TurnOff => Asked {
      title: ctx.t_args("connections.mcp.confirmOff.title", [("name", name)]),
      body: ctx.t_args(
        if users.len() == 1 {
          "connections.mcp.confirmOff.bodyOne"
        } else {
          "connections.mcp.confirmOff.body"
        },
        [("profiles", users.join(", "))],
      ),
      act: (
        "connection-confirm-turn-off",
        ctx.t("connections.mcp.turnOff"),
        Icon::Power,
        Work::SetEnabled(false),
      ),
      keep: ctx.t("connections.mcp.confirmOff.keep"),
    },
    Narrowing::Remove => Asked {
      title: ctx.t_args("connections.mcp.confirmRemove.title", [("name", name)]),
      body: ctx.t("connections.mcp.confirmRemove.body"),
      act: (
        "connection-confirm-remove",
        ctx.t("connections.mcp.remove"),
        Icon::Trash2,
        Work::Remove,
      ),
      keep: ctx.t("connections.mcp.confirmRemove.keep"),
    },
  }
}

fn confirmation(ctx: &Ctx, server: &McpServer, confirm: Narrowing, users: &[String], actions: &Actions) -> Element {
  let Asked {
    title,
    body,
    act,
    keep: keep_label,
  } = asked(ctx, server.name().as_str(), confirm, users);
  let back = actions.confirm.clone();
  let keep =
    ActionButton::new("connection-confirm-keep", ButtonKind::Secondary, keep_label).on_click(move || back(None));
  let (start, close, id, work) = (actions.start.clone(), actions.confirm.clone(), server.id(), act.3);
  let go = ActionButton::new(act.0, ButtonKind::Danger, act.1)
    .icon(act.2)
    .on_click(move || {
      close(None);
      start(id, work);
    });
  Column::new()
    .id("connection-confirm")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::WarningMuted)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Warning))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Start)
        .child(icon(Icon::TriangleAlert, IconSize::Md, PaletteColor::Warning))
        .child(
          Column::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .spacing(ExtraSpacing::Fine.role())
            .child(
              Text::new(&title)
                .id("connection-confirm-title")
                .variant(ConnectionsText::ConfirmTitle.role()),
            )
            .child(
              Text::new(&body)
                .variant(ExtraTypography::Note.role())
                .color(PaletteColor::TextSecondary),
            ),
        ),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(Spacer::new().flex(1.0))
        .child(keep)
        .child(go),
    )
    .into()
}
