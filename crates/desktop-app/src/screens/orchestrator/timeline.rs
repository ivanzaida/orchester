//! The conversation (`Chat` in `ZJzV0`): one row per turn, newest last, in a list that keeps the rows out of view
//! unbuilt and opens on the newest turn. A turn is the person's message (`Your message` `mr4lC`) with where it stands
//! (*Sending…*, then *Delivered* and when, `check-check` in `success`), the Orchestrator's answer
//! (`Agent message` `dcd0m` with `waypoints`: its replies as Markdown, their links as `ui::chat::links` shows and
//! follows them, and its actions as cards, `action`), and how the
//! turn ended: while it thinks, the working line with *Stop response* (`Thinking` `PR7nz`); stopped or cut short, an
//! event line (`Event divider` `YKpmi`) and *Ask again*; failed, a danger notice with *Retry* (and *Open
//! Connections* or *Open settings* when the connection is why), or for an earlier turn an event line.

use std::{borrow::Cow, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text, VirtualizedList},
  layout::{
    Alignment,
    scrollbar::{ScrollBarPlacement, ScrollBarStyle},
  },
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::AccountStartProblem,
  id::ProjectId,
  orchestrator::{ActionOutcome, ChatEvent, HeldWork, SessionChange, TurnFailure, TurnState},
};
use orchester_workspace::{display, orchestrator::tools::ChatScope};

use super::{
  held::{HeldCard, HeldProps},
  proposal::{ProposalProps, StartProposal},
  turns::{Part, Turn},
};
use crate::{
  navigation::Navigation,
  routes::{ConnectionsView, ProjectPage, Route},
  theme::{ConversationText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    chat::{
      Avatar, chat_row, delivery_line,
      links::{Shown, present},
      person_message,
    },
    icon::{Icon, icon},
    markdown::{LinkHandler, MarkdownView},
    notice::{Notice, NoticeTone},
  },
};

/// What every turn row needs besides its turn: the project, the connection the Orchestrator runs on, and what the
/// person can do about the last turn.
#[derive(Clone)]
pub(super) struct TurnEnv {
  pub(super) project: ProjectId,
  pub(super) name: Arc<str>,
  /// The name of the connection the Orchestrator runs on, when it has one.
  pub(super) connection: Option<Arc<str>>,
  /// The conversation's tools' scope, once it is kept: proposed starts are decided in it.
  pub(super) scope: Option<ChatScope>,
  /// Stops the answer under way.
  pub(super) stop: Arc<dyn Fn() + Send + Sync>,
  /// Sends the words once more, as a new message.
  pub(super) ask_again: Arc<dyn Fn(Arc<str>) + Send + Sync>,
  /// What a press on a link in a reply does.
  pub(super) links: LinkHandler,
}

/// One row of the list.
#[derive(Clone)]
pub(super) struct TurnRow {
  pub(super) turn: Arc<Turn>,
  /// The newest turn: only its answer can be stopped or asked again.
  pub(super) last: bool,
  /// Nothing is being sent and the Orchestrator has what it runs on.
  pub(super) can_ask: bool,
  /// Its answer is being stopped.
  pub(super) stopping: bool,
  pub(super) env: Arc<TurnEnv>,
}

/// Compared by what it shows; the callbacks are the screen's.
impl PartialEq for TurnRow {
  fn eq(&self, other: &Self) -> bool {
    self.turn == other.turn
      && self.last == other.last
      && self.can_ask == other.can_ask
      && self.stopping == other.stopping
      && self.env.project == other.env.project
      && self.env.name == other.env.name
      && self.env.connection == other.env.connection
      && self.env.scope == other.env.scope
  }
}

impl lurq::app::component::DevtoolsInspectable for TurnRow {}

/// The list of `rows`, opening on the newest.
pub(super) fn list(ctx: &mut Ctx, rows: Vec<TurnRow>) -> Element {
  let newest = rows.last().map(|row| row.turn.key());
  // The thumb keeps a lane of its own, so it never covers a message's end.
  let scrollbar = ScrollBarStyle {
    placement: ScrollBarPlacement::Reserved,
    ..ctx.theme().scrollbar().clone()
  };
  VirtualizedList::new_shared(ctx, Arc::new(rows))
    .list_key("orchestrator-chat")
    .scrollbar(scrollbar)
    .reveal_key(newest)
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .mount_keyed::<TurnView, _, _, _>(|row| row.turn.key(), Clone::clone)
}

struct TurnView;

impl Component for TurnView {
  type Props = TurnRow;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let row = ctx.props::<TurnRow>().clone();
    let key = row.turn.key();
    let delivery = match row.turn.at {
      None => delivery_line(
        "orchestrator-delivery",
        (Icon::LoaderCircle, PaletteColor::TextMuted),
        &ctx.t("orchestrator.delivery.sending"),
      ),
      Some(at) => delivery_line(
        "orchestrator-delivery",
        (Icon::CheckCheck, PaletteColor::Success),
        &ctx.t_args(
          "orchestrator.delivery.delivered",
          [("time", display::clock(at).as_str())],
        ),
      ),
    };
    let message = person_message(
      ctx,
      &format!("orchestrator-message-{key}"),
      (&row.turn.text, row.turn.at),
      Some(delivery),
    );
    let answer = answer(ctx, &row);
    let end = if row.turn.message.is_some() {
      ending(ctx, &row)
    } else {
      Vec::new()
    };
    super::body::turn_room(
      Column::new()
        .id(format!("orchestrator-turn-{key}"))
        .describe("turn", turn_name(&row.turn.state))
        .width(Dimension::Pct(100.0))
        .padding_vertical(SpacingSize::Sm)
        .spacing(SpacingSize::Lg)
        .child(message)
        .with_children(answer)
        .with_children(end),
    )
  }
}

/// The Orchestrator's answer: its replies and actions, under its name, its model and when it began.
fn answer(ctx: &mut Ctx, row: &TurnRow) -> Option<Element> {
  let first = row.turn.answer.first()?;
  let time = display::clock(first.at());
  let meta = match row.turn.model() {
    Some(model) => ctx
      .t_args(
        "orchestrator.answerMeta",
        [("model", model.as_str()), ("time", time.as_str())],
      )
      .to_string(),
    None => time,
  };
  let key = row.turn.key();
  let body = row
    .turn
    .answer
    .iter()
    .enumerate()
    .map(|(index, part)| match part {
      Part::Reply { text: said, .. } => reply(ctx, &format!("orchestrator-reply-{key}-{index}"), said, &row.env.links),
      Part::Action { action, at, seq } => {
        let id = format!("orchestrator-action-{key}-{index}");
        match (&action.outcome, &row.env.scope) {
          (ActionOutcome::Proposed(start), Some(scope)) => ctx.mount_keyed::<StartProposal>(
            &id,
            ProposalProps {
              id: id.clone(),
              scope: scope.clone(),
              seq: *seq,
              task: action.task.clone().unwrap_or_default(),
              start: (**start).clone(),
              at: *at,
            },
          ),
          (ActionOutcome::Done, Some(scope)) if action.held.is_some() => ctx.mount_keyed::<HeldCard>(
            &id,
            HeldProps {
              id: id.clone(),
              project: (row.env.project, row.env.name.to_string()),
              root: scope.root.clone(),
              task: action.task.clone(),
              held: action.held.clone().unwrap_or(HeldWork {
                group: None,
                waiting: Vec::new(),
              }),
              at: *at,
            },
          ),
          _ => super::action::card(ctx, &id, (action, *at), (row.env.project, &row.env.name)),
        }
      }
      Part::Event {
        event: ChatEvent::NewSession { because },
        ..
      } => {
        let words = match because {
          SessionChange::Connection => ctx.t("orchestrator.newSession.connection"),
          SessionChange::Account => ctx.t("orchestrator.newSession.account"),
        };
        event(ctx, &format!("orchestrator-event-{key}-{index}"), Icon::History, &words)
      }
    })
    .collect();
  Some(chat_row(
    format!("orchestrator-answer-{key}"),
    Avatar::Orchestrator,
    (&ctx.t("orchestrator.title"), &meta),
    body,
  ))
}

/// A reply as Markdown, its links shown as the chat follows them (`links`); `id` names it.
fn reply(ctx: &mut Ctx, id: &str, said: &Arc<str>, links: &LinkHandler) -> Element {
  let shown = match present(said, &|shown| link_words(ctx, shown)) {
    Cow::Borrowed(_) => said.clone(),
    Cow::Owned(shown) => Arc::from(shown),
  };
  MarkdownView::new(id, shown, links.clone()).render(ctx)
}

/// How a link to the web is shown with its host, and a link the chat cannot follow as text.
fn link_words(ctx: &Ctx, shown: Shown<'_>) -> String {
  match shown {
    Shown::Web { link, host } => ctx.t_args("orchestrator.link.web", [("link", link), ("host", host)]),
    Shown::Address { text, address } => ctx.t_args("orchestrator.link.address", [("text", text), ("address", address)]),
  }
  .to_string()
}

/// How the turn ended, with what the person can do about the newest one.
fn ending(ctx: &mut Ctx, row: &TurnRow) -> Vec<Element> {
  match &row.turn.state {
    TurnState::Answered => Vec::new(),
    TurnState::Thinking if row.last => thinking(ctx, row),
    TurnState::Thinking => Vec::new(),
    TurnState::Stopped => {
      let key = if row.turn.acted() {
        "orchestrator.stopped.acted"
      } else {
        "orchestrator.stopped.nothing"
      };
      // When it was stopped; a turn kept before the time was recorded says when it was sent.
      let time = row.turn.ended.or(row.turn.at).map(display::clock).unwrap_or_default();
      let words = ctx.t_args(key, [("time", time.as_str())]);
      again(ctx, row, event(ctx, "orchestrator-stopped", Icon::Square, &words))
    }
    TurnState::Failed(TurnFailure::Interrupted) => again(
      ctx,
      row,
      event(
        ctx,
        "orchestrator-interrupted",
        Icon::CirclePause,
        &ctx.t("orchestrator.interrupted"),
      ),
    ),
    TurnState::Failed(failure) if row.last => vec![failure_notice(ctx, row, failure)],
    TurnState::Failed(failure) => vec![event(
      ctx,
      "orchestrator-failed",
      Icon::CircleAlert,
      &earlier(ctx, failure),
    )],
  }
}

/// The working line with *Stop response*, and what stopping does.
fn thinking(ctx: &mut Ctx, row: &TurnRow) -> Vec<Element> {
  let stop = row.env.stop.clone();
  let mut button =
    ActionButton::new("orchestrator-stop", ButtonKind::Ghost, ctx.t("orchestrator.stop")).icon(Icon::Square);
  button = if row.stopping {
    button.loading()
  } else {
    button.on_click(move || stop())
  };
  let line = Row::new()
    .id("orchestrator-thinking")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::LoaderCircle, IconSize::Base, PaletteColor::TextMuted))
        .child(
          Text::new(&ctx.t("orchestrator.thinking"))
            .variant(ExtraTypography::Note.role())
            .nowrap(),
        ),
    )
    .child(Spacer::new().flex(1.0))
    .child(button);
  let note = Text::new(&ctx.t("orchestrator.stopNote"))
    .id("orchestrator-stop-note")
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0));
  vec![
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .child(line)
      .child(note)
      .into(),
  ]
}

/// `event`, then, for the newest turn, *Ask again* and what it does.
fn again(ctx: &Ctx, row: &TurnRow, event: Element) -> Vec<Element> {
  if !row.last {
    return vec![event];
  }
  let ask = row.env.ask_again.clone();
  let words = row.turn.text.clone();
  let mut button = ActionButton::new(
    "orchestrator-ask-again",
    ButtonKind::Secondary,
    ctx.t("orchestrator.askAgain"),
  )
  .icon(Icon::RotateCcw);
  button = if row.can_ask {
    button.on_click(move || ask(words.clone()))
  } else {
    button.disabled()
  };
  let action = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(button)
    .child(
      Text::new(&ctx.t("orchestrator.askAgainNote"))
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    );
  vec![event, action.into()]
}

/// An event line: a `border` rule at each side of a 12 icon and its words in 11/500 `text_secondary`.
fn event(ctx: &Ctx, id: &str, glyph: Icon, words: &str) -> Element {
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  let rule = || {
    Row::new()
      .flex(1.0)
      .height(Dimension::Px(line))
      .background(PaletteColor::Border)
  };
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(rule())
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextSecondary))
    .child(
      Text::new(words)
        .id(format!("{id}-label"))
        .variant(ConversationText::Divider.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(rule())
    .into()
}

/// Why the newest turn failed, with *Retry* and where to fix it.
fn failure_notice(ctx: &mut Ctx, row: &TurnRow, failure: &TurnFailure) -> Element {
  let connection = row.env.connection.clone();
  let unreachable = || match &connection {
    Some(name) => ctx.t_args("orchestrator.failure.unreachable", [("connection", &**name)]),
    None => ctx.t("orchestrator.failure.unreachableUnknown"),
  };
  let (title, body, fix) = match failure {
    TurnFailure::NotSignedIn => (
      unreachable(),
      ctx.t("orchestrator.failure.notSignedIn"),
      Fix::Connections,
    ),
    TurnFailure::ConnectionGone => (
      unreachable(),
      ctx.t("orchestrator.failure.connectionGone"),
      Fix::Settings,
    ),
    TurnFailure::Account(problem) => (unreachable(), account(ctx, problem), Fix::Connections),
    TurnFailure::CannotStart { .. } => (unreachable(), ctx.t("orchestrator.failure.cannotStart"), Fix::None),
    TurnFailure::Failed { reason } => (
      ctx.t("orchestrator.failure.failed"),
      ctx.t_args("orchestrator.failure.failedBody", [("reason", reason.as_str())]),
      Fix::None,
    ),
    TurnFailure::Interrupted => (
      ctx.t("orchestrator.failure.failed"),
      ctx.t("orchestrator.interrupted"),
      Fix::None,
    ),
    TurnFailure::TimeLimit { minutes } => (
      ctx.t("orchestrator.failure.failed"),
      ctx.t_args("orchestrator.failure.timeLimit", [("minutes", minutes.to_string())]),
      Fix::None,
    ),
  };
  let mut notice = Notice::new("orchestrator-failure", NoticeTone::Danger, title)
    .icon(Icon::CloudOff)
    .body(body);
  let ask = row.env.ask_again.clone();
  let words = row.turn.text.clone();
  let mut retry = ActionButton::new(
    "orchestrator-retry",
    ButtonKind::Secondary,
    ctx.t("orchestrator.failure.retry"),
  )
  .icon(Icon::RotateCcw);
  retry = if row.can_ask {
    retry.on_click(move || ask(words.clone()))
  } else {
    retry.disabled()
  };
  notice = notice.action(retry);
  if let Some(open) = fix.button(ctx, row.env.project) {
    notice = notice.action(open);
  }
  notice.into()
}

/// Where the person fixes what kept the Orchestrator from answering.
#[derive(Clone, Copy)]
enum Fix {
  Connections,
  Settings,
  None,
}

impl Fix {
  fn button(self, ctx: &mut Ctx, project: ProjectId) -> Option<ActionButton> {
    let (id, key, page) = match self {
      Self::Connections => (
        "orchestrator-open-connections",
        "orchestrator.failure.openConnections",
        ProjectPage::Connections(ConnectionsView::default()),
      ),
      Self::Settings => (
        "orchestrator-open-settings",
        "orchestrator.notReady.openSettings",
        ProjectPage::OrchestratorSettings,
      ),
      Self::None => return None,
    };
    let navigation = Navigation::of(ctx);
    let route = Route::Project(project, page);
    Some(
      ActionButton::new(id, ButtonKind::Primary, ctx.t(key))
        .icon(Icon::ArrowUpRight)
        .on_click(move || navigation.go(route.clone())),
    )
  }
}

/// Why the connection's account cannot start the CLI.
fn account(ctx: &Ctx, problem: &AccountStartProblem) -> Arc<str> {
  match problem {
    AccountStartProblem::NoAccount => ctx.t("orchestrator.failure.accountNone"),
    AccountStartProblem::FolderMissing { folder } => ctx.t_args(
      "orchestrator.failure.accountFolderMissing",
      [("folder", folder.display().to_string().as_str())],
    ),
    AccountStartProblem::Unsupported => ctx.t("orchestrator.failure.accountUnsupported"),
  }
}

/// An earlier turn's failure, on its event line.
fn earlier(ctx: &Ctx, failure: &TurnFailure) -> Arc<str> {
  match failure {
    TurnFailure::NotSignedIn => ctx.t("orchestrator.failure.earlierNotSignedIn"),
    TurnFailure::Failed { reason } => ctx.t_args("orchestrator.failure.earlier", [("reason", reason.as_str())]),
    TurnFailure::TimeLimit { minutes } => ctx.t_args(
      "orchestrator.failure.earlierTimeLimit",
      [("minutes", minutes.to_string())],
    ),
    TurnFailure::ConnectionGone
    | TurnFailure::Account(_)
    | TurnFailure::CannotStart { .. }
    | TurnFailure::Interrupted => ctx.t("orchestrator.failure.earlierUnreachable"),
  }
}

/// The name `describe` reports a turn by, for tests and QA.
const fn turn_name(state: &TurnState) -> &'static str {
  match state {
    TurnState::Thinking => "thinking",
    TurnState::Answered => "answered",
    TurnState::Stopped => "stopped",
    TurnState::Failed(_) => "failed",
  }
}
