//! The Memory filter's panel (`Detail` `s0hmJ` in Y1, `AofW2` in Y2, `tKZqZ` in Y6): the picked record's head —
//! *Memory* and its status over its title — its chips (the status, the type, whose it is, *Pinned*, *This computer* in
//! a team project) with who wrote it and what happened to it since at their end, what approving it replaces or what
//! deprecating it did, its *Text* and *Tags*, where it came from (*Source*) and the invocations that were given it
//! (*Given to agents*); its actions are pinned at the bottom ([`bar`]). A record being reviewed, edited or written
//! takes the panel as its editor ([`crate::screens::memory::editor`]).

mod bar;
mod head;
mod source;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::MemoryId,
  memory::{Memory, MemoryStatus},
};
use orchester_workspace::memory::read::ProjectMemory;

pub(in crate::screens::memory) use self::head::status_word;
use self::head::{chips, head, notice};
use super::super::{parts, state::MemoryState};
use crate::theme::{ExtraSpacing, HubSpacing, HubText, MemoryText};

#[derive(Clone, PartialEq)]
pub(in crate::screens::memory) struct PanelProps {
  pub memory: Arc<ProjectMemory>,
  pub record: MemoryId,
  /// The project is a team's: its memory is this computer's only.
  pub team: bool,
}

impl DevtoolsInspectable for PanelProps {}

pub(in crate::screens::memory) struct RecordPanel {
  /// Every invocation that was given the record is listed.
  all_given: Signal<bool>,
}

impl Component for RecordPanel {
  type Props = PanelProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      all_given: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PanelProps>().clone();
    let state = MemoryState::of(ctx);
    let memory = &props.memory;
    let Some(record) = memory
      .waiting
      .iter()
      .chain(&memory.records)
      .find(|record| record.id() == props.record)
    else {
      return Element::new();
    };
    let given = source::given(ctx, &state, record);
    // A proposal was given to no agent.
    let given = (record.status() != MemoryStatus::Proposed)
      .then_some(given)
      .and_then(|given| source::given_section(ctx, given, &self.all_given));
    let body = body()
      .child(head(ctx, record))
      .child(chips(ctx, memory, record, props.team))
      .with_children(notice(ctx, memory, record))
      .child(text(ctx, record))
      .with_children(tags(ctx, record))
      .child(source::source(ctx, &state, memory, record))
      .with_children(given);
    let failure = state.failure_note(ctx, "memory-panel", Some(record.id()));
    frame(
      "memory-panel",
      (record.status().name(), body),
      failure,
      bar::bar(ctx, &state, record),
    )
  }
}

/// The column the panel's sections stand in (`Scroll` `mQscF`): 24 from its sides, 20 under its top and apart, and
/// nothing over its bottom: the design fits a record's sections over the pinned bar with what its rows round to.
pub(in crate::screens::memory) fn body() -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(HubSpacing::BodyTop.role())
        .left(HubSpacing::Sections.role())
        .right(HubSpacing::Sections.role()),
    )
    .spacing(HubSpacing::BodyTop.role())
}

/// A panel of the Memory filter, `id` with what it shows in its `status`: `body` scrolls over the pinned `bar`, and why
/// the last change did nothing stands between them.
pub(in crate::screens::memory) fn frame(
  id: &str,
  (status, body): (&str, Column),
  failure: Option<Row>,
  bar: Element,
) -> Element {
  let scroll = ScrollVertical::new(body)
    .id(format!("{id}-scroll"))
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0));
  Column::new()
    .id(id.to_owned())
    .describe("status", status.to_owned())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .child(scroll)
    .with_children(failure.map(|failure| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(HubSpacing::Sections.role(), SpacingSize::Sm))
        .child(failure)
    }))
    .child(bar)
    .into()
}

/// A section's label (`Label` `jJ0Xd`) over `content`, 8 apart.
fn section(id: &str, label: &str, content: impl Into<Element>) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(label)
        .id(format!("{id}-label"))
        .variant(HubText::PanelLabel.role())
        .nowrap(),
    )
    .child(content)
}

/// *Text*: what the record says in a `border` box (`Text box` `lc2jE`), in `text_muted` when no agent is given it.
fn text(ctx: &Ctx, record: &Memory) -> Column {
  let given = matches!(record.status(), MemoryStatus::Active | MemoryStatus::Proposed);
  let said = Text::new(record.content().text())
    .id("memory-panel-text")
    .variant(MemoryText::Record.role())
    .color(if given {
      PaletteColor::TextPrimary
    } else {
      PaletteColor::TextMuted
    })
    .selectable(true)
    .flex(1.0)
    .min_width(Dimension::Px(0.0));
  let boxed = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(said);
  section("memory-panel-text-section", &ctx.t("memory.editor.text"), boxed)
}

/// *Tags* (`Tags` `T25p2m`); none on a record without tags.
fn tags(ctx: &Ctx, record: &Memory) -> Option<Column> {
  let chips = parts::tags("memory-panel", record)?;
  Some(section(
    "memory-panel-tags-section",
    &ctx.t("memory.editor.tags"),
    chips,
  ))
}
