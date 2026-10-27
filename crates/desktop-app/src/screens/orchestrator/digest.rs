//! What waited for the person while they were away (`Digest` `ZEubG` in `01 Conversation` `wRSL4`): the waits of the
//! project's agents and the ends of its runs that started no turn of the Orchestrator's, kept one after another with
//! no message of the person's between them, gathered into one `border` frame, radius 10: its head (36 high on
//! `surface_raised`, 12 at its sides, 8 apart: a 13 `hand` in `warning`, *N waiting for you* in 12.5/500, *since* the
//! first's time in 12 `text_muted`, and the chevron that folds it) and one 34 line per item under a `border` line (12
//! at its sides, 10 apart: its icon in `warning`, what waits in 12.5 `text_secondary`, the task and run in Geist Mono
//! 11.5 `text_muted`, the time in Geist Mono 11 and its link). Items the Orchestrator took up are not in it: they start
//! a turn.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::{AgentWait, RunEnd, WaitKind},
  time::Timestamp,
};
use orchester_workspace::display;

use super::{
  line::{LineLink, link_element},
  run_end, wait,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, OrchestratorText},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The head's height (`Qyvv9`) and an item's (`jSFfy`).
const HEAD_HEIGHT: f32 = 36.0;
const ITEM_HEIGHT: f32 = 34.0;

/// A wait in a digest: its row's key, the wait and when it was kept.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Waited {
  pub(super) key: String,
  pub(super) wait: AgentWait,
  pub(super) at: Option<Timestamp>,
}

/// A digest of `waits`, named `id`, in project `project`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct DigestProps {
  pub(super) id: String,
  pub(super) project: ProjectId,
  pub(super) waits: Vec<Waited>,
}

impl DevtoolsInspectable for DigestProps {}

/// Whether `wait`, which started no turn, waits for the person and so goes in a digest: one the Orchestrator answers
/// by itself starts a turn of its own.
pub(super) const fn for_the_person(wait: &AgentWait) -> bool {
  !wait.automatic
}

pub(super) struct Digest {
  open: Signal<bool>,
}

impl Component for Digest {
  type Props = DigestProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self { open: ctx.signal(true) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DigestProps>().clone();
    let open = self.open.get();
    let head = head(ctx, &props, &self.open);
    let items: Vec<Element> = if open {
      props.waits.iter().map(|waited| item(ctx, &props, waited)).collect()
    } else {
      Vec::new()
    };
    Column::new()
      .id(props.id.clone())
      .describe("count", props.waits.len().to_string())
      .describe("open", open.to_string())
      .width(Dimension::Pct(100.0))
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .clip()
      .child(head)
      .with_children(items)
  }
}

/// The head: how many wait, since when, and the chevron that folds the digest.
fn head(ctx: &Ctx, props: &DigestProps, open: &Signal<bool>) -> Element {
  let count = props.waits.len();
  let title = if count == 1 {
    ctx.t("orchestrator.digest.one")
  } else {
    ctx.t_args("orchestrator.digest.other", [("count", count.to_string())])
  };
  let since = props
    .waits
    .first()
    .and_then(|first| first.at)
    .map(|at| ctx.t_args("orchestrator.digest.since", [("time", display::clock(at))]));
  let toggle = open.clone();
  pressable(move || toggle.update(|open| *open = !*open))
    .id(format!("{}-head", props.id))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .child(icon(Icon::Hand, IconSize::Base, PaletteColor::Warning))
    .child(
      Text::new(&title)
        .id(format!("{}-title", props.id))
        .variant(OrchestratorText::Emphasis.role())
        .nowrap(),
    )
    .with_children(since.map(|since| {
      Text::new(&since)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
    }))
    .child(Spacer::new().flex(1.0))
    .child(icon(
      if open.get() { Icon::ChevronUp } else { Icon::ChevronDown },
      IconSize::Sm,
      PaletteColor::TextMuted,
    ))
    .into()
}

/// One wait's line.
fn item(ctx: &mut Ctx, props: &DigestProps, waited: &Waited) -> Element {
  let id = format!("orchestrator-digest-item-{}", waited.key);
  let (glyph, what, link) = match waited.wait.kind {
    WaitKind::Ended(end) => (
      run_end::look(end).0,
      ctx.t(run_end_words(end)),
      run_end::open_summary(ctx, end, props.project, &waited.wait.task),
    ),
    kind => {
      let named = waited.wait.agent.as_deref().or(waited.wait.step.as_deref());
      let (glyph, key) = wait::look(kind, named.is_some());
      let link = wait::open_card(ctx, props.project, &waited.wait.task).map(|link| LineLink {
        label: ctx.t("orchestrator.line.answer"),
        ..link
      });
      (glyph, ctx.t_args(key, [("agent", named.unwrap_or_default())]), link)
    }
  };
  let target: Arc<str> = wait::run_target(ctx, &waited.wait);
  Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ITEM_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Base, PaletteColor::Warning))
    .child(
      Text::new(&what)
        .variant(OrchestratorText::Line.role())
        .color(PaletteColor::TextSecondary)
        .nowrap()
        .flex_shrink(0.0),
    )
    .child(
      Text::new(&target)
        .variant(ExtraTypography::MonoKey.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(waited.at.map(|at| {
      Text::new(&display::clock(at))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap()
    }))
    .with_children(link.map(|link| link_element(&format!("{id}-open"), link)))
    .into()
}

/// What a digest's line says of a run's end: how it stopped, in a few words.
const fn run_end_words(end: RunEnd) -> &'static str {
  match end {
    RunEnd::Succeeded => "orchestrator.digest.end.succeeded",
    RunEnd::Failed => "orchestrator.digest.end.failed",
    RunEnd::Stopped { .. } => "orchestrator.digest.end.stopped",
    RunEnd::Limit { .. } => "orchestrator.digest.end.limit",
    RunEnd::Interrupted => "orchestrator.digest.end.interrupted",
  }
}
