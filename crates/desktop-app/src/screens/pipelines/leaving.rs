//! In-place leave decision, using the confirmation pattern RWODj.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  node::Element,
};

use crate::{
  navigation::{Navigation, guard::LeaveGuard},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
  },
};

pub fn question(ctx: &mut Ctx, guard: &LeaveGuard, writing: bool, name: &str) -> Option<Element> {
  if !guard.requested() {
    return None;
  }
  let navigation = Navigation::of(ctx);
  let leave = guard.clone();
  let keep = guard.clone();
  let confirm = ActionButton::new("pipeline-leave-confirm", ButtonKind::Danger, ctx.t("pipelines.leave"))
    .on_click(move || leave.leave(&navigation));
  Some(
    card("pipeline-leave-question", CardSurface::Decision)
      .background(PaletteColor::SurfaceRaised)
      .rounded(ExtraRadius::Tile.role())
      .padding(SpacingSize::Lg)
      .spacing(ExtraSpacing::Loose.role())
      .child(Text::new(&ctx.t_args("pipelines.leaveNamed", [("name", name)])).variant(ExtraTypography::Strong.role()))
      .child(crate::ui::inspector::note(
        "pipeline-leave-discard",
        crate::ui::icon::Icon::FilePen,
        &ctx.t("pipelines.leaveDiscard"),
      ))
      .child(crate::ui::inspector::note(
        "pipeline-leave-saved",
        crate::ui::icon::Icon::ShieldCheck,
        &ctx.t("pipelines.leaveSaved"),
      ))
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .child(
            ActionButton::new("pipeline-leave-keep", ButtonKind::Secondary, ctx.t("pipelines.keep"))
              .on_click(move || keep.keep_editing()),
          )
          .child(if writing { confirm.disabled() } else { confirm }),
      )
      .into(),
  )
}
