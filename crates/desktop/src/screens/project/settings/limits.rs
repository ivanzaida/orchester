//! Project settings / Limits (02.M `c7e729`): the maximum run budget, the default run budget, the
//! daily project budget and the default time limit of active agent time. A budget saves when its
//! field is left or Enter is pressed, and a time limit when it is chosen, once every field is
//! valid; the topbar says whether all changes are saved. Changes apply to new runs.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, ScrollVertical, Select, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  limits::ProjectLimits,
  tokens::{compact_tokens, grouped},
};

use super::{
  super::ProjectNav,
  SettingsSave,
  general::{CONTENT_WIDTH, readback, sized_setting_row},
};
use crate::{
  background,
  budget::{LimitsErrors, LimitsForm, minutes_text, read_budget, time_limit_choices, unbounded},
  messages::{self, Message},
  tasks::usage::{ProjectUsage, save_limits_in},
  theme::ExtraTypography,
  ui::{
    field::{field_error, select_style},
    form::form,
    icon::Icon,
    message_text,
    notice::Notice,
    unit_input::{FocusedInput, UnitInput},
  },
  workspace::OpenProject,
};

/// Width of a setting's input column (`MusI9`) and the height of its input (`la8Zc`).
const VALUE_WIDTH: f32 = 240.0;
const INPUT_HEIGHT: f32 = 36.0;

#[derive(Clone)]
pub struct LimitsProps {
  pub open: OpenProject,
  /// Written by the tab, read by the settings topbar.
  pub status: Signal<SettingsSave>,
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

/// Cloned by the save it commits, which reads the fields when it runs.
#[derive(Clone)]
pub struct LimitsTab {
  max: Signal<String>,
  default: Signal<String>,
  daily: Signal<String>,
  time: Signal<String>,
  /// Which input has the focus, for its focused stroke.
  focused: FocusedInput,
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
      focused: ctx.signal(None),
      stored: ctx.signal(None),
      nav,
    };
    let save = save_action(ctx);
    // A chosen time limit saves at once, as a text field does when it is left.
    let commit = tab.commit(
      save.clone(),
      &ctx.props::<LimitsProps>().open,
      ctx.t("number.groupSeparator"),
    );
    ctx.watch(&tab.time, move |_| commit());
    let status = ctx.props::<LimitsProps>().status.clone();
    let (stored, nav) = (tab.stored.clone(), tab.nav.clone());
    ctx.watch(&save.state(), move |state| {
      if state.is_pending() {
        status.set(SettingsSave::Saving);
        return;
      }
      match (&state.data, &state.error) {
        (_, Some(error)) => status.set(SettingsSave::Failed(error.clone())),
        (Some(saved), None) => {
          stored.set(Some(*saved));
          status.set(SettingsSave::Saved);
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
    let errors = self
      .entered(&separator, Signal::get)
      .err()
      .map(|errors| *errors)
      .unwrap_or_default();
    let commit = self.commit(save, &props.open, separator.clone());
    let field = |id, value: &Signal<String>, unit: Arc<str>, error: Option<&Message>| {
      UnitInput::new(id, value.clone(), unit, self.focused.clone())
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
    let time = time_select(ctx, &self.time);
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
  /// The limits as entered, each field read with `read`.
  fn entered(
    &self,
    separator: &str,
    read: impl Fn(&Signal<String>) -> String,
  ) -> Result<ProjectLimits, Box<LimitsErrors>> {
    LimitsForm {
      max_run_budget: &read(&self.max),
      default_run_budget: &read(&self.default),
      daily_budget: &read(&self.daily),
      default_time_limit: &read(&self.time),
      separator,
    }
    .limits()
  }

  /// Saves the limits as entered, when every field is valid and they differ from what is stored.
  fn commit(&self, save: SaveAction, open: &OpenProject, separator: Arc<str>) -> Arc<dyn Fn() + Send + Sync> {
    let fields = self.clone();
    let repo_root = open.repo_root.clone();
    Arc::new(move || {
      let Ok(limits) = fields.entered(&separator, Signal::get_untracked) else {
        return;
      };
      let changed = fields
        .stored
        .get_untracked()
        .is_some_and(|Stored(stored)| stored != limits);
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

/// Default time limit (`F8BPR`): whole minutes of active agent time, chosen in a select
/// (`bPPG9`) that saves at once.
fn time_row(ctx: &Ctx, select: Element, error: Option<&Message>) -> Element {
  let note = match error {
    Some(error) => field_error(&message_text(ctx, error)),
    None => readback(&ctx.t("settings.limits.activeTime")),
  };
  sized_setting_row(
    VALUE_WIDTH,
    &ctx.t("settings.limits.timeLimit"),
    &ctx.t("settings.limits.timeLimitDetail"),
    vec![select, note],
  )
}

/// The time limit select (`bPPG9`): "30 minutes". Its value is the limit's whole minutes as
/// the Limits form reads them.
fn time_select(ctx: &Ctx, time: &Signal<String>) -> Element {
  let current = time.get().trim().parse::<u64>().ok();
  let options = time_limit_choices(current).into_iter().map(|minutes| {
    let count = minutes.to_string();
    let key = if minutes == 1 {
      "settings.limits.minutesOption.one"
    } else {
      "settings.limits.minutesOption.other"
    };
    let label = ctx.t_args(key, [("count", count.as_str())]);
    (count, label)
  });
  Element::from(
    Select::new(time.clone())
      .options(options)
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(INPUT_HEIGHT))
      .style(select_style(ctx, INPUT_HEIGHT)),
  )
  .id("limits-default-time-limit")
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
