//! The conversation (`Chat` `ECQg8` in `01 Conversation` `wRSL4`): one row per turn, newest last, 18 apart, in a list
//! that keeps the rows out of view unbuilt and opens at the very end of the newest turn, following it
//! (`ui::chat::follow`). A turn is the person's message (`Your message`, right-aligned on `own_message`) with where it
//! stands (*Sending…*, then *Delivered* and when), the Orchestrator's answer ([`super::answer`]), and how the turn
//! ended ([`super::ending`]). A wait of an agent of the project opens a row of its own with its line
//! ([`super::wait`]), what the turn it started by itself said and did nested under it as a quote; the end of a run its
//! line ([`super::run_end`]), an event of a task's run its line ([`super::task_event`]) and a daily report its card
//! ([`super::report`]), with the turn they started, if any. The person's watch commands and the lines the app writes
//! about what the conversation watches are rows of their own, with no turn ([`super::watch`]).
//! Waits for the person kept one after another, with no message of theirs between them, gather into one digest
//! ([`super::digest`]).

use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Column, Stack, Text, VirtualizedList},
  core::Signal,
  layout::StackAlignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ChatMessageId, ProjectId},
  orchestrator::{Queued, TurnState, WaitKind},
};
use orchester_workspace::{orchestrator::tools::ChatScope, workspace::OpenProject};

use super::{
  digest::{Digest, DigestProps, Waited, for_the_person},
  plan_only::Changing,
  report::{ReportCard, ReportProps},
  rewind::toolbar::RewindEnv,
  send::QueuedChange,
  turns::{Arrival, Turn},
  wait::Status,
  watch::WatchFacts,
  watching::Live,
};
use crate::{
  screens::pipeline_link::PipelineLinks,
  theme::{DROPPED_OPACITY, OrchestratorSpacing, chat_scrollbar},
  ui::{
    chat::{follow::Follow, names::ProjectNames},
    markdown::LinkHandler,
  },
};

/// What every turn row needs besides its turn: the project, the connection the Orchestrator runs on, and what the
/// person can do about the last turn and the messages that wait for it.
#[derive(Clone)]
pub(super) struct TurnEnv {
  pub(super) project: ProjectId,
  pub(super) name: Arc<str>,
  /// Where a link to one of the project's pipelines opens.
  pub(super) pipelines: PipelineLinks,
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
  pub(super) plans: Signal<OpenedPlans>,
  /// The project, which a plan's approval asks the runner of; what *Change it* does with a plan; and the plan the
  /// composer changes, by its entry (`super::plan_only`).
  pub(super) open: OpenProject,
  pub(super) change_plan: Arc<dyn Fn(Changing) + Send + Sync>,
  pub(super) changing: Option<i64>,
  /// A turn of the conversation still thinks: a plan is approved once it ended.
  pub(super) answering: bool,
  /// Edit and *Rewind to here* (`super::rewind`).
  pub(super) rewind: RewindEnv,
  /// What the conversation leads now, which a `/watching` row shows, and its *Stop watching* (`super::watching`).
  pub(super) watching: Live,
  /// What the lines about watching name beside what they keep (`super::watch`).
  pub(super) watch: Arc<WatchFacts>,
}

/// The answers whose todo list the person opened under their name line, by the message that started each.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct OpenedPlans(BTreeSet<ChatMessageId>);

impl lurq::app::component::DevtoolsInspectable for OpenedPlans {}

impl OpenedPlans {
  pub(super) fn is_open(&self, message: ChatMessageId) -> bool {
    self.0.contains(&message)
  }

  /// Opens the list of `message`'s answer, or folds it again.
  pub(super) fn flip(&mut self, message: ChatMessageId) {
    if !self.0.remove(&message) {
      self.0.insert(message);
    }
  }
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
      && self.env.changing == other.env.changing
      && self.env.rewind.shows() == other.env.rewind.shows()
      && (!self.turn.asks_watching() || self.env.watching == other.env.watching)
      && self.env.watch == other.env.watch
  }
}

impl lurq::app::component::DevtoolsInspectable for TurnRow {}

/// A row of the list: a turn, a digest of waits for the person, or a note under the last turn. No room is added under
/// the last row while *Jump to latest* floats over the chat's foot: the list's end stays where it is whether the pill
/// shows or not, and the pill hides once that end is in view.
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
}

impl lurq::app::component::DevtoolsInspectable for Line {}

impl Line {
  fn key(&self) -> String {
    match self {
      Self::Turn { row, .. } => row.turn.key(),
      Self::Digest { digest, .. } => digest.id.clone(),
      Self::Note(_) => "orchestrator-note".to_owned(),
    }
  }
}

/// The list of `rows`, as `follow` follows it (`ui::chat::follow`), with `note` under the last turn: it opens at the
/// very end of the newest turn and keeps it in view as turns come and replies grow, until the person scrolls up; then
/// *Jump to latest* floats over its foot. Opened from History's search at a match, it opens at the row `found` (its
/// key) instead, until the person scrolls.
pub(super) fn list(
  ctx: &mut Ctx,
  follow: &Follow,
  (rows, found): (Vec<TurnRow>, Option<String>),
  note: Option<Arc<str>>,
) -> Element {
  let arrived = arrivals(&rows);
  let reveal = follow.follow(rows.len(), arrived, found);
  let following = follow.following.get();
  let mut lines = gathered(rows);
  lines.extend(note.map(Line::Note));
  let scrollbar = chat_scrollbar(&ctx.theme().scrollbar());
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
        room(ctx, Column::new().width(Dimension::Pct(100.0)).child(mounted), first)
      }
      Line::Note(words) => room(
        ctx,
        Column::new().width(Dimension::Pct(100.0)).child(
          Text::new(&words)
            .id("orchestrator-not-ready")
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextMuted)
            .width(Dimension::Pct(100.0)),
        ),
        false,
      ),
    }
  }
}

/// A turn's row: what opens it, the answer and how it ended; or a compaction's, a rewind's or a watch entry's row, with
/// under a /watch's line the cards of its targets other conversations lead; or a `/watching` and its answer from the
/// live state (`super::watching`). While a message is edited in place, what sending it drops is dimmed: its own answer
/// under the line that counts them, and every row after it (`super::rewind::editing`).
fn turn(ctx: &mut Ctx, row: &TurnRow, first: bool) -> Element {
  let key = row.turn.key();
  let dropped = row.env.rewind.drops(&row.turn);
  if row.turn.asks_watching() {
    let column = super::watching::row(ctx, row, &key);
    return room(ctx, dimmed(column, dropped), first);
  }
  if let Some(event) = &row.turn.compaction {
    let id = format!("orchestrator-compaction-{key}");
    let line = super::compaction::line(ctx, &id, event, row.turn.at)
      .or_else(|| {
        let at = row.turn.at?;
        super::rewind::line::line(
          ctx,
          &format!("orchestrator-rewound-{key}"),
          event,
          (at, row.env.project),
        )
      })
      .or_else(|| super::rewind::forked::line(ctx, &format!("orchestrator-forked-{key}"), event, row.env.project))
      .or_else(|| {
        super::watch::row(
          ctx,
          &format!("orchestrator-watch-{key}"),
          event,
          (row.turn.at, &row.env),
        )
      });
    // A /watch's line, then the cards of its targets other conversations lead (05 `EIjDH`).
    let cards = row
      .turn
      .led
      .as_ref()
      .map(|led| super::led::column(ctx, &key, led, &row.env));
    let column = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(OrchestratorSpacing::Items.role())
      .with_children(line)
      .with_children(cards);
    return room(ctx, dimmed(column, dropped), first);
  }
  let (opening, answer) = opening(ctx, row, &key);
  // A message that waits has no answer yet, and an answer a message took mid-way ends under that message.
  let end = if row.turn.message.is_some() && row.turn.arrival != Arrival::Waiting && !row.turn.continued {
    super::ending::ending(ctx, row)
  } else {
    Vec::new()
  };
  let edited = row
    .env
    .rewind
    .editing()
    .filter(|(editing, _)| Some(editing.message) == row.turn.message);
  let column = Column::new()
    .id(format!("orchestrator-turn-{key}"))
    .describe("turn", turn_name(row.turn.state.as_ref()))
    .describe("arrival", arrival_name(row.turn.arrival))
    .width(Dimension::Pct(100.0))
    .spacing(OrchestratorSpacing::Items.role())
    .child(opening);
  let column = match edited {
    Some((_, after)) if answer.is_some() || !end.is_empty() => column
      .with_children((after > 0).then(|| super::rewind::editing::drop_line(ctx, after)))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(OrchestratorSpacing::Items.role())
          .opacity(DROPPED_OPACITY)
          .with_children(answer)
          .with_children(end),
      ),
    _ => column.with_children(answer).with_children(end),
  };
  room(ctx, dimmed(column, dropped), first)
}

/// What opens `row`, keyed `key`, and its answer. A wait of an agent, a run's end, a daily report and a subscription
/// that ended by itself open their row with their own line or card; a person's turn with their message. What a wait's
/// turn said nests under its line.
fn opening(ctx: &mut Ctx, row: &TurnRow, key: &str) -> (Element, Option<Element>) {
  if let Some(summary) = &row.turn.watch_ended {
    let id = format!("orchestrator-watch-ended-{key}");
    let line = super::watch::ended_line(ctx, &id, summary, row.turn.at);
    return (line, super::answer::answer(ctx, row));
  }
  match (&row.turn.wait, &row.turn.report) {
    (Some(wait), _) => {
      let id = format!("orchestrator-wait-{key}");
      let turned = row.turn.state.is_some();
      match wait.kind {
        WaitKind::Ended(end) => {
          let ended = (&**wait, end, row.turn.at, turned);
          let line = super::run_end::line(ctx, &id, ended, (row.turn.moved, row.env.project));
          (line, super::answer::answer(ctx, row))
        }
        WaitKind::Event(event) => {
          let facts = (row.turn.event.as_ref(), row.env.project);
          let line = super::task_event::line(ctx, &id, (wait, event, row.turn.at), facts);
          (line, super::answer::answer(ctx, row))
        }
        _ => {
          let standing = (row.turn.stands, row.turn.request.as_ref());
          let status = Status::of(wait, standing, row.turn.state.as_ref(), &row.turn.answer);
          let line = super::wait::line(ctx, &id, (wait, row.turn.at, turned), (status, row.env.project));
          if wait.kind == WaitKind::Approval {
            return super::wait::approval::opening(ctx, row, (&id, line), status);
          }
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
      super::delivery::opening_message(ctx, row, key),
      super::answer::answer(ctx, row),
    ),
  }
}

/// `column`, dimmed while sending the edit of an earlier message drops it.
fn dimmed(column: Column, dropped: bool) -> Column {
  if dropped {
    column.opacity(DROPPED_OPACITY).describe("dropped", "true")
  } else {
    column
  }
}

/// A row's room in the scrolling turns: the chat's column, centred, 18 under it, and the chat's top room over the
/// first.
fn room(ctx: &Ctx, content: Column, first: bool) -> Element {
  let padding = Padding::new().bottom(OrchestratorSpacing::Items.role());
  let padding = if first {
    padding.top(OrchestratorSpacing::Start.role())
  } else {
    padding
  };
  super::body::turn_room(ctx, content.padding(padding)).into()
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
