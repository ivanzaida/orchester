//! What Settings › Automation says of the window (*The window* `GADec` in `04 · Automation`): *Look at the window*,
//! which every client with the token may always do, so its switch is on and fixed beside a lock and *Always on*
//! (`Look at the window` `yMdW4`), and the line under *Agents can control this window*: the app reads window control
//! when it starts, so the line says what this run has — the build's default, the person's stored choice, a choice
//! that applies from the next start, or what `ORCHESTER_APP_MCP_WINDOW_CONTROL` decided for this run
//! (`McpSettings::this_run`).

use lurq::{app::ctx::Ctx, components::Row, layout::Alignment, node::Element};

use crate::{
  mcp::{McpLink, McpSettings, ThisRun, settings::WINDOW_CONTROL_VARIABLE},
  ui::{
    icon::Icon,
    setting_rows::{SettingRow, marked_words},
    toggle::{Toggle, ToggleKind},
  },
};

const LOOK: &str = "settings-look-at-window";

/// *Look at the window*: on, and no one can turn it off.
pub fn look_at_the_window(ctx: &mut Ctx) -> Element {
  let title = ctx.t("settings.automation.lookAtWindow.title");
  let control = Row::new()
    .id(format!("{LOOK}-control"))
    .spacing(lurq::app::theme::SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(marked_words(
      &format!("{LOOK}-always"),
      Icon::Lock,
      &ctx.t("settings.automation.lookAtWindow.always"),
    ))
    .child(
      Toggle::new(LOOK, ToggleKind::Switch, true, title.clone())
        .hide_label()
        .settled(),
    );
  SettingRow::new(LOOK, title)
    .hint(ctx.t("settings.automation.lookAtWindow.hint"))
    .info(ctx.t("settings.automation.lookAtWindow.description"))
    .control(control)
    .render(ctx)
}

/// What applies to this run while *Agents can control this window* shows `chosen`.
pub fn window_control_hint(ctx: &mut Ctx, chosen: bool) -> String {
  let settings = ctx
    .use_context::<McpLink>()
    .map_or_else(McpSettings::of_build, |link| link.status().settings);
  let variable = [("variable", WINDOW_CONTROL_VARIABLE)];
  let words = match settings.this_run(chosen) {
    ThisRun::AsBuilt if cfg!(debug_assertions) => ctx.t("settings.automation.windowControl.buildDevelopment"),
    ThisRun::AsBuilt => ctx.t("settings.automation.windowControl.buildRelease"),
    ThisRun::AsChosen(true) => ctx.t("settings.automation.windowControl.chosenOn"),
    ThisRun::AsChosen(false) => ctx.t("settings.automation.windowControl.chosenOff"),
    ThisRun::NextStart(true) => ctx.t("settings.automation.windowControl.nextStartOn"),
    ThisRun::NextStart(false) => ctx.t("settings.automation.windowControl.nextStartOff"),
    ThisRun::Environment(true) => ctx.t_args("settings.automation.windowControl.environmentOn", variable),
    ThisRun::Environment(false) => ctx.t_args("settings.automation.windowControl.environmentOff", variable),
  };
  words.to_string()
}
