//! The app's settings (`Settings · Automation · dark` `oD7hy`). Only *Let automation change agent
//! permissions* is built: the person's own switch for what automation may widen through the MCP server,
//! which no tool can change. The page tabs and the other rows of the design come with the Settings
//! screen's own task.

mod automation;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};

use crate::{
  routes::Screen,
  theme::{ExtraTypography, SettingsSpacing},
};

/// The form's width (`Form` `dpkR7`).
const FORM_WIDTH: f32 = 720.0;

pub fn screen(ctx: &mut Ctx) -> Element {
  let title = ctx.t(Screen::Settings.title_key());
  Column::new()
    .id(format!("screen-{}", Screen::Settings.name()))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding_top(SettingsSpacing::PanelTop.role())
    .padding_horizontal(SettingsSpacing::PanelSide.role())
    .padding_bottom(SpacingSize::Xl)
    .spacing(SpacingSize::Lg)
    .child(Text::new(&title).variant(ExtraTypography::Display.role()))
    .child(
      Column::new()
        .id("settings-form")
        .width(Dimension::Px(FORM_WIDTH))
        .padding_top(SpacingSize::Sm)
        .child(ctx.mount_keyed::<automation::ChangePermissions>("change-permissions", automation::NoProps)),
    )
    .into()
}
