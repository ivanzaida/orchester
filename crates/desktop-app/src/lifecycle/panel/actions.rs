//! The closing panel's actions (`Action bar` `giZVL`): Quit now on the left, Cancel and Wait for agents to
//! save on the right. While the agents save Cancel is unavailable, with why, and the primary becomes how
//! much Orchester still waits for; once every run is saved and git is done, *Closing Orchester…* says the
//! runners exit, and Quit now stays until they did.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::close::{CloseFlow, Stage};

use super::{ClosingPanel, parts::progress};
use crate::{
  lifecycle::keys::Choice,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

impl ClosingPanel {
  /// The actions in `stage`, with `left` runs saving and git operations running.
  pub(super) fn actions(&self, ctx: &Ctx, stage: Stage, left: usize, close: &CloseFlow) -> Element {
    let press = |choice: Choice| {
      let close = close.clone();
      move || choice.press(&close)
    };
    let mut row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center);
    row = row.child(
      ActionButton::new(
        "closing-quit-now",
        ButtonKind::Secondary,
        ctx.t("lifecycle.close.quitNow"),
      )
      .icon(Icon::Power)
      .in_form()
      .element_ref(self.button(Choice::QuitNow))
      .on_click(press(Choice::QuitNow)),
    );
    row = row.child(Spacer::new().flex(1.0));
    let cancel = ActionButton::new("closing-cancel", ButtonKind::Secondary, ctx.t("lifecycle.close.cancel"))
      .in_form()
      .element_ref(self.button(Choice::Cancel));
    let actions: Element = match stage {
      Stage::Confirm => row
        .child(cancel.on_click(press(Choice::Cancel)))
        .child(
          ActionButton::new("closing-wait", ButtonKind::Primary, ctx.t("lifecycle.close.wait"))
            .icon(Icon::Save)
            .in_form()
            .element_ref(self.button(Choice::Wait))
            .on_click(press(Choice::Wait)),
        )
        .into(),
      Stage::Waiting => {
        let count = left.to_string();
        let key = if left == 1 {
          "lifecycle.close.closesAfter.one"
        } else {
          "lifecycle.close.closesAfter.other"
        };
        let row = row
          .child(cancel.disabled())
          .child(progress(&ctx.t_args(key, [("count", count.as_str())])));
        // `Reason line` (`NGPaQ`): the lock and why, at the actions' end, 8 under them.
        let reason = Row::new()
          .width(Dimension::Pct(100.0))
          .justify(Justify::End)
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted))
          .child(
            Text::new(&ctx.t("lifecycle.close.cancelUnavailable"))
              .id("closing-cancel-reason")
              .variant(ExtraTypography::Note.role())
              .nowrap(),
          );
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(row)
          .child(reason)
          .into()
      }
      Stage::Saved => row.child(progress(&ctx.t("lifecycle.close.closing"))).into(),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Lg))
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(actions)
      .into()
  }
}
