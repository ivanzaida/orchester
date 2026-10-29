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
        // A retry uses the reviewer the controls show now.
        if !tab.outside()
          && let Ok(asked) = tab.draft()
        {
          // Retrying makes the change again: whoever presses it must be one who may.
          tab.note_input();
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
        tab.attempted.set(Asked(None));
        tab.failed.set(false);
        status.set(Status::Saved);
      }),
    )
    .into()
  }
}
