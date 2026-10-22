//! Writing a new thread on chosen lines or on the whole pull request (`Feedback composer · Request a fix` `IJYbw`):
//! what it asks (*Ask a question* or *Request a fix*), to which agent, where, the words, what sending does (`Run
//! line` `r9nHF`), and *Cancel* and the send button; a run that ended says that sending follows it up (`elqd8`), and
//! a pull request that moved on while the person wrote asks where the request applies before it goes
//! ([`super::moved`]). Ctrl+Enter (Cmd+Enter) sends, Enter adds a line, Esc closes and keeps the draft.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, SelectOption, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  diff::Diff,
  id::ResourceId,
  pull_request::feedback::{FeedbackAnchor, FeedbackKind, LineAnchor},
  time::Timestamp,
};
use orchester_workspace::{
  display::short_revision,
  pull_requests::feedback::{BoardTask, Gate, NewThread, ThreadState},
};

use super::{
  Composing, Draft, FeedbackState, Write,
  moved::{changed, moved},
  parts::visibility,
  thread::Env,
  words,
};
use crate::{
  theme::{DISABLED_OPACITY, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField},
    icon::{Icon, icon},
    kbd::key_cap,
    notice::{Notice, NoticeTone},
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
    unavailable::reason_line,
  },
};

/// The words' box (`IBFli` in `TlVQu`) and the agent's select (`SMJMA`).
const AREA_HEIGHT: f32 = 56.0;
const TARGET_WIDTH: f32 = 160.0;

/// What sending does when pressed.
pub(super) type Press = Arc<dyn Fn() + Send + Sync>;

/// A new thread's composer.
pub struct NewProps {
  pub env: Env,
  /// Lines of a file, or the whole pull request.
  pub composing: Composing,
  /// The code of the chosen lines as the diff shows them; empty for the whole pull request.
  pub excerpt: Vec<String>,
  /// The pull request's diff at its head, where the lines are looked for when the head moved on.
  pub latest: Option<Arc<Diff>>,
}

/// Opens the composer of `composing`, its draft going to `default` unless the person picked another agent.
pub fn open(state: &FeedbackState, composing: Composing, default: Option<ResourceId>) {
  let draft = state.drafts.of(&composing, FeedbackKind::Question);
  if draft.target.get_untracked().is_none() {
    draft.target.set(default);
  }
  state.follow.set(false);
  state.composing.set(Some(composing));
}

pub fn new_thread(ctx: &mut Ctx, props: &NewProps) -> Element {
  let env = &props.env;
  let Some(task) = env.board.task.clone() else {
    return Element::new();
  };
  let draft = env.state.drafts.of(&props.composing, FeedbackKind::Question);
  let kind = draft.kind.get();
  let moved = moved(props);
  let send = send_new(env, (&props.composing, anchor(props)), &draft);
  let blocked = blocked(ctx, (&task, &draft), moved.is_some());
  let label = send_label(ctx, &task.gate, kind);
  let press = (blocked.is_none() && !env.writes.busy()).then(|| send.clone());
  let mut column = Column::new()
    .id("feedback-composer")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(ExtraRadius::Tile.role())
    .on_key_down(keys(&env.state, press.clone()))
    .child(head(ctx, (&task, &draft)))
    .child(anchor_row(ctx, props, &draft))
    .child(words_box(ctx, "feedback-composer-text", (&task, &draft)));
  if let Gate::FollowUp { ended_at } = task.gate {
    column = column.child(ended(ctx, &task, (ended_at, &draft)));
  }
  if let Some(moved) = &moved {
    column = column.child(changed(ctx, props, moved));
  }
  column
    .child(run_line(ctx, env, (&task, &draft)))
    .child(foot(ctx, env, (blocked, label, press)))
    .into()
}

/// What the new thread is about: the chosen lines with their code, or the whole pull request.
fn anchor(props: &NewProps) -> Option<FeedbackAnchor> {
  match &props.composing {
    Composing::Lines { path, side, span, .. } => LineAnchor::new(path, *side, *span, props.excerpt.clone())
      .ok()
      .map(FeedbackAnchor::Lines),
    Composing::PullRequest => Some(FeedbackAnchor::PullRequest),
    Composing::Reply(_) => None,
  }
}

/// Why the draft cannot be sent now; `None` when it can; an empty reason while only its words are missing.
fn blocked(ctx: &Ctx, (task, draft): (&BoardTask, &Draft), moved: bool) -> Option<Arc<str>> {
  if let Gate::Closed(state) = task.gate {
    return Some(ctx.t_args(words::closed_key(state), [("task", task.key.to_string())]));
  }
  if moved {
    return Some(ctx.t("pull_requests.feedback.composer.changed.reason"));
  }
  draft.text.get().trim().is_empty().then(|| Arc::from(""))
}

fn send_label(ctx: &Ctx, gate: &Gate, kind: FeedbackKind) -> Arc<str> {
  match (gate, kind) {
    (Gate::FollowUp { .. }, _) => ctx.t("pull_requests.feedback.composer.sendFollowUp"),
    (_, FeedbackKind::Question) => ctx.t("pull_requests.feedback.composer.sendQuestion"),
    (_, FeedbackKind::Fix) => ctx.t("pull_requests.feedback.composer.sendFix"),
  }
}

/// Sends the new thread as written.
fn send_new(env: &Env, (composing, anchor): (&Composing, Option<FeedbackAnchor>), draft: &Draft) -> Press {
  let (writes, number, head) = (env.writes.clone(), env.number.get(), env.head.clone());
  let (composing, draft) = (composing.clone(), draft.clone());
  Arc::new(move || {
    let (Some(anchor), Some(recipient)) = (anchor.clone(), draft.target.get_untracked()) else {
      return;
    };
    let revision = match &composing {
      Composing::Lines { revision, .. } => Some(revision.clone()),
      _ => head.clone(),
    };
    let Some(revision) = revision else {
      return;
    };
    let new = NewThread {
      number,
      anchor,
      kind: draft.kind.get_untracked(),
      text: draft.text.get_untracked(),
      revision,
      recipient,
    };
    writes.run(Write::Start(draft.key, new), Some(composing.clone()));
  })
}

/// Ctrl+Enter (Cmd+Enter) sends; Esc closes and keeps the draft.
pub(super) fn keys(state: &FeedbackState, press: Option<Press>) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  let composing = state.composing.clone();
  move |event: KeyboardEvent| {
    if event.composing {
      return;
    }
    if event.key == "Escape" {
      event.prevent_default();
      composing.set(None);
      return;
    }
    if event.key == "Enter" && (event.ctrl || event.meta) {
      event.prevent_default();
      if let Some(press) = &press {
        press();
      }
    }
  }
}

/// What it asks, and to which agent of the task.
fn head(ctx: &mut Ctx, (task, draft): (&BoardTask, &Draft)) -> Row {
  let kind = draft.kind.get();
  let segment = |id: &str, key: &str, value: FeedbackKind| {
    let chosen = draft.kind.clone();
    Segment::new(id.to_owned(), SegmentFace::Text(ctx.t(key)), kind == value, move || {
      chosen.set(value)
    })
  };
  let mode = segmented(
    "feedback-composer-mode",
    [
      segment(
        "feedback-composer-ask",
        "pull_requests.feedback.composer.ask",
        FeedbackKind::Question,
      ),
      segment(
        "feedback-composer-fix",
        "pull_requests.feedback.composer.fix",
        FeedbackKind::Fix,
      ),
    ],
    Focus::Keyboard,
  );
  let targets = task
    .targets
    .iter()
    .map(|target| {
      SelectOption::new(
        Some(target.profile),
        ctx.t_args(
          "pull_requests.feedback.composer.target",
          [("agent", target.role.to_string()), ("task", task.key.to_string())],
        ),
      )
    })
    .collect();
  let target = SelectField::new(
    "feedback-composer-target",
    ctx.t("pull_requests.feedback.composer.to"),
    draft.target.clone(),
    targets,
  )
  .leading(Icon::Bot)
  .width(Dimension::Px(TARGET_WIDTH))
  .render(ctx);
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(mode)
    .child(Row::new().flex(1.0))
    .child(
      Text::new(&ctx.t("pull_requests.feedback.composer.to"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(target)
}

/// Where the thread is about and, once something is written, when the draft was last kept.
fn anchor_row(ctx: &Ctx, props: &NewProps, draft: &Draft) -> Row {
  let anchor = match &props.composing {
    Composing::Lines {
      path,
      side,
      span,
      revision,
    } => {
      let at = ctx.t_args(
        "pull_requests.feedback.anchor.at",
        [
          ("lines", words::lines(ctx, (Some(*side), *span)).to_string()),
          ("revision", short_revision(revision.as_str()).to_owned()),
        ],
      );
      ctx.t_args(
        "pull_requests.feedback.anchor.file",
        [("path", path.to_string()), ("anchor", at.to_string())],
      )
    }
    _ => {
      let head = props.env.head.as_ref();
      let revision = head
        .map(|head| short_revision(head.as_str()).to_owned())
        .unwrap_or_default();
      ctx.t_args("pull_requests.feedback.anchor.whole", [("revision", revision)])
    }
  };
  let saved = draft.saved_at.get().map(|at| {
    Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Save, IconSize::Sm, PaletteColor::TextMuted))
      .child(
        Text::new(&ctx.t_args("pull_requests.feedback.composer.saved", [("time", words::time(at))]))
          .id("feedback-composer-saved")
          .variant(TypographyStyle::Caption)
          .nowrap(),
      )
  });
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&anchor)
        .id("feedback-composer-anchor")
        .variant(ExtraTypography::MonoNote.role())
        .nowrap(),
    )
    .child(Row::new().flex(1.0))
    .with_children(saved)
}

/// The words, in a box that grows with its lines; typing keeps the draft's time.
pub(super) fn words_box(ctx: &mut Ctx, id: &str, (task, draft): (&BoardTask, &Draft)) -> Element {
  let agent = task
    .default
    .and_then(|profile| task.target(profile))
    .map(|target| target.role.to_string())
    .unwrap_or_default();
  let placeholder = match draft.kind.get() {
    FeedbackKind::Question => "pull_requests.feedback.composer.placeholderQuestion",
    FeedbackKind::Fix => "pull_requests.feedback.composer.placeholderFix",
  };
  let saved = draft.saved_at.clone();
  FieldInput::new(
    id.to_owned(),
    ctx.t("pull_requests.feedback.composer.request"),
    draft.text.clone(),
  )
  .text_area()
  .area_height(AREA_HEIGHT)
  .placeholder(ctx.t_args(placeholder, [("agent", agent)]))
  .on_typed(move |_| saved.set(Some(Timestamp::now())))
  .render(ctx)
}

/// The task's run ended ready for review: sending follows it up (`O8lamu`).
fn ended(ctx: &Ctx, task: &BoardTask, (ended_at, draft): (Option<Timestamp>, &Draft)) -> Element {
  let agent = target_role(task, draft);
  let key = task.key.to_string();
  let title = match ended_at {
    Some(at) => ctx.t_args(
      "pull_requests.feedback.composer.ended.title",
      [("task", key.clone()), ("time", words::time(at))],
    ),
    None => ctx.t_args(
      "pull_requests.feedback.composer.ended.titleUnknown",
      [("task", key.clone())],
    ),
  };
  let branch = task.run.as_ref().map(|run| run.branch.to_string()).unwrap_or_default();
  let body = match task.run.as_ref().and_then(|run| run.budget) {
    Some(budget) => {
      let (left, limit) = words::budget(budget);
      ctx.t_args(
        "pull_requests.feedback.composer.ended.body",
        [
          ("agent", agent),
          ("task", key),
          ("branch", branch),
          ("left", left),
          ("budget", limit),
        ],
      )
    }
    None => ctx.t_args(
      "pull_requests.feedback.composer.ended.bodyNoBudget",
      [("agent", agent), ("task", key), ("branch", branch)],
    ),
  };
  Notice::new("feedback-composer-ended", NoticeTone::Info, title)
    .icon(Icon::CircleStop)
    .body(body)
    .into()
}

/// The role of the agent the draft goes to.
fn target_role(task: &BoardTask, draft: &Draft) -> String {
  draft
    .target
    .get()
    .and_then(|profile| task.target(profile))
    .map(|target| target.role.to_string())
    .unwrap_or_default()
}

/// What sending does: the agent answers without code changes, or the task goes on with the agent's permissions,
/// after the fix request before it when one changes the branch now, with what is left of its budget.
fn run_line(ctx: &Ctx, env: &Env, (task, draft): (&BoardTask, &Draft)) -> Row {
  let (agent, key) = (target_role(task, draft), task.key.to_string());
  let before = env
    .board
    .open()
    .find(|thread| {
      thread.kind == FeedbackKind::Fix
        && matches!(
          thread.state,
          ThreadState::Working { .. } | ThreadState::Delivered(_) | ThreadState::Queued(_)
        )
    })
    .map(|thread| words::short_anchor(ctx, thread).to_string());
  let budget = task.run.as_ref().and_then(|run| run.budget).map(words::budget);
  let text = match (draft.kind.get(), before, budget) {
    (FeedbackKind::Question, ..) => ctx.t_args(
      "pull_requests.feedback.composer.runQuestion",
      [("agent", agent), ("task", key)],
    ),
    (FeedbackKind::Fix, Some(anchor), Some((left, limit))) => ctx.t_args(
      "pull_requests.feedback.composer.runFixAfter",
      [
        ("task", key),
        ("agent", agent),
        ("anchor", anchor),
        ("left", left),
        ("budget", limit),
      ],
    ),
    (FeedbackKind::Fix, Some(anchor), None) => ctx.t_args(
      "pull_requests.feedback.composer.runFixAfterNoBudget",
      [("task", key), ("agent", agent), ("anchor", anchor)],
    ),
    (FeedbackKind::Fix, None, Some((left, limit))) => ctx.t_args(
      "pull_requests.feedback.composer.runFix",
      [("task", key), ("agent", agent), ("left", left), ("budget", limit)],
    ),
    (FeedbackKind::Fix, None, None) => ctx.t_args(
      "pull_requests.feedback.composer.runFixNoBudget",
      [("task", key), ("agent", agent)],
    ),
  };
  Row::new()
    .id("feedback-composer-run")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::ListChecks, IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(&text).variant(ExtraTypography::Note.role()).nowrap())
}

/// That it stays in Orchester, why it cannot be sent yet, *Cancel*, the keys and the send button.
fn foot(ctx: &Ctx, env: &Env, (blocked, label, press): (Option<Arc<str>>, Arc<str>, Option<Press>)) -> Row {
  let cancel = {
    let composing = env.state.composing.clone();
    ActionButton::new(
      "feedback-composer-cancel",
      ButtonKind::Ghost,
      ctx.t("pull_requests.feedback.composer.cancel"),
    )
    .small()
    .on_click(move || composing.set(None))
  };
  let send = send_button(
    ActionButton::new("feedback-composer-send", ButtonKind::Primary, label).icon(Icon::Send),
    press.as_ref(),
    env.writes.busy(),
  );
  let reason = blocked
    .filter(|reason| !reason.is_empty())
    .map(|reason| Row::new().child(reason_line("feedback-composer-reason", &reason).width(Dimension::Auto)));
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(visibility(ctx, "feedback-composer-visibility", false))
    .child(Row::new().flex(1.0))
    .with_children(reason)
    .child(cancel)
    .child(send_keys(ctx, "feedback-composer-keys", press.is_some()))
    .child(send)
}

/// A send button: under way while a write runs, pressed when it can be, disabled otherwise.
pub(super) fn send_button(button: ActionButton, press: Option<&Press>, busy: bool) -> ActionButton {
  match (press, busy) {
    (_, true) => button.loading(),
    (Some(press), false) => {
      let press = press.clone();
      button.on_click(move || press())
    }
    (None, false) => button.disabled(),
  }
}

/// The key cap of the shortcut that sends, faded while sending cannot be pressed.
pub(super) fn send_keys(ctx: &Ctx, id: &str, available: bool) -> Element {
  let keys = ctx.t(if cfg!(target_os = "macos") {
    "pull_requests.feedback.composer.sendMac"
  } else {
    "pull_requests.feedback.composer.send"
  });
  let cap = Row::new().child(key_cap(id, &keys));
  if available {
    cap.into()
  } else {
    cap.opacity(DISABLED_OPACITY).into()
  }
}
