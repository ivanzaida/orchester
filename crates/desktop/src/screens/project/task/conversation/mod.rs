//! The Conversation tab as a chat with a step rail on the right (`docs/design/orchester-workspace.md`,
//! "Decided: C with the rail on the right", Pencil group `A5H6ln`): one timeline per run of what its
//! agents did and what was said, as chat rows ([`row`]) with *Show* and search ([`toolbar`]), what a
//! filter or search leaves out ([`readback`]), the working line as the chat's last row, following it
//! live and *Jump to latest* while it does not ([`follow`]), the composer ([`composer`]); at the right
//! the step rail, which filters the chat by step ([`rail`], collapsed to [`strip`]), or Details in its
//! place ([`details`], [`side`]). The transcript is read again whenever the runner reports a change;
//! the run's tool requests apart from it, as the project's waiting requests change ([`requests`]).

mod approval_card;
mod cards;
mod chat;
mod chips;
mod composer;
mod details;
mod divider;
mod follow;
mod live_state;
mod message;
mod rail;
mod readback;
mod requests;
mod row;
mod segments;
mod send;
mod side;
mod strip;
mod toolbar;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, ScrollVertical},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{RunId, StepExecutionId, TaskId},
  integration::Leftovers,
  resource::StepKey,
};
use parking_lot::Mutex;

use self::{
  details::{DetailsOn, DetailsPane, remembered, save_action},
  divider::step_label,
  follow::{LeftAt, Reveal},
  rail::RailToggle,
};
use super::{
  activity::{OpenRows, RevealRow, Toggles},
  notice::Lost,
  tab_state,
};
use crate::{
  background, messages,
  messages::Message,
  runner::Runners,
  screens::project::ProjectNav,
  tasks::{
    conversation::Conversation,
    detail::{RunView, TaskDetail},
    timeline::{
      self, Sources,
      load::{Loaded, transcript_in},
      view::Show,
    },
    transcript::Entry,
  },
  theme::gutter_scrollbar,
  ui::{button::ActionButton, icon::Icon},
};

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

/// The step the chat is narrowed to, picked in the rail.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct StepFilter(pub(super) Option<StepKey>);

impl lurq::app::component::DevtoolsInspectable for StepFilter {}

/// The *Show* control's choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ShowChoice(pub(super) Show);

impl lurq::app::component::DevtoolsInspectable for ShowChoice {}

/// What rows of the chat share: the run, the composer they write into and the pane they open.
#[derive(Clone)]
struct Timeline {
  repo_root: PathBuf,
  task: TaskId,
  run: RunId,
  /// The run's number in the project, for its end.
  run_number: u64,
  task_title: Arc<str>,
  /// The task, for the names and numbers of the run's steps.
  detail: Arc<TaskDetail>,
  toggles: Toggles,
  /// The composer's draft: *Edit in composer* puts a message back into it.
  draft: Signal<String>,
  /// Puts the focus in the composer.
  focus: Signal<bool>,
  details: DetailsPane,
  /// Bumped to read the task again, once something was sent.
  reload: Signal<u64>,
  /// Scrolls the chat to a row: a problem row's link to the request it names.
  reveal: RevealRow,
}

impl Timeline {
  /// The name of the step `execution` executed.
  fn step_name(&self, execution: StepExecutionId) -> Option<Arc<str>> {
    let run = self.detail.run.as_ref()?;
    run.step_of(execution).map(|step| step.name.clone())
  }

  /// "03 Implement" for the step named `name`, or the name alone once the pipeline has no such step.
  fn labelled(&self, ctx: &Ctx, name: &Arc<str>) -> Arc<str> {
    self
      .detail
      .run
      .as_ref()
      .and_then(|run| run.steps.iter().find(|step| step.name == *name))
      .map_or_else(|| name.clone(), |step| step_label(ctx, step.number, &step.name))
  }
}

/// The run's transcript, as read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read(Arc<Vec<Entry>>);

impl lurq::app::component::DevtoolsInspectable for Read {}

pub(super) struct ConversationTab {
  show: Signal<ShowChoice>,
  step: Signal<StepFilter>,
  query: Signal<String>,
  /// The search field holds the focus: the composer does not take it then.
  searching: Signal<bool>,
  raw: Signal<bool>,
  following: Signal<bool>,
  /// Rows when the list stopped following, for "N new".
  left_at: Arc<Mutex<LeftAt>>,
  /// Whether the list has shown rows yet: the first time, it starts at the end.
  started: Arc<Mutex<bool>>,
  /// Whether the list has laid out every one of its rows since it showed rows.
  measured: Signal<bool>,
  /// The card that waits for the person, which the chat shows until the person scrolls.
  reveal: Arc<Mutex<Reveal>>,
  /// Bumped when a row asks the chat to reveal another, so the chat renders and reveals it.
  revealed: Signal<u64>,
  toggles: Toggles,
  scroll: lurq::layout::layout_kind::ScrollState,
  retry: Signal<u64>,
  draft: Signal<String>,
  details: DetailsPane,
  rail: RailToggle,
}

impl Component for ConversationTab {
  type Props = ConversationProps;

  fn create(ctx: &mut Ctx) -> Self {
    // First, as in every render: futures keep their place by the order they are asked for.
    let save = save_action(ctx);
    let details = DetailsPane {
      open: ctx.signal(None),
      on: ctx.signal(DetailsOn::default()),
      save,
    };
    Self {
      show: ctx.signal(ShowChoice::default()),
      step: ctx.signal(StepFilter::default()),
      query: ctx.signal(String::new()),
      searching: ctx.signal(false),
      raw: ctx.signal(false),
      following: ctx.signal(true),
      left_at: Arc::new(Mutex::new(LeftAt::default())),
      started: Arc::new(Mutex::new(false)),
      measured: ctx.signal(false),
      reveal: Arc::new(Mutex::new(Reveal::default())),
      revealed: ctx.signal(0),
      toggles: Toggles(ctx.signal(OpenRows::default())),
      scroll: lurq::layout::layout_kind::ScrollState::new(),
      retry: ctx.signal(0),
      draft: ctx.signal(String::new()),
      details,
      rail: RailToggle {
        collapsed: ctx.signal(None),
        over: ctx.signal(false),
      },
    }
  }

  fn after_layout(&self) {
    self.reveal_asked();
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ConversationProps>().clone();
    save_action(ctx);
    let details_open = self.details.is_open(remembered(ctx));
    let run_id = props.detail.run.as_ref().map(RunView::id);
    let retry = self.retry.get();
    let deps = (props.repo_root.clone(), run_id, props.revision, retry);
    let load = ctx.future(deps, |(repo_root, run, ..)| async move {
      let Some(run) = run else {
        return Ok(Read(Arc::default()));
      };
      background::run(move || transcript_in(&repo_root, run))
        .await?
        .map(Read)
        .map_err(|error| {
          tracing::error!(%error, "cannot read the run's transcript");
          messages::store_error(&error)
        })
    });
    let state = load.state().get();
    let read = state.data.as_ref().map_or(0, |Read(entries)| entries.len());
    let requests = requests::requests(ctx, (props.repo_root.clone(), run_id), (read, retry));
    let Some(run) = props.detail.run.as_ref() else {
      let waiting = without_run(ctx, &props.conversation);
      return self.layout(ctx, &props, None, waiting);
    };
    ctx.provide(self.timeline(&props, run));
    let loaded = match (state.data.zip(requests.data), state.error.or(requests.error)) {
      (Some((Read(entries), requests)), _) => Loaded::of(entries, requests),
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
      detail: props.detail.clone(),
      toggles: self.toggles.clone(),
      draft: self.draft.clone(),
      focus: props.focus.clone(),
      details: self.details.clone(),
      reload: props.reload.clone(),
      reveal: self.reveal_row(),
    }
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

/// `child` across the chat, with its side padding (`pCrjU`).
pub(super) fn column(child: Element) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Section)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
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
