//! The head of the Merge queue screen (`Header` in `SMzTk`): *Merge queue* and what the queue does, and beside them
//! the base branch it delivers into with its head, and how the person's checkout stands (a 6 dot: `success` when it is
//! clean, `warning` otherwise, the box then `warning_muted` in `warning`). Under it, while delivery is blocked, the
//! notice that says why with *Open folder* and *Retry* (`mM0HY`).

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::merge_queue::{Blocked, QueueState};
use orchester_workspace::{
  display::{short_revision, shorten_path},
  merge_queue::{BaseState, CheckoutState, QueueRead},
};

use super::actions::QueueActions;
use crate::{
  theme::{DeliveryText, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
  },
};

/// The size of the checkout's dot (`Dot` in `aYFBa`).
const DOT: f32 = 6.0;
/// Characters of the checkout's path in the base box before its middle gives way to an ellipsis: the box stays the
/// size the design draws it (`XEGEf`, 271 wide with `H:/projects/kontur`) beside a long path too.
const PATH_CHARS: usize = 28;

pub(super) fn header(ctx: &Ctx, read: &QueueRead, checkout: &Arc<str>) -> Row {
  let lead = read.base.as_ref().map_or_else(
    || ctx.t("delivery.queue.leadNoBase"),
    |base| ctx.t_args("delivery.queue.lead", [("base", base.branch.as_str())]),
  );
  Row::new()
    .id("merge-queue-header")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xl)
    .align_items(Alignment::Start)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(Text::new(&ctx.t("delivery.queue.title")).variant(ExtraTypography::Display.role()))
        .child(
          Text::new(&lead)
            .id("merge-queue-lead")
            .variant(DeliveryText::Lead.role()),
        ),
    )
    .with_children(read.base.as_ref().map(|base| base_box(ctx, base, checkout)))
}

fn base_box(ctx: &Ctx, base: &BaseState, checkout: &Arc<str>) -> Column {
  let clean = base.checkout == CheckoutState::Clean;
  // The panel's own fill, or `warning_muted` in `warning` while the checkout keeps delivery out.
  let (fill, stroke, mark) = if clean {
    (PaletteColor::SurfacePanel, PaletteColor::Border, PaletteColor::Success)
  } else {
    (PaletteColor::WarningMuted, PaletteColor::Warning, PaletteColor::Warning)
  };
  let head = base.head.as_ref().map_or_else(
    || ctx.t_args("delivery.queue.base.noHead", [("branch", base.branch.as_str())]),
    |head| {
      ctx.t_args(
        "delivery.queue.base.value",
        [
          ("branch", base.branch.as_str()),
          ("revision", short_revision(head.as_str())),
        ],
      )
    },
  );
  let (state, state_color) = checkout_words(ctx, &base.checkout);
  let key = |key: &str| {
    Text::new(&ctx.t(key))
      .variant(ExtraTypography::Property.role())
      .color(PaletteColor::TextMuted)
  };
  Column::new()
    .id("merge-queue-base")
    .flex_shrink(0.0)
    .padding(Padding::symmetric(
      ExtraSpacing::Loose.role(),
      ExtraSpacing::Inset.role(),
    ))
    .spacing(ExtraSpacing::Snug.role())
    .background(fill)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(RadiusSize::Lg)
    .describe("checkout", if clean { "clean" } else { "not_clean" })
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::GitBranch, IconSize::Base, PaletteColor::TextSecondary))
        .child(key("delivery.queue.base.key"))
        .child(
          Text::new(&head)
            .id("merge-queue-base-head")
            .variant(ExtraTypography::PropertyMono.role()),
        ),
    )
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(
          Row::new()
            .width(Dimension::Px(DOT))
            .height(Dimension::Px(DOT))
            .rounded(ExtraRadius::Full.role())
            .background(mark),
        )
        .child(key("delivery.queue.base.checkout"))
        .child(
          Text::new(&shorten_path(Path::new(&**checkout), PATH_CHARS))
            .id("merge-queue-checkout-path")
            .variant(ExtraTypography::PropertyMono.role())
            .nowrap(),
        )
        .child(
          Text::new(&state)
            .id("merge-queue-checkout-state")
            .variant(ExtraTypography::Property.role())
            .color(state_color),
        ),
    )
}

fn checkout_words(ctx: &Ctx, checkout: &CheckoutState) -> (Arc<str>, PaletteColor) {
  match checkout {
    CheckoutState::Clean => (ctx.t("delivery.queue.base.clean"), PaletteColor::TextSecondary),
    CheckoutState::Uncommitted(files) if files.len() == 1 => {
      (ctx.t("delivery.queue.base.uncommittedOne"), PaletteColor::Warning)
    }
    CheckoutState::Uncommitted(files) => (
      ctx.t_args(
        "delivery.queue.base.uncommittedOther",
        [("count", files.len().to_string())],
      ),
      PaletteColor::Warning,
    ),
    CheckoutState::OtherBranch(Some(branch)) => (
      ctx.t_args("delivery.queue.base.otherBranch", [("branch", branch.as_str())]),
      PaletteColor::Warning,
    ),
    CheckoutState::OtherBranch(None) => (ctx.t("delivery.queue.base.detached"), PaletteColor::Warning),
    CheckoutState::InProgress => (ctx.t("delivery.queue.base.inProgress"), PaletteColor::Warning),
    CheckoutState::Unknown => (ctx.t("delivery.queue.base.unknown"), PaletteColor::TextMuted),
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
