//! Project settings / Limits (02.M `c7e729`): the maximum run budget, the default run budget, the
//! daily project budget and the default time limit of active agent time. A setting saves when its
//! field is left or Enter is pressed, once every field is valid; the topbar says whether all
//! changes are saved. Changes apply to new runs.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, ScrollVertical, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  limits::ProjectLimits,
  tokens::{compact_tokens, grouped},
};

use super::{
  super::ProjectNav,
  general::{CONTENT_WIDTH, readback, sized_setting_row},
};
use crate::{
  background,
  budget::{LimitsErrors, LimitsForm, minutes_text, read_budget, unbounded},
  messages::{self, Message},
  tasks::usage::{ProjectUsage, save_limits_in},
  theme::ExtraTypography,
  ui::{field::field_error, form::form, icon::Icon, message_text, notice::Notice, unit_input::UnitInput},
  workspace::OpenProject,
};

/// Width of a setting's input column (`MusI9`) and the height of its input (`la8Zc`).
const VALUE_WIDTH: f32 = 240.0;
const INPUT_HEIGHT: f32 = 36.0;

/// Whether the Limits tab's changes are stored, as its topbar says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LimitsSave {
  Saved,
  Saving,
  Failed(Message),
}

impl lurq::app::component::DevtoolsInspectable for LimitsSave {}

#[derive(Clone)]
pub struct LimitsProps {
  pub open: OpenProject,
  /// Written by the tab, read by the settings topbar.
  pub status: Signal<LimitsSave>,
}

impl PartialEq for LimitsProps {
  fn eq(&self, other: &Self) -> bool {
    self.open == other.open
  }
}

impl lurq::app::component::DevtoolsInspectable for LimitsProps {}

/// The limits as stored in the project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stored(ProjectLimits);

impl lurq::app::component::DevtoolsInspectable for Stored {}

type SaveAction = FutureAction<(std::path::PathBuf, ProjectLimits), Stored, Message>;

pub struct LimitsTab {
  max: Signal<String>,
  default: Signal<String>,
  daily: Signal<String>,
  time: Signal<String>,
  /// The limits as stored; `None` until read.
  stored: Signal<Option<Stored>>,
  nav: ProjectNav,
}

impl Component for LimitsTab {
  type Props = LimitsProps;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let tab = Self {
      max: ctx.signal(String::new()),
      default: ctx.signal(String::new()),
      daily: ctx.signal(String::new()),
      time: ctx.signal(String::new()),
      stored: ctx.signal(None),
      nav,
    };
    let save = save_action(ctx);
    let status = ctx.props::<LimitsProps>().status.clone();
    let (stored, nav) = (tab.stored.clone(), tab.nav.clone());
    ctx.watch(&save.state(), move |state| {
      if state.is_pending() {
        status.set(LimitsSave::Saving);
        return;
      }
      match (&state.data, &state.error) {
        (_, Some(error)) => status.set(LimitsSave::Failed(error.clone())),
        (Some(saved), None) => {
          stored.set(Some(*saved));
          status.set(LimitsSave::Saved);
          // New task and the status bar read the limits again.
          nav.tasks_changed();
        }
        (None, None) => {}
      }
    });
    tab
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LimitsProps>().clone();
    let save = save_action(ctx);
    let separator = ctx.t("number.groupSeparator");
    self.load(ctx, &props.open, &separator);
    let entered = LimitsForm {
      max_run_budget: &self.max.get(),
      default_run_budget: &self.default.get(),
      daily_budget: &self.daily.get(),
      default_time_limit: &self.time.get(),
      separator: &separator,
    }
    .limits();
    let errors = entered
      .as_ref()
      .err()
      .map(|errors| (**errors).clone())
      .unwrap_or_default();
    let commit = self.commit(save, &props.open, entered.ok());
    let field = |id, value: &Signal<String>, unit: Arc<str>, error: Option<&Message>| {
      UnitInput::new(id, value.clone(), unit)
        .invalid(error.is_some())
        .on_blur(commit.clone())
        .size(Dimension::Pct(100.0), Dimension::Px(INPUT_HEIGHT))
    };
    let rows: Vec<Element> = self
      .budget_settings(&errors)
      .into_iter()
      .map(|setting| {
        let input = field(setting.id, setting.value, ctx.t("tokens.unit"), setting.error).tokens(separator.clone());
        setting.render(ctx, input, &separator)
      })
      .collect();
    let time = field(
      "limits-default-time-limit",
      &self.time,
      ctx.t("runs.limit.minutes"),
      errors.default_time_limit.as_ref(),
    );
    let content = Column::new()
      .id("settings-limits")
      .width(Dimension::Px(CONTENT_WIDTH))
      .padding_vertical(SpacingSize::Lg)
      .padding_horizontal(SpacingSize::Section)
      .spacing(SpacingSize::Sm)
      .child(Text::new(&ctx.t("settings.limits.tokenBudgets")).variant(ExtraTypography::SectionHeading.role()))
      .with_children(rows)
      .child(Text::new(&ctx.t("settings.limits.time")).variant(ExtraTypography::SectionHeading.role()))
      .child(time_row(ctx, time, errors.default_time_limit.as_ref()))
      .child(
        Notice::new(
          Icon::History,
          ctx.t("settings.limits.changesTitle"),
          ctx.t("settings.limits.changesDetail"),
        )
        .id("settings-limits-notice"),
      );
    form(
      Some(commit),
      ScrollVertical::new(content).width(Dimension::Pct(100.0)).flex(1.0),
    )
  }
}

impl LimitsTab {
  /// Saves `entered`, when every field is valid and it differs from what is stored.
  fn commit(
    &self,
    save: SaveAction,
    open: &OpenProject,
    entered: Option<ProjectLimits>,
  ) -> Arc<dyn Fn() + Send + Sync> {
    let (stored, repo_root) = (self.stored.clone(), open.repo_root.clone());
    Arc::new(move || {
      let Some(limits) = entered else {
        return;
      };
      let changed = stored.get_untracked().is_some_and(|Stored(stored)| stored != limits);
      if changed && !save.state().get_untracked().is_pending() {
        save.run((repo_root.clone(), limits));
      }
    })
  }

  fn budget_settings<'a>(&'a self, errors: &'a LimitsErrors) -> [BudgetSetting<'a>; 3] {
    let defaults = ProjectLimits::DEFAULT;
    [
      BudgetSetting {
        id: "limits-max-run-budget",
        keys: ("settings.limits.max", "settings.limits.maxDetail"),
        value: &self.max,
        default: Some(defaults.max_run_budget().get()),
        error: errors.max_run_budget.as_ref(),
      },
      BudgetSetting {
        id: "limits-default-run-budget",
        keys: ("settings.limits.default", "settings.limits.defaultDetail"),
        value: &self.default,
        default: None,
        error: errors.default_run_budget.as_ref(),
      },
      BudgetSetting {
        id: "limits-daily-budget",
        keys: ("settings.limits.daily", "settings.limits.dailyDetail"),
        value: &self.daily,
        default: None,
        error: errors.daily_budget.as_ref(),
      },
    ]
  }

  /// Reads the stored limits and fills the fields with them once.
  fn load(&self, ctx: &mut Ctx, open: &OpenProject, separator: &str) {
    let load = ctx.future(open.repo_root.clone(), |repo_root| async move {
      background::run(move || ProjectUsage::load(&repo_root, orchester_core::time::Timestamp::now()))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's limits");
          messages::store_error(&error)
        })
    });
    let Some(usage) = load.state().get().data else {
      return;
    };
    if self.stored.get_untracked().is_some() {
      return;
    }
    let limits = usage.limits;
    self.stored.set(Some(Stored(limits)));
    self.max.set(grouped(limits.max_run_budget().get(), separator));
    self.default.set(grouped(limits.default_run_budget().get(), separator));
    self.daily.set(grouped(limits.daily_budget().get(), separator));
    self.time.set(minutes_text(limits.default_time_limit()));
  }
}

/// One token budget setting (`Ml60C`, `y7xpzT`, `XpyYy`).
struct BudgetSetting<'a> {
  id: &'static str,
  /// Name and description.
  keys: (&'static str, &'static str),
  value: &'a Signal<String>,
  /// The design's default, which the read-back names (for the maximum, as in `SkKCa`).
  default: Option<u64>,
  error: Option<&'a Message>,
}

impl BudgetSetting<'_> {
  /// The setting with its input and its read-back or error.
  fn render(self, ctx: &Ctx, input: UnitInput, separator: &str) -> Element {
    let note = match self.error {
      Some(error) => field_error(&message_text(ctx, error)),
      None => {
        let budget = read_budget(&self.value.get(), separator, unbounded())
          .ok()
          .map(|budget| budget.get());
        let count = budget.map(compact_tokens).unwrap_or_default();
        let key = if budget.is_some() && budget == self.default {
          "tokens.readBackDefault"
        } else {
          "tokens.readBack"
        };
        readback(&ctx.t_args(key, [("count", count.as_str())]))
      }
    };
    let (name, description) = self.keys;
    sized_setting_row(
      VALUE_WIDTH,
      &ctx.t(name),
      &ctx.t(description),
      vec![input.render(ctx), note],
    )
  }
}

/// Default time limit (`F8BPR`): whole minutes of active agent time.
fn time_row(ctx: &Ctx, input: UnitInput, error: Option<&Message>) -> Element {
  let note = match error {
    Some(error) => field_error(&message_text(ctx, error)),
    None => readback(&ctx.t("settings.limits.activeTime")),
  };
  sized_setting_row(
    VALUE_WIDTH,
    &ctx.t("settings.limits.timeLimit"),
    &ctx.t("settings.limits.timeLimitDetail"),
    vec![input.render(ctx), note],
  )
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  ctx.future_action(|(repo_root, limits): (std::path::PathBuf, ProjectLimits)| async move {
    background::run(move || save_limits_in(&repo_root, &limits))
      .await?
      .map(|()| Stored(limits))
      .map_err(|error| {
        tracing::error!(%error, "cannot save the project's limits");
        messages::store_error(&error)
      })
  })
}
