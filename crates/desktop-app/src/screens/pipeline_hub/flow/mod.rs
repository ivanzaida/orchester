//! The steps of a pipeline drawn as the flow they run in (`Flow` `CX0tV` in `Steps` `uSoll`, `Pipelines · v2` P1; the
//! amber card of P8): a vertical line with a node per step from *ON TASK START* to where the run ends, a card per step,
//! an approval gate on the line after its step, steps that run side by side in one *RUN IN PARALLEL* frame, and where a
//! failure sends the run back on its card. Nothing of the layout is stored: it is read from the steps' dependencies
//! (`orchester_workspace::pipelines::flow`). A pipeline with no steps, or whose steps one line cannot show, keeps the
//! start and the end and says so between them ([`note_row`]). A card opens its step through today's step page until the
//! step opens in the panel (PV2-07).

mod card;
mod fix;
mod parts;
mod rail;
mod words;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Button, Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ResourceId, resource::FlowError};
use orchester_workspace::pipelines::flow::{FlowRead, PipelineFlow, Stage, StepCard};

use self::{
  card::{Opening, Place},
  fix::Fix,
  rail::Node,
};
use crate::{
  navigation::{Navigation, link::Open},
  project::ShownProject,
  routes::{PipelineView, ProjectPage, Route},
  theme::HubText,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
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
  fn open(&self, view: PipelineView) -> Open {
    self.navigation.to(Route::Project(
      self.project.open.project.id(),
      ProjectPage::Pipeline(self.pipeline, view),
    ))
  }

  fn button(&self, view: PipelineView) -> Button {
    Press::Open(self.open(view)).button()
  }

  fn fix(&self, fix: Fix) -> Button {
    let route = Route::Project(self.project.open.project.id(), fix.page());
    Press::Open(self.navigation.to(route)).button()
  }
}

/// *Edit steps*: the editor the pipeline has today, where steps are added, moved and removed until the panel does.
pub(super) fn edit_steps(ctx: &Ctx, id: &str, open: Open) -> ActionButton {
  ActionButton::new(id.to_owned(), ButtonKind::Ghost, ctx.t("pipeline_hub.panel.openSteps"))
    .icon(Icon::Pencil)
    .opens(open)
}

/// The flow of pipeline `pipeline`: from the start to the end, with the steps, or the note that says why there are none
/// to draw, between them.
pub(super) fn flow(
  ctx: &Ctx,
  (project, navigation): (&ShownProject, &Navigation),
  pipeline: ResourceId,
  read: &FlowRead,
) -> Element {
  let opens = Opens {
    navigation,
    project,
    pipeline,
  };
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
  match &read.flow {
    Ok(flow) => rows.extend(drawing(ctx, &opens, flow)),
    Err(error) => rows.push(note_row(ctx, undrawable(ctx, &opens, error))),
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
    rows.push(note_row(ctx, no_steps(ctx, opens)));
  }
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
    rows.push(rail::row(
      ctx,
      "pipeline-hub-flow-row-deliver",
      &Node::Step(deliver.number),
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
    let fix = Fix::of(&step.runs).map(|fix| (opens.fix(fix), fix.words(ctx)));
    card::step(
      ctx,
      step,
      Opening {
        open: opens.button(PipelineView::Step(step.key.clone())),
        fix,
        place,
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

/// What stands on the line between the start and the end when there is nothing to draw there.
fn note_row(ctx: &Ctx, note: Element) -> Element {
  rail::row(ctx, "pipeline-hub-flow-row-note", &Node::Line, (note, SpacingSize::Lg))
}

/// A draft with no steps: a muted line and *Edit steps*, until steps are added in place.
fn no_steps(ctx: &Ctx, opens: &Opens<'_>) -> Element {
  Row::new()
    .id("pipeline-hub-flow-empty")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("pipeline_hub.flow.noSteps"))
        .id("pipeline-hub-flow-empty-words")
        .variant(HubText::PanelHint.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(edit_steps(
      ctx,
      "pipeline-hub-flow-empty-edit",
      opens.open(PipelineView::default()),
    ))
    .into()
}

/// Steps one line cannot show: a step that waits for only some of what runs before it, or steps that wait for each
/// other. They are not guessed at; the page that edits steps, which the notice opens, shows their dependencies.
fn undrawable(ctx: &Ctx, opens: &Opens<'_>, error: &FlowError) -> Element {
  let (title, body) = match error {
    FlowError::Branches(_) => ("pipeline_hub.flow.branches.title", "pipeline_hub.flow.branches.detail"),
    FlowError::Broken(_) => ("pipeline_hub.flow.broken.title", "pipeline_hub.flow.broken.detail"),
  };
  Notice::new("pipeline-hub-flow-undrawable", NoticeTone::Warning, ctx.t(title))
    .flat()
    .body(ctx.t(body))
    .action(edit_steps(
      ctx,
      "pipeline-hub-flow-undrawable-edit",
      opens.open(PipelineView::default()),
    ))
    .into()
}
