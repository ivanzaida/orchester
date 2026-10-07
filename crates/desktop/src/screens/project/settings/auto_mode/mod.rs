//! Project settings / General › Auto mode (`orchester_core::run::review`; no frame yet, its rows follow
//! *Agents and the repository*): *New runs use auto mode*, off until a person turns it on, and the
//! reviewer: any of the project's agent connections, a model of it from the same list as an agent
//! profile's model field, and the effort where the model has efforts. Until a person chooses, the first
//! Claude Code connection reviews with Haiku 4.5. Each saves when it changes; the topbar says whether it
//! is saved, and a change that is refused or cannot be saved says why there while the section shows the
//! settings as they were. The row says which model reviews, or why auto mode has no reviewer.

mod choice;
mod rows;

use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Text},
  core::{Signal, SignalValue},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::AgentConnection, id::ConnectionId, resource::ModelId, run::review::AutoModeSettings,
  store::ConnectionStore,
};
use orchester_store::{ProjectDb, StoreError};

use self::{
  choice::{Change, Stored, with_connection, with_effort, with_model},
  rows::new_runs_row,
};
use super::{SettingsSave, outside::OutsideSettings};
use crate::{
  background,
  messages::{self, Message},
  theme::ExtraTypography,
  ui::{field::field_error, message_text},
  workspace::OpenProject,
};

#[derive(Clone)]
pub struct AutoModeSettingsProps {
  pub open: OpenProject,
  /// Written by the section, read by the settings topbar.
  pub status: Signal<SettingsSave>,
}

impl PartialEq for AutoModeSettingsProps {
  fn eq(&self, other: &Self) -> bool {
    self.open == other.open
  }
}

impl lurq::app::component::DevtoolsInspectable for AutoModeSettingsProps {}

/// The settings as last read or saved.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Saved(AutoModeSettings);

impl lurq::app::component::DevtoolsInspectable for Saved {}

/// What a save was asked to store, and what it stored: the person's change put on the settings as
/// they were stored then, so a setting automation changed meanwhile keeps its change.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Done {
  asked: AutoModeSettings,
  stored: AutoModeSettings,
}

impl lurq::app::component::DevtoolsInspectable for Done {}

/// The stored settings at a reading of [`OutsideSettings`].
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read(Stored, u64);

impl lurq::app::component::DevtoolsInspectable for Read {}

/// The connection chosen in its select: `None` for the default reviewer.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ConnectionPick(Option<ConnectionId>);

impl lurq::app::component::DevtoolsInspectable for ConnectionPick {}

/// The model chosen in its select; `None` until the stored one is read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Chosen(Option<ModelId>);

impl lurq::app::component::DevtoolsInspectable for Chosen {}

/// The effort chosen in its select; `None` for the connection's default.
#[derive(Clone, Debug, PartialEq, Eq)]
struct EffortPick(Option<String>);

impl lurq::app::component::DevtoolsInspectable for EffortPick {}

/// Where to save, the settings the change was made on, and the settings it makes.
type SaveAction = FutureAction<(PathBuf, AutoModeSettings, AutoModeSettings), Done, Message>;

/// The selects' values: each saves when a person changes it.
#[derive(Clone)]
struct Picks {
  connection: Signal<ConnectionPick>,
  model: Signal<Chosen>,
  effort: Signal<EffortPick>,
}

impl Picks {
  /// Shows `stored`'s reviewer in the selects.
  fn show(&self, stored: &Stored) {
    let resolved = stored.resolved();
    set_changed(
      &self.connection,
      ConnectionPick(stored.settings.reviewer.as_ref().map(|choice| choice.connection)),
    );
    set_changed(
      &self.model,
      Chosen(resolved.as_ref().map(|(_, model, _)| model.clone())),
    );
    set_changed(&self.effort, EffortPick(resolved.and_then(|(_, _, effort)| effort)));
  }
}

/// Sets `signal` to `value` only when it differs, so a select shows a saved value without a change of
/// its own.
fn set_changed<T: SignalValue + Clone + PartialEq + Send + Sync + 'static>(signal: &Signal<T>, value: T) {
  if signal.get_untracked() != value {
    signal.set(value);
  }
}

/// Takes a person's change at once: the section shows it while it saves, and a change made while an
/// earlier one saves is saved right after it, so none is lost and the last one wins. A change that is
/// refused, or a save that fails, says why in the topbar and the section shows the last saved settings.
#[derive(Clone)]
struct Commit {
  stored: Signal<Option<Stored>>,
  /// The settings as last read or saved.
  saved: Signal<Option<Saved>>,
  picks: Picks,
  save: SaveAction,
  status: Signal<SettingsSave>,
  root: PathBuf,
}

impl Commit {
  /// Applies `next` to the settings shown, when it changes them, and saves them.
  fn change(&self, next: &dyn Fn(&Stored) -> Change) {
    let Some(mut current) = self.stored.get_untracked() else {
      return;
    };
    let Some(Saved(base)) = self.saved.get_untracked() else {
      return;
    };
    let settings = match next(&current) {
      Ok(Some(settings)) if settings != current.settings => settings,
      Ok(_) => {
        self.picks.show(&current);
        return;
      }
      Err(refused) => {
        self.picks.show(&current);
        self.status.set(SettingsSave::Failed(refused));
        return;
      }
    };
    current.settings = settings.clone();
    self.stored.set(Some(current.clone()));
    self.picks.show(&current);
    if !self.save.state().get_untracked().is_pending() {
      self.save.run((self.root.clone(), base, settings));
    }
  }

  /// After a save: shows what it stored, or saves the settings shown again when a change came
  /// meanwhile.
  fn saved(&self, done: &Done) {
    self.saved.set(Some(Saved(done.stored.clone())));
    let Some(mut current) = self.stored.get_untracked() else {
      return;
    };
    if current.settings == done.asked {
      if current.settings != done.stored {
        current.settings = done.stored.clone();
        self.stored.set(Some(current.clone()));
        self.picks.show(&current);
      }
      self.status.set(SettingsSave::Saved);
    } else {
      self.save.run((self.root.clone(), done.asked.clone(), current.settings));
    }
  }

  /// After a failed save: the section shows the last saved settings again, and the topbar says why.
  fn failed(&self, error: &Message) {
    if let (Some(mut current), Some(Saved(saved))) = (self.stored.get_untracked(), self.saved.get_untracked()) {
      current.settings = saved;
      self.stored.set(Some(current.clone()));
      self.picks.show(&current);
    }
    self.status.set(SettingsSave::Failed(error.clone()));
  }
}

pub struct AutoModeSection {
  stored: Signal<Option<Stored>>,
  picks: Picks,
  commit: Commit,
  outside: OutsideSettings,
}

impl Component for AutoModeSection {
  type Props = AutoModeSettingsProps;

  fn create(ctx: &mut Ctx) -> Self {
    let stored = ctx.signal(None);
    let picks = Picks {
      connection: ctx.signal(ConnectionPick(None)),
      model: ctx.signal(Chosen(None)),
      effort: ctx.signal(EffortPick(None)),
    };
    let save = save_action(ctx);
    let commit = Commit {
      stored: stored.clone(),
      saved: ctx.signal(None),
      picks: picks.clone(),
      save: save.clone(),
      status: ctx.props::<AutoModeSettingsProps>().status.clone(),
      root: ctx.props::<AutoModeSettingsProps>().open.repo_root.clone(),
    };
    let after = commit.clone();
    ctx.watch(&save.state(), move |state| {
      if state.is_pending() {
        after.status.set(SettingsSave::Saving);
        return;
      }
      match (&state.data, &state.error) {
        (_, Some(error)) => after.failed(error),
        (Some(done), None) => after.saved(done),
        (None, None) => {}
      }
    });
    let on_connection = commit.clone();
    ctx.watch(&picks.connection, move |pick| {
      on_connection.change(&|current| with_connection(current, pick.0));
    });
    let on_model = commit.clone();
    ctx.watch(&picks.model, move |chosen| {
      on_model.change(&|current| match chosen.0.clone() {
        Some(model) => with_model(current, model),
        None => Ok(None),
      });
    });
    let on_effort = commit.clone();
    ctx.watch(&picks.effort, move |pick| {
      on_effort.change(&|current| with_effort(current, pick.0.clone()));
    });
    Self {
      stored,
      picks,
      commit,
      outside: OutsideSettings::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<AutoModeSettingsProps>().open.clone();
    // Futures are matched by call order: the action comes first, as in `create`.
    save_action(ctx);
    let read_error = self.load(ctx, &open);
    let heading = Text::new(&ctx.t("settings.autoMode.section")).variant(ExtraTypography::SectionHeading.role());
    let column = Column::new()
      .id("settings-auto-mode-section")
      .width(Dimension::Pct(100.0))
      .padding_top(SpacingSize::Md)
      .spacing(SpacingSize::Sm)
      .child(heading)
      .with_children(self.outside.notice(ctx, "settings-auto-mode-changed-outside"));
    let read_error = read_error.map(|error| field_error(&message_text(ctx, &error)));
    let Some(stored) = self.stored.get() else {
      return column.with_children(read_error);
    };
    // A failed reading after a change keeps the settings as last read, and says why.
    column
      .with_children(read_error)
      .child(new_runs_row(ctx, &stored, &self.commit))
      .child(self.reviewer_row(ctx, &stored))
  }
}

impl AutoModeSection {
  /// Reads the stored settings and the connections, again after automation changed them (unless the
  /// person's change is saving, see [`OutsideSettings`]); why they cannot be read, if so.
  fn load(&self, ctx: &mut Ctx, open: &OpenProject) -> Option<Message> {
    let deps = (open.repo_root.clone(), self.outside.reading(ctx));
    let read = ctx.future(deps, |(repo_root, reading)| async move {
      background::run(move || stored_in(&repo_root))
        .await?
        .map(|stored| Read(stored, reading))
        .map_err(|error| {
          tracing::error!(%error, "cannot read the auto mode settings");
          messages::store_error(&error)
        })
    });
    let state = read.state().get();
    let saving = self.commit.save.state().get().is_pending();
    if !state.is_pending()
      && let Some(Read(read, reading)) = state.data
      && self.outside.take(reading, saving)
    {
      self.picks.show(&read);
      self.commit.saved.set(Some(Saved(read.settings.clone())));
      self.stored.set(Some(read));
    }
    state.error
  }
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  ctx.future_action(
    |(repo_root, base, asked): (PathBuf, AutoModeSettings, AutoModeSettings)| async move {
      background::run(move || {
        let mut db = ProjectDb::open(&repo_root)?;
        let stored = rebased(&base, &asked, db.auto_mode_settings()?);
        db.save_auto_mode_settings(&stored)?;
        Ok::<_, StoreError>(Done { asked, stored })
      })
      .await?
      .map_err(|error| {
        tracing::error!(%error, "cannot save the auto mode settings");
        messages::store_error(&error)
      })
    },
  )
}

/// The person's change from `base` to `asked`, put on the settings as stored now (`current`): what
/// they did not change keeps what automation may have changed meanwhile.
fn rebased(base: &AutoModeSettings, asked: &AutoModeSettings, current: AutoModeSettings) -> AutoModeSettings {
  AutoModeSettings {
    new_runs: if asked.new_runs == base.new_runs {
      current.new_runs
    } else {
      asked.new_runs
    },
    reviewer: if asked.reviewer == base.reviewer {
      current.reviewer
    } else {
      asked.reviewer.clone()
    },
  }
}

/// The settings of the project at `repo_root`, and its agent connections.
fn stored_in(repo_root: &std::path::Path) -> Result<Stored, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  let connections = db
    .connections()?
    .into_iter()
    .filter(|connection| AgentConnection::try_from(connection.clone()).is_ok())
    .collect();
  Ok(Stored {
    settings: db.auto_mode_settings()?,
    connections,
  })
}
