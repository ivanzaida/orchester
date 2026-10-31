//! The frame of a start page's step (`Startup · redesign` `qzzYw`; `Page` `qlOo2` in *Join a team* `B914L`): the
//! breadcrumb back to the start and the step line at the top, the step's title and lead, its body and a note under
//! it, in a column 960 wide centered in the content panel, which scrolls where the window is too low. The start
//! page's other steps (ORC-293) take it over.
//!
//! The breadcrumb is the start page's own: its crumbs are 12.5 and the step's own crumb `text_primary`, where the
//! kit's ([`crate::ui::breadcrumb`]) draws 12 and `text_secondary`.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  navigation::Navigation,
  routes::Route,
  theme::{ExtraRadius, ExtraSpacing, IconSize, StartupSpacing, StartupText},
  ui::{
    icon::{Icon, icon},
    pressable::opening,
  },
};

/// The column the step's parts are in (`Inner` `MOeLZ`).
const INNER_WIDTH: f32 = 960.0;
/// A step's number in its ring (`Number` `P6t15`).
const STEP_NUMBER: f32 = 18.0;
/// The line between two steps (`Line` `TLV5R`): 24 long, 1 high.
const STEP_LINE: f32 = 24.0;
const STEP_LINE_HEIGHT: f32 = 1.0;

/// The steps from the start page to a team's tasks on this computer (`Steps` `F7crC`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StartStep {
  Join,
  Project,
  Folder,
  Download,
}

impl StartStep {
  const ALL: [Self; 4] = [Self::Join, Self::Project, Self::Folder, Self::Download];

  const fn key(self) -> &'static str {
    match self {
      Self::Join => "startup.steps.join",
      Self::Project => "startup.steps.project",
      Self::Folder => "startup.steps.folder",
      Self::Download => "startup.steps.download",
    }
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Join => "join",
      Self::Project => "project",
      Self::Folder => "folder",
      Self::Download => "download",
    }
  }
}

/// A crumb between *Start* and the step's own, which opens its place.
pub(crate) struct Ancestor {
  pub label: String,
  pub route: Route,
}

/// What a step shows in its frame.
pub(crate) struct StepFrame {
  /// The step's screen, whose element ids start with it.
  pub id: &'static str,
  /// The crumbs between *Start* and the step's own.
  pub ancestors: Vec<Ancestor>,
  /// The step's own crumb.
  pub crumb: String,
  pub step: StartStep,
  pub title: String,
  pub lead: String,
  pub body: Element,
  pub note: Option<Element>,
}

/// The step in its frame.
pub(crate) fn step_frame(ctx: &mut Ctx, frame: StepFrame) -> Element {
  let StepFrame {
    id,
    ancestors,
    crumb,
    step,
    title,
    lead,
    body,
    note,
  } = frame;
  let top = Row::new()
    .id(format!("{id}-top"))
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .justify(Justify::SpaceBetween)
    .child(breadcrumb(ctx, id, ancestors, &crumb))
    .child(steps(ctx, id, step));
  let head = Column::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&title)
        .id(format!("{id}-title"))
        .variant(StartupText::Title.role()),
    )
    .child(
      Text::new(&lead)
        .id(format!("{id}-lead"))
        .variant(StartupText::Lead.role())
        .width(Dimension::Pct(100.0)),
    );
  let inner = Column::new()
    .id(format!("{id}-inner"))
    .width(Dimension::Pct(100.0))
    .max_width(Dimension::Px(INNER_WIDTH))
    .spacing(SpacingSize::Xl)
    .child(top)
    .child(head)
    .child(body)
    .with_children(note);
  let page = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(StartupSpacing::Page.role())
        .bottom(SpacingSize::Section)
        .left(StartupSpacing::PageSide.role())
        .right(StartupSpacing::PageSide.role()),
    )
    .align_items(Alignment::Center)
    .child(inner);
  Column::new()
    .id(format!("screen-{id}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(
      ScrollVertical::new(page)
        .id(format!("{id}-scroll"))
        .width(Dimension::Pct(100.0))
        .height(Dimension::Pct(100.0)),
    )
    .into()
}

/// *Start*, the step's `ancestors` and its own crumb (`Breadcrumb` `LTKnD`, `pRFzL`); *Start* opens All projects.
fn breadcrumb(ctx: &mut Ctx, id: &str, ancestors: Vec<Ancestor>, crumb: &str) -> Row {
  let start = Ancestor {
    label: ctx.t("startup.crumbs.start").to_string(),
    route: Route::AllProjects,
  };
  let navigation = Navigation::of(ctx);
  let mut row = Row::new()
    .id(format!("{id}-crumbs"))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center);
  for (index, ancestor) in std::iter::once(start).chain(ancestors).enumerate() {
    let name = if index == 0 {
      "start".to_owned()
    } else {
      index.to_string()
    };
    row = row
      .child(
        opening(navigation.to(ancestor.route))
          .id(format!("{id}-crumb-{name}"))
          .flex_shrink(0.0)
          .describe("a11y_name", ancestor.label.clone())
          .hovered_style(Style::new().border_bottom(Border::inside(BorderSize::Sm, PaletteColor::TextMuted)))
          .child(Text::new(&ancestor.label).variant(StartupText::Crumb.role()).nowrap()),
      )
      .child(icon(Icon::ChevronRight, IconSize::Sm, PaletteColor::TextMuted));
  }
  row.child(
    Text::new(crumb)
      .id(format!("{id}-crumb-current"))
      .variant(StartupText::CrumbCurrent.role())
      .nowrap(),
  )
}

/// The step line (`F7crC`, `YLUz1`): a step before `current` as a `success` check, the others as their number in a
/// ring; each with its name, `current` in `text_primary`, a line between two.
fn steps(ctx: &Ctx, id: &str, current: StartStep) -> Row {
  let mut row = Row::new()
    .id(format!("{id}-steps"))
    .flex_shrink(0.0)
    .spacing(StartupSpacing::Steps.role())
    .align_items(Alignment::Center);
  let reached = StartStep::ALL.iter().position(|step| *step == current);
  for (index, step) in StartStep::ALL.into_iter().enumerate() {
    if index > 0 {
      row = row.child(
        Row::new()
          .width(Dimension::Px(STEP_LINE))
          .height(Dimension::Px(STEP_LINE_HEIGHT))
          .background(PaletteColor::BorderStrong),
      );
    }
    let here = step == current;
    let done = reached.is_some_and(|reached| index < reached);
    let label = Text::new(&ctx.t(step.key()))
      .variant(if here {
        StartupText::StepLabelCurrent.role()
      } else {
        StartupText::StepLabel.role()
      })
      .nowrap();
    let (mark, label) = if done {
      let check = icon(Icon::CircleCheck, IconSize::Md, PaletteColor::Success);
      (Element::from(check), label.color(PaletteColor::TextSecondary))
    } else {
      (step_number(index + 1, here).into(), label)
    };
    row = row.child(
      Row::new()
        .id(format!("{id}-step-{}", step.name()))
        .describe("current", here.to_string())
        .describe("done", done.to_string())
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(mark)
        .child(label),
    );
  }
  row
}

/// A step's `number` in its ring, in `text_primary` when it is the current one.
fn step_number(number: usize, here: bool) -> Row {
  let (ring, digit) = if here {
    (PaletteColor::TextPrimary, PaletteColor::TextPrimary)
  } else {
    (PaletteColor::BorderStrong, PaletteColor::TextMuted)
  };
  Row::new()
    .width(Dimension::Px(STEP_NUMBER))
    .height(Dimension::Px(STEP_NUMBER))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::Full.role())
    .border(Border::inside(BorderSize::Sm, ring))
    .child(
      Text::new(&number.to_string())
        .variant(StartupText::StepNumber.role())
        .color(digit)
        .nowrap(),
    )
}

/// A note under a step (`Note` `n9aRSG`): a 13 info glyph and its words, and a link after them when it has one.
pub(crate) fn note(id: &str, text: &str, link: Option<Element>) -> Element {
  Row::new()
    .id(id.to_owned())
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(text)
        .id(format!("{id}-text"))
        .variant(StartupText::Sub.role()),
    )
    .with_children(link)
    .into()
}
