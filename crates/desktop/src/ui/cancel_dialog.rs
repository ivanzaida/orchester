//! The confirmation that Canceled stops active runs (`N1vwaM` for one task, `hFH4D` for a
//! selection): title, explanation, a notice about the work or the runs, then Keep and Stop at the
//! right. A failure shows above the actions while the dialog stays open.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::layout_kind::Justify,
  node::{Element, border::Border, dimension::Dimension},
};

use super::{button::ActionButton, field::wrapping_field_error, icon::Icon};
use crate::theme::{ExtraColor, ExtraTypography};

/// The dialog's width (`hNMDh` in `N1vwaM`, `EMYqR` in `hFH4D`).
const DIALOG_WIDTH: f32 = 620.0;

/// What the dialog says and how its buttons are named; `pending` while the runs are stopping. The
/// explanation and the notice are missing while the active run is not read yet.
pub struct CancelDialog<'a> {
  pub id: &'static str,
  pub title: Arc<str>,
  pub explanation: Option<Arc<str>>,
  pub notice: Option<(Arc<str>, Arc<str>)>,
  pub keep: (Arc<str>, &'static str),
  pub confirm: (Arc<str>, &'static str),
  pub pending: bool,
  pub error: Option<&'a str>,
}

/// The dialog's panel; the caller puts it on the backdrop of a modal.
pub fn cancel_dialog(
  ctx: &Ctx,
  dialog: CancelDialog<'_>,
  keep: impl Fn() + Send + Sync + 'static,
  confirm: impl Fn() + Send + Sync + 'static,
) -> Element {
  let notice = dialog
    .notice
    .map(|(heading, detail)| super::organization_notice::organization_notice(&heading, &detail));
  let confirm_label = if dialog.pending {
    ctx.t("tasks.cancel.stopping")
  } else {
    dialog.confirm.0
  };
  let actions = Row::new()
    .width(Dimension::Pct(100.0))
    .justify(Justify::End)
    .spacing(crate::theme::ExtraSpacing::Stack.role())
    .child(
      ActionButton::new(dialog.keep.0)
        .icon(Icon::X)
        .id(dialog.keep.1)
        .in_form()
        .on_click(keep)
        .enabled(!dialog.pending)
        .render(ctx),
    )
    .child(
      ActionButton::new(confirm_label)
        .icon(if dialog.pending { Icon::Loader } else { Icon::Square })
        .id(dialog.confirm.1)
        .in_form()
        .on_click(confirm)
        .enabled(!dialog.pending)
        .render(ctx),
    );
  Column::new()
    .id(dialog.id)
    .width(Dimension::Px(DIALOG_WIDTH))
    .padding(SpacingSize::Section)
    .spacing(SpacingSize::Lg)
    .background(ExtraColor::Surface.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(Text::new(&dialog.title).variant(ExtraTypography::DialogTitle.role()))
    .with_children(dialog.explanation.map(|explanation| {
      Text::new(&explanation)
        .variant(ExtraTypography::Explanation.role())
        .width(Dimension::Pct(100.0))
    }))
    .with_children(notice)
    .with_children(dialog.error.map(wrapping_field_error))
    .child(actions)
    .into()
}
