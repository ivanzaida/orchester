//! A published version's steps drawn read-only (a version viewed in `Pipelines · v2`, PV2-11): the line with its cards,
//! gates, parallel frames and Deliver card, none of it a control. A card is not pressed, has no `›`, takes no hover and
//! is not a Tab stop, and no handle, ⋯, key or *Add step* is drawn: the editable flow is [`super::view`].

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::pipelines::flow::{FlowRead, PipelineFlow, Stage, StepCard};

use super::{
  Opens,
  card::{self, Look, Opening, Place},
  fix::Fix,
  parts,
  rail::{self, Node},
  view::{no_steps, note_row, start_row},
  words,
};
use crate::routes::PipelineView;

/// The flow of `read`, from the start to the end.
pub(super) fn drawn(ctx: &Ctx, opens: &Opens<'_>, read: &FlowRead) -> Element {
  let mut rows: Vec<Element> = vec![start_row(ctx)];
  match &read.flow {
    Ok(flow) => rows.extend(drawing(ctx, opens, flow)),
    Err(error) => rows.push(note_row(ctx, super::unordered::notice(ctx, read, error, None))),
  }
  let (label, hint) = words::end(ctx, read.ending);
  rows.push(rail::row(
    ctx,
    "pipeline-hub-flow-row-end",
    &Node::End,
    (parts::end(&label, &hint).into(), SpacingSize::Sm),
  ));
  Column::new()
    .id("pipeline-hub-flow")
    .width(Dimension::Pct(100.0))
    .with_children(rows)
    .into()
}

/// The rows between the start and the end: the stages with their gates, then the Deliver card.
fn drawing(ctx: &Ctx, opens: &Opens<'_>, flow: &PipelineFlow) -> Vec<Element> {
  let mut rows = Vec::new();
  if flow.stages.is_empty() {
    rows.push(note_row(ctx, no_steps(ctx)));
  }
  for (index, stage) in flow.stages.iter().enumerate() {
    rows.push(stage_row(ctx, opens, stage));
    for (gate_index, gate) in stage.gates.iter().enumerate() {
      let id = format!("pipeline-hub-flow-row-gate-{index}-{gate_index}");
      let line = parts::gate(
        index * 100 + gate_index,
        &ctx.t("pipeline_hub.flow.gate"),
        &words::gate(ctx, gate, flow.deliver.as_ref().map(|deliver| deliver.number)),
        gate,
      );
      rows.push(rail::row(ctx, &id, &Node::Gate, (line.into(), SpacingSize::Lg)));
    }
  }
  if let Some(deliver) = &flow.deliver {
    rows.push(rail::row(
      ctx,
      "pipeline-hub-flow-row-deliver",
      &Node::Deliver,
      (
        card::deliver(ctx, deliver, opens.button(PipelineView::Deliver)),
        SpacingSize::Sm,
      ),
    ));
  }
  rows
}

/// A stage: one step on the line with its own node, or steps that run side by side in their frame.
fn stage_row(ctx: &Ctx, opens: &Opens<'_>, stage: &Stage) -> Element {
  let sm = SpacingSize::Sm;
  let card = |step: &StepCard, place: Place| {
    let fix = Fix::of_card(step).and_then(|fix| opens.fix(fix).map(|button| (button, fix.words(ctx))));
    card::step(
      ctx,
      step,
      Opening {
        open: opens.button(PipelineView::Step(step.key.clone())),
        fix,
        place,
        handle: None,
        more: None,
        look: Look::Plain,
      },
    )
  };
  let [only] = stage.cards.as_slice() else {
    let first = stage.cards.first().map_or(0, |step| step.number);
    let (label, hint) = (
      ctx.t("pipeline_hub.flow.parallel"),
      ctx.t(if stage.cards.len() > 2 {
        "pipeline_hub.flow.parallelAll"
      } else {
        "pipeline_hub.flow.parallelBoth"
      }),
    );
    let group = parts::parallel(
      &format!("pipeline-hub-flow-parallel-{first}"),
      (&label, &hint),
      stage.cards.iter().map(|step| card(step, Place::InGroup)).collect(),
    );
    return rail::row(
      ctx,
      &format!("pipeline-hub-flow-row-{first}"),
      &Node::Parallel,
      (group, sm),
    );
  };
  let node = if only.runs.needs_you() {
    Node::NotReady(only.number)
  } else {
    Node::Step(only.number)
  };
  rail::row(
    ctx,
    &format!("pipeline-hub-flow-row-{}", only.number),
    &node,
    (card(only, Place::OnTheLine), sm),
  )
}
