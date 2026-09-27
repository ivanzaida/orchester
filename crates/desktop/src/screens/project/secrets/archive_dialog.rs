//! Archive secret, confirmed in a dialog (the inspector's `MR3H0`; no confirmation is drawn, so it
//! follows the publish confirmation `jYL1V`). Archiving deletes the value from the keychain, which
//! cannot be undone, and says which tools stop working. Archive runs once; while it runs the
//! button is disabled.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::secret::Secret;

use super::library::handling_notice;
use crate::{
  messages::Message,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    field::field_error,
    form::{Submit, form},
    icon::Icon,
    message_text,
  },
};

/// Dialog width and action bar height (`jYL1V`, `M4vhEv`).
const DIALOG_WIDTH: f32 = 600.0;
const ACTIONS_HEIGHT: f32 = 76.0;
const BUTTON_HEIGHT: f32 = 36.0;

pub struct ArchiveDialog<'a> {
  pub secret: &'a Secret,
  pub project: &'a str,
  /// Tools that use the secret.
  pub uses: usize,
  pub pending: bool,
  /// Why the last attempt failed.
  pub error: Option<Message>,
  /// Archives the secret; `None` while it runs.
  pub confirm: Submit,
  pub cancel: Arc<dyn Fn() + Send + Sync>,
}

pub fn archive_dialog(ctx: &Ctx, dialog: &ArchiveDialog) -> Element {
  let name = dialog.secret.name().as_str();
  let header = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(&ctx.t_args("secrets.archive.title", [("name", name)])).variant(ExtraTypography::DialogTitle.role()),
    )
    .child(
      Text::new(&ctx.t_args("secrets.archive.explanation", [("project", dialog.project)]))
        .variant(ExtraTypography::Explanation.role()),
    );
  let uses = match dialog.uses {
    0 => ctx.t("secrets.archive.unused"),
    1 => ctx.t("secrets.archive.usedByOne"),
    count => ctx.t_args("secrets.archive.usedByOther", [("count", count.to_string().as_str())]),
  };
  let body = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .padding_bottom(SpacingSize::Lg)
    .child(handling_notice(&uses, None));
  let content = Column::new()
    .id("archive-secret-dialog")
    .width(Dimension::Px(DIALOG_WIDTH))
    .background(ExtraColor::Surface.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(header)
    .child(body)
    .child(actions(ctx, dialog));
  backdrop(form(dialog.confirm.clone(), content))
}

fn actions(ctx: &Ctx, dialog: &ArchiveDialog) -> Element {
  let label = if dialog.pending {
    "secrets.archive.archiving"
  } else {
    "secrets.archive.confirm"
  };
  let confirm = ActionButton::new(ctx.t(label))
    .primary()
    .icon(Icon::Archive)
    .id("archive-secret-confirm")
    .in_form()
    .height(Dimension::Px(BUTTON_HEIGHT));
  let confirm = match dialog.confirm.clone() {
    Some(archive) => confirm.on_click(move || archive()),
    // Off while the archive runs.
    None => confirm.on_click(|| {}).enabled(false),
  };
  let cancel = dialog.cancel.clone();
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ACTIONS_HEIGHT))
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .with_children(dialog.error.as_ref().map(|error| {
      Column::new()
        .id("archive-secret-error")
        .child(field_error(&message_text(ctx, error)))
    }))
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new(ctx.t("common.cancel"))
        .id("archive-secret-cancel")
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(move || cancel())
        .render(ctx),
    )
    .child(confirm.render(ctx))
    .into()
}
