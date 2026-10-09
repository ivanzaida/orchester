//! Plan review (`Task · Summary · plan review` `ciGbZ`): a run whose gated step waits for a decision on
//! its plan. Under where the task stands and the run's stages, the plan's card ([`card`]): the plan as
//! its agent wrote it, a choice of the step's versions, and the task's acceptance criteria. Under the
//! column, pinned while it scrolls, the decision bar ([`bar`]): the note for the agent, *Request
//! changes*, which needs the note, and *Approve plan & continue*. A decision goes to the runner, which
//! records it and continues the run or gives the step's agent the note; an earlier version reads without
//! a decision, as the bar always decides the plan that waits.

mod bar;
mod card;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::run::PlanVersion;
use orchester_workspace::tasks::plan::PlanReview;

use self::bar::{Choice, DecisionBar};
use super::{TaskContext, action::decision_action, arranged, stages::stages, task_notice, this_run_panel};
use crate::{services::Services, ui::markdown::LinkHandler};

/// The version the card shows; `None` is the one that waits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown(Option<PlanVersion>);

impl lurq::app::component::DevtoolsInspectable for Shown {}

pub struct PlanDecision {
  shown: Signal<Shown>,
  note: Signal<String>,
  asked: Signal<Option<Choice>>,
  /// Opens a link of the plan in the browser; kept so the plan's Markdown is laid out only when its text
  /// changes.
  on_link: LinkHandler,
}

impl Component for PlanDecision {
  type Props = TaskContext;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      shown: ctx.signal(Shown(None)),
      note: ctx.signal(String::new()),
      asked: ctx.signal(None),
      on_link: std::sync::Arc::new(|url: &str| {
        if let Err(error) = orchester_exec::external::open_url(url) {
          tracing::warn!(%error, "the plan's link was not opened");
        }
      }),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TaskContext>().clone();
    let action = decision_action::<Choice>(ctx, props.project.open.clone());
    let Some(run) = props.detail.run.as_ref() else {
      return Column::new().into();
    };
    let Some(review) = PlanReview::of(run) else {
      return Column::new().into();
    };
    let gated = run
      .run
      .awaiting_plan()
      .first()
      .and_then(|execution| run.step_of(*execution))
      .map(|step| step.key.as_str().to_owned());
    let now = Services::of(ctx).clock.now();
    let task = props.detail.run_task.as_ref().unwrap_or(&props.detail.task);
    let shown = review.shown(self.shown.get().0).clone();
    let plan = card::PlanCard {
      review: &review,
      shown: &shown,
      task,
      on_link: &self.on_link,
    };
    let parts: Vec<Element> = vec![
      task_notice(ctx, &props).into(),
      stages(run, gated.as_deref(), now),
      plan.render(ctx, &self.shown),
    ];
    let bar = DecisionBar {
      review: &review,
      run,
      note: &self.note,
      asked: &self.asked,
      action: &action,
    }
    .render(ctx);
    let side = this_run_panel(ctx, &props, None);
    arranged(parts, Some(bar), side)
  }
}
