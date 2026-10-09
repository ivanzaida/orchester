//! A task's Conversation tab (`Task · Conversation · working` `D1PYAe`, states `B67XdS`): its latest run as a
//! chat, in the order things happened ([`timeline`]): agents' messages with their tool groups, the person's
//! messages, step dividers, returns and restarts, folded finished steps, result cards, problem rows, the
//! cards that wait for a decision (C3's, `timeline::cards`) and while an agent works the working line;
//! following it live, or *Jump to latest* once the person scrolled up. Over it the toolbar's run state
//! chip ([`toolbar`]); before the run can be shown, why not ([`states`]): loading, couldn't load, queued,
//! no run yet, unreadable. Under it the composer ([`composer`]).
//!
//! The task page (P1) reads the task and hands it here ([`tab`]); the transcript and the run's requests are
//! read here ([`read`]), again as the runner reports changes of the project's runs (the feed counts them per
//! project, not per run). What the person opened and how far they scrolled are the run's: another run of
//! the task starts the tab afresh. The step rail, *Show*, the search and Details are not part of this
//! build (D4): the chat shows every step.

mod composer;
mod read;
mod states;
mod timeline;
mod toolbar;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::InvocationId, run::RunStatus};
use orchester_workspace::{
  messages::Message,
  tasks::{
    conversation::{Conversation, PanelState},
    detail::RunView,
    timeline::{
      self as items, Sources,
      load::Loaded,
      view::{self, Choice},
    },
  },
};

use self::{
  read::{Transcript, TranscriptRead, Waiting, with_requests},
  timeline::{Follow, Timeline, Toggles},
  toolbar::RunState,
};
use super::TaskContext;
use crate::{project::TasksState, services::Services, ui::markdown::plain};

/// The tab's content for `task`, as the task page read it. Keyed by the task and its latest run, so a new
/// run starts with nothing opened and following its end.
pub fn tab(ctx: &mut Ctx, task: &TaskContext) -> Element {
  let run = task
    .detail
    .run
    .as_ref()
    .map_or_else(|| "none".to_owned(), |run| run.id().to_string());
  ctx.mount_keyed::<ConversationTab>(&format!("task-conversation-{}-{run}", task.key), task.clone())
}

struct ConversationTab {
  composer: composer::ComposerLink,
  follow: Follow,
  toggles: Toggles,
  retry: Signal<u64>,
  transcript: TranscriptRead,
}

impl Component for ConversationTab {
  type Props = TaskContext;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      composer: composer::ComposerLink::new(ctx),
      follow: Follow::new(ctx),
      toggles: Toggles::new(ctx),
      retry: ctx.signal(0),
      transcript: TranscriptRead::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let task = ctx.props::<TaskContext>().clone();
    let detail = &task.detail;
    // What the latest run left once it ended, as last read: a removed worktree closes the conversation.
    let leftovers = detail
      .run
      .as_ref()
      .and_then(|run| task.left.as_read(run.id()))
      .and_then(Result::ok);
    let conversation = Conversation::of(detail, task.lost(), leftovers.as_ref());
    let composer = composer::dock(ctx, (&self.composer, &self.follow), &task, &conversation);
    let (body, state) = match &detail.run {
      Some(run) => self.run_body(ctx, &task, run, &conversation),
      None => (states::without_run(ctx, detail, &conversation), None),
    };
    let state = state.unwrap_or_else(|| RunState::of(&conversation, detail, false));
    Column::new()
      .id("conversation")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .child(toolbar::toolbar(ctx, state))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .child(body),
      )
      .child(composer)
  }
}

impl ConversationTab {
  /// The chat of `run`, or why it cannot be shown yet; and the toolbar's state while it follows the run.
  fn run_body(
    &self,
    ctx: &mut Ctx,
    task: &TaskContext,
    run: &RunView,
    conversation: &Conversation,
  ) -> (Element, Option<RunState>) {
    let live = conversation.state == PanelState::Live;
    let loaded = match self.read(ctx, task, run) {
      Ok(loaded) => loaded,
      Err(Some(error)) => {
        let root = &task.project.open.repo_root;
        return (states::failed(ctx, (run, live), &error, (root, &self.retry)), None);
      }
      Err(None) => return (states::loading(ctx, run), None),
    };
    let items = items::build(&Sources {
      run,
      entries: &loaded.entries,
      conversation,
      pending: &loaded.pending,
      decided: &loaded.decided,
      questions: &loaded.questions,
      merge: task.detail.merge.as_ref(),
    });
    let open = |key: &str, default: bool| self.toggles.is_open(key, default);
    let shown = timeline::without_usage(view::shown(&items, &Choice::default(), &open, &plain::rendered));
    // An agent that waits in its turn for the answer to its question does not work: the run waits for the
    // person (C·15), so there is no working line and the chip says so.
    let asking = live && !run.waiting_questions.is_empty();
    let following = self.follow.following.get();
    let state = RunState::of(conversation, &task.detail, asking).following(following);
    if shown.rows.is_empty() {
      return (states::nothing_yet(ctx, run), Some(state));
    }
    ctx.provide(Timeline {
      project: task.project.open.project.id(),
      key: task.key.clone(),
      detail: task.detail.clone(),
      toggles: self.toggles.clone(),
      working: working_invocations(run, live),
    });
    let working = (live && !asking && run.run.status() == &RunStatus::Running).then(|| timeline::Working {
      agent: run.current_step().and_then(|step| step.agent.clone()),
      last: loaded.entries.last().map(|entry| entry.at),
    });
    let arrived = loaded.entries.len() + conversation.entries.len();
    let chat = timeline::chat(ctx, &self.follow, (&shown, &items), (working, arrived));
    (chat, Some(state))
  }

  /// The run's transcript with its requests; while they are not read, why not: being read (`None`), or the
  /// reading failed.
  fn read(&self, ctx: &mut Ctx, task: &TaskContext, run: &RunView) -> Result<Loaded, Option<Message>> {
    let feed = &task.project.feed;
    let repo_root: PathBuf = task.project.open.repo_root.clone();
    let hold = Services::of(ctx).hold;
    let retry = self.retry.get();
    // The run's events and the runner's transcript changes both move the transcript on.
    let revision = feed.changes.get() + run.run.last_sequence().get();
    let read = self
      .transcript
      .read(ctx, (repo_root.clone(), run.id(), hold.clone()), (revision, retry));
    let waiting = match feed.tasks.get() {
      TasksState::Ready(tasks) => Some(Waiting::of(&tasks, run.id())),
      TasksState::Loading | TasksState::Failed(_) => None,
    };
    let entries = read.data.as_ref().map(|Transcript(entries)| entries.clone());
    let sources = with_requests(ctx, (repo_root, run.id(), hold), (entries, waiting, retry));
    match (sources.data.flatten(), read.error.or(sources.error)) {
      (Some(loaded), _) => Ok(loaded),
      (None, error) => Err(error),
    }
  }
}

/// The invocations of `run` whose agent works now; none while no agent works (`live` false).
fn working_invocations(run: &RunView, live: bool) -> Arc<[InvocationId]> {
  if !live {
    return Arc::from([]);
  }
  run
    .invocations
    .iter()
    .filter(|invocation| invocation.ended_at.is_none())
    .map(|invocation| invocation.id)
    .collect()
}
