//! A task's Conversation tab (`Task · Conversation · working` `D1PYAe`, states `B67XdS`): its latest run as a
//! chat, in the order things happened ([`timeline`]): agents' messages with their tool groups, the person's
//! messages, step dividers, returns and restarts, folded finished steps, result cards, problem rows, the
//! cards that wait for a decision (C3's, `timeline::cards`) and while an agent works the working line;
//! following it live, or *Jump to latest* once the person scrolled up. Over it the toolbar's run state
//! chip ([`toolbar`]); before the run can be shown, why not ([`states`]): loading, couldn't load, queued,
//! no run yet, unreadable. Under it the composer ([`composer`]).
//!
//! The task page (P1) loads the task and hands it here ([`tab`]); the transcript and the run's requests
//! are read here ([`read`]), again as the runner reports changes. The step rail, *Show*, the search and
//! Details are not part of this build (D4): the chat shows every step.

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
use orchester_core::{integration::Leftovers, project::TaskKey, run::RunStatus};
use orchester_workspace::{
  runner::state::RunnerState,
  tasks::{
    conversation::{Conversation, PanelState},
    detail::{RunView, TaskDetail},
    timeline::{
      self as items, Sources,
      view::{self, Choice},
    },
  },
};

use self::{
  read::{Transcript, TranscriptRead, Waiting, with_requests},
  timeline::{Follow, Timeline, Toggles},
  toolbar::RunState,
};
use crate::{
  project::{ShownProject, TasksState},
  services::Services,
  ui::markdown::plain,
};

/// The tab's content for `detail`, the task `key` of `project`, as the task page loaded it.
pub fn tab(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey, detail: Arc<TaskDetail>) -> Element {
  ctx.mount_keyed::<ConversationTab>(
    &format!("task-conversation-{key}"),
    ConversationProps {
      project: project.clone(),
      key: key.clone(),
      detail,
    },
  )
}

#[derive(Clone, PartialEq)]
struct ConversationProps {
  project: ShownProject,
  key: TaskKey,
  detail: Arc<TaskDetail>,
}

impl lurq::app::component::DevtoolsInspectable for ConversationProps {}

struct ConversationTab {
  composer: composer::ComposerLink,
  follow: Follow,
  toggles: Toggles,
  retry: Signal<u64>,
  transcript: TranscriptRead,
}

impl Component for ConversationTab {
  type Props = ConversationProps;

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
    let props = ctx.props::<ConversationProps>().clone();
    let online = props
      .project
      .feed
      .status
      .get()
      .is_some_and(|status| *status.state() == RunnerState::Online);
    let detail = &props.detail;
    // A run whose worktree was removed keeps nothing an agent could continue in: no agent reads messages.
    let removed = detail
      .run
      .as_ref()
      .and_then(|run| run.worktree_removed)
      .map(|_| Leftovers::default());
    let conversation = Conversation::of(detail, detail.lost_its_runner(online), removed.as_ref());
    let composer = composer::dock(ctx, &self.composer, (&props.project, &props.key, detail), &conversation);
    let body = match &detail.run {
      Some(run) => self.run_body(ctx, &props, run, &conversation),
      None => (states::without_run(ctx, detail, &conversation), None),
    };
    let (body, state) = body;
    let state = state.unwrap_or_else(|| RunState::of(&conversation, detail, false));
    Column::new()
      .id("conversation")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
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
    props: &ConversationProps,
    run: &RunView,
    conversation: &Conversation,
  ) -> (Element, Option<RunState>) {
    let feed = &props.project.feed;
    let repo_root: PathBuf = props.project.open.repo_root.clone();
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
    let sources = with_requests(ctx, (repo_root.clone(), run.id(), hold), (entries, waiting, retry));
    let loaded = match (sources.data.flatten(), read.error.or(sources.error)) {
      (Some(loaded), _) => loaded,
      (None, Some(error)) => {
        let failed = states::failed(ctx, run, &error, (&repo_root, &self.retry));
        return (failed, None);
      }
      (None, None) => return (states::loading(ctx, run), None),
    };
    let items = items::build(&Sources {
      run,
      entries: &loaded.entries,
      conversation,
      pending: &loaded.pending,
      decided: &loaded.decided,
      questions: &loaded.questions,
      merge: props.detail.merge.as_ref(),
    });
    let open = |key: &str, default: bool| self.toggles.is_open(key, default);
    let shown = view::shown(&items, &Choice::default(), &open, &plain::rendered);
    // An agent that waits in its turn for the answer to its question does not work: the run waits for the
    // person (C·15), so there is no working line and the chip says so.
    let asking = conversation.state == PanelState::Live && !run.waiting_questions.is_empty();
    let following = self.follow.following.get();
    let state = RunState::of(conversation, &props.detail, asking).following(following);
    if shown.rows.is_empty() {
      return (states::nothing_yet(ctx, run), Some(state));
    }
    ctx.provide(Timeline {
      project: props.project.open.project.id(),
      key: props.key.clone(),
      detail: props.detail.clone(),
      toggles: self.toggles.clone(),
    });
    let working =
      (conversation.state == PanelState::Live && !asking && run.run.status() == &RunStatus::Running).then(|| {
        timeline::Working {
          agent: run
            .current_step()
            .and_then(|step| step.agent.clone())
            .unwrap_or_else(|| "".into()),
          last: loaded.entries.last().map(|entry| entry.at),
        }
      });
    let chat = timeline::chat(ctx, &self.follow, (&shown, &items), working);
    (chat, Some(state))
  }
}
