//! The step strip on an open step (`Step strip` in `Pipelines · v2` P2 `u2rYEH`, P13 `KRQ5J`): the whole flow in one
//! row, a chip with the number and name of each step, `›` between them, a hand where the run waits for the person, the
//! steps that run side by side in one frame (`04 ∥ 05`) and the Deliver step last. The open step is outlined in
//! `accent`; a chip opens its step in the same panel.
//!
//! A flow longer than the strip scrolls sideways: it opens with the open step in view, shows ‹ and › only on the side
//! that has more (each moves by one screen), the wheel and a swipe scroll it, a thin thumb under the chips shows where
//! it is, and ← and → open the previous and the next step, keeping it in view. The strip measures its own room from
//! its last layout.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{KeyboardEvent, ScrollEvent},
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Row, ScrollHorizontal, Stack, Text},
  core::{ElementRef, Ref, Signal},
  layout::{
    Alignment,
    layout_kind::{Justify, ScrollState},
  },
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::StepKey,
};

use super::pieces::{self, Chip, Piece};
use crate::{
  navigation::Navigation,
  routes::{PipelineView, PipelinesFilter, PipelinesView, ProjectPage, Route},
  screens::pipeline_hub::flow::number,
  theme::{
    ExtraColor, ExtraRadius, ExtraSpacing, FlowText, IconSize, StripSpacing, StripText, chip_radius, strip_scrollbar,
  },
  ui::{
    icon::{Icon, glyph, icon},
    pressable::{Focus, Press, pressable},
  },
};

/// The strip's height (`Step strip`) and a chip's (`Step 03`).
const STRIP_HEIGHT: f32 = 36.0;
const CHIP_HEIGHT: f32 = 24.0;
/// A parallel group's frame is 4 higher than a chip (`Parallel`).
const GROUP_HEIGHT: f32 = 28.0;
/// The gate's round hand (`Gate`).
const GATE: f32 = 16.0;
/// The panel ‹ or › stands in (`More left` `ZS10A`, `More right` `iZauO` in P13 `KRQ5J`): 36 wide and 2 less high than
/// the strip, with the 24 square button in it; the chips stop 4 short of it.
const PANEL: f32 = 36.0;
const BUTTON: f32 = 24.0;
const ARROW_ZONE: f32 = PANEL + 4.0;
/// A scroll of less than this is not one.
const EPSILON: f32 = 0.5;

#[derive(Clone)]
pub(super) struct Props {
  pub project: ProjectId,
  pub pipeline: ResourceId,
  pub pieces: Vec<Piece>,
  /// The open step.
  pub current: StepKey,
  /// What the Deliver chip sets before it opens the pipeline: its *A run delivers* row is shown.
  pub reveal_delivers: Signal<bool>,
}
/// The props are equal when they show the same: the signal is the panel's own.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (self.project, self.pipeline, &self.pieces, &self.current)
      == (other.project, other.pipeline, &other.pieces, &other.current)
  }
}
impl DevtoolsInspectable for Props {}

/// The strip of pipeline `props.pipeline`, which stays mounted while its steps are opened one after another.
pub(super) fn strip(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("pipeline-hub-step-strip-{}", props.pipeline);
  ctx.mount_keyed::<Strip>(&key, props)
}

/// Which sides of the strip have more chips than show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct More {
  before: bool,
  after: bool,
  /// The strip's room in whole pixels, which places the panel at its end: a strip that is resized draws it again.
  room: i32,
}
impl DevtoolsInspectable for More {}

/// What the strip measures of itself and keeps between layouts.
#[derive(Clone)]
struct Measured {
  scroll: ScrollState,
  viewport: ElementRef,
  content: ElementRef,
  current: ElementRef,
  more: Signal<More>,
  /// Whether the open step still has to be brought into view.
  reveal: Ref<bool>,
}

impl Measured {
  /// After a layout: the open step in view once its chip has been laid out, and which sides have more.
  fn settle(&self) {
    let (room, content) = (self.viewport.width(), self.content.width());
    if room <= 0.0 || content <= 0.0 {
      return;
    }
    if self.reveal.get() && self.current.is_attached() && self.current.width() > 0.0 {
      self.reveal.set(false);
      self.show_current(room, content);
    }
    let at = self.scroll.scroll_x();
    let more = More {
      before: at > EPSILON,
      after: at + room < content - EPSILON,
      room: room.round() as i32,
    };
    if self.more.get_untracked() != more {
      self.more.set(more);
    }
  }

  /// Leaves the open chip where it is when it shows whole clear of ‹ and ›, else puts it in the middle.
  fn show_current(&self, room: f32, content: f32) {
    let most = (content - room).max(0.0);
    if most <= 0.0 {
      return;
    }
    let (offset, width) = (self.current.x() - self.content.x(), self.current.width());
    let at = self.scroll.scroll_x();
    if offset >= at + ARROW_ZONE && offset + width <= at + room - ARROW_ZONE {
      return;
    }
    self
      .scroll
      .set_scroll_pending((offset - (room - width) / 2.0).clamp(0.0, most), 0.0);
  }

  /// ‹ or ›: one screen towards `direction` (-1 or 1), the chips that show between the two panels.
  fn page(&self, direction: f32) {
    let (room, content) = (self.viewport.width(), self.content.width());
    let most = (content - room).max(0.0);
    let screen = (room - 2.0 * ARROW_ZONE).max(0.0);
    let to = (self.scroll.scroll_x() + direction * screen).clamp(0.0, most);
    self.scroll.set_scroll_pending(to, 0.0);
  }
}

struct Strip {
  measured: Measured,
  /// The step last brought into view.
  followed: Ref<Option<StepKey>>,
}

impl Component for Strip {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      measured: Measured {
        scroll: ScrollState::new(),
        viewport: ctx.element_ref(),
        content: ctx.element_ref(),
        current: ctx.element_ref(),
        more: ctx.signal(More::default()),
        reveal: ctx.create_ref(true),
      },
      followed: ctx.create_ref(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    if self.followed.get().as_ref() != Some(&props.current) {
      self.followed.set(Some(props.current.clone()));
      self.measured.reveal.set(true);
    }
    for element in [&self.measured.viewport, &self.measured.content, &self.measured.current] {
      let measured = self.measured.clone();
      element.observe_rect(move |_| measured.settle());
    }
    let more = self.measured.more.get();
    let chips = self.chips(ctx, &props);
    let mut frame = Stack::new()
      .id("pipeline-hub-step-strip-frame")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(STRIP_HEIGHT))
      .child(self.scroller(ctx, chips));
    if more.before {
      frame = frame.child(self.edge(ctx, Edge::Before, more.room as f32));
    }
    if more.after {
      frame = frame.child(self.edge(ctx, Edge::After, more.room as f32));
    }
    Row::new()
      .id("pipeline-hub-step-strip")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(STRIP_HEIGHT))
      .flex_shrink(0.0)
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Sm)
      .clip()
      .on_key_down(keys(&props, Navigation::of(ctx)))
      .child(frame)
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Edge {
  Before,
  After,
}

impl Strip {
  /// The chips in a row: what the strip scrolls.
  fn chips(&self, ctx: &mut Ctx, props: &Props) -> Row {
    let navigation = Navigation::of(ctx);
    let mut row = Row::new()
      .id("pipeline-hub-step-chips")
      .ref_element(self.measured.content.clone())
      .padding(Padding::horizontal(ExtraSpacing::Snug.role()))
      .spacing(ExtraSpacing::Fine.role())
      .align_items(Alignment::Center)
      .height(Dimension::Px(STRIP_HEIGHT));
    for (index, piece) in props.pieces.iter().enumerate() {
      if index > 0 && !matches!(piece, Piece::Gate) {
        row = row.child(icon(Icon::ChevronRight, IconSize::Sm, PaletteColor::TextMuted));
      }
      row = match piece {
        Piece::Chip(chip) => row.child(self.chip(props, &navigation, chip)),
        Piece::Parallel(chips) => row.child(self.group(ctx, props, &navigation, chips)),
        Piece::Gate => row.child(gate(index)),
      };
    }
    row
  }

  fn scroller(&self, ctx: &Ctx, chips: Row) -> Element {
    let measured = self.measured.clone();
    ScrollHorizontal::new(chips)
      .with_scroll_state(self.measured.scroll.clone())
      .id("pipeline-hub-step-scroll")
      .ref_element(self.measured.viewport.clone())
      .scrollbar(strip_scrollbar(&ctx.theme().scrollbar()))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(STRIP_HEIGHT))
      .on_scroll(move |event: ScrollEvent| {
        // lurq's sideways scroller leaves a vertical wheel to the containers around it: the strip takes the stronger
        // delta, a wheel's or a swipe's.
        let (room, content) = (measured.viewport.width(), measured.content.width());
        let most = (content - room).max(0.0);
        if most <= 0.0 {
          return;
        }
        event.prevent_default();
        let delta = if event.delta_y.abs() > event.delta_x.abs() {
          event.delta_y
        } else {
          event.delta_x
        };
        measured
          .scroll
          .set_scroll_pending((measured.scroll.scroll_x() - delta).clamp(0.0, most), 0.0);
        measured.settle();
      })
      .into()
  }

  /// ‹ or ›: a panel in the strip's own fill and a hairline on its inner side, at the strip's end, over the chips cut
  /// there, with a 24 square button in it. It takes only its own place, so the chips beside it stay pressable.
  fn edge(&self, ctx: &Ctx, edge: Edge, room: f32) -> Element {
    let measured = self.measured.clone();
    let (direction, glyph, id, label) = match edge {
      Edge::Before => (
        -1.0,
        Icon::ChevronLeft,
        "pipeline-hub-step-strip-before",
        "pipeline_hub.step.strip.before",
      ),
      Edge::After => (
        1.0,
        Icon::ChevronRight,
        "pipeline-hub-step-strip-after",
        "pipeline_hub.step.strip.after",
      ),
    };
    let button = Focus::Keyboard
      .apply(pressable(move || {
        measured.page(direction);
        measured.settle();
      }))
      .id(id.to_owned())
      .describe("a11y_name", ctx.t(label))
      .width(Dimension::Px(BUTTON))
      .height(Dimension::Px(BUTTON))
      .justify(Justify::Center)
      .align_items(Alignment::Center)
      .rounded(chip_radius())
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary));
    let hairline = Border::inside(BorderSize::Sm, PaletteColor::Border);
    let (left, panel) = match edge {
      Edge::Before => (1.0, Row::new().border_right(hairline)),
      Edge::After => ((room - PANEL - 1.0).max(0.0), Row::new().border_left(hairline)),
    };
    panel
      .id(format!("{id}-panel"))
      .absolute(left, 1.0, Dimension::Px(PANEL), Dimension::Px(STRIP_HEIGHT - 2.0))
      .background(PaletteColor::SurfaceRaised)
      .justify(Justify::Center)
      .align_items(Alignment::Center)
      .child(button)
      .into()
  }

  fn chip(&self, props: &Props, navigation: &Navigation, chip: &Chip) -> Element {
    let press = match &chip.key {
      Some(key) => {
        let route = Route::Project(
          props.project,
          ProjectPage::Pipeline(props.pipeline, PipelineView::Step(key.clone())),
        );
        Press::Open(navigation.to(route))
      }
      // The Deliver step is the pipeline's own: it opens the pipeline, with its *A run delivers* row shown.
      None => {
        let route = Route::Project(
          props.project,
          ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(props.pipeline))),
        );
        let (navigation, reveal) = (navigation.clone(), props.reveal_delivers.clone());
        Press::run(move || {
          reveal.set(true);
          navigation.go(route.clone());
        })
      }
    };
    let current = chip.key.as_ref() == Some(&props.current);
    let id = chip.key.as_ref().map_or_else(
      || "pipeline-hub-step-chip-deliver".to_owned(),
      |key| format!("pipeline-hub-step-chip-{key}"),
    );
    let number = number(chip.number);
    let button = Focus::Keyboard
      .apply(press.button())
      .id(id)
      .describe("number", number.clone())
      .describe("state", if current { "current" } else { "other" })
      .describe("a11y_name", chip.name.to_string())
      .height(Dimension::Px(CHIP_HEIGHT))
      .flex_shrink(0.0)
      .padding(Padding::horizontal(StripSpacing::ChipSide.role()))
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .rounded(chip_radius())
      .child(Text::new(&number).variant(StripText::Number.role()).nowrap())
      .child(
        Text::new(&chip.name)
          .variant(if current { StripText::Current } else { StripText::Name }.role())
          .nowrap(),
      );
    if current {
      button
        .ref_element(self.measured.current.clone())
        .background(ExtraColor::Hover.role())
        .border_inside(BorderSize::Sm, PaletteColor::Accent)
        .into()
    } else {
      button
        .hovered_style(Style::new().background(ExtraColor::Hover.role()))
        .into()
    }
  }

  /// Steps that run side by side, in one frame with `∥` between them.
  fn group(&self, ctx: &Ctx, props: &Props, navigation: &Navigation, chips: &[Chip]) -> Element {
    let first = chips.first().map_or(0, |chip| chip.number);
    let mut row = Row::new()
      .id(format!("pipeline-hub-step-group-{first}"))
      .height(Dimension::Px(GROUP_HEIGHT))
      .flex_shrink(0.0)
      .padding(Padding::horizontal(ExtraSpacing::Tight.role()))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .rounded(chip_radius())
      .border_inside(BorderSize::Sm, PaletteColor::Border);
    for (index, chip) in chips.iter().enumerate() {
      if index > 0 {
        row = row.child(
          Text::new(&ctx.t("pipeline_hub.step.strip.parallel"))
            .variant(FlowText::Hint.role())
            .nowrap(),
        );
      }
      row = row.child(self.chip(props, navigation, chip));
    }
    row.into()
  }
}

/// The hand where the run waits for the person (`Gate`): round, in `warning` over its own faint fill.
fn gate(index: usize) -> Element {
  Row::new()
    .id(format!("pipeline-hub-step-gate-{index}"))
    .width(Dimension::Px(GATE))
    .height(Dimension::Px(GATE))
    .flex_shrink(0.0)
    .justify(Justify::Center)
    .align_items(Alignment::Center)
    .rounded(ExtraRadius::Full.role())
    .background(PaletteColor::WarningMuted)
    .child(Text::new(&glyph(Icon::Hand)).variant(StripText::GateHand.role()))
    .into()
}

/// ← and → open the previous and the next step, unless a field has the keys.
fn keys(props: &Props, navigation: Navigation) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  let (order, current) = (pieces::order(&props.pieces), props.current.clone());
  let (project, pipeline) = (props.project, props.pipeline);
  move |event: KeyboardEvent| {
    let plain = !(event.ctrl || event.alt || event.meta || event.shift);
    if event.default_prevented() || event.text_input_focused || event.composing || !plain {
      return;
    }
    let step: isize = match event.key.as_str() {
      "ArrowLeft" => -1,
      "ArrowRight" => 1,
      _ => return,
    };
    let Some(at) = order.iter().position(|key| *key == current) else {
      return;
    };
    let Some(next) = at.checked_add_signed(step).and_then(|to| order.get(to)) else {
      return;
    };
    event.prevent_default();
    navigation.go(Route::Project(
      project,
      ProjectPage::Pipeline(pipeline, PipelineView::Step(next.clone())),
    ));
  }
}
