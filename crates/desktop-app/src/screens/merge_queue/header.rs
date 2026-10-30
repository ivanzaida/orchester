//! The head of the Merge queue screen (`Info` `IqFXZ` in 05 `S1pTL`): what the queue does — the Review hub's head and
//! tab name the page (`screens::review`) — and at its end how the person's checkout stands: a 6 dot (`success` while
//! it keeps no lane out, `warning` otherwise), *Checkout*, the branch it has and whether it is clean. Its path is in
//! the blocked notice. Under it, while delivery is blocked, the notice that says why with *Open folder* and *Retry*
//! (`mM0HY`).

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::merge_queue::{Blocked, QueueState};
use orchester_workspace::merge_queue::{BaseState, CheckoutState, QueueRead};

use super::actions::QueueActions;
use crate::{
  theme::{DeliveryText, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// The size of the checkout's dot and of a lane's (`Dot` `fBuea`, `L74dei`).
pub(super) const DOT: f32 = 6.0;

pub(super) fn header(ctx: &Ctx, read: &QueueRead) -> Row {
  Row::new()
    .id("merge-queue-header")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("delivery.queue.leadLanes"))
        .id("merge-queue-lead")
        .variant(DeliveryText::Lead.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(read.base.as_ref().map(|base| checkout(ctx, base)))
}

/// A 6 dot of `color` (`Dot` `fBuea`).
pub(super) fn dot(color: PaletteColor) -> Row {
  Row::new()
    .width(Dimension::Px(DOT))
    .height(Dimension::Px(DOT))
    .flex_shrink(0.0)
    .rounded(ExtraRadius::Full.role())
    .background(color)
}

/// How the checkout stands (`Checkout` `yZyeK`).
fn checkout(ctx: &Ctx, base: &BaseState) -> Row {
  // A checkout the runs land beside does not keep delivery out either.
  let clean = matches!(base.checkout, CheckoutState::Clean | CheckoutState::Beside(_));
  let mark = if clean {
    PaletteColor::Success
  } else {
    PaletteColor::Warning
  };
  let (state, state_color) = checkout_words(ctx, base);
  Row::new()
    .id("merge-queue-checkout")
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("checkout", if clean { "clean" } else { "not_clean" })
    .child(dot(mark))
    .child(
      Text::new(&ctx.t("delivery.queue.checkout.key"))
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(
      Text::new(&state)
        .id("merge-queue-checkout-state")
        .variant(ExtraTypography::Property.role())
        .color(state_color)
        .nowrap(),
    )
}

/// The branch the checkout has and how it stands, for `base`'s lane.
fn checkout_words(ctx: &Ctx, base: &BaseState) -> (Arc<str>, PaletteColor) {
  let on = [("branch", base.branch.as_str().to_owned())];
  match &base.checkout {
    CheckoutState::Clean => (
      ctx.t_args("delivery.queue.checkout.clean", on),
      PaletteColor::TextSecondary,
    ),
    CheckoutState::Uncommitted(files) if files.len() == 1 => (
      ctx.t_args("delivery.queue.checkout.uncommittedOne", on),
      PaletteColor::Warning,
    ),
    CheckoutState::Uncommitted(files) => (
      ctx.t_args(
        "delivery.queue.checkout.uncommittedOther",
        [
          ("branch", base.branch.as_str().to_owned()),
          ("count", files.len().to_string()),
        ],
      ),
      PaletteColor::Warning,
    ),
    CheckoutState::InProgress => (
      ctx.t_args("delivery.queue.checkout.inProgress", on),
      PaletteColor::Warning,
    ),
    CheckoutState::OtherBranch(Some(branch)) => (
      ctx.t_args("delivery.queue.checkout.on", [("branch", branch.as_str())]),
      PaletteColor::Warning,
    ),
    CheckoutState::Beside(Some(branch)) => (
      ctx.t_args("delivery.queue.checkout.on", [("branch", branch.as_str())]),
      PaletteColor::TextSecondary,
    ),
    CheckoutState::OtherBranch(None) => (ctx.t("delivery.queue.checkout.detached"), PaletteColor::Warning),
    CheckoutState::Beside(None) => (ctx.t("delivery.queue.checkout.detached"), PaletteColor::TextSecondary),
    CheckoutState::Unknown => (ctx.t("delivery.queue.checkout.unknown"), PaletteColor::TextMuted),
  }
}

/// While the run being delivered is blocked: why, with *Open folder* and *Retry*.
pub(super) fn blocked(
  ctx: &Ctx,
  read: &QueueRead,
  (root, checkout): (&Path, &Arc<str>),
  actions: &QueueActions,
) -> Option<Element> {
  let blocked = read.rows.iter().find_map(|row| match &row.entry.state {
    QueueState::Blocked(blocked) => Some((blocked, row)),
    _ => None,
  })?;
  let (blocked, row) = blocked;
  let base = super::words::base(row);
  let path = &**checkout;
  let (title, body) = match blocked {
    Blocked::Uncommitted { .. } => (
      ctx.t_args("delivery.queue.blocked.uncommittedTitle", [("base", base)]),
      ctx.t_args("delivery.queue.blocked.uncommittedBody", [("path", path)]),
    ),
    Blocked::OtherBranch { .. } => (
      ctx.t_args("delivery.queue.blocked.otherBranchTitle", [("base", base)]),
      ctx.t_args(
        "delivery.queue.blocked.otherBranchBody",
        [("base", base), ("path", path)],
      ),
    ),
    Blocked::InProgress => (
      ctx.t("delivery.queue.blocked.inProgressTitle"),
      ctx.t_args("delivery.queue.blocked.inProgressBody", [("path", path)]),
    ),
    Blocked::HeldLocally => (
      ctx.t("delivery.queue.blocked.heldTitle"),
      ctx.t("delivery.queue.blocked.heldBody"),
    ),
  };
  let folder = root.to_path_buf();
  let open_folder = ActionButton::new(
    "merge-queue-open-folder",
    ButtonKind::Secondary,
    ctx.t("delivery.queue.blocked.openFolder"),
  )
  .on_click(move || open_folder(&folder));
  let retry = actions.retries(
    ActionButton::new(
      "merge-queue-retry",
      ButtonKind::Primary,
      ctx.t("delivery.queue.blocked.retry"),
    )
    .icon(Icon::RotateCcw),
  );
  Some(
    Notice::new("merge-queue-blocked", NoticeTone::Warning, title)
      .icon(Icon::CirclePause)
      .body(body)
      .action(open_folder)
      .action(retry)
      .into(),
  )
}

/// Opens the person's checkout in the system's file manager, on a thread of its own.
fn open_folder(folder: &Path) {
  let folder = folder.to_path_buf();
  let opened = std::thread::Builder::new()
    .name("open-checkout".to_owned())
    .spawn(move || {
      if let Err(error) = orchester_exec::external::open_path(&folder) {
        tracing::warn!(%error, "the system did not open the checkout's folder");
      }
    });
  if let Err(error) = opened {
    tracing::error!(%error, "cannot start opening the checkout's folder");
  }
}
