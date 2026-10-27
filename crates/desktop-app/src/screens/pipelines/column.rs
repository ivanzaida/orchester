//! The column beside the flow, as the route says: the chosen step with its pages, the Deliver step, or the
//! pipeline's summary. The publish review is the editor's (`super::publication`), a viewed version's changes its
//! view's (`super::viewing`).
use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};

use super::{
  state::{State, View},
  step_column::{ColumnProps, StepColumn},
  step_fields::Props,
  summary::{Summary, SummaryProps},
};

/// What the column shows; `locked` while a question or a reload holds the page: its fields wait then. `busy` also
/// while a write runs: what leaves the page waits for it, but typing goes on and is saved after it. The step column
/// asks the save question under its title; the Deliver step and the summary at their top.
pub fn column(ctx: &mut Ctx, state: &State, (locked, busy): (bool, bool)) -> Element {
  let editing = state.get();
  let viewing = editing.viewing.is_some();
  if editing.view == View::Deliver {
    let deliver = super::deliver::column(ctx, state, locked);
    return with_question(ctx, deliver);
  }
  let shown = editing.shown();
  if let Some(key) = editing.view.step()
    && let Some(step) = shown.step(key)
  {
    let page = match &editing.view {
      View::StepPage(_, page) => Some(*page),
      _ => None,
    };
    let props = ColumnProps {
      step: Props {
        state: state.clone(),
        step: step.clone(),
        locked: locked || viewing,
        navigation_locked: busy || viewing,
        viewing,
      },
      page,
    };
    return ctx.mount_keyed::<StepColumn>(&format!("step-column-{key}-{}", editing.epoch), props);
  }
  let summary = ctx.mount_keyed::<Summary>(
    &format!("pipeline-summary-{}", editing.epoch),
    SummaryProps {
      state: state.clone(),
      locked: locked || viewing,
    },
  );
  with_question(ctx, summary)
}

/// `content` under the save question while it is asked.
fn with_question(ctx: &mut Ctx, content: Element) -> Element {
  let Some(question) = super::actions::column_question(ctx) else {
    return content;
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(question)
    .child(content)
    .into()
}
