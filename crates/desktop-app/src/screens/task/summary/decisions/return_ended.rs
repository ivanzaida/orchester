//! *Return the work to an earlier step* above *Run details* of a run that ended (the project task ORC-44 · Return a
//! finished run to an earlier step): the person picks one of the agent steps the run executed, writes the note its
//! agent reads, and the run reopens there in a new attempt; the steps after it run again
//! ([`orchester_workspace::tasks::return_work`], the same return `continue_run`'s `return_to` sends). *Return to a
//! step* in the run's header points here. A run that waits for a person returns its work from its decision
//! card instead. The design has no frame of its own for it yet: it reuses the return part of the retry decision
//! (`Return the work to` `qrV0F`, the note `wJTjt`, *Return to … with note* in `j7DjNG` of `DVuXu`). A merged run, one
//! whose worktree was removed and one whose worktree holds a merge cannot be returned; the button says why. Returning
//! is a person's decision: window input from an MCP client takes it only while the person lets automation answer
//! approvals, and it is recorded as automation's then.

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::{SelectOption, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  resource::StepKey,
  run::{Answerer, ReviewNote},
};
use orchester_workspace::{
  decisions::DecisionKind,
  messages::Message,
  tasks::{detail::RunView, return_work},
};

use super::{
  TaskContext,
  action::{DecisionAction, decision_action, state},
  pointing::Pointing,
  ready::in_worktree,
  step_label,
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, answerer},
  navigation::pointed::PointedCard,
  theme::{ExtraSpacing, ExtraTypography, TaskText},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    field::{FieldInput, SelectField, field, field_error},
    icon::Icon,
    message::message_text,
    unavailable::reason_line,
  },
};

/// The step the work goes back to; `None` before the choice is seeded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target(Option<StepKey>);

impl lurq::app::component::DevtoolsInspectable for Target {}

/// The return under way, for its button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Returning;

impl lurq::app::component::DevtoolsInspectable for Returning {}

/// `props`' latest run, when it ended so its work can go back to one of its agent steps and was not merged. A Not
/// shared task is read-only: its run goes back to no step either.
pub fn returnable(props: &TaskContext) -> Option<&RunView> {
  let run = props.detail.run.as_ref()?;
  let merged =
    props.detail.merge.as_ref().is_some_and(|merge| merge.run == run.id()) || props.jobs.run(run.id()).merged.is_some();
  let refused = props.detail.not_shared() || merged || return_work::targets(run).is_empty();
  (!refused && run.run.can_return_ended()).then_some(run)
}

/// The card of the [`returnable`] run: mounted afresh as the run changes, so a note written for it does not outlive it.
pub fn return_card(ctx: &mut Ctx, props: &TaskContext) -> Option<Element> {
  let run = returnable(props)?;
  let key = format!("return-ended-{}-{}", run.id(), run.run.last_sequence().get());
  Some(ctx.mount_keyed::<ReturnEnded>(&key, props.clone()))
}

pub struct ReturnEnded {
  target: Signal<Target>,
  note: Signal<String>,
  asked: Signal<Option<Returning>>,
  /// Window input pressed *Return to … with note* while the person does not let automation decide for them.
  person_only: Signal<bool>,
  gate: Option<InputGate>,
  pointing: Pointing,
}

impl Component for ReturnEnded {
  type Props = TaskContext;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      target: ctx.signal(Target(None)),
      note: ctx.signal(String::new()),
      asked: ctx.signal(None),
      person_only: ctx.signal(false),
      gate: ctx.use_context::<InputGate>(),
      pointing: Pointing::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TaskContext>().clone();
    let action = decision_action::<Returning>(ctx, props.project.open.clone());
    let Some(run) = props.detail.run.as_ref() else {
      return lurq::components::Column::new();
    };
    // Read before anything only some renders build: lurq keeps futures by their order.
    let worktree = in_worktree(ctx, &props, (run.id(), 0));
    let targets = return_work::targets(run);
    if self.target.get_untracked().0.is_none() {
      let default = return_work::implementing_step(run)
        .filter(|step| targets.contains(step))
        .or_else(|| targets.last().cloned());
      self.target.set(Target(default));
    }
    self.pointing.arrived(ctx, &props, PointedCard::Return);
    let unavailable = match worktree.as_ref() {
      Some(worktree) => return_work::refusal(&props.detail, run, worktree).map(|refusal| refusal.message()),
      None => None,
    }
    .map(|message| message_text(ctx, &message))
    .or_else(|| (!props.online).then(|| ctx.t("task.controls.offline")));
    let (pending, refused) = state(&action, self.asked.get());
    let parts = Parts {
      run,
      targets: &targets,
      unavailable,
      read: worktree.is_some(),
      pending,
    };
    let card = card("task-return-ended", CardSurface::Decision)
      .spacing(ExtraSpacing::Loose.role())
      .child(
        Text::new(&ctx.t("decisions.returnEnded.title"))
          .id("task-return-ended-title")
          .variant(TaskText::SectionTitle.role()),
      )
      .child(
        Text::new(&ctx.t("decisions.returnEnded.body"))
          .id("task-return-ended-body")
          .variant(ExtraTypography::Note.role())
          .width(Dimension::Pct(100.0)),
      )
      .with_children(self.fields(ctx, &parts))
      .child(self.button(ctx, &parts, &action))
      .with_children(
        parts
          .unavailable
          .as_ref()
          .map(|reason| reason_line("task-return-ended-reason", reason)),
      )
      .with_children(self.person_only.get().then(|| {
        field_error(
          "task-return-ended-person-only",
          &message_text(ctx, &Message::new("appMcp.errors.automationNotAllowed")),
        )
      }))
      .with_children(refused.map(|refused| field_error("task-return-ended-refused", &message_text(ctx, &refused))));
    self.pointing.ring(card)
  }
}

/// What the card is drawn from.
struct Parts<'a> {
  run: &'a RunView,
  targets: &'a [StepKey],
  /// Why the work cannot go back now.
  unavailable: Option<Arc<str>>,
  /// The run's worktree was read, so whether it can go back is known.
  read: bool,
  pending: Option<Returning>,
}

impl ReturnEnded {
  /// The step choice and the note for its agent.
  fn fields(&self, ctx: &mut Ctx, parts: &Parts<'_>) -> Vec<Element> {
    let options: Vec<SelectOption<Target>> = parts
      .targets
      .iter()
      .map(|key| SelectOption::new(Target(Some(key.clone())), name(ctx, parts.run, key)))
      .collect();
    let agent = self
      .chosen(parts)
      .and_then(|key| parts.run.steps.iter().find(|step| step.key == key))
      .and_then(|step| step.agent.clone())
      .unwrap_or_else(|| ctx.t("decisions.theAgent"));
    let to = ctx.t("decisions.returnEnded.returnTo");
    let select = SelectField::new("task-return-ended-step", to.clone(), self.target.clone(), options).render(ctx);
    let note_label = ctx.t_args("decisions.returnEnded.noteFor", [("agent", &*agent)]);
    let note = FieldInput::new("task-return-ended-note", note_label.clone(), self.note.clone())
      .text_area()
      .placeholder(ctx.t_args("decisions.returnEnded.notePlaceholder", [("agent", &*agent)]))
      .render(ctx);
    vec![
      field("task-return-ended-step-field", Some(&to), select, None).into(),
      field("task-return-ended-note-field", Some(&note_label), note, None).into(),
    ]
  }

  /// The chosen step, while it is one the work may go back to.
  fn chosen(&self, parts: &Parts<'_>) -> Option<StepKey> {
    self.target.get().0.filter(|key| parts.targets.contains(key))
  }

  /// *Return to … with note*: pressable once a step is chosen and the note says something, while the run may go back.
  fn button(&self, ctx: &Ctx, parts: &Parts<'_>, action: &DecisionAction<Returning>) -> ActionButton {
    let chosen = self.chosen(parts);
    let step = chosen
      .as_ref()
      .and_then(|key| parts.run.steps.iter().find(|step| step.key == *key))
      .map_or_else(String::new, |step| step.name.to_string());
    let button = ActionButton::new(
      "task-return-ended-button",
      ButtonKind::Secondary,
      ctx.t_args("decisions.returnEnded.returnWithNote", [("step", step.as_str())]),
    )
    .icon(Icon::CornerDownLeft)
    .in_form()
    .fill_width();
    if let Some(reason) = &parts.unavailable {
      return button.unavailable(reason.clone());
    }
    if parts.pending.is_some() {
      return button.loading();
    }
    let note = ReviewNote::new(&self.note.get()).ok();
    let (Some(step), Some(note), true) = (chosen, note, parts.read) else {
      return button.disabled();
    };
    // The same return `continue_run`'s `return_to` sends, decided by whoever pressed.
    let returned = |by| return_work::command(parts.run, step.clone(), (note.clone(), None), by);
    let (person, automation) = (returned(Answerer::Person), returned(Answerer::Automation));
    let (action, asked) = (action.clone(), self.asked.clone());
    let (gate, person_only) = (self.gate.clone(), self.person_only.clone());
    button.on_click(move || {
      // A second press while the first is under way would replace it (lurq's `FutureAction::run`).
      if action.state().get_untracked().is_pending() {
        return;
      }
      let Some(by) = answerer(gate.as_ref(), Guarded::Decision(DecisionKind::Stuck)) else {
        tracing::warn!("synthetic input returned an ended run without consent; not taken");
        person_only.set(true);
        return;
      };
      person_only.set(false);
      asked.set(Some(Returning));
      let command = if by == Answerer::Automation {
        &automation
      } else {
        &person
      };
      action.run((Returning, command.clone()));
    })
  }
}

/// "03 Implement", or the key of a step the run no longer lists.
fn name(ctx: &Ctx, run: &RunView, key: &StepKey) -> Arc<str> {
  run
    .steps
    .iter()
    .find(|step| step.key == *key)
    .map_or_else(|| Arc::from(key.as_str()), |step| step_label(ctx, step))
}
