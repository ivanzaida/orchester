//! Beside the Overview's tasks, what waits for the person (`Needs you` `NbKDR`): its count, in `warning`
//! while anything waits, over a card for each (`Needs-you card` `rkhZ2`): what it waits for and for how
//! long, what it is, its task, and the way to where it is decided. Nothing waiting is said in a quiet card
//! (`Empty` `yZd0J`); while the tasks are read, a reading line in an outlined card (`R` `NpDgz`). What
//! waits, and in which order, is `orchester_workspace::tasks::attention`.

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
  attention::{NeedItem, NeedKind, Waited},
  overview::figure,
};

use super::{open_task, tasks::reading};
use crate::{
  routes::TaskTab,
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

/// One thing waiting, the `index`th: its kind and wait, what it is, its task, and its action. A request
/// whose task cannot be read says so and has no action: there is no task to open.
fn need_card(ctx: &mut Ctx, project: ProjectId, (index, item): (usize, &NeedItem), now: Timestamp) -> Column {
  let (glyph, action, tab) = look(item.kind);
  let key = item.task.as_ref().map_or("unknown", |task| &task.key);
  let id = format!("overview-need-{index}-{key}-{}", kind_name(item.kind));
  let waited = match item.waited(now) {
    Waited::For(time) => ctx.t_args("overview.needs.waiting", [("time", message_text(ctx, &time))]),
    Waited::Since(time) => ctx.t_args("overview.needs.since", [("time", time)]),
  };
  let top = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, PaletteColor::Warning))
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
  let button =
    ActionButton::new(format!("{id}-action"), ButtonKind::Secondary, ctx.t(action)).icon(action_icon(item.kind));
  match &item.task {
    Some(task) => {
      let open = open_task(ctx, project, &task.key, tab);
      card.child(Row::new().child(button.on_click(open)))
    }
    // No screen reviews memory records yet: the action stays in place and says so, as the Needs you panel's.
    None if item.kind == NeedKind::Memory => {
      let reason = ctx.t("overview.needs.memoryUnavailable");
      card
        .child(Row::new().child(button.unavailable(reason.clone())))
        .child(reason_line(&format!("{id}-reason"), &reason))
    }
    None => card,
  }
}

/// A kind's mark (`Kind icon` `y6Zq0D`), the key of its action's words, and the tab of its task where it
/// is decided: a request and a question are cards of the conversation, the rest is on Summary. The
/// project's memory has no task: its tab is not used.
const fn look(kind: NeedKind) -> (Icon, &'static str, TaskTab) {
  match kind {
    NeedKind::Approval => (Icon::ShieldAlert, "overview.needs.action.review", TaskTab::Conversation),
    NeedKind::Question => (
      Icon::MessageCircleQuestion,
      "overview.needs.action.answer",
      TaskTab::Conversation,
    ),
    NeedKind::PlanReview => (Icon::Map, "overview.needs.action.reviewPlan", TaskTab::Summary),
    NeedKind::Decision => (Icon::Hand, "overview.needs.action.openTask", TaskTab::Summary),
    NeedKind::Input => (Icon::Hand, "overview.needs.action.openTask", TaskTab::Conversation),
    NeedKind::StoppedAtLimit => (Icon::Gauge, "overview.needs.action.raiseLimit", TaskTab::Summary),
    NeedKind::Interrupted => (Icon::CirclePause, "overview.needs.action.openTask", TaskTab::Summary),
    NeedKind::ReadyToMerge => (Icon::GitMerge, "overview.needs.action.reviewMerge", TaskTab::Summary),
    NeedKind::Memory => (Icon::Brain, "overview.needs.action.reviewRecords", TaskTab::Summary),
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
