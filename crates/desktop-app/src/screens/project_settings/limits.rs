//! Limits (jupiT, iZS4K): save on leave, per-field validation and optional-limit readbacks.
use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::tokens::compact_tokens;
use orchester_workspace::{
  budget::LimitsErrors, messages::Message, project_settings::LimitsText, tasks::outside::Outside,
};

use super::{
  Status, TabProps, page,
  work::{self, Change, Read, Save},
};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  field::{NumberField, field_error},
  form_section::{Place, section},
  message::message_text,
  notice::{Notice, NoticeTone},
};

#[derive(Clone)]
pub(super) struct Limits {
  default: Signal<String>,
  daily: Signal<String>,
  minutes: Signal<String>,
  slots: Signal<String>,
  stored: Signal<Option<Read>>,
  revision: Signal<u64>,
  separator: Arc<str>,
  submitted: Signal<Submitted>,
  queued: Signal<bool>,
}
#[derive(Clone, PartialEq)]
struct Submitted(Option<LimitsText>);
impl DevtoolsInspectable for Submitted {}
#[derive(Clone, Copy)]
enum Field {
  Default,
  Daily,
  Time,
  Slots,
}
impl Field {
  const fn id(self) -> &'static str {
    match self {
      Self::Default => "settings-default-budget",
      Self::Daily => "settings-daily-budget",
      Self::Time => "settings-time-limit",
      Self::Slots => "settings-agent-slots",
    }
  }
  const fn keys(self) -> (&'static str, &'static str, &'static str) {
    match self {
      Self::Default => (
        "project_settings.defaultBudget",
        "project_settings.tokens",
        "project_settings.noRunLimit",
      ),
      Self::Daily => (
        "project_settings.dailyBudget",
        "project_settings.tokens",
        "project_settings.noDailyLimit",
      ),
      Self::Time => (
        "project_settings.timeLimit",
        "project_settings.minutes",
        "project_settings.noTimeLimit",
      ),
      Self::Slots => (
        "project_settings.slots",
        "project_settings.agentsUnit",
        "project_settings.slotsRange",
      ),
    }
  }
}
impl Component for Limits {
  type Props = TabProps;
  fn create(ctx: &mut Ctx) -> Self {
    let tab = Self {
      default: ctx.signal(String::new()),
      daily: ctx.signal(String::new()),
      minutes: ctx.signal(String::new()),
      slots: ctx.signal(String::new()),
      stored: ctx.signal(None),
      revision: ctx.signal(0),
      separator: ctx.t("project_settings.groupSeparator"),
      submitted: ctx.signal(Submitted(None)),
      queued: ctx.signal(false),
    };
    let save = work::action(ctx);
    work::follow(
      ctx,
      &save,
      ctx.props::<TabProps>().status.clone(),
      tab.revision.clone(),
      tab.stored.clone(),
    );
    let (after, next, root) = (
      tab.clone(),
      save.clone(),
      ctx.props::<TabProps>().project.open.repo_root.clone(),
    );
    ctx.watch(&save.state(), move |state| {
      if !state.is_pending()
        && state.error.is_none()
        && let Some(saved) = &state.data
      {
        after.accept(saved.0.limits);
        if after.queued.get_untracked() {
          after.submit(&root, &next);
        }
      }
    });
    tab
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let save = work::action(ctx);
    let revision = self.revision.get() + props.project.feed.outside.get().of(Outside::Limits);
    let state = work::load(ctx, props.project.open.repo_root.clone(), revision);
    let Some(data) = state.data.clone().or_else(|| self.stored.get()) else {
      return page::scroll(work::pending(ctx, &state, &self.revision));
    };
    if self.stored.get_untracked().is_none() {
      self.fill(data.clone());
    }
    let mut stored = self.stored.get().expect("fields were filled from the load");
    let changed = self.text().limits(stored.0.limits, &self.separator).as_ref() != Ok(&stored.0.limits);
    let outside = !state.is_pending() && stored.0.limits != data.0.limits;
    if outside && !changed && !save.state().get().is_pending() {
      self.fill(data.clone());
      stored = data.clone();
    }
    if !save.state().get().is_pending() && save.state().get().error.is_none() {
      props
        .status
        .set(if self.text().limits(stored.0.limits, &self.separator).is_err() {
          Status::Invalid
        } else if changed {
          Status::Unsaved
        } else {
          Status::Saved
        });
    }
    let errors = self
      .text()
      .limits(stored.0.limits, &self.separator)
      .err()
      .map(|e| *e)
      .unwrap_or_default();
    let commit = self.commit(&props, &save);
    let fields = self.sections(ctx, &errors, &commit);
    let content = Column::new()
      .spacing(crate::theme::ExtraSpacing::Roomy.role())
      .with_children(work::error(ctx, &save))
      .with_children(save.state().get().error.map(|_| {
        let commit = commit.clone();
        ActionButton::new(
          "settings-limits-retry",
          ButtonKind::Secondary,
          ctx.t("project_settings.retry"),
        )
        .on_click(move || commit())
      }))
      .with_children(
        state
          .error
          .map(|e| field_error("settings-limits-read-error", &message_text(ctx, &e))),
      )
      .with_children((outside && changed).then(|| self.outside(ctx, data)))
      .with_children(fields)
      .child(page::review_notice(ctx, &props.project, false));
    page::scroll(content)
  }
}
impl Limits {
  fn text(&self) -> LimitsText {
    LimitsText {
      default: self.default.get(),
      daily: self.daily.get(),
      minutes: self.minutes.get(),
      slots: self.slots.get(),
    }
  }
  fn fill(&self, read: Read) {
    let text = LimitsText::of(read.0.limits, &self.separator);
    self.default.set(text.default);
    self.daily.set(text.daily);
    self.minutes.set(text.minutes);
    self.slots.set(text.slots);
    self.stored.set(Some(read));
  }
  fn accept(&self, limits: orchester_core::limits::ProjectLimits) {
    let Some(sent) = self.submitted.get_untracked().0 else {
      return;
    };
    let read = LimitsText::of(limits, &self.separator);
    // Rebasing can change untouched fields. Read them back while keeping edits made after submission.
    for (signal, submitted, saved) in [
      (&self.default, &sent.default, read.default),
      (&self.daily, &sent.daily, read.daily),
      (&self.minutes, &sent.minutes, read.minutes),
      (&self.slots, &sent.slots, read.slots),
    ] {
      if signal.get_untracked() == *submitted {
        signal.set(saved);
      }
    }
    self.submitted.set(Submitted(None));
  }
  fn commit(&self, props: &TabProps, save: &Save) -> Arc<dyn Fn() + Send + Sync> {
    let (tab, save, root) = (self.clone(), save.clone(), props.project.open.repo_root.clone());
    Arc::new(move || tab.submit(&root, &save))
  }
  fn submit(&self, root: &Path, save: &Save) {
    if save.state().get_untracked().is_pending() {
      // A later blur must be committed even when it reverted to the pre-save baseline.
      self.queued.set(true);
      return;
    }
    self.queued.set(false);
    let Some(base) = self.stored.get_untracked() else {
      return;
    };
    let text = self.text();
    let Ok(asked) = text.limits(base.0.limits, &self.separator) else {
      return;
    };
    if asked != base.0.limits {
      self.submitted.set(Submitted(Some(text)));
      save.run((
        root.to_path_buf(),
        Change::Limits {
          base: base.0.limits,
          asked,
        },
      ));
    }
  }
  fn number(
    &self,
    ctx: &mut Ctx,
    kind: Field,
    error: Option<&Message>,
    commit: &Arc<dyn Fn() + Send + Sync>,
  ) -> Element {
    let value = match kind {
      Field::Default => &self.default,
      Field::Daily => &self.daily,
      Field::Time => &self.minutes,
      Field::Slots => &self.slots,
    };
    let text = value.get();
    let (label, unit, empty) = kind.keys();
    let readback = if matches!(kind, Field::Slots) || text.trim().is_empty() {
      ctx.t(empty)
    } else if matches!(kind, Field::Time) {
      ctx.t_args("project_settings.timeReadback", [("count", text)])
    } else {
      let count = orchester_workspace::budget::read_optional_budget(&text, &self.separator)
        .map(|v| v.map(|v| compact_tokens(v.get())).unwrap_or_default())
        .unwrap_or_default();
      ctx.t_args("project_settings.budgetReadback", [("count", count)])
    };
    let commit = commit.clone();
    let mut field = NumberField::new(kind.id(), ctx.t(label), value.clone(), ctx.t(unit), readback)
      .width(Dimension::Px(page::CONTROL_WIDTH))
      .read_back_alignment(Alignment::Start)
      .on_leave(move || commit());
    if !matches!(kind, Field::Slots) {
      field = field.placeholder(ctx.t("project_settings.noLimit"));
    }
    if matches!(kind, Field::Default | Field::Daily) {
      field = field.grouped(self.separator.clone());
    }
    if let Some(error) = error {
      let saved = self.saved_value(ctx, kind);
      field = field.error(ctx.t_args(
        "project_settings.invalidStillSaved",
        [("reason", message_text(ctx, error)), ("saved", saved.into())],
      ));
    }
    field.render(ctx)
  }
  fn saved_value(&self, ctx: &Ctx, kind: Field) -> String {
    let limits = self
      .stored
      .get()
      .expect("limits fields have a stored baseline")
      .0
      .limits;
    let value = match kind {
      Field::Slots => return limits.agent_slots().get().to_string(),
      Field::Time => {
        return limits.default_time_limit().map_or_else(
          || ctx.t("project_settings.noLimit").to_string(),
          |v| {
            ctx
              .t_args(
                "project_settings.savedTime",
                [("count", (v.as_secs() / 60).to_string())],
              )
              .to_string()
          },
        );
      }
      Field::Daily => limits.daily_budget_limit(),
      Field::Default => limits.default_run_budget(),
    };
    value.map_or_else(
      || ctx.t("project_settings.noLimit").to_string(),
      |v| {
        ctx
          .t_args("project_settings.savedBudget", [("count", compact_tokens(v.get()))])
          .to_string()
      },
    )
  }

  fn sections(&self, ctx: &mut Ctx, errors: &LimitsErrors, commit: &Arc<dyn Fn() + Send + Sync>) -> Vec<Element> {
    let default = self.number(ctx, Field::Default, errors.default_run_budget.as_ref(), commit);
    let daily = self.number(ctx, Field::Daily, errors.daily_budget.as_ref(), commit);
    let time = self.number(ctx, Field::Time, errors.default_time_limit.as_ref(), commit);
    let slots = self.number(ctx, Field::Slots, errors.agent_slots.as_ref(), commit);
    vec![
      section(
        "settings-budget-section",
        &ctx.t("project_settings.tokenBudgets"),
        None,
        vec![
          page::row(
            ctx,
            "settings-default",
            ("project_settings.defaultBudget", "project_settings.defaultBudgetDetail"),
            default,
            Place::Within,
          ),
          page::row(
            ctx,
            "settings-daily",
            ("project_settings.dailyBudget", "project_settings.dailyBudgetDetail"),
            daily,
            Place::Last,
          ),
        ],
      )
      .into(),
      section(
        "settings-time-section",
        &ctx.t("project_settings.time"),
        None,
        vec![page::row(
          ctx,
          "settings-time",
          ("project_settings.timeLimit", "project_settings.timeDetail"),
          time,
          Place::Last,
        )],
      )
      .into(),
      section(
        "settings-agents-section",
        &ctx.t("project_settings.agents"),
        None,
        vec![page::row(
          ctx,
          "settings-slots",
          ("project_settings.slots", "project_settings.slotsDetail"),
          slots,
          Place::Last,
        )],
      )
      .into(),
    ]
  }
  fn outside(&self, ctx: &Ctx, read: Read) -> Element {
    let tab = self.clone();
    Notice::new(
      "settings-changed-outside",
      NoticeTone::Warning,
      ctx.t("project_settings.outsideTitle"),
    )
    .body(ctx.t("project_settings.outsideDetail"))
    .action(
      ActionButton::new(
        "settings-reload",
        ButtonKind::Secondary,
        ctx.t("project_settings.reload"),
      )
      .on_click(move || tab.fill(read.clone())),
    )
    .into()
  }
}
