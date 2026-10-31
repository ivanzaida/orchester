//! The status bar's update item (`Row 2 · status bar` of `Auto-updater · redesign` `acFkz`): it shows only while the
//! update needs the person or has news. Ready, *Update 0.4.0 ready · Restart* in `info` with `circle-arrow-up` (S2
//! `RccSH`), whose press is *Restart to update*: it installs at once with no run at work, otherwise it asks (03
//! `MGILm`, [`crate::shell::update`]); waiting for runs, *Update 0.4.0 · after 3 runs end* with `hourglass` (S3
//! `UCZt3`), whose press asks again; installing, *Installing 0.4.0*; after the update, for the first minutes, *Updated
//! to 0.4.0* in `success` with `circle-check` (S6 `wZaZb`); and an update that did not install in `danger`, its card
//! saying why and its press opening the installer's log.

use std::{
  path::PathBuf,
  sync::Arc,
  time::{Duration, Instant},
};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, StreamEmitter},
    theme::PaletteColor,
  },
  node::Element,
};
use orchester_workspace::app_updates::{
  AppUpdates, UpdateState,
  install::{InstallStep, JustUpdated},
};

use super::{
  facts::{BarAction, Dispatch},
  item::{glyph, item, label, pressed},
};
use crate::ui::{
  hover_card::{CardWords, hover_card},
  icon::Icon,
  pressable::Press,
};

/// How long *Updated to* shows after the update's first start.
const UPDATED_FOR: Duration = Duration::from_secs(10 * 60);

/// What the item shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateFacts {
  /// Nothing the person needs to know: no item.
  #[default]
  Quiet,
  /// A verified update is ready; no run is at work.
  Ready { version: String },
  /// It waits for `runs` runs at work to end.
  Waiting { version: String, runs: usize },
  /// The runners stop, the databases are copied aside and the installer takes over.
  Installing { version: String },
  /// This start finished the update to `version`.
  Updated { version: String },
  /// `version` did not install, for `reason`; the installer's log, when it wrote one.
  NotInstalled {
    version: String,
    reason: String,
    log: Option<PathBuf>,
  },
}

impl DevtoolsInspectable for UpdateFacts {}

impl UpdateFacts {
  /// The item for `state`, with `updated` while *Updated to* shows.
  pub fn of(state: &UpdateState, updated: Option<&JustUpdated>) -> Self {
    match state {
      UpdateState::Ready(update) => Self::Ready {
        version: update.version.to_string(),
      },
      UpdateState::WaitingForRuns { update, runs }
      | UpdateState::Installing {
        update,
        step: InstallStep::WaitingForRuns(runs),
      } => Self::Waiting {
        version: update.version.to_string(),
        runs: runs.len(),
      },
      UpdateState::Installing { update, .. } => Self::Installing {
        version: update.version.to_string(),
      },
      UpdateState::InstallFailed { version, failure } => Self::NotInstalled {
        version: version.to_string(),
        reason: failure.reason.clone(),
        log: failure.log.clone(),
      },
      _ => updated.map_or(Self::Quiet, |updated| Self::Updated {
        version: updated.version.to_string(),
      }),
    }
  }
}

/// The item's facts, whenever the update's state changes, with *Updated to* for its first minutes.
pub fn update_facts(ctx: &mut Ctx, updates: &AppUpdates) -> UpdateFacts {
  ctx
    .stream(
      updates.clone(),
      |updates, emitter: StreamEmitter<UpdateFacts, ()>| async move {
        let until = Instant::now() + UPDATED_FOR;
        let mut changes = updates.subscribe();
        loop {
          let fresh = Instant::now() < until;
          let state = changes.borrow_and_update().clone();
          let facts = UpdateFacts::of(&state, updates.just_updated().filter(|_| fresh));
          if !emitter.emit(facts) {
            return;
          }
          tokio::select! {
            changed = changes.changed() => if changed.is_err() { return },
            () = tokio::time::sleep_until(until.into()), if fresh => {}
          }
        }
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_default()
}

/// The item, or nothing while the update has nothing to say.
pub fn item_of(ctx: &mut Ctx, facts: &UpdateFacts, dispatch: &Dispatch) -> Option<Element> {
  let restart = || {
    let dispatch = dispatch.clone();
    Press::run(move || dispatch(BarAction::RestartToUpdate))
  };
  let element = match facts {
    UpdateFacts::Quiet => return None,
    UpdateFacts::Ready { version } => {
      let words = ctx
        .t_args("shell.status.update.ready", [("version", version.clone())])
        .to_string();
      pressed("status-update", restart())
        .describe("state", "ready")
        .describe("a11y_name", words.clone())
        .child(glyph(Icon::CircleArrowUp, PaletteColor::Info))
        .child(label(&words, PaletteColor::Info).id("status-update-text"))
        .into()
    }
    UpdateFacts::Waiting { version, runs } => {
      let key = if *runs == 1 {
        "shell.status.update.waiting.one"
      } else {
        "shell.status.update.waiting.other"
      };
      let words = ctx
        .t_args(key, [("version", version.clone()), ("count", runs.to_string())])
        .to_string();
      pressed("status-update", restart())
        .describe("state", "waiting")
        .describe("a11y_name", words.clone())
        .child(glyph(Icon::Hourglass, PaletteColor::Info))
        .child(label(&words, PaletteColor::Info).id("status-update-text"))
        .into()
    }
    UpdateFacts::Installing { version } => {
      let words = ctx
        .t_args("shell.status.update.installing", [("version", version.clone())])
        .to_string();
      item("status-update")
        .describe("state", "installing")
        .describe("a11y_name", words.clone())
        .child(glyph(Icon::LoaderCircle, PaletteColor::Info))
        .child(label(&words, PaletteColor::Info).id("status-update-text"))
        .into()
    }
    UpdateFacts::Updated { version } => {
      let words = ctx
        .t_args("shell.status.update.updated", [("version", version.clone())])
        .to_string();
      item("status-update")
        .describe("state", "updated")
        .describe("a11y_name", words.clone())
        .child(glyph(Icon::CircleCheck, PaletteColor::Success))
        .child(label(&words, PaletteColor::Success).id("status-update-text"))
        .into()
    }
    UpdateFacts::NotInstalled { version, reason, log } => {
      return Some(not_installed(ctx, version, reason, log, dispatch));
    }
  };
  Some(element)
}

/// An update that did not install: its card says why; a press opens the installer's log, when it wrote one.
fn not_installed(ctx: &mut Ctx, version: &str, reason: &str, log: &Option<PathBuf>, dispatch: &Dispatch) -> Element {
  let words = ctx
    .t_args("shell.status.update.failed", [("version", version.to_owned())])
    .to_string();
  let press = match log {
    Some(log) => {
      let (dispatch, log) = (dispatch.clone(), log.clone());
      Press::run(move || dispatch(BarAction::OpenFile(log.clone())))
    }
    None => Press::run(|| {}),
  };
  let anchor = pressed("status-update", press)
    .describe("state", "failed")
    .describe("reason", reason.to_owned())
    .describe("a11y_name", words.clone())
    .child(glyph(Icon::CircleAlert, PaletteColor::Danger))
    .child(label(&words, PaletteColor::Danger).id("status-update-text"));
  let hint = if log.is_some() {
    "shell.update.failedHintLog"
  } else {
    "shell.update.failedHint"
  };
  let card = CardWords {
    id: Arc::from("status-update-card"),
    title: words,
    dot: PaletteColor::Danger,
    state: reason.to_owned(),
    body: ctx.t("shell.update.failedBody").to_string(),
    hint: ctx.t(hint).to_string(),
    note: None,
  };
  hover_card(ctx, anchor, card)
}
