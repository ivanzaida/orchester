//! The Conversation tab (`docs/design/orchester-workspace.md`, "Conversation (merged with
//! Activity)", Pencil group `fTfl8`): one timeline per run of what its agents did and what was said,
//! with its step filter, *Show* and search ([`toolbar`]), what a filter or search leaves out
//! ([`readback`]), the timeline itself ([`row`]), *Jump to latest* while it does not follow, the
//! working line, the composer ([`composer`]) and the Details pane ([`details`]). The transcript and
//! the run's tool requests are read again whenever the runner reports a change.

mod approval_card;
mod cards;
mod composer;
mod composer_text;
mod details;
mod group;
mod live_state;
mod message;
mod readback;
mod row;
mod segments;
mod send;
mod toolbar;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{component::Component, ctx::Ctx, events::scroll_event::ScrollEvent, theme::SpacingSize},
  components::{Column, Row, ScrollVertical, VirtualizedList},
  core::Signal,
  layout::{
    Alignment,
    layout_kind::{Justify, ScrollState},
  },
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{RunId, StepExecutionId, TaskId},
  integration::Leftovers,
};
use parking_lot::Mutex;

use self::{
  composer::{ComposerDock, ComposerProps},
  details::{DetailsPane, details_pane, remembered, save_action},
  readback::readback,
  row::{RowProps, TimelineRowView},
  toolbar::toolbar,
};
use super::{
  activity::{LiveFooter, LiveFooterProps, OpenRows, Toggles},
  notice::Lost,
  tab_state,
};
use crate::{
  background, messages,
  messages::Message,
  runner::Runners,
  screens::project::ProjectNav,
  tasks::{
    conversation::{Conversation, PanelState},
    detail::{RunView, TaskDetail},
    timeline::{
      self, Item, Sources,
      load::{Loaded, load_in},
      view::{self, Choice, Show, Shown},
    },
  },
  theme::gutter_scrollbar,
  ui::{button::ActionButton, icon::Icon},
};

/// The timeline's column: at most this wide, centred in the tab (`RrScB`).
pub(super) const COLUMN_WIDTH: f32 = 840.0;
/// *Jump to latest* (`iwe3p`) and how close to the end still counts as at the end.
const JUMP_HEIGHT: f32 = 30.0;
const FOLLOW_SLACK: f32 = 24.0;

/// Puts the focus in the composer once it shows (Ask agent, Reply); provided by the task page.
#[derive(Clone)]
pub(super) struct ComposerFocus(pub(super) Signal<bool>);

/// What the tab needs besides the task.
pub(super) struct TabContext<'a> {
  pub(super) repo_root: &'a std::path::Path,
  pub(super) lost: Option<Lost>,
  /// What the latest run left once it ended, once read.
  pub(super) leftovers: Option<&'a Leftovers>,
  /// Set to have the composer take the focus (Ask agent, Reply).
  pub(super) focus: &'a Signal<bool>,
  /// Bumped once a message is stored, so the task is read again.
  pub(super) reload: &'a Signal<u64>,
  /// Changes whenever the runner reports a change: the transcript is read again.
  pub(super) revision: (u64, u64),
}

pub(super) fn conversation_tab(ctx: &mut Ctx, detail: &Arc<TaskDetail>, tab: &TabContext<'_>) -> Element {
  let conversation = Conversation::of(detail, tab.lost.is_some(), tab.leftovers);
  ctx.mount_keyed::<ConversationTab>(
    "task-conversation",
    ConversationProps {
      repo_root: tab.repo_root.to_path_buf(),
      detail: detail.clone(),
      conversation: Arc::new(conversation),
      revision: tab.revision,
      focus: tab.focus.clone(),
      reload: tab.reload.clone(),
    },
  )
}

#[derive(Clone)]
pub(super) struct ConversationProps {
  repo_root: PathBuf,
  detail: Arc<TaskDetail>,
  conversation: Arc<Conversation>,
  revision: (u64, u64),
  focus: Signal<bool>,
  reload: Signal<u64>,
}

impl PartialEq for ConversationProps {
  fn eq(&self, other: &Self) -> bool {
    self.repo_root == other.repo_root
      && self.detail == other.detail
      && self.conversation == other.conversation
      && self.revision == other.revision
  }
}

impl lurq::app::component::DevtoolsInspectable for ConversationProps {}

/// The step execution the timeline is narrowed to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct StepFilter(pub(super) Option<StepExecutionId>);

impl lurq::app::component::DevtoolsInspectable for StepFilter {}

/// The *Show* control's choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ShowChoice(pub(super) Show);

impl lurq::app::component::DevtoolsInspectable for ShowChoice {}

/// What rows of the timeline share: the run, the composer they write into and the pane they open.
#[derive(Clone)]
struct Timeline {
  repo_root: PathBuf,
  task: TaskId,
  run: RunId,
  /// The run's number in the project, for its end marker.
  run_number: u64,
  task_title: Arc<str>,
  toggles: Toggles,
  /// The composer's draft: *Edit in composer* puts a message back into it.
  draft: Signal<String>,
  /// Puts the focus in the composer.
  focus: Signal<bool>,
  details: DetailsPane,
  /// Bumped to read the task again, once something was sent.
  reload: Signal<u64>,
}

/// The card the timeline reveals, and whether the person has scrolled since the tab opened.
#[derive(Default)]
struct Reveal {
  key: Option<String>,
  scrolled: bool,
}

/// The run's transcript and requests, as read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read(Loaded);

impl lurq::app::component::DevtoolsInspectable for Read {}

pub(super) struct ConversationTab {
  show: Signal<ShowChoice>,
  step: Signal<StepFilter>,
  query: Signal<String>,
  raw: Signal<bool>,
  following: Signal<bool>,
  /// Rows when the list stopped following, for "N new".
  left_at: Signal<usize>,
  /// Rows of the last render, to notice new ones; `None` before the first.
  shown: Arc<Mutex<Option<usize>>>,
  /// The card that waits for the person, which the timeline shows until the person scrolls.
  reveal: Arc<Mutex<Reveal>>,
  toggles: Toggles,
  scroll: ScrollState,
  retry: Signal<u64>,
  draft: Signal<String>,
  details: DetailsPane,
}

impl Component for ConversationTab {
  type Props = ConversationProps;

  fn create(ctx: &mut Ctx) -> Self {
    // First, as in every render: futures keep their place by the order they are asked for.
    let save = save_action(ctx);
    let details = DetailsPane {
      open: ctx.signal(None),
      on: ctx.signal(StepFilter(None)),
      save,
    };
    Self {
      show: ctx.signal(ShowChoice::default()),
      step: ctx.signal(StepFilter::default()),
      query: ctx.signal(String::new()),
      raw: ctx.signal(false),
      following: ctx.signal(true),
      left_at: ctx.signal(0),
      shown: Arc::new(Mutex::new(None)),
      reveal: Arc::new(Mutex::new(Reveal::default())),
      toggles: Toggles(ctx.signal(OpenRows::default())),
      scroll: ScrollState::new(),
      retry: ctx.signal(0),
      draft: ctx.signal(String::new()),
      details,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ConversationProps>().clone();
    save_action(ctx);
    let details_open = self.details.is_open(remembered(ctx));
    let deps = (
      props.repo_root.clone(),
      props.detail.run.as_ref().map(RunView::id),
      props.revision,
      self.retry.get(),
    );
    let load = ctx.future(deps, |(repo_root, run, ..)| async move {
      let Some(run) = run else {
        return Ok(Read(Loaded::default()));
      };
      background::run(move || load_in(&repo_root, run))
        .await?
        .map(Read)
        .map_err(|error| {
          tracing::error!(%error, "cannot read the run's transcript");
          messages::store_error(&error)
        })
    });
    let Some(run) = props.detail.run.as_ref() else {
      let waiting = without_run(ctx, &props.conversation);
      return self.layout(ctx, &props, None, waiting);
    };
    ctx.provide(self.timeline(&props, run));
    let state = load.state().get();
    let loaded = match (state.data, state.error) {
      (Some(Read(loaded)), _) => loaded,
      (None, Some(error)) => {
        let failed = self.failed(ctx, &error);
        return self.layout(ctx, &props, None, failed);
      }
      (None, None) => {
        let number = run.run.number().get().to_string();
        let loading = tab_state::state(
          "activity-loading",
          Icon::Loader,
          &ctx.t_args("runs.activity.loadingTitle", [("run", number.as_str())]),
          &ctx.t("runs.activity.loadingDetail"),
          Vec::new(),
        );
        return self.layout(ctx, &props, None, loading);
      }
    };
    let items = timeline::build(&Sources {
      run,
      entries: &loaded.entries,
      conversation: &props.conversation,
      pending: &loaded.pending,
      decided: &loaded.decided,
      merge: props.detail.merge.as_ref(),
    });
    self.with_items(ctx, &props, run, (&items, &loaded), details_open)
  }
}

impl ConversationTab {
  fn timeline(&self, props: &ConversationProps, run: &RunView) -> Timeline {
    let task = props.detail.run_task.as_ref().unwrap_or(&props.detail.task);
    Timeline {
      repo_root: props.repo_root.clone(),
      task: props.detail.task.id(),
      run: run.id(),
      run_number: u64::from(run.run.number().get()),
      task_title: task.details().title.as_str().into(),
      toggles: self.toggles.clone(),
      draft: self.draft.clone(),
      focus: props.focus.clone(),
      details: self.details.clone(),
      reload: props.reload.clone(),
    }
  }

  fn choice(&self) -> Choice {
    Choice {
      show: self.show.get().0,
      step: self.step.get().0,
      query: self.query.get(),
    }
  }

  fn with_items(
    &self,
    ctx: &mut Ctx,
    props: &ConversationProps,
    run: &RunView,
    (items, loaded): (&[Item], &Loaded),
    details_open: bool,
  ) -> Element {
    let choice = self.choice();
    let toggles = self.toggles.clone();
    let shown = view::shown(items, &choice, &|key, default| toggles.is_open(key, default));
    let live = props.conversation.state == PanelState::Live;
    let reveal = self.follow(shown.rows.len(), items);
    let body: Element = if shown.rows.is_empty() {
      self.empty(ctx, run, (items, &choice))
    } else {
      self.list(ctx, &shown, reveal)
    };
    let readback = readback(ctx, run, (&choice, &shown), (&self.step, &self.query, &self.show));
    let jump = (!self.following.get() && !shown.rows.is_empty()).then(|| self.jump(ctx, shown.rows.len()));
    let working = live.then(|| {
      let last = loaded.entries.last().map(|entry| entry.at);
      let agent = run
        .current_step()
        .and_then(|step| step.agent.clone())
        .unwrap_or_else(|| "".into());
      column(ctx.mount_keyed::<LiveFooter>("conversation-working", LiveFooterProps { agent, last }))
    });
    let main = Column::new()
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .child(toolbar(
        ctx,
        run,
        &self.toolbar_state(&props.conversation, details_open),
      ))
      .with_children(readback.map(column))
      .child(body)
      .with_children(jump)
      .with_children(working);
    let pane = details_pane(
      ctx,
      (run, &props.repo_root),
      &loaded.entries,
      (&self.details, details_open, self.step.get().0),
    );
    self.layout(ctx, props, pane, main.into())
  }

  fn toolbar_state(&self, conversation: &Conversation, details_open: bool) -> toolbar::ToolbarState {
    toolbar::ToolbarState {
      step: self.step.clone(),
      show: self.show.clone(),
      query: self.query.clone(),
      raw: self.raw.clone(),
      details: self.details.clone(),
      details_open,
      state: conversation.state,
      asks: conversation.open_question().is_some(),
      following: self.following.get(),
    }
  }

  /// The tab: the main column above the composer, and the Details pane when it is open.
  fn layout(&self, ctx: &mut Ctx, props: &ConversationProps, pane: Option<Element>, main: Element) -> Element {
    let composer = ctx.mount_keyed::<ComposerDock>(
      "conversation-composer-dock",
      ComposerProps {
        repo_root: props.repo_root.clone(),
        detail: props.detail.clone(),
        conversation: props.conversation.clone(),
        draft: self.draft.clone(),
        focus: props.focus.clone(),
        reload: props.reload.clone(),
        sent: self.sent_handler(),
        details_open: pane.is_some(),
      },
    );
    Row::new()
      .id("conversation")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(
        Column::new()
          .id("conversation-main")
          .flex(1.0)
          .height(Dimension::Pct(100.0))
          .child(Column::new().width(Dimension::Pct(100.0)).flex(1.0).child(main))
          .child(composer),
      )
      .with_children(pane)
      .into()
  }

  /// Once a message was sent, the timeline goes to its end and follows again.
  fn sent_handler(&self) -> Arc<dyn Fn() + Send + Sync> {
    let (scroll, following) = (self.scroll.clone(), self.following.clone());
    Arc::new(move || {
      scroll.scroll_to_bottom_pending();
      following.set(true);
    })
  }

  /// Keeps the newest row in view while following. The first time, the timeline starts at the end;
  /// until the person scrolls, the first card that waits for them is the list's reveal, from the
  /// render it appears in (the run's state may read before its question) until it no longer waits.
  /// The key stays the same on every render: a reveal that changes to `None` is dropped before the
  /// list has measured its way to the card.
  fn follow(&self, rows: usize, items: &[Item]) -> Option<String> {
    let mut shown = self.shown.lock();
    let mut reveal = self.reveal.lock();
    if !reveal.scrolled {
      reveal.key = items
        .iter()
        .find(|item| timeline::waits_for_you(item))
        .map(|item| item.key.to_string());
    }
    if (reveal.key.is_none() || shown.is_none()) && self.following.get_untracked() && rows > shown.unwrap_or(0) {
      self.scroll.scroll_to_bottom_pending();
    }
    *shown = Some(rows);
    reveal.key.clone()
  }

  fn list(&self, ctx: &mut Ctx, shown: &Shown, reveal: Option<String>) -> Element {
    let raw = self.raw.get();
    let rows: Arc<Vec<RowProps>> = Arc::new(
      shown
        .rows
        .iter()
        .map(|row| RowProps { row: row.clone(), raw })
        .collect(),
    );
    let count = rows.len();
    let (scroll, following, left_at) = (self.scroll.clone(), self.following.clone(), self.left_at.clone());
    let revealed = self.reveal.clone();
    VirtualizedList::new_shared(ctx, rows)
      .list_key("conversation-timeline")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .with_scroll_state(self.scroll.clone())
      .scrollbar(gutter_scrollbar())
      .reveal_key(reveal)
      .on_scroll(move |event: ScrollEvent| {
        // The person scrolls: the card the tab opened at no longer holds the list.
        *revealed.lock() = Reveal {
          key: None,
          scrolled: true,
        };
        // The handler runs before the wheel moves the list, which moves by the opposite of the delta.
        let end = (scroll.content_height() - scroll.viewport_height()).max(0.0);
        let at_end = scroll.scroll_y() - event.delta_y >= end - FOLLOW_SLACK;
        if at_end != following.get_untracked() {
          if !at_end {
            left_at.set(count);
          }
          following.set(at_end);
        }
      })
      .mount_keyed::<TimelineRowView, _, _, _>(|row| row.row.key().to_string(), Clone::clone)
  }

  /// "Jump to latest · N new" (`iwe3p`), centred above the composer.
  fn jump(&self, ctx: &Ctx, rows: usize) -> Element {
    let new = rows.saturating_sub(self.left_at.get());
    let label = if new == 0 {
      ctx.t("runs.activity.jump")
    } else {
      ctx.t_args("runs.activity.jumpNew", [("count", new.to_string().as_str())])
    };
    let (scroll, following) = (self.scroll.clone(), self.following.clone());
    let button = ActionButton::new(label)
      .icon(Icon::ArrowDown)
      .id("conversation-jump")
      .height(Dimension::Px(JUMP_HEIGHT))
      .on_click(move || {
        scroll.scroll_to_bottom_pending();
        following.set(true);
      })
      .render(ctx);
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding_bottom(SpacingSize::Md)
      .justify(Justify::Center)
      .child(button)
      .into()
  }

  /// Nothing to show: the run has not started, or a search or filter found nothing (`zxIMy`,
  /// `gLVQQ`).
  fn empty(&self, ctx: &mut Ctx, run: &RunView, (items, choice): (&[Item], &Choice)) -> Element {
    if choice.query.trim().is_empty() {
      return tab_state::not_started(ctx, Some(run));
    }
    let elsewhere = choice.step.map(|_| {
      view::shown(
        items,
        &Choice {
          step: None,
          ..choice.clone()
        },
        &|_, default| default,
      )
    });
    readback::nothing_found(ctx, run, (choice, elsewhere.as_ref()), (&self.step, &self.query))
  }

  /// Couldn't load the conversation: the run is unaffected; the runner log and a retry are offered.
  fn failed(&self, ctx: &mut Ctx, error: &Message) -> Element {
    tracing::debug!(?error, "the transcript could not be read");
    let runners = ctx
      .use_context::<Runners>()
      .expect("the root provides Runners before mounting screens");
    let retry = self.retry.clone();
    let actions = vec![
      ActionButton::new(ctx.t("runner.connection.openLog"))
        .icon(Icon::FileText)
        .id("activity-open-log")
        .on_click(move || runners.open_log())
        .render(ctx),
      ActionButton::new(ctx.t("runs.activity.tryAgain"))
        .icon(Icon::RefreshCw)
        .id("activity-retry")
        .on_click(move || retry.update(|attempt| *attempt += 1))
        .render(ctx),
    ];
    tab_state::state(
      "activity-failed",
      Icon::CloudOff,
      &ctx.t("runs.activity.failedTitle"),
      &ctx.t("runs.activity.failedDetail"),
      actions,
    )
  }
}

/// A task without a run: the messages that wait for its run to start, or why there is nothing yet.
fn without_run(ctx: &mut Ctx, conversation: &Conversation) -> Element {
  if conversation.entries.is_empty() {
    return tab_state::not_started(ctx, None);
  }
  let messages = conversation.entries.iter().map(|entry| {
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Sm)
      .child(message::person_message(ctx, entry, Vec::new()))
      .into()
  });
  let messages: Vec<Element> = messages.collect();
  ScrollVertical::new(column(
    Column::new()
      .id("conversation-waiting")
      .width(Dimension::Pct(100.0))
      .padding_top(SpacingSize::Md)
      .with_children(messages)
      .into(),
  ))
  .width(Dimension::Pct(100.0))
  .flex(1.0)
  .scrollbar(gutter_scrollbar())
  .into()
}

/// `child` in the timeline's column: at most [`COLUMN_WIDTH`] wide, centred, with the tab's side
/// padding.
pub(super) fn column(child: Element) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Section)
    .justify(Justify::Center)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .max_width(Dimension::Px(COLUMN_WIDTH))
        .align_items(Alignment::Stretch)
        .child(child),
    )
    .into()
}

/// The project navigation, which rows open the Changes tab and the Summary with.
pub(super) fn nav(ctx: &mut Ctx) -> ProjectNav {
  ctx
    .use_context::<ProjectNav>()
    .expect("the project shell provides ProjectNav")
}
