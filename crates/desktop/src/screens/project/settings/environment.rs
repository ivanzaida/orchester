//! Project settings / Environment (not drawn yet; it follows the Limits tab, 02.M `c7e729`): the
//! variables the project adds to the environment of every agent CLI and command tool its runs
//! start, on top of the cleaned environment every program gets, as `NAME=value` lines like a
//! command tool's (06.02 `YEWFo`). They save when the field is left, once every line is valid; the
//! topbar says whether all changes are saved. A Rust repository without `CARGO_TARGET_DIR` gets a
//! suggestion to share one build folder between the runs' worktrees; nothing is set until the
//! person adds it.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
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
  environment::{CARGO_TARGET_DIR, ProjectEnvironment, save_in},
  messages::{self, Message},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  tools::form::{environment_text, parse_environment},
  ui::{
    button::ActionButton,
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

/// The variables as stored after a save.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Saved(CommandEnvironment);

impl lurq::app::component::DevtoolsInspectable for Saved {}

type SaveAction = FutureAction<(PathBuf, CommandEnvironment), Saved, Message>;

/// Cloned by the save it commits, which reads the field when it runs.
#[derive(Clone)]
pub struct EnvironmentTab {
  text: Signal<String>,
  /// The variables as stored and the suggestion for the repository; `None` until read.
  stored: Signal<Option<ProjectEnvironment>>,
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
        (Some(Saved(saved)), None) => {
          stored.update(|stored| {
            if let Some(stored) = stored {
              stored.variables = saved.clone();
              stored.suggested_target = stored
                .suggested_target
                .take()
                .filter(|_| saved.get(CARGO_TARGET_DIR).is_none());
            }
          });
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
      .child(Text::new(&ctx.t("settings.environment.heading")).variant(ExtraTypography::SectionHeading.role()))
      .child(Text::new(&ctx.t("settings.environment.detail")).variant(ExtraTypography::Explanation.role()))
      .child(self.variables(ctx, error.as_ref(), &commit))
      .with_children(self.suggestion(ctx, &commit))
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
    let input = styled_input(ctx, self.text.clone(), ExtraTypography::MonoField.role())
      .id("settings-environment-variables")
      .textarea()
      .rows(AREA_ROWS.0, AREA_ROWS.1)
      .width(Dimension::Pct(100.0))
      .on_blur(move || blur());
    let area = Column::new()
      .width(Dimension::Pct(100.0))
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
      .rounded(ExtraRadius::Notice.role())
      .child(input);
    let note = error.map_or(FieldNote::None, |error| FieldNote::Error(message_text(ctx, error)));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(field(&ctx.t("settings.environment.variables"), area.into(), note))
      .child(Text::new(&ctx.t("settings.environment.hint")).variant(TypographyStyle::Caption))
      .into()
  }

  /// For a Rust repository without `CARGO_TARGET_DIR`: one build folder for every run's worktree,
  /// added with a click.
  fn suggestion(&self, ctx: &Ctx, commit: &Arc<dyn Fn() + Send + Sync>) -> Option<Element> {
    let stored = self.stored.get()?;
    let target = stored.suggested_target?;
    let target = target.to_str()?.to_owned();
    if self.text.get().lines().any(|line| {
      line
        .split_once('=')
        .is_some_and(|(name, _)| name.trim().eq_ignore_ascii_case(CARGO_TARGET_DIR))
    }) {
      return None;
    }
    let (text, commit) = (self.text.clone(), commit.clone());
    let line = format!("{CARGO_TARGET_DIR}={target}");
    let add = ActionButton::new(ctx.t("settings.environment.addTarget"))
      .icon(Icon::Plus)
      .id("settings-environment-add-target")
      .in_form()
      .on_click(move || {
        text.update(|text| {
          if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
          }
          text.push_str(&line);
        });
        commit();
      })
      .render(ctx);
    Some(
      Notice::new(
        Icon::Lightbulb,
        ctx.t("settings.environment.targetTitle"),
        ctx.t("settings.environment.targetDetail"),
      )
      .action(add)
      .id("settings-environment-target")
      .into(),
    )
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
        .is_some_and(|stored| stored.variables != variables);
      if changed && !save.state().get_untracked().is_pending() {
        save.run((repo_root.clone(), variables));
      }
    })
  }

  /// Reads the stored variables and fills the field with them once.
  fn load(&self, ctx: &mut Ctx, open: &OpenProject) {
    let load = ctx.future(open.repo_root.clone(), |repo_root| async move {
      background::run(move || ProjectEnvironment::load_in(&repo_root))
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
    self.text.set(environment_text(&environment.variables));
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
        background::run(move || save_in(&repo_root, &variables).map(|()| Saved(variables)))
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
