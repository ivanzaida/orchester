//! Plan review (`Decision · plan` `x0PVdp` in `01c` `SaKMC`): a run whose gated step waits for a decision on its
//! plan. At the top of the Summary the plan's card ([`card`]): the plan as its agent wrote it, a choice of the step's
//! versions, and the task's acceptance criteria; at its end the decision ([`bar`]): the note for the agent, *Request
//! changes*, which needs the note, and *Approve plan & continue*. A decision goes to the runner, which records it and
//! continues the run or gives the step's agent the note; an earlier version reads without a decision, as the card
//! always decides the plan that waits.

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
use super::{TaskContext, action::decision_action, arranged, pointing::Pointing};
use crate::{navigation::pointed::PointedCard, ui::markdown::LinkHandler};

/// The version the card shows; `None` is the one that waits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown(Option<PlanVersion>);

impl lurq::app::component::DevtoolsInspectable for Shown {}

pub struct PlanDecision {
  shown: Signal<Shown>,
  note: Signal<String>,
  asked: Signal<Option<Choice>>,
  person_only: Signal<bool>,
  /// Opens a link of the plan in the browser; kept so the plan's Markdown is laid out only when its text
  /// changes.
  on_link: LinkHandler,
  pointing: Pointing,
}

impl Component for PlanDecision {
  type Props = TaskContext;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      shown: ctx.signal(Shown(None)),
      note: ctx.signal(String::new()),
      asked: ctx.signal(None),
      person_only: ctx.signal(false),
      on_link: crate::ui::chat::links(ctx),
      pointing: Pointing::new(ctx),
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
    self.pointing.arrived(ctx, &props, PointedCard::Plan);
    let task = props.detail.run_task.as_ref().unwrap_or(&props.detail.task);
    let shown = review.shown(self.shown.get().0).clone();
    let bar = DecisionBar {
      review: &review,
      run,
      note: &self.note,
      asked: &self.asked,
      action: &action,
      gate: ctx.use_context(),
      person_only: &self.person_only,
      online: props.online,
      earlier: shown.version != review.waiting().version,
    }
    .render(ctx);
    let plan = card::PlanCard {
      review: &review,
      shown: &shown,
      task,
      on_link: &self.on_link,
      pointing: &self.pointing,
    };
    arranged(vec![plan.render(ctx, &self.shown, bar)])
  }
}
