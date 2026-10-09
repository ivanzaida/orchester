//! The action bar under the form (`Action bar` `mqcqC`): a note at the start, the actions at the end and,
//! under them, why an action cannot be pressed (`s7jKkA`, `i4Y9hL`, `Qp1B0`) or why the last one failed
//! (`H8GYVZ`). While an action is under way its button turns its loader and none can be pressed
//! (`J2QjVq`). *Discard changes* asks in place of the bar (`LVI8K`), never in a dialog.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  FORM_WIDTH,
  state::{Need, Phase, Subject, View},
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
  },
};

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
}

pub fn action_bar(ctx: &Ctx, view: &View, confirming_discard: bool, on: &Handlers) -> Element {
  if confirming_discard {
    return discard_question(ctx, view, on);
  }
  let (note, actions, reasons) = match &view.phase {
    Phase::NotStarted { problems, .. } => (
      Some(ctx.t("task_form.queued.retryNote")),
      vec![retry_button(ctx, false, on)],
      problems
        .iter()
        .map(|problem| problem_line(&message_text(ctx, problem)))
        .collect(),
    ),
    Phase::Starting(_) if view.retrying => (
      Some(ctx.t("task_form.queued.retryNote")),
      vec![retry_button(ctx, true, on)],
      Vec::new(),
    ),
    _ => (note(ctx, view), actions(ctx, view, on), reasons(ctx, view)),
  };
  frame()
    .id("task-form-bar")
    .child(
      Row::new()
        .width(Dimension::Px(FORM_WIDTH))
        .spacing(SpacingSize::Lg)
        .align_items(Alignment::Start)
        .child(Column::new().flex(1.0).with_children(note.map(|note| {
          Text::new(&note)
            .id("task-form-bar-note")
            .variant(ExtraTypography::Prose.role())
            .color(PaletteColor::TextMuted)
        })))
        .child(
          Column::new()
            .max_width(Dimension::Px(FORM_WIDTH))
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::End)
            .child(
              Row::new()
                .id("task-form-actions")
                .spacing(SpacingSize::Sm)
                .align_items(Alignment::Center)
                .with_children(actions),
            )
            .with_children((!reasons.is_empty()).then(|| {
              Column::new()
                .id("task-form-reasons")
                .spacing(SpacingSize::Xs)
                .align_items(Alignment::End)
                .with_children(reasons)
            })),
        ),
    )
    .into()
}

fn frame() -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Xl, SpacingSize::Md))
    .justify(Justify::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
}

/// What the bar says at its start: what is under way, why Create & run is not offered, or where kept
/// changes came from.
fn note(ctx: &Ctx, view: &View) -> Option<Arc<str>> {
  match (&view.phase, &view.subject) {
    (Phase::Creating, _) => Some(ctx.t("task_form.progress.creatingForRun")),
    (Phase::Starting(_), _) => Some(ctx.t("task_form.progress.starting")),
    (Phase::Saving, Subject::New) => Some(ctx.t("task_form.progress.creatingDraft")),
    (Phase::Editing, Subject::Draft(_) | Subject::Ran(_)) if view.changed && view.kept_at.is_some() => {
      let at = view
        .kept_at
        .map(orchester_workspace::display::clock)
        .unwrap_or_default();
      Some(ctx.t_args("task_form.kept.note", [("time", at)]))
    }
    (_, Subject::Ran(_)) => Some(ctx.t("task_form.reasons.ranNoRun")),
    _ => None,
  }
}

fn actions(ctx: &Ctx, view: &View, on: &Handlers) -> Vec<Element> {
  let busy = view.locked();
  let saving = matches!(view.phase, Phase::Saving);
  let running = matches!(view.phase, Phase::Creating | Phase::Starting(_));
  let mut actions = Vec::new();
  let edits = !view.subject.is_new();
  if edits && view.changed && !busy {
    actions.push(button(
      ActionButton::new(
        "task-form-discard",
        ButtonKind::Secondary,
        ctx.t("task_form.actions.discard"),
      ),
      Some(on.ask_discard.clone()),
      (false, false),
    ));
  }
  let offers_run = !matches!(view.subject, Subject::Ran(_));
  let (save_id, save_key) = if edits {
    ("task-form-save", "task_form.actions.saveChanges")
  } else {
    ("task-form-create", "task_form.actions.create")
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
    ActionButton::new(save_id, save_kind, ctx.t(save_key)),
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

/// Why each action cannot be pressed now, then why the last save or Create & run failed.
fn reasons(ctx: &Ctx, view: &View) -> Vec<Element> {
  let mut lines = Vec::new();
  if !view.locked() {
    let edits = !view.subject.is_new();
    let save = if edits {
      match view.save_changes_needs() {
        None => Some(ctx.t("task_form.reasons.nothingChanged")),
        Some(needs) => reason(ctx, "task_form.reasons.save", &needs),
      }
    } else {
      reason(ctx, "task_form.reasons.create", &view.create_needs())
    };
    lines.extend(save.map(|text| lock_line("task-form-save-reason", &text)));
    if !matches!(view.subject, Subject::Ran(_)) {
      let run = reason(ctx, "task_form.reasons.run", &view.run_needs());
      lines.extend(run.map(|text| lock_line("task-form-run-reason", &text)));
    }
  }
  for failure in view.failure.iter().flat_map(|failures| failures.iter()) {
    lines.push(problem_line(&message_text(ctx, failure)));
  }
  lines
}

/// "<action> unavailable: <what it needs>.", or `None` when it needs nothing.
fn reason(ctx: &Ctx, key: &str, needs: &[Need]) -> Option<Arc<str>> {
  let words: Vec<Arc<str>> = needs.iter().map(|need| ctx.t(need.key())).collect();
  let what = list(ctx, &words)?;
  Some(ctx.t_args(key, [("what", what.as_ref())]))
}

/// `items` as a list in words: "a", "a and b", "a, b and c".
fn list(ctx: &Ctx, items: &[Arc<str>]) -> Option<Arc<str>> {
  match items {
    [] => None,
    [one] => Some(one.clone()),
    [first, second] => Some(ctx.t_args(
      "task_form.reasons.list.two",
      [("first", first.as_ref()), ("second", second.as_ref())],
    )),
    [first, rest @ ..] => {
      let rest = list(ctx, rest)?;
      Some(ctx.t_args(
        "task_form.reasons.list.more",
        [("first", first.as_ref()), ("rest", rest.as_ref())],
      ))
    }
  }
}

/// Why an action cannot be pressed: a lock and the words, 12 `text_muted` (`Reason` `SACr6`).
fn lock_line(id: &str, text: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("role", "unavailable-reason")
    .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(text)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex_shrink(1.0),
    )
    .into()
}

/// What went wrong, e.g. the runner's refusal: a `warning` mark and the words in `text_secondary`
/// (`Runner reason` `sp6x2`).
fn problem_line(text: &str) -> Element {
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("role", "problem")
    .child(icon(Icon::CircleAlert, IconSize::Base, PaletteColor::Warning))
    .child(Text::new(text).variant(TypographyStyle::Description).flex_shrink(1.0))
    .into()
}

/// Discard changes, asked in place of the bar (`Action bar` `B926Jt` in `LVI8K`).
fn discard_question(ctx: &Ctx, view: &View, on: &Handlers) -> Element {
  let key = view
    .subject
    .task()
    .map(|task| task.key().to_string())
    .unwrap_or_default();
  let consequence = if matches!(view.subject, Subject::Ran(_)) {
    "task_form.discard.consequenceRan"
  } else {
    "task_form.discard.consequence"
  };
  let question = Row::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .child(icon(Icon::TriangleAlert, IconSize::Nav, PaletteColor::Warning))
    .child(
      Column::new()
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&ctx.t_args("task_form.discard.question", [("key", key)]))
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
    .child(
      Row::new()
        .width(Dimension::Px(FORM_WIDTH))
        .spacing(SpacingSize::Lg)
        .align_items(Alignment::Center)
        .child(question)
        .child(
          Row::new()
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::Center)
            .child(keep)
            .child(discard),
        ),
    )
    .into()
}
