//! The conversation (`Chat` in `ZJzV0`): one row per turn, newest last, in a list that keeps the rows out of view
//! unbuilt and opens at the very end of the newest turn, following it (`ui::chat::follow`). A turn is the person's
//! message (`Your message` `mr4lC`) with where it stands
//! (*Sending…*, then *Delivered* and when, `check-check` in `success`), the Orchestrator's answer
//! (`Agent message` `dcd0m` with `waypoints`: its replies as every chat draws what an agent wrote,
//! `ui::chat::agent_text`, and its actions as cards, `action`), and how the turn ended: while it thinks, the working
//! line with *Stop response* (`Thinking` `PR7nz`); stopped or cut short, an
//! event line (`Event divider` `YKpmi`) and *Ask again*; failed, a danger notice with *Retry* (and *Open
//! Connections* or *Open settings* when the connection is why, `failure`), or for an earlier turn an event line. A
//! wait of an agent of the project opens a row of its own with its card (`wait`, linking to the task's Conversation),
//! and the answer and ending of the turn it started by itself, if any; such a turn ends without *Ask again* or
//! *Retry*.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Stack, Text, VirtualizedList},
  layout::{
    Alignment, StackAlignment,
    scrollbar::{ScrollBarPlacement, ScrollBarStyle},
  },
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::{ActionOutcome, ChatAction, ChatEvent, HeldWork, SessionChange, TurnFailure, TurnState},
  time::Timestamp,
};
use orchester_workspace::{display, orchestrator::tools::ChatScope};

use super::{
  change::{ChangeCard, ChangeProps},
  failure::{earlier, failure_notice},
  held::{HeldCard, HeldProps},
  proposal::{ProposalProps, StartProposal},
  turns::{Part, Turn},
};
use crate::{
  theme::{ConversationText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    chat::{
      Avatar, agent_text, chat_row, delivery_line,
      follow::{Follow, jump_room},
      names::ProjectNames,
      person_message,
    },
    icon::{Icon, icon},
    markdown::LinkHandler,
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
  /// The projects' names, for what a link to another project's place says.
  pub(super) names: ProjectNames,
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
      && self.env.names == other.env.names
  }
}

impl lurq::app::component::DevtoolsInspectable for TurnRow {}

/// A row of the list: a turn, or the room under the last one while *Jump to latest* floats over the chat's foot, so
/// the turn can scroll clear of it.
#[derive(Clone, PartialEq)]
enum Line {
  Turn(TurnRow),
  JumpRoom,
}

impl lurq::app::component::DevtoolsInspectable for Line {}

impl Line {
  fn key(&self) -> String {
    match self {
      Self::Turn(row) => row.turn.key(),
      Self::JumpRoom => "orchestrator-jump-room".to_owned(),
    }
  }
}

/// The list of `rows`, as `follow` follows it (`ui::chat::follow`): it opens at the very end of the newest turn and
/// keeps it in view as turns come and replies grow, until the person scrolls up; then *Jump to latest* floats over
/// its foot.
pub(super) fn list(ctx: &mut Ctx, follow: &Follow, rows: Vec<TurnRow>) -> Element {
  let arrived = arrivals(&rows);
  let reveal = follow.follow(rows.len(), arrived, None);
  let following = follow.following.get();
  let lines: Vec<Line> = rows
    .into_iter()
    .map(Line::Turn)
    .chain((!following).then_some(Line::JumpRoom))
    .collect();
  // The thumb keeps a lane of its own, so it never covers a message's end.
  let scrollbar = ScrollBarStyle {
    placement: ScrollBarPlacement::Reserved,
    ..ctx.theme().scrollbar().clone()
  };
  let list = follow
    .list(VirtualizedList::new_shared(ctx, Arc::new(lines)), arrived, reveal)
    .list_key("orchestrator-chat")
    .scrollbar(scrollbar)
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .mount_keyed::<TurnView, _, _, _>(Line::key, Clone::clone);
  let jump = (!following).then(|| follow.jump(ctx, arrived));
  let turns = Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(list)
    .child(follow.end(ctx, "orchestrator-follow-end"));
  Stack::new()
    .id("orchestrator-chat")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .stack_align(StackAlignment::BottomCenter)
    .child(turns)
    .with_children(jump)
    .into()
}

/// What the person can see arrive, counted: each turn, each part of its answer, and its end. Anything new moves a
/// following chat to its end, and *N new* counts it.
fn arrivals(rows: &[TurnRow]) -> usize {
  rows
    .iter()
    .map(|row| {
      let ended = row
        .turn
        .state
        .as_ref()
        .is_some_and(|state| *state != TurnState::Thinking);
      1 + row.turn.answer.len() + usize::from(ended)
    })
    .sum()
}

struct TurnView;

impl Component for TurnView {
  type Props = Line;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Line::Turn(row) = ctx.props::<Line>().clone() else {
      return jump_room();
    };
    let key = row.turn.key();
    // A wait of an agent and a daily report open their row with their own card (`super::wait`, `super::report`); a
    // person's turn with their message.
    let opening = match (&row.turn.wait, &row.turn.report) {
      (Some(wait), _) => super::wait::card(
        ctx,
        &format!("orchestrator-wait-{key}"),
        (wait, row.turn.at, row.turn.state.is_some()),
        row.env.project,
      ),
      (None, Some(report)) => super::report::card(
        ctx,
        &format!("orchestrator-report-{key}"),
        (report, row.turn.at, row.turn.state.is_some()),
        &row.env,
      ),
      (None, None) => opening_message(ctx, &row, &key),
    };
    let answer = answer(ctx, &row);
    let end = if row.turn.message.is_some() {
      ending(ctx, &row)
    } else {
      Vec::new()
    };
    super::body::turn_room(
      Column::new()
        .id(format!("orchestrator-turn-{key}"))
        .describe("turn", turn_name(row.turn.state.as_ref()))
        .width(Dimension::Pct(100.0))
        .padding_vertical(SpacingSize::Sm)
        .spacing(SpacingSize::Lg)
        .child(opening)
        .with_children(answer)
        .with_children(end),
    )
    .into()
  }
}

/// The person's message that opens `row`'s turn, keyed `key`, with where it stands.
fn opening_message(ctx: &mut Ctx, row: &TurnRow, key: &str) -> Element {
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
  person_message(
    ctx,
    &format!("orchestrator-message-{key}"),
    (&row.turn.text, row.turn.at),
    Some(delivery),
  )
  .into()
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
  let parts: Vec<Element> = row
    .turn
    .answer
    .iter()
    .enumerate()
    .map(|(index, part)| match part {
      Part::Reply { text: said, .. } => agent_text(
        ctx,
        &format!("orchestrator-reply-{key}-{index}"),
        said,
        (row.env.project, &row.env.names),
        &row.env.links,
      ),
      Part::Action { action, at, seq } => action_card(
        ctx,
        row,
        &format!("orchestrator-action-{key}-{index}"),
        (action, *at, *seq),
      ),
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
  // The answer's parts, 10 apart: replies, cards and events (`b55NqO`'s *Answer*).
  let body = Column::new()
    .id(format!("orchestrator-answer-{key}-parts"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .with_children(parts);
  Some(chat_row(
    format!("orchestrator-answer-{key}"),
    Avatar::Orchestrator,
    (&ctx.t("orchestrator.title"), &meta),
    vec![body.into()],
  ))
}

/// The card of `action`, taken at `at` as entry `seq` of the conversation; `id` names it.
fn action_card(ctx: &mut Ctx, row: &TurnRow, id: &str, (action, at, seq): (&ChatAction, Timestamp, i64)) -> Element {
  let id = id.to_owned();
  if action.change.is_some() || action.auto_mode {
    return ctx.mount_keyed::<ChangeCard>(
      &id,
      ChangeProps {
        id: id.clone(),
        scope: row.env.scope.clone(),
        project: (row.env.project, Arc::from(row.env.name.as_ref())),
        seq,
        action: action.clone(),
        at,
      },
    );
  }
  match (&action.outcome, &row.env.scope) {
    (ActionOutcome::Proposed(start), Some(scope)) => ctx.mount_keyed::<StartProposal>(
      &id,
      ProposalProps {
        id: id.clone(),
        scope: scope.clone(),
        seq,
        task: action.task.clone().unwrap_or_default(),
        start: (**start).clone(),
        applying: action.claimed.is_some(),
        at,
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
        at,
      },
    ),
    _ => super::action::card(ctx, &id, (action, at), (row.env.project, &row.env.name)),
  }
}

/// How the turn ended, with what the person can do about the newest one; nothing for a wait that started no turn.
/// A turn a wait started has no words of the person's to send again, so it ends with its event line alone.
fn ending(ctx: &mut Ctx, row: &TurnRow) -> Vec<Element> {
  let Some(state) = &row.turn.state else {
    return Vec::new();
  };
  match state {
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
    TurnState::Failed(failure) if row.last && !row.turn.on_its_own() => vec![failure_notice(ctx, row, failure)],
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

/// `event`, then, for the newest turn a person's message started, *Ask again* and what it does.
fn again(ctx: &Ctx, row: &TurnRow, event: Element) -> Vec<Element> {
  if !row.last || row.turn.on_its_own() {
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

/// The name `describe` reports a turn by, for tests and QA; `none` for a wait that started no turn.
const fn turn_name(state: Option<&TurnState>) -> &'static str {
  match state {
    None => "none",
    Some(TurnState::Thinking) => "thinking",
    Some(TurnState::Answered) => "answered",
    Some(TurnState::Stopped) => "stopped",
    Some(TurnState::Failed(_)) => "failed",
  }
}
