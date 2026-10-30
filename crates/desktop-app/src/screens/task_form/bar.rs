//! The action bar at the bottom of the content panel (`Action bar` `IWxaO`): 56 high, 28 at its sides, a `border`
//! line over it. At its start one line, the single reason the next action cannot be pressed or what will happen
//! (`super::reason`); at its end the actions and the key cap of the main one: *Create* and *Create & run* with
//! `Ctrl Enter` on New task, *Discard changes* and *Save changes* with `Ctrl S` on Edit task (a draft keeps *Create &
//! run*), *Retry run* while the queued task's run has not started. While an action is under way its button turns its
//! loader and none can be pressed. *Discard changes* asks in place of the bar (`LVI8K`), never in a dialog.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  reason::reason,
  state::{Phase, Subject, View},
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, TaskFormSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    kbd::key_cap,
    tooltip::Truncated,
  },
};

/// The bar's height (`Action bar` `IWxaO`); the module form's bar (`super::super::modules`) is as tall.
pub(in crate::screens) const HEIGHT: f32 = 56.0;

pub type Handler = Arc<dyn Fn() + Send + Sync>;

/// What the bar's buttons do.
pub struct Handlers {
  /// Create, or Save changes; `None` while the values cannot be saved.
  pub save: Option<Handler>,
  /// Create & run, or Retry run; `None` while neither can be asked for.
  pub run: Option<Handler>,
  pub ask_discard: Handler,
  pub keep_editing: Handler,
  pub discard: Handler,
  /// Save chosen versions; `None` until every conflicting field is chosen.
  pub save_chosen: Option<Handler>,
  pub back_to_editing: Handler,
}

/// The main action's keys on this platform: Create & run's, or Save changes' where there is no Create & run.
pub fn shortcut_key(ctx: &Ctx, runs: bool) -> Arc<str> {
  let mac = cfg!(target_os = "macos");
  ctx.t(match (runs, mac) {
    (true, false) => "task_form.keys.ctrlEnter",
    (true, true) => "task_form.keys.cmdEnter",
    (false, false) => "task_form.keys.ctrlS",
    (false, true) => "task_form.keys.cmdS",
  })
}

pub fn action_bar(ctx: &mut Ctx, view: &View, confirming_discard: bool, on: &Handlers) -> Element {
  if confirming_discard {
    return discard_question(ctx, view, on);
  }
  let actions = match &view.phase {
    Phase::NotStarted { .. } => vec![retry_button(ctx, false, on)],
    Phase::Starting(_) if view.retrying => vec![retry_button(ctx, true, on)],
    _ if view.conflict.is_some() => conflict_actions(ctx, view, on),
    _ => actions(ctx, view, on),
  };
  frame()
    .id("task-form-bar")
    .with_children(reason_line(ctx, view))
    .child(
      Row::new()
        .id("task-form-actions")
        .flex_shrink(0.0)
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Center)
        .with_children(actions),
    )
    .into()
}

/// The bar's frame, which New module and Edit module put their actions in too.
pub(in crate::screens) fn frame() -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEIGHT))
    .flex_shrink(0.0)
    .padding(Padding::horizontal(TaskFormSpacing::Side.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
}

/// The line at the bar's start: its 12 icon and its words in 12, cut to one line with the whole in a tooltip.
fn reason_line(ctx: &mut Ctx, view: &View) -> Vec<Element> {
  let Some(reason) = reason(ctx, view) else {
    return vec![Row::new().flex(1.0).into()];
  };
  let (glyph, color, role) = reason.kind.look();
  let words = Truncated::new(
    "task-form-reason-text",
    reason.words.clone(),
    TypographyStyle::Description,
  )
  .color(if role == "problem" {
    PaletteColor::TextSecondary
  } else {
    PaletteColor::TextMuted
  })
  .render(ctx);
  vec![
    Row::new()
      .id("task-form-reason")
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .describe("role", role)
      .describe("reason", reason.words)
      .child(icon(glyph, IconSize::Sm, color))
      .child(Row::new().flex_shrink(1.0).min_width(Dimension::Px(0.0)).child(words))
      .into(),
  ]
}

fn actions(ctx: &Ctx, view: &View, on: &Handlers) -> Vec<Element> {
  let busy = view.locked();
  let saving = matches!(view.phase, Phase::Saving);
  let running = matches!(view.phase, Phase::Creating | Phase::Starting(_));
  let edits = !view.subject.is_new();
  let offers_run = !matches!(view.subject, Subject::Ran(_));
  let mut actions = Vec::new();
  // New task offers Discard only for edits kept from an earlier visit (`Fd1ai`).
  if (edits || view.kept_at.is_some()) && view.changed && !busy {
    actions.push(button(
      ActionButton::new(
        "task-form-discard",
        ButtonKind::Ghost,
        ctx.t("task_form.actions.discard"),
      )
      .icon(Icon::Undo2),
      Some(on.ask_discard.clone()),
      (false, false),
    ));
  }
  let (save_id, save_key, save_icon) = if edits {
    ("task-form-save", "task_form.actions.saveChanges", Icon::Check)
  } else {
    ("task-form-create", "task_form.actions.create", Icon::Plus)
  };
  let save_ready = if edits {
    view.save_changes_needs().is_some_and(|needs| needs.is_empty())
  } else {
    view.create_needs().is_empty()
  };
  let save_kind = if offers_run {
    ButtonKind::Secondary
  } else {
    ButtonKind::Primary
  };
  actions.push(button(
    ActionButton::new(save_id, save_kind, ctx.t(save_key)).icon(save_icon),
    on.save.clone(),
    (saving, busy || !save_ready),
  ));
  if offers_run {
    actions.push(button(
      ActionButton::new(
        "task-form-run",
        ButtonKind::Primary,
        ctx.t("task_form.actions.createAndRun"),
      )
      .icon(Icon::Play),
      on.run.clone(),
      (running, busy || !view.run_needs().is_empty()),
    ));
  }
  actions.push(key_cap("task-form-shortcut", &shortcut_key(ctx, offers_run)));
  actions
}

/// `button` running `handler`, turning its loader while `loading`, faded while `disabled` or without
/// a handler.
fn button(button: ActionButton, handler: Option<Handler>, (loading, disabled): (bool, bool)) -> Element {
  let button = button.in_form();
  if loading {
    return button.loading().into();
  }
  match handler.filter(|_| !disabled) {
    Some(handler) => button.on_click(move || handler()).into(),
    None => button.disabled().into(),
  }
}

fn retry_button(ctx: &Ctx, loading: bool, on: &Handlers) -> Element {
  button(
    ActionButton::new("task-form-retry", ButtonKind::Primary, ctx.t("task_form.queued.retry")).icon(Icon::RotateCcw),
    on.run.clone(),
    (loading, false),
  )
}

/// Back to editing and Save chosen versions (`Action bar` `E2h20` in `PenUu`).
fn conflict_actions(ctx: &Ctx, view: &View, on: &Handlers) -> Vec<Element> {
  let saving = matches!(view.phase, Phase::Saving);
  vec![
    button(
      ActionButton::new(
        "task-form-back-to-editing",
        ButtonKind::Secondary,
        ctx.t("task_form.conflict.back"),
      ),
      Some(on.back_to_editing.clone()),
      (false, saving),
    ),
    button(
      ActionButton::new(
        "task-form-save-chosen",
        ButtonKind::Primary,
        ctx.t("task_form.conflict.save"),
      ),
      on.save_chosen.clone(),
      (saving, false),
    ),
  ]
}

/// Discard changes, asked in place of the bar (`Action bar` `B926Jt` in `LVI8K`).
fn discard_question(ctx: &Ctx, view: &View, on: &Handlers) -> Element {
  let key = view
    .subject
    .task()
    .map(|task| task.key().to_string())
    .unwrap_or_default();
  let (question, consequence) = match view.subject {
    Subject::New => ("task_form.discard.questionNew", "task_form.discard.consequenceNew"),
    Subject::Draft(_) => ("task_form.discard.question", "task_form.discard.consequence"),
    Subject::Ran(_) => ("task_form.discard.question", "task_form.discard.consequenceRan"),
  };
  let question = Row::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::TriangleAlert, IconSize::Nav, PaletteColor::Warning))
    .child(
      Column::new()
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&ctx.t_args(question, [("key", key)]))
            .id("task-form-discard-question")
            .variant(ExtraTypography::Strong.role()),
        )
        .child(Text::new(&ctx.t(consequence)).variant(TypographyStyle::Description)),
    );
  let keep = button(
    ActionButton::new(
      "task-form-keep-editing",
      ButtonKind::Secondary,
      ctx.t("task_form.actions.keepEditing"),
    ),
    Some(on.keep_editing.clone()),
    (false, false),
  );
  let discard = button(
    ActionButton::new(
      "task-form-confirm-discard",
      ButtonKind::Danger,
      ctx.t("task_form.actions.discard"),
    ),
    Some(on.discard.clone()),
    (false, false),
  );
  frame()
    .id("task-form-bar")
    .describe("state", "confirm-discard")
    .background(PaletteColor::SurfaceRaised)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .child(question)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(keep)
        .child(discard),
    )
    .into()
}
