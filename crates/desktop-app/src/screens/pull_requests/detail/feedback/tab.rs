//! The Agent feedback tab (`Pull request feedback · Agent feedback tab` `S5G87Z`): the toolbar (`Feedback toolbar`
//! `aNPdP`: Open and Resolved, which requests, that it stays in Orchester, *Ask about the pull request*), the agent
//! the threads go to (`Agent strip` `WhTBI`), the threads ([`super::list`]); a thread opens beside the list (`Thread
//! panel` `Z3JKS` in `DgYrV`). Without a linked task the tab says why no agent can be asked (`F4WO43`).

use std::{num::NonZeroU64, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, SelectOption, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::FeedbackId, revision::Revision, tokens::compact_tokens};
use orchester_workspace::{
  messages::Message,
  pull_requests::feedback::{FeedbackBoard, Gate, ThreadState},
};

use super::{
  Composing, FeedbackState, Filter, KindFilter, Writes,
  list::list,
  panel::{PanelProps, panel},
  parts::visibility,
  words,
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  theme::{ExtraSpacing, ExtraTypography, IconSize, PullRequestsSpacing},
  ui::{
    avatar::person_avatar,
    button::{ActionButton, ButtonKind},
    field::SelectField,
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    pressable::Focus,
    state::EmptyState,
    tabs::{Segment, SegmentFace, segmented},
    unavailable::reason_line,
  },
};

/// The toolbar's height (`aNPdP`), the kind select's width (`Iu3LR`), the thread panel's width (`Z3JKS`) and the
/// widest the no-task card runs (`z86ps`).
const TOOLBAR_HEIGHT: f32 = 52.0;
const KIND_WIDTH: f32 = 150.0;
const PANEL_WIDTH: f32 = 560.0;
const NO_TASK_WIDTH: f32 = 560.0;

#[derive(Clone, PartialEq)]
pub struct TabProps {
  pub project: ShownProject,
  pub number: NonZeroU64,
  pub board: Option<Arc<FeedbackBoard>>,
  pub failed: Option<Message>,
  /// The pull request's head as last read.
  pub head: Option<Revision>,
  /// The thread open beside the list, as the route names it.
  pub selected: Option<FeedbackId>,
  pub state: FeedbackState,
  pub writes: Writes,
}

pub fn feedback_tab(ctx: &mut Ctx, props: &TabProps) -> Element {
  let column = Column::new()
    .id("feedback-tab")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0));
  let Some(board) = props.board.clone() else {
    return column
      .with_children(props.failed.as_ref().map(|error| failure(ctx, error)))
      .into();
  };
  if board.task.is_none() && board.threads.is_empty() {
    return column.child(no_task(ctx, props.number)).into();
  }
  let panel_open = props.selected.is_some() || props.state.composing.get() == Some(Composing::PullRequest);
  let list = list(ctx, props, &board, panel_open);
  let body = Row::new()
    .id("feedback-body")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .child(list)
    .with_children(panel_open.then(|| {
      Column::new()
        .id("feedback-panel-frame")
        .width(Dimension::Px(PANEL_WIDTH))
        .flex_shrink(0.0)
        .height(Dimension::Pct(100.0))
        .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(panel(
          ctx,
          &PanelProps {
            tab: props.clone(),
            board: board.clone(),
          },
        ))
    }));
  column
    .child(toolbar(ctx, props, &board))
    .with_children(props.state.failure.get().map(|error| failure(ctx, &error)))
    .with_children(board.task.as_ref().map(|_| strip(ctx, props, &board)))
    .child(body)
    .into()
}

/// Why the latest read or write failed, under the toolbar.
fn failure(ctx: &Ctx, error: &Message) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Md)
        .left(PullRequestsSpacing::Detail.role())
        .right(PullRequestsSpacing::Detail.role()),
    )
    .child(
      Notice::new(
        "feedback-failure",
        NoticeTone::Danger,
        ctx.t("pull_requests.feedback.loadFailed"),
      )
      .body(message_text(ctx, error)),
    )
    .into()
}

/// The pull request has no linked task: no agent can be asked (`F4WO43`).
fn no_task(ctx: &Ctx, number: NonZeroU64) -> Element {
  let card = EmptyState::new(
    "feedback-no-task",
    Icon::Unlink,
    ctx.t("pull_requests.feedback.noTask.title"),
    ctx.t_args("pull_requests.feedback.noTask.detail", [("number", number.to_string())]),
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Xl, ExtraSpacing::Roomy.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .max_width(Dimension::Px(NO_TASK_WIDTH))
        .child(card),
    )
    .child(
      Row::new().child(
        reason_line(
          "feedback-no-task-reason",
          &ctx.t("pull_requests.feedback.noTask.reason"),
        )
        .width(Dimension::Auto),
      ),
    )
    .into()
}

/// Open and Resolved, which requests, that it stays in Orchester, and *Ask about the pull request*.
fn toolbar(ctx: &mut Ctx, props: &TabProps, board: &FeedbackBoard) -> Row {
  let ask = ask(ctx, props, board);
  let state = &props.state;
  let open = board.open().count();
  let filter = state.filter.get();
  let segment = |id: &str, key: &str, count: usize, value: Filter| {
    let chosen = state.filter.clone();
    Segment::new(
      id.to_owned(),
      SegmentFace::Text(ctx.t_args(key, [("count", count.to_string())])),
      filter == value,
      move || chosen.set(value),
    )
  };
  let filters = segmented(
    "feedback-filter",
    [
      segment(
        "feedback-filter-open",
        "pull_requests.feedback.toolbar.open",
        open,
        Filter::Open,
      ),
      segment(
        "feedback-filter-resolved",
        "pull_requests.feedback.toolbar.resolved",
        board.threads.len() - open,
        Filter::Resolved,
      ),
    ],
    Focus::Pointer,
  );
  let kinds = [
    (KindFilter::All, "pull_requests.feedback.toolbar.all"),
    (KindFilter::Questions, "pull_requests.feedback.toolbar.questions"),
    (KindFilter::Fixes, "pull_requests.feedback.toolbar.fixes"),
  ]
  .into_iter()
  .map(|(kind, key)| SelectOption::new(kind, ctx.t(key)))
  .collect();
  let kind = SelectField::new(
    "feedback-kind",
    ctx.t("pull_requests.feedback.toolbar.kind"),
    state.kind.clone(),
    kinds,
  )
  .width(Dimension::Px(KIND_WIDTH))
  .render(ctx);
  Row::new()
    .id("feedback-toolbar")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOOLBAR_HEIGHT))
    .padding(Padding::horizontal(PullRequestsSpacing::Detail.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(filters)
    .child(kind)
    .child(Row::new().flex(1.0))
    .child(visibility(ctx, "feedback-visibility", false))
    .child(ask)
}

/// *Ask about the pull request*: a new thread on the whole pull request, in the panel.
fn ask(ctx: &mut Ctx, props: &TabProps, board: &FeedbackBoard) -> ActionButton {
  let navigation = Navigation::of(ctx);
  let button = ActionButton::new(
    "feedback-ask",
    ButtonKind::Secondary,
    ctx.t("pull_requests.feedback.toolbar.ask"),
  )
  .icon(Icon::MessageCircleQuestion);
  let Some(task) = &board.task else {
    return button.unavailable(ctx.t("pull_requests.feedback.noTask.title"));
  };
  if let Gate::Closed(state) = task.gate {
    return button.unavailable(ctx.t_args(words::closed_key(state), [("task", task.key.to_string())]));
  }
  let (state, default, project, number) = (
    props.state.clone(),
    task.default,
    props.project.open.project.id(),
    props.number,
  );
  button.on_click(move || {
    super::composer::open(&state, Composing::PullRequest, default);
    navigation.replace(Route::Project(project, ProjectPage::PullRequestFeedback(number, None)));
  })
}

/// `Run 3 · 52k tokens on feedback · 380k of 1.4M left`.
fn spent(ctx: &Ctx, run: &orchester_workspace::pull_requests::feedback::RunFacts) -> Arc<str> {
  let tokens = compact_tokens(run.feedback_tokens);
  match run.budget {
    Some(budget) => {
      let (left, limit) = words::budget(budget);
      ctx.t_args(
        "pull_requests.feedback.strip.budget",
        [
          ("run", run.number.to_string()),
          ("tokens", tokens),
          ("left", left),
          ("budget", limit),
        ],
      )
    }
    None => ctx.t_args(
      "pull_requests.feedback.strip.noBudget",
      [("run", run.number.to_string()), ("tokens", tokens)],
    ),
  }
}

/// The agent the threads go to, what it works on, and the run with what it spent on feedback (`WhTBI`).
fn strip(ctx: &mut Ctx, props: &TabProps, board: &FeedbackBoard) -> Element {
  let task = board.task.as_ref().expect("the strip shows for a linked task");
  let target = task.default.and_then(|profile| task.target(profile));
  let agent = target.map(|target| target.role.clone()).unwrap_or_default();
  let initial: String = agent.chars().take(1).collect();
  let model = target.and_then(|target| words::model(ctx, target.provider, target.model.as_deref()));
  let summary = summary(ctx, board, &agent);
  let budget = task.run.as_ref().map(|run| spent(ctx, run));
  let conversation = {
    let navigation = Navigation::of(ctx);
    let to = Route::Project(
      props.project.open.project.id(),
      ProjectPage::Task(task.key.clone(), TaskView::of(TaskTab::Conversation)),
    );
    ActionButton::new(
      "feedback-open-conversation",
      ButtonKind::Ghost,
      ctx.t("pull_requests.feedback.strip.openConversation"),
    )
    .icon(Icon::MessagesSquare)
    .opens(navigation.to(to))
  };
  Row::new()
    .id("feedback-strip")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      PullRequestsSpacing::Detail.role(),
      ExtraSpacing::Loose.role(),
    ))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(person_avatar("feedback-strip-avatar", &initial))
    .child(
      Text::new(&ctx.t_args(
        "pull_requests.feedback.strip.agent",
        [("agent", agent.to_string()), ("task", task.key.to_string())],
      ))
      .id("feedback-strip-agent")
      .variant(TypographyStyle::Label)
      .nowrap(),
    )
    .with_children(model.map(|model| {
      Text::new(&model)
        .id("feedback-strip-model")
        .variant(ExtraTypography::Note.role())
        .nowrap()
    }))
    .child(
      Row::new()
        .width(Dimension::Px(1.0))
        .height(Dimension::Px(IconSize::Md.size()))
        .background(PaletteColor::Border),
    )
    .child(
      Text::new(&summary)
        .id("feedback-strip-summary")
        .variant(TypographyStyle::Description)
        .nowrap(),
    )
    .child(Row::new().flex(1.0))
    .with_children(budget.map(|budget| {
      Text::new(&budget)
        .id("feedback-strip-budget")
        .variant(ExtraTypography::Note.role())
        .nowrap()
    }))
    .child(conversation)
    .into()
}

/// `Working on 1 request · 1 queued · 1 waits for you to verify`, or that nothing is with the agent.
fn summary(ctx: &Ctx, board: &FeedbackBoard, agent: &str) -> Arc<str> {
  let count = |matches: fn(&ThreadState) -> bool| board.open().filter(|thread| matches(&thread.state)).count();
  let working = count(|state| matches!(state, ThreadState::Working { .. } | ThreadState::Delivered(_)));
  let queued = count(|state| matches!(state, ThreadState::Queued(_)));
  let verify = count(|state| matches!(state, ThreadState::ReadyToVerify { .. }));
  let counted = |one: &str, other: &str, count: usize| {
    if count == 1 {
      ctx.t(one)
    } else {
      ctx.t_args(other, [("count", count.to_string())])
    }
  };
  let mut parts: Vec<Arc<str>> = Vec::new();
  if working > 0 {
    parts.push(counted(
      "pull_requests.feedback.strip.working.one",
      "pull_requests.feedback.strip.working.other",
      working,
    ));
  }
  if queued > 0 {
    parts.push(ctx.t_args("pull_requests.feedback.strip.queued", [("count", queued.to_string())]));
  }
  if verify > 0 {
    parts.push(counted(
      "pull_requests.feedback.strip.verify.one",
      "pull_requests.feedback.strip.verify.other",
      verify,
    ));
  }
  if parts.is_empty() {
    return ctx.t_args("pull_requests.feedback.strip.idle", [("agent", agent)]);
  }
  Arc::from(parts.join(&*ctx.t("pull_requests.feedback.separator")))
}
