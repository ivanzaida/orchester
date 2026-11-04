//! *Updates* in Settings › General, live (`crate::updates` draws it): where this copy's update stands as
//! `AppUpdates` follows it, the channel and *Update automatically* as stored, read through their query (which the
//! window read as the session started, and reads again after each save and each check, `queries`), and what its
//! presses do. A setting saves as it is chosen, off the UI thread, and the header's save state follows it; the choice
//! shows until the stored settings have it, and a failed save shows them again. *Check now* and *Resume* run on the
//! app's runtime; *Restart to update* installs at once or asks about the runs at work (`shell::update`).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState, StreamEmitter},
  },
  core::Signal,
  node::Element,
};
use orchester_core::app_update::{Channel, UpdateSettings};
use orchester_workspace::{
  app_updates::{AppUpdates, UpdateState},
  background,
  messages::{self, Message},
};

use crate::{
  queries::{
    self, Session,
    app::{self, StoredUpdates},
  },
  routes::UpdatesShow,
  screens::settings::save_state::{Outcome, SaveState},
  services::Services,
  shell::update::Restart,
  ui::message::message_text,
  updates::{SectionProps, UpdateAction, UpdateDispatch, UpdatesFacts, UpdatesSection},
};

const ID: &str = "settings-updates";

/// A setting the person changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
  Channel(Channel),
  Automatic(bool),
}

impl DevtoolsInspectable for Change {}

impl Change {
  fn applied(self, settings: UpdateSettings) -> UpdateSettings {
    match self {
      Self::Channel(channel) => UpdateSettings { channel, ..settings },
      Self::Automatic(automatic) => UpdateSettings { automatic, ..settings },
    }
  }
}

type SaveAction = FutureAction<Change, (), Message>;

/// What the route asks *Updates* to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown(pub Option<UpdatesShow>);

impl DevtoolsInspectable for Shown {}

pub struct UpdatesGroup {
  /// The change being saved, shown until the stored settings have it, or its save failed.
  chosen: Signal<Option<Change>>,
  /// A page or a log that could not be opened.
  unopened: Signal<Option<String>>,
}

impl Component for UpdatesGroup {
  type Props = Shown;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      chosen: ctx.signal(None),
      unopened: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Shown(show) = *ctx.props::<Shown>();
    let services = Services::of(ctx);
    let restart = Restart::of(ctx);
    // Hooks in the same order on every render: the action, the stream, then the read.
    let save = save_action(ctx, &services.updates);
    let saving = save.state().get();
    let state = follow(ctx, &services.updates);
    let stored = queries::state(&ctx.query(app::update_settings(Session(services.clone()))));
    if let Some(saves) = SaveState::of(ctx) {
      saves.report(ID, Outcome::of(&saving));
    }
    let chosen = self.settle(&saving, stored.data.as_ref());
    let settings = stored
      .data
      .as_ref()
      .map(|stored| chosen.map_or(stored.settings, |change| change.applied(stored.settings)));
    // A press refused in the state set for testing says so while that state shows.
    let refused = services.updates.is_set_for_testing() && restart.refused().as_ref() == Some(&state);
    let problem = if refused {
      Some(ctx.t("settings.general.updates.setForTesting").to_string())
    } else {
      self.problem(ctx, (&stored, &saving))
    };
    let facts = UpdatesFacts {
      running: services.updates.this_app().version.to_string(),
      state,
      just_updated: services.updates.just_updated(),
      settings,
      last_checked: stored.data.as_ref().map(|stored| stored.last_checked),
      problem,
      show,
    };
    let dispatch = self.dispatch(&services, restart, save);
    ctx.mount_keyed::<UpdatesSection>("settings-updates-section", SectionProps { facts, dispatch })
  }
}

impl UpdatesGroup {
  /// What the section's presses do in this window.
  fn dispatch(&self, services: &Services, restart: Restart, save: SaveAction) -> UpdateDispatch {
    let (updates, runtime, links) = (
      services.updates.clone(),
      services.runtime.clone(),
      services.links.clone(),
    );
    let (chosen, unopened) = (self.chosen.clone(), self.unopened.clone());
    Arc::new(move |action| match action {
      UpdateAction::CheckNow => {
        let updates = updates.clone();
        runtime.spawn(async move { updates.check_now().await });
      }
      UpdateAction::Pause => updates.pause(),
      UpdateAction::Resume => {
        let updates = updates.clone();
        runtime.spawn(async move { updates.resume().await });
      }
      UpdateAction::Restart => restart.press(),
      UpdateAction::Channel(channel) => {
        chosen.set(Some(Change::Channel(channel)));
        save.run(Change::Channel(channel));
      }
      UpdateAction::Automatic(automatic) => {
        chosen.set(Some(Change::Automatic(automatic)));
        save.run(Change::Automatic(automatic));
      }
      UpdateAction::Open(url) => match links.open(&url) {
        Ok(()) => unopened.set(None),
        Err(error) => {
          tracing::warn!(%error, url, "a page of the update could not be opened");
          unopened.set(Some(url));
        }
      },
      UpdateAction::OpenLog(log) => match orchester_exec::external::open_path(&log) {
        Ok(()) => unopened.set(None),
        Err(error) => {
          tracing::warn!(%error, log = %log.display(), "the installer's log could not be opened");
          unopened.set(Some(log.display().to_string()));
        }
      },
    })
  }

  /// The change being saved, while it saves and until the stored settings have it; forgotten once they do, or once its
  /// save failed, when the stored settings show again.
  fn settle(&self, saving: &FutureState<(), Message>, stored: Option<&StoredUpdates>) -> Option<Change> {
    let chosen = self.chosen.get();
    let (Some(change), Some(stored)) = (chosen, stored) else {
      return chosen;
    };
    if !saving.is_pending() && (saving.is_rejected() || change.applied(stored.settings) == stored.settings) {
      self.chosen.set(None);
      return None;
    }
    chosen
  }

  /// Why the settings can't be read or a change was not saved, or what could not be opened.
  fn problem(
    &self,
    ctx: &Ctx,
    (stored, saving): (&FutureState<StoredUpdates, Message>, &FutureState<(), Message>),
  ) -> Option<String> {
    let (key, reason) = match (&stored.error, &saving.error) {
      (Some(reason), _) => ("settings.general.updates.unreadable", reason),
      (None, Some(reason)) if !saving.is_pending() => ("settings.general.updates.notSaved", reason),
      _ => {
        let url = self.unopened.get()?;
        return Some(
          ctx
            .t_args("settings.general.updates.notOpened", [("url", url)])
            .to_string(),
        );
      }
    };
    let reason = message_text(ctx, reason);
    Some(ctx.t_args(key, [("reason", &*reason)]).to_string())
  }
}

/// Saves the person's change. `AppUpdates` counts the save, which reads the settings' query again (`queries`).
fn save_action(ctx: &mut Ctx, updates: &AppUpdates) -> SaveAction {
  let updates = updates.clone();
  ctx.future_action(move |change: Change| {
    let updates = updates.clone();
    async move {
      background::run(move || match change {
        Change::Channel(channel) => updates.set_channel(channel),
        Change::Automatic(automatic) => updates.set_automatic(automatic),
      })
      .await?
      .map_err(|error| {
        tracing::error!(%error, "an update setting was not saved");
        messages::store_error(&error)
      })
    }
  })
}

/// The update's state whenever it changes.
fn follow(ctx: &mut Ctx, updates: &AppUpdates) -> UpdateState {
  ctx
    .stream(
      updates.clone(),
      |updates, emitter: StreamEmitter<UpdateState, ()>| async move {
        let mut states = updates.subscribe();
        loop {
          let state = states.borrow_and_update().clone();
          if !emitter.emit(state) || states.changed().await.is_err() {
            return;
          }
        }
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_else(|| updates.state())
}
