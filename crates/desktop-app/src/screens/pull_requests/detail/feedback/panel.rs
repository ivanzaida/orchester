//! A thread beside the Agent feedback list (`Thread panel` `Z3JKS` in `DgYrV`): its kind, state, *Go to line* and
//! *Close*; where it is and to whom; the code it is about (`Excerpt` `rWQEO`); the messages, what happened, the notes,
//! what can be done now (`Thread actions` `bYXhh`) and a reply. *Ask about the pull request* opens a new thread's
//! composer here instead.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::{Alignment, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::pull_request::feedback::DiffSide;
use orchester_workspace::pull_requests::feedback::{FeedbackBoard, ThreadState, ThreadView};

use super::{
  Composing, Held, TabProps,
  composer::{NewProps, new_thread},
  parts::{activity, events, messages, notes, status},
  reply::{ReplyProps, reply},
  thread::{Env, actions, conversation_button},
  words,
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, PullRequestsSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    icon_button::IconButton,
  },
};

/// A line number's column in the excerpt, as in the diff (`ey7iM`).
const NUMBER_WIDTH: f32 = 44.0;

#[derive(Clone)]
pub struct PanelProps {
  pub tab: TabProps,
  pub board: Arc<FeedbackBoard>,
}

pub fn panel(ctx: &mut Ctx, props: &PanelProps) -> Element {
  let env = Env {
    project: props.tab.project.clone(),
    number: props.tab.number,
    head: props.tab.head.clone(),
    state: props.tab.state.clone(),
    writes: props.tab.writes.clone(),
    board: props.board.clone(),
    navigation: Navigation::of(ctx),
  };
  let thread = props.tab.selected.and_then(|id| props.board.thread(id)).cloned();
  let content: Element = match thread {
    Some(thread) => thread_panel(ctx, &env, &thread),
    None => new_thread(
      ctx,
      &NewProps {
        env: env.clone(),
        composing: Composing::PullRequest,
        excerpt: Vec::new(),
        latest: None,
      },
    ),
  };
  ScrollVertical::new(
    Column::new()
      .id("feedback-panel")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        SpacingSize::Xl,
        PullRequestsSpacing::FeedbackPanel.role(),
      ))
      .spacing(ExtraSpacing::Inset.role())
      .child(content),
  )
  .id("feedback-panel-scroll")
  .width(Dimension::Pct(100.0))
  .height(Dimension::Pct(100.0))
  .into()
}

fn thread_panel(ctx: &mut Ctx, env: &Env, thread: &ThreadView) -> Element {
  let task = words::task_key(env.board.task.as_ref());
  let id = thread.id;
  let mut column = Column::new()
    .id(format!("feedback-panel-{id}"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inset.role())
    .child(head(ctx, env, thread))
    .child(
      Text::new(&anchor(ctx, env, thread, &task))
        .id("feedback-panel-anchor")
        .variant(ExtraTypography::MonoKey.role())
        .width(Dimension::Pct(100.0)),
    )
    .with_children(excerpt(thread))
    .with_children(messages(ctx, thread, &task));
  if matches!(
    thread.state,
    ThreadState::Queued(_) | ThreadState::Delivered(_) | ThreadState::Working { .. } | ThreadState::NotDelivered { .. }
  ) {
    column = column.with_children(events(ctx, thread, (&task, &env.board.threads)));
  }
  if let (Some(board_task), Some(conversation)) = (
    env.board.task.as_ref(),
    conversation_button(ctx, env, "feedback-panel-conversation"),
  ) {
    column = column.with_children(activity(ctx, thread, board_task, conversation));
  }
  if let Some(board_task) = env.board.task.as_ref() {
    column = column.with_children(notes(ctx, thread, (board_task, env.head.as_ref())));
  }
  let shown = actions(ctx, env, thread, true);
  if !shown.is_empty() {
    column = column.child(
      Row::new()
        .id("feedback-panel-actions")
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .with_children(shown),
    );
  }
  if env.state.composing.get() == Some(Composing::Reply(id)) {
    column = column.child(reply(
      ctx,
      &ReplyProps {
        env: env.clone(),
        thread: thread.clone(),
      },
    ));
  }
  column.into()
}

/// The kind as the panel's title, the state, *Go to line* for lines, and *Close*.
fn head(ctx: &mut Ctx, env: &Env, thread: &ThreadView) -> Row {
  let navigation = Navigation::of(ctx);
  let (project, number, id) = (env.project.open.project.id(), env.number, thread.id);
  let go = thread.path().map(|path| {
    let (navigation, reveal, path) = (navigation.clone(), env.state.reveal.clone(), path.to_owned());
    ActionButton::new(
      "feedback-panel-go",
      ButtonKind::Ghost,
      ctx.t("pull_requests.feedback.actions.goToLine"),
    )
    .icon(Icon::ArrowUpRight)
    .small()
    .on_click(move || {
      reveal.set(Held(Some(id)));
      navigation.replace(Route::Project(
        project,
        ProjectPage::PullRequest(number, Some(path.clone())),
      ));
    })
  });
  let close = IconButton::new(
    "feedback-panel-close",
    Icon::X,
    ctx.t("pull_requests.feedback.actions.close"),
  )
  .on_click(move || navigation.replace(Route::Project(project, ProjectPage::PullRequestFeedback(number, None))))
  .render(ctx);
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(
      words::kind_icon(thread.kind),
      IconSize::Nav,
      PaletteColor::TextSecondary,
    ))
    .child(
      Text::new(&words::kind_word(ctx, thread.kind))
        .id("feedback-panel-title")
        .variant(TypographyStyle::Title)
        .nowrap(),
    )
    .child(status(ctx, "feedback-panel-state", thread))
    .child(Row::new().flex(1.0))
    .with_children(go)
    .child(close)
}

/// `src/webhooks/sender.rs · new line 40 · 7c1e9f2 · to Builder · KON-12`.
fn anchor(ctx: &Ctx, env: &Env, thread: &ThreadView, task: &str) -> Arc<str> {
  let anchor = words::card_anchor(ctx, thread, (env.head.as_ref(), task));
  match thread.path() {
    Some(path) => ctx.t_args(
      "pull_requests.feedback.anchor.file",
      [("path", path.to_owned()), ("anchor", anchor.to_string())],
    ),
    None => anchor,
  }
}

/// The code the thread is about, as it was when it was written, numbered on its side (`Excerpt` `rWQEO`).
fn excerpt(thread: &ThreadView) -> Option<Element> {
  let lines = thread.anchor.lines()?;
  let side = lines.side();
  let rows = lines
    .excerpt()
    .iter()
    .zip(lines.span().start()..)
    .map(|(code, number)| {
      let (old, new) = match side {
        DiffSide::Old => (number.to_string(), String::new()),
        DiffSide::New => (String::new(), number.to_string()),
      };
      let number = |value: &str| {
        Text::new(value)
          .variant(ExtraTypography::MonoNote.role())
          .text_align(TextAlign::Right)
          .nowrap()
          .width(Dimension::Px(NUMBER_WIDTH))
      };
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(crate::theme::ControlHeight::Xs.value()))
        .spacing(SpacingSize::Lg)
        .align_items(Alignment::Center)
        .child(number(&old))
        .child(number(&new))
        .child(Text::new(code).variant(TypographyStyle::Mono).nowrap())
        .into()
    });
  Some(
    Column::new()
      .id("feedback-panel-excerpt")
      .width(Dimension::Pct(100.0))
      .background(ExtraColor::CodeBackground.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(lurq::app::theme::RadiusSize::Md)
      .with_children(rows.collect::<Vec<Element>>())
      .into(),
  )
}
