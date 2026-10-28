//! *Waiting for you* (`Waiting for you` `N72RI` in 01 `LXhun`), in the Orchestrator's digest pattern: a `border` frame,
//! radius 10, its head 36 high on `surface_raised` (a 13 `hand` in `warning`, *6 waiting for you* in 12.5/500 and
//! *oldest first*), then a 34 line per thing that waits, oldest first: its icon, what waits in 12.5 `text_secondary`,
//! its task and run in Geist Mono 11.5, how long in Geist Mono 11 and the one link that acts on it, to the same place
//! the Needs you panel opens (`navigation::waiting`). One that leads nowhere says why in place of its link. At most six
//! lines; *Show all N in Tasks* opens Tasks on Needs you. What waits and its count are every Needs you's
//! (`orchester_workspace::tasks::attention`), the title bar's pill too.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::{
  overview::WAITS_SHOWN,
  tasks::attention::{NeedItem, NeedKind},
};

use super::section::{age, link, mono_time, muted, show_all};
use crate::{
  navigation::{Navigation, waiting::Destination},
  routes::{ProjectPage, Route, TaskList, TaskListTab},
  theme::{ExtraSpacing, ExtraTypography, IconSize, OverviewText},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::Press,
  },
};

/// The head's height (`l4C6vQ`) and a line's (`TyACW`).
const HEAD_HEIGHT: f32 = 36.0;
const LINE_HEIGHT: f32 = 34.0;

/// The digest of `items`, oldest first.
pub(super) fn digest(ctx: &mut Ctx, project: ProjectId, items: &[NeedItem], now: Timestamp) -> Column {
  let count = items.len();
  let title = if count == 1 {
    ctx.t("overview.waits.title.one")
  } else {
    ctx.t_args("overview.waits.title.other", [("count", count.to_string())])
  };
  let lines: Vec<Element> = items
    .iter()
    .take(WAITS_SHOWN)
    .enumerate()
    .map(|(index, item)| wait_line(ctx, project, (index, item), now).into())
    .collect();
  let more = (count > WAITS_SHOWN).then(|| {
    let label = ctx.t_args("overview.waits.showAll", [("count", count.to_string())]);
    let open = Navigation::of(ctx).to(Route::Project(
      project,
      ProjectPage::Tasks(TaskList::new(TaskListTab::NeedsYou)),
    ));
    show_all("overview-waits-all", &label, Press::Open(open))
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
  });
  frame("overview-waits")
    .describe("count", count.to_string())
    .child(digest_head(
      "overview-waits",
      &title,
      Some(&ctx.t("overview.waits.order")),
    ))
    .with_children(lines)
    .with_children(more)
}

/// The digest's frame: a `border` stroke, radius 10.
pub(super) fn frame(id: &str) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
}

/// The digest's head: its hand, `title` and `meta`.
pub(super) fn digest_head(id: &str, title: &str, meta: Option<&str>) -> Row {
  Row::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .child(icon(Icon::Hand, IconSize::Base, PaletteColor::Warning))
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(OverviewText::Head.role())
        .nowrap(),
    )
    .with_children(meta.map(muted))
}

/// The `index`th thing that waits, under `overview-wait-<index>-<task>-<kind>`.
fn wait_line(ctx: &mut Ctx, project: ProjectId, (index, item): (usize, &NeedItem), now: Timestamp) -> Row {
  let key = item.task().map_or("unknown", |task| &task.key);
  let id = format!("overview-wait-{index}-{key}-{}", kind_name(item.kind));
  let target = match (item.task(), item.waiting.run) {
    (Some(task), Some((_, number))) => Some(ctx.t_args(
      "overview.waits.target",
      [("task", task.key.to_string()), ("run", number.get().to_string())],
    )),
    (Some(task), None) => Some(Arc::from(&*task.key)),
    (None, _) => None,
  };
  let end: Element = match Destination::of(project, item) {
    Ok(destination) => {
      let navigation = Navigation::of(ctx);
      let open = Arc::new(move |at| destination.open(&navigation, at));
      link(&format!("{id}-open"), &ctx.t(link_key(item.kind)), Press::Open(open))
    }
    Err(reason) => muted(&message_text(ctx, &reason)).id(format!("{id}-reason")).into(),
  };
  Row::new()
    .id(id.clone())
    .describe("kind", kind_name(item.kind))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LINE_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph(item.kind), IconSize::Base, tone(item.kind)))
    .child(
      Text::new(&message_text(ctx, &item.title))
        .id(format!("{id}-what"))
        .variant(OverviewText::Line.role())
        .color(PaletteColor::TextSecondary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Text::new(target.as_deref().unwrap_or_default())
        .variant(ExtraTypography::MonoKey.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(mono_time(&format!("{id}-age"), &age(ctx, item.since(), now)))
    .child(end)
}

/// A kind's mark (`XHPE6`, `snYMY`, `k2Kk3x` …): a merge waits in `success`, the rest in `warning`.
const fn glyph(kind: NeedKind) -> Icon {
  match kind {
    NeedKind::Approval => Icon::ShieldAlert,
    NeedKind::Question => Icon::MessageCircleQuestion,
    NeedKind::PlanReview => Icon::Map,
    NeedKind::Decision | NeedKind::Input => Icon::Hand,
    NeedKind::StoppedAtLimit => Icon::Gauge,
    NeedKind::Interrupted | NeedKind::DeliveryBlocked => Icon::CirclePause,
    NeedKind::ReadyToMerge => Icon::GitMerge,
    NeedKind::ReadyToComplete => Icon::FileText,
    NeedKind::CommittedNothing => Icon::TriangleAlert,
    NeedKind::Conflicts => Icon::GitCompareArrows,
    NeedKind::DeliveryStopped => Icon::GitBranch,
    NeedKind::Memory => Icon::Brain,
  }
}

const fn tone(kind: NeedKind) -> PaletteColor {
  match kind {
    NeedKind::ReadyToMerge => PaletteColor::Success,
    _ => PaletteColor::Warning,
  }
}

/// The one link that acts on a kind.
const fn link_key(kind: NeedKind) -> &'static str {
  match kind {
    NeedKind::Approval => "overview.waits.link.review",
    NeedKind::Question | NeedKind::Input => "overview.waits.link.answer",
    NeedKind::PlanReview => "overview.waits.link.reviewPlan",
    NeedKind::Decision | NeedKind::CommittedNothing => "overview.waits.link.decide",
    NeedKind::StoppedAtLimit => "overview.waits.link.raiseLimit",
    NeedKind::Interrupted | NeedKind::DeliveryStopped => "overview.waits.link.summary",
    NeedKind::ReadyToMerge => "overview.waits.link.merge",
    NeedKind::ReadyToComplete => "overview.waits.link.complete",
    NeedKind::Conflicts => "overview.waits.link.resolve",
    NeedKind::DeliveryBlocked => "overview.waits.link.mergeQueue",
    NeedKind::Memory => "overview.waits.link.records",
  }
}

/// The kind's name for tools and element ids.
pub(super) const fn kind_name(kind: NeedKind) -> &'static str {
  match kind {
    NeedKind::Approval => "approval",
    NeedKind::Question => "question",
    NeedKind::PlanReview => "plan",
    NeedKind::Decision => "decision",
    NeedKind::Input => "input",
    NeedKind::StoppedAtLimit => "limit",
    NeedKind::Interrupted => "interrupted",
    NeedKind::ReadyToMerge => "merge",
    NeedKind::ReadyToComplete => "complete",
    NeedKind::CommittedNothing => "committed-nothing",
    NeedKind::Conflicts => "conflicts",
    NeedKind::DeliveryStopped => "delivery-stopped",
    NeedKind::DeliveryBlocked => "delivery-blocked",
    NeedKind::Memory => "memory",
  }
}

/// The digest's head while the tasks are read: *Waiting for you*, its count unknown.
pub(super) fn reading_head(ctx: &Ctx) -> Row {
  digest_head("overview-waits", &ctx.t("overview.waits.loading"), None)
}
