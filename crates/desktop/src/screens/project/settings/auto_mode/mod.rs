//! Project settings / General › Auto mode (`orchester_core::run::review`; no frame yet, its rows follow
//! *Agents and the repository*): *New runs use auto mode*, off until a person turns it on, and the
//! reviewer: any of the project's agent connections, a model of it from the same list as an agent
//! profile's model field, and the effort where the model has efforts. Until a person chooses, the first
//! Claude Code connection reviews with Haiku 4.5. Each saves when it changes; the topbar says whether it
//! is saved. The row says which model reviews, or why auto mode has no reviewer.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Select, Text},
  core::{Signal, SignalValue},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentConnection, Connection, ModelSupport},
  id::ConnectionId,
  resource::ModelId,
  run::review::{AutoModeSettings, ReviewerChoice, default_review_model, resolve_reviewer, reviewer_kind},
  store::ConnectionStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  super::task::auto_mode::reviewed_by,
  SettingsSave,
  general::{readback, sized_setting_row},
};
use crate::{
  agents::form::{effort_options, model_options},
  background,
  messages::{self, Message},
  tasks::run_approvals::{ReviewerShown, unavailable_key},
  theme::ExtraTypography,
  ui::{
    field::{field_error, select_style},
    message_text,
    switch::{Switch, locked_reason},
  },
  workspace::OpenProject,
};

/// A setting's value column (`LmaBU`) and a select's height (`bhw3I`).
const VALUE_WIDTH: f32 = 300.0;
const SELECT_HEIGHT: f32 = 32.0;

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

/// The settings as stored, and the project's agent connections.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stored {
  settings: AutoModeSettings,
  connections: Vec<Connection>,
}

impl lurq::app::component::DevtoolsInspectable for Stored {}

impl Stored {
  /// The reviewer these settings resolve to: its connection, model and effort.
  fn resolved(&self) -> Option<(&Connection, ModelId, Option<String>)> {
    resolve_reviewer(&self.settings, &self.connections)
      .ok()
      .map(|resolved| (resolved.connection, resolved.model, resolved.effort))
  }
}

/// The settings a save stored.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Saved(AutoModeSettings);

impl lurq::app::component::DevtoolsInspectable for Saved {}

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

type SaveAction = FutureAction<(PathBuf, AutoModeSettings), Saved, Message>;

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
/// earlier one saves is saved right after it, so none is lost and the last one wins.
#[derive(Clone)]
struct Commit {
  stored: Signal<Option<Stored>>,
  picks: Picks,
  save: SaveAction,
  root: PathBuf,
}

impl Commit {
  /// Applies `next` to the settings shown, when it changes them, and saves them.
  fn change(&self, next: &dyn Fn(&Stored) -> Option<AutoModeSettings>) {
    let Some(mut current) = self.stored.get_untracked() else {
      return;
    };
    let Some(settings) = next(&current).filter(|settings| *settings != current.settings) else {
      return;
    };
    current.settings = settings.clone();
    self.stored.set(Some(current.clone()));
    self.picks.show(&current);
    if !self.save.state().get_untracked().is_pending() {
      self.save.run((self.root.clone(), settings));
    }
  }

  /// After a save of `saved`: saves the settings shown again when a change came meanwhile.
  fn saved(&self, saved: &AutoModeSettings) -> bool {
    let Some(current) = self.stored.get_untracked() else {
      return true;
    };
    if current.settings == *saved {
      return true;
    }
    self.save.run((self.root.clone(), current.settings));
    false
  }
}

pub struct AutoModeSection {
  stored: Signal<Option<Stored>>,
  picks: Picks,
  commit: Commit,
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
      picks: picks.clone(),
      save: save.clone(),
      root: ctx.props::<AutoModeSettingsProps>().open.repo_root.clone(),
    };
    let (status, after) = (ctx.props::<AutoModeSettingsProps>().status.clone(), commit.clone());
    ctx.watch(&save.state(), move |state| {
      if state.is_pending() {
        status.set(SettingsSave::Saving);
        return;
      }
      match (&state.data, &state.error) {
        (_, Some(error)) => status.set(SettingsSave::Failed(error.clone())),
        (Some(Saved(saved)), None) => {
          if after.saved(saved) {
            status.set(SettingsSave::Saved);
          }
        }
        (None, None) => {}
      }
    });
    let on_connection = commit.clone();
    ctx.watch(&picks.connection, move |pick| {
      on_connection.change(&|current| Some(with_connection(current, pick.0)));
    });
    let on_model = commit.clone();
    ctx.watch(&picks.model, move |chosen| {
      on_model.change(&|current| chosen.0.clone().and_then(|model| with_model(current, model)));
    });
    let on_effort = commit.clone();
    ctx.watch(&picks.effort, move |pick| {
      on_effort.change(&|current| with_effort(current, pick.0.clone()));
    });
    Self { stored, picks, commit }
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
      .child(heading);
    let Some(stored) = self.stored.get() else {
      return column.with_children(read_error.map(|error| field_error(&message_text(ctx, &error))));
    };
    column
      .child(new_runs_row(ctx, &stored, &self.commit))
      .child(self.reviewer_row(ctx, &stored))
  }
}

/// The settings with the reviewer on connection `pick` (`None`: the default) and its default model; the
/// same settings when `pick` is the connection already chosen.
fn with_connection(current: &Stored, pick: Option<ConnectionId>) -> AutoModeSettings {
  if pick == current.settings.reviewer.as_ref().map(|choice| choice.connection) {
    return current.settings.clone();
  }
  let reviewer = pick.and_then(|id| {
    let connection = current.connections.iter().find(|connection| connection.id() == id)?;
    Some(ReviewerChoice {
      connection: id,
      model: default_review_model(connection)?,
      effort: None,
    })
  });
  // A connection that names no model to review with cannot be chosen yet: the settings stay.
  match (pick, reviewer) {
    (Some(_), None) => current.settings.clone(),
    (_, reviewer) => AutoModeSettings {
      reviewer,
      ..current.settings.clone()
    },
  }
}

/// The settings with the resolved reviewer's connection reviewing with `model`; its effort stays when the
/// model takes it.
fn with_model(current: &Stored, model: ModelId) -> Option<AutoModeSettings> {
  let (connection, resolved, effort) = current.resolved()?;
  if resolved == model {
    return None;
  }
  let takes = |effort: &String| {
    AgentConnection::try_from(connection.clone()).is_ok_and(|agent| effort_options(&agent, &model).contains(effort))
  };
  Some(AutoModeSettings {
    reviewer: Some(ReviewerChoice {
      connection: connection.id(),
      effort: effort.filter(takes),
      model,
    }),
    ..current.settings.clone()
  })
}

/// The settings with the resolved reviewer at `effort`.
fn with_effort(current: &Stored, effort: Option<String>) -> Option<AutoModeSettings> {
  let (connection, model, resolved) = current.resolved()?;
  (resolved != effort).then(|| AutoModeSettings {
    reviewer: Some(ReviewerChoice {
      connection: connection.id(),
      model,
      effort,
    }),
    ..current.settings.clone()
  })
}

impl AutoModeSection {
  /// Reads the stored settings and the connections once; why they cannot be read, if so.
  fn load(&self, ctx: &mut Ctx, open: &OpenProject) -> Option<Message> {
    let read = ctx.future(open.repo_root.clone(), |repo_root| async move {
      background::run(move || stored_in(&repo_root)).await?.map_err(|error| {
        tracing::error!(%error, "cannot read the auto mode settings");
        messages::store_error(&error)
      })
    });
    let state = read.state().get();
    if let (Some(read), None) = (state.data, self.stored.get_untracked()) {
      self.picks.show(&read);
      self.stored.set(Some(read));
    }
    state.error
  }

  /// *Reviewer*: the connection, its model and effort, and which model reviews or why none does.
  fn reviewer_row(&self, ctx: &Ctx, stored: &Stored) -> Element {
    // Without an agent connection there is nothing to choose: the row says why auto mode has no reviewer.
    let mut value: Vec<Element> = (!stored.connections.is_empty())
      .then(|| self.connection_select(ctx, stored))
      .into_iter()
      .collect();
    let resolved = resolve_reviewer(&stored.settings, &stored.connections);
    match &resolved {
      Ok(reviewer) => {
        let agent = AgentConnection::try_from(reviewer.connection.clone()).ok();
        if let Some(agent) = &agent {
          value.push(self.model_select(ctx, agent, &reviewer.model));
          value.extend(self.effort_select(ctx, agent, &reviewer.model));
        }
        let shown = ReviewerShown {
          connection: reviewer.connection.name().as_str().into(),
          model: reviewer.model.clone(),
          effort: reviewer.effort.clone(),
        };
        value.push(readback(&reviewed_by(ctx, &shown)).id("settings-auto-mode-reviewer"));
      }
      Err(why) => value.push(locked_reason(
        "settings-auto-mode-unavailable".to_owned(),
        &ctx.t(unavailable_key(*why)),
      )),
    }
    sized_setting_row(
      VALUE_WIDTH,
      &ctx.t("settings.autoMode.reviewerName"),
      &ctx.t("settings.autoMode.reviewerDescription"),
      value,
    )
  }

  /// The project's agent connections, the default reviewer first.
  fn connection_select(&self, ctx: &Ctx, stored: &Stored) -> Element {
    let default = resolve_reviewer(
      &AutoModeSettings {
        reviewer: None,
        ..stored.settings.clone()
      },
      &stored.connections,
    )
    .map_or_else(
      |_| ctx.t("settings.autoMode.defaultNone"),
      |reviewer| {
        ctx.t_args(
          "settings.autoMode.defaultConnection",
          [("connection", reviewer.connection.name().as_str())],
        )
      },
    );
    let mut options: Vec<(ConnectionPick, Arc<str>)> = vec![(ConnectionPick(None), default)];
    for connection in &stored.connections {
      let name = connection.name().as_str();
      let label = if reviewer_kind(connection).is_some() {
        name.into()
      } else {
        ctx.t_args("settings.autoMode.cannotReview", [("connection", name)])
      };
      options.push((ConnectionPick(Some(connection.id())), label));
    }
    if let Some(choice) = &stored.settings.reviewer
      && !stored
        .connections
        .iter()
        .any(|connection| connection.id() == choice.connection)
    {
      options.push((
        ConnectionPick(Some(choice.connection)),
        ctx.t("settings.autoMode.removedConnection"),
      ));
    }
    select(
      ctx,
      Select::new(self.picks.connection.clone()).options(options),
      "settings-auto-mode-connection",
    )
  }

  /// The reviewer connection's models, as an agent profile's model field lists them.
  fn model_select(&self, ctx: &Ctx, connection: &AgentConnection, current: &ModelId) -> Element {
    let options: Vec<(Chosen, Arc<str>)> = model_options(connection, Some(current))
      .into_iter()
      .map(|option| {
        let label = match option.support {
          ModelSupport::Listed => option.id.as_str().into(),
          ModelSupport::NotVerified => ctx.t_args("agents.modelNotVerified", [("model", option.id.as_str())]),
        };
        (Chosen(Some(option.id)), label)
      })
      .collect();
    select(
      ctx,
      Select::new(self.picks.model.clone()).options(options),
      "settings-auto-mode-model",
    )
  }

  /// The model's efforts, when it has any.
  fn effort_select(&self, ctx: &Ctx, connection: &AgentConnection, model: &ModelId) -> Option<Element> {
    let efforts = effort_options(connection, model);
    if efforts.is_empty() {
      return None;
    }
    let options = std::iter::once((EffortPick(None), ctx.t("agents.effortDefault"))).chain(
      efforts
        .iter()
        .map(|effort| (EffortPick(Some(effort.clone())), effort.as_str().into())),
    );
    Some(select(
      ctx,
      Select::new(self.picks.effort.clone()).options(options),
      "settings-auto-mode-effort",
    ))
  }
}

/// `select` at the settings' select height, `id` for tooling.
fn select<T>(ctx: &Ctx, select: Select<T>, id: &'static str) -> Element
where
  T: SignalValue + Clone + PartialEq + Send + Sync + 'static,
  Vec<T>: SignalValue,
{
  Element::from(
    select
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(SELECT_HEIGHT))
      .style(select_style(ctx, SELECT_HEIGHT)),
  )
  .id(id)
}

/// *New runs use auto mode*, which saves when it flips; fixed while auto mode has no reviewer.
fn new_runs_row(ctx: &Ctx, stored: &Stored, commit: &Commit) -> Element {
  let on = stored.settings.new_runs;
  let state = ctx.t(if on {
    "settings.autoMode.on"
  } else {
    "settings.autoMode.off"
  });
  let switch = Switch::new("settings-auto-mode-new-runs", state, on).emphasized();
  let switch = if stored.resolved().is_some() {
    let commit = commit.clone();
    switch.render(move || {
      commit.change(&|current| {
        Some(AutoModeSettings {
          new_runs: !current.settings.new_runs,
          ..current.settings.clone()
        })
      });
    })
  } else {
    switch.locked()
  };
  sized_setting_row(
    VALUE_WIDTH,
    &ctx.t("settings.autoMode.newRunsName"),
    &ctx.t("settings.autoMode.newRunsDescription"),
    vec![switch],
  )
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  ctx.future_action(|(repo_root, settings): (PathBuf, AutoModeSettings)| async move {
    let saved = settings.clone();
    background::run(move || ProjectDb::open(&repo_root)?.save_auto_mode_settings(&settings))
      .await?
      .map(|()| Saved(saved))
      .map_err(|error| {
        tracing::error!(%error, "cannot save the auto mode settings");
        messages::store_error(&error)
      })
  })
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
