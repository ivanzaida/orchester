//! The record editor on the project's page, as the record's own page (`05c Memory record` `CF0R7`, `05e Review
//! proposal` `Ybk74`): what approving replaces as a diff on top while a proposal is reviewed, then the *Record* group's
//! rows — *Type* and *Key · optional* in 240 fields, *Importance* as a segmented control, *Text* and *Tags* under their
//! labels, *Pinned · always give it to agents* with its switch — and the footer: what saving does, then *Cancel* and
//! *Save record*, or *Reject* and *Approve*. The page's crumb, head and side are the record page's
//! ([`crate::screens::memory::record_page`]).

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, SelectOption, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::memory::{Importance, MemoryKind};
use orchester_workspace::memory::{change::Change, listing::KIND_ORDER};

use super::{EditorMode, EditorProps, ImportanceChoice, KindChoice, RecordEditor};
use crate::{
  screens::{
    hub::parts,
    memory::{state::MemoryState, words, work::Write},
  },
  theme::{HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField},
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
    toggle::{Toggle, ToggleKind},
  },
};

/// A field's width in the record's rows (`x7cbm`, `RTAOi` in `Qw8g5`).
const FIELD_WIDTH: f32 = 240.0;
/// The text's box (`s4566J`).
const TEXT_HEIGHT: f32 = 64.0;

impl RecordEditor {
  /// The editor as the main column of the record's page.
  pub(super) fn page(&self, ctx: &mut Ctx, state: &MemoryState, props: &EditorProps, id: &str) -> Element {
    let mut column = Column::new()
      .id(id.to_owned())
      .width(Dimension::Pct(100.0))
      .describe("mode", super::mode_name(props.mode))
      .spacing(HubSpacing::Sections.role());
    if let Some(replaced) = &props.replaced {
      column = column.child(self.replaces(ctx, (replaced, props.memory.as_ref()), (id, true)));
    }
    let record = props.memory.as_ref().map(orchester_core::memory::Memory::id);
    column
      .child(self.record_rows(ctx, props, id))
      .with_children(state.failure_note(ctx, id, record))
      .child(self.page_footer(ctx, state, props, id))
      .into()
  }

  /// The *Record* group (`ymE2V`).
  fn record_rows(&self, ctx: &mut Ctx, props: &EditorProps, id: &str) -> Element {
    let kind_label = ctx.t("memory.editor.type");
    let kinds = KIND_ORDER
      .into_iter()
      .filter(|kind| *kind != MemoryKind::Handoff || self.kind.get().0 == MemoryKind::Handoff)
      .map(|kind| SelectOption::new(KindChoice(kind), words::kind_name(ctx, kind)))
      .collect();
    let kind = SelectField::new(format!("{id}-type"), kind_label.clone(), self.kind.clone(), kinds).render(ctx);
    let key_label = ctx.t("memory.editor.key");
    let key = FieldInput::new(format!("{id}-key"), key_label.clone(), self.key.clone())
      .mono()
      .render(ctx);
    let text = FieldInput::new(format!("{id}-text"), ctx.t("memory.editor.text"), self.text.clone())
      .text_area()
      .area_height(TEXT_HEIGHT)
      .render(ctx);
    let mut rows: Vec<Element> = vec![
      parts::field(&format!("{id}-type-field"), &kind_label, None, sized(kind)).into(),
      parts::field(&format!("{id}-key-field"), &key_label, None, sized(key)).into(),
      parts::field(
        &format!("{id}-importance-field"),
        &ctx.t("memory.editor.importance"),
        None,
        self.importance_segments(ctx, id),
      )
      .into(),
      parts::stacked_field(
        &format!("{id}-text-field"),
        (&ctx.t("memory.editor.text"), None),
        text,
        None,
      )
      .into(),
      parts::stacked_field(
        &format!("{id}-tags-row"),
        (&ctx.t("memory.editor.tags"), None),
        self.tags_field(ctx, id, false),
        None,
      )
      .into(),
    ];
    if props.mode == EditorMode::Edit {
      let pinned = self.pinned.clone();
      let title = ctx.t("memory.editor.pinned");
      let switch = Toggle::new(
        format!("{id}-pin"),
        ToggleKind::Switch,
        self.pinned.get(),
        title.clone(),
      )
      .hide_label()
      .on_change(move |on| pinned.set(on));
      rows.push(
        parts::field(
          &format!("{id}-pin-field"),
          &title,
          Some(&ctx.t("memory.editor.pinnedHintProject")),
          switch,
        )
        .into(),
      );
    }
    let group = "memory-record-group";
    parts::group(
      group,
      parts::head(group, &ctx.t("memory.editor.record"), None, None),
      parts::boxed(&format!("{id}-fields"), rows),
    )
    .into()
  }

  /// *Low · Normal · High* (`M7BjL`).
  fn importance_segments(&self, ctx: &Ctx, id: &str) -> Row {
    let chosen = self.importance.get().0;
    let segments = [Importance::Low, Importance::Normal, Importance::High].map(|importance| {
      let set = self.importance.clone();
      Segment::new(
        format!("{id}-importance-{}", importance_name(importance)),
        SegmentFace::Text(words::importance(ctx, importance)),
        chosen == importance,
        move || set.set(ImportanceChoice(importance)),
      )
    });
    segmented(&format!("{id}-importance"), segments, Focus::Keyboard)
  }

  /// What saving does, then *Cancel* and *Save record*, or *Reject* and *Approve* (`vj1hs`, `b6qp3c`).
  fn page_footer(&self, ctx: &mut Ctx, state: &MemoryState, props: &EditorProps, id: &str) -> Element {
    let record = props.memory.as_ref().map(orchester_core::memory::Memory::id);
    let busy = state.busy();
    let held = |button: ActionButton| if busy { button.disabled() } else { button };
    let note = ctx.t(match props.mode {
      EditorMode::Review => "memory.editor.approveNotePage",
      EditorMode::Edit | EditorMode::New => "memory.editor.saveNote",
    });
    let mut row = Row::new()
      .id(format!("{id}-footer"))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&note)
          .id(format!("{id}-note"))
          .variant(HubText::Hint.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      );
    match (&props.memory, props.mode) {
      (Some(memory), EditorMode::Review) => {
        let (state, record, revision) = (state.clone(), memory.id(), memory.revision());
        row = row.child(held(
          ActionButton::new(
            format!("{id}-reject"),
            ButtonKind::DangerGhost,
            ctx.t("memory.actions.reject"),
          )
          .on_click(move || state.write(Write::Change(record, revision, Change::Reject))),
        ));
      }
      _ => {
        let close = state.clone();
        row = row.child(held(
          ActionButton::new(format!("{id}-cancel"), ButtonKind::Ghost, ctx.t("memory.editor.cancel"))
            .on_click(move || close.leave_page()),
        ));
      }
    }
    let primary = self.primary(ctx, state, props, id);
    let primary = if state.pending_for(record) {
      primary.loading()
    } else {
      held(primary)
    };
    row.child(primary).into()
  }
}

/// A control in a record row's 240 field.
fn sized(control: Element) -> Row {
  Row::new()
    .width(Dimension::Px(FIELD_WIDTH))
    .flex_shrink(0.0)
    .child(control)
}

const fn importance_name(importance: Importance) -> &'static str {
  match importance {
    Importance::Low => "low",
    Importance::Normal => "normal",
    Importance::High => "high",
  }
}
