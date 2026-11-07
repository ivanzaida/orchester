//! What the app writes in an Orchestrator conversation about what it watches, with no agent turn (ORC-777;
//! `docs/design/orchestrator-watch.md`, `04 · Leading two modules` `OKtX2` and `Parts · Lines` `avDZO` a–d): the
//! person's watch command as its bubble ([`command`]), the lines about the conversation's subscriptions ([`lines`]) and
//! the end of one that ended by itself ([`ended`]). None of them is an agent's message.
//!
//! Each line is one row, 4 in at its sides, its parts 10 apart: a 13 icon, its words in 12.5 `text_secondary`, what it
//! adds after them in 12 `text_muted` filling the row, and at its end the time or *Open ↗* in 12 `text_secondary`.

mod command;
mod ended;
mod facts;
mod lines;

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{CursorIcon, Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  orchestrator::{ChatEvent, WatchTarget, watch::NamedTarget},
  time::Timestamp,
};
use orchester_workspace::display;

pub(super) use self::{ended::line as ended_line, facts::WatchFacts};
use super::timeline::TurnEnv;
use crate::{
  navigation::link::Open,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize, OrchestratorText},
  ui::{
    icon::{Icon, icon},
    pressable::opening,
  },
};

/// The row of watch entry `event`, named `id`, kept at `at`: the person's command, or a line about what the
/// conversation watches; `None` for another event.
pub(super) fn row(
  ctx: &mut Ctx,
  id: &str,
  event: &ChatEvent,
  (at, env): (Option<Timestamp>, &TurnEnv),
) -> Option<Element> {
  match event {
    ChatEvent::WatchCommand { command } => Some(command::bubble(ctx, id, command, at)),
    ChatEvent::Watch { line } => lines::line(ctx, id, line, (at, env)),
    _ => None,
  }
}

/// What a line says: its icon in its color, its words, and what it adds after them.
struct Said {
  glyph: (Icon, PaletteColor),
  words: Arc<str>,
  aside: Option<Arc<str>>,
}

/// The row of a line named `id` that says `said`, with `end` at its end; `outcome` is what tools read of it.
fn line_row(id: &str, outcome: &'static str, said: Said, end: Option<Element>) -> Row {
  let (glyph, color) = said.glyph;
  let fill: Element = match said.aside {
    Some(aside) => Text::new(&aside)
      .id(format!("{id}-aside"))
      .variant(OrchestratorText::LineAside.role())
      .nowrap()
      .text_overflow(TextOverflow::Elipsis)
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .into(),
    None => Spacer::new().flex(1.0).into(),
  };
  Row::new()
    .id(id.to_owned())
    .describe("outcome", outcome)
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(SpacingSize::Xs))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color).flex_shrink(0.0))
    .child(
      Text::new(&said.words)
        .id(format!("{id}-words"))
        .variant(OrchestratorText::Line.role())
        .color(PaletteColor::TextSecondary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(fill)
    .with_children(end)
}

/// The time `at` at a line's end, named `{id}-time`.
fn time(id: &str, at: Option<Timestamp>) -> Option<Element> {
  at.map(|at| {
    Text::new(&display::clock(at))
      .id(format!("{id}-time"))
      .variant(OrchestratorText::LineEnd.role())
      .nowrap()
      .flex_shrink(0.0)
      .into()
  })
}

/// Words that open a place with `open`, named `id`, in 12 `text_secondary`, the `hover` fill under the pointer.
fn link(id: &str, words: Arc<str>, open: Open) -> Element {
  let text = Text::new(&words).variant(OrchestratorText::LineEnd.role()).nowrap();
  opening(open)
    .id(id.to_owned())
    .describe("a11y_name", words)
    .flex_shrink(0.0)
    .rounded(ExtraRadius::Link.role())
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .cursor(CursorIcon::Pointer)
    .child(text)
    .into()
}

/// `named` as people read it: its name when it was kept, the app's words for the tasks without a module and the whole
/// project.
fn target_name(ctx: &Ctx, named: &NamedTarget) -> Arc<str> {
  match (&named.name, named.target) {
    (Some(name), _) => Arc::from(name.as_str()),
    (None, WatchTarget::WithoutModule) => ctx.t("orchestrator.watch.target.withoutModule"),
    (None, WatchTarget::Project) => ctx.t("orchestrator.watch.target.project"),
    (None, WatchTarget::Task(_) | WatchTarget::Module(_)) => ctx.t("orchestrator.watch.target.gone"),
  }
}

/// `names` as one list: *A*, *A and B*, *A, B and C*.
fn joined(ctx: &Ctx, names: &[Arc<str>]) -> Arc<str> {
  match names {
    [] => Arc::from(""),
    [one] => one.clone(),
    [first, last] => ctx.t_args("orchestrator.watch.join.pair", [("first", &**first), ("last", &**last)]),
    [first, rest @ ..] => {
      let next = joined(ctx, rest);
      ctx.t_args("orchestrator.watch.join.more", [("first", &**first), ("next", &*next)])
    }
  }
}
