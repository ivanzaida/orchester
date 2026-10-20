//! A record's editor, open in the record's place (`Memory / Record editor · review` `t0jt2k`, `Memory / Record
//! editor · edit` `MO5Mu`), or above the list for *New record*: its head names what is edited and by whom, then the
//! type, key, importance, text and tags, what approving replaces, the facts the person does not change (source and
//! the agent's confidence; author and versions), and the footer's actions. *Review proposed record* saves the
//! person's edits with the approval; *Edit* saves them in place, keeping the author; *New record* writes a person's
//! record, active at once, to this task or the project. One component for the three: mode is a prop.

mod fields;
mod replaces;

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
use orchester_core::memory::{Importance, Memory, MemoryKind};
use orchester_workspace::memory::{
  change::Change,
  draft::{RecordDraft, SaveTo},
};

use super::{
  parts,
  state::{MemoryState, Opened},
  words,
  work::Write,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    icon_button::IconButton,
  },
};

/// What the editor does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorMode {
  /// *Review proposed record*: approve it with the person's edits, or reject it.
  Review,
  /// *Edit*: change what an active record says in place.
  Edit,
  /// *New record*.
  New,
}

#[derive(Clone, PartialEq)]
pub struct EditorProps {
  pub mode: EditorMode,
  /// The record reviewed or edited; `None` for a new one.
  pub memory: Option<Memory>,
  /// Who and where, under the title (`cEtf8`).
  pub subtitle: Arc<str>,
  /// The active record approving the proposal replaces.
  pub replaced: Option<Replaced>,
  /// The facts under the fields: their labels and values (`OVIEu`).
  pub facts: Arc<[(Arc<str>, Arc<str>)]>,
  /// The task whose conversation *Show in Conversation ↗* under the first fact opens, when the record came from one.
  pub source_task: Option<orchester_core::project::TaskKey>,
}

impl DevtoolsInspectable for EditorProps {}

/// The active record approving a proposal replaces (`i09ZK` in `t0jt2k`, `nbYtu` in `kq8Am`).
#[derive(Clone, PartialEq)]
pub struct Replaced {
  pub memory: Memory,
  /// Who wrote it where and when.
  pub meta: Arc<str>,
  /// The agent that proposed the record reviewed.
  pub proposer: Option<Arc<str>>,
  /// How many invocations were given it, where the page read it (the project's page).
  pub given: Option<usize>,
}

/// A record's type as a select holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindChoice(pub MemoryKind);
impl DevtoolsInspectable for KindChoice {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportanceChoice(pub Importance);
impl DevtoolsInspectable for ImportanceChoice {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveToChoice(pub SaveTo);
impl DevtoolsInspectable for SaveToChoice {}

pub struct RecordEditor {
  kind: Signal<KindChoice>,
  key: Signal<String>,
  text: Signal<String>,
  tags: Signal<Vec<String>>,
  /// What the tag field holds before it becomes a tag.
  tag: Signal<String>,
  /// Why the last tag typed is no tag.
  tag_problem: Signal<Option<orchester_workspace::messages::Message>>,
  importance: Signal<ImportanceChoice>,
  pinned: Signal<bool>,
  save_to: Signal<SaveToChoice>,
}

impl Component for RecordEditor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<EditorProps>().clone();
    let draft = props.memory.as_ref().map_or_else(RecordDraft::blank, RecordDraft::of);
    let task_tab = MemoryState::of(ctx).task.is_some();
    Self {
      kind: ctx.signal(KindChoice(draft.kind)),
      key: ctx.signal(draft.key),
      text: ctx.signal(draft.text),
      tags: ctx.signal(draft.tags),
      tag: ctx.signal(String::new()),
      tag_problem: ctx.signal(None),
      importance: ctx.signal(ImportanceChoice(draft.importance)),
      pinned: ctx.signal(props.memory.as_ref().is_some_and(Memory::is_pinned)),
      save_to: ctx.signal(SaveToChoice(if task_tab { SaveTo::Task } else { SaveTo::Project })),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<EditorProps>().clone();
    let state = MemoryState::of(ctx);
    let id = editor_id(&props);
    Column::new()
      .id(id.clone())
      .width(Dimension::Pct(100.0))
      .describe("mode", mode_name(props.mode))
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .rounded(lurq::app::theme::RadiusSize::Lg)
      .child(self.head(ctx, &state, &props, &id))
      .child(self.fields(ctx, &state, &props, &id))
      .child(self.footer(ctx, &state, &props, &id))
  }
}

/// The editor's element id: of the record it edits, or of *New record*.
fn editor_id(props: &EditorProps) -> String {
  match &props.memory {
    Some(memory) => format!("memory-editor-{}", memory.id()),
    None => "memory-editor-new".to_owned(),
  }
}

const fn mode_name(mode: EditorMode) -> &'static str {
  match mode {
    EditorMode::Review => "review",
    EditorMode::Edit => "edit",
    EditorMode::New => "new",
  }
}

impl RecordEditor {
  /// What the fields say now, as a record's draft.
  fn draft(&self) -> RecordDraft {
    RecordDraft {
      kind: self.kind.get_untracked().0,
      key: self.key.get_untracked(),
      text: self.text.get_untracked(),
      tags: self.tags.get_untracked(),
      importance: self.importance.get_untracked().0,
    }
  }

  /// The glyph, the title and who and where, the status, and the fold (`F01ciQ`).
  fn head(&self, ctx: &mut Ctx, state: &MemoryState, props: &EditorProps, id: &str) -> Element {
    let kind = self.kind.get().0;
    let lower = words::kind_lower(ctx, kind);
    let title = match props.mode {
      EditorMode::Review => ctx.t_args("memory.editor.reviewTitle", [("kind", &*lower)]),
      EditorMode::Edit => ctx.t_args("memory.editor.editTitle", [("kind", &*lower)]),
      EditorMode::New => ctx.t("memory.editor.newTitle"),
    };
    let status = match (props.mode, &props.memory) {
      (EditorMode::Review, _) => Some(parts::status(
        &format!("{id}-status"),
        (Icon::Hand, ctx.t("memory.proposal.waiting")),
        PaletteColor::Warning,
      )),
      (EditorMode::Edit, Some(memory)) if memory.edited_at().is_some() => Some(parts::status(
        &format!("{id}-status"),
        (Icon::Pencil, ctx.t("memory.editor.editedByYou")),
        PaletteColor::TextSecondary,
      )),
      _ => None,
    };
    let close = state.clone();
    let fold = IconButton::new(format!("{id}-close"), Icon::ChevronUp, ctx.t("memory.editor.close"))
      .inspector()
      .on_click(move || close.close())
      .render(ctx);
    let mut titles = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Tight.role())
      .child(
        Text::new(&title)
          .id(format!("{id}-title"))
          .variant(ExtraTypography::FormSection.role()),
      );
    if !props.subtitle.is_empty() {
      titles = titles.child(Text::new(&props.subtitle).variant(TypographyStyle::Caption));
    }
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Md))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(icon(words::glyph(kind), IconSize::Md, PaletteColor::TextSecondary))
      .child(titles)
      .with_children(status)
      .child(fold)
      .into()
  }

  /// The footer (`SK5qH`, `a8HTeY`): what the mode does besides saving at its start, a note, *Cancel* and the
  /// primary action; why the last change failed above them.
  fn footer(&self, ctx: &mut Ctx, state: &MemoryState, props: &EditorProps, id: &str) -> Element {
    let record = props.memory.as_ref().map(Memory::id);
    let pending = state.pending_for(record);
    let busy = state.busy();
    let held = |button: ActionButton| if busy { button.disabled() } else { button };
    let mut row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center);
    row = row.with_children(self.secondary(ctx, state, props, id).into_iter().map(&held));
    if props.mode == EditorMode::Review {
      // The project's page keeps the note short, so the footer fits its narrower column (`uEOgZ` in `kq8Am`).
      let note = ctx.t(if state.task.is_none() {
        "memory.editor.rejectNoteShort"
      } else {
        "memory.editor.rejectNote"
      });
      row = row.child(
        Text::new(&note)
          .id(format!("{id}-reject-note"))
          .variant(TypographyStyle::Caption),
      );
    }
    let note = ctx.t(match props.mode {
      EditorMode::Review => "memory.editor.approveNote",
      EditorMode::Edit | EditorMode::New => "memory.editor.saveNote",
    });
    let close = state.clone();
    let cancel = held(
      ActionButton::new(format!("{id}-cancel"), ButtonKind::Ghost, ctx.t("memory.editor.cancel"))
        .on_click(move || close.close()),
    );
    let primary = self.primary(ctx, state, props, id);
    let primary = if pending { primary.loading() } else { held(primary) };
    row = row
      .child(Spacer::new().flex(1.0))
      .child(Text::new(&note).variant(TypographyStyle::Caption))
      .child(cancel)
      .child(primary);
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Md))
      .spacing(SpacingSize::Sm)
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .with_children(state.failure_note(ctx, id, record))
      .child(row)
      .into()
  }

  /// *Reject* on a proposal; *Deprecate* and *Delete…* on an active record.
  fn secondary(&self, ctx: &Ctx, state: &MemoryState, props: &EditorProps, id: &str) -> Vec<ActionButton> {
    let Some(memory) = &props.memory else {
      return Vec::new();
    };
    let (record, revision) = (memory.id(), memory.revision());
    let write = |change: Change| {
      let state = state.clone();
      move || state.write(Write::Change(record, revision, change.clone()))
    };
    match props.mode {
      EditorMode::Review => vec![
        ActionButton::new(
          format!("{id}-reject"),
          ButtonKind::Ghost,
          ctx.t("memory.actions.reject"),
        )
        .icon(Icon::X)
        .on_click(write(Change::Reject)),
      ],
      EditorMode::Edit => {
        let delete = state.clone();
        vec![
          ActionButton::new(
            format!("{id}-deprecate"),
            ButtonKind::Ghost,
            ctx.t("memory.actions.deprecate"),
          )
          .icon(Icon::Archive)
          .on_click(write(Change::Deprecate)),
          ActionButton::new(
            format!("{id}-delete"),
            ButtonKind::DangerGhost,
            ctx.t("memory.editor.delete"),
          )
          .icon(Icon::Trash2)
          .on_click(move || delete.open(Opened::Delete(record))),
        ]
      }
      EditorMode::New => Vec::new(),
    }
  }

  /// *Approve* or *Save record*; it cannot be pressed while the text is blank.
  fn primary(&self, ctx: &Ctx, state: &MemoryState, props: &EditorProps, id: &str) -> ActionButton {
    let label = ctx.t(match props.mode {
      EditorMode::Review => "memory.actions.approve",
      EditorMode::Edit | EditorMode::New => "memory.editor.save",
    });
    let button = ActionButton::new(format!("{id}-save"), ButtonKind::Primary, label).icon(Icon::Check);
    if self.text.get().trim().is_empty() {
      return button.disabled();
    }
    let (fields, state, props) = (self.clone_fields(), state.clone(), props.clone());
    button.on_click(move || {
      let draft = fields.draft();
      let pinned = fields.pinned.get_untracked();
      let write = match (&props.memory, props.mode) {
        (Some(memory), EditorMode::Review) => {
          Write::Change(memory.id(), memory.revision(), Change::Approve(Some(draft)))
        }
        (Some(memory), _) => Write::Change(memory.id(), memory.revision(), Change::Edit { draft, pinned }),
        (None, _) => {
          let task = state.task.as_ref().map(|task| task.id);
          Write::Create(fields.save_to.get_untracked().0.scope(task), draft, pinned)
        }
      };
      state.write(write);
    })
  }

  /// The editor's fields, for a press to read when it lands.
  fn clone_fields(&self) -> Self {
    Self {
      kind: self.kind.clone(),
      key: self.key.clone(),
      text: self.text.clone(),
      tags: self.tags.clone(),
      tag: self.tag.clone(),
      tag_problem: self.tag_problem.clone(),
      importance: self.importance.clone(),
      pinned: self.pinned.clone(),
      save_to: self.save_to.clone(),
    }
  }
}

/// The editor of `opened`, when it is one of `memory`.
pub fn opened_on(opened: Opened, memory: &Memory) -> Option<EditorMode> {
  match opened {
    Opened::Review(id) if id == memory.id() => Some(EditorMode::Review),
    Opened::Edit(id) if id == memory.id() => Some(EditorMode::Edit),
    _ => None,
  }
}
