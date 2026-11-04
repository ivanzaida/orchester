//! The todo list of the step at work, pinned over the composer (`07 · Conversation · the plan while the step works`
//! `Y6ouq`): 32 in from the tab's sides as the composer is, 8 over it, its head saying who keeps the list and in
//! which step (*Builder · 03 Implement*). Only while the run goes on and only for a step that has not ended; a step
//! whose agent keeps no list pins nothing (07b `r44wqm`). Once the step ends its list folds into its divider
//! (`timeline::todo`).

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::run::StepStatus;
use orchester_workspace::tasks::{
  conversation::{Conversation, PanelState},
  detail::{RunView, StepView},
};

use super::timeline::step_label;
use crate::ui::todo_list::{PinnedPlan, PinnedProps};

/// Whether the steps' lists pin over the composer while the conversation stands at `state`: while the run goes on.
pub(super) const fn pins(state: PanelState) -> bool {
  matches!(state, PanelState::Live | PanelState::Waiting)
}

/// Whether `step`'s list is pinned while the run goes on: it works, or waits for the person, and has not ended.
pub(super) const fn works(step: &StepView) -> bool {
  matches!(step.status, Some(StepStatus::Running | StepStatus::Waiting(_)))
}

/// The lists of the run's steps at work, over the composer; `None` while none is pinned.
pub(super) fn pinned(ctx: &mut Ctx, run: Option<&RunView>, conversation: &Conversation) -> Option<Element> {
  if !pins(conversation.state) {
    return None;
  }
  let lists: Vec<(&StepView, _)> = run?
    .steps
    .iter()
    .filter(|step| works(step))
    .filter_map(|step| Some((step, step.todo.clone()?)))
    .collect();
  if lists.is_empty() {
    return None;
  }
  // Mounted by place, not by step: a box the person folded stays folded as the next step pins its list.
  let boxes: Vec<Element> = lists
    .into_iter()
    .enumerate()
    .map(|(index, (step, list))| {
      let label = step_label(ctx, step.number, &step.name);
      let owner = ctx.t_args(
        "conversation.plan.owner",
        [("agent", step.agent.as_deref().unwrap_or_default()), ("step", &*label)],
      );
      let props = PinnedProps {
        id: format!("conversation-plan-{}", step.key).into(),
        list,
        owner,
      };
      ctx.mount_keyed::<PinnedPlan>(&format!("conversation-plan-pinned-{index}"), props)
    })
    .collect();
  Some(
    Column::new()
      .id("conversation-plan")
      .width(Dimension::Pct(100.0))
      .padding_horizontal(SpacingSize::Section)
      .padding_bottom(SpacingSize::Sm)
      .spacing(SpacingSize::Sm)
      .with_children(boxes)
      .into(),
  )
}
