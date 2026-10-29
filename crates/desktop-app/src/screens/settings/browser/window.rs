//! *Browser window* (`Browser window` `GoSUF` in `j3Lre`): *Open the Browser window when an agent starts browsing*,
//! on until a person turns it off (the person's decision 2026-10-29; the frame draws it off). The Browser window is not
//! in this version yet, so the switch only keeps the choice for it, as the group's note says. A change shows at once
//! and is saved off the UI thread; one that was not saved says why under the row.

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
};

use super::super::save_state::{Outcome, SaveState};
use crate::{
  services::Services,
  ui::{
    field::field_error,
    message::message_text,
    setting_rows::{SettingGroup, SettingRow},
    toggle::{Toggle, ToggleKind},
  },
};

const ID: &str = "settings-browser-window";

type SaveAction = FutureAction<bool, (), Message>;

/// The group with its switch; its props are the switch as stored.
pub(super) struct WindowSwitch {
  /// The switch as the person last turned it; `None` until they do.
  chosen: Signal<Option<bool>>,
}

impl Component for WindowSwitch {
  type Props = bool;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      chosen: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let stored = *ctx.props::<bool>();
    let save = save_action(ctx);
    let state = save.state().get();
    if let Some(saves) = SaveState::of(ctx) {
      saves.report(ID, Outcome::of(&state));
    }
    // The person's choice once it saved or while it saves; the stored value until then, or after a save failed.
    let shown = match (&state.error, self.chosen.get()) {
      (None, Some(chosen)) => chosen,
      _ => stored,
    };
    let title = ctx.t("settings.browser.window.label");
    let toggle = Toggle::new(ID, ToggleKind::Switch, shown, title.clone()).hide_label();
    let toggle = if state.is_pending() {
      toggle
    } else {
      let chosen = self.chosen.clone();
      toggle.on_change(move |on| {
        chosen.set(Some(on));
        save.run(on);
      })
    };
    let problem = state.error.as_ref().map(|reason| {
      field_error(
        &format!("{ID}-error"),
        &ctx.t_args(
          "settings.browser.window.notSaved",
          [("reason", message_text(ctx, reason))],
        ),
      )
      .into()
    });
    let row = SettingRow::new(ID, title)
      .hint(ctx.t("settings.browser.window.hint"))
      .control(toggle)
      .below_all(problem)
      .render(ctx);
    SettingGroup::new("settings-browser-window-group", ctx.t("settings.browser.window.title"))
      .row(row)
      .note(ctx.t("settings.browser.window.note"))
  }
}

/// Saves the switch the person turned.
fn save_action(ctx: &mut Ctx) -> SaveAction {
  let workspace = Services::of(ctx).workspace;
  ctx.future_action(move |opens: bool| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.set_opens_browser_window(opens))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "Open the Browser window when an agent starts browsing was not saved");
          messages::store_error(&error)
        })
    }
  })
}
