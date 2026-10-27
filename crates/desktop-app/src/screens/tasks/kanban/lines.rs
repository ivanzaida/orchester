//! The lines over the board's columns and the hint under a lifted card: a change that did not happen, *{{task}} was not
//! changed. {{reason}}* on `danger_muted` until it is dismissed (`Error line` in `14 Shared project` `EFrKQ`, `A drop
//! that fails` `Crd26`); where a task canceled from the board went, *KON-16 is Canceled.* with Reopen task and Show
//! Canceled (`Done line` in `Where a canceled card goes` `VAbCz`); and while a card is lifted, what dropping it where it
//! is over does, with Esc that cancels (`Hint` `n5NsV` in `02 Dragging` `Yyeme`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, TypographyStyle},
  },
  components::{Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::task::organization::WorkflowStatus;
use orchester_workspace::tasks::moves::{self, Decision, Move};

use super::{
  actions::{BoardState, Drag, Failed},
  drop::refusal_of,
  model::ColumnKey,
};
use crate::{
  screens::task_close::{Closer, Subject},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    icon_button::IconButton,
    kbd::key_cap,
    message::message_text,
  },
};

/// The hint's height (`n5NsV`).
const HINT_HEIGHT: f32 = 32.0;

/// *KON-27 was not changed. The task was changed since you opened it…*, until dismissed.
pub fn failure(ctx: &mut Ctx, state: &BoardState, failed: &Failed) -> Element {
  let reason = message_text(ctx, &failed.message);
  let text = ctx.t_args("task_close.failed", [("task", &*failed.key), ("reason", &*reason)]);
  let dismiss = state.failed.clone();
  Row::new()
    .id("tasks-board-failed")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::DangerMuted)
    .child(
      Text::new(&text)
        .id("tasks-board-failed-text")
        .variant(ExtraTypography::Lead.role())
        .color(PaletteColor::TextPrimary)
        .flex(1.0),
    )
    .child(
      IconButton::new("tasks-board-failed-dismiss", Icon::X, ctx.t("tasks.board.failedDismiss"))
        .small()
        .on_click(move || dismiss.set(None))
        .render(ctx),
    )
    .into()
}

/// *KON-16 is Canceled.* with Reopen task and Show Canceled.
pub fn canceled(ctx: &Ctx, (state, closer): (&BoardState, &Closer), task: &Subject) -> Element {
  let (reopen, subject) = (closer.clone(), task.clone());
  let display = state.display.clone();
  Row::new()
    .id("tasks-board-canceled")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .child(
      Text::new(&ctx.t_args("tasks.board.canceled", [("task", &*task.key)]))
        .id("tasks-board-canceled-text")
        .variant(ExtraTypography::Lead.role())
        .color(PaletteColor::TextPrimary),
    )
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new("tasks-board-canceled-reopen", ButtonKind::Ghost, ctx.t("task_close.menu.reopen"))
        .icon(Icon::RotateCcw)
        .on_click(move || reopen.reopen(subject.clone())),
    )
    .child(
      ActionButton::new("tasks-board-canceled-show", ButtonKind::Ghost, ctx.t("tasks.board.showCanceled"))
        .icon(Icon::Eye)
        .on_click(move || display.update(|display| display.show_canceled = true)),
    )
    .into()
}

/// What dropping `drag`'s card where it is over does, and that Esc cancels.
pub fn hint(ctx: &Ctx, drag: &Drag) -> Element {
  let key = &*drag.card.row.key;
  let words = match drag.over.as_ref() {
    None if drag.card.row.run_not_ended => ctx.t_args("tasks.board.hint.onlyClose", [("task", key)]),
    None => ctx.t_args("tasks.board.hint.move", [("task", key)]),
    Some(target) if refusal_of(&drag.card, target).is_some() => {
      ctx.t_args("tasks.board.hint.refused", [("task", key)])
    }
    Some(target) => match &target.column {
      ColumnKey::Status(status) => match moves::decide(&drag.card.row, *status) {
        Decision::Move(Move::Queue) => ctx.t_args("tasks.board.hint.queue", [("task", key)]),
        Decision::Move(Move::ReturnToDraft) => ctx.t_args("tasks.board.hint.draft", [("task", key)]),
        Decision::Move(Move::RunAgain) => ctx.t_args("tasks.board.hint.runAgain", [("task", key)]),
        Decision::Move(Move::Close(_)) if *status == WorkflowStatus::Done => {
          ctx.t_args("tasks.board.hint.done", [("task", key)])
        }
        Decision::Move(Move::Close(_)) => ctx.t_args("tasks.board.hint.cancel", [("task", key)]),
        _ => ctx.t_args("tasks.board.hint.move", [("task", key)]),
      },
      _ => ctx.t_args("tasks.board.hint.move", [("task", key)]),
    },
  };
  Row::new()
    .id("tasks-board-hint")
    .height(Dimension::Px(HINT_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .box_shadow(ShadowStyle::Md)
    .child(
      Text::new(&words)
        .id("tasks-board-hint-text")
        .variant(TypographyStyle::Description)
        .nowrap(),
    )
    .child(key_cap("tasks-board-hint-esc", &ctx.t("tasks.board.keys.esc")))
    .child(
      Text::new(&ctx.t("tasks.board.hint.cancels"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// Room under the hint and the selection bar, so they float over the columns' foot.
pub fn foot_room() -> lurq::node::padding::Padding {
  Padding::new().bottom(ExtraSpacing::Inset.role())
}
