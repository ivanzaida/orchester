//! Explicit recovery keeps the draft and never retries from render.
use lurq::{app::ctx::Ctx, node::Element};

use super::{Asked, AutoMode, Props};
use crate::{
  screens::project_settings::{Status, work::Save},
  ui::{
    button::{ActionButton, ButtonKind},
    notice::{Notice, NoticeTone},
  },
};

impl AutoMode {
  pub(super) fn retry(&self, ctx: &Ctx, props: &Props, save: &Save) -> Element {
    let (tab, props, save) = (self.clone(), props.clone(), save.clone());
    let button = ActionButton::new(
      "settings-auto-retry",
      ButtonKind::Secondary,
      ctx.t("project_settings.retry"),
    );
    if self.outside() {
      return button.disabled().into();
    }
    button
      .on_click(move || {
        if !tab.outside()
          && let Ok(mut asked) = tab.draft()
        {
          // The switch reads back storage after failure; retain only its requested state.
          // Reviewer retry always uses the current visible controls.
          if let Some(attempt) = tab.attempted.get_untracked().0 {
            asked.new_runs = attempt.new_runs;
          }
          tab.request(&props, &save, asked);
        }
      })
      .into()
  }
  pub(super) fn reload(&self, ctx: &Ctx, props: &Props) -> Element {
    let (tab, status) = (self.clone(), props.tab.status.clone());
    Notice::new(
      "settings-auto-outside",
      NoticeTone::Warning,
      ctx.t("project_settings.outsideTitle"),
    )
    .body(ctx.t("project_settings.outsideDetail"))
    .action(
      ActionButton::new(
        "settings-auto-reload",
        ButtonKind::Secondary,
        ctx.t("project_settings.reload"),
      )
      .on_click(move || {
        tab.fill(tab.supplied.get_untracked().0);
        tab.asking.set(Asked(None));
        tab.attempted.set(Asked(None));
        tab.failed.set(false);
        status.set(Status::Saved);
      }),
    )
    .into()
  }
}
