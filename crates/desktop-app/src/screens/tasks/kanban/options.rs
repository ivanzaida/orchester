//! The board's Display options (`Display popover` `kQPuZ` in `09 Display options` `J0ncJ`), which the header's Display
//! opens while the board shows: List or Board, what the columns and the rows group by and how the cards are ordered
//! (*Sorting never changes the run queue.*), which card properties show, as chips that turn each on or off, whether
//! empty columns and the Canceled column show, and *Reset to default*. Every choice is kept for the project. A 320
//! wide panel, `surface_raised` in a `border_strong` stroke, radius 10, 12 inside, 10 between its parts.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::tasks::browse::TaskSort;

use super::display::{Columns, Display, Lanes, Layout, Property};
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    menu::{MenuItem, menu_panel, separator},
    pressable::{Focus, pressable},
    tabs::{Segment, SegmentFace, segmented},
    toggle::{Toggle, ToggleKind},
  },
};

/// The panel's width (`kQPuZ`), a select's (`Is54m`) and a property chip's height (`ZDOu5`).
pub const PANEL_WIDTH: f32 = 320.0;
const SELECT_WIDTH: f32 = 150.0;
const SELECT_HEIGHT: f32 = 26.0;
/// A select's row (`Columns` `kYW7F`).
const SELECT_ROW: f32 = 28.0;
const CHIP_HEIGHT: f32 = 22.0;
/// A list a select opens (`Columns menu` `xK96w`).
const LIST_WIDTH: f32 = 180.0;
/// The card properties, a row of chips each (`FkNYM`, `YoVQM`, `eZlmp`).
const CHIP_ROWS: [&[Property]; 3] = [
  &[Property::Key, Property::Priority, Property::Points, Property::Module],
  &[Property::Tags, Property::Run, Property::Readiness, Property::Agent],
  &[Property::Age, Property::Blocked, Property::Pipeline],
];
/// The orders, with the i18n keys of their words: the list's.
const SORTS: [(TaskSort, &str); 5] = [
  (TaskSort::Manual, "tasks.sort.manual"),
  (TaskSort::Newest, "tasks.sort.newest"),
  (TaskSort::Title, "tasks.sort.title"),
  (TaskSort::Status, "tasks.sort.status"),
  (TaskSort::Priority, "tasks.sort.priority"),
];

#[derive(Clone)]
pub struct OptionsProps {
  pub display: Signal<Display>,
  pub sort: Signal<TaskSort>,
}

/// Compared by their signals.
impl PartialEq for OptionsProps {
  fn eq(&self, other: &Self) -> bool {
    self.display.id() == other.display.id() && self.sort.id() == other.sort.id()
  }
}

impl DevtoolsInspectable for OptionsProps {}

/// Which select's list is open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Open {
  #[default]
  None,
  Columns,
  Rows,
  Order,
}

impl DevtoolsInspectable for Open {}

pub struct BoardOptions {
  open: Signal<Open>,
  shown: Signal<bool>,
  anchors: [ElementRef; 3],
}

impl Component for BoardOptions {
  type Props = OptionsProps;

  fn create(ctx: &mut Ctx) -> Self {
    let (open, shown) = (ctx.signal(Open::None), ctx.signal(false));
    let closed = open.clone();
    ctx.watch(&shown, move |shown: &bool| {
      if !*shown {
        closed.set(Open::None);
      }
    });
    Self {
      open,
      shown,
      anchors: Default::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<OptionsProps>().clone();
    let display = props.display.get();
    let open = self.open.get();
    if (open != Open::None) != self.shown.get_untracked() {
      self.shown.set(open != Open::None);
    }
    let order = SORTS
      .iter()
      .find(|(sort, _)| *sort == props.sort.get())
      .map_or("tasks.sort.manual", |(_, key)| key);
    let groups = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xs)
      .child(self.select(
        ctx,
        ("columns", "tasks.board.options.columns"),
        columns_key(display.columns),
        Open::Columns,
      ))
      .child(self.select(
        ctx,
        ("rows", "tasks.board.options.rows"),
        lanes_key(display.lanes),
        Open::Rows,
      ))
      .child(self.select(ctx, ("order", "tasks.board.options.order"), order, Open::Order))
      .child(
        Row::new()
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .child(icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted))
          .child(Text::new(&ctx.t("tasks.display.note")).variant(ExtraTypography::Note.role())),
      );
    let list = self.list(ctx, &props, open);
    Column::new()
      .id("tasks-board-options")
      .width(Dimension::Px(PANEL_WIDTH))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Loose.role())
      .rounded(RadiusSize::Lg)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .child(layout_switch(ctx, &props.display, "tasks-board-options-layout", true))
      .child(groups)
      .child(separator(ctx))
      .child(properties(ctx, &props.display, &display))
      .child(separator(ctx))
      .child(checks(ctx, &props.display, &display))
      .child(foot(ctx, &props.display))
      .with_children(list)
  }
}

impl BoardOptions {
  /// A select's row: its name, and the box that opens its list.
  fn select(&self, ctx: &Ctx, (name, label): (&str, &str), value: &str, which: Open) -> Row {
    let anchor = match which {
      Open::Rows => &self.anchors[1],
      Open::Order => &self.anchors[2],
      Open::Columns | Open::None => &self.anchors[0],
    };
    let open = self.open.clone();
    let glyph = match which {
      Open::Rows => Icon::Rows2,
      Open::Order => Icon::ArrowDownUp,
      Open::Columns | Open::None => Icon::CircleDotDashed,
    };
    let chosen = self.open.get() == which;
    let field = pressable(move || open.set(if chosen { Open::None } else { which }))
      .id(format!("tasks-board-options-{name}"))
      .ref_element(anchor.clone())
      .width(Dimension::Px(SELECT_WIDTH))
      .height(Dimension::Px(SELECT_HEIGHT))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(ExtraSpacing::Snug.role())
      .rounded(RadiusSize::Md)
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(
        BorderSize::Sm,
        if chosen {
          PaletteColor::BorderFocus
        } else {
          PaletteColor::Border
        },
      ))
      .describe("a11y_name", ctx.t(label))
      .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
      .child(
        Text::new(&ctx.t(value))
          .id(format!("tasks-board-options-{name}-value"))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextPrimary)
          .flex(1.0)
          .nowrap(),
      )
      .child(icon(Icon::ChevronDown, IconSize::Sm, PaletteColor::TextMuted));
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(SELECT_ROW))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&ctx.t(label))
          .variant(TypographyStyle::Description)
          .flex(1.0)
          .nowrap(),
      )
      .child(field)
  }

  /// The open select's list.
  fn list(&self, ctx: &mut Ctx, props: &OptionsProps, open: Open) -> Option<Popup> {
    let display = props.display.clone();
    let now = display.get();
    let (rows, anchor): (Vec<MenuItem>, &ElementRef) = match open {
      Open::None => return None,
      Open::Columns => (
        Columns::ALL
          .into_iter()
          .map(|columns| {
            let display = display.clone();
            MenuItem::new(
              format!("tasks-board-options-columns-{}", columns.name()),
              ctx.t(columns_key(columns)),
              move || display.update(|display| display.columns = columns),
            )
            .icon(columns_glyph(columns))
            .chosen(now.columns == columns)
          })
          .collect(),
        &self.anchors[0],
      ),
      Open::Rows => (
        Lanes::ALL
          .into_iter()
          .map(|lanes| {
            let display = display.clone();
            MenuItem::new(
              format!("tasks-board-options-rows-{}", lanes.name()),
              ctx.t(lanes_key(lanes)),
              move || display.update(|display| display.lanes = lanes),
            )
            .chosen(now.lanes == lanes)
          })
          .collect(),
        &self.anchors[1],
      ),
      Open::Order => (
        SORTS
          .into_iter()
          .map(|(sort, key)| {
            let chosen = props.sort.clone();
            MenuItem::new(format!("tasks-board-options-order-{key}"), ctx.t(key), move || {
              chosen.set(sort)
            })
            .chosen(props.sort.get() == sort)
          })
          .collect(),
        &self.anchors[2],
      ),
    };
    let rows: Vec<Element> = rows.into_iter().map(|row| row.render(ctx, &self.shown)).collect();
    Some(
      Popup::new(
        anchor.clone(),
        menu_panel("tasks-board-options-list")
          .width(Dimension::Px(LIST_WIDTH))
          .with_children(rows),
      )
      .open(self.shown.clone())
      .placement(Placement::BottomEnd)
      .offset(0.0, ExtraSpacing::Snug.value()),
    )
  }
}

/// List or Board, as a segmented control: in the header its two glyphs (`Layout` `h3iCkX`), in the panel with their
/// words (`Layout` `O1Xwma`). The choice is kept for the project.
pub fn layout_switch(ctx: &Ctx, display: &Signal<Display>, id: &str, words: bool) -> Row {
  let now = display.get().layout;
  let segment = |layout: Layout, (name, glyph, label): (&str, Icon, &str)| {
    let display = display.clone();
    let label = ctx.t(label);
    let face = if words {
      SegmentFace::Text(label)
    } else {
      SegmentFace::Icon(glyph, label)
    };
    Segment::new(format!("{id}-{name}"), face, now == layout, move || {
      display.update(|display| display.layout = layout);
    })
  };
  let switch = segmented(
    id,
    [
      segment(Layout::List, ("list", Icon::List, "tasks.layout.list")),
      segment(Layout::Board, ("board", Icon::Kanban, "tasks.layout.board")),
    ],
    Focus::Pointer,
  );
  if words {
    switch.width(Dimension::Pct(100.0))
  } else {
    switch
  }
}

/// *Card properties* and a chip for each, `accent_muted` in a `border_strong` stroke while it shows.
fn properties(ctx: &Ctx, display: &Signal<Display>, now: &Display) -> Column {
  let rows = CHIP_ROWS.iter().map(|row| {
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .with_children(row.iter().map(|property| chip(ctx, display, now, *property)))
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&ctx.t("tasks.board.options.properties"))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextMuted),
    )
    .with_children(rows)
}

fn chip(ctx: &Ctx, display: &Signal<Display>, now: &Display, property: Property) -> lurq::components::Button {
  let on = now.shows(property);
  let toggle = display.clone();
  let chip = pressable(move || toggle.update(|display| display.toggle(property)))
    .id(format!("tasks-board-options-property-{}", property.name()))
    .height(Dimension::Px(CHIP_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .rounded(ExtraRadius::Full.role())
    .describe("state", if on { "on" } else { "off" })
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(
      Text::new(&ctx.t(property_key(property)))
        .variant(if on {
          ExtraTypography::NoteStrong.role()
        } else {
          ExtraTypography::Note.role()
        })
        .nowrap(),
    );
  if on {
    chip
      .background(PaletteColor::AccentMuted)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
  } else {
    chip.border(Border::inside(BorderSize::Sm, PaletteColor::Border))
  }
}

/// *Show empty columns* and *Show Canceled column*.
fn checks(ctx: &Ctx, display: &Signal<Display>, now: &Display) -> Column {
  let (empty, canceled) = (display.clone(), display.clone());
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(
      Toggle::new(
        "tasks-board-options-empty",
        ToggleKind::Checkbox,
        now.show_empty,
        ctx.t("tasks.board.options.showEmpty"),
      )
      .on_change(move |on| empty.update(|display| display.show_empty = on)),
    )
    .child(
      Toggle::new(
        "tasks-board-options-canceled",
        ToggleKind::Checkbox,
        now.show_canceled,
        ctx.t("tasks.board.options.showCanceled"),
      )
      .on_change(move |on| canceled.update(|display| display.show_canceled = on)),
    )
}

/// *Reset to default* and that every choice is kept for the project.
fn foot(ctx: &Ctx, display: &Signal<Display>) -> Row {
  let reset = display.clone();
  Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(
      pressable(move || reset.update(|display| *display = display.reset()))
        .id("tasks-board-options-reset")
        .child(Text::new(&ctx.t("tasks.board.options.reset")).variant(TypographyStyle::Description)),
    )
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&ctx.t("tasks.board.options.saved")).variant(ExtraTypography::Note.role()))
}

const fn columns_key(columns: Columns) -> &'static str {
  match columns {
    Columns::Status => "tasks.board.options.byStatus",
    Columns::Priority => "tasks.board.options.byPriority",
    Columns::Module => "tasks.board.options.byModule",
    Columns::Pipeline => "tasks.board.options.byPipeline",
  }
}

const fn columns_glyph(columns: Columns) -> Icon {
  match columns {
    Columns::Status => Icon::CircleDotDashed,
    Columns::Priority => Icon::SignalHigh,
    Columns::Module => Icon::Target,
    Columns::Pipeline => Icon::Workflow,
  }
}

const fn lanes_key(lanes: Lanes) -> &'static str {
  match lanes {
    Lanes::None => "tasks.board.options.noRows",
    Lanes::Module => "tasks.board.options.byModule",
    Lanes::Priority => "tasks.board.options.byPriority",
  }
}

const fn property_key(property: Property) -> &'static str {
  match property {
    Property::Key => "tasks.board.property.key",
    Property::Priority => "tasks.board.property.priority",
    Property::Points => "tasks.board.property.points",
    Property::Module => "tasks.board.property.module",
    Property::Tags => "tasks.board.property.tags",
    Property::Run => "tasks.board.property.run",
    Property::Readiness => "tasks.board.property.readiness",
    Property::Agent => "tasks.board.property.agent",
    Property::Age => "tasks.board.property.age",
    Property::Blocked => "tasks.board.property.blocked",
    Property::Pipeline => "tasks.board.property.pipeline",
  }
}
