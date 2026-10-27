//! The conversation (`Chat` `ECQg8` in `01 Conversation` `wRSL4`): one row per turn, newest last, 18 apart, in a list
//! that keeps the rows out of view unbuilt and opens at the very end of the newest turn, following it
//! (`ui::chat::follow`). A turn is the person's message (`Your message`, right-aligned on `own_message`) with where it
//! stands (*Sending…*, then *Delivered* and when), the Orchestrator's answer ([`super::answer`]), and how the turn
//! ended ([`super::ending`]). A wait of an agent of the project opens a row of its own with its line
//! ([`super::wait`]), what the turn it started by itself said and did nested under it as a quote; the end of a run its
//! line ([`super::run_end`]) and a daily report its card ([`super::report`]), with the turn they started, if any.
//! Waits for the person kept one after another, with no message of theirs between them, gather into one digest
//! ([`super::digest`]).

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Column, Stack, Text, VirtualizedList},
  layout::{
    StackAlignment,
    scrollbar::{ScrollBarPlacement, ScrollBarStyle},
  },
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ChatMessageId, ProjectId},
  orchestrator::{Queued, TurnState, WaitKind},
};
use orchester_workspace::orchestrator::tools::ChatScope;

use super::{
  digest::{Digest, DigestProps, Waited, for_the_person},
  report::{ReportCard, ReportProps},
  send::QueuedChange,
  turns::{Arrival, Turn},
  wait::Status,
};
use crate::{
  theme::OrchestratorSpacing,
  ui::{
    chat::{
      follow::{Follow, jump_room},
      names::ProjectNames,
    },
    markdown::LinkHandler,
  },
};

/// What every turn row needs besides its turn: the project, the connection the Orchestrator runs on, and what the
/// person can do about the last turn and the messages that wait for it.
#[derive(Clone)]
pub(super) struct TurnEnv {
  pub(super) project: ProjectId,
  pub(super) name: Arc<str>,
  /// The project's folder, whose data keeps the files attached to the messages.
  pub(super) root: PathBuf,
  /// The name of the connection the Orchestrator runs on, when it has one.
  pub(super) connection: Option<Arc<str>>,
  /// The conversation's tools' scope, once it is kept: proposed starts are decided in it.
  pub(super) scope: Option<ChatScope>,
  /// Stops the answer under way.
  pub(super) stop: Arc<dyn Fn() + Send + Sync>,
  /// Sends the words once more, as a new message.
  pub(super) ask_again: Arc<dyn Fn(Arc<str>) + Send + Sync>,
  /// Opens the composer's runs-on picker, where another account is chosen.
  pub(super) pick_account: Arc<dyn Fn() + Send + Sync>,
  /// Takes a queued message, with its words, back into the composer, or removes it.
  pub(super) change_queued: Arc<dyn Fn(ChatMessageId, Arc<str>, QueuedChange) + Send + Sync>,
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

/// A row of the list: a turn, a digest of waits for the person, a note under the last turn, or the room under the
/// last one while *Jump to latest* floats over the chat's foot, so the turn can scroll clear of it.
#[derive(Clone, PartialEq)]
enum Line {
  /// A turn; `first` is the list's first row, which keeps the chat's top room over it.
  Turn {
    row: TurnRow,
    first: bool,
  },
  Digest {
    digest: DigestProps,
    first: bool,
  },
  /// Words under the last turn in 12 `text_muted` (`Note` `JHr2a` in `02 Not set up` `Wd5CL`).
  Note(Arc<str>),
  JumpRoom,
}

impl lurq::app::component::DevtoolsInspectable for Line {}

impl Line {
  fn key(&self) -> String {
    match self {
      Self::Turn { row, .. } => row.turn.key(),
      Self::Digest { digest, .. } => digest.id.clone(),
      Self::Note(_) => "orchestrator-note".to_owned(),
      Self::JumpRoom => "orchestrator-jump-room".to_owned(),
    }
  }
}

/// The list of `rows`, as `follow` follows it (`ui::chat::follow`), with `note` under the last turn: it opens at the
/// very end of the newest turn and keeps it in view as turns come and replies grow, until the person scrolls up; then
/// *Jump to latest* floats over its foot.
pub(super) fn list(ctx: &mut Ctx, follow: &Follow, rows: Vec<TurnRow>, note: Option<Arc<str>>) -> Element {
  let arrived = arrivals(&rows);
  let reveal = follow.follow(rows.len(), arrived, None);
  let following = follow.following.get();
  let mut lines = gathered(rows);
  lines.extend(note.map(Line::Note));
  if !following {
    lines.push(Line::JumpRoom);
  }
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

/// A row that goes in a digest: a wait or a run's end for the person that started no turn and still stands; one dealt
/// with since keeps its own line.
fn digested(row: &TurnRow) -> Option<Waited> {
  let wait = row.turn.wait.as_deref()?;
  (row.turn.state.is_none() && row.turn.stands && for_the_person(wait)).then(|| Waited {
    key: row.turn.key(),
    wait: wait.clone(),
    at: row.turn.at,
  })
}

/// `rows` as the list's lines: two or more waits for the person kept one after another gather into a digest.
fn gathered(rows: Vec<TurnRow>) -> Vec<Line> {
  let mut lines: Vec<Line> = Vec::new();
  let mut run: Vec<TurnRow> = Vec::new();
  let flush = |lines: &mut Vec<Line>, run: &mut Vec<TurnRow>| {
    let first = lines.is_empty();
    match run.len() {
      0 => {}
      1 => lines.push(Line::Turn {
        row: run.remove(0),
        first,
      }),
      _ => {
        let project = run[0].env.project;
        let waits: Vec<Waited> = run.drain(..).filter_map(|row| digested(&row)).collect();
        lines.push(Line::Digest {
          digest: DigestProps {
            id: format!("orchestrator-digest-{}", waits[0].key),
            project,
            waits,
          },
          first,
        });
      }
    }
  };
  for row in rows {
    if digested(&row).is_some() {
      run.push(row);
      continue;
    }
    flush(&mut lines, &mut run);
    let first = lines.is_empty();
    lines.push(Line::Turn { row, first });
  }
  flush(&mut lines, &mut run);
  lines
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
        .is_some_and(|state| state.has_turn() && !state.is_thinking());
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
    match ctx.props::<Line>().clone() {
      Line::Turn { row, first } => turn(ctx, &row, first),
      Line::Digest { digest, first } => {
        let key = digest.id.clone();
        let mounted = ctx.mount_keyed::<Digest>(&key, digest);
        room(Column::new().width(Dimension::Pct(100.0)).child(mounted), first)
      }
      Line::Note(words) => room(
        Column::new().width(Dimension::Pct(100.0)).child(
          Text::new(&words)
            .id("orchestrator-not-ready")
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextMuted)
            .width(Dimension::Pct(100.0)),
        ),
        false,
      ),
      Line::JumpRoom => jump_room(),
    }
  }
}

/// A turn's row: what opens it, the answer and how it ended.
fn turn(ctx: &mut Ctx, row: &TurnRow, first: bool) -> Element {
  let key = row.turn.key();
  // A wait of an agent, a run's end and a daily report open their row with their own line or card; a person's turn
  // with their message. What a wait's turn said nests under its line.
  let (opening, answer) = match (&row.turn.wait, &row.turn.report) {
    (Some(wait), _) => {
      let id = format!("orchestrator-wait-{key}");
      let turned = row.turn.state.is_some();
      match wait.kind {
        WaitKind::Ended(end) => {
          let line = super::run_end::line(ctx, &id, (wait, end, row.turn.at, turned), row.env.project);
          (line, super::answer::answer(ctx, row))
        }
        _ => {
          let status = Status::of(wait, row.turn.stands, row.turn.state.as_ref(), &row.turn.answer);
          let line = super::wait::line(ctx, &id, (wait, row.turn.at, turned), (status, row.env.project));
          (line, super::answer::nested(ctx, row))
        }
      }
    }
    (None, Some(report)) => {
      let id = format!("orchestrator-report-{key}");
      let props = ReportProps {
        id: id.clone(),
        report: (**report).clone(),
        at: row.turn.at,
        turned: row.turn.state.is_some(),
      };
      let card = ctx.mount_keyed::<ReportCard>(&id, props);
      (card, super::answer::answer(ctx, row))
    }
    (None, None) => (
      super::delivery::opening_message(ctx, row, &key),
      super::answer::answer(ctx, row),
    ),
  };
  // A message that waits has no answer yet, and an answer a message took mid-way ends under that message.
  let end = if row.turn.message.is_some() && row.turn.arrival != Arrival::Waiting && !row.turn.continued {
    super::ending::ending(ctx, row)
  } else {
    Vec::new()
  };
  room(
    Column::new()
      .id(format!("orchestrator-turn-{key}"))
      .describe("turn", turn_name(row.turn.state.as_ref()))
      .describe("arrival", arrival_name(row.turn.arrival))
      .width(Dimension::Pct(100.0))
      .spacing(OrchestratorSpacing::Items.role())
      .child(opening)
      .with_children(answer)
      .with_children(end),
    first,
  )
}

/// A row's room in the scrolling turns: the chat's column, centred, 18 under it, and the chat's top room over the
/// first.
fn room(content: Column, first: bool) -> Element {
  let padding = Padding::new().bottom(OrchestratorSpacing::Items.role());
  let padding = if first {
    padding.top(OrchestratorSpacing::Start.role())
  } else {
    padding
  };
  super::body::turn_room(content.padding(padding)).into()
}

/// The name `describe` reports a turn by, for tests and QA; `none` for a wait that started no turn.
const fn turn_name(state: Option<&TurnState>) -> &'static str {
  match state {
    None => "none",
    Some(TurnState::Thinking) => "thinking",
    Some(TurnState::Answered) => "answered",
    Some(TurnState::Stopped) => "stopped",
    Some(TurnState::Failed(_)) => "failed",
    Some(TurnState::Queued(Queued::AfterAnswer)) => "queued",
    Some(TurnState::Queued(Queued::Next)) => "queued-next",
    Some(TurnState::Queued(Queued::NotTaken)) => "not-taken",
    Some(TurnState::Steering) => "steering",
    Some(TurnState::Joined) => "joined",
  }
}

/// The name `describe` reports how a row's message reached the answer by, for tests and QA.
const fn arrival_name(arrival: Arrival) -> &'static str {
  match arrival {
    Arrival::Own => "own",
    Arrival::Joined => "joined",
    Arrival::Waiting => "waiting",
  }
}
