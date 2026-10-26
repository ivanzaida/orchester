//! An MCP server's page (`13` `Ptkxo`): its switch, Edit and its … menu (Rename, which its form does, and Remove) in
//! the head, its status with who uses it; *Server*: what the agent's CLI starts or reaches (program, arguments, folder,
//! or endpoint and authorization); *Sends*: its variables, a secret named by its name and environment only, and the
//! tools that run without asking; its tools, which Orchester cannot list; and the profiles that choose it. Turning a
//! server off that profiles choose, and removing one, ask in place under the groups (`Turn off · question` `Q2LiC`);
//! turning one on widens what agents may do, so it asks a person in place the same way (`widen`). A server profiles
//! still choose cannot be removed, and says which they are. A variable whose secret is gone says *Secret missing* with
//! Open Secrets.

use std::{path::Path, sync::Arc};

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::connection::mcp::{HttpServer, McpAuthorization, McpEnvValue, McpServer, McpTransport, StdioServer};
use orchester_workspace::{connections::mcp::enabling, display};

use super::{Actions, Confirm, Inspecting, Parts, title, used_by_row};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, SecretsView},
  screens::{
    connections::{
      widen::{WidenConfirm, WidenLook, WidenProps},
      words::{mcp_glyph, mcp_status},
      work::Work,
    },
    project_settings::kit::{Group, SettingRow, Tone, question, value},
  },
  theme::ExtraTypography,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    menu::MenuItem,
    message::message_text,
    toggle::{Toggle, ToggleKind},
  },
};

/// The longest program path a row shows in full.
const PATH_CHARS: usize = 40;

pub(super) fn parts(ctx: &mut Ctx, server: &McpServer, inspecting: &Inspecting<'_>, actions: &Actions) -> Parts {
  let users: Vec<String> = inspecting
    .listed
    .used_by(server.id())
    .map(|used| used.role.to_string())
    .collect();
  let detail = if users.is_empty() {
    ctx.t("connections.mcp.notUsedShort")
  } else {
    ctx.t_args("connections.mcp.usedByShort", [("profiles", users.join(", "))])
  };
  let asks = Asks {
    start: actions.start.clone(),
    confirm: actions.confirm.clone(),
  };
  let switch = switch(
    ctx,
    ("connection-switch", server),
    (inspecting.running, inspecting.confirming),
    &asks,
    !users.is_empty(),
  );
  let busy = inspecting.running.is_some();
  let edit = ActionButton::new("connection-edit", ButtonKind::Secondary, ctx.t("connections.mcp.editShort")).icon(Icon::Pencil);
  let edit = if busy { edit.disabled() } else { edit.opens((actions.edit)(server.id())) };
  let question = inspecting
    .confirming
    .map(|confirm| asked(ctx, server, (confirm, &users), &asks));
  let mut groups = vec![server_group(ctx, server, inspecting), sends_group(ctx, server, inspecting), tools_group(ctx)];
  groups.push(used_by_row(ctx, &users, actions, "connections.mcp.notUsed"));
  let off = (!server.enabled() && !users.is_empty()).then(|| {
    let key = if users.len() == 1 {
      "connections.mcp.offUsersOne"
    } else {
      "connections.mcp.offUsers"
    };
    super::sentence(
      "connection-off-note",
      &ctx.t_args(key, [("profiles", users.join(", "))]),
      lurq::app::theme::PaletteColor::TextMuted,
    )
  });
  Parts {
    glyph: mcp_glyph(server),
    title: title(server.name().as_str()).into(),
    status: mcp_status(server.enabled()),
    status_detail: Some(detail),
    actions: vec![switch, edit.into()],
    menu: menu(ctx, server, inspecting, actions, &users),
    notices: off.into_iter().collect(),
    groups,
    question,
  }
}

/// What a server's switch and its questions do: start work on it, and ask (or stop asking) in place.
pub(in crate::screens::connections) struct Asks {
  pub start: Arc<dyn Fn(orchester_core::id::ConnectionId, Work) + Send + Sync>,
  pub confirm: Arc<dyn Fn(Option<Confirm>) + Send + Sync>,
}

/// The switch `id`: off at once when no profile chooses the server, else asked in place; on asked in place, as it
/// widens. Nothing is taken while work runs on the server or a question is open.
pub(in crate::screens::connections) fn switch(
  ctx: &Ctx,
  (id, server): (&str, &McpServer),
  (running, confirming): (Option<Work>, Option<Confirm>),
  asks: &Asks,
  used: bool,
) -> Element {
  let (server_id, start, confirm) = (server.id(), asks.start.clone(), asks.confirm.clone());
  let enabled = server.enabled();
  let toggle = Toggle::new(
    id.to_owned(),
    ToggleKind::Switch,
    enabled,
    ctx.t(if enabled {
      "connections.mcp.turnOff"
    } else {
      "connections.mcp.turnOn"
    }),
  )
  .hide_label();
  match (running, confirming) {
    (Some(_), _) | (None, Some(_)) => toggle.into(),
    (None, None) if enabled && used => toggle.on_change(move |_| confirm(Some(Confirm::TurnOff))).into(),
    (None, None) if enabled => toggle.on_change(move |_| start(server_id, Work::SetEnabled(false))).into(),
    (None, None) => toggle.on_change(move |_| confirm(Some(Confirm::TurnOn))).into(),
  }
}

/// The question `confirm` asks in place about `server`, which the profiles `users` choose.
pub(in crate::screens::connections) fn asked(
  ctx: &mut Ctx,
  server: &McpServer,
  (confirm, users): (Confirm, &[String]),
  asks: &Asks,
) -> Element {
  match confirm {
    Confirm::TurnOn => turn_on(ctx, server, asks),
    Confirm::TurnOff => narrowing(ctx, server, (Narrowing::TurnOff, users), asks),
    Confirm::Remove => narrowing(ctx, server, (Narrowing::Remove, users), asks),
  }
}

/// Rename, which Edit MCP server does, and Remove: unavailable, with why, while profiles choose the server.
fn menu(ctx: &Ctx, server: &McpServer, inspecting: &Inspecting<'_>, actions: &Actions, users: &[String]) -> Vec<MenuItem> {
  let rename = MenuItem::opens(
    "connection-rename",
    ctx.t("connections.rename.action"),
    (actions.edit)(server.id()),
  )
  .icon(Icon::Pencil);
  let remove = if users.is_empty() {
    match inspecting.running {
      Some(_) => MenuItem::unavailable("connection-remove", ctx.t("connections.mcp.remove"), ctx.t("connections.busy")),
      None => {
        let confirm = actions.confirm.clone();
        MenuItem::new("connection-remove", ctx.t("connections.mcp.remove"), move || {
          confirm(Some(Confirm::Remove));
        })
      }
    }
  } else {
    MenuItem::unavailable(
      "connection-remove",
      ctx.t("connections.mcp.remove"),
      ctx.t_args("connections.mcp.removeInUse", [("profiles", users.join(", "))]),
    )
  };
  vec![rename, remove.icon(Icon::Trash2)]
}

/// *Server* (`neHGO`): what the agent's CLI starts or reaches.
fn server_group(ctx: &mut Ctx, server: &McpServer, inspecting: &Inspecting<'_>) -> Element {
  let rows = match server.transport() {
    McpTransport::Stdio(stdio) => stdio_rows(ctx, stdio),
    McpTransport::Http(http) => http_rows(ctx, http, inspecting),
  };
  Group::new("connection-properties", ctx.t("connections.mcp.serverGroup"))
    .rows(rows)
    .into()
}

/// A row with its value at the end, in Geist Mono for a path, a command or an address.
fn row(ctx: &mut Ctx, id: &str, (label, shown): (&str, &str), mono: bool) -> SettingRow {
  let value = value(&format!("{id}-value"), shown);
  let value = if mono {
    value.variant(ExtraTypography::PropertyMono.role()).color(lurq::app::theme::PaletteColor::TextSecondary)
  } else {
    value
  };
  SettingRow::new(id.to_owned(), ctx.t(label)).control(value)
}

/// What the agent's CLI starts: the program, its arguments and folder.
fn stdio_rows(ctx: &mut Ctx, stdio: &StdioServer) -> Vec<Element> {
  let arguments: Vec<&str> = stdio.args().iter().map(|arg| arg.as_str()).collect();
  let (folder, mono) = match stdio.working_directory() {
    Some(folder) => (folder.as_str().to_owned(), true),
    None => (ctx.t("connections.mcp.theWorktree").to_string(), false),
  };
  let runs_as = row(ctx, "connection-type", ("connections.mcp.runsAs", &ctx.t("connections.mcp.typeStdio")), false)
    .hint(ctx.t("connections.mcp.startedByCli"))
    .render(ctx);
  let program = display::shorten_path(Path::new(stdio.program().as_str()), PATH_CHARS);
  let mut rows = vec![runs_as, row(ctx, "connection-program", ("connections.mcp.property.program", &program), true).render(ctx)];
  if !arguments.is_empty() {
    rows.push(
      row(
        ctx,
        "connection-arguments",
        ("connections.mcp.property.arguments", &arguments.join(" ")),
        true,
      )
      .render(ctx),
    );
  }
  rows.push(row(ctx, "connection-folder", ("connections.mcp.property.folder", &folder), mono).render(ctx));
  rows
}

/// Where the agent's CLI reaches the server, and with which secret.
fn http_rows(ctx: &mut Ctx, http: &HttpServer, inspecting: &Inspecting<'_>) -> Vec<Element> {
  let authorization = http.authorization();
  let secret = authorization
    .secret()
    .map_or_else(Default::default, |id| secret_name(ctx, inspecting, id));
  let authentication = match authorization {
    McpAuthorization::None => ctx.t("connections.mcp.noAuthentication"),
    McpAuthorization::Bearer { .. } => ctx.t_args("connections.mcp.bearer", [("secret", &*secret)]),
    McpAuthorization::Basic { username, .. } => ctx.t_args(
      "connections.mcp.basic",
      [("username", username.as_str()), ("secret", &*secret)],
    ),
    McpAuthorization::BasicCredential { .. } => ctx.t_args("connections.mcp.basicCredential", [("secret", &*secret)]),
    McpAuthorization::Header { name, .. } => ctx.t_args(
      "connections.mcp.header",
      [("header", name.as_str()), ("secret", &*secret)],
    ),
  };
  vec![
    row(ctx, "connection-type", ("connections.mcp.runsAs", &ctx.t("connections.mcp.typeHttp")), false)
      .hint(ctx.t("connections.mcp.reachedByCli"))
      .render(ctx),
    row(ctx, "connection-endpoint", ("connections.mcp.property.endpoint", http.url().as_str()), true).render(ctx),
    row(
      ctx,
      "connection-authentication",
      ("connections.property.authentication", &authentication),
      false,
    )
    .render(ctx),
  ]
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

/// *Sends* (`z6ILot`): its variables, a secret by its name, and the tools that run without asking.
fn sends_group(ctx: &mut Ctx, server: &McpServer, inspecting: &Inspecting<'_>) -> Element {
  let variables: Vec<String> = server
    .environment()
    .iter()
    .map(|(name, shown)| match shown {
      McpEnvValue::Text(text) => ctx
        .t_args("connections.mcp.variableText", [("name", name.as_str()), ("value", text.as_str())])
        .to_string(),
      McpEnvValue::Secret(secret) => ctx
        .t_args(
          "connections.mcp.variableSecret",
          [("name", name.as_str()), ("secret", &*secret_name(ctx, inspecting, *secret))],
        )
        .to_string(),
    })
    .collect();
  let missing = server
    .secrets()
    .into_iter()
    .any(|id| !inspecting.listed.secrets.iter().any(|secret| secret.id() == id));
  let shown = if variables.is_empty() {
    ctx.t("connections.mcp.noVariables")
  } else {
    variables.join("; ").into()
  };
  let mut variables_row = row(ctx, "connection-variables", ("connections.mcp.variables", &shown), false);
  if missing {
    let open = Navigation::of(ctx).to(Route::Project(
      inspecting.open.project.id(),
      ProjectPage::Secrets(SecretsView::default()),
    ));
    variables_row = variables_row
      .toned(ctx.t("connections.mcp.secretMissing.title"), Tone::Warning)
      .info(ctx.t("connections.mcp.secretMissing.detail"))
      .control(
        ActionButton::new("connection-open-secrets", ButtonKind::Secondary, ctx.t("connections.mcp.openSecrets"))
          .icon(Icon::KeyRound)
          .compact()
          .opens(open),
      );
  }
  let tools = server.exempt_tools().tools();
  let exempt = if tools.is_empty() {
    ctx.t("connections.mcp.exemptToolsNone")
  } else {
    tools.iter().map(|tool| tool.as_str()).collect::<Vec<_>>().join(", ").into()
  };
  let exempt_row = row(ctx, "connection-exempt-tools", ("connections.mcp.exemptTools", &exempt), false)
    .info(ctx.t("connections.mcp.exemptToolsNote"));
  let rows = vec![variables_row.render(ctx), exempt_row.render(ctx)];
  Group::new("connection-variables", ctx.t("connections.mcp.sendsGroup"))
    .rows(rows)
    .into()
}

/// Its tools, which Orchester cannot list: Refresh tools stays unavailable with why.
fn tools_group(ctx: &mut Ctx) -> Element {
  let reason = ctx.t("connections.mcp.toolsNote");
  let row = SettingRow::new("connection-discovered-tools", ctx.t("connections.mcp.tools"))
    .hint(reason.clone())
    .control(
      ActionButton::new(
        "connection-refresh-tools",
        ButtonKind::Secondary,
        ctx.t("connections.mcp.refreshTools"),
      )
      .compact()
      .unavailable(reason),
    )
    .render(ctx);
  Group::new("connection-tools", ctx.t("connections.mcp.tools"))
    .row(row)
    .into()
}

/// Turning the server on, asked of a person in place with what it widens (`widen`).
fn turn_on(ctx: &mut Ctx, server: &McpServer, asks: &Asks) -> Element {
  let reasons: Arc<[Arc<str>]> = enabling(server, true)
    .iter()
    .map(|reason| message_text(ctx, reason))
    .collect();
  let (start, close, keep, id) = (
    asks.start.clone(),
    asks.confirm.clone(),
    asks.confirm.clone(),
    server.id(),
  );
  let props = WidenProps {
    look: WidenLook::Box,
    affirming: false,
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

/// What the page asks before it narrows what agents get.
#[derive(Clone, Copy)]
enum Narrowing {
  TurnOff,
  Remove,
}

/// Turning the server off, or removing it, asked in place: what it does, the action and the way back.
fn narrowing(ctx: &Ctx, server: &McpServer, (narrowing, users): (Narrowing, &[String]), asks: &Asks) -> Element {
  let name = server.name().as_str();
  let (title, body, (id, label, glyph, work), keep) = match narrowing {
    Narrowing::TurnOff => (
      ctx.t_args("connections.mcp.confirmOff.title", [("name", name)]),
      ctx.t_args(
        if users.len() == 1 {
          "connections.mcp.confirmOff.bodyOne"
        } else {
          "connections.mcp.confirmOff.body"
        },
        [("profiles", users.join(", "))],
      ),
      (
        "connection-confirm-turn-off",
        ctx.t("connections.mcp.turnOff"),
        Icon::Power,
        Work::SetEnabled(false),
      ),
      ctx.t("connections.mcp.confirmOff.keep"),
    ),
    Narrowing::Remove => (
      ctx.t_args("connections.mcp.confirmRemove.title", [("name", name)]),
      ctx.t("connections.mcp.confirmRemove.body"),
      (
        "connection-confirm-remove",
        ctx.t("connections.mcp.removeServer"),
        Icon::Trash2,
        Work::Remove,
      ),
      ctx.t("connections.mcp.confirmRemove.keep"),
    ),
  };
  let (start, close, id_of, back) = (asks.start.clone(), asks.confirm.clone(), server.id(), asks.confirm.clone());
  let go = ActionButton::new(id, ButtonKind::Danger, label).icon(glyph).on_click(move || {
    close(None);
    start(id_of, work);
  });
  let keep = ActionButton::new("connection-confirm-keep", ButtonKind::Ghost, keep).on_click(move || back(None));
  question("connection-confirm", &title, &[body], vec![go.into(), keep.into()]).into()
}
