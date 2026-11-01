//! Limits (`Limits` `tmV5e` in `02 · Runs`): the run and daily budgets, the time limit and how many agents and QA apps
//! work at once, number fields with their units and read-backs, each saved as a person leaves it, with per-field
//! validation and a change made outside the screen said under the group. Raising or removing the daily budget lets
//! runs it stopped go on, a person's decision: window input from an MCP client saves it only while the person lets
//! automation answer approvals.
use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Column, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::tokens::compact_tokens;
use orchester_workspace::{
  budget::LimitsErrors, decisions::DecisionKind, messages::Message, project_settings::LimitsText,
  tasks::outside::Outside,
};

use super::super::{
  Status, TabProps,
  kit::{Group, SettingRow},
  work::{self, Change, Read, Save},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  theme::ExtraTypography,
  ui::{
    button::{ActionButton, ButtonKind},
    field::{NumberField, field_error},
    message::message_text,
    notice::{Notice, NoticeTone},
    skeleton,
    tooltip::Tooltip,
  },
};

/// A limit's box (`Box` `jJ4rP`).
const FIELD_WIDTH: f32 = 160.0;

#[derive(Clone)]
pub(super) struct Limits {
  save: Save,
  default: Signal<String>,
  daily: Signal<String>,
  minutes: Signal<String>,
  slots: Signal<String>,
  qa: Signal<String>,
  stored: Signal<Option<Read>>,
  revision: Signal<u64>,
  own: work::OwnSaves,
  separator: Arc<str>,
  submitted: Signal<Submitted>,
  /// A save asked for while another ran, with whether every input that asked for it may raise the daily budget.
  queued: Signal<Option<bool>>,
  gate: Option<InputGate>,
  /// Window input raised or removed the daily budget while the person does not let automation decide for them.
  person_only: Signal<bool>,
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
  /// *QA apps at once* (`DsTyK`), built as *Agents at once*.
  Qa,
}
impl Field {
  const fn id(self) -> &'static str {
    match self {
      Self::Default => "settings-default-budget",
      Self::Daily => "settings-daily-budget",
      Self::Time => "settings-time-limit",
      Self::Slots => "settings-agent-slots",
      Self::Qa => "settings-qa-slots",
    }
  }
  /// The row's element id.
  const fn row(self) -> &'static str {
    match self {
      Self::Default => "settings-default",
      Self::Daily => "settings-daily",
      Self::Time => "settings-time",
      Self::Slots => "settings-slots",
      Self::Qa => "settings-qa",
    }
  }
  /// The row's label, its hint, what its (i) says, and the field's unit.
  const fn keys(self) -> [&'static str; 4] {
    match self {
      Self::Default => [
        "project_settings.runBudget",
        "project_settings.runBudgetHint",
        "project_settings.defaultBudgetDetail",
        "project_settings.tokens",
      ],
      Self::Daily => [
        "project_settings.dailyBudgetLabel",
        "project_settings.dailyBudgetHint",
        "project_settings.dailyBudgetDetail",
        "project_settings.tokens",
      ],
      Self::Time => [
        "project_settings.timeLimitLabel",
        "project_settings.timeLimitHint",
        "project_settings.timeDetail",
        "project_settings.minutesUnit",
      ],
      Self::Slots => [
        "project_settings.slotsLabel",
        "project_settings.countRange",
        "project_settings.slotsDetail",
        "project_settings.agentsUnit",
      ],
      Self::Qa => [
        "project_settings.qaSlots",
        "project_settings.countRange",
        "project_settings.qaSlotsDetail",
        "project_settings.appsUnit",
      ],
    }
  }
  /// A count of slots: always entered, and with nothing to read back.
  const fn counts(self) -> bool {
    matches!(self, Self::Slots | Self::Qa)
  }
}
/// The Limits group while it is read (`10 · Project settings · Runs · loading` `lUFjj`): its heading, its note, its
/// foot and every row's label, hint and unit real, each value a bone in its field.
pub(super) fn bones(ctx: &mut Ctx) -> Element {
  let rows = [Field::Default, Field::Daily, Field::Time, Field::Slots, Field::Qa].map(|kind| {
    let [label, hint, info, unit] = kind.keys();
    let value = if kind.counts() { 14.0 } else { 46.0 };
    SettingRow::new(format!("{}-reading", kind.row()), ctx.t(label))
      .hint(ctx.t(hint))
      .info(ctx.t(info))
      .control(skeleton::unit_field(ctx, FIELD_WIDTH, value, &ctx.t(unit)))
      .render(ctx)
  });
  let note = Text::new(&ctx.t("project_settings.limitsNote")).variant(ExtraTypography::Note.role());
  Group::new("settings-limits-reading", ctx.t("project_settings.limits"))
    .end(note)
    .rows(rows)
    .foot(&ctx.t("project_settings.limitsFoot"))
    .into()
}
impl Component for Limits {
  type Props = TabProps;
  fn create(ctx: &mut Ctx) -> Self {
    let tab = Self {
      save: work::action(ctx, ctx.props::<TabProps>().project.open.project.id()),
      default: ctx.signal(String::new()),
      daily: ctx.signal(String::new()),
      minutes: ctx.signal(String::new()),
      slots: ctx.signal(String::new()),
      qa: ctx.signal(String::new()),
      stored: ctx.signal(None),
      revision: ctx.signal(0),
      own: work::OwnSaves::new(ctx, &[Outside::Limits]),
      separator: ctx.t("project_settings.groupSeparator"),
      submitted: ctx.signal(Submitted(None)),
      queued: ctx.signal(None),
      gate: ctx.use_context::<InputGate>(),
      person_only: ctx.signal(false),
    };
    let save = tab.save.clone();
    work::follow(
      ctx,
      &save,
      ctx.props::<TabProps>().status.clone(),
      (tab.revision.clone(), tab.stored.clone(), tab.own.clone()),
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
        if let Some(may_raise) = after.queued.get_untracked() {
          after.submit(&root, &next, may_raise);
        }
      }
    });
    tab
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let save = self.save.clone();
    let revision = self.revision.get() + self.own.count(&props.project.feed);
    let state = work::load(ctx, props.project.open.repo_root.clone(), revision, &self.own);
    let Some(data) = state.data.clone().or_else(|| self.stored.get()) else {
      return work::pending(ctx, &state, &self.revision, bones);
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
    if !changed && !save.state().get().is_pending() && save.state().get().error.is_some() {
      self.submitted.set(Submitted(None));
      self.queued.set(None);
      save.state().set(lurq::app::ctx::FutureState::idle());
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
    let rows = self.rows(ctx, &errors, &commit);
    let note = Text::new(&ctx.t("project_settings.limitsNote"))
      .id("settings-limits-note")
      .variant(ExtraTypography::Note.role());
    let note = Tooltip::new("settings-changes-notice", ctx.t("project_settings.changesDetail")).wrap(ctx, note);
    let mut group = Group::new("settings-limits", ctx.t("project_settings.limits"))
      .end(note)
      .rows(rows)
      .foot(&ctx.t("project_settings.limitsFoot"));
    if let Some(error) = work::error(ctx, &save) {
      let commit = commit.clone();
      group = group.after(
        Column::new()
          .spacing(lurq::app::theme::SpacingSize::Sm)
          .align_items(Alignment::Start)
          .child(error)
          .child(
            ActionButton::new(
              "settings-limits-retry",
              ButtonKind::Secondary,
              ctx.t("project_settings.retry"),
            )
            .on_click(move || commit()),
          ),
      );
    }
    if let Some(error) = state.error {
      group = group.after(field_error("settings-limits-read-error", &message_text(ctx, &error)));
    }
    if let Some(refused) = self.person_only_line(ctx, changed) {
      group = group.after(refused);
    }
    if outside && changed {
      group = group.after(self.outside(ctx, data));
    }
    group.into()
  }
}
impl Limits {
  /// Why a client's raise of the daily budget was not saved, while the fields still hold it.
  fn person_only_line(&self, ctx: &Ctx, changed: bool) -> Option<Element> {
    if !changed && self.person_only.get_untracked() {
      self.person_only.set(false);
    }
    self.person_only.get().then(|| {
      field_error(
        "settings-limits-person-only",
        &message_text(ctx, &Message::new("appMcp.errors.automationNotAllowed")),
      )
      .into()
    })
  }
  fn text(&self) -> LimitsText {
    LimitsText {
      default: self.default.get(),
      daily: self.daily.get(),
      minutes: self.minutes.get(),
      slots: self.slots.get(),
      qa: self.qa.get(),
    }
  }
  fn fill(&self, read: Read) {
    let text = LimitsText::of(read.0.limits, &self.separator);
    self.default.set(text.default);
    self.daily.set(text.daily);
    self.minutes.set(text.minutes);
    self.slots.set(text.slots);
    self.qa.set(text.qa);
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
      (&self.qa, &sent.qa, read.qa),
    ] {
      if signal.get_untracked() == *submitted {
        signal.set(saved);
      }
    }
    self.submitted.set(Submitted(None));
  }
  fn commit(&self, props: &TabProps, save: &Save) -> Arc<dyn Fn() + Send + Sync> {
    let (tab, save, root) = (self.clone(), save.clone(), props.project.open.repo_root.clone());
    Arc::new(move || {
      // Read while the input that left the field is delivered; a save queued behind another keeps it.
      let may_raise = input_may_decide(tab.gate.as_ref(), Guarded::Decision(DecisionKind::Limit));
      tab.submit(&root, &save, may_raise);
    })
  }
  /// Saves the fields as they read now; `may_raise` is whether the input that asked may raise or remove the daily
  /// budget.
  fn submit(&self, root: &Path, save: &Save, may_raise: bool) {
    if save.state().get_untracked().is_pending() {
      // A later blur must be committed even when it reverted to the pre-save baseline. Queued by inputs that may not
      // raise the daily budget, it does not.
      let queued = self
        .queued
        .get_untracked()
        .map_or(may_raise, |earlier| earlier && may_raise);
      self.queued.set(Some(queued));
      return;
    }
    self.queued.set(None);
    let Some(base) = self.stored.get_untracked() else {
      return;
    };
    let text = self.text();
    let Ok(asked) = text.limits(base.0.limits, &self.separator) else {
      return;
    };
    if base.0.limits.raises_daily_budget(asked) && !may_raise {
      tracing::warn!("synthetic input raised the daily budget without consent; not saved");
      self.person_only.set(true);
      return;
    }
    self.person_only.set(false);
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
  /// What a value set reads back as; nothing for a count, or for a limit that is not set (its box says No limit).
  fn read_back(&self, ctx: &Ctx, kind: Field, text: String) -> Arc<str> {
    if kind.counts() || text.trim().is_empty() {
      return Arc::from("");
    }
    if matches!(kind, Field::Time) {
      return ctx.t_args("project_settings.timeReadback", [("count", text)]);
    }
    let count = orchester_workspace::budget::read_optional_budget(&text, &self.separator)
      .map(|v| v.map(|v| compact_tokens(v.get())).unwrap_or_default())
      .unwrap_or_default();
    ctx.t_args(
      if matches!(kind, Field::Daily) {
        "project_settings.dailyBudgetPerDay"
      } else {
        "project_settings.budgetReadback"
      },
      [("count", count)],
    )
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
      Field::Qa => &self.qa,
    };
    let [label, _, _, unit] = kind.keys();
    let readback = self.read_back(ctx, kind, value.get());
    let commit = commit.clone();
    let mut field = NumberField::new(kind.id(), ctx.t(label), value.clone(), ctx.t(unit), readback)
      .width(Dimension::Px(FIELD_WIDTH))
      .on_leave(move || commit());
    if !kind.counts() {
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
      Field::Qa => return limits.qa_slots().get().to_string(),
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

  fn rows(&self, ctx: &mut Ctx, errors: &LimitsErrors, commit: &Arc<dyn Fn() + Send + Sync>) -> Vec<Element> {
    [
      (Field::Default, errors.default_run_budget.as_ref()),
      (Field::Daily, errors.daily_budget.as_ref()),
      (Field::Time, errors.default_time_limit.as_ref()),
      (Field::Slots, errors.agent_slots.as_ref()),
      (Field::Qa, errors.qa_slots.as_ref()),
    ]
    .into_iter()
    .map(|(kind, error)| {
      let field = self.number(ctx, kind, error, commit);
      let [label, hint, info, _] = kind.keys();
      SettingRow::new(kind.row(), ctx.t(label))
        .hint(ctx.t(hint))
        .info(ctx.t(info))
        .control(field)
        .render(ctx)
    })
    .collect()
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
