//! The Agent feedback tab's threads (`Thread list` `mbTB8`): those the filters leave, by file in the order the files
//! first got one, the whole pull request's last (`Group · …` `A1s7U`), each a row (`Feedback thread row` `ZtcOK`), and
//! what the list says at its end (`Footer` `E8m4wq`) or in place of rows.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::pull_requests::feedback::{FeedbackBoard, ThreadState, ThreadView};

use super::{Filter, KindFilter, TabProps, parts::status, words};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, PullRequestsSpacing, PullRequestsText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::pressable,
    state::EmptyState,
  },
};

/// A row's state column with and without the panel open (`Ldr0W` in `S5G87Z` and `DgYrV`).
const STATE_WIDTH: f32 = 132.0;
const STATE_WIDTH_BESIDE_PANEL: f32 = 116.0;

/// The threads the filters leave and what the list says at its end, scrolling.
pub(super) fn list(ctx: &mut Ctx, props: &TabProps, board: &FeedbackBoard, panel_open: bool) -> Element {
  let filter = props.state.filter.get();
  let kind = props.state.kind.get();
  let shown: Vec<&ThreadView> = board
    .threads
    .iter()
    .filter(|thread| thread.state.is_open() == (filter == Filter::Open) && kind.shows(thread.kind))
    .collect();
  let mut groups: Vec<(Option<&str>, Vec<&ThreadView>)> = Vec::new();
  for thread in &shown {
    let path = thread.path();
    match groups.iter_mut().find(|(group, _)| *group == path) {
      Some((_, threads)) => threads.push(thread),
      None => groups.push((path, vec![thread])),
    }
  }
  groups.sort_by_key(|(path, _)| path.is_none());
  let task = words::task_key(board.task.as_ref());
  let mut column = Column::new().id("feedback-list").width(Dimension::Pct(100.0)).padding(
    Padding::new()
      .top(SpacingSize::Xs)
      .left(PullRequestsSpacing::Detail.role())
      .right(if panel_open {
        SpacingSize::Lg
      } else {
        PullRequestsSpacing::Detail.role()
      }),
  );
  for (path, threads) in &groups {
    column = column.child(group_head(ctx, *path, threads.len()));
    for thread in threads {
      column = column.child(row(ctx, props, (thread, &task, board), panel_open));
    }
  }
  let column = if shown.is_empty() {
    column.child(empty(ctx, props, board))
  } else {
    column.child(footer(ctx, filter, shown.len()))
  };
  ScrollVertical::new(column)
    .id("feedback-list-scroll")
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .into()
}

/// A file's path or *Whole pull request*, and how many threads it has (`A1s7U`).
fn group_head(ctx: &Ctx, path: Option<&str>, count: usize) -> Row {
  let (glyph, name) = match path {
    Some(path) => (
      Icon::FileCode,
      Text::new(path).variant(ExtraTypography::PropertyMono.role()),
    ),
    None => (
      Icon::GitPullRequest,
      Text::new(&ctx.t("pull_requests.feedback.group.whole"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary),
    ),
  };
  let count = if count == 1 {
    ctx.t("pull_requests.feedback.group.one")
  } else {
    ctx.t_args("pull_requests.feedback.group.other", [("count", count.to_string())])
  };
  Row::new()
    .id(format!("feedback-group-{}", path.unwrap_or("whole")))
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Lg)
        .bottom(ExtraSpacing::Snug.role())
        .left(SpacingSize::Md)
        .right(SpacingSize::Md),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(name.nowrap())
    .child(Text::new(&count).variant(ExtraTypography::Note.role()).nowrap())
}

/// A thread's row (`ZtcOK`): where it stands, what it asks and where, what happened last; *Go to line* opens its
/// lines in Files changed, *Open thread* the whole pull request's thread beside the list. A press on the row opens
/// the thread beside the list.
fn row(
  ctx: &mut Ctx,
  props: &TabProps,
  (thread, task, board): (&ThreadView, &str, &FeedbackBoard),
  panel_open: bool,
) -> Element {
  let navigation = Navigation::of(ctx);
  let (project, number, id) = (props.project.open.project.id(), props.number, thread.id);
  let selected = props.selected == Some(id);
  let open = {
    let navigation = navigation.clone();
    move || {
      navigation.replace(Route::Project(
        project,
        ProjectPage::PullRequestFeedback(number, Some(id)),
      ))
    }
  };
  let action = (!panel_open).then(|| match thread.path() {
    Some(path) => {
      let (reveal, path) = (props.state.reveal.clone(), path.to_owned());
      ActionButton::new(
        format!("feedback-row-{id}-go"),
        ButtonKind::Ghost,
        ctx.t("pull_requests.feedback.actions.goToLine"),
      )
      .icon(Icon::ArrowUpRight)
      .small()
      .on_click(move || {
        reveal.set(Some(id));
        navigation.replace(Route::Project(
          project,
          ProjectPage::PullRequest(number, Some(path.clone())),
        ));
      })
    }
    None => ActionButton::new(
      format!("feedback-row-{id}-open"),
      ButtonKind::Ghost,
      ctx.t("pull_requests.feedback.actions.openThread"),
    )
    .icon(Icon::PanelRightOpen)
    .small()
    .on_click(open.clone()),
  });
  let state_width = if panel_open {
    STATE_WIDTH_BESIDE_PANEL
  } else {
    STATE_WIDTH
  };
  let row = pressable(open)
    .id(format!("feedback-row-{id}"))
    .describe("state", if selected { "selected" } else { "enabled" })
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Start)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .width(Dimension::Px(state_width))
        .flex_shrink(0.0)
        .padding(Padding::new().top(ExtraSpacing::Hair.role()))
        .child(status(ctx, &format!("feedback-row-{id}-state"), thread)),
    )
    .child(row_body(ctx, props, (thread, task, board)))
    .with_children(action);
  if selected {
    row.background(ExtraColor::Hover.role()).into()
  } else {
    row.into()
  }
}

/// What a row says of its thread: its kind and where, what it asks, and its last line.
fn row_body(ctx: &Ctx, props: &TabProps, (thread, task, board): (&ThreadView, &str, &FeedbackBoard)) -> Column {
  let id = thread.id;
  let top = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(
      words::kind_icon(thread.kind),
      IconSize::Base,
      PaletteColor::TextSecondary,
    ))
    .child(
      Text::new(&words::kind_word(ctx, thread.kind))
        .variant(TypographyStyle::Label)
        .nowrap(),
    )
    .child(
      Text::new(&words::row_anchor(ctx, thread, props.head.as_ref()))
        .id(format!("feedback-row-{id}-anchor"))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap(),
    );
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xs)
    .child(top)
    .child(
      Text::new(&thread.current().body)
        .id(format!("feedback-row-{id}-request"))
        .variant(PullRequestsText::ThreadRequest.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(
      Text::new(&meta(ctx, thread, (task, &board.threads)))
        .id(format!("feedback-row-{id}-meta"))
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
}

/// The row's last line: who wrote it when and what happened since (`Bp4aY`).
pub(super) fn meta(ctx: &Ctx, thread: &ThreadView, (task, board): (&str, &[ThreadView])) -> Arc<str> {
  let current = thread.current();
  let time = words::time(current.at);
  let agent = &*thread.recipient;
  let reason = |reason: Arc<str>| {
    ctx.t_args(
      "pull_requests.feedback.meta.reason",
      [("time", time.clone()), ("reason", reason.to_string())],
    )
  };
  match &thread.state {
    ThreadState::Queued(hold) => reason(words::hold(ctx, hold, (task, agent), board)),
    ThreadState::Delivered(queue) => reason(words::queue(ctx, queue, (task, agent))),
    ThreadState::Working { since } => ctx.t_args(
      "pull_requests.feedback.meta.working",
      [
        ("time", time.clone()),
        ("agent", agent.to_owned()),
        ("elapsed", words::elapsed(ctx, *since).to_string()),
      ],
    ),
    ThreadState::Answered { at } => match thread.sources.first() {
      Some(source) => ctx.t_args(
        "pull_requests.feedback.meta.answeredSource",
        [
          ("agent", agent.to_owned()),
          ("time", words::time(*at)),
          ("path", source.path.clone()),
          ("line", source.line.to_string()),
        ],
      ),
      None => ctx.t_args(
        "pull_requests.feedback.meta.answered",
        [("agent", agent.to_owned()), ("time", words::time(*at))],
      ),
    },
    ThreadState::ReadyToVerify { .. } => super::parts::commit_meta(ctx, thread),
    ThreadState::NotDelivered { .. } => reason(ctx.t("pull_requests.feedback.state.notDelivered")),
    ThreadState::Resolved { at } => ctx.t_args("pull_requests.feedback.meta.resolved", [("time", words::time(*at))]),
    ThreadState::Withdrawn { at } => ctx.t_args("pull_requests.feedback.meta.withdrawn", [("time", words::time(*at))]),
  }
}

/// The list's end: how many threads are open or resolved, and what these notes are.
fn footer(ctx: &Ctx, filter: Filter, count: usize) -> Element {
  let (one, other) = match filter {
    Filter::Open => (
      "pull_requests.feedback.footer.open.one",
      "pull_requests.feedback.footer.open.other",
    ),
    Filter::Resolved => (
      "pull_requests.feedback.footer.resolved.one",
      "pull_requests.feedback.footer.resolved.other",
    ),
  };
  let text = if count == 1 {
    ctx.t(one)
  } else {
    ctx.t_args(other, [("count", count.to_string())])
  };
  Row::new()
    .id("feedback-footer")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Inset.role()))
    .child(Text::new(&text).variant(ExtraTypography::Note.role()))
    .into()
}

/// What the list shows when the filters leave no thread.
fn empty(ctx: &Ctx, props: &TabProps, board: &FeedbackBoard) -> Element {
  let filter = props.state.filter.get();
  let agent = board
    .task
    .as_ref()
    .and_then(|task| task.default.and_then(|profile| task.target(profile)))
    .map(|target| target.role.to_string())
    .unwrap_or_default();
  let some_of_filter = board
    .threads
    .iter()
    .any(|thread| thread.state.is_open() == (filter == Filter::Open));
  let (title, detail) = if props.state.kind.get() != KindFilter::All && some_of_filter {
    (
      ctx.t("pull_requests.feedback.empty.kind.title"),
      ctx.t("pull_requests.feedback.empty.kind.detail"),
    )
  } else if filter == Filter::Open {
    (
      ctx.t("pull_requests.feedback.empty.open.title"),
      ctx.t_args("pull_requests.feedback.empty.open.detail", [("agent", agent)]),
    )
  } else {
    (
      ctx.t("pull_requests.feedback.empty.resolved.title"),
      ctx.t("pull_requests.feedback.empty.resolved.detail"),
    )
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Lg))
    .child(EmptyState::new("feedback-empty", Icon::MessagesSquare, title, detail))
    .into()
}
