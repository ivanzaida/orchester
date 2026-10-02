//! Review & approve plan (02.11 `x3lVC`): the Summary of a task whose run waits for a decision
//! on a gated step's plan. The plan, as its agent wrote it (Markdown), with *Approve plan &
//! continue* and *Request changes*; beside it the task's acceptance criteria, the note that tells
//! the agent what should change, the run's tokens and the plan's version — a choice of the step's
//! versions once there are several, where earlier ones read without a decision. The note and the
//! decision buttons are a form (`xURvJ`: they take focus); a decision goes to the runner, which
//! records it and continues the run or resumes the step's agent with the note.

mod aside;
mod content;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
  },
  components::Row,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{RunId, StepExecutionId},
  run::{Plan, PlanDecision, PlanVersion, ReviewNote},
};
use orchester_ipc::Command;

pub(super) use self::aside::note_field;
use crate::{
  messages::Message,
  runner::Runners,
  tasks::{
    detail::{RunView, TaskDetail},
    plan::PlanReview,
    run::control_message,
  },
  ui::form::form,
};

/// What the review shows, compared to render it again when the run changes.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanReviewProps {
  pub review: Arc<PlanReview>,
  /// The task's title and acceptance criteria.
  pub task: Arc<str>,
  pub criteria: Arc<[String]>,
  /// The run's tokens against its budget: "96.4k / 1.4M tokens".
  pub tokens: Arc<str>,
}

impl lurq::app::component::DevtoolsInspectable for PlanReviewProps {}

/// The version the review shows; `None` is the one that waits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shown(Option<PlanVersion>);

impl lurq::app::component::DevtoolsInspectable for Shown {}

/// Which decision was asked for last, for the button that shows it under way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
  Approve,
  RequestChanges,
}

impl lurq::app::component::DevtoolsInspectable for Choice {}

/// A decision on one plan, as the runner takes it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Decision {
  run: RunId,
  execution: StepExecutionId,
  version: PlanVersion,
  decision: PlanDecision,
}

impl lurq::app::component::DevtoolsInspectable for Decision {}

type DecisionAction = FutureAction<Decision, (), Message>;

fn decision_action(ctx: &mut Ctx) -> DecisionAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  ctx.future_action(move |decision: Decision| {
    let runners = runners.clone();
    async move {
      let command = Command::DecidePlan {
        run: decision.run,
        execution: decision.execution,
        version: decision.version,
        decision: decision.decision,
      };
      runners.control_run(command).await.map_err(|error| {
        tracing::warn!(%error, "the runner did not take the plan decision");
        control_message(&error)
      })
    }
  })
}

pub struct PlanReviewPanel {
  note: Signal<String>,
  note_focused: Signal<bool>,
  shown: Signal<Shown>,
  requested: Signal<Option<Choice>>,
}

impl Component for PlanReviewPanel {
  type Props = PlanReviewProps;

  fn create(ctx: &mut Ctx) -> Self {
    decision_action(ctx);
    Self {
      note: ctx.signal(String::new()),
      note_focused: ctx.signal(false),
      shown: ctx.signal(Shown(None)),
      requested: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PlanReviewProps>().clone();
    let action = decision_action(ctx);
    let shown = props.review.shown(self.shown.get().0).clone();
    let decisions = self.decisions(&props.review, &action);
    let content = content::content(ctx, &props, &shown, &decisions);
    let aside = aside::aside(ctx, &props, (&self.note, &self.note_focused), &self.shown);
    form(
      None,
      Row::new()
        .id("plan-review")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .child(content)
        .child(aside),
    )
  }
}

/// The decision controls as the content needs them: what each button does now, and what is
/// under way or failed.
pub(super) struct Decisions {
  approve: Option<Arc<dyn Fn() + Send + Sync>>,
  request_changes: Option<Arc<dyn Fn() + Send + Sync>>,
  pending: Option<Choice>,
  failure: Option<Message>,
}

impl PlanReviewPanel {
  fn decisions(&self, review: &PlanReview, action: &DecisionAction) -> Decisions {
    let state = action.state().get();
    let pending = state.is_pending().then(|| self.requested.get()).flatten();
    let waiting: &Plan = review.waiting();
    let decide = |choice: Choice, decision: PlanDecision| -> Arc<dyn Fn() + Send + Sync> {
      let (action, requested) = (action.clone(), self.requested.clone());
      let request = Decision {
        run: review.run,
        execution: waiting.execution,
        version: waiting.version,
        decision,
      };
      Arc::new(move || {
        requested.set(Some(choice));
        action.run(request.clone());
      })
    };
    let idle = !state.is_pending();
    // A blank note is the only error: it leaves Request changes disabled.
    let note = ReviewNote::new(&self.note.get()).ok();
    Decisions {
      approve: idle.then(|| decide(Choice::Approve, PlanDecision::Approved)),
      request_changes: note
        .filter(|_| idle)
        .map(|note| decide(Choice::RequestChanges, PlanDecision::ChangesRequested { note })),
      pending,
      failure: state.error.filter(|_| idle),
    }
  }
}

/// The Summary while a plan waits for a decision (02.11 `x3lVC`); a new version mounts it anew,
/// with an empty note.
pub(super) fn screen(ctx: &mut Ctx, detail: &TaskDetail, run: &RunView, review: PlanReview) -> Element {
  let key = format!("plan-review-{}-{}", run.id(), review.waiting().version);
  let task = detail.run_task.as_ref().unwrap_or(&detail.task).details();
  let props = PlanReviewProps {
    review: Arc::new(review),
    task: task.title.as_str().into(),
    criteria: task.acceptance_criteria.clone().into(),
    tokens: super::summary::budget_value(ctx, run),
  };
  ctx.mount_keyed::<PlanReviewPanel>(&key, props)
}
