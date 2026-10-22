//! What *What automation may do* says of the window (`oD7hy`, its states `HNQBR`): *Look at the window*, which every
//! client with the token may always do, so its switch is on and fixed beside a lock and *Always on* (`Row · Look at
//! the window` `baK0O`), and the hint under *Agents can control this window* (`Hint` `iqm0X`): the app reads window
//! control when it starts, so the hint says what this run has — the build's default, a choice that applies from the
//! next start, or what `ORCHESTER_APP_MCP_WINDOW_CONTROL` decided for this run (`McpSettings::this_run`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::Element,
};

use crate::{
  mcp::{McpLink, McpSettings, ThisRun, settings::WINDOW_CONTROL_VARIABLE},
  theme::{ExtraSpacing, IconSize},
  ui::{
    form_section::{Label, Place, side_row},
    icon::{Icon, icon},
    toggle::{Toggle, ToggleKind},
  },
};

const LOOK: &str = "settings-look-at-window";

/// *Look at the window*: on, and no one can turn it off.
pub fn look_at_the_window(ctx: &Ctx) -> Element {
  let title = ctx.t("settings.automation.lookAtWindow.title");
  let control = Row::new()
    .id(format!("{LOOK}-control"))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("settings.automation.lookAtWindow.always"))
        .id(format!("{LOOK}-always"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(Toggle::new(LOOK, ToggleKind::Switch, true, title.clone()).hide_label());
  side_row(
    Label::new(LOOK, title).description(ctx.t("settings.automation.lookAtWindow.description")),
    control,
    Place::Within,
    false,
  )
}

/// What applies to this run while *Agents can control this window* shows `chosen`.
pub fn window_control_hint(ctx: &mut Ctx, chosen: bool) -> String {
  let settings = ctx
    .use_context::<McpLink>()
    .map_or_else(McpSettings::of_build, |link| link.status().settings);
  let variable = [("variable", WINDOW_CONTROL_VARIABLE)];
  let words = match settings.this_run(chosen) {
    ThisRun::AsStarted if cfg!(debug_assertions) => ctx.t("settings.automation.windowControl.buildDevelopment"),
    ThisRun::AsStarted => ctx.t("settings.automation.windowControl.buildRelease"),
    ThisRun::NextStart(true) => ctx.t("settings.automation.windowControl.nextStartOn"),
    ThisRun::NextStart(false) => ctx.t("settings.automation.windowControl.nextStartOff"),
    ThisRun::Environment(true) => ctx.t_args("settings.automation.windowControl.environmentOn", variable),
    ThisRun::Environment(false) => ctx.t_args("settings.automation.windowControl.environmentOff", variable),
  };
  words.to_string()
}
