//! The comment box under the chosen lines (`State · 1 · Writing a comment` `zl5Ld`, `2 · Writing a question` `zRVag`):
//! a box on `surface_raised` in the `border_focus` stroke, radius 6. Its head, 30 high on `surface_panel`: *Comment* /
//! *Question* segmented (a comment by default), the lines, and *New* at its end; the words in 13, 8 over and under
//! and 14 at the sides (`Field` `YUYFQ`), which take the focus as the box opens or takes other lines; then what the
//! kind does, *Cancel*, and *Add comment* or *Ask Implement*, 26 high. Ctrl Enter (Cmd Enter on macOS) does the same,
//! Esc cancels and keeps the words. A comment waits unsent in the tray; a question goes to Implement at once. Inside a
//! thread, the same field changes a comment not sent yet (*Save*) or writes a reply to an addressed comment (*Add
//! comment*), which waits unsent too; it takes the focus as it opens.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextInput},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  conversation::MessageBody,
  diff::FileDiff,
  pull_request::feedback::{DiffSide, LineAnchor, LineSpan},
  review::ThreadKind,
};
use orchester_workspace::tasks::review_threads::read::ThreadView;

use super::{
  Env,
  actions::{Kept, Write},
  state::{BoxField, Closes, Composing, Held, InThread},
  words::lines,
};
use crate::{
  theme::{ExtraSpacing, ReviewText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// The most lines the words grow to before they scroll, and the box's own head (`EyQD3`).
const MAX_LINES: usize = 12;
const HEAD_HEIGHT: f32 = 30.0;

/// What the box's send does, unless it cannot be sent now.
type Send = Option<Arc<dyn Fn() + std::marker::Send + Sync>>;

/// The comment box under the chosen lines.
pub(super) fn new_box(ctx: &mut Ctx, env: &Env) -> Element {
  let Some(composing) = env.state.composing.get().0 else {
    return Element::new();
  };
  let Held(kind) = env.state.kind.get();
  let text = env.state.text.get();
  let send = sendable(env, &text).then(|| new_thread(env, &composing, kind));
  let (hint, label, glyph, button) = match kind {
    ThreadKind::Comment => (
      "review_page.composer.commentHint",
      "review_page.composer.addComment",
      Icon::MessageSquarePlus,
      ButtonKind::Secondary,
    ),
    ThreadKind::Question => (
      "review_page.composer.questionHint",
      "review_page.composer.askImplement",
      Icon::Send,
      ButtonKind::Primary,
    ),
  };
  let submit = submit(
    ActionButton::new("review-composer-send", button, ctx.t(label)).icon(glyph),
    send.as_ref(),
    env.writes.busy(),
  );
  let close = env.state.clone();
  let foot = foot(
    ctx,
    ("review-composer", &ctx.t(hint)),
    move || close.close(false),
    submit,
  );
  Column::new()
    .id("review-composer")
    .describe("kind", kind.as_str())
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderFocus))
    .rounded(RadiusSize::Md)
    .on_key_down(keys(send, {
      let state = env.state.clone();
      move || state.close(false)
    }))
    .child(head(ctx, env, &composing, kind))
    .child(field(
      ctx,
      ("review-composer-text", &env.state.field),
      &env.state.text,
      &placeholder(ctx, kind),
    ))
    .child(foot)
    .into()
}

/// The box inside `view`: a comment's new words, or a reply to it.
pub(super) fn in_thread_box(ctx: &mut Ctx, env: &Env, view: &ThreadView, what: InThread) -> Element {
  let id = format!("review-thread-{}-box", view.thread.id());
  let text = env.state.in_thread_text.get();
  let send = sendable(env, &text).then(|| in_thread(env, view, what));
  let (label, glyph) = match what {
    InThread::Edit(_) => ("review_page.composer.save", Icon::Check),
    InThread::FollowUp(_) => ("review_page.composer.addComment", Icon::MessageSquarePlus),
  };
  let submit = submit(
    ActionButton::new(format!("{id}-send"), ButtonKind::Secondary, ctx.t(label)).icon(glyph),
    send.as_ref(),
    env.writes.busy(),
  );
  let close = env.state.clone();
  let hint = match what {
    InThread::Edit(_) => ctx.t("review_page.composer.editHint"),
    InThread::FollowUp(_) => ctx.t("review_page.composer.replyHint"),
  };
  Column::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderFocus))
    .rounded(RadiusSize::Md)
    .on_key_down(keys(send, {
      let state = env.state.clone();
      move || state.close_in_thread()
    }))
    .child(field(
      ctx,
      (&format!("{id}-text"), &env.state.in_thread_field),
      &env.state.in_thread_text,
      &ctx.t("review_page.composer.commentPlaceholder"),
    ))
    .child(foot(ctx, (&id, &hint), move || close.close_in_thread(), submit))
    .into()
}

/// Whether words written can go now: some are written and no change runs.
fn sendable(env: &Env, text: &str) -> bool {
  !text.trim().is_empty() && !env.writes.busy_untracked()
}

/// The head: *Comment* / *Question*, the lines, and *New*.
fn head(ctx: &Ctx, env: &Env, composing: &Composing, kind: ThreadKind) -> Row {
  let segment = |id: &str, key: &str, value: ThreadKind| {
    let chosen = env.state.kind.clone();
    Segment::new(id.to_owned(), SegmentFace::Text(ctx.t(key)), kind == value, move || {
      chosen.set(Held(value));
    })
  };
  let mode = segmented(
    "review-composer-kind",
    [
      segment(
        "review-composer-comment",
        "review_page.thread.comment",
        ThreadKind::Comment,
      ),
      segment(
        "review-composer-question",
        "review_page.thread.question",
        ThreadKind::Question,
      ),
    ],
    Focus::Keyboard,
  );
  Row::new()
    .id("review-composer-head")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfacePanel)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(mode)
    .child(
      Text::new(&lines(ctx, composing.span))
        .id("review-composer-lines")
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(Row::new().flex(1.0))
    .child(
      Text::new(&ctx.t("review_page.composer.new"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
}

fn placeholder(ctx: &Ctx, kind: ThreadKind) -> Arc<str> {
  match kind {
    ThreadKind::Comment => ctx.t("review_page.composer.commentPlaceholder"),
    ThreadKind::Question => ctx.t("review_page.composer.questionPlaceholder"),
  }
}

/// The words, 8 over and under and 14 at the sides, growing with their lines; the field takes the focus when its box
/// opened since it was drawn last.
fn field(ctx: &Ctx, (id, at): (&str, &BoxField), value: &Signal<String>, placeholder: &Arc<str>) -> Element {
  let theme = ctx.theme();
  let resolve = |color| {
    FormTextRole {
      typography: ReviewText::Message.role(),
      color,
    }
    .resolve(&theme.typography(), &theme.palette())
  };
  at.focus_once(ctx);
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Sm))
    .child(
      TextInput::styled(value.clone(), resolve(PaletteColor::TextPrimary))
        .placeholder_style(resolve(PaletteColor::TextMuted))
        .placeholder(placeholder)
        .caret_color(PaletteColor::TextPrimary)
        .id(id.to_owned())
        .describe("a11y_name", placeholder.clone())
        .width(Dimension::Pct(100.0))
        .multiline()
        .rows(1, MAX_LINES)
        .ref_element(at.field.clone()),
    )
    .into()
}

/// What the kind does, *Cancel* and the send button.
fn foot(
  ctx: &Ctx,
  (id, hint): (&str, &str),
  cancel: impl Fn() + std::marker::Send + Sync + 'static,
  submit: ActionButton,
) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(ExtraSpacing::Inset.role())
        .right(ExtraSpacing::Inset.role())
        .bottom(SpacingSize::Sm),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(hint)
        .id(format!("{id}-hint"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      ActionButton::new(
        format!("{id}-cancel"),
        ButtonKind::Ghost,
        ctx.t("review_page.composer.cancel"),
      )
      .icon(Icon::X)
      .snug()
      .on_click(cancel),
    )
    .child(submit.snug())
}

/// The send button: under way while a change runs, pressed when the words can go, disabled otherwise.
fn submit(button: ActionButton, send: Option<&Arc<dyn Fn() + std::marker::Send + Sync>>, busy: bool) -> ActionButton {
  match (send, busy) {
    (_, true) => button.loading(),
    (Some(send), false) => {
      let send = send.clone();
      button.on_click(move || send())
    }
    (None, false) => button.disabled(),
  }
}

/// Ctrl Enter (Cmd Enter) sends; Esc closes and keeps the words.
fn keys(
  send: Send,
  close: impl Fn() + std::marker::Send + Sync + 'static,
) -> impl Fn(KeyboardEvent) + std::marker::Send + Sync + 'static {
  move |event: KeyboardEvent| {
    if event.composing {
      return;
    }
    if event.key == "Escape" {
      event.prevent_default();
      close();
      return;
    }
    if event.key == "Enter" && (event.ctrl || event.meta) {
      event.prevent_default();
      if let Some(send) = &send {
        send();
      }
    }
  }
}

/// Sends a new thread of `kind` on the chosen lines: a comment kept unsent, or a question asked at once.
fn new_thread(env: &Env, composing: &Composing, kind: ThreadKind) -> Arc<dyn Fn() + std::marker::Send + Sync> {
  let file = env
    .diff
    .files()
    .iter()
    .find(|file| *file.path() == *composing.path)
    .cloned();
  let (writes, text, composing) = (env.writes.clone(), env.state.text.clone(), composing.clone());
  Arc::new(move || {
    let excerpt = file
      .as_ref()
      .map(|file| excerpt(file, composing.side, composing.span))
      .unwrap_or_default();
    let anchor = LineAnchor::new(&composing.path, composing.side, composing.span, excerpt);
    let (Ok(anchor), Ok(body)) = (anchor, MessageBody::new(&text.get_untracked())) else {
      tracing::warn!("the comment box's lines or words could not be kept");
      return;
    };
    let head = composing.head.clone();
    let write = match kind {
      ThreadKind::Comment => Write::Keep(Kept::Comment(anchor, head, body)),
      ThreadKind::Question => Write::Ask(anchor, head, body),
    };
    writes.run_closing(write, Closes::Box);
  })
}

/// Sends what the box inside `view` holds.
fn in_thread(env: &Env, view: &ThreadView, what: InThread) -> Arc<dyn Fn() + std::marker::Send + Sync> {
  let (writes, text, thread) = (env.writes.clone(), env.state.in_thread_text.clone(), view.thread.id());
  Arc::new(move || {
    let Ok(body) = MessageBody::new(&text.get_untracked()) else {
      tracing::warn!("the words written in a thread could not be kept");
      return;
    };
    let write = match what {
      InThread::Edit(message) => Kept::Edit(message, body),
      InThread::FollowUp(_) => Kept::Reply(thread, body),
    };
    writes.run_closing(Write::Keep(write), Closes::InThread);
  })
}

/// The code of lines `span` on `side` of `file`.
fn excerpt(file: &FileDiff, side: DiffSide, span: LineSpan) -> Vec<String> {
  file
    .hunks()
    .iter()
    .flat_map(|hunk| hunk.lines())
    .filter_map(|line| {
      let number = match side {
        DiffSide::Old => line.old_number(),
        DiffSide::New => line.new_number(),
      }?;
      span.contains(number).then(|| line.text().to_owned())
    })
    .collect()
}
