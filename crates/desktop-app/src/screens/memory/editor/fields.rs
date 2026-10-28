//! The editor's fields (`W1lQf` in `t0jt2k`, `RZJhD` in `MO5Mu`): *Type*, *Key · optional* and *Importance* on one
//! line, *Text*, *Tags* with *Add a tag…*; *Save to* for a new record on a task's tab; the notice of the record
//! approving replaces; *Pinned · always give it to agents*; and the facts.

use lurq::{
  app::{
    ctx::Ctx,
    events::TextInputEvent,
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, SelectOption, Text, TextInput},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::memory::{Importance, MemoryKind};
use orchester_workspace::memory::{draft::SaveTo, listing::KIND_ORDER};

use super::{EditorMode, EditorProps, ImportanceChoice, KindChoice, RecordEditor, SaveToChoice};
use crate::{
  screens::memory::{parts, state::MemoryState, words},
  theme::{ControlHeight, ExtraSpacing, ExtraTypography},
  ui::{
    chip::TagChip,
    field::{FieldInput, SelectField, field, field_error},
    message::message_text,
    toggle::{Toggle, ToggleKind},
  },
};

/// The widths of *Type* and *Importance* (`g0wn1w`, `iaPeX`), a fact's label (`L0cFs`) and *Save to* (`iosXE`).
const TYPE_WIDTH: f32 = 170.0;
const IMPORTANCE_WIDTH: f32 = 150.0;
const FACT_LABEL_WIDTH: f32 = 150.0;
const SAVE_TO_WIDTH: f32 = 260.0;
/// The text's box (`bOIk4`).
const TEXT_HEIGHT: f32 = 64.0;

impl RecordEditor {
  pub(super) fn fields(&self, ctx: &mut Ctx, state: &MemoryState, props: &EditorProps, id: &str) -> Element {
    let line = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(self.kind_field(ctx, id))
      .child(self.key_field(ctx, id))
      .child(self.importance_field(ctx, id));
    let text = FieldInput::new(format!("{id}-text"), ctx.t("memory.editor.text"), self.text.clone())
      .text_area()
      .area_height(TEXT_HEIGHT);
    let mut fields = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
      .spacing(ExtraSpacing::Inset.role())
      .child(line)
      .child(field(
        &format!("{id}-text-field"),
        Some(&ctx.t("memory.editor.text")),
        text.render(ctx),
        None,
      ))
      .child(self.tags_field(ctx, id, true));
    if props.mode == EditorMode::New && state.task.is_some() {
      fields = fields.child(self.save_to_field(ctx, state, id));
    }
    if let Some(replaced) = &props.replaced {
      let page = state.task.is_none();
      fields = fields.child(self.replaces(ctx, (replaced, props.memory.as_ref()), (id, page)));
    }
    if props.mode != EditorMode::Review {
      let of_task = state.task.is_some() && self.save_to.get().0 == SaveTo::Task;
      fields = fields.child(self.pin_field(ctx, id, of_task));
    }
    if !props.facts.is_empty() {
      fields = fields.child(facts(ctx, state, props, id));
    }
    fields.into()
  }

  fn kind_field(&self, ctx: &Ctx, id: &str) -> Element {
    let label = ctx.t("memory.editor.type");
    let options = KIND_ORDER
      .into_iter()
      .filter(|kind| *kind != MemoryKind::Handoff || self.kind.get().0 == MemoryKind::Handoff)
      .map(|kind| SelectOption::new(KindChoice(kind), words::kind_name(ctx, kind)))
      .collect();
    let select = SelectField::new(format!("{id}-type"), label.clone(), self.kind.clone(), options).render(ctx);
    Column::new()
      .width(Dimension::Px(TYPE_WIDTH))
      .flex_shrink(0.0)
      .child(field(&format!("{id}-type-field"), Some(&label), select, None))
      .into()
  }

  fn key_field(&self, ctx: &mut Ctx, id: &str) -> Element {
    let label = ctx.t("memory.editor.key");
    let input = FieldInput::new(format!("{id}-key"), label.clone(), self.key.clone()).mono();
    Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .child(field(&format!("{id}-key-field"), Some(&label), input.render(ctx), None))
      .into()
  }

  fn importance_field(&self, ctx: &Ctx, id: &str) -> Element {
    let label = ctx.t("memory.editor.importance");
    let options = [Importance::Low, Importance::Normal, Importance::High]
      .into_iter()
      .map(|importance| SelectOption::new(ImportanceChoice(importance), words::importance(ctx, importance)))
      .collect();
    let select = SelectField::new(
      format!("{id}-importance"),
      label.clone(),
      self.importance.clone(),
      options,
    )
    .render(ctx);
    Column::new()
      .width(Dimension::Px(IMPORTANCE_WIDTH))
      .flex_shrink(0.0)
      .child(field(&format!("{id}-importance-field"), Some(&label), select, None))
      .into()
  }

  /// *Tags*: the record's tags as removable chips in a field-like box, and *Add a tag…*, where a space, a comma or
  /// leaving the field makes what was typed a tag (`B0ra2`); under its `labelled` label, or under the row's on the
  /// record's page.
  pub(super) fn tags_field(&self, ctx: &Ctx, id: &str, labelled: bool) -> Element {
    let label = ctx.t("memory.editor.tags");
    let mut chips = Row::new()
      .id(format!("{id}-tags"))
      .width(Dimension::Pct(100.0))
      .min_height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::horizontal(ExtraSpacing::Snug.role()))
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md);
    for tag in self.tags.get() {
      let (tags, removed) = (self.tags.clone(), tag.clone());
      chips = chips.child(TagChip::new(format!("{id}-tag-{tag}"), tag.clone()).removable(
        ctx.t_args("memory.editor.removeTag", [("tag", tag.as_str())]),
        move || tags.update(|tags| tags.retain(|kept| *kept != removed)),
      ));
    }
    let mut column = Column::new()
      .id(format!("{id}-tags-field"))
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .with_children(labelled.then(|| Text::new(&label).variant(TypographyStyle::FieldLabel)))
      .child(chips.child(self.tag_input(ctx, id)));
    if let Some(problem) = self.tag_problem.get() {
      column = column.child(field_error(&format!("{id}-tag-error"), &message_text(ctx, &problem)));
    }
    column.into()
  }

  fn tag_input(&self, ctx: &Ctx, id: &str) -> TextInput {
    let theme = ctx.theme();
    let style = |color| {
      FormTextRole {
        typography: TypographyStyle::Description,
        color,
      }
      .resolve(&theme.typography(), &theme.palette())
    };
    let typed = self.clone_fields();
    let left = self.clone_fields();
    TextInput::styled(self.tag.clone(), style(PaletteColor::TextPrimary))
      .placeholder(&ctx.t("memory.editor.addTag"))
      .placeholder_style(style(PaletteColor::TextMuted))
      .caret_color(PaletteColor::TextPrimary)
      .id(format!("{id}-tag"))
      .describe("a11y_name", ctx.t("memory.editor.tags"))
      .flex(1.0)
      .single_line()
      .on_input(move |event: TextInputEvent| {
        let typed_now = event.new_value();
        if let Some(word) = typed_now.strip_suffix([' ', ',']) {
          event.prevent_default();
          typed.add_tag(word);
        }
      })
      .on_blur(move || {
        let word = left.tag.get_untracked();
        left.add_tag(&word);
      })
  }

  /// Makes `word` a tag, unless it is blank; says why when it is no tag.
  fn add_tag(&self, word: &str) {
    let word = word.trim();
    if word.is_empty() {
      self.tag.set(String::new());
      return;
    }
    match self.draft().with_tag(word) {
      Ok(draft) => {
        self.tags.set(draft.tags);
        self.tag.set(String::new());
        self.tag_problem.set(None);
      }
      Err(problem) => self.tag_problem.set(Some(problem.message())),
    }
  }

  /// *Save to*: this task or the project, and that a person's record is active at once (`iosXE`).
  fn save_to_field(&self, ctx: &Ctx, state: &MemoryState, id: &str) -> Element {
    let label = ctx.t("memory.editor.saveTo");
    let task = state.task.as_ref().map(|task| task.key.to_string()).unwrap_or_default();
    let options = vec![
      SelectOption::new(
        SaveToChoice(SaveTo::Task),
        ctx.t_args("memory.editor.saveToTask", [("task", task.as_str())]),
      ),
      SelectOption::new(
        SaveToChoice(SaveTo::Project),
        ctx.t_args("memory.editor.saveToProject", [("project", &*state.project)]),
      ),
    ];
    let select = SelectField::new(format!("{id}-save-to"), label.clone(), self.save_to.clone(), options).render(ctx);
    Column::new()
      .id(format!("{id}-save-to-field"))
      .width(Dimension::Px(SAVE_TO_WIDTH))
      .child(field(
        &format!("{id}-save-to-field"),
        Some(&label),
        select,
        Some(&ctx.t("memory.editor.saveToHint")),
      ))
      .into()
  }

  /// *Pinned · always give it to agents* with its switch (`PyiL4`), and whose invocations a pinned record reaches:
  /// this task's when it is saved to the task (`z2JrY`), else the project's.
  fn pin_field(&self, ctx: &Ctx, id: &str, of_task: bool) -> Element {
    let pinned = self.pinned.clone();
    let title = ctx.t("memory.editor.pinned");
    let hint = ctx.t(if of_task {
      "memory.editor.pinnedHint"
    } else {
      "memory.editor.pinnedHintProject"
    });
    Row::new()
      .id(format!("{id}-pin-field"))
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(
        Row::new().padding(Padding::new().top(ExtraSpacing::Hair.role())).child(
          Toggle::new(
            format!("{id}-pin"),
            ToggleKind::Switch,
            self.pinned.get(),
            title.clone(),
          )
          .hide_label()
          .on_change(move |on| pinned.set(on)),
        ),
      )
      .child(
        Column::new()
          .flex(1.0)
          .spacing(ExtraSpacing::Tight.role())
          .child(Text::new(&title).variant(TypographyStyle::Label))
          .child(Text::new(&hint).variant(TypographyStyle::Caption)),
      )
      .into()
  }
}

/// The facts the person does not change, a label and its value each (`OVIEu`, `EMaJF`); the source's
/// *Show in Conversation ↗* under its value.
fn facts(ctx: &mut Ctx, state: &MemoryState, props: &EditorProps, id: &str) -> Element {
  let mut facts = Column::new().id(format!("{id}-facts")).width(Dimension::Pct(100.0));
  for (index, (label, value)) in props.facts.iter().enumerate() {
    let mut values = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Tight.role())
      .child(
        Text::new(value)
          .variant(ExtraTypography::Prose.role())
          .color(PaletteColor::TextPrimary),
      );
    if index == 0
      && let Some(task) = &props.source_task
    {
      // The project's page lists every task's proposals, so its link names the task (`I7UAFO` in `kq8Am`).
      let words = if state.task.is_none() {
        ctx.t_args("memory.editor.sourceLinkTask", [("task", task.to_string())])
      } else {
        ctx.t("memory.editor.sourceLink")
      };
      values = values.child(parts::conversation_link(
        ctx,
        (state.project_id, task),
        (&format!("{id}-source-link"), &words),
      ));
    }
    facts = facts.child(
      Row::new()
        .id(format!("{id}-fact-{index}"))
        .width(Dimension::Pct(100.0))
        .padding(Padding::vertical(SpacingSize::Xs))
        .spacing(SpacingSize::Md)
        .child(
          Text::new(label)
            .width(Dimension::Px(FACT_LABEL_WIDTH))
            .flex_shrink(0.0)
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextMuted),
        )
        .child(values),
    );
  }
  facts.into()
}
