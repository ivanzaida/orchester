//! A record's details, open in place under its row on the project's page (`Details` `dLiYt` and `Footer` `FBRTD` in
//! `aBtsx`, `TeKGM`), side by side 28 apart: *Author and source* — who wrote it and where, the agent's confidence and
//! the conversation it came from; *History* — the person's edit and its earlier versions; *Given to agents* — the
//! invocations that were given it. Under them its actions: Edit, Pin or Unpin, Deprecate and Delete on an active
//! record, Restore and Delete on a deprecated one, Delete only on a rejected, superseded or withdrawn one.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::MemoryId,
  memory::{GivenMemory, Memory, MemoryAuthor, MemoryStatus},
};
use orchester_workspace::{
  background,
  memory::{
    change::Change,
    listing,
    origin::Workplace,
    read::{GivenTo, ProjectMemory},
  },
  messages::{self, Message},
};

use super::{
  opened, parts,
  state::{MemoryState, Opened, Shared},
  words,
  work::Write,
};
use crate::{
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize, MemorySpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    inspector::{self, PropertyValue},
    message::message_text,
    pressable::pressable,
  },
};

/// The invocations listed before *Show all N invocations* (`pMJQF`).
const GIVEN_SHOWN: usize = 2;

#[derive(Clone, PartialEq)]
pub struct DetailsProps {
  pub memory: Arc<ProjectMemory>,
  pub record: MemoryId,
}

impl DevtoolsInspectable for DetailsProps {}

pub struct RecordDetails {
  /// Every invocation that was given the record is listed.
  all_given: Signal<bool>,
}

impl Component for RecordDetails {
  type Props = DetailsProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      all_given: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DetailsProps>().clone();
    let state = MemoryState::of(ctx);
    let Some(record) = props.memory.records.iter().find(|record| record.id() == props.record) else {
      return Element::new();
    };
    let given = given(ctx, &state, record);
    let columns = Row::new()
      .id("memory-details")
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .right(SpacingSize::Lg)
          .bottom(SpacingSize::Md)
          .left(MemorySpacing::RowText.role()),
      )
      .spacing(ExtraSpacing::Roomy.role())
      .align_items(Alignment::Start)
      .child(column(source(ctx, &state, &props.memory, record)))
      .with_children(history(ctx, &props.memory, record).map(column))
      .child(column(self.given_section(ctx, given)));
    Element::from(
      Column::new()
        .width(Dimension::Pct(100.0))
        .child(columns)
        .child(footer(ctx, &state, record)),
    )
  }
}

/// A part of the details, as wide as the others, a line over it and 14 above its title (`z21AE2`).
fn column(section: Column) -> Column {
  section
    .width(Dimension::Auto)
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .padding(Padding::new().top(ExtraSpacing::Inset.role()))
}

/// The invocations given `record`, read when it is chosen; `Err(None)` while they are read.
fn given(ctx: &mut Ctx, state: &MemoryState, record: &Memory) -> Result<Arc<GivenTo>, Option<Message>> {
  let hold = Services::of(ctx).hold;
  let read = ctx
    .future(
      (state.repo.clone(), record.id(), record.revision()),
      move |(repo, id, _)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || GivenTo::load_in(&repo, id).map(Shared::new))
            .await?
            .map_err(|error| messages::store_error(&error))
        }
      },
    )
    .state()
    .get();
  if let Some(error) = read.error.clone().filter(|_| read.is_rejected()) {
    return Err(Some(error));
  }
  read.data.map(|given| given.0).ok_or(None)
}

/// *Author and source*: *Author*, *Source*, *Transcript* and *Confidence*, and the conversation it came from
/// (`z21AE2`).
fn source(ctx: &mut Ctx, state: &MemoryState, memory: &ProjectMemory, record: &Memory) -> Column {
  let mut rows = Column::new().width(Dimension::Pct(100.0));
  let property = |id: &str, label: &str, value: &str| inspector::property(id, label, value, PropertyValue::Plain);
  match memory.origins.written(record) {
    Some(place) => {
      let agent = place.agent.clone().unwrap_or_default();
      rows = rows.child(property(
        "memory-details-author",
        &ctx.t("memory.inspector.author"),
        &ctx.t_args("memory.inspector.agent", [("agent", &*agent)]),
      ));
      let source = where_worked(ctx, place);
      rows = rows.child(property(
        "memory-details-source",
        &ctx.t("memory.editor.source"),
        &source,
      ));
      if !record.sources().is_empty() {
        rows = rows.child(property(
          "memory-details-transcript",
          &ctx.t("memory.inspector.transcript"),
          &ctx.t_args(
            "memory.inspector.entries",
            [("count", record.sources().len().to_string())],
          ),
        ));
      }
      rows = rows.child(property(
        "memory-details-confidence",
        &ctx.t("memory.inspector.confidence"),
        &ctx.t_args(
          "memory.inspector.confidenceValue",
          [
            ("confidence", &*words::confidence_title(ctx, record.confidence())),
            ("agent", &*agent),
          ],
        ),
      ));
    }
    None => {
      rows = rows.child(property(
        "memory-details-author",
        &ctx.t("memory.inspector.author"),
        &ctx.t("memory.inspector.person"),
      ));
    }
  }
  rows = rows.with_children(decided(ctx, record));
  let mut section = inspector::section("memory-details-source-section", SpacingSize::Sm)
    .child(inspector::section_title(
      "memory-details-source-title",
      &ctx.t("memory.inspector.source"),
      None,
    ))
    .child(rows);
  if let (MemoryAuthor::Agent(_), Some(task)) = (record.author(), opened::source_task(record, &memory.origins)) {
    let words = ctx.t_args("memory.inspector.showEntries", [("task", task.to_string())]);
    section = section.child(parts::conversation_link(
      ctx,
      (state.project_id, &task),
      ("memory-details-conversation", &words),
    ));
  }
  section
}

/// When a person deprecated or rejected it (`tQ6vV`, `ZqB2a`).
fn decided(ctx: &Ctx, record: &Memory) -> Option<Row> {
  let label = match record.status() {
    MemoryStatus::Deprecated => ctx.t("memory.status.deprecated"),
    MemoryStatus::Rejected => ctx.t("memory.status.rejected"),
    _ => return None,
  };
  let value = ctx.t_args(
    "memory.inspector.byYou",
    [("time", words::when(ctx, record.updated_at()))],
  );
  Some(inspector::property(
    "memory-details-decided",
    &label,
    &value,
    PropertyValue::Plain,
  ))
}

/// *History*: the person's last edit, and the earlier versions (`lvTgB`).
fn history(ctx: &Ctx, memory: &ProjectMemory, record: &Memory) -> Option<Column> {
  let earlier = listing::earlier_versions(record, &memory.records);
  if earlier.is_empty() && record.edited_at().is_none() {
    return None;
  }
  let meta = match earlier.len() {
    0 => None,
    1 => Some(ctx.t("memory.editor.versionsOne")),
    count => Some(ctx.t_args("memory.editor.versionsOther", [("count", count.to_string())])),
  };
  let mut section = inspector::section("memory-details-history", SpacingSize::Sm).child(inspector::section_title(
    "memory-details-history-title",
    &ctx.t("memory.inspector.history"),
    meta.as_deref(),
  ));
  if let Some(edited) = record.edited_at() {
    section = section.child(change(
      "memory-details-edited",
      Icon::Pencil,
      &ctx.t_args("memory.inspector.edited", [("time", words::when(ctx, edited))]),
    ));
  }
  let total = earlier.len();
  for (index, version) in earlier.iter().enumerate() {
    let head = ctx.t_args(
      "memory.inspector.version",
      [
        ("version", (total - index).to_string()),
        ("time", words::when(ctx, version.updated_at())),
      ],
    );
    section = section.child(
      Column::new()
        .id(format!("memory-details-version-{index}"))
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .left(ExtraSpacing::Loose.role())
            .top(ExtraSpacing::Tight.role())
            .bottom(ExtraSpacing::Tight.role()),
        )
        .spacing(ExtraSpacing::Fine.role())
        .border_left(Border::inside(BorderSize::Md, PaletteColor::BorderStrong))
        .child(
          Text::new(&head)
            .variant(TypographyStyle::FieldLabel)
            .color(PaletteColor::TextMuted),
        )
        .child(
          Text::new(version.content().text())
            .variant(ExtraTypography::Prose.role())
            .color(PaletteColor::TextMuted),
        )
        .child(Text::new(&words::meta(ctx, version, &memory.origins, true)).variant(TypographyStyle::Caption)),
    );
  }
  Some(section)
}

/// A change in a record's history: its glyph and what happened when (`Change row` `XoQDX`).
fn change(id: &str, glyph: Icon, words: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(words)
        .flex(1.0)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextSecondary),
    )
}

impl RecordDetails {
  /// *Given to agents*: how many invocations got it, the latest of them, and *Show all N invocations* (`JAJzt`).
  fn given_section(&self, ctx: &mut Ctx, given: Result<Arc<GivenTo>, Option<Message>>) -> Column {
    let section = inspector::section("memory-details-given", SpacingSize::Sm);
    let title = |meta: Option<&str>| {
      inspector::section_title("memory-details-given-title", &ctx.t("memory.inspector.given"), meta)
    };
    let given = match given {
      Ok(given) => given,
      Err(None) => {
        return section
          .child(title(None))
          .child(Text::new(&ctx.t("memory.inspector.givenReading")).variant(TypographyStyle::Caption));
      }
      Err(Some(reason)) => {
        return section.child(title(None)).child(crate::ui::field::field_error(
          "memory-details-given-error",
          &message_text(ctx, &reason),
        ));
      }
    };
    let count = given.invocations.len();
    let meta = if count == 1 {
      ctx.t("memory.inspector.invocationsOne")
    } else {
      ctx.t_args("memory.inspector.invocations", [("count", count.to_string())])
    };
    let mut section = section.child(title(Some(&meta)));
    if count == 0 {
      return section.child(Text::new(&ctx.t("memory.inspector.givenNone")).variant(TypographyStyle::Caption));
    }
    let all = self.all_given.get();
    let shown = if all { count } else { count.min(GIVEN_SHOWN) };
    for (index, invocation) in given.invocations.iter().take(shown).enumerate() {
      section = section.child(invocation_row(ctx, index, invocation, given.workplace(invocation)));
    }
    if count > GIVEN_SHOWN {
      let label = if all {
        ctx.t("memory.inspector.showFewer")
      } else {
        ctx.t_args("memory.inspector.showAll", [("count", count.to_string())])
      };
      let toggle = self.all_given.clone();
      section = section.child(
        pressable(move || toggle.update(|all| *all = !*all))
          .id("memory-details-given-all")
          .describe("a11y_name", label.clone())
          .child(Text::new(&label).variant(crate::theme::MemoryText::Link.role())),
      );
    }
    section
  }
}

/// Where an agent worked, without its role: *KON-8 · Run 104 · 02 Plan*.
fn where_worked(ctx: &Ctx, place: Workplace) -> Arc<str> {
  let mut parts = Vec::new();
  parts.extend(place.task.as_ref().map(ToString::to_string));
  let place = Workplace {
    agent: None,
    task: None,
    ..place
  };
  parts.extend(words::workplace(ctx, &place, false));
  words::join(ctx, &parts)
}

/// An invocation that was given the record: where it worked, and who, why, how many tokens and when (`gYTmk`).
fn invocation_row(ctx: &Ctx, index: usize, invocation: &GivenMemory, place: Workplace) -> Row {
  let mut why = Vec::new();
  why.extend(place.agent.as_deref().map(str::to_owned));
  why.push(reason(ctx, invocation.reason).to_string());
  why.push(
    ctx
      .t_args("memory.inspector.tokens", [("tokens", invocation.tokens.to_string())])
      .to_string(),
  );
  why.push(words::when(ctx, invocation.started_at));
  Row::new()
    .id(format!("memory-details-invocation-{index}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Fine.role()))
    .spacing(SpacingSize::Sm)
    .child(icon(Icon::Bot, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Hair.role())
        .child(
          Text::new(&where_worked(ctx, place))
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextPrimary),
        )
        .child(Text::new(&words::join(ctx, &why)).variant(TypographyStyle::Caption)),
    )
}

/// Why an invocation was given a record, as its details say it.
fn reason(ctx: &Ctx, reason: orchester_core::memory::InjectionReason) -> Arc<str> {
  use orchester_core::memory::InjectionReason;
  ctx.t(match reason {
    InjectionReason::Handoff => "memory.reason.handoff",
    InjectionReason::Pinned => "memory.reason.pinned",
    InjectionReason::Decision => "memory.reason.decision",
    InjectionReason::Warning => "memory.reason.warning",
    InjectionReason::Match => "memory.reason.match",
  })
}

/// The record's actions (`FBRTD`): Edit, Pin or Unpin and Deprecate, then Delete at the end; Restore on a deprecated
/// record.
fn footer(ctx: &Ctx, state: &MemoryState, record: &Memory) -> Row {
  let (id, revision) = (record.id(), record.revision());
  let busy = state.busy();
  let held = |button: ActionButton| if busy { button.disabled() } else { button };
  let write = |change: Change| {
    let state = state.clone();
    move || state.write(Write::Change(id, revision, change.clone()))
  };
  let mut row = Row::new()
    .id("memory-details-actions")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Loose.role())
        .right(SpacingSize::Lg)
        .bottom(ExtraSpacing::Loose.role())
        .left(MemorySpacing::RowText.role()),
    )
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border));
  match record.status() {
    MemoryStatus::Active => {
      let edit = state.clone();
      let (glyph, label) = if record.is_pinned() {
        (Icon::PinOff, "memory.actions.unpin")
      } else {
        (Icon::Pin, "memory.actions.pin")
      };
      row = row
        .child(held(
          ActionButton::new(
            "memory-details-edit",
            ButtonKind::Secondary,
            ctx.t("memory.actions.edit"),
          )
          .icon(Icon::Pencil)
          .on_click(move || edit.open(Opened::Edit(id))),
        ))
        .child(held(
          ActionButton::new("memory-details-pin", ButtonKind::Ghost, ctx.t(label))
            .icon(glyph)
            .on_click(write(Change::Pin(!record.is_pinned()))),
        ))
        .child(held(
          ActionButton::new(
            "memory-details-deprecate",
            ButtonKind::Ghost,
            ctx.t("memory.actions.deprecate"),
          )
          .icon(Icon::Archive)
          .on_click(write(Change::Deprecate)),
        ));
    }
    MemoryStatus::Deprecated => {
      row = row.child(held(
        ActionButton::new(
          "memory-details-restore",
          ButtonKind::Secondary,
          ctx.t("memory.actions.restore"),
        )
        .icon(Icon::RotateCcw)
        .on_click(write(Change::Restore)),
      ));
    }
    MemoryStatus::Proposed | MemoryStatus::Superseded | MemoryStatus::Rejected | MemoryStatus::Withdrawn => {}
  }
  let delete = state.clone();
  row.child(Spacer::new().flex(1.0)).child(held(
    ActionButton::new(
      "memory-details-delete",
      ButtonKind::DangerGhost,
      ctx.t("memory.actions.delete"),
    )
    .icon(Icon::Trash2)
    .on_click(move || delete.open(Opened::Delete(id))),
  ))
}
