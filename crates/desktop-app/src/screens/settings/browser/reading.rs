//! Settings › Browser while the browser is looked for (`L12L · Settings · Browser · loading` `vC4Jf` in `Settings
//! loading · redesign` `iDl7k`, the rules of `17 · Patterns · settings loading` `d2yqa`): both groups with their
//! headings, row labels and help lines real; the browser found (after its check), its program's path and the profiles'
//! folder (before Copy) are bones, and so is the window's switch; the lower group fades. *Choose another program…*
//! comes with what is read. The bones show after the skeleton's delay, named *Looking for the browser…*.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};

use super::rows::PROFILES_VALUE;
use crate::{
  theme::{ControlHeight, ExtraTypography, IconSize, SKELETON_FADE, SettingsSpacing},
  ui::{
    icon::{Icon, icon},
    setting_rows::{SettingGroup, SettingRow},
    skeleton::{self, Skeleton},
  },
};

/// The browser's name and its version (`Text bone` `heoli`, `AFe5k` in `uch13`).
const FOUND: (f32, f32) = (98.0, 70.0);
/// The program's path (`Text bone` `faxek`).
const PROGRAM: f32 = 220.0;
/// The profiles' folder (`Text bone` `paEsV`).
const PROFILES: f32 = 147.0;

/// Both groups as bones.
pub(super) fn skeleton(ctx: &mut Ctx) -> Element {
  let program = program_group(ctx);
  let window = window_group(ctx);
  let groups = Column::new()
    .id("settings-browser-reading-groups")
    .width(Dimension::Pct(100.0))
    .spacing(SettingsSpacing::Page.role())
    .child(program)
    .child(window);
  Skeleton::new("settings-browser-reading", ctx.t("settings.browser.reading")).show(ctx, groups)
}

/// *Browser program*: *Browser*, *Program* and *Browser profiles* with their help lines, their values as bones.
fn program_group(ctx: &mut Ctx) -> Element {
  let found = Row::new()
    .flex_shrink(0.0)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleCheck, IconSize::Base, PaletteColor::Success))
    .child(skeleton::line(ctx, ExtraTypography::Lead.role(), FOUND.0))
    .child(skeleton::line(ctx, ExtraTypography::PropertyMono.role(), FOUND.1));
  let browser = SettingRow::new("settings-browser-found-reading", ctx.t("settings.browser.found.label"))
    .hint(ctx.t("settings.browser.found.found"))
    .control(found)
    .render(ctx);
  let path = skeleton::line(ctx, ExtraTypography::PropertyMono.role(), PROGRAM);
  let program = SettingRow::new(
    "settings-browser-program-reading",
    ctx.t("settings.browser.program.label"),
  )
  .hint(ctx.t("settings.browser.program.hint"))
  .detail(path)
  .render(ctx);
  // The folder's bone, then Copy's glyph where Copy will stand, waiting for the folder.
  let copy = Row::new()
    .width(Dimension::Px(ControlHeight::Sm.value()))
    .height(Dimension::Px(ControlHeight::Sm.value()))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(Icon::Copy, IconSize::Md, PaletteColor::TextMuted));
  let folder = Row::new()
    .width(Dimension::Px(PROFILES_VALUE))
    .flex_shrink(0.0)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .justify(Justify::End)
    .child(skeleton::line(ctx, ExtraTypography::PropertyMono.role(), PROFILES))
    .child(copy);
  let profiles = SettingRow::new(
    "settings-browser-profiles-reading",
    ctx.t("settings.browser.profiles.label"),
  )
  .hint(ctx.t("settings.browser.profiles.hint"))
  .control(folder)
  .render(ctx);
  SettingGroup::new(
    "settings-browser-program-reading",
    ctx.t("settings.browser.program.title"),
  )
  .row(browser)
  .row(program)
  .row(profiles)
  .into()
}

/// *Browser window*: its row with the switch as a bone, the group fading as the lower one (`JyQQ5`).
fn window_group(ctx: &mut Ctx) -> Element {
  let row = SettingRow::new(
    "settings-browser-window-reading",
    ctx.t("settings.browser.window.label"),
  )
  .hint(ctx.t("settings.browser.window.hint"))
  .control(skeleton::switch())
  .render(ctx);
  let group: Element = SettingGroup::new(
    "settings-browser-window-group-reading",
    ctx.t("settings.browser.window.title"),
  )
  .row(row)
  .into();
  Column::new()
    .width(Dimension::Pct(100.0))
    .opacity(SKELETON_FADE[0])
    .child(group)
    .into()
}
