//! The steps of a pipeline drawn as the flow they run in (`Flow` `CX0tV` in `Steps` `uSoll`, `Pipelines · v2` P1; the
//! amber card of P8): a vertical line with a node per step from *ON TASK START* to where the run ends, a card per step,
//! an approval gate on the line after its step, steps that run side by side in one *RUN IN PARALLEL* frame, and where a
//! failure sends the run back on its card. Nothing of the layout is stored: it is read from the steps' dependencies
//! (`orchester_workspace::pipelines::flow`), and steps whose dependencies one line cannot show are not drawn but said
//! so ([`undrawable`]). A card opens its step through today's step page until the step opens in the panel (PV2-07).

mod card;
mod parts;
mod rail;
mod words;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Button, Column},
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ResourceId, resource::FlowError};
use orchester_workspace::pipelines::flow::{PipelineFlow, Stage};

use self::{card::Place, rail::Node};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{PipelineView, ProjectPage, Route},
  ui::{
    notice::{Notice, NoticeTone},
    pressable::Press,
  },
};

/// How the flow reaches the routes of the steps.
struct Opens<'a> {
  navigation: &'a Navigation,
  project: &'a ShownProject,
  pipeline: ResourceId,
}

impl Opens<'_> {
  fn route(&self, view: PipelineView) -> Route {
    Route::Project(
      self.project.open.project.id(),
      ProjectPage::Pipeline(self.pipeline, view),
    )
  }

  fn button(&self, view: PipelineView) -> Button {
    Press::Open(self.navigation.to(self.route(view))).button()
  }
}

/// The flow of pipeline `pipeline`, or why its steps are not drawn.
pub(super) fn flow(
  ctx: &Ctx,
  (project, navigation): (&ShownProject, &Navigation),
  pipeline: ResourceId,
  drawn: &Result<PipelineFlow, FlowError>,
) -> Element {
  let opens = Opens {
    navigation,
    project,
    pipeline,
  };
  match drawn {
    Ok(flow) => drawing(ctx, &opens, flow),
    Err(error) => undrawable(ctx, error),
  }
}

fn drawing(ctx: &Ctx, opens: &Opens<'_>, flow: &PipelineFlow) -> Element {
  let sm = SpacingSize::Sm;
  let mut rows: Vec<Element> = vec![rail::row(
    ctx,
    "pipeline-hub-flow-row-start",
    &Node::Start,
    (
      parts::start(
        &ctx.t("pipeline_hub.flow.onStart"),
        &ctx.t("pipeline_hub.flow.onStartHint"),
      )
      .into(),
      SpacingSize::Lg,
    ),
  )];
  for (index, stage) in flow.stages.iter().enumerate() {
    rows.push(stage_row(ctx, opens, stage));
    for (gate_index, gate) in stage.gates.iter().enumerate() {
      let id = format!("pipeline-hub-flow-row-gate-{index}-{gate_index}");
      let line = parts::gate(
        index * 100 + gate_index,
        &ctx.t("pipeline_hub.flow.gate"),
        &words::gate(ctx, gate),
        gate,
      );
      rows.push(rail::row(ctx, &id, &Node::Gate, (line.into(), SpacingSize::Lg)));
    }
  }
  if let Some(deliver) = &flow.deliver {
    let card = card::deliver(ctx, deliver, opens.button(PipelineView::Deliver));
    rows.push(rail::row(
      ctx,
      "pipeline-hub-flow-row-deliver",
      &Node::Step(deliver.number),
      (card, sm),
    ));
  }
  let (label, hint) = words::end(ctx, flow);
  rows.push(rail::row(
    ctx,
    "pipeline-hub-flow-row-end",
    &Node::End,
    (parts::end(&label, &hint).into(), sm),
  ));
  Column::new()
    .id("pipeline-hub-flow")
    .width(Dimension::Pct(100.0))
    .with_children(rows)
    .into()
}

/// A stage: one step on the line with its own node, or steps that run side by side in their frame.
fn stage_row(ctx: &Ctx, opens: &Opens<'_>, stage: &Stage) -> Element {
  let sm = SpacingSize::Sm;
  let card = |step: &orchester_workspace::pipelines::flow::StepCard, place: Place| {
    card::step(ctx, step, (opens.button(PipelineView::Step(step.key.clone())), place))
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

/// Steps one line cannot show: a step that waits for only some of what runs before it, or steps that wait for each
/// other. They are not guessed at; the page that edits steps, which the head opens, shows their dependencies.
fn undrawable(ctx: &Ctx, error: &FlowError) -> Element {
  let (title, body) = match error {
    FlowError::Branches(_) => ("pipeline_hub.flow.branches.title", "pipeline_hub.flow.branches.detail"),
    FlowError::Broken(_) => ("pipeline_hub.flow.broken.title", "pipeline_hub.flow.broken.detail"),
  };
  Notice::new("pipeline-hub-flow-undrawable", NoticeTone::Warning, ctx.t(title))
    .flat()
    .body(ctx.t(body))
    .into()
}
