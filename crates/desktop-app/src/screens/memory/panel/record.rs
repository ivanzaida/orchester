//! The inspector of the record a press chose on the project's page (`Inspector · db.engine` `X8Jpna`): its type
//! and key; its status, pin and importance with its tags; what it says; who wrote it and where, the agent's
//! confidence and the conversation it came from; its history (the person's edit, its earlier versions); the
//! invocations that were given it; and its actions — Edit, Pin or Unpin, Deprecate and Delete on an active record,
//! Restore and Delete on a deprecated one (`s2nyY`), Delete only on a rejected, superseded or withdrawn one (`K6aZ2j`).

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
  memory::{GivenMemory, Importance, Memory, MemoryAuthor, MemoryStatus},
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

use super::{body, frame, header};
use crate::{
  screens::memory::{
    opened, parts,
    state::{MemoryState, Opened, Shared},
    words,
    work::Write,
  },
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    inspector::{self, PropertyValue},
    message::message_text,
    pressable::pressable,
  },
};

/// The invocations listed before *Show all N invocations* (`i5kaZV`).
const GIVEN_SHOWN: usize = 2;

#[derive(Clone, PartialEq)]
pub struct InspectorProps {
  pub memory: Arc<ProjectMemory>,
  pub record: MemoryId,
}

impl DevtoolsInspectable for InspectorProps {}

pub struct RecordInspector {
  /// Every invocation that was given the record is listed.
  all_given: Signal<bool>,
}

impl Component for RecordInspector {
  type Props = InspectorProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      all_given: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<InspectorProps>().clone();
    let state = MemoryState::of(ctx);
    let Some(record) = props.memory.records.iter().find(|record| record.id() == props.record) else {
      return Element::new();
    };
    let given = given(ctx, &state, record);
    let key = record
      .content()
      .key()
      .map_or_else(String::new, |key| key.as_str().to_owned());
    let title = words::kind_name(ctx, record.content().kind());
    let mut details = body()
      .child(status_line(ctx, record))
      .child(
        Text::new(record.content().text())
          .id("memory-inspector-text")
          .variant(ExtraTypography::Lead.role())
          .color(if record.status() == MemoryStatus::Active {
            PaletteColor::TextPrimary
          } else {
            PaletteColor::TextMuted
          }),
      )
      .child(source(ctx, &state, &props.memory, record));
    details = details.with_children(history(ctx, &props.memory, record));
    details = details.child(self.given_section(ctx, given));
    details = details.with_children(closed_note(ctx, record));
    frame(
      "memory-inspector",
      header(words::glyph(record.content().kind()), &title, &key),
      details,
      Some(footer(ctx, &state, record)),
    )
  }
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

/// Its status, whether it is pinned and how important, and its tags (`yRl7q`).
fn status_line(ctx: &Ctx, record: &Memory) -> Row {
  let (glyph, words, color) = match words::closed_status(ctx, record.status()) {
    Some((glyph, words)) => (glyph, words, PaletteColor::TextSecondary),
    None => (Icon::CircleCheck, ctx.t("memory.status.active"), PaletteColor::Success),
  };
  let mut line = Row::new()
    .id("memory-inspector-status")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(parts::status("memory-inspector-state", (glyph, words), color));
  if record.is_pinned() {
    line = line.child(parts::status(
      "memory-inspector-pinned",
      (Icon::Pin, ctx.t("memory.status.pinned")),
      PaletteColor::TextSecondary,
    ));
  }
  if record.content().importance() != Importance::Normal {
    let glyph = if record.content().importance() == Importance::High {
      Icon::ChevronsUp
    } else {
      Icon::ChevronDown
    };
    line = line.child(parts::status(
      "memory-inspector-importance",
      (glyph, words::importance(ctx, record.content().importance())),
      PaletteColor::TextSecondary,
    ));
  }
  line
    .child(Spacer::new().flex(1.0))
    .with_children(parts::tags("memory-inspector", record))
}

/// *Author*, *Source*, *Transcript* and *Confidence*, and the conversation it came from (`Bl1Or`).
fn source(ctx: &mut Ctx, state: &MemoryState, memory: &ProjectMemory, record: &Memory) -> Element {
  let mut rows = Column::new().width(Dimension::Pct(100.0));
  let property = |id: &str, label: &str, value: &str| inspector::property(id, label, value, PropertyValue::Plain);
  match memory.origins.written(record) {
    Some(place) => {
      let agent = place.agent.clone().unwrap_or_default();
      rows = rows.child(property(
        "memory-inspector-author",
        &ctx.t("memory.inspector.author"),
        &ctx.t_args("memory.inspector.agent", [("agent", &*agent)]),
      ));
      let source = where_worked(ctx, place);
      rows = rows.child(property(
        "memory-inspector-source",
        &ctx.t("memory.editor.source"),
        &source,
      ));
      if !record.sources().is_empty() {
        rows = rows.child(property(
          "memory-inspector-transcript",
          &ctx.t("memory.inspector.transcript"),
          &ctx.t_args(
            "memory.inspector.entries",
            [("count", record.sources().len().to_string())],
          ),
        ));
      }
      rows = rows.child(property(
        "memory-inspector-confidence",
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
        "memory-inspector-author",
        &ctx.t("memory.inspector.author"),
        &ctx.t("memory.inspector.person"),
      ));
    }
  }
  rows = rows.with_children(decided(ctx, record));
  let mut section = inspector::section("memory-inspector-source-section", SpacingSize::Sm).child(rows);
  if let (MemoryAuthor::Agent(_), Some(task)) = (record.author(), opened::source_task(record, &memory.origins)) {
    let words = ctx.t_args("memory.inspector.showEntries", [("task", task.to_string())]);
    section = section.child(parts::conversation_link(
      ctx,
      (state.project_id, &task),
      ("memory-inspector-conversation", &words),
    ));
  }
  section.into()
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
    "memory-inspector-decided",
    &label,
    &value,
    PropertyValue::Plain,
  ))
}

/// *History*: the person's last edit, and the earlier versions (`u94wW`).
fn history(ctx: &Ctx, memory: &ProjectMemory, record: &Memory) -> Option<Element> {
  let earlier = listing::earlier_versions(record, &memory.records);
  if earlier.is_empty() && record.edited_at().is_none() {
    return None;
  }
  let meta = match earlier.len() {
    0 => None,
    1 => Some(ctx.t("memory.editor.versionsOne")),
    count => Some(ctx.t_args("memory.editor.versionsOther", [("count", count.to_string())])),
  };
  let mut section = inspector::section("memory-inspector-history", SpacingSize::Sm).child(inspector::section_title(
    "memory-inspector-history-title",
    &ctx.t("memory.inspector.history"),
    meta.as_deref(),
  ));
  if let Some(edited) = record.edited_at() {
    section = section.child(change(
      "memory-inspector-edited",
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
        .id(format!("memory-inspector-version-{index}"))
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
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextMuted),
        )
        .child(Text::new(&words::meta(ctx, version, &memory.origins, true)).variant(TypographyStyle::Caption)),
    );
  }
  Some(section.into())
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

impl RecordInspector {
  /// *Given to agents*: how many invocations got it, the latest of them, and *Show all N invocations* (`RU2uX`).
  fn given_section(&self, ctx: &mut Ctx, given: Result<Arc<GivenTo>, Option<Message>>) -> Element {
    let section = inspector::section("memory-inspector-given", SpacingSize::Sm);
    let given = match given {
      Ok(given) => given,
      Err(None) => {
        return section
          .child(inspector::section_title(
            "memory-inspector-given-title",
            &ctx.t("memory.inspector.given"),
            None,
          ))
          .child(Text::new(&ctx.t("memory.inspector.givenReading")).variant(TypographyStyle::Caption))
          .into();
      }
      Err(Some(reason)) => {
        return section
          .child(inspector::section_title(
            "memory-inspector-given-title",
            &ctx.t("memory.inspector.given"),
            None,
          ))
          .child(crate::ui::field::field_error(
            "memory-inspector-given-error",
            &message_text(ctx, &reason),
          ))
          .into();
      }
    };
    let count = given.invocations.len();
    let meta = if count == 1 {
      ctx.t("memory.inspector.invocationsOne")
    } else {
      ctx.t_args("memory.inspector.invocations", [("count", count.to_string())])
    };
    let mut section = section.child(inspector::section_title(
      "memory-inspector-given-title",
      &ctx.t("memory.inspector.given"),
      Some(&meta),
    ));
    if count == 0 {
      return section
        .child(Text::new(&ctx.t("memory.inspector.givenNone")).variant(TypographyStyle::Caption))
        .into();
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
          .id("memory-inspector-given-all")
          .describe("a11y_name", label.clone())
          .child(Text::new(&label).variant(crate::theme::MemoryText::Link.role())),
      );
    }
    section.into()
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
    .id(format!("memory-inspector-invocation-{index}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Fine.role()))
    .spacing(SpacingSize::Sm)
    .child(icon(Icon::Bot, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Hair.role())
        .child(
          Text::new(&where_worked(ctx, place))
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextPrimary),
        )
        .child(Text::new(&words::join(ctx, &why)).variant(TypographyStyle::Caption)),
    )
}

/// Why an invocation was given a record, as the inspector says it.
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

/// What a deprecated or rejected record means now (`wMDvA`, `fG9SC`).
fn closed_note(ctx: &Ctx, record: &Memory) -> Option<Element> {
  let (glyph, words) = match record.status() {
    MemoryStatus::Deprecated => (
      Icon::Archive,
      match record.content().key() {
        Some(key) => ctx.t_args("memory.inspector.deprecatedKeyed", [("key", key.as_str())]),
        None => ctx.t("memory.inspector.deprecated"),
      },
    ),
    MemoryStatus::Rejected => (Icon::Ban, ctx.t("memory.inspector.rejected")),
    _ => return None,
  };
  Some(
    inspector::section("memory-inspector-notes", SpacingSize::Sm)
      .child(inspector::note("memory-inspector-note", glyph, &words))
      .into(),
  )
}

/// The record's actions (`b3veQ`): Edit, Pin or Unpin and Deprecate, then Delete at the end; Restore on a deprecated
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
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      SpacingSize::Lg,
      crate::theme::ExtraSpacing::Loose.role(),
    ))
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
            "memory-inspector-edit",
            ButtonKind::Secondary,
            ctx.t("memory.actions.edit"),
          )
          .icon(Icon::Pencil)
          .on_click(move || edit.open(Opened::Edit(id))),
        ))
        .child(held(
          ActionButton::new("memory-inspector-pin", ButtonKind::Ghost, ctx.t(label))
            .icon(glyph)
            .on_click(write(Change::Pin(!record.is_pinned()))),
        ))
        .child(held(
          ActionButton::new(
            "memory-inspector-deprecate",
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
          "memory-inspector-restore",
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
      "memory-inspector-delete",
      ButtonKind::DangerGhost,
      ctx.t("memory.actions.delete"),
    )
    .icon(Icon::Trash2)
    .on_click(move || delete.open(Opened::Delete(id))),
  ))
}
