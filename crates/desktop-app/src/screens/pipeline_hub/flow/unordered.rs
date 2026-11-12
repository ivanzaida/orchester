//! Steps with no order to draw (`Notice · no order` in `Pipelines · v2` P16c `YHAyx`): steps that wait for each other,
//! for a step that is not there, or break another rule that leaves them no flow. The hub never makes such steps (a
//! step's *Waits for* offers only steps that do not wait for it) and a version cannot hold them, so the notice is the
//! defence for an edit that has them. It says why in the warning tone and offers *Make it a line*: each step then waits
//! for the step listed before it, which replaces every dependency in the edit, and the flow is drawn at once; *Save*
//! stores it and *Cancel* puts the old steps back. Steps a line does not put right (two steps with one key) keep the
//! notice without the action.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::resource::{FlowError, PipelineEdit, PipelineError};
use orchester_workspace::pipelines::flow::FlowRead;

use super::words::number;
use crate::{
  screens::pipeline_hub::edit::Edit,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// What *Make it a line* acts on: the panel's edit, as it stands now.
pub(super) struct Line<'a> {
  pub edit: &'a Edit,
  pub current: &'a PipelineEdit,
  /// A save is under way or asks: nothing is changed now.
  pub locked: bool,
}

/// The notice for `read`, whose flow failed with `error`. Without `line` (a version viewed read-only) it only says that
/// the steps have no order.
pub(super) fn notice(ctx: &Ctx, read: &FlowRead, error: &FlowError, line: Option<Line<'_>>) -> Element {
  let notice = Notice::new(
    "pipeline-hub-flow-undrawable",
    NoticeTone::Warning,
    ctx.t("pipeline_hub.flow.broken.title"),
  );
  let Some(line) = line else {
    return notice.into();
  };
  if !line.current.line_fixes() {
    return notice.body(ctx.t("pipeline_hub.flow.broken.noLine")).into();
  }
  let body = match error {
    FlowError::Broken(PipelineError::Cycle(_)) => cycle_words(ctx, read, line.current),
    FlowError::Broken(_) => None,
  };
  let action = make_line(ctx, "pipeline-hub-flow-undrawable-line", line.edit);
  notice
    .body(body.unwrap_or_else(|| ctx.t("pipeline_hub.flow.broken.detail")))
    .action(if line.locked { action.disabled() } else { action })
    .into()
}

/// *Make it a line*, which gives the steps of `edit` one line.
pub(super) fn make_line(ctx: &Ctx, id: &str, edit: &Edit) -> ActionButton {
  let edit = edit.clone();
  ActionButton::new(
    id.to_owned(),
    ButtonKind::Secondary,
    ctx.t("pipeline_hub.flow.broken.line"),
  )
  .icon(Icon::Plus)
  .on_click(move || {
    edit.change(|held| {
      held.make_line();
    });
  })
}

/// *05 Review and 06 Write notes wait for each other, so neither can start …*, the steps of the circle by the numbers
/// their cards have in the order of the steps; `None` when the circle cannot be named.
fn cycle_words(ctx: &Ctx, read: &FlowRead, current: &PipelineEdit) -> Option<Arc<str>> {
  let circle = current.cycle()?;
  let mut cards: Vec<_> = circle
    .iter()
    .filter_map(|key| read.unplaced.iter().find(|card| card.key == *key))
    .collect();
  cards.sort_by_key(|card| card.number);
  let named: Vec<String> = cards
    .into_iter()
    .map(|card| {
      ctx
        .t_args(
          "pipeline_hub.flow.move.step",
          [("number", number(card.number)), ("name", card.name.to_string())],
        )
        .to_string()
    })
    .collect();
  match named.as_slice() {
    [first, second] => Some(ctx.t_args(
      "pipeline_hub.flow.broken.cycleTwo",
      [("first", first.as_str()), ("second", second.as_str())],
    )),
    [head @ .., last] if !head.is_empty() => {
      let steps = ctx.t_args(
        "pipeline_hub.flow.move.several",
        [("head", head.join(", ")), ("last", last.clone())],
      );
      Some(ctx.t_args("pipeline_hub.flow.broken.cycle", [("steps", steps.to_string())]))
    }
    _ => None,
  }
}
