//! The record editor in the Memory filter's panel (`Detail · editor` `p639jn` in Y3 `hnMhi`), in place of the record
//! while the list stays: *Memory* and where the record stands over *Edit decision*, *Review proposed decision* or *New
//! record*; the fields as the task's editor has them — *Type*, *Key · optional* and *Importance* on one line, *Text*,
//! *Tags* with *Add a tag…*, what approving replaces, *Pinned · always give it to agents* with its hint — then what the
//! person does not change under *Record*; and pinned at the bottom *Save record* or *Approve*, *Cancel*, what saving
//! does, and *Delete…* or *Reject* at the end.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Row, Text, TextOverflow},
  node::{Element, dimension::Dimension},
};
use orchester_core::memory::Memory;
use orchester_workspace::memory::change::Change;

use super::{EditorMode, EditorProps, RecordEditor, mode_name};
use crate::{
  screens::{
    memory::{
      hub::panel::{body, frame, status_word},
      parts,
      state::{MemoryState, Opened},
      words,
      work::Write,
    },
    team_hub::{bar::bar_ending, panel::head_named, rows},
  },
  theme::HubText,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

impl RecordEditor {
  /// The editor as the panel of the record it edits, or of *New record*.
  pub(super) fn panel(&self, ctx: &mut Ctx, state: &MemoryState, props: &EditorProps, id: &str) -> Element {
    if self.focus_text.get_untracked() {
      ctx.focus(&self.text_input);
      self.focus_text.set(false);
    }
    let body = body()
      .id(format!("{id}-body"))
      .child(self.panel_head(ctx, props))
      .with_children(self.field_list(ctx, state, props, id))
      .with_children(record_section(ctx, state, props, id));
    let record = props.memory.as_ref().map(Memory::id);
    let failure = state.failure_note(ctx, id, record);
    frame(
      id,
      (mode_name(props.mode), body),
      failure,
      self.panel_bar(ctx, state, props, id),
    )
  }

  /// *Memory* and where the record stands (*Active · webhook-retry*) over what is done to it (`Head` `Q03dN0`).
  fn panel_head(&self, ctx: &Ctx, props: &EditorProps) -> Row {
    let kind = words::kind_lower(ctx, self.kind.get().0);
    let title = match props.mode {
      EditorMode::Review => ctx.t_args("memory.editor.reviewTitle", [("kind", &*kind)]),
      EditorMode::Edit => ctx.t_args("memory.editor.editTitle", [("kind", &*kind)]),
      EditorMode::New => ctx.t("memory.editor.newTitle"),
    };
    let stands = props.memory.as_ref().map(|memory| {
      let mut parts = vec![status_word(ctx, memory.status()).to_string()];
      parts.extend(memory.content().key().map(|key| key.as_str().to_owned()));
      words::join(ctx, &parts).to_string()
    });
    head_named(&title, (ctx.t("memory.title").to_string(), stands), None)
  }

  /// *Save record* or *Approve*, *Cancel*, what saving does, and *Delete…* on an active record or *Reject* on a
  /// proposal (`Actions` `Q7T0g0`).
  fn panel_bar(&self, ctx: &Ctx, state: &MemoryState, props: &EditorProps, id: &str) -> Element {
    let record = props.memory.as_ref().map(Memory::id);
    let busy = state.busy();
    let held = |button: ActionButton| if busy { button.disabled() } else { button };
    let primary = self.primary(ctx, state, props, id).pinned();
    let primary = if state.pending_for(record) {
      primary.loading()
    } else {
      held(primary)
    };
    let close = state.clone();
    let cancel = held(
      ActionButton::new(
        format!("{id}-cancel"),
        ButtonKind::Secondary,
        ctx.t("memory.editor.cancel"),
      )
      .pinned()
      .on_click(move || close.close()),
    );
    let note = ctx.t(match props.mode {
      EditorMode::Review => "memory.editor.approveNote",
      EditorMode::Edit | EditorMode::New => "memory.editor.saveNote",
    });
    let end = props.memory.as_ref().and_then(|memory| {
      let (record, revision, state) = (memory.id(), memory.revision(), state.clone());
      let end = match props.mode {
        EditorMode::Review => ActionButton::new(
          format!("{id}-reject"),
          ButtonKind::DangerGhost,
          ctx.t("memory.actions.reject"),
        )
        .icon(Icon::X)
        .on_click(move || state.write(Write::Change(record, revision, Change::Reject))),
        EditorMode::Edit => ActionButton::new(
          format!("{id}-delete"),
          ButtonKind::DangerGhost,
          ctx.t("memory.editor.delete"),
        )
        .icon(Icon::Trash2)
        .on_click(move || state.open(Opened::Delete(record))),
        EditorMode::New => return None,
      };
      Some(held(end.pinned()).into())
    });
    bar_ending(
      &format!("{id}-actions"),
      vec![primary.into(), cancel.into()],
      Some((note.to_string(), lurq::app::theme::PaletteColor::TextMuted)),
      end,
    )
  }
}

/// *Record*: what the person does not change, a row each (`Record` `lThsB`), the source's with *Show in Conversation
/// ↗*; none for a new record.
fn record_section(ctx: &mut Ctx, state: &MemoryState, props: &EditorProps, id: &str) -> Option<Element> {
  if props.facts.is_empty() {
    return None;
  }
  let mut link = props.source_task.as_ref().map(|task| {
    parts::conversation_link(
      ctx,
      (state.project_id, task),
      (&format!("{id}-source-link"), &ctx.t("memory.editor.sourceLink")),
    )
  });
  let rows = props
    .facts
    .iter()
    .enumerate()
    .map(|(index, (label, value))| {
      let fact = format!("{id}-fact-{index}");
      // A long value ends in an ellipsis where the row ends (`Author` `Qfewq` in Y3).
      let said = Text::new(value)
        .id(format!("{fact}-value"))
        .variant(HubText::Value.role())
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis);
      let value = Row::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Sm)
        .child(said)
        .with_children(if index == 0 { link.take() } else { None });
      rows::row(&fact, label, None, value)
    })
    .collect();
  Some(
    rows::section(
      &format!("{id}-record"),
      &ctx.t("memory.editor.record"),
      Vec::new(),
      rows,
    )
    .into(),
  )
}
