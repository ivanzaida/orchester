//! The side question pane (`Side question` `q4Nz4i` in 12b `gC8a7`): 400 wide to the right of the transcript,
//! `surface_panel` with a `border` line on its left. Its head (44 high: the icon, *Side question*, *{task} · run {n}*
//! and a close button), the scope line that says where the question goes (`eye-off`), the thread of questions and
//! answers ([`super::thread`]), and its composer in the run composer's style: *Ask a follow-up about run {n}…*, a lock
//! with *Reads only · changes nothing*, and *Ask*, or *Stop* while an answer is coming.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text, TextInput},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{RunId, TaskId},
  project::TaskKey,
  run::side_question::SideQuestion,
};
use orchester_workspace::{background, decisions::DecisionKind, messages, tasks::side_question};

use super::{SideQuestions, thread};
use crate::{
  project::{ShownProject, run_changes, settled_read::settled_read},
  theme::{ExtraSpacing, IconSize, SideSpacing, SideText},
  ui::{
    button::{ActionButton, ButtonKind},
    composer::message_scrollbar,
    icon::{Icon, icon},
    message::message_text,
    pressable::pressable,
  },
};

/// The pane's width and its head's height (`q4Nz4i`, `U91Ex`).
const PANE_WIDTH: f32 = 400.0;
const HEAD_HEIGHT: f32 = 44.0;
/// The close button's square in the head (`UeiTJ`).
const CLOSE_SIZE: f32 = 24.0;
/// The most lines the question grows to before its lines scroll.
const MAX_LINES: usize = 6;

/// What the pane shows: the run it asks about, of task `key` of `project`, and whose agent it is; whether a message
/// still reaches that agent (`sendable`), the decision sending the answer takes (`decision`), and whether the run runs
/// (its transcript grows).
#[derive(Clone, PartialEq)]
pub(in crate::screens::task::conversation) struct PaneProps {
  pub(in crate::screens::task::conversation) project: ShownProject,
  pub(in crate::screens::task::conversation) task: TaskId,
  pub(in crate::screens::task::conversation) key: TaskKey,
  pub(in crate::screens::task::conversation) run: RunId,
  pub(in crate::screens::task::conversation) number: u32,
  pub(in crate::screens::task::conversation) agent: Arc<str>,
  pub(in crate::screens::task::conversation) sendable: bool,
  pub(in crate::screens::task::conversation) decision: Option<DecisionKind>,
  pub(in crate::screens::task::conversation) running: bool,
  pub(in crate::screens::task::conversation) not_shared: bool,
}

impl DevtoolsInspectable for PaneProps {}

/// The pane for the run `props` names, keyed by it.
pub(in crate::screens::task::conversation) fn pane(ctx: &mut Ctx, props: PaneProps) -> Element {
  ctx.mount_keyed::<SidePane>(&format!("side-question-{}", props.run), props)
}

struct SidePane;

impl Component for SidePane {
  type Props = PaneProps;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PaneProps>().clone();
    let state = SideQuestions::of(ctx).expect("the Conversation tab provides the side questions before its pane");
    let questions = read_questions(ctx, &props, &state);
    let readable = read_transcript(ctx, &props);
    let thread = thread::thread(ctx, (&props, &state), &questions, readable);
    let composer = composer(ctx, (&props, &state), !questions.is_empty());
    Column::new()
      .id("side-question")
      .width(Dimension::Px(PANE_WIDTH))
      .flex_shrink(0.0)
      .height(Dimension::Pct(100.0))
      .background(PaletteColor::SurfacePanel)
      .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(head(ctx, &props, &state))
      .child(scope(ctx, &props))
      .child(
        ScrollVertical::new(thread)
          .id("side-question-thread")
          .with_scroll_state(state.scroll.clone())
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0)),
      )
      .child(composer)
  }
}

/// The task's side questions, as read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Asked(Vec<SideQuestion>);

impl DevtoolsInspectable for Asked {}

/// The task's side questions about the run, as last read, read again once an ask or a send ended.
fn read_questions(ctx: &mut Ctx, props: &PaneProps, state: &SideQuestions) -> Vec<SideQuestion> {
  let root: PathBuf = props.project.open.repo_root.clone();
  let (task, run) = (props.task, props.run);
  let read = ctx.future((root, task, state.revision.get()), move |(root, task, _)| async move {
    background::run(move || side_question::side_questions_in(&root, task))
      .await?
      .map(Asked)
      .map_err(|error| messages::store_error(&error))
  });
  let read = read.state().get();
  read
    .data
    .unwrap_or_default()
    .0
    .into_iter()
    .filter(|question| question.run == run)
    .collect()
}

/// Whether the run has a transcript to read yet; while that is not known, it is taken to have one. Read again as the
/// project's runs change state and, while the run runs, as it changes; another run's lines do not.
fn read_transcript(ctx: &mut Ctx, props: &PaneProps) -> bool {
  let root: PathBuf = props.project.open.repo_root.clone();
  let changes = run_changes::of_shown_run(ctx, &props.project, Some(props.run), props.running);
  let read = settled_read(
    ctx,
    "side question transcript",
    (root, props.run, changes),
    move |(root, run, _)| async move { background::run(move || side_question::has_transcript_in(&root, run)).await? },
  );
  read.data.unwrap_or(true)
}

/// The head: the icon, *Side question*, the task and run, and the close button (`U91Ex`).
fn head(ctx: &mut Ctx, props: &PaneProps, state: &SideQuestions) -> Element {
  let run = ctx.t_args(
    "side_question.run",
    [("task", props.key.to_string()), ("run", props.number.to_string())],
  );
  let open = state.open.clone();
  let close = pressable(move || open.set(false))
    .id("side-question-close")
    .describe("a11y_name", ctx.t("side_question.close"))
    .size(Dimension::Px(CLOSE_SIZE), Dimension::Px(CLOSE_SIZE))
    .justify(lurq::layout::layout_kind::Justify::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(lurq::node::Style::new().background(crate::theme::ExtraColor::Hover.role()))
    .child(icon(Icon::X, IconSize::Md, PaletteColor::TextMuted));
  Row::new()
    .id("side-question-head")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .flex_shrink(0.0)
    .padding(Padding::new().left(SpacingSize::Md).right(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(
      Icon::MessageCircleQuestion,
      IconSize::Md,
      PaletteColor::TextSecondary,
    ))
    .child(
      Text::new(&ctx.t("side_question.title"))
        .variant(SideText::Title.role())
        .nowrap(),
    )
    .child(
      Text::new(&run)
        .id("side-question-run")
        .variant(SideText::Quiet.role())
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(close)
    .into()
}

/// The scope line: the run's agent does not see this, and what answers it (`ALuWh`).
fn scope(ctx: &mut Ctx, props: &PaneProps) -> Element {
  let words = ctx.t_args(
    "side_question.scope",
    [("agent", &*props.agent), ("run", &props.number.to_string())],
  );
  Row::new()
    .id("side-question-scope")
    .width(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .padding(
      Padding::new()
        .top(ExtraSpacing::Loose.role())
        .bottom(ExtraSpacing::Loose.role())
        .left(SpacingSize::Md)
        .right(SpacingSize::Md),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::EyeOff, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .id("side-question-scope-text")
        .variant(SideText::Scope.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// The pane's composer (`v3gD7f`): the question, then a lock with *Reads only · changes nothing* and *Ask*, or *Stop*
/// while an answer is coming; why the last press did nothing under it, if it did not.
fn composer(ctx: &mut Ctx, (props, state): (&PaneProps, &SideQuestions), follow_up: bool) -> Element {
  let field = question_box(ctx, (props, state), follow_up);
  let refused = state.refusal().map(|refusal| {
    Text::new(&message_text(ctx, &refusal))
      .id("side-question-refused")
      .variant(SideText::Failed.role())
      .width(Dimension::Pct(100.0))
  });
  Column::new()
    .id("side-question-composer")
    .width(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .padding(
      Padding::new()
        .top(SideSpacing::Gap.role())
        .left(SpacingSize::Md)
        .right(SpacingSize::Md)
        .bottom(SpacingSize::Md),
    )
    .spacing(SpacingSize::Sm)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(field)
    .with_children(refused)
    .into()
}

/// The question field, as the run composer's: it grows with its lines up to [`MAX_LINES`]; Enter asks.
fn question_input(
  ctx: &mut Ctx,
  state: &SideQuestions,
  placeholder: &str,
  on_enter: impl Fn() + Send + Sync + 'static,
) -> Element {
  let theme = ctx.theme();
  let resolve = |color| {
    FormTextRole {
      typography: TypographyStyle::Body,
      color,
    }
    .resolve(&theme.typography(), &theme.palette())
  };
  TextInput::styled(state.draft.clone(), resolve(PaletteColor::TextPrimary))
    .placeholder_style(resolve(PaletteColor::TextMuted))
    .caret_color(PaletteColor::TextPrimary)
    .id("side-question-input")
    .describe("a11y_name", placeholder.to_owned())
    .textarea()
    .rows(1, MAX_LINES)
    .scrollbar(message_scrollbar(ctx))
    .width(Dimension::Pct(100.0))
    .placeholder(placeholder)
    .on_key_down(move |event: KeyboardEvent| {
      if event.key == "Enter" && !event.shift && !event.composing {
        event.prevent_default();
        on_enter();
      }
    })
    .into()
}

/// The composer's box: the question field, and its bar under it.
fn question_box(ctx: &mut Ctx, (props, state): (&PaneProps, &SideQuestions), follow_up: bool) -> Element {
  let key = if follow_up {
    "side_question.composer.followUp"
  } else {
    "side_question.composer.placeholder"
  };
  let placeholder = ctx.t_args(key, [("run", props.number.to_string().as_str())]);
  let asking = state.asking();
  let ask = {
    let (state, project, task, run) = (state.clone(), props.project.open.clone(), props.task, props.run);
    move || {
      let text = state.draft.get_untracked();
      if !text.trim().is_empty() && !state.asking() {
        state.ask(&project, (task, run), &text);
        state.draft.set(String::new());
      }
    }
  };
  let input = question_input(ctx, state, &placeholder, ask.clone());
  let action = if asking {
    let (state, project) = (state.clone(), props.project.open.clone());
    ActionButton::new("side-question-stop", ButtonKind::Secondary, ctx.t("side_question.stop"))
      .icon(Icon::Square)
      .on_click(move || state.stop(&project))
  } else {
    let button =
      ActionButton::new("side-question-ask", ButtonKind::Primary, ctx.t("side_question.ask")).icon(Icon::ArrowUp);
    if state.draft.get().trim().is_empty() {
      button.disabled()
    } else {
      button.on_click(ask)
    }
  };
  let bar = Row::new()
    .id("side-question-bar")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("side_question.composer.readOnly"))
        .variant(SideText::Quiet.role())
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(action);
  Column::new()
    .id("side-question-box")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Loose.role())
        .left(SideSpacing::Gap.role())
        .right(SideSpacing::Gap.role())
        .bottom(SpacingSize::Sm),
    )
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(RadiusSize::Lg)
    .child(input)
    .child(bar)
    .into()
}
