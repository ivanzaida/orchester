//! The non-modal draft replacement question in qCku8/SzRf6.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row},
  node::{Element, dimension::Dimension},
};

use super::{parts, state::State};
use crate::{
  theme::{ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

pub fn question(ctx: &Ctx, state: &State, writing: bool) -> Element {
  let editing = state.get();
  let version = orchester_workspace::pipelines::write::next_version(&editing.opened)
    .map_or_else(String::new, |v| v.get().to_string());
  let replace = state.clone();
  let keep = state.clone();
  let confirm = ActionButton::new(
    "pipeline-restore-confirm",
    ButtonKind::Primary,
    ctx.t_args("pipelines.replaceDraft", [("version", version.clone())]),
  )
  .icon(Icon::Undo2)
  .on_click(move || {
    if !writing {
      replace.update(|s| s.restore());
    }
  });
  Column::new()
    .id("pipeline-restore-question")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Sm)
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::WarningMuted)
    .border_inside(BorderSize::Sm, PaletteColor::Warning)
    .rounded(RadiusSize::Md)
    .child(
      Row::new()
        .spacing(ExtraSpacing::Tight.role())
        .child(icon(Icon::TriangleAlert, IconSize::Sm, PaletteColor::Warning))
        .child(parts::caption(
          &ctx.t_args("pipelines.replaceDraftTitle", [("version", version)]),
        )),
    )
    .child(parts::note(&ctx.t("pipelines.replaceDraftDetail")))
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .child(if writing { confirm.disabled() } else { confirm })
        .child(
          ActionButton::new(
            "pipeline-restore-keep",
            ButtonKind::Secondary,
            ctx.t("pipelines.keepDraft"),
          )
          .on_click(move || keep.update(|s| s.restore_requested = false)),
        ),
    )
    .into()
}
