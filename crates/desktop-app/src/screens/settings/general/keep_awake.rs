//! *Keep the computer awake while agents work* (`docs/plan/orchestrator-autonomy/OA-08-…`): off until a person turns
//! it on. With it on, the app asks the system not to go to sleep by itself while an agent works or a turn of the
//! Orchestrator thinks, and lets go at once after (`crate::awake`). The row is built as *Send error reports* is
//! (`super::error_reports`, `tMyiy`): a switch that shows the stored setting, saved off the UI thread, the header's
//! save state following it. Its own frame is the person's to draw (`frames.md` F); until then it stands in its own
//! group, *Work*, drawn with the same parts.

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

use crate::{
  queries::{self, Session, app},
  screens::settings::save_state::{Outcome, SaveState},
  services::Services,
  ui::{
    field::field_error,
    message::message_text,
    setting_rows::SettingRow,
    skeleton::{self, Skeleton},
    toggle::{Toggle, ToggleKind},
  },
};

const ID: &str = "settings-keep-awake";

type SaveAction = FutureAction<bool, bool, Message>;

pub struct KeepAwakeRow {
  /// The setting as the person last chose it; `None` until they do.
  chosen: Signal<Option<bool>>,
}

impl Component for KeepAwakeRow {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      chosen: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let services = Services::of(ctx);
    // Hooks in the same order on every render: the action, then the read, which each save invalidates.
    let save = save_action(ctx, &services);
    let state = save.state().get();
    let awake = services.awake.clone();
    let read = queries::state(&ctx.query(app::keeps_awake(Session(services))));
    if state.is_rejected() && self.chosen.get_untracked().is_some() {
      self.chosen.set(None);
    }
    queries::settle(&self.chosen, read.data.as_ref(), state.is_pending());
    let shown = self.chosen.get().or(read.data);
    if let Some(saves) = SaveState::of(ctx) {
      saves.report(ID, Outcome::of(&state));
    }
    let control: Element = match shown {
      None if read.is_pending() => Skeleton::new(format!("{ID}-reading"), ctx.t("settings.reading"))
        .inline()
        .show(ctx, skeleton::switch()),
      _ => {
        let label = ctx.t("settings.general.work.keepAwake.label");
        let switch = Toggle::new(ID, ToggleKind::Switch, shown.unwrap_or(false), label).hide_label();
        // Until the stored value is known, and while it is read or saved, the switch stays as it is.
        if shown.is_some() && !state.is_pending() && !read.is_pending() {
          let chosen = self.chosen.clone();
          switch
            .on_change(move |on| {
              // Turned off, the computer may sleep at once; turned on, the next look at a project's work holds it.
              if !on {
                awake.release();
              }
              chosen.set(Some(on));
              save.run(on);
            })
            .into()
        } else {
          switch.into()
        }
      }
    };
    let problem = problem(ctx, (&read.error, &state.error));
    SettingRow::new(ID, ctx.t("settings.general.work.keepAwake.label"))
      .hint(ctx.t("settings.general.work.keepAwake.hint"))
      .control(control)
      .below_all(problem)
      .render(ctx)
  }
}

/// Why the setting cannot be read or was not saved, under the row.
fn problem(ctx: &Ctx, (read, save): (&Option<Message>, &Option<Message>)) -> Option<Element> {
  let (key, reason) = match (read, save) {
    (Some(reason), _) => ("settings.general.work.keepAwake.unreadable", reason),
    (None, Some(reason)) => ("settings.general.work.keepAwake.notSaved", reason),
    (None, None) => return None,
  };
  let reason = message_text(ctx, reason);
  Some(field_error(&format!("{ID}-error"), &ctx.t_args(key, [("reason", &*reason)])).into())
}

/// Saves the person's choice; the workspace counts the change, which reads the setting again (`queries`). The app
/// holds the computer awake, or lets go, at each project's next look (`crate::awake`).
fn save_action(ctx: &mut Ctx, services: &Services) -> SaveAction {
  let workspace = services.workspace.clone();
  ctx.future_action(move |on: bool| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.set_keeps_awake(on))
        .await?
        .map(|()| on)
        .map_err(|error| {
          tracing::error!(%error, "whether to keep the computer awake was not saved");
          messages::store_error(&error)
        })
    }
  })
}
