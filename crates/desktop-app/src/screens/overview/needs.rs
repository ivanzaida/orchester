//! Beside the Overview's tasks, what waits for the person (`Needs you` `NbKDR`): its count, in `warning`
//! while anything waits, over a card for each (`Needs-you card` `rkhZ2`): what it waits for and for how
//! long, what it is, its task, and the way to where it is decided. Nothing waiting is said in a quiet card
//! (`Empty` `yZd0J`); while the tasks are read, a reading line in an outlined card (`R` `NpDgz`). What
//! waits, and in which order, is `orchester_workspace::tasks::attention`.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::dimension::Dimension,
};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::tasks::{
  attention::{NeedItem, NeedKind},
  overview::figure,
};

use super::tasks::reading;
use crate::{
  navigation::{Navigation, waiting::Destination},
  theme::{ExtraSpacing, ExtraTypography, IconSize, OverviewText},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card_frame},
    icon::{Icon, icon},
    message::message_text,
    unavailable::reason_line,
  },
};

/// The column; `items` is `None` while the tasks are read.
pub(super) fn column(ctx: &mut Ctx, project: ProjectId, items: Option<&[NeedItem]>, now: Timestamp) -> Column {
  let (count, color) = match items {
    Some(items) if !items.is_empty() => (figure(items.len()), PaletteColor::Warning),
    Some(_) => (figure(0), PaletteColor::TextMuted),
    None => (ctx.t("overview.unknown").to_string(), PaletteColor::TextMuted),
  };
  let head = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("overview.needs.title")).variant(TypographyStyle::Title))
    .child(
      Text::new(&count)
        .id("overview-needs-count")
        .variant(TypographyStyle::Description)
        .color(color),
    );
  let column = Column::new()
    .id("overview-needs")
    .spacing(ExtraSpacing::Loose.role())
    .child(head);
  match items {
    None => column.child(
      card_frame("overview-needs-reading", CardSurface::Outline)
        .child(reading(&ctx.t("overview.needs.reading"), "overview-needs-reading-line")),
    ),
    Some([]) => column.child(nothing_waits(ctx)),
    Some(items) => column.with_children(
      items
        .iter()
        .enumerate()
        .map(|(index, item)| need_card(ctx, project, (index, item), now)),
    ),
  }
}

/// Nothing waits for the person.
fn nothing_waits(ctx: &Ctx) -> Column {
  card_frame("overview-needs-empty", CardSurface::Outline)
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Snug.role())
    .child(icon(Icon::CircleCheck, IconSize::Lg, PaletteColor::Success))
    .child(Text::new(&ctx.t("overview.needs.emptyTitle")).variant(ExtraTypography::Strong.role()))
    .child(
      Text::new(&ctx.t("overview.needs.emptyDetail"))
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0)),
    )
}

/// One thing waiting, the `index`th: its kind and wait, what it is, its task, and its action, which leads where
/// the Needs you panel's does and points at the same card. What leads nowhere yet (the project's memory, a
/// request whose task cannot be read) says why in place of its action.
fn need_card(ctx: &mut Ctx, project: ProjectId, (index, item): (usize, &NeedItem), now: Timestamp) -> Column {
  let key = item.task().map_or("unknown", |task| &task.key);
  let id = format!("overview-need-{index}-{key}-{}", kind_name(item.kind));
  let waited = message_text(ctx, &item.wait_line(now, |time| message_text(ctx, time).to_string()));
  let top = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(glyph(item.kind), IconSize::Base, PaletteColor::Warning))
    .child(Text::new(&message_text(ctx, &item.kind.label())).variant(OverviewText::Kind.role()))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&waited).variant(TypographyStyle::Caption).nowrap());
  let card = card_frame(&id, CardSurface::Raised)
    .padding(ExtraSpacing::Inset.role())
    .spacing(SpacingSize::Sm)
    .describe("kind", kind_name(item.kind))
    .child(top)
    .child(
      Text::new(&message_text(ctx, &item.title))
        .variant(ExtraTypography::Strong.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(Text::new(&message_text(ctx, &item.context())).variant(ExtraTypography::Note.role()));
  let button = ActionButton::new(
    format!("{id}-action"),
    ButtonKind::Secondary,
    message_text(ctx, &item.kind.action()),
  )
  .icon(action_icon(item.kind));
  match Destination::of(project, item) {
    Ok(destination) => {
      let navigation = Navigation::of(ctx);
      card.child(Row::new().child(button.opens(Arc::new(move |at| destination.open(&navigation, at)))))
    }
    Err(reason) => {
      let reason = message_text(ctx, &reason);
      card
        .child(Row::new().child(button.unavailable(reason.clone())))
        .child(reason_line(&format!("{id}-reason"), &reason))
    }
  }
}

/// A kind's mark (`Kind icon` `y6Zq0D`).
const fn glyph(kind: NeedKind) -> Icon {
  match kind {
    NeedKind::Approval => Icon::ShieldAlert,
    NeedKind::Question => Icon::MessageCircleQuestion,
    NeedKind::PlanReview => Icon::Map,
    NeedKind::Decision | NeedKind::Input => Icon::Hand,
    NeedKind::StoppedAtLimit => Icon::Gauge,
    NeedKind::Interrupted => Icon::CirclePause,
    NeedKind::ReadyToMerge => Icon::GitMerge,
    NeedKind::Memory => Icon::Brain,
  }
}

/// The action's glyph: a chevron for a request and a question, answered in their card of the
/// conversation (Review, Answer in `OuAbC`, `EO87l`), an arrow out to the task for the rest (`g35HoL`).
const fn action_icon(kind: NeedKind) -> Icon {
  match kind {
    NeedKind::Approval | NeedKind::Question => Icon::ChevronDown,
    _ => Icon::ArrowUpRight,
  }
}

/// The kind's name for tools and element ids.
const fn kind_name(kind: NeedKind) -> &'static str {
  match kind {
    NeedKind::Approval => "approval",
    NeedKind::Question => "question",
    NeedKind::PlanReview => "plan",
    NeedKind::Decision => "decision",
    NeedKind::Input => "input",
    NeedKind::StoppedAtLimit => "limit",
    NeedKind::Interrupted => "interrupted",
    NeedKind::ReadyToMerge => "merge",
    NeedKind::Memory => "memory",
  }
}
