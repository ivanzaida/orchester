//! One line of the list: a group's header (`Group · Waiting for you` `ED9cJ`: 34 high, 10 over its line, 8 at its sides
//! and between its parts; a 12 `text_muted` chevron that folds the group, its 13 glyph in its tone, its name in 12/500
//! `text_secondary` and its count in 12 `text_muted`), a task's row ([`super::row`]), or the one row-high line that
//! stands where *Waiting for you* has no row (`Nothing waiting` `KJbty`, `03`).

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::{
  items::{Folding, Group},
  row::{self, ROW_HEIGHT, RowProps},
};
use crate::{
  theme::{ExtraSpacing, IconSize, TasksSpacing},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// A group's header line (`ED9cJ`).
pub(super) const GROUP_HEIGHT: f32 = 34.0;

/// A group's header line.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct GroupLine {
  pub group: Group,
  pub count: usize,
  pub folded: bool,
}

/// One line of the list.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Line {
  Group(GroupLine),
  Row(Box<RowProps>),
  /// Nothing waits for the person.
  Empty,
  /// Something waits, but the search hides it.
  NoMatch,
}

impl DevtoolsInspectable for Line {}

/// The height a line takes with the gap under it.
pub(super) fn height(line: &Line) -> f32 {
  let own = match line {
    Line::Group(_) => GROUP_HEIGHT,
    Line::Row(_) | Line::Empty | Line::NoMatch => ROW_HEIGHT,
  };
  own + ExtraSpacing::Tight.value()
}

/// The key a line is mounted by.
pub(super) fn key_of(line: &Line) -> String {
  match line {
    Line::Group(line) => format!("group-{:?}", line.group),
    Line::Row(props) => props.key.to_string(),
    Line::Empty => "empty".to_owned(),
    Line::NoMatch => "no-match".to_owned(),
  }
}

pub(super) struct ReviewLine {
  hovered: Signal<bool>,
}

impl Component for ReviewLine {
  type Props = Line;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let line = match ctx.props::<Line>().clone() {
      Line::Group(line) => group(ctx, &line),
      Line::Row(props) => row::row(ctx, &props, &self.hovered),
      Line::Empty => nothing(ctx, "review-empty", "review.empty.title", Some("review.empty.hint")),
      Line::NoMatch => nothing(ctx, "review-no-match", "review.noMatch", None),
    };
    // 2 under each line, and the page's 28 at their end: the list runs to the panel's edge for its scrollbar.
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .bottom(ExtraSpacing::Tight.role())
          .right(TasksSpacing::Sides.role()),
      )
      .child(line)
  }
}

/// A group's glyph and its tone.
const fn look(group: Group) -> (Icon, PaletteColor) {
  match group {
    Group::Waiting => (Icon::Eye, PaletteColor::Info),
    Group::WithAgent => (Icon::LoaderCircle, PaletteColor::TextMuted),
    Group::Approved => (Icon::CircleCheck, PaletteColor::Success),
  }
}

pub(super) const fn label_key(group: Group) -> &'static str {
  match group {
    Group::Waiting => "review.groups.waiting",
    Group::WithAgent => "review.groups.withAgent",
    Group::Approved => "review.groups.approved",
  }
}

/// The id a group's header line and its parts are known by.
pub(super) fn group_id(group: Group) -> &'static str {
  match group {
    Group::Waiting => "review-group-waiting",
    Group::WithAgent => "review-group-with-agent",
    Group::Approved => "review-group-approved",
  }
}

/// A group's header line, which folds the group away or unfolds it.
fn group(ctx: &mut Ctx, line: &GroupLine) -> Element {
  let folding = ctx
    .use_context::<Folding>()
    .expect("Review's list provides its groups' folding");
  let group = line.group;
  let id = group_id(group);
  let (glyph, color) = look(group);
  let label = ctx.t(label_key(group));
  pressable(move || folding.toggle(group))
    .id(id)
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(GROUP_HEIGHT))
    .padding(
      Padding::new()
        .top(TasksSpacing::GroupTop.role())
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .describe("a11y_name", ctx.t_args("review.groups.toggle", [("group", &*label)]))
    .describe("state", if line.folded { "folded" } else { "open" })
    .child(icon(
      if line.folded {
        Icon::ChevronRight
      } else {
        Icon::ChevronDown
      },
      IconSize::Sm,
      PaletteColor::TextMuted,
    ))
    .child(icon(glyph, IconSize::Base, color).id(format!("{id}-icon")))
    .child(
      Text::new(&label)
        .id(format!("{id}-label"))
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(
      Text::new(&line.count.to_string())
        .id(format!("{id}-count"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// The row-high line where *Waiting for you* has no row (`KJbty`): a check, what it says and, while nothing waits at
/// all, when a task shows up here.
fn nothing(ctx: &Ctx, id: &str, words: &str, hint: Option<&str>) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(icon(Icon::Check, IconSize::Md, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t(words))
        .id(format!("{id}-text"))
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .with_children(hint.map(|hint| {
      Text::new(&ctx.t(hint))
        .id(format!("{id}-hint"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
    }))
    .into()
}
