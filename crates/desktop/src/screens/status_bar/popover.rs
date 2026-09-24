//! Operations popover (`E1msJS`, anchored example `imT2E`): opens upward from the activity
//! item, 6px above the bar, with the running operations and the session's RECENT events
//! (unresolved failures pinned first). Escape or a click outside closes it.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::{Ctx, Placement},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Rect, Row, ScrollVertical, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};

use super::{
  POPOVER_GAP,
  activity_item::event_text,
  hint::{Anchors, HintAnchor},
  item::{dismiss, glyph, value},
  open::Opener,
  timed::{Elapsed, ElapsedProps},
};
use crate::{
  activity::{ActivityLog, Event, Operation, OperationId, Outcome, Place, Target},
  display,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{icon::Icon, message_text},
};

/// Popover width (`E1msJS`). The recent list scrolls beyond this height; the design shows
/// three events and does not say how a long session looks.
const WIDTH: f32 = 420.0;
const LIST_MAX_HEIGHT: f32 = 360.0;
const DIVIDER_HEIGHT: f32 = 1.0;
/// Width of the chevron column (the 12px icon), kept empty on rows that open nothing so every
/// row's time lines up.
const CHEVRON_WIDTH: f32 = 12.0;

pub fn popover(ctx: &mut Ctx, log: &ActivityLog, opener: &Opener, anchors: &Anchors) -> Element {
  let running: Vec<&Operation> = log.running().collect();
  let recent = log.recent();
  let mut list = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Hair.role());
  for operation in &running {
    list = list.child(running_row(ctx, operation, opener));
  }
  if !recent.is_empty() {
    if !running.is_empty() {
      list = list
        .child(Rect::new(Dimension::Pct(100.0), Dimension::Px(DIVIDER_HEIGHT)).background(ExtraColor::Hairline.role()));
    }
    list = list
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .padding_top(ExtraSpacing::Inline.role())
          .padding_horizontal(SpacingSize::Sm)
          .padding_bottom(SpacingSize::Xs)
          .child(Text::new(&ctx.t("statusBar.operations.recent")).variant(ExtraTypography::Overline.role())),
      )
      .with_children(recent.iter().map(|event| recent_row(ctx, event, opener)));
  }
  let panel = Column::new()
    .id("status-operations")
    .width(Dimension::Px(WIDTH))
    .padding(SpacingSize::Sm)
    .spacing(ExtraSpacing::Hair.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(header(ctx, running.len()))
    .child(
      ScrollVertical::new(list)
        .width(Dimension::Pct(100.0))
        .max_height(Dimension::Px(LIST_MAX_HEIGHT)),
    )
    .child(footer(ctx, opener));
  Popup::new(anchors.get(HintAnchor::Activity).clone(), panel)
    .open(opener.popover().clone())
    .placement(Placement::TopStart)
    .offset(0.0, POPOVER_GAP)
    .into()
}

/// "Background operations" with the number running (`FSYIz`).
fn header(ctx: &Ctx, running: usize) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Tight.role())
    .padding_horizontal(SpacingSize::Sm)
    .padding_bottom(SpacingSize::Sm)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("statusBar.operations.title")).variant(ExtraTypography::NoticeHeading.role()))
    .child(value(&running.to_string(), PaletteColor::TextMuted))
    .into()
}

/// Retention note and "Open runner log" (`sQkff`).
fn footer(ctx: &Ctx, opener: &Opener) -> Element {
  let mut row = Row::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Sm)
    .padding_horizontal(SpacingSize::Sm)
    .padding_bottom(SpacingSize::Xs)
    .justify(Justify::SpaceBetween)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(&ctx.t("statusBar.operations.retention"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    );
  if opener.can_open(&Target::RunnerLog) {
    let log = opener.clone();
    row = row.child(
      Row::new()
        .id("status-operations-open-log")
        .cursor(CursorIcon::Pointer)
        .on_click(move |_| log.open_log())
        .child(Text::new(&ctx.t("runner.connection.openLog")).variant(TypographyStyle::Link)),
    );
  }
  row.into()
}

/// Loader, title, meta, elapsed time and the chevron when it opens something (`ZaltO`).
fn running_row(ctx: &mut Ctx, operation: &Operation, opener: &Opener) -> Element {
  let elapsed = ctx.mount_keyed::<Elapsed>(
    &format!("status-operation-elapsed-{:?}", operation.id()),
    ElapsedProps {
      since: operation.started(),
    },
  );
  let title = message_text(ctx, operation.title());
  let meta = meta(ctx, operation.project(), operation.target());
  row(
    RowParts {
      icon: glyph(Icon::Loader, PaletteColor::TextPrimary),
      title: text_block(&title, meta.as_deref(), PaletteColor::TextPrimary),
      time: elapsed,
    },
    operation.target(),
    None,
    opener,
  )
}

/// Check (or alert) icon, title, meta and the event time (`s7uHum`); an unresolved failure
/// has a dismiss control.
fn recent_row(ctx: &Ctx, event: &Event, opener: &Opener) -> Element {
  let icon = match event.outcome() {
    Outcome::Finished => Icon::Check,
    Outcome::Failed { .. } => Icon::AlertCircle,
  };
  let color = if event.is_unresolved() {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  let title = event_text(ctx, event);
  let meta = meta(ctx, event.project(), event.target());
  let failure = event.is_unresolved().then(|| event.id());
  row(
    RowParts {
      icon: glyph(icon, color.clone()),
      title: text_block(&title, meta.as_deref(), color),
      time: value(&display::clock_time(event.at()), PaletteColor::TextMuted),
    },
    event.target(),
    failure,
    opener,
  )
}

struct RowParts {
  icon: Element,
  title: Element,
  time: Element,
}

fn row(parts: RowParts, target: &Target, failure: Option<OperationId>, opener: &Opener) -> Element {
  let mut row = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Sm)
    .spacing(ExtraSpacing::Inline.role())
    .rounded(RadiusSize::Md)
    .child(parts.icon)
    .child(parts.title)
    .child(parts.time);
  let chevron = Row::new()
    .width(Dimension::Px(CHEVRON_WIDTH))
    .justify(Justify::Center)
    .flex_shrink(0.0);
  if opener.can_open(target) {
    let (target, open) = (target.clone(), opener.clone());
    row = row
      .child(chevron.child(glyph(Icon::ChevronRight, PaletteColor::TextMuted)))
      .cursor(CursorIcon::Pointer)
      .hovered_style(Style::new().background(PaletteColor::AccentMuted))
      .on_click(move |_| open.open(&target, failure));
  } else {
    row = row.child(chevron);
  }
  if let Some(id) = failure {
    let dismissing = opener.clone();
    row = row.child(dismiss("status-operation-dismiss", move || dismissing.dismiss(id)));
  }
  row.into()
}

fn text_block(title: &str, meta: Option<&str>, color: PaletteColor) -> Element {
  let mut block = Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Mark.role())
    .child(Text::new(title).variant(TypographyStyle::FieldLabel).color(color));
  if let Some(meta) = meta {
    block = block.child(
      Text::new(meta)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    );
  }
  block.into()
}

/// "Kontur · opens the pipeline": the project and what clicking opens, as far as known.
fn meta(ctx: &Ctx, project: Option<&str>, target: &Target) -> Option<Arc<str>> {
  let opens = opens_key(target).map(|key| ctx.t(key));
  match (project, opens) {
    (Some(project), Some(opens)) => {
      Some(ctx.t_args("statusBar.operations.meta", [("project", project), ("opens", &*opens)]))
    }
    (Some(project), None) => Some(project.into()),
    (None, opens) => opens,
  }
}

fn opens_key(target: &Target) -> Option<&'static str> {
  Some(match target {
    Target::Nothing => return None,
    Target::RunnerLog => "statusBar.opens.runnerLog",
    Target::Project { place, .. } => match place {
      Place::Overview => "statusBar.opens.overview",
      Place::Tasks => "statusBar.opens.tasks",
      Place::Pipeline(_) => "statusBar.opens.pipeline",
      Place::AgentProfile(_) => "statusBar.opens.agentProfile",
      Place::Connections => "statusBar.opens.connections",
    },
  })
}

/// Label of the action that opens a failure's object ("Open task" in H).
pub fn open_key(target: &Target) -> &'static str {
  match target {
    Target::Nothing | Target::RunnerLog => "runner.connection.openLog",
    Target::Project { place, .. } => match place {
      Place::Overview => "statusBar.open.overview",
      Place::Tasks => "statusBar.open.tasks",
      Place::Pipeline(_) => "statusBar.open.pipeline",
      Place::AgentProfile(_) => "statusBar.open.agentProfile",
      Place::Connections => "statusBar.open.connections",
    },
  }
}
