//! Project settings / Limits (02.M `c7e729`): the default run budget, the daily project budget, the
//! default time limit of active agent time and how many agents work at once. The default run budget
//! and time limit are optional with no maximum: empty, or *No limit*, leaves new tasks without one
//! (user decision 2026-10-09). A budget or the agent count saves when its field is left or Enter is
//! pressed, and a time limit when it is chosen, once every field is valid; the topbar says whether
//! all changes are saved. Changes apply to new runs, except the agent count, which the runner
//! applies at once.

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
  limits::{AgentSlots, ProjectLimits},
  tokens::{compact_tokens, grouped},
};

use super::{
  super::ProjectNav,
  SettingsSave,
  general::{CONTENT_WIDTH, readback, sized_setting_row},
  outside::{OutsideSettings, Reading},
};
use crate::{
  background,
  budget::{LimitsErrors, LimitsForm, minutes_text, read_optional_budget, time_limit_choices},
  messages::{self, Message},
  runner::Runners,
  tasks::{
    outside::Outside,
    usage::{ProjectUsage, update_limits},
  },
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

/// The limits and usage as read at a reading of [`OutsideSettings`].
#[derive(Clone, Debug, PartialEq)]
struct Read(ProjectUsage, Reading);

impl lurq::app::component::DevtoolsInspectable for Read {}

/// What a save was asked to store, and what it stored: the person's change put on the limits as
/// stored then, so a limit automation changed meanwhile keeps its change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Done {
  asked: ProjectLimits,
  stored: ProjectLimits,
}

impl lurq::app::component::DevtoolsInspectable for Done {}

/// Where to save, the limits the change was made on, and the limits it makes.
type SaveAction = FutureAction<(std::path::PathBuf, ProjectLimits, ProjectLimits), Done, Message>;

/// Cloned by the save it commits, which reads the fields when it runs.
#[derive(Clone)]
pub struct LimitsTab {
  default: Signal<String>,
  daily: Signal<String>,
  time: Signal<String>,
  slots: Signal<String>,
  /// Which input has the focus, for its focused stroke.
  focused: FocusedInput,
  /// The limits as stored; `None` until read.
  stored: Signal<Option<Stored>>,
  nav: ProjectNav,
  /// Takes a changed agent count to the project's runner.
  runners: Runners,
  outside: OutsideSettings,
}

impl Component for LimitsTab {
  type Props = LimitsProps;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let runners = ctx.use_context::<Runners>().expect("the application provides Runners");
    let tab = Self {
      default: ctx.signal(String::new()),
      daily: ctx.signal(String::new()),
      time: ctx.signal(String::new()),
      slots: ctx.signal(String::new()),
      focused: ctx.signal(None),
      stored: ctx.signal(None),
      nav,
      runners,
      outside: OutsideSettings::new(ctx, Outside::Limits),
    };
    let save = save_action(ctx, &tab.runners);
    // A chosen time limit saves at once, as a text field does when it is left.
    let commit = tab.commit(
      save.clone(),
      &ctx.props::<LimitsProps>().open,
      ctx.t("number.groupSeparator"),
    );
    ctx.watch(&tab.time, move |_| commit());
    let status = ctx.props::<LimitsProps>().status.clone();
    let (after, separator) = (tab.clone(), ctx.t("number.groupSeparator"));
    ctx.watch(&save.state(), move |state| {
      if state.is_pending() {
        status.set(SettingsSave::Saving);
        return;
      }
      match (&state.data, &state.error) {
        (_, Some(error)) => status.set(SettingsSave::Failed(error.clone())),
        (Some(done), None) => {
          after.saved(*done, &separator);
          status.set(SettingsSave::Saved);
        }
        (None, None) => {}
      }
    });
    tab
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LimitsProps>().clone();
    let save = save_action(ctx, &self.runners);
    let separator = ctx.t("number.groupSeparator");
    let saving = save.state().get().is_pending();
    self.load(ctx, (&props.open, &props.status), &separator, saving);
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
    let slots = field(
      "limits-agent-slots",
      &self.slots,
      ctx.t("settings.limits.agentsUnit"),
      errors.agent_slots.as_ref(),
    );
    let content = Column::new()
      .id("settings-limits")
      .width(Dimension::Px(CONTENT_WIDTH))
      .padding_vertical(SpacingSize::Lg)
      .padding_horizontal(SpacingSize::Section)
      .spacing(SpacingSize::Sm)
      // The section's rhythm (24, `c7e729`) below the notice, the column's 8 included.
      .with_children(
        self
          .outside
          .notice(ctx, "settings-limits-changed-outside")
          .map(|notice| Column::new().width(Dimension::Pct(100.0)).padding_bottom(SpacingSize::Md).child(notice)),
      )
      .child(Text::new(&ctx.t("settings.limits.tokenBudgets")).variant(ExtraTypography::SectionHeading.role()))
      .with_children(rows)
      .child(Text::new(&ctx.t("settings.limits.time")).variant(ExtraTypography::SectionHeading.role()))
      .child(time_row(ctx, time, errors.default_time_limit.as_ref()))
      .child(Text::new(&ctx.t("settings.limits.agents")).variant(ExtraTypography::SectionHeading.role()))
      .child(slots_row(ctx, slots.render(ctx), errors.agent_slots.as_ref()))
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
      default_run_budget: &read(&self.default),
      daily_budget: &read(&self.daily),
      default_time_limit: &read(&self.time),
      agent_slots: &read(&self.slots),
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
      let Some(Stored(base)) = fields.stored.get_untracked() else {
        return;
      };
      if base != limits && !save.state().get_untracked().is_pending() {
        save.run((repo_root.clone(), base, limits));
      }
    })
  }

  fn budget_settings<'a>(&'a self, errors: &'a LimitsErrors) -> [BudgetSetting<'a>; 2] {
    [
      BudgetSetting {
        id: "limits-default-run-budget",
        keys: ("settings.limits.default", "settings.limits.defaultDetail"),
        value: &self.default,
        error: errors.default_run_budget.as_ref(),
      },
      BudgetSetting {
        id: "limits-daily-budget",
        keys: ("settings.limits.daily", "settings.limits.dailyDetail"),
        value: &self.daily,
        error: errors.daily_budget.as_ref(),
      },
    ]
  }

  /// Reads the stored limits and fills the fields with them; again after automation changed them,
  /// unless the person has a change under way: one being saved (`saving`), or fields that differ from
  /// what is stored (see [`OutsideSettings`]).
  fn load(&self, ctx: &mut Ctx, (open, status): (&OpenProject, &Signal<SettingsSave>), separator: &str, saving: bool) {
    let current = self.outside.reading(ctx);
    let load = ctx.future((open.repo_root.clone(), current), |(repo_root, reading)| async move {
      background::run(move || ProjectUsage::load(&repo_root, orchester_core::time::Timestamp::now()))
        .await?
        .map(|usage| Read(usage, reading))
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's limits");
          messages::store_error(&error)
        })
    });
    let state = load.state().get();
    let pending = state.is_pending();
    let Some(Read(usage, reading)) = state.data.filter(|_| !pending) else {
      return;
    };
    let edited = self
      .stored
      .get_untracked()
      .is_some_and(|Stored(stored)| self.entered(separator, Signal::get_untracked).ok() != Some(stored));
    if self.outside.take(reading, current, saving || edited) {
      self.fill(usage.limits, separator);
      // A save refused earlier is settled: the fields show what is stored, which it left as it was.
      if matches!(status.get_untracked(), SettingsSave::Failed(_)) {
        status.set(SettingsSave::Saved);
      }
    }
  }

  /// After a save: the fields show what it stored when they still hold what it was asked to store,
  /// and the limits are read again.
  fn saved(&self, done: Done, separator: &str) {
    self.stored.set(Some(Stored(done.stored)));
    if done.stored != done.asked && self.entered(separator, Signal::get_untracked).ok() == Some(done.asked) {
      self.fill(done.stored, separator);
    }
    self.outside.saved();
    // New task and the status bar read the limits again.
    self.nav.tasks_changed();
  }

  /// Shows `limits`, as stored, in the fields.
  fn fill(&self, limits: ProjectLimits, separator: &str) {
    self.stored.set(Some(Stored(limits)));
    self.default.set(
      limits
        .default_run_budget()
        .map(|budget| grouped(budget.get(), separator))
        .unwrap_or_default(),
    );
    self.daily.set(grouped(limits.daily_budget().get(), separator));
    self.slots.set(limits.agent_slots().get().to_string());
    // Last: setting the time limit saves what the fields hold, as a person's choice does. With every
    // other field filled first they hold what is stored, so nothing is saved; filled earlier, the old
    // agent count would be saved over the new one.
    self.time.set(minutes_text(limits.default_time_limit()));
  }
}

/// One token budget setting (`y7xpzT`, `XpyYy`).
struct BudgetSetting<'a> {
  id: &'static str,
  /// Name and description.
  keys: (&'static str, &'static str),
  value: &'a Signal<String>,
  error: Option<&'a Message>,
}

impl BudgetSetting<'_> {
  /// The setting with its input and its read-back or error: an empty field reads back as no limit.
  fn render(self, ctx: &Ctx, input: UnitInput, separator: &str) -> Element {
    let note = match self.error {
      Some(error) => field_error(&message_text(ctx, error)),
      None => match read_optional_budget(&self.value.get(), separator).ok().flatten() {
        Some(budget) => readback(&ctx.t_args("tokens.readBack", [("count", compact_tokens(budget.get()).as_str())])),
        None => readback(&ctx.t("tokens.noLimit")),
      },
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

/// Agents working at once (not drawn; laid out as the budgets are): a whole number of agents with
/// what it means, its range and default read back under it.
fn slots_row(ctx: &Ctx, input: Element, error: Option<&Message>) -> Element {
  let note = match error {
    Some(error) => field_error(&message_text(ctx, error)),
    None => readback(&ctx.t_args(
      "settings.limits.agentSlotsRange",
      [
        ("min", AgentSlots::MIN.to_string()),
        ("max", AgentSlots::MAX.to_string()),
        ("default", AgentSlots::DEFAULT.get().to_string()),
      ],
    )),
  };
  sized_setting_row(
    VALUE_WIDTH,
    &ctx.t("settings.limits.agentSlots"),
    &ctx.t("settings.limits.agentSlotsDetail"),
    vec![input, note],
  )
}

/// The time limit select (`bPPG9`): "No limit", then "30 minutes" and the other choices. Its value is
/// the limit's whole minutes as the Limits form reads them, empty for no limit.
fn time_select(ctx: &Ctx, time: &Signal<String>) -> Element {
  let current = time.get().trim().parse::<u64>().ok();
  let minutes = time_limit_choices(current).into_iter().map(|minutes| {
    let count = minutes.to_string();
    let key = if minutes == 1 {
      "settings.limits.minutesOption.one"
    } else {
      "settings.limits.minutesOption.other"
    };
    let label = ctx.t_args(key, [("count", count.as_str())]);
    (count, label)
  });
  let options = std::iter::once((String::new(), ctx.t("tokens.noLimit"))).chain(minutes);
  Element::from(
    Select::new(time.clone())
      .options(options)
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(INPUT_HEIGHT))
      .style(select_style(ctx, INPUT_HEIGHT)),
  )
  .id("limits-default-time-limit")
}

fn save_action(ctx: &mut Ctx, runners: &Runners) -> SaveAction {
  let runners = runners.clone();
  ctx.future_action(
    move |(repo_root, base, asked): (std::path::PathBuf, ProjectLimits, ProjectLimits)| {
      let runners = runners.clone();
      async move {
        let saved = background::run(move || update_limits(&runners, &repo_root, |stored| rebased(base, asked, stored)))
          .await?
          .map_err(|error| {
            tracing::error!(%error, "cannot save the project's limits");
            messages::store_error(&error)
          })?;
        // The person's change and one made elsewhere meanwhile do not fit together: nothing is saved.
        saved
          .map(|stored| Done { asked, stored })
          .ok_or_else(|| Message::new("settings.limits.changedOutsideConflict"))
      }
    },
  )
}

/// The person's change from `base` to `asked`, put on the limits as stored now (`stored`): a limit they
/// did not change keeps what automation may have changed meanwhile. `None` when the result breaks the
/// rules of the limits.
fn rebased(base: ProjectLimits, asked: ProjectLimits, stored: ProjectLimits) -> Option<ProjectLimits> {
  fn pick<T: PartialEq + Copy>(base: T, asked: T, stored: T) -> T {
    if asked == base { stored } else { asked }
  }
  let limits = ProjectLimits::new(
    pick(
      base.default_run_budget(),
      asked.default_run_budget(),
      stored.default_run_budget(),
    ),
    pick(base.daily_budget(), asked.daily_budget(), stored.daily_budget()),
    pick(
      base.default_time_limit(),
      asked.default_time_limit(),
      stored.default_time_limit(),
    ),
  )
  .ok()?;
  Some(limits.with_agent_slots(pick(base.agent_slots(), asked.agent_slots(), stored.agent_slots())))
}
