//! A record in a list (`Memory / Record row` `J1utD`): its type's glyph; its key and, on the task's latest handoff,
//! the flag *Latest · given to every agent*; what it says (a handoff's summary, then its lists as label and text
//! lines); the meta line with its tags; *Supersedes N earlier versions*, which unfolds them; and at its end the pin,
//! the status of a record no agent is given and, under the pointer, its actions, whose room the end keeps while they
//! are hidden, so the text never reflows under the pointer. A record no agent is given says what it says in
//! `text_muted`; a deprecated one is restored or deleted, the others only deleted. On the project's page a press
//! opens the record's details in place, and a second press closes them.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::memory::{Memory, MemoryKind, MemoryStatus};
use orchester_workspace::memory::{change::Change, handoff::HandoffParts};

use super::{
  parts,
  state::{MemoryState, Opened},
  words,
  work::Write,
};
use crate::{
  theme::{ControlHeight, ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, MemoryText},
  ui::{
    icon::{Icon, icon},
    icon_button::IconButton,
    pressable::pressable,
  },
};

/// The width of a handoff list's label (`HzwHN` in `J1utD`).
const LIST_LABEL_WIDTH: f32 = 150.0;

/// An earlier version of a record, as its history shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Earlier {
  pub text: Arc<str>,
  pub meta: Arc<str>,
}

#[derive(Clone, PartialEq)]
pub struct RowProps {
  pub memory: Memory,
  pub meta: Arc<str>,
  /// The task's latest handoff, which every agent of the task is given.
  pub latest: bool,
  pub earlier: Arc<[Earlier]>,
  /// A press opens its details in place, or closes them while they are open (the project's page).
  pub details: Option<Details>,
}

/// Whether a row's details are open in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Details {
  Closed,
  Open,
}

impl DevtoolsInspectable for RowProps {}

pub struct RecordRow {
  hovered: Signal<bool>,
}

impl Component for RecordRow {
  type Props = RowProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let state = MemoryState::of(ctx);
    let memory = &props.memory;
    let id = format!("memory-record-{}", memory.id());
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    let row = Row::new()
      .id(id.clone())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
      .spacing(SpacingSize::Md)
      .rounded(ExtraRadius::Tile.role())
      .describe("status", memory.status().name())
      .describe("kind", memory.content().kind().name())
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .child(parts::glyph(words::glyph(memory.content().kind())))
      .child(body(ctx, &state, &props, &id))
      .child(self.trailing(ctx, &state, memory, &id));
    // An open record's row sits in its details' box, which colors it (`aBtsx`).
    let row = if self.hovered.get() && props.details != Some(Details::Open) {
      row.background(ExtraColor::Hover.role())
    } else {
      row
    };
    let Some(details) = props.details else {
      return Element::from(row);
    };
    let record = memory.id();
    pressable(move || state.choose(record))
      .id(format!("{id}-select"))
      .width(Dimension::Pct(100.0))
      .describe("details", if details == Details::Open { "open" } else { "closed" })
      .child(row)
      .into()
  }
}

impl RecordRow {
  /// The pin, a closed record's status, and the actions under the pointer or the room they take (`I74BQb`).
  fn trailing(&self, ctx: &mut Ctx, state: &MemoryState, memory: &Memory, id: &str) -> Element {
    let mut trailing = Row::new()
      .flex_shrink(0.0)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center);
    if memory.is_pinned() {
      trailing = trailing.child(icon(Icon::Pin, IconSize::Base, PaletteColor::TextPrimary).id(format!("{id}-pinned")));
    }
    if let Some(status) = words::closed_status(ctx, memory.status()) {
      trailing = trailing.child(parts::status(
        &format!("{id}-status"),
        status,
        PaletteColor::TextSecondary,
      ));
    }
    let actions = if self.hovered.get() && !state.busy() {
      actions(ctx, state, memory, id)
    } else {
      actions_room(memory, id)
    };
    trailing.child(actions).into()
  }
}

/// What the record says, its meta line and its history (`OVrv3`).
fn body(ctx: &mut Ctx, state: &MemoryState, props: &RowProps, id: &str) -> Element {
  let memory = &props.memory;
  let content = memory.content();
  let shown = memory.status() == MemoryStatus::Active || memory.status() == MemoryStatus::Proposed;
  let color = if shown {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextMuted
  };
  let handoff = (content.kind() == MemoryKind::Handoff).then(|| HandoffParts::of(content.text()));
  let text = handoff
    .as_ref()
    .map_or(content.text(), |parts| parts.summary.as_str())
    .to_owned();
  let mut body = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xs);
  body = body.with_children(head(ctx, props, id));
  if !text.is_empty() {
    body = body.child(
      Text::new(&text)
        .id(format!("{id}-text"))
        .variant(ExtraTypography::Lead.role())
        .color(color),
    );
  }
  if let Some(parts) = handoff.filter(|parts| !parts.lists.is_empty()) {
    body = body.child(lists(ctx, &parts, id));
  }
  body = body.child(
    Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&props.meta)
          .id(format!("{id}-meta"))
          .variant(TypographyStyle::Caption),
      )
      .with_children(parts::tags(id, memory)),
  );
  if !props.earlier.is_empty() {
    body = body.child(history(ctx, state, props, id));
  }
  body.into()
}

/// The key and the latest handoff's flag, when the record has either (`KEpSk`).
fn head(ctx: &Ctx, props: &RowProps, id: &str) -> Option<Row> {
  let key = props.memory.content().key();
  if key.is_none() && !props.latest {
    return None;
  }
  Some(
    Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .with_children(key.map(|key| {
        Text::new(key.as_str())
          .id(format!("{id}-key"))
          .variant(ExtraTypography::MonoNote.role())
          .nowrap()
      }))
      .with_children(
        props
          .latest
          .then(|| parts::flag(&format!("{id}-latest"), &ctx.t("memory.latestHandoff"))),
      ),
  )
}

/// A handoff's lists, each a label and its items (`O4Z55Y`).
fn lists(ctx: &Ctx, handoff: &HandoffParts, id: &str) -> Column {
  Column::new()
    .id(format!("{id}-lists"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Tight.role()))
    .spacing(ExtraSpacing::Fine.role())
    .with_children(handoff.lists.iter().map(|(list, items)| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .child(
          Text::new(&words::handoff_list(ctx, *list))
            .width(Dimension::Px(LIST_LABEL_WIDTH))
            .flex_shrink(0.0)
            .variant(MemoryText::ListLabel.role()),
        )
        .child(
          Text::new(&items.join("\n"))
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .variant(ExtraTypography::Prose.role()),
        )
    }))
}

/// *Supersedes N earlier versions*, which unfolds them: each muted behind a 2 rule (`nlx3o`, `lLbsO`).
fn history(ctx: &mut Ctx, state: &MemoryState, props: &RowProps, id: &str) -> Element {
  let open = state.history_open(props.memory.id());
  let count = props.earlier.len();
  let label = if count == 1 {
    ctx.t("memory.history.one")
  } else {
    ctx.t_args("memory.history.other", [("count", count.to_string())])
  };
  let (toggle, record) = (state.clone(), props.memory.id());
  let link = pressable(move || toggle.toggle_history(record))
    .id(format!("{id}-history"))
    .describe("a11y_name", label.clone())
    .describe("state", if open { "open" } else { "closed" })
    .spacing(SpacingSize::Xs)
    .child(icon(
      if open { Icon::ChevronDown } else { Icon::ChevronRight },
      IconSize::Sm,
      PaletteColor::TextMuted,
    ))
    .child(Text::new(&label).variant(TypographyStyle::FieldLabel).nowrap());
  let mut history = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(link);
  if open {
    for (index, earlier) in props.earlier.iter().enumerate() {
      history = history.child(
        Column::new()
          .id(format!("{id}-earlier-{index}"))
          .width(Dimension::Pct(100.0))
          .padding(
            Padding::new()
              .left(ExtraSpacing::Loose.role())
              .top(ExtraSpacing::Tight.role())
              .bottom(ExtraSpacing::Tight.role()),
          )
          .spacing(ExtraSpacing::Tight.role())
          .border_left(Border::inside(BorderSize::Md, PaletteColor::Border))
          .child(
            Text::new(&earlier.text)
              .variant(ExtraTypography::Prose.role())
              .color(PaletteColor::TextMuted),
          )
          .child(Text::new(&earlier.meta).variant(TypographyStyle::Caption)),
      );
    }
  }
  history.into()
}

/// What one of a record's actions does when pressed.
enum Press {
  Open(Opened),
  Write(Change),
}

/// One of a record's actions: its id suffix, glyph, label key and what it does.
struct Action {
  name: &'static str,
  glyph: Icon,
  label: &'static str,
  press: Press,
}

/// A record's actions under the pointer (`O8wNd`), in their order: Edit, Pin or Unpin and Deprecate on an active
/// record, Restore on a deprecated one, and Delete on every record.
fn actions_of(memory: &Memory) -> Vec<Action> {
  let record = memory.id();
  let action = |name, glyph, label, press| Action {
    name,
    glyph,
    label,
    press,
  };
  let mut actions = Vec::new();
  match memory.status() {
    MemoryStatus::Active => {
      let pinned = memory.is_pinned();
      let (glyph, label) = if pinned {
        (Icon::PinOff, "memory.actions.unpin")
      } else {
        (Icon::Pin, "memory.actions.pin")
      };
      actions.push(action(
        "edit",
        Icon::Pencil,
        "memory.actions.edit",
        Press::Open(Opened::Edit(record)),
      ));
      actions.push(action("pin", glyph, label, Press::Write(Change::Pin(!pinned))));
      actions.push(action(
        "deprecate",
        Icon::Archive,
        "memory.actions.deprecate",
        Press::Write(Change::Deprecate),
      ));
    }
    MemoryStatus::Deprecated => {
      actions.push(action(
        "restore",
        Icon::ArchiveRestore,
        "memory.actions.restore",
        Press::Write(Change::Restore),
      ));
    }
    MemoryStatus::Proposed | MemoryStatus::Superseded | MemoryStatus::Rejected | MemoryStatus::Withdrawn => {}
  }
  actions.push(action(
    "delete",
    Icon::Trash2,
    "memory.actions.delete",
    Press::Open(Opened::Delete(record)),
  ));
  actions
}

/// The record's actions as buttons (`O8wNd`).
fn actions(ctx: &mut Ctx, state: &MemoryState, memory: &Memory, id: &str) -> Element {
  let (record, revision) = (memory.id(), memory.revision());
  let buttons: Vec<Element> = actions_of(memory)
    .into_iter()
    .map(|action| {
      let state = state.clone();
      let press: Box<dyn Fn() + Send + Sync> = match action.press {
        Press::Open(opened) => Box::new(move || state.open(opened)),
        Press::Write(change) => Box::new(move || state.write(Write::Change(record, revision, change.clone()))),
      };
      IconButton::new(format!("{id}-{}", action.name), action.glyph, ctx.t(action.label))
        .inspector()
        .on_click(press)
        .render(ctx)
    })
    .collect();
  Row::new()
    .id(format!("{id}-actions"))
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .with_children(buttons)
    .into()
}

/// The room the record's actions take, kept while they are hidden so that the pointer coming onto the row moves
/// nothing in it: the design draws them in the row's trailing column beside the text (`I74BQb`), never over it.
fn actions_room(memory: &Memory, id: &str) -> Element {
  let count = actions_of(memory).len() as f32;
  let side = ControlHeight::Inspector.value();
  Row::new()
    .id(format!("{id}-actions-room"))
    .width(Dimension::Px(
      count * side + (count - 1.0) * ExtraSpacing::Tight.value(),
    ))
    .height(Dimension::Px(side))
    .into()
}
