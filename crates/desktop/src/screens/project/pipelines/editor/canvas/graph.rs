//! What the pipeline canvas (`C6Bjps`) draws, at 100 %: the description, the steps laid out by
//! dependency level with their connectors and annotations, and the failure route notes under
//! them (`pFKSd`). An empty draft shows the first-step slot and how to begin (`angoF`).
//! Selecting a card selects its step; a published version is read-only, so its cards do nothing.
//! Compared with the draft, each step the draft changes carries a tag on its card's top edge
//! (03.08 `Fb42P`).

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Stack, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::resource::{Pipeline, PipelineChange, Step, StepKey, changes};

use super::super::{
  EditorHandle,
  header::{add_step, new_step_name},
  starter_edit,
  step_card::step_card,
};
use crate::{
  pipelines::{
    data::Assignables,
    layout::{
      CARD_HEIGHT, CARD_WIDTH, CONTENT_MARGIN, Chevron, Direction, FIRST_ROW_Y, GraphLayout, NOTES_BELOW, ORIGIN_X,
      Segment, layout,
    },
    zoom::Size,
  },
  theme::{CONNECTOR_STROKE, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
};

/// Positions of the description, trigger label and empty-state explanation (`fSDu6`, `wRjZD`,
/// `r8A5GG`) and the empty explanation's width; the line pitch of the route notes, and the room
/// under the last one.
const NOTE_PITCH: f32 = 20.0;
const NOTES_BOTTOM: f32 = 16.0;
const DESCRIPTION_Y: f32 = 30.0;
const TRIGGER_Y: f32 = 76.0;
const EMPTY_EXPLANATION_Y: f32 = 252.0;
const EMPTY_EXPLANATION_WIDTH: f32 = 460.0;
/// The *Changed in draft* tag's height, centered on its card's top edge, and how far its right end
/// is from the card's (`Yjlao` in 03.08 `Fb42P`).
const TAG_HEIGHT: f32 = 20.0;
const TAG_INSET: f32 = 6.0;

/// A published version compared with what the editor holds: the steps on the canvas the editor
/// changes, and the draft's number; `None` when the editor holds the latest version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compared {
  draft: Option<u32>,
  steps: Vec<StepKey>,
}

impl Compared {
  pub fn new(version: &Pipeline, current: &Pipeline, draft: Option<u32>) -> Self {
    let steps = changes(version, current)
      .into_iter()
      .filter_map(|change| match change {
        PipelineChange::Step { key, .. } => Some(key),
        PipelineChange::Description => None,
      })
      .collect();
    Self { draft, steps }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphProps {
  pub steps: Arc<Vec<Step>>,
  pub description: Arc<str>,
  pub selected: Option<StepKey>,
  pub cycle: Option<Vec<StepKey>>,
  pub assignables: Assignables,
  /// A published version: nothing can be selected or added.
  pub read_only: bool,
  /// The version is compared with what the editor holds.
  pub compared: Option<Compared>,
}

impl lurq::app::component::DevtoolsInspectable for GraphProps {}

impl GraphProps {
  /// The dependency that closes the cycle, as step positions.
  fn closing(&self) -> Option<(usize, usize)> {
    let cycle = self.cycle.as_ref()?;
    let position = |key: &StepKey| self.steps.iter().position(|step| &step.key == key);
    Some((position(cycle.first()?)?, position(cycle.last()?)?))
  }

  fn layout(&self) -> GraphLayout {
    layout(&self.steps, self.closing())
  }

  /// The size of everything drawn at 100 %, with the margin around it.
  pub fn size(&self) -> Size {
    let graph = self.layout();
    let width = (graph.right + CONTENT_MARGIN).max(ORIGIN_X + EMPTY_EXPLANATION_WIDTH + CONTENT_MARGIN);
    let height = if self.steps.is_empty() {
      EMPTY_EXPLANATION_Y + CARD_HEIGHT + CONTENT_MARGIN
    } else {
      graph.bottom + NOTES_BELOW + NOTE_PITCH * routes(&self.steps).count() as f32 + NOTES_BOTTOM
    };
    Size { width, height }
  }
}

pub struct CanvasGraph {
  handle: EditorHandle,
}

impl Component for CanvasGraph {
  type Props = GraphProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      handle: ctx
        .use_context::<EditorHandle>()
        .expect("the editor provides EditorHandle"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<GraphProps>().clone();
    let size = props.size();
    let graph = props.layout();
    let content = Stack::new()
      .id("pipeline-canvas-graph")
      .width(Dimension::Px(size.width))
      .height(Dimension::Px(size.height))
      .child(description(ctx, &props.description))
      .child(
        Text::new(&ctx.t("pipelines.canvas.trigger"))
          .variant(ExtraTypography::CanvasLabel.role())
          .absolute_position(ORIGIN_X, TRIGGER_Y),
      );
    if props.steps.is_empty() {
      return content
        .child(self.first_step_slot(ctx))
        .child(self.empty_explanation(ctx));
    }
    let notes_top = graph.bottom + NOTES_BELOW;
    let notes = route_notes(ctx, &props.steps)
      .into_iter()
      .enumerate()
      .map(|(line, note)| {
        Text::new(&note)
          .variant(TypographyStyle::Caption)
          .nowrap()
          .absolute_position(ORIGIN_X, notes_top + NOTE_PITCH * line as f32)
      });
    self.graph(ctx, content, &props, &graph).with_children(notes)
  }
}

impl CanvasGraph {
  fn graph(&self, ctx: &Ctx, mut content: Stack, props: &GraphProps, graph: &GraphLayout) -> Stack {
    let cycle_segments = graph.cycle.iter().flat_map(|cycle| cycle.segments.iter());
    for segment in &graph.segments {
      content = content.child(line(segment, PaletteColor::TextMuted));
    }
    for segment in cycle_segments {
      content = content.child(line(segment, PaletteColor::TextPrimary));
    }
    for chevron in &graph.chevrons {
      content = content.child(chevron_mark(*chevron, PaletteColor::TextSecondary));
    }
    for &(x, y) in &graph.parallel_labels {
      content = content.child(
        Text::new(&ctx.t("pipelines.canvas.parallel"))
          .variant(ExtraTypography::CanvasLabel.role())
          .color(PaletteColor::TextSecondary)
          .absolute_position(x, y),
      );
    }
    for &(x, y, count) in &graph.join_labels {
      let text = if count == 2 {
        ctx.t("pipelines.canvas.bothMustPass")
      } else {
        ctx.t_args("pipelines.canvas.allMustPass", [("count", count.to_string().as_str())])
      };
      content = content.child(
        Text::new(&text)
          .variant(ExtraTypography::Footnote.role())
          .absolute_position(x, y),
      );
    }
    if let Some(cycle) = &graph.cycle {
      let (waiting, dependency) = (&props.steps[cycle.waiting].name, &props.steps[cycle.dependency].name);
      let label = ctx.t_args(
        "pipelines.canvas.cycle",
        [
          ("step", waiting.as_str().to_uppercase().as_str()),
          ("dependency", dependency.as_str().to_uppercase().as_str()),
        ],
      );
      content = content
        .child(chevron_mark(cycle.chevron, PaletteColor::TextPrimary))
        .child(
          Text::new(&label)
            .variant(ExtraTypography::CanvasLabel.role())
            .color(PaletteColor::TextPrimary)
            .nowrap()
            .absolute_position(cycle.label.0, cycle.label.1),
        );
    }
    let select = (!props.read_only).then_some(&self.handle);
    for card in &graph.cards {
      let step = &props.steps[card.index];
      let selected = props.selected.as_ref() == Some(&step.key);
      content = content.child(
        step_card(ctx, step, card.index + 1, selected, &props.assignables, select).absolute(
          card.x,
          card.y,
          Dimension::Px(CARD_WIDTH),
          Dimension::Px(CARD_HEIGHT),
        ),
      );
    }
    let Some(compared) = &props.compared else {
      return content;
    };
    let label = match compared.draft {
      Some(draft) => ctx.t_args(
        "pipelines.history.changedInDraft",
        [("draft", draft.to_string().as_str())],
      ),
      None => ctx.t("pipelines.history.changedInLatest"),
    };
    // Drawn after every card, so a tag stays over the card's edge.
    for card in &graph.cards {
      let key = &props.steps[card.index].key;
      if compared.steps.contains(key) {
        content = content.child(changed_tag(&label, key, (card.x, card.y)));
      }
    }
    content
  }

  /// Add first step (`mYTsP`).
  fn first_step_slot(&self, ctx: &Ctx) -> Element {
    let handle = self.handle.clone();
    let name = new_step_name(ctx, &[]);
    let content = Column::new()
      .flex(1.0)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Plus, IconSize::Notice, PaletteColor::TextPrimary))
      .child(Text::new(&ctx.t("pipelines.canvas.addFirstStep")).variant(TypographyStyle::Label))
      .child(Text::new(&ctx.t("pipelines.canvas.addFirstStepDetail")).variant(TypographyStyle::Caption));
    let slot = surface()
      .id("pipeline-add-first-step")
      .absolute(
        ORIGIN_X,
        FIRST_ROW_Y,
        Dimension::Px(CARD_WIDTH),
        Dimension::Px(CARD_HEIGHT),
      )
      .align_items(Alignment::Center)
      .border(Border::inside(BorderSize::Sm, PaletteColor::TextSecondary))
      .rounded(RadiusSize::Lg)
      .child(content);
    pressable(slot, move || add_step(&handle, name.clone()))
      .hovered_style(Style::new().background(PaletteColor::SurfaceRaised))
      .into()
  }

  /// No steps yet (`r8A5GG`).
  fn empty_explanation(&self, ctx: &Ctx) -> Element {
    let (add, replace) = (self.handle.clone(), self.handle.clone());
    let name = new_step_name(ctx, &[]);
    let starter = starter_edit(ctx, &self.handle);
    let actions = Row::new()
      .padding_top(SpacingSize::Xs)
      .spacing(ExtraSpacing::Stack.role())
      .child(
        ActionButton::new(ctx.t("pipelines.editor.addStep"))
          .primary()
          .icon(Icon::Plus)
          .id("pipeline-empty-add-step")
          .on_click(move || add_step(&add, name.clone()))
          .render(ctx),
      )
      .child(
        ActionButton::new(ctx.t("pipelines.canvas.useStarter"))
          .icon(Icon::LayoutTemplate)
          .id("pipeline-use-starter")
          .on_click(move || {
            let starter = starter.clone();
            replace.replace(move |edit| {
              let description = edit.description().to_owned();
              *edit = starter;
              edit.set_description(&description);
            });
          })
          .render(ctx),
      );
    Column::new()
      .width(Dimension::Px(EMPTY_EXPLANATION_WIDTH))
      .absolute_position(ORIGIN_X, EMPTY_EXPLANATION_Y)
      .spacing(ExtraSpacing::Stack.role())
      .child(Text::new(&ctx.t("pipelines.canvas.noStepsTitle")).variant(ExtraTypography::SectionHeading.role()))
      .child(Text::new(&ctx.t("pipelines.canvas.noStepsDetail")).variant(ExtraTypography::Explanation.role()))
      .child(actions)
      .into()
  }
}

fn description(ctx: &Ctx, text: &str) -> Element {
  let (text, color) = if text.trim().is_empty() {
    (ctx.t("pipelines.canvas.noDescription"), PaletteColor::TextSecondary)
  } else {
    (text.into(), PaletteColor::TextSecondary)
  };
  Text::new(&text)
    .variant(TypographyStyle::Description)
    .color(color)
    .absolute_position(ORIGIN_X, DESCRIPTION_Y)
    .into()
}

/// *Changed in draft v5* (`Yjlao`): the compact status label's look with the field stroke, its right
/// end near the card's, centered on the card's top edge. It takes no pointer: the card under it is
/// read-only.
fn changed_tag(label: &str, key: &StepKey, (x, y): (f32, f32)) -> Element {
  let tag = Row::new()
    .id(format!("step-card-changed-{key}"))
    .height(Dimension::Px(TAG_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::AccentMuted)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Sm)
    .child(icon(Icon::Pencil, IconSize::Tag, PaletteColor::TextPrimary))
    .child(
      Text::new(label)
        .variant(ExtraTypography::CompactStatusLabel.role())
        .nowrap(),
    );
  Row::new()
    .absolute(
      x,
      y - TAG_HEIGHT / 2.0,
      Dimension::Px(CARD_WIDTH - TAG_INSET),
      Dimension::Px(TAG_HEIGHT),
    )
    .justify(Justify::End)
    .pointer_events_none()
    .child(tag)
    .into()
}

/// A connector piece as a thin filled rectangle.
fn line(segment: &Segment, color: PaletteColor) -> Element {
  let half = CONNECTOR_STROKE / 2.0;
  let (x, y, width, height) = if (segment.y1 - segment.y2).abs() < f32::EPSILON {
    (segment.x1, segment.y1 - half, segment.x2 - segment.x1, CONNECTOR_STROKE)
  } else {
    (segment.x1 - half, segment.y1, CONNECTOR_STROKE, segment.y2 - segment.y1)
  };
  Row::new()
    .absolute(x, y, Dimension::Px(width), Dimension::Px(height))
    .background(color)
    .into()
}

fn chevron_mark(chevron: Chevron, color: PaletteColor) -> Element {
  let glyph = match chevron.direction {
    Direction::Right => Icon::ChevronRight,
    Direction::Down => Icon::ChevronDown,
  };
  icon(glyph, IconSize::Dismiss, color)
    .absolute_position(chevron.x, chevron.y)
    .into()
}

/// Each step with a failure route, with the step it returns to.
fn routes(steps: &[Step]) -> impl Iterator<Item = (&Step, &Step, u32)> {
  steps.iter().filter_map(|step| {
    let route = step.on_failure.as_ref()?;
    let target = steps.iter().find(|candidate| candidate.key == route.return_to)?;
    Some((step, target, route.max_retries.get()))
  })
}

/// "If Review does not pass, return to Implement. Max 2 retries." for each failure route.
fn route_notes(ctx: &Ctx, steps: &[Step]) -> Vec<Arc<str>> {
  routes(steps)
    .map(|(step, target, retries)| {
      ctx.t_args(
        "pipelines.canvas.routeNote",
        [
          ("step", step.name.as_str()),
          ("target", target.name.as_str()),
          ("count", retries.to_string().as_str()),
        ],
      )
    })
    .collect()
}
