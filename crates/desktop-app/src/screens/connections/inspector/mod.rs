//! A connection's page under Project settings › Connections (`05b` `PGwka`, an MCP server `13` `Ptkxo`, GitHub `14`
//! `KItiZ`): the crumb back to Connections, its head (a 36 tile, its name in 16/600 over its status, its actions and a
//! … menu at the end), what its status asks for, its details as groups of rows, and a question asked in place under
//! them. Each kind fills it: an agent provider (`agent`, on an API provider `provider`), GitHub (`github`), an MCP
//! server (`mcp`). An agent or GitHub connection is renamed in place of its title (`rename`), and a check pressed here
//! says how it goes and what it found under the head (`result`).

mod agent;
mod github;
mod mcp;
mod provider;
mod rename;
mod result;

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ConnectionId, time::Timestamp};
use orchester_workspace::{connections::data::ProjectConnections, display, messages::Message, workspace::OpenProject};

pub(super) use self::mcp::{Asks, asked, switch as mcp_switch};
use super::{
  data::Entry,
  words::{CHECKING, Shown},
  work::Work,
};
use crate::{
  navigation::link::Open,
  screens::project_settings::kit::{Glyph, SettingRow, back, entity_head},
  theme::{ExtraSpacing, IconSize, ProjectSettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
    message::message_text,
  },
};

/// The … menu's width (`Menu` in `05b` `PGwka`).
const MENU_WIDTH: f32 = 220.0;

/// A question the page asks in place before it acts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Confirm {
  /// Turn off an MCP server that profiles choose.
  TurnOff,
  /// Remove an MCP server (`Turn off · question` `Q2LiC`).
  Remove,
  /// Turn on an MCP server, which widens what the agents of the profiles that choose it get (`widen`).
  TurnOn,
}

impl lurq::app::component::DevtoolsInspectable for Confirm {}

/// Where the check last pressed in this screen on the connection (Check again, Refresh models) stands.
#[derive(Clone, Copy, Debug)]
pub(super) enum Tested<'a> {
  /// It runs, or what it stored is not read back yet.
  Checking,
  /// It ran: the connection's last check is what it found.
  Checked,
  /// It could not run, or what it found could not be stored.
  Failed(&'a Message),
  /// It ran and stored what it found, but reading the connections again failed for this reason.
  Unread(&'a Message),
}

/// What the page shows about the work on its connection.
pub(super) struct Inspecting<'a> {
  pub listed: &'a ProjectConnections,
  /// The person's agent accounts, which a CLI connection runs on one of.
  pub accounts: &'a crate::services::accounts::Accounts,
  pub entry: Entry<'a>,
  pub open: &'a OpenProject,
  pub project: &'a str,
  /// The work running on the connection now.
  pub running: Option<Work>,
  /// Why the last work other than a check on the connection failed.
  pub failed: Option<&'a Message>,
  /// The check pressed on the connection in this screen, if one was.
  pub tested: Option<Tested<'a>>,
  pub confirming: Option<Confirm>,
  pub now: Timestamp,
}

/// What the page's actions do.
#[derive(Clone)]
pub(super) struct Actions {
  pub start: Arc<dyn Fn(ConnectionId, Work) + Send + Sync>,
  pub confirm: Arc<dyn Fn(Option<Confirm>) + Send + Sync>,
  /// What opens the page that edits a connection.
  pub edit: Arc<dyn Fn(ConnectionId) -> Open + Send + Sync>,
  /// Opens the Agents screen, where a profile chooses its connection and servers.
  pub agents: Open,
  /// What opens Connections itself, the crumb's way back.
  pub back: Open,
  /// The name is edited in place of the title.
  pub renaming: Signal<bool>,
  /// The … menu and its button.
  pub more: (Signal<bool>, ElementRef),
}

/// A page's parts as a kind fills them.
pub(super) struct Parts {
  pub glyph: Glyph,
  /// The name, or its rename in place.
  pub title: Element,
  pub status: Shown,
  /// What the status line says after the status's word, e.g. when it was checked or who uses it.
  pub status_detail: Option<Arc<str>>,
  /// The head's actions, before the … menu.
  pub actions: Vec<Element>,
  /// The … menu's choices; without any, no menu.
  pub menu: Vec<MenuItem>,
  /// What the status asks for, under the head: a sentence, a command to run, a check's result, a warning.
  pub notices: Vec<Element>,
  pub groups: Vec<Element>,
  /// The question asked in place, under the groups.
  pub question: Option<Element>,
}

/// The page of `inspecting.entry`.
pub(super) fn page(ctx: &mut Ctx, inspecting: &Inspecting<'_>, actions: &Actions) -> Element {
  let parts = match inspecting.entry {
    Entry::Agent(connection) => agent::parts(ctx, connection, inspecting, actions),
    Entry::GitHub(connection) => github::parts(ctx, connection, inspecting, actions),
    Entry::Mcp(server) => mcp::parts(ctx, server, inspecting, actions),
  };
  let crumb = back(
    "connection-crumb",
    (&ctx.t("project_settings.connections"), None),
    actions.back.clone(),
  );
  let status = status_line(
    ctx,
    "connection-inspector-status",
    parts.status,
    parts.status_detail.as_deref(),
  );
  let mut head_actions = parts.actions;
  if !parts.menu.is_empty() {
    head_actions.push(more_menu(ctx, actions, parts.menu));
  }
  let head = entity_head(
    ctx,
    "connection-inspector-header",
    (parts.glyph, parts.title),
    status.into(),
    head_actions,
  );
  let failure = inspecting
    .failed
    .map(|failure| Element::from(field_error("connection-work-error", &message_text(ctx, failure))));
  Column::new()
    .id("connection-inspector")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Roomy.role())
    .describe("connection", inspecting.entry.id().to_string())
    .child(crumb)
    .child(head)
    .with_children(failure)
    .with_children(parts.notices)
    .with_children(parts.groups)
    .with_children(parts.question)
    .into()
}

/// The … button and the menu it opens at its end.
fn more_menu(ctx: &mut Ctx, actions: &Actions, items: Vec<MenuItem>) -> Element {
  let (open, anchor) = actions.more.clone();
  let toggle = open.clone();
  let button = IconButton::new("connection-more", Icon::Ellipsis, ctx.t("connections.more"))
    .element(anchor.clone())
    .on_click(move || toggle.update(|open| *open = !*open))
    .render(ctx);
  let panel = items.into_iter().fold(
    menu_panel("connection-more-menu").width(Dimension::Px(MENU_WIDTH)),
    |panel, item| panel.child(item.render(ctx, &open)),
  );
  Row::new().child(button).child(menu_at_end(anchor, open, panel)).into()
}

/// The connection's name as the head shows it.
fn title(name: &str) -> Text {
  Text::new(name)
    .id("connection-inspector-title")
    .variant(ProjectSettingsText::PageTitle.role())
}

/// A status as a dot of its tone and its word, with what follows it (`Status` `l1Gh8O`).
pub(super) fn status_line(ctx: &Ctx, id: &str, status: Shown, detail: Option<&str>) -> Row {
  let word = ctx.t(status.key);
  let said = match detail {
    Some(detail) => ctx.t_args("connections.statusWith", [("status", &*word), ("detail", detail)]),
    None => word.clone(),
  };
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("status", word)
    .child(icon(status.icon, IconSize::Sm, status.tone.color()))
    .child(
      Text::new(&said)
        .variant(TypographyStyle::Description)
        .color(status.tone.word_color())
        .nowrap(),
    )
}

/// The status a connection shows: checking while its check runs.
fn shown(inspecting: &Inspecting<'_>, status: Shown) -> Shown {
  if inspecting.running == Some(Work::Check) {
    CHECKING
  } else {
    status
  }
}

/// What the status means and what to do about it, under the head, in `tone`.
fn sentence(id: &str, words: &str, tone: PaletteColor) -> Element {
  Text::new(words)
    .id(id.to_owned())
    .variant(ProjectSettingsText::Hint.role())
    .color(tone)
    .width(Dimension::Pct(100.0))
    .into()
}

/// Check again or Refresh models on `id`: checking while it runs, unavailable while other work does.
fn check_button(inspecting: &Inspecting<'_>, (id, label): (&str, Arc<str>), actions: &Actions) -> ActionButton {
  let button = ActionButton::new(id.to_owned(), ButtonKind::Secondary, label).icon(Icon::RefreshCw);
  match inspecting.running {
    Some(Work::Check) => button.loading(),
    Some(_) => button.disabled(),
    None => {
      let (start, connection) = (actions.start.clone(), inspecting.entry.id());
      button.on_click(move || start(connection, Work::Check))
    }
  }
}

/// *Rename* in the … menu: the title becomes a field; unavailable while other work runs on the connection.
fn rename_item(ctx: &Ctx, inspecting: &Inspecting<'_>, actions: &Actions) -> MenuItem {
  if inspecting.running.is_some() {
    return MenuItem::unavailable(
      "connection-rename",
      ctx.t("connections.rename.action"),
      ctx.t("connections.busy"),
    )
    .icon(Icon::Pencil);
  }
  let renaming = actions.renaming.clone();
  MenuItem::new("connection-rename", ctx.t("connections.rename.action"), move || {
    renaming.set(true)
  })
  .icon(Icon::Pencil)
}

/// When the connection was last checked, or that it was not.
fn last_checked(ctx: &Ctx, now: Timestamp, at: Option<Timestamp>) -> Arc<str> {
  match at {
    Some(at) => message_text(ctx, &display::elapsed(now, at)),
    None => ctx.t("connections.notCheckedYet"),
  }
}

/// *Used by* (`Used by` `k0X4BW`): the profiles that choose the connection, with Agents ↗; or that none does.
fn used_by_row(ctx: &mut Ctx, labels: &[String], actions: &Actions, none_key: &str) -> Element {
  let (label, hint) = if labels.is_empty() {
    (ctx.t(none_key), ctx.t("connections.usedByHintNone"))
  } else {
    (labels.join(", ").into(), ctx.t("connections.usedByHint"))
  };
  let agents = ActionButton::new(
    "connection-open-agents",
    ButtonKind::Ghost,
    ctx.t("connections.agentsLink"),
  )
  .icon(Icon::ArrowUpRight)
  .compact()
  .opens(actions.agents.clone());
  let row = SettingRow::new("connection-used-by", label)
    .hint(hint)
    .control(agents)
    .render(ctx);
  crate::screens::project_settings::kit::Group::new("connection-used-by-group", ctx.t("connections.usedBy"))
    .row(row)
    .into()
}

/// A row of a page's details: its label, its value at the end, in Geist Mono for a path or a command.
fn detail_row(ctx: &mut Ctx, id: &str, (label, value): (&str, &str), mono: bool) -> Element {
  let value = crate::screens::project_settings::kit::value(&format!("{id}-value"), value);
  let value = if mono {
    value
      .variant(crate::theme::ExtraTypography::PropertyMono.role())
      .color(PaletteColor::TextSecondary)
  } else {
    value
  };
  SettingRow::new(id.to_owned(), label).control(value).render(ctx)
}

/// The tone a page's sentence is said in: `warning` for a status that asks for something, else `text_muted`.
fn sentence_tone(status: Shown) -> PaletteColor {
  match status.tone {
    super::words::Tone::Warning | super::words::Tone::Danger => PaletteColor::Warning,
    _ => PaletteColor::TextMuted,
  }
}
