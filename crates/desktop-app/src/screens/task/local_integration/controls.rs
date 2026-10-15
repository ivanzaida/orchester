//! The inspector beside the evidence (`Action and recovery inspector` `atP1P`): the step that waits for the person
//! in a `warning` stroke, its heading, what it explains, its full-width actions and a note under them
//! (`Prepare checkout` `hULqu`), then the recovery record or how the queue stands (`qX5uF`).

mod acknowledgement;
mod confirmation;
mod preparation;
mod recovery;

use std::sync::{Arc, atomic::Ordering};

pub(super) use acknowledgement::AcknowledgementState;
use acknowledgement::ReviewIdentity;
use confirmation::decision;
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::input_source,
    theme::SpacingSize,
  },
  components::{Column, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::integration::local::LocalPhase;
use orchester_workspace::tasks::local_integration::IntegrationView;
pub(super) use recovery::aborted;

use super::{
  LocalPage, Mode, TaskContext,
  words::phase_title,
  work::{Action, Request, Work},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, Protected},
  theme::LocalText,
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card_frame},
  },
};

pub(super) type Managed = Arc<parking_lot::Mutex<Option<orchester_workspace::tasks::integration_jobs::LocalDecision>>>;

#[derive(Clone)]
struct Props {
  view: Arc<IntegrationView>,
  mode: Signal<Mode>,
  selected: Signal<super::Selection>,
  message: Signal<String>,
  name: Signal<String>,
  trigger: Trigger,
  busy: bool,
  narrow: bool,
  editor_dirty: Signal<bool>,
  acknowledgement: AcknowledgementState,
  managed: Managed,
  job: Option<orchester_workspace::tasks::integration_jobs::RunJobs>,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.trigger.task == other.trigger.task
      && self.view == other.view
      && self.busy == other.busy
      && self.narrow == other.narrow
  }
}
impl DevtoolsInspectable for Props {}

/// What starts the page's work: the task, the page's worker, its stop flag, the input gate and the operation's
/// version the decision was made against.
#[derive(Clone)]
pub(super) struct Trigger {
  task: TaskContext,
  work: Work,
  stop: Arc<std::sync::atomic::AtomicBool>,
  gate: InputGate,
  generation: Option<u64>,
}

impl Trigger {
  pub(super) fn of(ctx: &mut Ctx, page: &LocalPage, task: &TaskContext, view: &IntegrationView) -> Self {
    Self {
      task: task.clone(),
      work: page.work.clone(),
      stop: page.stop.clone(),
      gate: ctx.use_context().unwrap_or_default(),
      generation: view.operation.as_ref().map(|operation| operation.version()),
    }
  }

  fn request(&self, action: Action) -> Request {
    Request {
      task: self.task.clone(),
      action,
      stop: self.stop.clone(),
      input: None,
      generation: self.generation,
    }
  }

  /// A button `id` of `kind` saying `label` that runs `action`, or that cannot be pressed.
  pub(super) fn button(
    &self,
    ctx: &Ctx,
    (id, kind, label): (&str, ButtonKind, &str),
    action: Option<Action>,
    disabled: bool,
  ) -> ActionButton {
    let button = ActionButton::new(id, kind, ctx.t(label));
    if disabled {
      return button.disabled();
    }
    let Some(action) = action else {
      return button;
    };
    let request = self.request(action);
    let (work, gate) = (self.work.clone(), self.gate.clone());
    button.on_click(move || {
      let mut request = request.clone();
      request.initiate(&gate);
      request.stop.store(false, Ordering::Release);
      work.run_if_idle(request);
    })
  }
}

pub(super) fn inspector(
  ctx: &mut Ctx,
  page: &LocalPage,
  (task, view): (&TaskContext, &IntegrationView),
  busy: bool,
  narrow: bool,
) -> Element {
  let services = crate::services::Services::of(ctx);
  let job = task
    .detail
    .run
    .as_ref()
    .map(|run| services.jobs.of(task.project.open.project.id()).of(run.id()));
  let props = Props {
    view: Arc::new(view.clone()),
    mode: page.mode.clone(),
    selected: page.selected.clone(),
    message: page.message.clone(),
    name: page.name.clone(),
    trigger: Trigger::of(ctx, page, task, view),
    busy,
    narrow,
    editor_dirty: page.editor_dirty.clone(),
    acknowledgement: page.acknowledgement.clone(),
    managed: page.managed.clone(),
    job,
  };
  ctx.mount_keyed::<Inspector>("local-inspector", props)
}

struct Inspector {
  gate: Option<InputGate>,
  protected: parking_lot::Mutex<Option<Protected>>,
  agree: Signal<bool>,
  refused: Signal<bool>,
  identity: Arc<parking_lot::Mutex<Option<ReviewIdentity>>>,
}
impl Component for Inspector {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      gate: ctx.use_context(),
      protected: parking_lot::Mutex::new(None),
      agree: ctx.props::<Props>().acknowledgement.agree.clone(),
      refused: ctx.props::<Props>().acknowledgement.refused.clone(),
      identity: ctx.props::<Props>().acknowledgement.identity.clone(),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let phase = props
      .view
      .operation
      .as_ref()
      .map_or(LocalPhase::Review, |operation| operation.phase());
    let mode = props.mode.get();
    let guarded = matches!(mode, Mode::Abort | Mode::Cancel | Mode::Cleanup) || phase == LocalPhase::AwaitingApproval;
    let pending = decision(&props, &mode, phase).is_some();
    let mut managed = props.managed.lock();
    if pending && managed.is_none() {
      *managed = props.job.as_ref().and_then(|job| job.local_decision());
    } else if !pending {
      *managed = None;
    }
    drop(managed);
    let mut protected = self.protected.lock();
    if guarded && protected.is_none() {
      *protected = self
        .gate
        .as_ref()
        .map(|gate| gate.protect(Guarded::LocalIntegrationQuestion));
    } else if !guarded {
      *protected = None;
    }
    drop(protected);
    let identity = ReviewIdentity::of(&props, &mode);
    let mut prior = self.identity.lock();
    if prior.as_ref() != Some(&identity) {
      self.agree.set(false);
      self.refused.set(false);
      *prior = Some(identity);
    }
    drop(prior);
    let panel = self.actions(ctx, &props, &mode, phase);
    Column::new()
      .id("local-side")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .child(panel)
      .with_children(recovery::card(ctx, &props.view, &mode, props.narrow))
  }
}

impl Inspector {
  fn actions(&self, ctx: &mut Ctx, props: &Props, mode: &Mode, phase: LocalPhase) -> Element {
    if let Some(evidence) = &props.view.external {
      return external_panel(ctx, props, evidence);
    }
    if props.busy {
      let stop = props.trigger.stop.clone();
      return action_panel(ctx, phase_title(phase), None)
        .child(
          button(ctx, props, "local-stop", "local_integration.pause", None, false)
            .on_click(move || stop.store(true, Ordering::Release)),
        )
        .into();
    }
    match decision(props, mode, phase) {
      Some(decision) => self.confirmation(ctx, props, (mode, phase), decision),
      None => self.available(ctx, props, phase),
    }
  }

  fn available(&self, ctx: &Ctx, props: &Props, phase: LocalPhase) -> Element {
    if phase == LocalPhase::Review {
      return self.preparation(ctx, props);
    }
    let panel = action_panel(ctx, phase_title(phase), None);
    let action = |id: &str, label: &str, action: Action| button(ctx, props, id, label, Some(action), false);
    match phase {
      LocalPhase::Prepared => panel.with_children(self.prepared(ctx, props)),
      LocalPhase::Canceling => panel.child(action(
        "local-resume-cancel",
        "local_integration.reconcile",
        Action::Reconcile,
      )),
      LocalPhase::Canceled => {
        if props
          .view
          .operation
          .as_ref()
          .and_then(|op| op.preservation())
          .is_some_and(|saved| saved.retained)
        {
          panel.child(mode_button(
            ctx,
            props,
            ("local-cleanup-canceled", ButtonKind::Primary),
            "local_integration.cleanup",
            Mode::Cleanup,
          ))
        } else {
          panel.child(action("local-restart", "local_integration.refresh", Action::Refresh))
        }
      }
      LocalPhase::MergedAwaitingRestore => {
        panel.child(action("local-restore", "local_integration.restore", Action::Restore))
      }
      LocalPhase::RestorationVerified => panel.child(mode_button(
        ctx,
        props,
        ("local-review-cleanup", ButtonKind::Primary),
        "local_integration.cleanup",
        Mode::Cleanup,
      )),
      LocalPhase::NeedsReconciliation => self.reconciliation(ctx, props, panel),
      LocalPhase::Complete => panel,
      _ => panel.child(action(
        "local-reconcile",
        "local_integration.reconcile",
        Action::Reconcile,
      )),
    }
    .into()
  }

  fn reconciliation(&self, ctx: &Ctx, props: &Props, panel: Column) -> Column {
    let panel = panel.child(button(
      ctx,
      props,
      "local-reconcile",
      "local_integration.reconcile",
      Some(Action::Reconcile),
      false,
    ));
    if props
      .view
      .operation
      .as_ref()
      .is_some_and(|op| op.merged().is_none() && op.preservation().is_none())
    {
      panel.child(
        props
          .trigger
          .button(
            ctx,
            (
              "local-review-current",
              ButtonKind::Secondary,
              "local_integration.refresh",
            ),
            Some(Action::Refresh),
            false,
          )
          .fill_width(),
      )
    } else {
      panel
    }
  }
}

/// The step's card for an integration made outside Orchester: merged without Orchester, as the Summary says, or
/// blocked until it is reconciled.
fn external_panel(ctx: &Ctx, props: &Props, evidence: &orchester_core::integration::local::ContainedResult) -> Element {
  if !super::words::merged_outside(evidence) {
    return action_panel(
      ctx,
      "local_integration.externalTitle",
      Some("local_integration.externalBody"),
    )
    .into();
  }
  let task = &props.trigger.task;
  let branch = task
    .detail
    .run
    .as_ref()
    .map(|run| run.run.branch().as_str())
    .unwrap_or_default();
  let (title, body) = super::words::merged_outside_words(ctx, branch, &props.view.checkout.branch);
  card_frame("local-actions", CardSurface::Pending)
    .padding(SpacingSize::Lg)
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&title)
        .id("local-actions-title")
        .variant(LocalText::Heading.role()),
    )
    .child(
      Text::new(&body)
        .id("local-actions-body")
        .variant(LocalText::Explanation.role()),
    )
    .into()
}

/// The step's card: `title` and what `body` explains; add its actions and note.
fn action_panel(ctx: &Ctx, title: &str, body: Option<&str>) -> Column {
  card_frame("local-actions", CardSurface::Pending)
    .padding(SpacingSize::Lg)
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&ctx.t(title))
        .id("local-actions-title")
        .variant(LocalText::Heading.role()),
    )
    .with_children(body.map(|body| {
      Text::new(&ctx.t(body))
        .id("local-actions-body")
        .variant(LocalText::Explanation.role())
    }))
}

/// A note under the step's actions (`Person-only preparation` `Q1k8Vy`).
fn guard_note(ctx: &Ctx, key: &str) -> Text {
  Text::new(&ctx.t(key))
    .id("local-actions-note")
    .variant(LocalText::Note.role())
}

fn mode_button(ctx: &Ctx, props: &Props, (id, kind): (&str, ButtonKind), label: &str, chosen: Mode) -> ActionButton {
  let mode = props.mode.clone();
  let managed = props.managed.clone();
  let job = props.job.clone();
  let gate = props.trigger.gate.clone();
  let work = props.trigger.work.clone();
  let refusal = props.trigger.request(Action::BusyRefused);
  ActionButton::new(id, kind, ctx.t(label))
    .fill_width()
    .on_click(move || {
      if !gate.local_input(input_source()).allowed() {
        let mut refused = refusal.clone();
        refused.action = Action::AuthorityRefused;
        work.run_if_idle(refused);
        return;
      }
      if chosen != Mode::Review {
        let mut guard = managed.lock();
        if guard.is_none() {
          *guard = job.as_ref().and_then(|job| job.local_decision());
          if guard.is_none() {
            work.run_if_idle(refusal.clone());
            return;
          }
        }
      }
      mode.set(chosen.clone());
    })
}

/// The step's primary action, as wide as its card.
fn button(ctx: &Ctx, props: &Props, id: &str, label: &str, action: Option<Action>, disabled: bool) -> ActionButton {
  props
    .trigger
    .button(ctx, (id, ButtonKind::Primary, label), action, disabled)
    .fill_width()
}
