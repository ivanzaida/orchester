//! Project settings / Environment (02.O `tXahd`, laid out like the Limits tab, 02.M `c7e729`): the
//! variables the project adds to the environment of every agent CLI and command tool its runs
//! start, on top of the cleaned environment every program gets, as `NAME=value` lines like a
//! command tool's (06.02 `YEWFo`). They save when the field is left, once every line is valid; the
//! topbar says whether all changes are saved. The variables are passed to every run unchanged,
//! whatever they name.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, ScrollVertical, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::resource::CommandEnvironment;

use super::{SettingsSave, general::CONTENT_WIDTH};
use crate::{
  activity::{Activity, operations},
  background,
  environment::{load_in, save_in},
  messages::{self, Message},
  theme::{ExtraSpacing, ExtraTypography},
  tools::form::{environment_text, parse_environment},
  ui::{
    field::{FieldNote, field, styled_input},
    form::form,
    icon::Icon,
    message_text,
    notice::Notice,
  },
  workspace::OpenProject,
};

/// Lines the variables field shows before it scrolls, as a command tool's (`YEWFo`).
const AREA_ROWS: (usize, usize) = (4, 12);
const VARIABLES_AREA_HEIGHT: f32 = 116.0;

#[derive(Clone)]
pub struct EnvironmentProps {
  pub open: OpenProject,
  /// Written by the tab, read by the settings topbar.
  pub status: Signal<SettingsSave>,
}

impl PartialEq for EnvironmentProps {
  fn eq(&self, other: &Self) -> bool {
    self.open == other.open
  }
}

impl lurq::app::component::DevtoolsInspectable for EnvironmentProps {}

/// The variables as stored, once read or after a save.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stored(CommandEnvironment);

impl lurq::app::component::DevtoolsInspectable for Stored {}

type SaveAction = FutureAction<(PathBuf, CommandEnvironment), Stored, Message>;

/// Cloned by the save it commits, which reads the field when it runs.
#[derive(Clone)]
pub struct EnvironmentTab {
  text: Signal<String>,
  /// `None` until read.
  stored: Signal<Option<Stored>>,
}

impl Component for EnvironmentTab {
  type Props = EnvironmentProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tab = Self {
      text: ctx.signal(String::new()),
      stored: ctx.signal(None),
    };
    let save = save_action(ctx);
    let props = ctx.props::<EnvironmentProps>().clone();
    let status = props.status.clone();
    let stored = tab.stored.clone();
    // A change made while an earlier one was saving is saved once that save is done.
    let commit = tab.commit(save.clone(), &props.open);
    ctx.watch(&save.state(), move |state| {
      if state.is_pending() {
        status.set(SettingsSave::Saving);
        return;
      }
      match (&state.data, &state.error) {
        (_, Some(error)) => status.set(SettingsSave::Failed(error.clone())),
        (Some(saved), None) => {
          stored.set(Some(saved.clone()));
          status.set(SettingsSave::Saved);
          commit();
        }
        (None, None) => {}
      }
    });
    tab
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<EnvironmentProps>().clone();
    let save = save_action(ctx);
    self.load(ctx, &props.open);
    let commit = self.commit(save, &props.open);
    let error = parse_environment(&self.text.get()).err();
    let content = Column::new()
      .id("settings-environment")
      .width(Dimension::Px(CONTENT_WIDTH))
      .padding_vertical(SpacingSize::Lg)
      .padding_horizontal(SpacingSize::Section)
      .spacing(SpacingSize::Md)
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(Text::new(&ctx.t("settings.environment.heading")).variant(ExtraTypography::SectionHeading.role()))
          .child(Text::new(&ctx.t("settings.environment.detail")).variant(ExtraTypography::Explanation.role())),
      )
      .child(self.variables(ctx, error.as_ref(), &commit))
      .child(
        Notice::new(
          Icon::History,
          ctx.t("settings.environment.changesTitle"),
          ctx.t("settings.environment.changesDetail"),
        )
        .id("settings-environment-notice"),
      );
    form(
      None,
      ScrollVertical::new(content).width(Dimension::Pct(100.0)).flex(1.0),
    )
  }
}

impl EnvironmentTab {
  /// The variables as `NAME=value` lines in a mono area drawn like a field, and why a line cannot
  /// be used.
  fn variables(&self, ctx: &Ctx, error: Option<&Message>, commit: &Arc<dyn Fn() + Send + Sync>) -> Element {
    let blur = commit.clone();
    let input = styled_input(ctx, self.text.clone(), ExtraTypography::MonoArea.role())
      .id("settings-environment-variables")
      .textarea()
      .rows(AREA_ROWS.0, AREA_ROWS.1)
      .width(Dimension::Pct(100.0))
      .on_blur(move || blur());
    let area = Column::new()
      .width(Dimension::Pct(100.0))
      .min_height(Dimension::Px(VARIABLES_AREA_HEIGHT))
      .padding(ExtraSpacing::Stack.role())
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(
        BorderSize::Sm,
        if error.is_some() {
          PaletteColor::BorderStrong
        } else {
          PaletteColor::Border
        },
      ))
      .rounded(RadiusSize::Md)
      .child(input);
    let note = error.map_or(FieldNote::None, |error| FieldNote::Error(message_text(ctx, error)));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(field(&ctx.t("settings.environment.variables"), area.into(), note))
      .child(Text::new(&ctx.t("settings.environment.hint")).variant(ExtraTypography::Hint.role()))
      .into()
  }

  /// Saves the variables as entered, when every line is valid and they differ from what is stored.
  fn commit(&self, save: SaveAction, open: &OpenProject) -> Arc<dyn Fn() + Send + Sync> {
    let tab = self.clone();
    let repo_root = open.repo_root.clone();
    Arc::new(move || {
      let Ok(variables) = parse_environment(&tab.text.get_untracked()) else {
        return;
      };
      let changed = tab
        .stored
        .get_untracked()
        .is_some_and(|Stored(stored)| stored != variables);
      if changed && !save.state().get_untracked().is_pending() {
        save.run((repo_root.clone(), variables));
      }
    })
  }

  /// Reads the stored variables and fills the field with them once.
  fn load(&self, ctx: &mut Ctx, open: &OpenProject) {
    let load = ctx.future(open.repo_root.clone(), |repo_root| async move {
      background::run(move || load_in(&repo_root).map(Stored))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's environment");
          messages::store_error(&error)
        })
    });
    let Some(environment) = load.state().get().data else {
      return;
    };
    if self.stored.get_untracked().is_some() {
      return;
    }
    self.text.set(environment_text(&environment.0));
    self.stored.set(Some(environment));
  }
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  let activity = Activity::of(ctx);
  let open = ctx.props::<EnvironmentProps>().open.clone();
  ctx.future_action(move |(repo_root, variables): (PathBuf, CommandEnvironment)| {
    let (activity, spec) = (activity.clone(), operations::project_environment(&open));
    async move {
      let save = async move {
        background::run(move || save_in(&repo_root, &variables).map(|()| Stored(variables)))
          .await?
          .map_err(|error| {
            tracing::error!(%error, "cannot save the project's environment");
            messages::store_error(&error)
          })
      };
      activity.track(spec, save).await
    }
  })
}
