//! Pipeline canvas (`C6Bjps`): the description, the steps laid out by dependency level with
//! their connectors and annotations, the failure route notes, and the zoom controls
//! (`iarCw`, unavailable). An empty pipeline shows the first-step slot and how to begin (`angoF`). Selecting
//! a card selects its step.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollBoth, Spacer, Stack, Text},
  layout::{Alignment, StackAlignment, layout_kind::Justify},
  node::{CursorIcon, Element, HitTestBehavior, Style, border::Border, dimension::Dimension},
};
use orchester_core::resource::{Step, StepKey, VersionNumber};

use super::{
  EditorHandle, Selection,
  header::{add_step, new_step_name},
  starter_edit,
  step_card::step_card,
};
use crate::{
  pipelines::{
    data::Assignables,
    editor::EditorState,
    layout::{
      CARD_HEIGHT, CARD_WIDTH, CONTENT_MARGIN, Chevron, Direction, FIRST_ROW_Y, GraphLayout, NOTES_BELOW, ORIGIN_X,
      Segment, layout,
    },
  },
  theme::{CONNECTOR_STROKE, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    notice::Notice,
    pressable::{Ring, pressable, surface},
  },
};

/// Positions of the description, trigger label and empty-state explanation (`fSDu6`, `wRjZD`,
/// `r8A5GG`) and the empty explanation's width; the line pitch of the route notes, and the room
/// the zoom controls take at the bottom (`iarCw`: 32 high, 24 from the edge).
const NOTE_PITCH: f32 = 20.0;
const OVERLAY_ROOM: f32 = 56.0;
const DESCRIPTION_Y: f32 = 30.0;
const TRIGGER_Y: f32 = 76.0;
const EMPTY_EXPLANATION_Y: f32 = 252.0;
const EMPTY_EXPLANATION_WIDTH: f32 = 460.0;
/// Zoom controls (`iarCw`) and the published notice (`O8ivsy`).
const ZOOM_WIDTH: f32 = 152.0;
const ZOOM_HEIGHT: f32 = 32.0;
const NOTICE_WIDTH: f32 = 396.0;
/// The zoom the canvas is always shown at.
const FULL_SIZE: &str = "100";

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasProps {
  steps: Arc<Vec<Step>>,
  description: Arc<str>,
  selected: Option<StepKey>,
  cycle: Option<Vec<StepKey>>,
  assignables: Assignables,
  just_published: Option<VersionNumber>,
}

impl lurq::app::component::DevtoolsInspectable for CanvasProps {}

impl CanvasProps {
  pub fn new(state: &EditorState, selection: &Selection) -> Self {
    Self {
      steps: Arc::new(state.edit().steps().to_vec()),
      description: state.edit().description().into(),
      selected: match selection {
        Selection::Step(key) => Some(key.clone()),
        Selection::Pipeline => None,
      },
      cycle: state.edit().cycle(),
      assignables: state.assignables().clone(),
      just_published: state
        .latest()
        .filter(|_| state.just_published())
        .map(|latest| latest.number),
    }
  }

  /// The dependency that closes the cycle, as step positions.
  fn closing(&self) -> Option<(usize, usize)> {
    let cycle = self.cycle.as_ref()?;
    let position = |key: &StepKey| self.steps.iter().position(|step| &step.key == key);
    Some((position(cycle.first()?)?, position(cycle.last()?)?))
  }
}

pub struct Canvas {
  handle: EditorHandle,
}

impl Component for Canvas {
  type Props = CanvasProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      handle: ctx
        .use_context::<EditorHandle>()
        .expect("the editor provides EditorHandle"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CanvasProps>().clone();
    let graph = layout(&props.steps, props.closing());
    let notes = route_notes(ctx, &props.steps);
    let width = (graph.right + CONTENT_MARGIN).max(ORIGIN_X + EMPTY_EXPLANATION_WIDTH + CONTENT_MARGIN);
    // The route notes sit under the graph (`pFKSd`) and the zoom controls float over the
    // canvas's bottom-left corner (`iarCw`), so the content ends with room for them.
    let notes_top = graph.bottom + NOTES_BELOW;
    let height = if props.steps.is_empty() {
      EMPTY_EXPLANATION_Y + CARD_HEIGHT + CONTENT_MARGIN
    } else {
      notes_top + NOTE_PITCH * notes.len() as f32 + OVERLAY_ROOM
    };
    let mut content = Stack::new()
      .width(Dimension::Px(width))
      .height(Dimension::Px(height))
      .child(description(ctx, &props.description))
      .child(
        Text::new(&ctx.t("pipelines.canvas.trigger"))
          .variant(ExtraTypography::CanvasLabel.role())
          .absolute_position(ORIGIN_X, TRIGGER_Y),
      );
    content = if props.steps.is_empty() {
      content
        .child(self.first_step_slot(ctx))
        .child(self.empty_explanation(ctx))
    } else {
      let notes = notes.iter().enumerate().map(|(line, note)| {
        Text::new(note)
          .variant(TypographyStyle::Caption)
          .nowrap()
          .absolute_position(ORIGIN_X, notes_top + NOTE_PITCH * line as f32)
      });
      self.graph(ctx, content, &props, &graph).with_children(notes)
    };
    Stack::with(
      StackAlignment::BottomStart,
      [
        ScrollBoth::new(content)
          .width(Dimension::Pct(100.0))
          .height(Dimension::Pct(100.0))
          .into(),
        self.overlay(ctx, props.just_published),
      ],
    )
    .id("pipeline-canvas")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
  }
}

impl Canvas {
  fn graph(&self, ctx: &Ctx, mut content: Stack, props: &CanvasProps, graph: &GraphLayout) -> Stack {
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
    for card in &graph.cards {
      let step = &props.steps[card.index];
      let selected = props.selected.as_ref() == Some(&step.key);
      content = content.child(
        step_card(ctx, step, card.index + 1, selected, &props.assignables, &self.handle).absolute(
          card.x,
          card.y,
          Dimension::Px(CARD_WIDTH),
          Dimension::Px(CARD_HEIGHT),
        ),
      );
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
    pressable(slot, Ring::Outer, move || add_step(&handle, name.clone()))
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

  /// The zoom controls (`iarCw`) and, right after publishing, the published notice
  /// (`O8ivsy`), over the canvas's bottom edge. The row itself lets clicks and scrolling
  /// through to the canvas.
  fn overlay(&self, ctx: &Ctx, published: Option<VersionNumber>) -> Element {
    let control = |glyph: Icon| icon(glyph, IconSize::Marker, PaletteColor::TextMuted);
    let controls = Row::new()
      .id("pipeline-zoom")
      .describe("state", "unavailable")
      .width(Dimension::Px(ZOOM_WIDTH))
      .height(Dimension::Px(ZOOM_HEIGHT))
      .padding_horizontal(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .justify(Justify::SpaceBetween)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Sm)
      .cursor(CursorIcon::NotAllowed)
      .child(control(Icon::Minus))
      .child(control(Icon::Plus))
      .child(
        Text::new(&ctx.t_args("pipelines.canvas.zoom", [("percent", FULL_SIZE)]))
          .variant(TypographyStyle::Mono)
          .color(PaletteColor::TextPrimary),
      )
      .child(control(Icon::Maximize))
      .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted));
    let mut bar = Row::new()
      .width(Dimension::Pct(100.0))
      .hit_test(HitTestBehavior::ContentOnly)
      .padding_horizontal(ExtraSpacing::Indent.role())
      .padding_bottom(SpacingSize::Lg)
      .spacing(SpacingSize::Md)
      .align_items(Alignment::End)
      .child(controls)
      .child(Spacer::new().flex(1.0));
    if let Some(version) = published {
      let version = version.to_string();
      bar = bar.child(
        Column::new().width(Dimension::Px(NOTICE_WIDTH)).child(
          Notice::new(
            Icon::CircleCheck,
            ctx.t_args("pipelines.published.title", [("version", version.as_str())]),
            ctx.t_args("pipelines.published.detail", [("version", version.as_str())]),
          )
          .id("pipeline-published-notice"),
        ),
      );
    }
    bar.into()
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

/// "If Review does not pass, return to Implement. Max 2 retries." for each failure route.
fn route_notes(ctx: &Ctx, steps: &[Step]) -> Vec<Arc<str>> {
  steps
    .iter()
    .filter_map(|step| {
      let route = step.on_failure.as_ref()?;
      let target = steps.iter().find(|candidate| candidate.key == route.return_to)?;
      Some(ctx.t_args(
        "pipelines.canvas.routeNote",
        [
          ("step", step.name.as_str()),
          ("target", target.name.as_str()),
          ("count", route.max_retries.get().to_string().as_str()),
        ],
      ))
    })
    .collect()
}
