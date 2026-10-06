//! Project settings / General › Auto mode (`orchester_core::run::review`; no frame yet, its rows follow
//! *Agents and the repository*): *New runs use auto mode*, off until a person turns it on, and the model
//! of the project's Claude Code connection that reviews. Each saves when it changes; the topbar says
//! whether it is saved. Without a Claude Code connection auto mode is unavailable, and the section says so.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Select, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentConnection, ModelSupport, reviewer::reviewer_connection},
  resource::ModelId,
  run::review::AutoModeSettings,
  store::ConnectionStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  SettingsSave,
  general::{readback, sized_setting_row},
};
use crate::{
  agents::form::model_options,
  background,
  messages::{self, Message},
  theme::ExtraTypography,
  ui::{
    field::{field_error, select_style},
    message_text,
    switch::{Switch, locked_reason},
  },
  workspace::OpenProject,
};

/// A setting's value column (`LmaBU`) and the model select's height (`bhw3I`).
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

/// The settings as stored, and the connection that reviews.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stored {
  settings: AutoModeSettings,
  reviewer: Option<AgentConnection>,
}

impl lurq::app::component::DevtoolsInspectable for Stored {}

/// The settings a save stored.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Saved(AutoModeSettings);

impl lurq::app::component::DevtoolsInspectable for Saved {}

/// The model chosen in the select; `None` until the stored one is read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Chosen(Option<ModelId>);

impl lurq::app::component::DevtoolsInspectable for Chosen {}

type SaveAction = FutureAction<(PathBuf, AutoModeSettings), Saved, Message>;

pub struct AutoModeSection {
  stored: Signal<Option<Stored>>,
  /// The model chosen in the select; saved when it differs from the stored one.
  model: Signal<Chosen>,
}

impl Component for AutoModeSection {
  type Props = AutoModeSettingsProps;

  fn create(ctx: &mut Ctx) -> Self {
    let section = Self {
      stored: ctx.signal(None),
      model: ctx.signal(Chosen(None)),
    };
    let save = save_action(ctx);
    let (status, stored) = (
      ctx.props::<AutoModeSettingsProps>().status.clone(),
      section.stored.clone(),
    );
    ctx.watch(&save.state(), move |state| {
      if state.is_pending() {
        status.set(SettingsSave::Saving);
        return;
      }
      match (&state.data, &state.error) {
        (_, Some(error)) => status.set(SettingsSave::Failed(error.clone())),
        (Some(Saved(saved)), None) => {
          stored.update(|stored| {
            if let Some(stored) = stored {
              stored.settings = saved.clone();
            }
          });
          status.set(SettingsSave::Saved);
        }
        (None, None) => {}
      }
    });
    let (stored, root) = (
      section.stored.clone(),
      ctx.props::<AutoModeSettingsProps>().open.repo_root.clone(),
    );
    ctx.watch(&section.model, move |model| {
      let Some(current) = stored.get_untracked() else {
        return;
      };
      match &model.0 {
        Some(model) if *model != current.settings.model && !save.state().get_untracked().is_pending() => {
          let settings = AutoModeSettings {
            model: model.clone(),
            ..current.settings
          };
          save.run((root.clone(), settings));
        }
        _ => {}
      }
    });
    section
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<AutoModeSettingsProps>().open.clone();
    // Futures are matched by call order: the action comes first, as in `create`.
    let save = save_action(ctx);
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
    column.child(new_runs_row(ctx, (&open, &stored), &save)).with_children(
      stored
        .reviewer
        .as_ref()
        .map(|reviewer| self.model_row(ctx, reviewer, &stored.settings.model)),
    )
  }
}

impl AutoModeSection {
  /// Reads the stored settings and the reviewer's connection once; why they cannot be read, if so.
  fn load(&self, ctx: &mut Ctx, open: &OpenProject) -> Option<Message> {
    let read = ctx.future(open.repo_root.clone(), |repo_root| async move {
      background::run(move || stored_in(&repo_root)).await?.map_err(|error| {
        tracing::error!(%error, "cannot read the auto mode settings");
        messages::store_error(&error)
      })
    });
    let state = read.state().get();
    if let (Some(read), None) = (state.data, self.stored.get_untracked()) {
      self.model.set(Chosen(Some(read.settings.model.clone())));
      self.stored.set(Some(read));
    }
    state.error
  }

  /// *Reviewer model*: the reviewer connection's models, the stored one first among them if it lists it
  /// not.
  fn model_row(&self, ctx: &Ctx, reviewer: &AgentConnection, current: &ModelId) -> Element {
    let options: Vec<(Chosen, Arc<str>)> = model_options(reviewer, Some(current))
      .into_iter()
      .map(|option| {
        let label = match option.support {
          ModelSupport::Listed => option.id.as_str().into(),
          ModelSupport::NotVerified => ctx.t_args("agents.modelNotVerified", [("model", option.id.as_str())]),
        };
        (Chosen(Some(option.id)), label)
      })
      .collect();
    let select = Element::from(
      Select::new(self.model.clone())
        .options(options)
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(SELECT_HEIGHT))
        .style(select_style(ctx, SELECT_HEIGHT)),
    )
    .id("settings-auto-mode-model");
    let connection = ctx.t_args(
      "settings.autoMode.connection",
      [("connection", reviewer.name().as_str())],
    );
    sized_setting_row(
      VALUE_WIDTH,
      &ctx.t("settings.autoMode.modelName"),
      &ctx.t("settings.autoMode.modelDescription"),
      vec![select, readback(&connection)],
    )
  }
}

/// *New runs use auto mode*, which saves when it flips.
fn new_runs_row(ctx: &Ctx, (open, stored): (&OpenProject, &Stored), save: &SaveAction) -> Element {
  let on = stored.settings.new_runs;
  let state = ctx.t(if on {
    "settings.autoMode.on"
  } else {
    "settings.autoMode.off"
  });
  let switch = Switch::new("settings-auto-mode-new-runs", state, on).emphasized();
  let switch = if stored.reviewer.is_some() {
    let (save, root) = (save.clone(), open.repo_root.clone());
    let next = AutoModeSettings {
      new_runs: !on,
      ..stored.settings.clone()
    };
    switch.render(move || {
      if !save.state().get_untracked().is_pending() {
        save.run((root.clone(), next.clone()));
      }
    })
  } else {
    switch.locked()
  };
  // Without a reviewer the switch is fixed, and says why in its own row.
  let unavailable = stored.reviewer.is_none().then(|| {
    locked_reason(
      "settings-auto-mode-unavailable".to_owned(),
      &ctx.t("settings.autoMode.unavailable"),
    )
  });
  sized_setting_row(
    VALUE_WIDTH,
    &ctx.t("settings.autoMode.newRunsName"),
    &ctx.t("settings.autoMode.newRunsDescription"),
    std::iter::once(switch).chain(unavailable).collect(),
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

/// The settings of the project at `repo_root`, and the connection its reviews would use.
fn stored_in(repo_root: &std::path::Path) -> Result<Stored, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  let connections = db.connections()?;
  Ok(Stored {
    settings: db.auto_mode_settings()?,
    // `.ok()`: the reviewer's connection is a Claude Code CLI connection, which always is an agent one.
    reviewer: reviewer_connection(&connections)
      .and_then(|connection| AgentConnection::try_from(connection.clone()).ok()),
  })
}
