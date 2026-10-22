//! A feedback thread drawn under its lines in Files changed (`Thread wrap` `p6wOsh`, `Thread · …` `nkpzr`): its
//! head (kind, where and to whom, state), the messages, what happened to the request, the agent's work or commit,
//! the notes, and its foot with what can be done now (`Thread foot` `N8FeN`); a reply opens under it. A resolved
//! thread stands collapsed on one line with *Reopen* (`abRGk`), and lines with several threads list them under a
//! line that says how many (`BHXTq`).

use std::{num::NonZeroU64, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{pull_request::feedback::Located, revision::Revision};
use orchester_workspace::pull_requests::feedback::{
  BoardTask, FeedbackBoard, Hold, NotDelivered, ThreadState, ThreadView,
};

use super::{
  Composing, FeedbackState, Held, Write, Writes,
  parts::{activity, events, messages, notes, status, visibility},
  reply::{ReplyProps, reply},
  words,
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// Where a thread stands in the diff: past the line numbers and the sign, 24 from the pane's end, 8 over it and 10
/// under it (`Thread wrap` `p6wOsh`).
const WRAP_START: f32 = 112.0;

/// What a thread is drawn with.
#[derive(Clone)]
pub struct Env {
  pub project: ShownProject,
  pub number: NonZeroU64,
  pub head: Option<Revision>,
  pub state: FeedbackState,
  pub writes: Writes,
  pub board: Arc<FeedbackBoard>,
  pub navigation: Navigation,
}

impl Env {
  fn task(&self) -> Option<&BoardTask> {
    self.board.task.as_ref()
  }

  fn key(&self) -> String {
    words::task_key(self.task())
  }
}

/// A thread under its lines, in the diff's pane.
pub fn wrapped(ctx: &mut Ctx, env: &Env, thread: &ThreadView) -> Element {
  if thread.state.is_open() {
    wrap(card(ctx, env, thread))
  } else {
    wrap(collapsed(ctx, env, thread))
  }
}

/// `inner` where a thread stands in the diff's pane (`Thread wrap` `p6wOsh`).
pub fn wrap(inner: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .bottom(ExtraSpacing::Loose.role())
        .left(WRAP_START)
        .right(SpacingSize::Xl),
    )
    .child(inner)
    .into()
}

/// An open thread's card.
pub fn card(ctx: &mut Ctx, env: &Env, thread: &ThreadView) -> Element {
  let id = thread.id;
  let task = env.key();
  let head = Row::new()
    .width(Dimension::Pct(100.0))
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
      Text::new(&words::card_anchor(ctx, thread, (env.head.as_ref(), &task)))
        .id(format!("feedback-{id}-anchor"))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap()
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(Row::new().flex(1.0))
    .child(status(ctx, &format!("feedback-{id}-state"), thread));
  let shows_events = matches!(
    thread.state,
    ThreadState::Queued(_) | ThreadState::Delivered(_) | ThreadState::Working { .. } | ThreadState::NotDelivered { .. }
  );
  let conversation = conversation_button(ctx, env, &format!("feedback-{id}-conversation"));
  let mut column = Column::new()
    .id(format!("feedback-{id}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(head)
    .with_children(messages(ctx, thread, &task));
  if shows_events {
    column = column.with_children(events(ctx, thread, (&task, &env.board.threads)));
  }
  if let (Some(task), Some(conversation)) = (env.task(), conversation) {
    column = column.with_children(activity(ctx, thread, task, conversation));
  }
  if let Some(task) = env.task() {
    column = column.with_children(notes(ctx, thread, (task, env.head.as_ref())));
  }
  column = column.child(foot(ctx, env, thread));
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

/// *Open conversation*: the task's conversation, where the agent works on the request.
pub fn conversation_button(ctx: &Ctx, env: &Env, id: &str) -> Option<ActionButton> {
  let task = env.task()?;
  let to = Route::Project(
    env.project.open.project.id(),
    ProjectPage::Task(task.key.clone(), TaskView::of(TaskTab::Conversation)),
  );
  Some(
    ActionButton::new(
      id.to_owned(),
      ButtonKind::Ghost,
      ctx.t("pull_requests.feedback.strip.openConversation"),
    )
    .icon(Icon::MessagesSquare)
    .small()
    .opens(env.navigation.to(to)),
  )
}

/// The card's foot: that it stays in Orchester, and what can be done now.
fn foot(ctx: &Ctx, env: &Env, thread: &ThreadView) -> Row {
  Row::new()
    .id(format!("feedback-{}-foot", thread.id))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(visibility(ctx, &format!("feedback-{}-visibility", thread.id), false))
    .child(Row::new().flex(1.0))
    .with_children(actions(ctx, env, thread, false))
}

/// What can be done with a thread now; `panel`: beside the list, where an answered question also offers a fix.
pub fn actions(ctx: &Ctx, env: &Env, thread: &ThreadView, panel: bool) -> Vec<Element> {
  let act = Act { ctx, env, thread };
  let fix = orchester_core::pull_request::feedback::FeedbackKind::Fix;
  if matches!(thread.at_head, Some(Located::Outdated)) && thread.state.is_open() {
    return vec![
      show_at(ctx, env, thread).into(),
      act.reply().into(),
      act.resolve(ButtonKind::Secondary).into(),
    ];
  }
  match &thread.state {
    ThreadState::Queued(hold) => act.queued(hold),
    ThreadState::Delivered(_) | ThreadState::Working { .. } => vec![act.reply().into()],
    ThreadState::NotDelivered { why, .. } => act.undelivered(*why),
    ThreadState::Answered { .. } if panel => vec![
      act.reply().into(),
      act
        .button(
          "fix",
          ButtonKind::Ghost,
          "pull_requests.feedback.actions.requestFix",
          Icon::Wrench,
        )
        .on_click(act.reply_as(fix))
        .into(),
      act.resolve(ButtonKind::Ghost).into(),
    ],
    ThreadState::Answered { .. } => vec![act.reply().into(), act.resolve(ButtonKind::Secondary).into()],
    ThreadState::ReadyToVerify { .. } => vec![
      act
        .button(
          "not-fixed",
          ButtonKind::Secondary,
          "pull_requests.feedback.actions.notFixed",
          Icon::Reply,
        )
        .on_click(act.reply_as(fix))
        .into(),
      act.resolve(ButtonKind::Primary).into(),
    ],
    ThreadState::Resolved { .. } => vec![
      act
        .button(
          "reopen",
          ButtonKind::Ghost,
          "pull_requests.feedback.actions.reopen",
          Icon::RotateCcw,
        )
        .on_click(act.write(Write::Resolve(thread.id, false)))
        .into(),
    ],
    ThreadState::Withdrawn { .. } => Vec::new(),
  }
}

/// Builds a thread's actions.
struct Act<'a> {
  ctx: &'a Ctx,
  env: &'a Env,
  thread: &'a ThreadView,
}

impl Act<'_> {
  fn button(&self, name: &str, kind: ButtonKind, key: &str, glyph: Icon) -> ActionButton {
    ActionButton::new(format!("feedback-{}-{name}", self.thread.id), kind, self.ctx.t(key))
      .icon(glyph)
      .small()
  }

  fn write(&self, write: Write) -> impl Fn() + Send + Sync + 'static {
    let writes = self.env.writes.clone();
    move || writes.run(write.clone(), None)
  }

  /// Opens the thread's reply, asking `kind`.
  fn reply_as(&self, kind: orchester_core::pull_request::feedback::FeedbackKind) -> impl Fn() + Send + Sync + 'static {
    let (composing, drafts, id) = (
      self.env.state.composing.clone(),
      self.env.state.drafts.clone(),
      self.thread.id,
    );
    move || {
      drafts.of(&Composing::Reply(id), kind).kind.set(Held(kind));
      composing.set(Some(Composing::Reply(id)));
    }
  }

  fn reply(&self) -> ActionButton {
    self
      .button(
        "reply",
        ButtonKind::Ghost,
        "pull_requests.feedback.actions.reply",
        Icon::Reply,
      )
      .on_click(self.reply_as(self.thread.kind))
  }

  fn resolve(&self, kind: ButtonKind) -> ActionButton {
    self
      .button("resolve", kind, "pull_requests.feedback.actions.resolve", Icon::Check)
      .on_click(self.write(Write::Resolve(self.thread.id, true)))
  }

  /// *Withdraw request* while Orchester holds it; *Open … limits* when the task's limit holds it.
  fn queued(&self, hold: &Hold) -> Vec<Element> {
    let mut shown: Vec<Element> = self
      .thread
      .held
      .map(|post| {
        self
          .button(
            "withdraw",
            ButtonKind::Ghost,
            "pull_requests.feedback.actions.withdraw",
            Icon::Undo2,
          )
          .on_click(self.write(Write::Withdraw(self.thread.id, post)))
          .into()
      })
      .into_iter()
      .collect();
    if *hold == Hold::Limit {
      shown.extend(limits(self.ctx, self.env));
    }
    shown
  }

  /// *Edit*, which writes the request again, and *Retry* when the agent's program did not take it.
  fn undelivered(&self, why: NotDelivered) -> Vec<Element> {
    let id = self.thread.id;
    let edit = {
      let (composing, drafts, body, kind) = (
        self.env.state.composing.clone(),
        self.env.state.drafts.clone(),
        self.thread.current().body.to_string(),
        self.thread.kind,
      );
      self
        .button(
          "edit",
          ButtonKind::Ghost,
          "pull_requests.feedback.actions.edit",
          Icon::Pencil,
        )
        .on_click(move || {
          drafts.of(&Composing::Reply(id), kind).text.set(body.clone());
          composing.set(Some(Composing::Reply(id)));
        })
    };
    let retry = match (why, self.thread.undelivered) {
      (NotDelivered::Cli(_), Some(message)) => Some(
        self
          .button(
            "retry",
            ButtonKind::Secondary,
            "pull_requests.feedback.actions.retry",
            Icon::RefreshCw,
          )
          .on_click(self.write(Write::Retry(id, message))),
      ),
      _ => None,
    };
    std::iter::once(edit.into()).chain(retry.map(Into::into)).collect()
  }
}

/// *Open KON-12 limits*: the task's Summary, where its limit is raised.
fn limits(ctx: &Ctx, env: &Env) -> Option<Element> {
  let task = env.task()?;
  let to = Route::Project(
    env.project.open.project.id(),
    ProjectPage::Task(task.key.clone(), TaskView::of(TaskTab::Summary)),
  );
  Some(
    ActionButton::new(
      "feedback-limits",
      ButtonKind::Secondary,
      ctx.t_args(
        "pull_requests.feedback.actions.limits",
        [("task", task.key.to_string())],
      ),
    )
    .icon(Icon::SlidersHorizontal)
    .small()
    .opens(env.navigation.to(to))
    .into(),
  )
}

/// *Show at 7c1e9f2*: Files changed at the revision an outdated thread was written on, its lines in view.
fn show_at(ctx: &Ctx, env: &Env, thread: &ThreadView) -> ActionButton {
  let revision = orchester_workspace::display::short_revision(thread.revision.as_str()).to_owned();
  let (pinned, reveal, navigation) = (
    env.state.pinned.clone(),
    env.state.reveal.clone(),
    env.navigation.clone(),
  );
  let (project, number, id, at, path) = (
    env.project.open.project.id(),
    env.number,
    thread.id,
    thread.revision.clone(),
    thread.path().map(str::to_owned),
  );
  ActionButton::new(
    format!("feedback-{id}-show-at"),
    ButtonKind::Ghost,
    ctx.t_args("pull_requests.feedback.actions.showAt", [("revision", revision)]),
  )
  .icon(Icon::History)
  .small()
  .on_click(move || {
    pinned.set(Held(Some(at.clone())));
    reveal.set(Held(Some(id)));
    navigation.replace(Route::Project(project, ProjectPage::PullRequest(number, path.clone())));
  })
}

/// A resolved or withdrawn thread on one line: its state, kind and lines, the start of what it asked, when, and
/// *Reopen* (`abRGk`).
fn collapsed(ctx: &mut Ctx, env: &Env, thread: &ThreadView) -> Element {
  let current = thread.current();
  let lines = thread
    .anchor
    .lines()
    .map(|lines| words::lines(ctx, (None, lines.span())))
    .unwrap_or_else(|| ctx.t("pull_requests.feedback.anchor.wholeShort"));
  Row::new()
    .id(format!("feedback-{}", thread.id))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(status(ctx, &format!("feedback-{}-state", thread.id), thread))
    .child(
      Text::new(&words::kind_word(ctx, thread.kind))
        .variant(TypographyStyle::Label)
        .nowrap(),
    )
    .child(Text::new(&lines).variant(ExtraTypography::MonoNote.role()).nowrap())
    .child(
      Text::new(&current.body)
        .variant(TypographyStyle::Description)
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Text::new(&ctx.t_args(
        "pull_requests.feedback.meta.written",
        [("time", words::time(current.at))],
      ))
      .variant(TypographyStyle::Caption)
      .nowrap(),
    )
    .with_children(actions(ctx, env, thread, false))
    .into()
}

/// Several threads end on one line: how many, *Add feedback on this line*, and a line for each (`BHXTq`); a press
/// on one opens it beside the Agent feedback list.
pub fn several(
  ctx: &mut Ctx,
  env: &Env,
  (line, threads): (u32, &[&ThreadView]),
  add: Option<Arc<dyn Fn() + Send + Sync>>,
) -> Element {
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t_args(
        "pull_requests.feedback.actions.threadsOnLine",
        [("count", threads.len().to_string()), ("line", line.to_string())],
      ))
      .variant(TypographyStyle::Caption)
      .nowrap(),
    )
    .child(Row::new().flex(1.0))
    .with_children(add.map(|add| {
      ActionButton::new(
        format!("feedback-add-{line}"),
        ButtonKind::Ghost,
        ctx.t("pull_requests.feedback.actions.addOnLine"),
      )
      .icon(Icon::Plus)
      .small()
      .on_click(move || add())
    }));
  let navigation = Navigation::of(ctx);
  let (project, number) = (env.project.open.project.id(), env.number);
  let lines = threads.iter().map(|thread| {
    let (id, navigation) = (thread.id, navigation.clone());
    let span = thread
      .anchor
      .lines()
      .map(|lines| words::lines(ctx, (None, lines.span())))
      .unwrap_or_default();
    crate::ui::pressable::pressable(move || {
      navigation.replace(Route::Project(
        project,
        ProjectPage::PullRequestFeedback(number, Some(id)),
      ));
    })
    .id(format!("feedback-{id}-line"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(status(ctx, &format!("feedback-{id}-state"), thread))
    .child(
      Text::new(&words::kind_word(ctx, thread.kind))
        .variant(TypographyStyle::Label)
        .nowrap(),
    )
    .child(Text::new(&span).variant(ExtraTypography::MonoNote.role()).nowrap())
    .child(
      Text::new(&thread.current().body)
        .variant(TypographyStyle::Description)
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
  });
  wrap(
    Column::new()
      .id(format!("feedback-several-{line}"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
      .spacing(SpacingSize::Sm)
      .background(PaletteColor::SurfacePanel)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(ExtraRadius::Tile.role())
      .child(head)
      .with_children(lines.collect::<Vec<Element>>())
      .into(),
  )
}
