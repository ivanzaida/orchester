//! The decision above *Run details* (`Decision` `c5VJ9A` in `wN5Bz`, `Namyk` in `hKct7`): a 16 glyph and its title,
//! what it is about, and its buttons as wide as the card, with what each does under it; 16 inside, 10 apart,
//! `surface_raised` in a 10 corner. *Ready to merge into main*: how it merges and what is cleared after it, *Merge into
//! main*, *Open changes* and *Reject*. *Conflicts need you*: *Resolve myself* (*Continue resolving* while the merge
//! waits in the run's worktree) and *Reject*. *Delivery stopped*: why, and *Take out of the queue*, after which the run
//! is merged from its task page as any run.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{merge_queue::Left, resource::MergeMethod};
use orchester_workspace::display::short_revision;

use super::{Act, View, card::base};
use crate::{
  navigation::Navigation,
  routes::{Route, TaskTab, TaskView},
  theme::{DeliveryText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    detail_row::{DetailValue, detail_row},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
  },
};

/// *Ready to merge into main*.
pub(super) fn review(ctx: &mut Ctx, view: &View<'_>) -> Element {
  let base = base(view.run);
  let checked = view
    .queue
    .delivery
    .as_ref()
    .and_then(|delivery| delivery.entry.checked.as_ref());
  let note = match checked {
    Some(checked) => ctx.t_args(
      "delivery.task.decision.readyBody",
      [
        ("base", base.to_owned()),
        ("revision", short_revision(checked.base.as_str()).to_owned()),
      ],
    ),
    None => ctx.t("delivery.task.decision.readyNoCheckBody"),
  };
  let method = match view.queue.deliver.method() {
    MergeMethod::MergeCommit => ctx.t("delivery.step.mergeCommit"),
    MergeMethod::Squash => ctx.t("delivery.step.squash"),
  };
  let merge = view.acts.button(
    ActionButton::new(
      "task-delivery-merge",
      ButtonKind::Primary,
      ctx.t_args("decisions.ready.merge", [("branch", base)]),
    )
    .icon(Icon::GitMerge)
    .fill_width(),
    Act::Approve,
  );
  let changes = ActionButton::new(
    "task-delivery-open-changes",
    ButtonKind::Secondary,
    ctx.t("delivery.task.decision.openChanges"),
  )
  .icon(Icon::FileDiff)
  .fill_width()
  .opens(Navigation::of(ctx).to(Route::Project(
    view.task.project.open.project.id(),
    view.task.page(TaskView::of(TaskTab::Changes)),
  )));
  decision(
    ctx,
    view,
    (
      Icon::Hand,
      &ctx.t_args("delivery.task.decision.readyTitle", [("base", base)]),
      &note,
    ),
    vec![
      detail_row(
        "task-delivery-method",
        &ctx.t("delivery.task.details.method"),
        &method,
        DetailValue::Text,
      )
      .into(),
      detail_row(
        "task-delivery-afterwards",
        &ctx.t("delivery.task.decision.afterwards"),
        &ctx.t("delivery.task.decision.afterwardsValue"),
        DetailValue::Text,
      )
      .into(),
      merge.into(),
      changes.into(),
      reject(ctx, view).into(),
      muted(&ctx.t_args("delivery.task.decision.readyNote", [("base", base)])),
    ],
  )
}

/// *Conflicts need you*; `in_merge` while the person's merge waits in the run's worktree, which `back` shows again.
pub(super) fn conflicts(ctx: &mut Ctx, view: &View<'_>, in_merge: bool, back: &Signal<bool>) -> Element {
  let base = base(view.run);
  let resolve = ActionButton::new(
    "task-delivery-resolve-myself",
    ButtonKind::Primary,
    ctx.t(if in_merge {
      "delivery.task.decision.continueResolving"
    } else {
      "delivery.task.decision.resolveMyself"
    }),
  )
  .icon(Icon::FilePenLine)
  .fill_width();
  let resolve = if in_merge {
    let back = back.clone();
    resolve.on_click(move || back.set(false))
  } else {
    view.acts.button(resolve, Act::ResolveMyself)
  };
  decision(
    ctx,
    view,
    (
      Icon::Hand,
      &ctx.t("delivery.task.decision.conflictsTitle"),
      &ctx.t("delivery.task.decision.conflictsBody"),
    ),
    vec![
      resolve.into(),
      muted(&ctx.t("delivery.task.decision.resolveNote")),
      reject(ctx, view).into(),
      muted(&ctx.t_args("delivery.task.decision.rejectNote", [("base", base)])),
    ],
  )
}

/// *Delivery stopped* with why.
pub(super) fn left(ctx: &mut Ctx, view: &View<'_>, left: &Left) -> Element {
  let take_out = view.acts.button(
    ActionButton::new(
      "task-delivery-take-out",
      ButtonKind::Secondary,
      ctx.t("delivery.task.decision.takeOut"),
    )
    .icon(Icon::X)
    .fill_width(),
    Act::Reject,
  );
  let title = if left.failed() {
    ctx.t("delivery.task.decision.failedTitle")
  } else {
    ctx.t("delivery.task.decision.stoppedTitle")
  };
  decision(
    ctx,
    view,
    (
      Icon::TriangleAlert,
      &title,
      &crate::screens::merge_queue::words::left_reason(ctx, left),
    ),
    vec![take_out.into(), muted(&ctx.t("delivery.task.decision.takeOutNote"))],
  )
}

/// *Reject*.
fn reject(ctx: &Ctx, view: &View<'_>) -> ActionButton {
  view.acts.button(
    ActionButton::new(
      "task-delivery-reject",
      ButtonKind::DangerGhost,
      ctx.t("delivery.task.decision.reject"),
    )
    .icon(Icon::X)
    .fill_width(),
    Act::Reject,
  )
}

/// The decision's card, in a `warning` stroke as it waits for the person: its glyph, title and note, then `parts`, and
/// why the last decision did not happen.
fn decision(ctx: &Ctx, view: &View<'_>, (glyph, title, note): (Icon, &str, &str), parts: Vec<Element>) -> Element {
  let refused = view
    .acts
    .refused()
    .map(|refused| field_error("task-delivery-refused", &message_text(ctx, &refused.message)));
  Column::new()
    .id("task-delivery-decision")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Warning))
    .rounded(RadiusSize::Lg)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Md, PaletteColor::Warning))
        .child(
          Text::new(title)
            .id("task-delivery-decision-title")
            .variant(DeliveryText::CardTitle.role()),
        ),
    )
    .child(
      Text::new(note)
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary)
        .width(Dimension::Pct(100.0)),
    )
    .with_children(parts)
    .with_children(refused)
    .into()
}

fn muted(text: &str) -> Element {
  Text::new(text)
    .variant(ExtraTypography::Note.role())
    .color(PaletteColor::TextMuted)
    .width(Dimension::Pct(100.0))
    .into()
}
