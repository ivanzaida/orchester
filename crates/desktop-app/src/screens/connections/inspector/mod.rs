//! The inspector beside the list (`Inspector · Claude · Kontur` `VIpsx`): the connection's header
//! (`Connection inspector header` `GOSxB`: a 32 glyph tile, its name in 14/600 and its status, with an action
//! at the end for an MCP server), its details in a body that scrolls, and a footer pinned under them that says
//! what its status means and holds its actions. Each kind fills it: an agent provider (`agent`), GitHub
//! (`github`), an MCP server (`mcp`).

mod agent;
mod github;
mod mcp;

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ConnectionId, time::Timestamp};
use orchester_workspace::{connections::data::ProjectConnections, display, messages::Message};

use super::{
  data::Entry,
  words::{CHECKING, Shown},
  work::Work,
};
use crate::{
  theme::{ConnectionsText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    inspector,
    message::message_text,
  },
};

/// The inspector's width (`VIpsx`) and its glyph tile (`Glyph` `O38ZPI`).
pub(super) const INSPECTOR_WIDTH: f32 = 360.0;
const GLYPH_TILE: f32 = 32.0;

/// A question the inspector asks in place before it acts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Confirm {
  /// Turn off an MCP server that profiles choose (`Confirm · Turn off` `D2FVJJ`).
  TurnOff,
  /// Remove an MCP server.
  Remove,
  /// Turn on an MCP server, which widens what the agents of the profiles that choose it get (`widen`).
  TurnOn,
}

impl lurq::app::component::DevtoolsInspectable for Confirm {}

/// What the inspector shows about the work on its connection.
pub(super) struct Inspecting<'a> {
  pub listed: &'a ProjectConnections,
  pub entry: Entry<'a>,
  pub project: &'a str,
  /// The work running on the connection now.
  pub running: Option<Work>,
  /// Why the last work on the connection failed.
  pub failed: Option<&'a Message>,
  pub confirming: Option<Confirm>,
  pub now: Timestamp,
}

/// What the inspector's actions do.
#[derive(Clone)]
pub(super) struct Actions {
  pub start: Arc<dyn Fn(ConnectionId, Work) + Send + Sync>,
  pub confirm: Arc<dyn Fn(Option<Confirm>) + Send + Sync>,
  pub edit_mcp: Arc<dyn Fn(ConnectionId) + Send + Sync>,
  /// Opens the Agents screen, where a profile chooses its connection and servers.
  pub agents: Arc<dyn Fn() + Send + Sync>,
}

/// The inspector of `inspecting.entry`.
pub(super) fn inspector(ctx: &mut Ctx, inspecting: &Inspecting<'_>, actions: &Actions) -> Element {
  let parts = match inspecting.entry {
    Entry::Agent(connection) => agent::parts(ctx, connection, inspecting, actions),
    Entry::GitHub(connection) => github::parts(ctx, connection, inspecting, actions),
    Entry::Mcp(server) => mcp::parts(ctx, server, inspecting, actions),
  };
  let body = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Md)
    .with_children(parts.body);
  Column::new()
    .id("connection-inspector")
    .width(Dimension::Px(INSPECTOR_WIDTH))
    .height(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .describe("connection", inspecting.entry.id().to_string())
    .child(parts.header)
    .child(
      ScrollVertical::new(body)
        .id("connection-inspector-body")
        .width(Dimension::Pct(100.0))
        .flex(1.0),
    )
    .with_children(parts.footer)
    .into()
}

/// An inspector's parts as a kind fills them.
pub(super) struct Parts {
  pub header: Element,
  pub body: Vec<Element>,
  pub footer: Option<Element>,
}

/// The header: the glyph tile, the name and the status, and `action` at the end, if any.
fn header(ctx: &Ctx, (glyph, name): (Icon, &str), status: Shown, action: Option<Element>) -> Element {
  let tile = Row::new()
    .width(Dimension::Px(GLYPH_TILE))
    .height(Dimension::Px(GLYPH_TILE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(glyph, IconSize::Nav, PaletteColor::TextSecondary));
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Text::new(name)
        .id("connection-inspector-title")
        .variant(ConnectionsText::InspectorTitle.role()),
    )
    .child(status_line(ctx, "connection-inspector-status", status, IconSize::Sm));
  Row::new()
    .id("connection-inspector-header")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(tile)
    .child(text)
    .with_children(action)
    .into()
}

/// A status as an icon of `size` and its word, 5 apart (`Status line` `YUNCc`).
pub(super) fn status_line(ctx: &Ctx, id: &str, status: Shown, size: IconSize) -> Row {
  let word = ctx.t(status.key);
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .describe("status", word.clone())
    .child(icon(status.icon, size, status.tone.color()))
    .child(
      Text::new(&word)
        .variant(TypographyStyle::Label)
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

/// The footer: what the status means, `more` under it, the rows of actions, and why one cannot be used.
fn footer(
  ctx: &Ctx,
  inspecting: &Inspecting<'_>,
  (sentence, more): (&str, Vec<Element>),
  (rows, reasons): (Vec<Vec<Element>>, Vec<Element>),
) -> Element {
  let failure = inspecting
    .failed
    .map(|failure| field_error("connection-work-error", &message_text(ctx, failure)));
  Column::new()
    .id("connection-inspector-footer")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(sentence)
        .id("connection-inspector-sentence")
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .with_children(more)
    .with_children(failure)
    .with_children(rows.into_iter().enumerate().map(|(index, actions)| {
      Row::new()
        .id(format!("connection-inspector-actions-{index}"))
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(actions)
    }))
    .with_children(reasons)
    .into()
}

/// Test connection or Check again on `id`: checking while it runs, unavailable while other work does.
fn check_button(inspecting: &Inspecting<'_>, (id, label): (&str, Arc<str>), actions: &Actions) -> ActionButton {
  let button = ActionButton::new(id.to_owned(), ButtonKind::Secondary, label).icon(Icon::PlugZap);
  match inspecting.running {
    Some(Work::Check) => button.loading(),
    Some(_) => button.disabled(),
    None => {
      let (start, connection) = (actions.start.clone(), inspecting.entry.id());
      button.on_click(move || start(connection, Work::Check))
    }
  }
}

/// When the connection was last checked, or that it was not.
fn last_checked(ctx: &Ctx, now: Timestamp, at: Option<Timestamp>) -> (Arc<str>, inspector::PropertyValue) {
  match at {
    Some(at) => (
      message_text(ctx, &display::elapsed(now, at)),
      inspector::PropertyValue::Plain,
    ),
    None => (ctx.t("connections.notCheckedYet"), inspector::PropertyValue::Muted),
  }
}

/// The profiles that use a connection, as links to the Agents screen, with how many there are; or that
/// none does.
fn used_by(ctx: &Ctx, labels: Vec<String>, actions: &Actions, none_key: &str) -> Element {
  let count = labels.len();
  let meta = match count {
    1 => ctx.t("connections.usage.one"),
    _ => ctx.t_args("connections.usage.other", [("count", count.to_string())]),
  };
  let section = inspector::section("connection-used-by", ExtraSpacing::Snug.role()).child(inspector::section_title(
    "connection-used-by-title",
    &ctx.t("connections.usedBy"),
    (count > 0).then_some(&*meta),
  ));
  if labels.is_empty() {
    return section
      .child(Text::new(&ctx.t(none_key)).variant(ExtraTypography::Note.role()))
      .into();
  }
  section
    .with_children(labels.into_iter().enumerate().map(|(index, label)| {
      inspector::link(
        &format!("connection-used-by-{index}"),
        Icon::Bot,
        &label,
        Some(actions.agents.clone()),
      )
    }))
    .into()
}
