//! *Edit* in place (09b `QJb0b`): the person's bubble becomes a field 520 wide (`Editor` `jiiRx`, `surface_input` in
//! a `border_focus` stroke, radius 10, 14 by 10 inside, its parts 10 apart) holding the words in 13 `text_primary`,
//! over its foot (`b38Jmu`): a 13 `message-square-x` in `warning` and *Sending drops the 3 messages after this one*
//! in 11.5 `text_secondary`, then *Cancel* and *Send*, 24 high. The design draws *Ctrl Enter sends* and *Esc cancels*
//! under the field (`Keys` `qSilt`); the app does not show them. Ctrl Enter (⌘ Enter on macOS) sends and Esc cancels in
//! the field too. The messages sending drops are dimmed to 40 % under the line *These 3 messages go when you send*
//! (`Drop line` `JRQTc`): a `border_strong` line at each side of a 12 `arrow-down` and the words in 11 `text_muted`, 10
//! apart.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextInput},
  core::ElementRef,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::workspace::OpenProject;

use super::{Failed, toolbar::RewindEnv};
use crate::{
  screens::task_form::brief::text_styles,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    tinted_note::person_only,
  },
};

/// The field's width (`Editor` `jiiRx`).
const EDITOR_WIDTH: f32 = 520.0;
/// The most lines the field grows to before its lines scroll.
const MAX_LINES: usize = 12;
/// A drop line's rules' height (`Y5pppa` in `JRQTc`).
const RULE_HEIGHT: f32 = 1.0;

/// The field of the message `id` being edited, which sending drops `after` messages after.
pub(in crate::screens::orchestrator) fn field(
  ctx: &mut Ctx,
  id: &str,
  (env, project): (&RewindEnv, &OpenProject),
  after: u32,
) -> Element {
  ctx.mount_keyed::<EditField>(
    &format!("orchestrator-edit-{id}"),
    FieldProps {
      id: Arc::from(id),
      after,
      env: env.clone(),
      project: project.clone(),
    },
  )
}

/// The line over the messages sending the edit drops: `count` of them.
pub(in crate::screens::orchestrator) fn drop_line(ctx: &Ctx, count: u32) -> Element {
  let words = if count == 1 {
    ctx.t("orchestrator.edit.dropLine.one")
  } else {
    ctx.t_args("orchestrator.edit.dropLine.other", [("count", count.to_string())])
  };
  let rule = || {
    Row::new()
      .flex(1.0)
      .height(Dimension::Px(RULE_HEIGHT))
      .background(PaletteColor::BorderStrong)
  };
  Row::new()
    .id("orchestrator-edit-drop-line")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(rule())
    .child(icon(Icon::ArrowDown, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .id("orchestrator-edit-drop-line-label")
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
    .child(rule())
    .into()
}

#[derive(Clone)]
struct FieldProps {
  id: Arc<str>,
  after: u32,
  env: RewindEnv,
  project: OpenProject,
}

/// Compared by what it shows; what sending does is the screen's.
impl PartialEq for FieldProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.after == other.after && self.env.shows() == other.env.shows()
  }
}

impl DevtoolsInspectable for FieldProps {}

struct EditField {
  input: ElementRef,
  /// The field took the focus when it opened.
  focused: lurq::core::Signal<bool>,
}

impl Component for EditField {
  type Props = FieldProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      input: ElementRef::new(),
      focused: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FieldProps>().clone();
    if !self.focused.get_untracked() {
      ctx.focus(&self.input);
      self.focused.set(true);
    }
    let editor = Column::new()
      .id(format!("{}-editor", props.id))
      .width(Dimension::Px(EDITOR_WIDTH))
      .max_width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        ExtraSpacing::Inset.role(),
        ExtraSpacing::Loose.role(),
      ))
      .spacing(ExtraSpacing::Loose.role())
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderFocus))
      .rounded(RadiusSize::Lg)
      .child(self.input(ctx, &props))
      .child(foot(ctx, &props));
    Column::new()
      .id(format!("{}-editing", props.id))
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::End)
      .child(editor)
      .with_children(why_not(ctx, &props))
  }
}

/// Why sending the edit without the dialog did nothing, while the dialog does not say it: `{id}-person-only` once
/// synthetic input sent it, `{id}-error` once the conversation could not go back.
fn why_not(ctx: &mut Ctx, props: &FieldProps) -> Option<Element> {
  let state = &props.env.state;
  if state.open.get() {
    return None;
  }
  if state.refused.get() {
    return Some(
      person_only(
        format!("{}-person-only", props.id),
        &ctx.t("orchestrator.rewind.personOnly"),
      )
      .into(),
    );
  }
  let action = state.action.state().get();
  let rejected = action.is_rejected();
  let Some(Failed::Unchanged(why)) = action.error.filter(|_| rejected) else {
    return None;
  };
  let reason = message_text(ctx, &why);
  Some(
    field_error(
      &format!("{}-error", props.id),
      &ctx.t_args("orchestrator.rewind.failed", [("reason", &*reason)]),
    )
    .into(),
  )
}

impl EditField {
  /// The words being edited, in 13 `text_primary`: Ctrl Enter sends them, Esc cancels the edit.
  fn input(&self, ctx: &mut Ctx, props: &FieldProps) -> TextInput {
    let [text, placeholder] = text_styles(ctx, TypographyStyle::Body, PaletteColor::TextPrimary);
    let state = props.env.state.clone();
    let (project, mode) = (props.project.clone(), props.env.mode);
    TextInput::styled(state.words.clone(), text)
      .placeholder_style(placeholder)
      .caret_color(PaletteColor::TextPrimary)
      .id(format!("{}-input", props.id))
      .describe("a11y_name", ctx.t("orchestrator.edit.field"))
      .textarea()
      .rows(1, MAX_LINES)
      .width(Dimension::Pct(100.0))
      .ref_element(self.input.clone())
      .on_key_down(move |event: KeyboardEvent| {
        if event.composing || event.default_prevented() {
          return;
        }
        if event.key == "Escape" {
          event.prevent_default();
          state.editing.set(None);
        } else if event.key == "Enter" && (event.ctrl || event.meta) {
          event.prevent_default();
          state.send_edit(&project, mode);
        }
      })
  }
}

/// What sending drops, then *Cancel* and *Send*; *Send* waits while the words say nothing or a rewind is under way.
fn foot(ctx: &mut Ctx, props: &FieldProps) -> Row {
  let state = props.env.state.clone();
  let drops = (props.after > 0).then(|| {
    let words = if props.after == 1 {
      ctx.t("orchestrator.edit.drops.one")
    } else {
      ctx.t_args("orchestrator.edit.drops.other", [("count", props.after.to_string())])
    };
    Text::new(&words)
      .id(format!("{}-drops", props.id))
      .variant(ExtraTypography::Note.role())
      .color(PaletteColor::TextSecondary)
  });
  let cancel_state = state.clone();
  let cancel = ActionButton::new(
    format!("{}-cancel", props.id),
    ButtonKind::Secondary,
    ctx.t("orchestrator.edit.cancel"),
  )
  .small()
  .on_click(move || cancel_state.editing.set(None));
  let (project, mode) = (props.project.clone(), props.env.mode);
  let send = ActionButton::new(
    format!("{}-send", props.id),
    ButtonKind::Primary,
    ctx.t("orchestrator.edit.send"),
  )
  .icon(Icon::Send)
  .small();
  let send = if state.action.state().get().is_pending() {
    send.loading()
  } else if state.words.get().trim().is_empty() {
    send.unavailable(ctx.t("orchestrator.edit.blank"))
  } else {
    send.on_click(move || state.send_edit(&project, mode))
  };
  Row::new()
    .id(format!("{}-foot", props.id))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(
      drops
        .is_some()
        .then(|| icon(Icon::MessageSquareX, IconSize::Base, PaletteColor::Warning)),
    )
    .child(Row::new().flex(1.0).min_width(Dimension::Px(0.0)).with_children(drops))
    .child(cancel)
    .child(send)
}
