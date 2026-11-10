//! What the runs-on picker is with no connection to choose (`No connections` `cjnbn` in `Runs-on picker · states`
//! `t7cdi`): only its foot, 360 wide on `surface_raised` in a `border_strong` stroke, radius `lg`, lifted by the `lg`
//! shadow, with no line over it. It is 8 by 12 and says that another account starts a new session (11.5/1.4
//! `text_muted`) and offers *Settings ↗* (12 `text_secondary`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::RunsOnProps;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  theme::{ExtraColor, ExtraRadius, ExtraTypography, OrchestratorText},
  ui::pressable::opening,
};

/// The picker's width (`cjnbn`).
const WIDTH: f32 = 360.0;

/// The picker with nothing to choose: its foot alone.
pub(super) fn empty_menu(ctx: &mut Ctx, props: &RunsOnProps) -> Element {
  let navigation = Navigation::of(ctx);
  let settings = opening(navigation.to(Route::Project(props.project, ProjectPage::OrchestratorSettings)))
    .id("orchestrator-settings")
    .describe("a11y_name", ctx.t("orchestrator.runsOn.settings"))
    .flex_shrink(0.0)
    .rounded(ExtraRadius::Link.role())
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .cursor(CursorIcon::Pointer)
    .child(
      Text::new(&ctx.t("orchestrator.runsOn.settings"))
        .variant(OrchestratorText::PickerLink.role())
        .nowrap(),
    );
  let foot = Row::new()
    .id("orchestrator-runs-on-foot")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("orchestrator.runsOn.newSession"))
        .variant(ExtraTypography::Note.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(settings);
  Column::new()
    .id("orchestrator-runs-on-menu")
    .width(Dimension::Px(WIDTH))
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(RadiusSize::Lg)
    .box_shadow(ShadowStyle::Lg)
    .child(foot)
    .into()
}
